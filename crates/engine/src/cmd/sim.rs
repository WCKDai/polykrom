//! MuJoCo simulation (`sim.mujoco`, #128, docs/mujoco.md): the design's MJCF export run in MuJoCo
//! by `tools/mujoco/runner.py`, a Python program started as a separate process, as
//! `analysis.static_stress` runs Nightfall: nothing of MuJoCo (Apache-2.0) is linked. Mode `stand`
//! holds the design's pose; mode `study` drives the actuators along a motion study with the base
//! free. Desktop only: `POLYKROM_MUJOCO_PYTHON` names a Python that has MuJoCo (default `python3`).

use polykrom_io::urdf::{Step, UrdfModel};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{bad, num, str_};
use crate::{Result, Session};

const CMD: &str = "sim.mujoco";

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new(CMD, "Simulate in MuJoCo", run).icon("play").noundo().params(
    "mode?: stand|study (default: study with `study`, else stand: hold the design's pose for hold_s, default 2); study?: id or name, \
     run cycles? times (default 1) at step_s? seconds per step (default 0.05) with the base free; settle_s? (default 0.5); fall_tilt_deg? \
     (45), fall_height_mm? (centre of mass; default half its settled height); video?: .mp4 path; mjcf?: a model exported from this design \
     (default: exported with a free base into a new temporary folder); floating?, kp?, damping?, armature?, effort? (file.export's); budget_s? \
     (300) → mjcf, folder, ok, fell, finite, resets, tilt_max_deg, base_z_mm, com_z_mm, actuators [{name, peak_force, force_limit, utilisation, \
     at_limit_s, error_deg|mm, max_error_…}], saturated; study: advance_mm, drift_mm, per_cycle_mm; stand: base_drop_mm; warnings. \
     Needs Python with MuJoCo (POLYKROM_MUJOCO_PYTHON, default python3; pip install mujoco numpy); desktop only; docs/mujoco.md",
)];

fn run(s: &mut Session, p: &Value) -> Result<Value> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (s, p);
        Err(bad(CMD, "not supported yet: MuJoCo runs as a separate program, in the desktop app only"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    native::run(s, p)
}

/// A number parameter from `lo` to `hi`, `default` when absent.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn within(p: &Value, k: &str, default: f64, lo: f64, hi: f64) -> Result<f64> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(default),
        Some(_) => num(p, k).filter(|x| *x >= lo && *x <= hi).ok_or_else(|| bad(CMD, format!("`{k}` must be a number from {lo} to {hi}"))),
    }
}

/// The MJCF options `file.export` takes, from this command's parameters: a free base unless
/// `floating` is false.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) fn export_options(p: &Value) -> Value {
    let mut o = json!({"format": "mjcf", "floating": p.get("floating").cloned().filter(|v| !v.is_null()).unwrap_or(json!(true))});
    for k in ["kp", "damping", "armature", "effort"] {
        if let Some(v) = p.get(k).filter(|v| !v.is_null()) {
            o[k] = v.clone();
        }
    }
    o
}

/// The runner's job for the model at `mjcf`: the mode, its times and limits and, for a study,
/// each actuator's track in the model's units (radians, metres), from `model`, the model's link
/// tree. Keys of joints the model has no actuator for are left out, with a warning.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) fn job(s: &Session, p: &Value, mjcf: &str, model: Option<&UrdfModel>) -> Result<(Value, Vec<String>)> {
    let study = match p.get("study") {
        None | Some(Value::Null) => None,
        Some(_) => s.doc.assembly.studies.get(super::motion::study_index(s, p, CMD)?),
    };
    let mode = match str_(p, "mode") {
        None if study.is_some() => "study",
        None => "stand",
        Some(m @ ("stand" | "study")) => m,
        Some(_) => return Err(bad(CMD, "`mode` must be stand or study")),
    };
    let mut job = json!({
        "mjcf": mjcf,
        "mode": mode,
        "settle_s": within(p, "settle_s", 0.5, 0.0, 60.0)?,
        "fall_tilt_deg": within(p, "fall_tilt_deg", 45.0, 0.0, 180.0)?,
    });
    if p.get("fall_height_mm").is_some_and(|v| !v.is_null()) {
        job["fall_height_mm"] = json!(within(p, "fall_height_mm", 0.0, -1e6, 1e6)?);
    }
    if let Some(v) = str_(p, "video") {
        job["video"] = json!(v);
    }
    if mode == "stand" {
        job["hold_s"] = json!(within(p, "hold_s", 2.0, 0.0, 3600.0)?);
        return Ok((job, Vec::new()));
    }
    let study = study.ok_or_else(|| bad(CMD, "mode study needs `study` (motion.list names them)"))?;
    let model = model.ok_or_else(|| bad(CMD, "a study needs the design's model"))?;
    // One track per actuator: keys of the same joint value (two keys, or a key and a copy) merge.
    let mut tracks: Vec<(String, Vec<[f64; 2]>)> = Vec::new();
    let mut warnings = Vec::new();
    for k in &study.keys {
        let driven = model.joints.iter().find(|j| j.source == Some((k.joint, k.index)) && j.step != Step::Fixed && j.mimic.is_none());
        let Some(j) = driven else {
            let name = s.doc.assembly.joints.iter().find(|j| j.id == k.joint).map_or_else(|| k.joint.to_string(), |j| j.name.clone());
            warnings.push(format!("`{name}` has no actuator in the model (a motion link drives it, or it doesn't move): its keys are left out"));
            continue;
        };
        let scale = if j.step == Step::Prismatic { 1e-3 } else { 1.0 };
        let points = k.points.iter().filter(|(t, v)| t.is_finite() && v.is_finite()).map(|(t, v)| [*t, v * scale]);
        match tracks.iter_mut().find(|(name, _)| *name == j.name) {
            Some((_, have)) => have.extend(points),
            None => tracks.push((j.name.clone(), points.collect())),
        }
    }
    // In step order, a later key at the same step replacing an earlier one.
    let tracks: Vec<Value> = tracks
        .into_iter()
        .filter(|(_, points)| !points.is_empty())
        .map(|(name, mut points)| {
            points.sort_by(|a, b| a[0].total_cmp(&b[0]));
            let mut kept: Vec<[f64; 2]> = Vec::with_capacity(points.len());
            for q in points {
                match kept.last_mut() {
                    Some(last) if last[0] == q[0] => *last = q,
                    _ => kept.push(q),
                }
            }
            json!({"actuator": name, "points": kept})
        })
        .collect();
    job["steps"] = json!(study.steps.max(1));
    job["step_s"] = json!(within(p, "step_s", 0.05, 1e-4, 60.0)?);
    job["cycles"] = json!(within(p, "cycles", 1.0, 1.0, 1000.0)?.round());
    job["tracks"] = Value::Array(tracks);
    Ok((job, warnings))
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::Read;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};

    use super::{CMD, export_options, job, within};
    use crate::params::{bad, str_};
    use crate::{EngineError, Result, Session};

    /// The runner, shipped inside the program and written next to each job.
    const RUNNER: &str = include_str!("../../../../tools/mujoco/runner.py");
    /// Most bytes read from the runner's output and error streams.
    const MAX_OUTPUT: u64 = 16 << 20;

    pub(super) fn run(s: &mut Session, p: &Value) -> Result<Value> {
        let checked = |path: &str| polykrom_io::vfs::check(path).map_err(|e| EngineError::Other(e.to_string()));
        // The video written where it was asked for, not in the runner's folder.
        let mut p = p.clone();
        if let Some(v) = str_(&p, "video").map(str::to_string) {
            checked(&v)?;
            if Path::new(&v).is_relative() {
                let here = std::env::current_dir().map_err(|e| bad(CMD, format!("video: {e}")))?;
                p["video"] = json!(here.join(&v).to_string_lossy());
            }
        }
        let p = &p;
        // The model given, checked against the agent's folders and then used by that same resolved
        // path (the runner starts in its own folder, so a relative path would point elsewhere).
        let given = match str_(p, "mjcf").filter(|m| !m.trim().is_empty() && m.len() < 4096) {
            Some(m) => {
                checked(m)?;
                let full = std::fs::canonicalize(m).map_err(|_| bad(CMD, format!("no MJCF model at {m}")))?;
                if !full.is_file() {
                    return Err(bad(CMD, format!("no MJCF model at {m}")));
                }
                let full = full.to_string_lossy().into_owned();
                checked(&full)?;
                Some(full)
            }
            None => None,
        };
        let given = given.as_deref();
        if given.is_none() && s.doc.assembly.joints.is_empty() {
            return Err(bad(CMD, "the design has no joints: make the robot components joined with joint.create (or give `mjcf`)"));
        }
        let budget = Duration::from_secs_f64(within(p, "budget_s", 300.0, 1.0, 86_400.0)?);
        // A new folder, kept, so the model can be looked at and the job run again.
        let dir = super::super::analysis::native::scratch(CMD, "mujoco")?;
        let mut warnings = Vec::new();
        let (mjcf, model) = match given {
            Some(m) => {
                let study = p.get("study").is_some_and(|v| !v.is_null());
                (m.to_string(), if study { Some(super::super::file::mjcf_package(s, m, &export_options(p))?.model) } else { None })
            }
            None => {
                let stem: String = s.doc.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).take(64).collect();
                let path = dir.join(format!("{}.xml", if stem.is_empty() { "robot" } else { &stem }));
                let pkg = super::super::file::mjcf_package(s, &path.to_string_lossy(), &export_options(p))?;
                for (name, bytes) in &pkg.meshes {
                    write(&dir.join(name), bytes)?;
                }
                write(&path, pkg.xml.as_bytes())?;
                warnings.extend(pkg.warnings);
                (path.to_string_lossy().into_owned(), Some(pkg.model))
            }
        };
        let (job, skipped) = job(s, p, &mjcf, model.as_ref())?;
        warnings.extend(skipped);
        write(&dir.join("runner.py"), RUNNER.as_bytes())?;
        write(&dir.join("job.json"), job.to_string().as_bytes())?;
        let mut out = runner(&dir, budget)?;
        if let Some(message) = out.get("error").and(out.get("message")).and_then(Value::as_str) {
            let hint = if out["error"] == "mujoco_missing" { " (or set POLYKROM_MUJOCO_PYTHON to a Python that has it)" } else { "" };
            return Err(bad(CMD, format!("{message}{hint}")));
        }
        if let Some(o) = out.as_object_mut() {
            o.insert("mjcf".into(), json!(mjcf));
            o.insert("folder".into(), json!(dir));
            o.insert("warnings".into(), json!(warnings));
        }
        Ok(out)
    }

    fn write(path: &Path, bytes: &[u8]) -> Result<()> {
        std::fs::write(path, bytes).map_err(|e| bad(CMD, format!("{}: {e}", path.display())))
    }

    /// Runs `runner.py job.json` in `dir` within `budget` and reads the JSON it prints last.
    fn runner(dir: &Path, budget: Duration) -> Result<Value> {
        let default = if cfg!(windows) { "python" } else { "python3" };
        let python = crate::geom::env::var_os("MUJOCO_PYTHON").filter(|v| !v.is_empty()).unwrap_or_else(|| default.into());
        let mut child = std::process::Command::new(&python)
            .arg(dir.join("runner.py"))
            .arg(dir.join("job.json"))
            .current_dir(dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                let python = python.to_string_lossy();
                bad(
                    CMD,
                    format!("cannot start Python `{python}` ({e}): install Python 3 and `pip install mujoco numpy`, or set POLYKROM_MUJOCO_PYTHON"),
                )
            })?;
        // Both streams are read as they come, so a long answer never fills a pipe and stalls it.
        let drain = |stream: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut text = String::new();
                if let Some(s) = stream {
                    let _ = s.take(MAX_OUTPUT).read_to_string(&mut text);
                }
                text
            })
        };
        let stdout = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
        let stderr = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if crate::cancel::requested() => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(crate::cancel::error(CMD));
                }
                Ok(None) if started.elapsed() > budget => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(bad(CMD, format!("the simulation ran past its budget of {} s (budget_s)", budget.as_secs())));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(bad(CMD, format!("waiting for the MuJoCo runner: {e}")));
                }
            }
        };
        let (text, errors) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
        let status = status?;
        // The result is the last line; MuJoCo may have printed warnings before it.
        match text.lines().rev().find(|l| !l.trim().is_empty()).and_then(|l| serde_json::from_str::<Value>(l).ok()) {
            Some(v) if v.is_object() => Ok(v),
            _ => {
                let tail: String = errors.chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect();
                Err(bad(CMD, format!("the MuJoCo runner gave no result ({status}): {}", tail.trim())))
            }
        }
    }
}

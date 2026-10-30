//! MJCF export (#27, docs/mjcf.md): bodies, hinges with their ranges, masses, actuators and the
//! free root. The models load and hold their pose in MuJoCo (the PR's evidence); these tests
//! check what the file says.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn temp(tag: &str) -> PathBuf {
    let d = polykrom_io::vfs::temp_dir().join(format!("solidcraft-mjcf-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::create_dir_all(&d);
    d
}

/// A two-link arm in aluminium: a grounded `Base` and an `Arm` on the revolute joint `Elbow`
/// (±60°) about x, each a 40 x 30 x 3 plate (3600 mm³) with an r4 x 40 knuckle (640π mm³) as a
/// second body, driven to 30° so the arm hangs out under gravity.
fn arm() -> Session {
    let mut s = Session::default();
    for name in ["Base", "Arm"] {
        run(&mut s, "component.activate", json!({"component": "root"}));
        run(&mut s, "component.create", json!({"name": name}));
        run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 3, "corner": [0, 0, 0]}));
        run(&mut s, "solid.cylinder", json!({"base": [0, 33, 4], "axis": [1, 0, 0], "radius": 4, "height": 40, "operation": "new"}));
    }
    let bodies: Vec<String> = s.model.state().bodies.iter().map(|b| b.name.clone()).collect();
    run(&mut s, "material.assign", json!({"bodies": bodies, "material": "Aluminum"}));
    let base = s.doc.find_component("Base").and_then(|c| s.doc.occurrence_of(c)).map(|o| o.id).unwrap_or(0);
    run(&mut s, "occurrence.ground", json!({"occurrence": base, "grounded": true}));
    run(
        &mut s,
        "joint.create",
        json!({"type": "revolute", "a": {"occurrence": "Base", "circle": [1, 37, 4]}, "b": {"occurrence": "Arm", "circle": [1, 37, 4]},
               "flip": true, "name": "Elbow", "limits": [[-60, 60]]}),
    );
    run(&mut s, "joint.drive", json!({"joint": "Elbow", "value": 30}));
    s
}

/// Export to `dir/name` and read the XML back.
fn export(s: &mut Session, dir: &std::path::Path, name: &str, extra: Value) -> (Value, String) {
    let path = dir.join(name);
    let mut p = json!({"path": path.to_string_lossy(), "format": "mjcf"});
    if let (Some(p), Some(extra)) = (p.as_object_mut(), extra.as_object()) {
        p.extend(extra.clone());
    }
    let r = run(s, "file.export", p);
    let xml = std::fs::read_to_string(&path).unwrap_or_default();
    (r, xml)
}

/// The `mass` of the body named `body`'s inertial.
fn mass(xml: &str, body: &str) -> f64 {
    let at = xml.find(&format!("<body name=\"{body}\"")).unwrap_or_else(|| panic!("no body {body} in\n{xml}"));
    let rest = &xml[at..];
    let m = rest.find("mass=\"").map(|i| &rest[i + 6..]).unwrap_or("");
    m.split('"').next().and_then(|v| v.parse().ok()).unwrap_or(f64::NAN)
}

#[test]
fn mjcf_of_a_two_link_arm() {
    let mut s = arm();
    let d = temp("arm");
    let (r, xml) = export(&mut s, &d, "arm.xml", json!({}));
    assert_eq!(r["format"], "Mjcf", "{r}");
    let mut files: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    files.sort();
    assert_eq!(files, ["arm-Arm.stl", "arm-Base.stl", "arm.xml"]);
    super::urdf_tests::well_formed(&xml).unwrap_or_else(|e| panic!("{e}\n{xml}"));
    let (lo, hi, q) = ((-60f64).to_radians(), 60f64.to_radians(), 30f64.to_radians());
    for want in [
        r#"<mujoco model="arm">"#.to_string(),
        r#"<compiler angle="radian" meshdir="." balanceinertia="true"/>"#.into(),
        r#"<mesh name="arm-Arm" file="arm-Arm.stl"/>"#.into(),
        r#"<body name="base_link" pos="0 0 0" quat="1 0 0 0">"#.into(),
        r#"<body name="Base" "#.into(),
        r#"<body name="Arm" "#.into(),
        format!(r#"<joint name="Elbow" type="hinge" axis="0 0 1" ref="{q}" range="{lo} {hi}" limited="true"/>"#),
        r#"<geom name="Arm" type="mesh" mesh="arm-Arm" "#.into(),
        format!(
            r#"<position name="Elbow" joint="Elbow" kp="1000" dampratio="1" ctrlrange="{lo} {hi}" ctrllimited="true" forcerange="-1000 1000" forcelimited="true"/>"#
        ),
        format!(r#"<key name="design" qpos="{q}" ctrl="{q}"/>"#),
        r#"<geom name="floor" type="plane""#.into(),
        r#"<joint damping="0.1" armature="0.01"/>"#.into(),
    ] {
        assert!(xml.contains(&want), "{want} missing from\n{xml}");
    }
    // The arm hangs in the base, and the base in base_link: welded, with no joint of its own.
    let (base, arm, elbow) = (xml.find(r#"<body name="Base""#), xml.find(r#"<body name="Arm""#), xml.find(r#"<joint name="Elbow""#));
    assert!(base < arm && arm < elbow, "{xml}");
    assert!(!xml.contains("freejoint"), "grounded: fixed in the world\n{xml}");
    // MuJoCo would let the arm collide with a base welded to the world: the pair is excluded.
    assert!(
        xml.contains(
            r#"<contact>
    <exclude body1="Base" body2="Arm"/>
  </contact>"#
        ),
        "{xml}"
    );
    // Mass is density x volume: 2.7 g/cm³ over 3600 + 640π mm³. Tolerance 0.5%: the knuckle is
    // a tessellated cylinder, which loses under 0.3% of its 2011 mm³.
    let want = 2.7e-6 * (3600.0 + 640.0 * std::f64::consts::PI);
    for body in ["Base", "Arm"] {
        let m = mass(&xml, body);
        assert!((m - want).abs() < 5e-3 * want, "{body}: {m} kg, expected {want}");
    }
    let warnings = r["warnings"].as_array().cloned().unwrap_or_default();
    assert!(warnings.iter().any(|w| w.as_str().is_some_and(|w| w.contains("no rated actuator drives Elbow"))), "{r}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A rated library actuator at the joint limits its actuator's force; `effort` limits the rest.
#[test]
fn mjcf_actuator_force_comes_from_ratings() {
    let mut s = arm();
    let d = temp("ratings");
    let (_, xml) = export(&mut s, &d, "a.xml", json!({"effort": 4.5, "kp": 7}));
    assert!(xml.contains(r#"kp="7" dampratio="1""#) && xml.contains(r#"forcerange="-4.5 4.5""#), "{xml}");
    let ratings = json!({"kind": "servo", "stall_torque_nm": 2.94, "speed_rpm": 50});
    let values = serde_json::Map::from_iter([(crate::cmd::parts::RATINGS_KEY.to_string(), ratings)]);
    run(&mut s, "component.metadata.set", json!({"component": "Arm", "values": values}));
    let (r, xml) = export(&mut s, &d, "a.xml", json!({"effort": 4.5}));
    assert!(xml.contains(r#"forcerange="-2.94 2.94""#), "{xml}");
    assert!(r["warnings"].as_array().is_some_and(|w| w.iter().all(|w| !w.as_str().unwrap_or("").contains("rated"))), "{r}");
    for bad in [json!({"kp": 0}), json!({"floating": "yes"}), json!({"bodies": ["Body1"]})] {
        let mut p = json!({"path": d.join("b.xml").to_string_lossy(), "format": "mjcf"});
        if let (Some(p), Some(bad)) = (p.as_object_mut(), bad.as_object()) {
            p.extend(bad.clone());
        }
        assert!(s.execute("file.export", &p).is_err(), "{p}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The reply says what the model holds, as the file does, for an agent that can't read the file;
/// a package goes into a folder that doesn't exist yet (#171).
#[test]
fn mjcf_reply_says_what_the_model_holds() {
    let mut s = arm();
    let ratings = json!({"kind": "servo", "stall_torque_nm": 1.912, "speed_rpm": 50});
    let values = serde_json::Map::from_iter([(crate::cmd::parts::RATINGS_KEY.to_string(), ratings)]);
    run(&mut s, "component.metadata.set", json!({"component": "Arm", "values": values}));
    let d = temp("reply");
    let (r, xml) = export(&mut s, &d, "dog-mjcf/meshes-here/arm.xml", json!({}));
    assert!(xml.contains(r#"forcerange="-1.912 1.912""#), "{xml}");
    let files = r["files"].as_array().map_or(0, Vec::len);
    assert_eq!(files, 3, "{r}");
    for (k, want) in [
        ("bodies", json!(3)),
        ("joints", json!(1)),
        ("actuators", json!(1)),
        ("meshes", json!(2)),
        ("free_root", json!(false)),
        ("keyframe", json!(true)),
    ] {
        assert_eq!(r[k], want, "{k}: {r}");
    }
    let a = &r["actuator_ranges"];
    assert_eq!(a.as_array().map(Vec::len), Some(1), "{r}");
    assert_eq!((&a[0]["name"], &a[0]["joint"], &a[0]["force"]), (&json!("Elbow"), &json!("Elbow"), &json!([-1.912, 1.912])), "{r}");
    // ±60° in radians, to the reply's 1e-6 rounding.
    let lim = 60f64.to_radians();
    for (k, want) in [(0, -lim), (1, lim)] {
        assert!((a[0]["ctrl"][k].as_f64().unwrap_or(0.0) - want).abs() < 1e-6, "{r}");
    }
    // The total is every body's `mass` in the file (tolerance: the reply's 1e-6 kg rounding).
    let total: f64 = xml.split("mass=\"").skip(1).filter_map(|m| m.split('"').next()?.parse::<f64>().ok()).sum();
    assert!((r["mass_kg"].as_f64().unwrap_or(0.0) - total).abs() < 1e-6, "{total} kg: {r}");
    let (r, _) = export(&mut s, &d, "f.xml", json!({"floating": true}));
    assert_eq!(r["free_root"], true, "{r}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Nothing grounded, or `floating`: the root is free, so the robot falls onto the floor.
#[test]
fn mjcf_floating_base_gets_a_freejoint() {
    let mut s = arm();
    let d = temp("free");
    let (_, xml) = export(&mut s, &d, "f.mjcf", json!({"floating": true, "scene": false}));
    let q = 30f64.to_radians();
    assert!(xml.contains(r#"<freejoint name="root"/>"#), "{xml}");
    assert!(xml.contains(&format!(r#"<key name="design" qpos="0 0 0 1 0 0 0 {q}" ctrl="{q}"/>"#)), "{xml}");
    assert!(!xml.contains("floor") && !xml.contains("<light") && !xml.contains("<exclude"), "{xml}");
    // The freejoint is on the root body, before any other joint.
    assert!(xml.find("<freejoint").zip(xml.find(r#"<body name="Base""#)).is_some_and(|(f, b)| f < b), "{xml}");

    let base = s.doc.find_component("Base").and_then(|c| s.doc.occurrence_of(c)).map(|o| o.id).unwrap_or(0);
    run(&mut s, "occurrence.ground", json!({"occurrence": base, "grounded": false}));
    let (_, xml) = export(&mut s, &d, "g.xml", json!({}));
    assert!(xml.contains(r#"<freejoint name="root"/>"#), "nothing grounded\n{xml}");
    let (_, xml) = export(&mut s, &d, "h.xml", json!({"floating": false}));
    assert!(!xml.contains("freejoint"), "{xml}");
    let _ = std::fs::remove_dir_all(&d);
}

/// `sim.mujoco` (#128, docs/mujoco.md): the arm with the study `Swing`, Elbow 30° → 55° → 30°
/// over 20 steps (up, away from the floor).
fn swinging_arm() -> Session {
    let mut s = arm();
    run(&mut s, "motion.study", json!({"name": "Swing", "steps": 20, "keys": [{"joint": "Elbow", "points": [[20, 30], [0, 30], [10, 55]]}]}));
    s
}

/// The job `sim.mujoco` gives the runner: the study's keys become the actuator's track in
/// radians, sorted by step, with the times and limits; no MuJoCo is needed.
#[test]
fn mujoco_job_of_a_study() {
    let s = swinging_arm();
    let model = super::mjcf_package(&s, "arm.xml", &crate::cmd::sim::export_options(&json!({}))).unwrap().model;
    let p = json!({"study": "Swing", "cycles": 2, "step_s": 0.02, "video": "swing.mp4"});
    let (job, warnings) = crate::cmd::sim::job(&s, &p, "arm.xml", Some(&model)).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let tracks = job["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1, "{job}");
    assert_eq!(tracks[0]["actuator"], "Elbow", "{job}");
    let points: Vec<(f64, f64)> = tracks[0]["points"].as_array().unwrap().iter().map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap())).collect();
    let (a, b, tol) = (30f64.to_radians(), 55f64.to_radians(), 1e-12);
    for ((step, value), (want_step, want)) in points.iter().zip([(0.0, a), (10.0, b), (20.0, a)]) {
        assert!((step - want_step).abs() < tol && (value - want).abs() < tol, "{points:?}");
    }
    assert_eq!(points.len(), 3);
    let mut rest = job.clone();
    rest.as_object_mut().unwrap().remove("tracks");
    assert_eq!(
        rest,
        json!({"mjcf": "arm.xml", "mode": "study", "settle_s": 0.5, "fall_tilt_deg": 45.0, "video": "swing.mp4", "steps": 20, "step_s": 0.02, "cycles": 2.0})
    );

    // Without a study it holds the design's pose.
    let (stand, _) = crate::cmd::sim::job(&s, &json!({"hold_s": 3, "fall_height_mm": 40}), "arm.xml", None).unwrap();
    // A second key on the same joint value merges into its actuator's one track, in step order.
    let mut twice = Session::new((*s.doc).clone());
    if let Some(study) = twice.doc_mut().assembly.studies.iter_mut().find(|st| st.name == "Swing") {
        let mut extra = study.keys[0].clone();
        extra.points = vec![(15.0, 40f64.to_radians())];
        study.keys.push(extra);
    }
    let (merged, _) = crate::cmd::sim::job(&twice, &p, "arm.xml", Some(&model)).unwrap();
    assert_eq!(merged["tracks"].as_array().map(Vec::len), Some(1), "{merged}");
    let steps: Vec<f64> = merged["tracks"][0]["points"].as_array().unwrap().iter().map(|q| q[0].as_f64().unwrap()).collect();
    assert_eq!(steps, [0.0, 10.0, 15.0, 20.0]);
    assert_eq!(stand, json!({"mjcf": "arm.xml", "mode": "stand", "settle_s": 0.5, "fall_tilt_deg": 45.0, "fall_height_mm": 40.0, "hold_s": 3.0}));
    for bad in [
        json!({"mode": "walk"}),
        json!({"mode": "study"}),
        json!({"study": "Nope"}),
        json!({"study": "Swing", "cycles": 0}),
        json!({"step_s": -1, "study": "Swing"}),
    ] {
        assert!(crate::cmd::sim::job(&s, &bad, "arm.xml", Some(&model)).is_err(), "{bad}");
    }
}

/// The runner on the exported arm, fixed to the world: it holds its 30° and swings through the
/// study. Skipped, with a note, where Python has no MuJoCo (`POLYKROM_MUJOCO_PYTHON`, else python3).
#[test]
fn mujoco_runs_the_arm() {
    let python = crate::geom::env::var_os("MUJOCO_PYTHON").unwrap_or_else(|| "python3".into());
    if !std::process::Command::new(&python).args(["-c", "import mujoco, numpy"]).status().is_ok_and(|s| s.success()) {
        eprintln!("skipped: {} has no MuJoCo (pip install mujoco numpy, or set POLYKROM_MUJOCO_PYTHON)", python.to_string_lossy());
        return;
    }
    let mut s = swinging_arm();
    let stand = run(&mut s, "sim.mujoco", json!({"floating": false}));
    assert_eq!(stand["mode"], "stand", "{stand}");
    assert_eq!((stand["ok"].as_bool(), stand["finite"].as_bool(), stand["resets"].as_u64()), (Some(true), Some(true), Some(0)), "{stand}");
    assert!(stand["fell"].is_null() && stand["free_base"] == false, "a fixed base can't fall: {stand}");
    let elbow = &stand["actuators"][0];
    assert_eq!(elbow["name"], "Elbow", "{stand}");
    // Gravity on a 15 g arm against a 1000 N m/rad servo: far under a hundredth of a degree.
    assert!(elbow["error_deg"].as_f64().is_some_and(|e| e < 0.01), "{stand}");
    assert!(elbow["peak_force"].as_f64().zip(elbow["force_limit"].as_f64()).is_some_and(|(f, l)| f > 0.0 && f < l), "{stand}");
    let mjcf = stand["mjcf"].as_str().unwrap_or_default().to_string();
    assert!(std::path::Path::new(&mjcf).is_file(), "{stand}");

    // The study on the model just exported.
    let swing = run(&mut s, "sim.mujoco", json!({"study": "Swing", "mjcf": mjcf, "step_s": 0.05, "cycles": 2}));
    assert_eq!((swing["mode"].as_str(), swing["ok"].as_bool(), swing["cycles"].as_u64()), (Some("study"), Some(true), Some(2)), "{swing}");
    // 2 cycles of 20 steps of 0.05 s after 0.5 s settling.
    assert!(swing["seconds"].as_f64().is_some_and(|t| (t - 2.5).abs() < 0.01), "{swing}");
    // A critically damped servo lags a ramp by 2 sqrt(J / kp) × speed: about 0.3° at 25° per 0.5 s
    // with the export's 0.01 kg m² armature.
    let elbow = &swing["actuators"][0];
    assert!(elbow["max_error_deg"].as_f64().is_some_and(|e| e > 0.1 && e < 1.0), "{swing}");
    let _ = std::fs::remove_dir_all(swing["folder"].as_str().unwrap_or_default());
    let _ = std::fs::remove_dir_all(stand["folder"].as_str().unwrap_or_default());
}

/// The `name="…"` attribute's numbers in the inertial of the body named `body`.
fn inertial(xml: &str, body: &str, name: &str) -> Vec<f64> {
    let at = xml.find(&format!("<body name=\"{body}\"")).unwrap_or_else(|| panic!("no body {body} in\n{xml}"));
    let rest = &xml[at..];
    let rest = &rest[rest.find("<inertial").unwrap_or(0)..];
    let v = rest.find(&format!(" {name}=\"")).map(|i| &rest[i + name.len() + 3..]).unwrap_or("");
    v.split('"').next().unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect()
}

/// Every edge of a binary STL's mesh (corners equal to the bit) shared by exactly two triangles
/// running it opposite ways: a closed, consistently oriented mesh.
fn stl_closed(bytes: &[u8]) -> bool {
    let n = bytes.get(80..84).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut index: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
    let mut half: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for t in 0..n {
        let mut ids = [0u32; 3];
        for (k, id) in ids.iter_mut().enumerate() {
            let o = 84 + t * 50 + 12 + k * 12;
            let p = [0, 4, 8].map(|d| bytes.get(o + d..o + d + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])));
            let next = index.len() as u32;
            *id = *index.entry(p).or_insert(next);
        }
        for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
            *half.entry((a, b)).or_insert(0) += 1;
        }
    }
    n > 0 && half.iter().all(|(&(a, b), &c)| c == 1 && half.get(&(b, a)) == Some(&1))
}

/// A hollow box (#216): 40 x 30 x 20 with a 36 x 26 x 16 void 2 mm in from every side, in
/// aluminium. Its body's mass and inertia tensor about the centre are the outer box's less the
/// void's, m = ρ(V₁ - V₂) and I_xx = ρ/12 (V₁(b₁² + c₁²) - V₂(b₂² + c₂²)) and so on, and its
/// mesh is closed (MuJoCo works out nothing from it).
#[test]
fn mjcf_hollow_shell_has_its_mass_and_inertia_and_a_closed_mesh() {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "Shell"}));
    run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 20, "corner": [0, 0, 0], "body_name": "Hollow"}));
    run(&mut s, "solid.box", json!({"length": 36, "width": 26, "height": 16, "corner": [2, 2, 2], "operation": "cut"}));
    run(&mut s, "material.assign", json!({"bodies": ["Hollow"], "material": "Aluminum"}));
    let occ = s.doc.find_component("Shell").and_then(|c| s.doc.occurrence_of(c)).map(|o| o.id).unwrap_or(0);
    run(&mut s, "occurrence.ground", json!({"occurrence": occ, "grounded": true}));
    let d = temp("hollow");
    let (r, xml) = export(&mut s, &d, "h.xml", json!({}));
    let warnings = r["warnings"].as_array().cloned().unwrap_or_default();
    assert!(!warnings.iter().any(|w| w.as_str().is_some_and(|w| w.contains("closed"))), "{r}");
    // Aluminium, 2.7 g/cm³ = 2.7e-6 kg/mm³; the tensor in kg m² (1e-6 per kg mm²).
    let rho = 2.7e-6;
    let (v1, v2) = (40.0 * 30.0 * 20.0, 36.0 * 26.0 * 16.0);
    let want_mass = rho * (v1 - v2);
    let moment = |b1: f64, c1: f64, b2: f64, c2: f64| rho / 12.0 * (v1 * (b1 * b1 + c1 * c1) - v2 * (b2 * b2 + c2 * c2)) * 1e-6;
    let want = [moment(30.0, 20.0, 26.0, 16.0), moment(40.0, 20.0, 36.0, 16.0), moment(40.0, 30.0, 36.0, 26.0)];
    let m = inertial(&xml, "Shell", "mass");
    assert!((m[0] - want_mass).abs() < 1e-6 * want_mass, "{m:?} vs {want_mass}");
    let full = inertial(&xml, "Shell", "fullinertia");
    assert_eq!(full.len(), 6, "{xml}");
    for (k, w) in want.iter().enumerate() {
        assert!((full[k] - w).abs() < 1e-6 * w, "I[{k}] {} vs {w}\n{xml}", full[k]);
    }
    for off in &full[3..] {
        assert!(off.abs() < 1e-9 * want[0], "products of inertia vanish: {full:?}");
    }
    // Its centre: the box's, (20, 15, 10) mm, in the link frame (the component sits at the origin).
    let pos = inertial(&xml, "Shell", "pos");
    for (k, w) in [0.020, 0.015, 0.010].iter().enumerate() {
        assert!((pos[k] - w).abs() < 1e-9, "{pos:?}");
    }
    let stl = std::fs::read(d.join("h-Shell.stl")).unwrap_or_default();
    assert!(stl_closed(&stl), "the shell's mesh is closed");
    let _ = std::fs::remove_dir_all(&d);
}

/// Each rated servo's gain is scaled to its rating against the strongest one's, so a micro servo
/// (TinyRex's SG90 jaw among STS3215s) reaches its force limit at the same error as the big ones
/// instead of a hundredth of a degree off its target; a gain asked for stays one for all.
#[test]
fn mjcf_gains_follow_the_ratings() {
    let mut s = arm();
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "component.create", json!({"name": "Jaw"}));
    run(&mut s, "solid.box", json!({"length": 20, "width": 10, "height": 3, "corner": [42, 30, 0]}));
    run(&mut s, "solid.cylinder", json!({"base": [42, 33, 4], "axis": [1, 0, 0], "radius": 4, "height": 10, "operation": "new"}));
    run(
        &mut s,
        "joint.create",
        json!({"type": "revolute", "a": {"occurrence": "Jaw", "circle": [43, 37, 4]}, "b": {"occurrence": "Arm", "circle": [1, 37, 4]},
               "flip": true, "name": "Hinge", "limits": [[-30, 30]]}),
    );
    // A joint's rating is its `a` side's (the servo's), then its `b` side's.
    for (component, nm) in [("Arm", 1.912), ("Jaw", 0.177)] {
        let ratings = json!({"kind": "servo", "stall_torque_nm": nm, "speed_rpm": 50});
        let values = serde_json::Map::from_iter([(crate::cmd::parts::RATINGS_KEY.to_string(), ratings)]);
        run(&mut s, "component.metadata.set", json!({"component": component, "values": values}));
    }
    let d = temp("gains");
    let (_, xml) = export(&mut s, &d, "a.xml", json!({}));
    let kp = |joint: &str| -> f64 {
        let at = xml.find(&format!(r#"<position name="{joint}""#)).unwrap_or_else(|| panic!("{joint}: {xml}"));
        let rest = &xml[at..];
        let k = rest.find("kp=\"").unwrap() + 4;
        rest[k..k + rest[k..].find('"').unwrap()].parse().unwrap()
    };
    assert_eq!(kp("Elbow"), 1000.0);
    assert!((kp("Hinge") - 1000.0 * 0.177 / 1.912).abs() < 0.01, "{}", kp("Hinge"));
    let (_, xml) = export(&mut s, &d, "b.xml", json!({"kp": 7}));
    assert_eq!(xml.matches(r#"kp="7""#).count(), 2, "{xml}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn mjcf_writes_a_loop_closure_and_leaves_passive_followers_unactuated() {
    let mut s = arm();
    // A second revolute joint between the same two links closes a loop; the exporter must not
    // throw it away as a URDF tree would.
    run(
        &mut s,
        "joint.create",
        json!({"type": "revolute", "a": {"occurrence": "Base", "circle": [1, 37, 4]},
               "b": {"occurrence": "Arm", "circle": [1, 37, 4]}, "flip": true, "name": "LoopPin"}),
    );
    let d = temp("loop");
    let (r, xml) = export(&mut s, &d, "loop.xml", json!({"actuated": ["Elbow"]}));
    assert_eq!(r["format"], "Mjcf", "{r}");
    assert_eq!(r["loops"].as_array().map(Vec::len), Some(1), "{r}");
    assert_eq!(r["actuators"], json!(1), "{r}");
    assert!(xml.contains("<connect body1="), "{xml}");
    assert!(!xml.contains("closes a loop"), "{xml}");
    let actuators = r["actuator_ranges"].as_array().unwrap();
    assert_eq!(actuators.len(), 1, "{r}");
    assert_eq!(actuators[0]["joint"], "Elbow", "{r}");
}

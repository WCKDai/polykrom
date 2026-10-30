//! Applying `inspect.polish`'s safe fixes (#193): its fillets and chamfers, all in one batch,
//! checked once. Only when the batch fails a check is it taken back and split: the fixes on the
//! bodies the failed check points at are settled each body on its own after the rest together, and
//! a batch on one body is halved, down to the fixes at fault. N fixes so cost one check plus a few
//! for each body at fault, instead of one per fix. A fix the kernel refuses (a fillet on a curved
//! edge, #184) changes nothing and is listed as skipped; one that leaves a body invalid, makes a
//! new clash with a neighbour, breaks a joint on the bodies it touches, moves a body's mass by
//! more than `mass_tol` or lowers its polish score is rolled back and listed with the reason.
//! Edges on the faces a joint origin names are left out before anything runs. The checks look at
//! the touched bodies, their joints and their neighbours only. Fixes that reshape a part stay
//! recommendations. The whole run is one step to undo.

use std::collections::{BTreeMap, BTreeSet};

use polykrom_doc::joints::Snap;
use serde_json::{Map, Value, json};

use super::CommandSpec;
use crate::params::{bad, bool_, num};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("polish.apply", "Apply Polish", apply).icon("polish").params(
    "fixes?: [ids from inspect.polish, such as \"F1\"] | categories?: [look|print|strength] (default every safe fix); bodies?, printed?, \
     skip?, sharp_deg?, style? (as inspect.polish: faceted applies chamfers where organic applies fillets); max_fixes?: how many of inspect.polish's fixes to try, 1…100 (default 20); mass_tol?: the \
     share of a body's volume a fix may change, 0…1 (default 0.05); batch?: bool (default true: every fix at once, checked once, and \
     split by the bodies a failed check points at, then in halves; false: one fix at a time, each checked) → before: {score, scores}, after: {score, scores}, \
     applied: [{id, body, command, params, score_before, score_after, ms (in the command), fallback? (a fillet the kernel refused, applied as a chamfer \
     as wide as its radius, and why)}], skipped: [{id, body, command, reason, ms?}] (refused by the kernel (remembered by body, command and parameters, so a \
     later run skips it at once), \
     every edge on a face a joint origin names, invalid body, new interference, joints no longer resolve, mass changed, score fell, or already fixed \
     by an earlier fix), held_edges: [{id, edges}] (edges left out of a fix because they lie on a face a joint origin names), fixes (what \
     inspect.polish still suggests), style, colours (as inspect.polish reports them after the run), fixes_left (how many of them are safe: run again), recommendations: [findings (of `categories` when given), most \
     severe first; fix: [ids] on one a listed safe fix still covers], checks (how many times touched \
     bodies were checked), ms: {inspect, fixes, checks}. A fix on a body an earlier fix changed is worked out again from the body as it \
     is now. One undo step",
)];

/// The inspect.polish parameters a run passes on.
const PASSED: [&str; 5] = ["bodies", "printed", "skip", "sharp_deg", "style"];
/// The categories inspect.polish reports.
const CATEGORIES: [&str; 6] = ["look", "print", "strength", "mass", "tidy", "assembly"];

/// A list of strings parameter (`None`: not given); anything else in it is refused, so a
/// malformed selection never widens to every fix.
fn strings(p: &Value, k: &str, cmd: &str) -> Result<Vec<String>> {
    match p.get(k).filter(|v| !v.is_null()) {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => {
            a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(cmd, format!("`{k}` must list strings")))).collect()
        }
        Some(_) => Err(bad(cmd, format!("`{k}` must be a list of strings"))),
    }
}

/// How far a fix may lower a body's score (rounding) or deepen a clash (mm) and still stay.
const SCORE_SLACK: f64 = 0.5;
const DEPTH_SLACK: f64 = 0.05;
/// How near (mm) another body's box must come to a touched body's to be checked for clashes.
const NEIGHBOUR_GAP: f64 = 1.0;
/// Refusals remembered; the memory starts over when full.
const MAX_REMEMBERED: usize = 10_000;

/// One of inspect.polish's fixes.
struct Fix {
    id: Value,
    body: String,
    command: String,
    category: Value,
    kind: Value,
    size: Option<f64>,
    params: Value,
}

/// A touched body as a check sees it.
struct BodyState {
    volume: Option<f64>,
    invalid: usize,
    score: Option<f64>,
}

/// What a batch is checked against: its bodies, the joints on them and the clashes around them.
struct State {
    bodies: BTreeMap<String, BodyState>,
    joint_errors: BTreeSet<u64>,
    /// The touched bodies each checked joint holds.
    joint_bodies: BTreeMap<u64, BTreeSet<String>>,
    /// Every placement of a touched body's part, with the touched body it is.
    owner: BTreeMap<String, String>,
    clashes: BTreeMap<(String, String), f64>,
}

/// A run's settings and what it has come to so far.
struct Run {
    ask: Value,
    mass_tol: f64,
    applied: Vec<Value>,
    skipped: Vec<Value>,
    /// Fixes settled as skipped: never tried again.
    out: BTreeSet<usize>,
    /// Bodies a kept fix changed.
    changed: BTreeSet<String>,
    checks: usize,
    /// Time (ms) in inspect.polish, in the fixes and in the checks.
    ms: [f64; 3],
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "polish.apply";
    let started = web_time::Instant::now();
    let mut ask: Map<String, Value> = PASSED.iter().filter_map(|k| Some((k.to_string(), p.get(*k).filter(|v| !v.is_null())?.clone()))).collect();
    let max = match p.get("max_fixes").filter(|v| !v.is_null()) {
        None => 20.0,
        Some(_) => num(p, "max_fixes").filter(|v| (1.0..=100.0).contains(v)).ok_or_else(|| bad(cmd, "`max_fixes` must be 1 to 100"))?,
    };
    let mass_tol = match p.get("mass_tol").filter(|v| !v.is_null()) {
        None => 0.05,
        Some(_) => num(p, "mass_tol").filter(|v| (0.0..=1.0).contains(v)).ok_or_else(|| bad(cmd, "`mass_tol` must be 0 to 1"))?,
    };
    let batched = bool_(p, "batch").unwrap_or(true);
    ask.insert("max_fixes".into(), json!(max));
    let ask = Value::Object(ask);
    let before = s.execute("inspect.polish", &ask)?;
    let (ids, categories) = (strings(p, "fixes", cmd)?, strings(p, "categories", cmd)?);
    if let Some(c) = categories.iter().find(|c| !CATEGORIES.contains(&c.as_str())) {
        return Err(bad(cmd, format!("unknown category `{c}` ({})", CATEGORIES.join(", "))));
    }
    let all = before["fixes"].as_array().cloned().unwrap_or_default();
    if let Some(missing) = ids.iter().find(|id| !all.iter().any(|f| f["id"] == id.as_str())) {
        return Err(bad(cmd, format!("no fix `{missing}` (inspect.polish lists them)")));
    }
    let mut fixes: Vec<Fix> = all
        .iter()
        .filter(|f| f["safe"] == true)
        .filter(|f| ids.is_empty() || ids.iter().any(|id| f["id"] == id.as_str()))
        .filter(|f| categories.is_empty() || categories.iter().any(|c| f["category"] == c.as_str()))
        .map(|f| Fix {
            id: f["id"].clone(),
            body: f["body"].as_str().unwrap_or_default().to_string(),
            command: f["command"].as_str().unwrap_or_default().to_string(),
            category: f["category"].clone(),
            kind: f["kind"].clone(),
            size: size(f),
            params: f["params"].clone(),
        })
        .collect();
    let mut run =
        Run { ask, mass_tol, applied: Vec::new(), skipped: Vec::new(), out: BTreeSet::new(), changed: BTreeSet::new(), checks: 0, ms: [0.0; 3] };
    run.ms[0] = started.elapsed().as_secs_f64() * 1000.0;
    let held = hold_joint_edges(s, &mut fixes, &mut run);
    // The whole run is one undo step: the fixes run with the history set aside, so they neither
    // add steps nor push the oldest ones out, and the outer command records the one step.
    let (history, redo) = (std::mem::take(&mut s.undo), std::mem::take(&mut s.redo));
    let order: Vec<usize> = (0..fixes.len()).filter(|i| !run.out.contains(i)).collect();
    let settled = if batched { batch(s, &fixes, &order, &mut run) } else { order.iter().try_for_each(|i| settle(s, &fixes, &[*i], &mut run)) };
    (s.undo, s.redo) = (history, redo);
    settled?;
    let t = web_time::Instant::now();
    let after = s.execute("inspect.polish", &run.ask)?;
    run.ms[0] += t.elapsed().as_secs_f64() * 1000.0;
    let rank = |v: &Value| match v["severity"].as_str() {
        Some("high") => 0,
        Some("medium") => 1,
        _ => 2,
    };
    let mut recommendations: Vec<Value> = after["bodies"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|b| {
            b["findings"].as_array().into_iter().flatten().map(
                move |f| json!({"body": b["body"], "category": f["category"], "severity": f["severity"], "kind": f["kind"], "message": f["message"]}),
            )
        })
        .chain(after["assembly"].as_array().into_iter().flatten().cloned())
        .collect();
    // A finding a listed safe fix still covers (max_fixes or a skip left it) says which, so an
    // agent knows another run does it.
    let left: Vec<&Value> = after["fixes"].as_array().into_iter().flatten().filter(|f| f["safe"] == true).collect();
    for r in &mut recommendations {
        let ids: Vec<Value> = left
            .iter()
            .filter(|f| f["body"] == r["body"] && f["category"] == r["category"])
            .filter(|_| matches!(r["kind"].as_str(), Some("sharp_edges" | "first_layer_edges" | "inside_corners")))
            .map(|f| f["id"].clone())
            .collect();
        if !ids.is_empty() {
            r["fix"] = json!(ids);
        }
    }
    // With categories, the recommendations are theirs: chosen before the cap of 50.
    recommendations.retain(|r| categories.is_empty() || categories.iter().any(|c| r["category"] == c.as_str()));
    recommendations.sort_by_key(rank);
    recommendations.truncate(50);
    // Applied and skipped in the order inspect.polish listed the fixes, whatever order they settled in.
    let place = |v: &Value| fixes.iter().position(|f| f.id == v["id"]).unwrap_or(usize::MAX);
    run.applied.sort_by_key(place);
    run.skipped.sort_by_key(place);
    let ms = |x: f64| (x * 10.0).round() / 10.0;
    Ok(json!({
        "before": {"score": before["score"], "scores": before["scores"]},
        "after": {"score": after["score"], "scores": after["scores"]},
        "applied": run.applied, "skipped": run.skipped, "held_edges": held, "fixes": after["fixes"], "fixes_left": left.len(),
        "recommendations": recommendations,
        "style": after["style"], "colours": after["colours"], "checks": run.checks, "ms": {"inspect": ms(run.ms[0]), "fixes": ms(run.ms[1]), "checks": ms(run.ms[2])},
    }))
}

/// A fix's radius or chamfer distance.
fn size(f: &Value) -> Option<f64> {
    f["params"]["radius"].as_f64().or_else(|| f["params"]["distance"].as_f64())
}

/// Runs fixes `which` one after another; a refused one is settled as skipped (it changed
/// nothing). Returns the ones that ran, with the parameters they ran with.
fn run_fixes(s: &mut Session, fixes: &[Fix], which: &[usize], run: &mut Run, changed: &mut BTreeSet<String>) -> Vec<(usize, Value)> {
    let mut done: Vec<(usize, Value)> = Vec::new();
    let t = web_time::Instant::now();
    for &i in which {
        let Some(fix) = fixes.get(i) else { continue };
        let skip = |run: &mut Run, reason: String| {
            run.skipped.push(json!({"id": fix.id, "body": fix.body, "command": fix.command, "reason": reason}));
            run.out.insert(i);
        };
        // A body an earlier fix changed: the same fix worked out again on it as it is now.
        let params = if changed.contains(&fix.body) {
            // Worked out again, its edges on faces a joint origin names are left out again.
            match fresh(s, &run.ask, fix).map(|p| hold_edges(s, &fix.body, p)) {
                Some((params, _)) if params["edges"].as_array().is_some_and(|e| !e.is_empty()) => params,
                Some(_) => {
                    skip(run, "every edge is on a face a joint origin is attached to".into());
                    continue;
                }
                None => {
                    skip(run, "already fixed by an earlier fix".into());
                    continue;
                }
            }
        } else {
            fix.params.clone()
        };
        // A refused fix leaves the design as it was: nothing to check or roll back. A refused
        // fillet is tried again as a chamfer as wide as its radius (a spline or curved edge the
        // kernel can't round yet, #184, walls too narrow for the ball). A refusal is remembered,
        // so a later pass doesn't spend the kernel's time on it again.
        let started = web_time::Instant::now();
        let mut chamfer = json!({"edges": params["edges"], "distance": params["radius"]});
        if let Some(b) = params.get("body") {
            chamfer["body"] = b.clone();
        }
        let ran = match attempt(s, &fix.body, &fix.command, &params) {
            Ok(local) => Ok(json!({"command": fix.command, "params": local})),
            Err(e) if fix.command == "solid.fillet" => match attempt(s, &fix.body, "solid.chamfer", &chamfer) {
                Ok(local) => Ok(json!({"command": "solid.chamfer", "params": local, "fallback": format!("chamfer: the fillet was refused ({e})")})),
                Err(c) => Err(format!("refused: {e}; as a chamfer: {c}")),
            },
            Err(e) => Err(format!("refused: {e}")),
        };
        let ms = (started.elapsed().as_secs_f64() * 10000.0).round() / 10.0;
        match ran {
            Ok(mut v) => {
                v["ms"] = json!(ms);
                changed.insert(fix.body.clone());
                done.push((i, v));
            }
            Err(why) => {
                run.skipped.push(json!({"id": fix.id, "body": fix.body, "command": fix.command, "reason": why, "ms": ms}));
                run.out.insert(i);
            }
        }
    }
    run.ms[1] += t.elapsed().as_secs_f64() * 1000.0;
    done
}

/// [`exec`], unless the kernel refused the same command with the same parameters (the same
/// edges, by point, and size) on the same body before, in this session: then the refusal again,
/// at once. Another fix on the body seldom makes a refused one work, and trying costs the kernel
/// up to minutes. A refusal is remembered.
fn attempt(s: &mut Session, body: &str, command: &str, params: &Value) -> std::result::Result<Value, String> {
    use std::hash::{Hash, Hasher};
    let key = Some({
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (body, command, params.to_string()).hash(&mut h);
        h.finish()
    });
    if let Some(why) = key.and_then(|k| s.polish_refused.lock().ok()?.get(&k).cloned()) {
        return Err(format!("{why} (refused before)"));
    }
    exec(s, command, params).map_err(|e| {
        let why = e.to_string();
        if let (Some(k), Ok(mut seen)) = (key, s.polish_refused.lock()) {
            if seen.len() >= MAX_REMEMBERED {
                seen.clear();
            }
            seen.insert(k, why.clone());
        }
        why
    })
}

/// Runs `command` inside polish.apply. Run inside this command, it doesn't get the mapping of
/// world picks into the body's own frame that a command run on its own gets: they are mapped
/// here. Returns the parameters it ran with.
fn exec(s: &mut Session, command: &str, params: &Value) -> Result<Value> {
    #[cfg(test)]
    if command == "solid.fillet" && REFUSE_FILLETS.with(std::cell::Cell::get) {
        return Err(crate::EngineError::Other("not supported yet: refused for the test".into()));
    }
    let local = crate::frames::to_local(s, command, params).unwrap_or_else(|| params.clone());
    s.execute(command, &local).map(|_| local)
}

#[cfg(test)]
thread_local! {
    /// Tests: polish.apply's fillets are refused, as the kernel refuses some (#184).
    pub(super) static REFUSE_FILLETS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Records fixes that ran and passed their check as applied (`ran`: the command, its
/// parameters, and why it fell back to a chamfer when it did).
fn accept(run: &mut Run, fixes: &[Fix], done: Vec<(usize, Value)>, was: &State, now: &State) {
    for (i, ran) in done {
        let Some(fix) = fixes.get(i) else { continue };
        let score = |st: &State| st.bodies.get(&fix.body).and_then(|b| b.score);
        let mut v = json!({"id": fix.id, "body": fix.body, "command": ran["command"], "params": ran["params"],
            "score_before": score(was), "score_after": score(now)});
        if let Some(why) = ran.get("fallback") {
            v["fallback"] = why.clone();
        }
        v["ms"] = ran["ms"].clone();
        run.applied.push(v);
        run.changed.insert(fix.body.clone());
    }
}

/// Runs the fixes in the order listed: those on bodies a joint origin names one at a time, each
/// run of the others between them as one batch, body after body with a checkpoint before each
/// body's, checked once. When a batch's check fails, the bodies before the first one it points at
/// stay as they are, and from that body on each body's fixes are settled on their own.
fn batch(s: &mut Session, fixes: &[Fix], which: &[usize], run: &mut Run) -> Result<()> {
    // Bodies a joint origin is attached to by name fail checks most, and whether a joint still
    // resolves depends on what changed before: their fixes go one at a time, and the fixes
    // between them in batches, all in the order listed, so the outcome is the one fix-by-fix
    // gives.
    let held = jointed_bodies(s);
    let mut run_of: Vec<usize> = Vec::new();
    for &i in which {
        let Some(fix) = fixes.get(i).filter(|_| !run.out.contains(&i)) else { continue };
        if held.contains(&fix.body) {
            flush(s, fixes, &mut run_of, run)?;
            settle(s, fixes, &[i], run)?;
        } else {
            run_of.push(i);
        }
    }
    flush(s, fixes, &mut run_of, run)
}

/// Settles a run of fixes: the first fix of each body in one batch, then each body's others one
/// at a time. Two fixes on one body are never checked together, so one can't hide the other's
/// change of mass or score (an inside fillet adds what an outside one takes away).
fn flush(s: &mut Session, fixes: &[Fix], run_of: &mut Vec<usize>, run: &mut Run) -> Result<()> {
    let groups = by_body(fixes, run_of);
    let firsts: Vec<(String, Vec<usize>)> = groups.iter().map(|(b, g)| (b.clone(), g.iter().take(1).copied().collect())).collect();
    let mut rest: Vec<usize> = groups.iter().flat_map(|(_, g)| g.iter().skip(1).copied()).collect();
    rest.sort_unstable();
    run_of.clear();
    batch_groups(s, fixes, &firsts, run)?;
    for i in rest {
        settle(s, fixes, &[i], run)?;
    }
    Ok(())
}

/// Fixes `which` by body, bodies in the order their first fix comes.
fn by_body(fixes: &[Fix], which: &[usize]) -> Vec<(String, Vec<usize>)> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for &i in which {
        let Some(fix) = fixes.get(i) else { continue };
        match groups.iter_mut().find(|(b, _)| *b == fix.body) {
            Some((_, g)) => g.push(i),
            None => groups.push((fix.body.clone(), vec![i])),
        }
    }
    groups
}

/// [`batch`]'s batch: `groups` (one body's fixes each) run together and checked once.
fn batch_groups(s: &mut Session, fixes: &[Fix], groups: &[(String, Vec<usize>)], run: &mut Run) -> Result<()> {
    if groups.is_empty() {
        return Ok(());
    }
    let touched: BTreeSet<String> = groups.iter().map(|(b, _)| b.clone()).collect();
    let was = state(s, run, &touched);
    // Before each body's fixes: the design, and what had been settled as skipped (a fix a
    // tentative one made disappear, or one refused on the changed body, may yet work).
    let mut marks = Vec::new();
    let mut done: Vec<Vec<(usize, Value)>> = Vec::new();
    let mut changed = run.changed.clone();
    for (_, g) in groups {
        marks.push((s.mark(), run.skipped.len(), run.out.clone()));
        done.push(run_fixes(s, fixes, g, run, &mut changed));
    }
    if done.iter().all(Vec::is_empty) {
        return Ok(());
    }
    let now = state(s, run, &touched);
    let Some((_, at_fault)) = worse(&was, &now, run.mass_tol) else {
        for d in done {
            accept(run, fixes, d, &was, &now);
        }
        return Ok(());
    };
    // Keep the bodies before the first one at fault; settle the rest body by body.
    let from = groups.iter().position(|(b, _)| at_fault.contains(b)).unwrap_or(0);
    if let Some((mark, skipped, out)) = marks.into_iter().nth(from) {
        s.go_back(mark);
        run.skipped.truncate(skipped);
        run.out = out;
    }
    for d in done.drain(..from) {
        accept(run, fixes, d, &was, &now);
    }
    for i in groups.iter().skip(from).flat_map(|(_, g)| g.iter().copied()) {
        settle(s, fixes, &[i], run)?;
    }
    Ok(())
}

/// Bodies of components a joint origin is attached to by a face or edge name: a fix there can
/// move or lose the origin.
fn jointed_bodies(s: &Session) -> BTreeSet<String> {
    let held: BTreeSet<u64> = s
        .doc
        .assembly
        .joints
        .iter()
        .flat_map(|j| [&j.a, &j.b])
        .chain(s.doc.assembly.origins.iter().map(|o| &o.origin))
        .filter(|o| !o.snap.face_names().is_empty())
        .map(|o| o.occurrence)
        .collect();
    // Occurrence 0 is the root, component 0.
    let comps: BTreeSet<u64> =
        s.doc.occurrences.iter().filter(|o| held.contains(&o.id)).map(|o| o.component).chain(held.contains(&0).then_some(0)).collect();
    s.world_state().bodies.iter().filter(|b| comps.contains(&b.component)).map(|b| b.name.clone()).collect()
}

/// Runs fixes `which` together in one checkpoint and checks the bodies they touched once; when
/// the check fails, goes back and settles each half on its own.
fn settle(s: &mut Session, fixes: &[Fix], which: &[usize], run: &mut Run) -> Result<()> {
    let which: Vec<usize> = which.iter().copied().filter(|i| !run.out.contains(i)).collect();
    if which.is_empty() {
        return Ok(());
    }
    let touched: BTreeSet<String> = which.iter().filter_map(|i| fixes.get(*i)).map(|f| f.body.clone()).collect();
    let was = state(s, run, &touched);
    let (mark, skipped, out) = (s.mark(), run.skipped.len(), run.out.clone());
    let mut changed = run.changed.clone();
    let done = run_fixes(s, fixes, &which, run, &mut changed);
    if done.is_empty() {
        return Ok(());
    }
    let now = state(s, run, &touched);
    match worse(&was, &now, run.mass_tol) {
        None => accept(run, fixes, done, &was, &now),
        Some((reason, at_fault)) => {
            // Back to before the batch, what it settled as skipped too.
            s.go_back(mark);
            run.skipped.truncate(skipped);
            run.out = out;
            if let [(i, _)] = done.as_slice() {
                if let Some(fix) = fixes.get(*i) {
                    run.skipped.push(json!({"id": fix.id, "body": fix.body, "command": fix.command, "reason": reason}));
                }
                run.out.insert(*i);
                return Ok(());
            }
            let left: Vec<usize> = done.iter().map(|(i, _)| *i).collect();
            let body = |i: &usize| fixes.get(*i).map(|f| f.body.clone()).unwrap_or_default();
            let bodies: BTreeSet<String> = left.iter().map(body).collect();
            let (bad, good): (Vec<usize>, Vec<usize>) = left.iter().partition(|i| at_fault.contains(&body(i)));
            if bad.is_empty() || bodies.len() == 1 {
                // Nothing to tell the bodies apart by: halves.
                let (a, b) = left.split_at(left.len() / 2);
                settle(s, fixes, a, run)?;
                settle(s, fixes, b, run)?;
            } else {
                // The bodies the checks point at are settled each on its own, after the rest
                // together.
                settle(s, fixes, &good, run)?;
                for name in bodies.iter().filter(|n| at_fault.contains(*n)) {
                    let own: Vec<usize> = bad.iter().copied().filter(|i| body(i) == *name).collect();
                    settle(s, fixes, &own, run)?;
                }
            }
        }
    }
    Ok(())
}

/// The fix of the same category, kind and size inspect.polish gives for its body now.
fn fresh(s: &mut Session, ask: &Value, fix: &Fix) -> Option<Value> {
    let mut one = ask.clone();
    one["bodies"] = json!([fix.body]);
    one["max_fixes"] = json!(100);
    let r = s.execute("inspect.polish", &one).ok()?;
    r["fixes"].as_array()?.iter().find(|f| f["category"] == fix.category && f["kind"] == fix.kind && size(f) == fix.size).map(|f| f["params"].clone())
}

/// Leaves out of each fix the edges a joint origin is attached to, by persistent name: the edges
/// an origin names and every edge of a face it names (a fillet there changes the face, and a name
/// that lists the face's neighbours with it), so the origin is still found. A fix left with none
/// is skipped. Returns the edges left out, by fix.
fn hold_joint_edges(s: &Session, fixes: &mut [Fix], run: &mut Run) -> Vec<Value> {
    let mut held = Vec::new();
    for (i, fix) in fixes.iter_mut().enumerate() {
        let (params, left_out) = hold_edges(s, &fix.body, fix.params.clone());
        if left_out == 0 {
            continue;
        }
        held.push(json!({"id": fix.id, "edges": left_out}));
        if params["edges"].as_array().is_none_or(|e| e.is_empty()) {
            run.skipped.push(
                json!({"id": fix.id, "body": fix.body, "command": fix.command, "reason": "every edge is on a face a joint origin is attached to"}),
            );
            run.out.insert(i);
        } else {
            fix.params = params;
        }
    }
    held
}

/// A fix's parameters on `body` without the edges a joint origin is attached to (see
/// [`hold_joint_edges`]), and how many were left out.
fn hold_edges(s: &Session, body: &str, mut params: Value) -> (Value, usize) {
    let st = s.world_state();
    let Some(b) = st.body(body) else { return (params, 0) };
    let component = |occ: u64| s.doc.occurrences.iter().find(|o| o.id == occ).map_or(0, |o| o.component);
    let snaps: Vec<&Snap> = s
        .doc
        .assembly
        .joints
        .iter()
        .flat_map(|j| [&j.a, &j.b])
        .chain(s.doc.assembly.origins.iter().map(|o| &o.origin))
        .filter(|o| component(o.occurrence) == b.component)
        .map(|o| &o.snap)
        .collect();
    if snaps.is_empty() {
        return (params, 0);
    }
    let points: Vec<Value> = params["edges"].as_array().cloned().unwrap_or_default();
    let at: Vec<polycore_geom::Vec3> = points.iter().filter_map(crate::params::vec3).collect();
    let names = polykrom_doc::naming::edge_names_at(b, &at);
    // The faces and edges the origins name; an edge on such a face changes the face (and its
    // name, which can list its neighbours), so the origin may no longer be found.
    let named: BTreeSet<&str> =
        snaps.iter().flat_map(|snap| snap.face_names()).flat_map(|n| std::iter::once(n.as_str()).chain(n.split('|'))).collect();
    let holds = |name: &str| named.contains(name) || name.split('|').any(|f| named.contains(f));
    let keep: Vec<Value> = points.iter().zip(&names).filter(|(_, n)| n.is_empty() || !holds(n)).map(|(p, _)| p.clone()).collect();
    let left_out = points.len().saturating_sub(keep.len());
    params["edges"] = json!(keep);
    (params, left_out)
}

/// The touched bodies' volume (from their display meshes), validity and polish score, the joints
/// on them that don't resolve, and their clashes with the bodies around them.
fn state(s: &mut Session, run: &mut Run, touched: &BTreeSet<String>) -> State {
    let t = web_time::Instant::now();
    run.checks += 1;
    let st = s.world_state();
    let mut bodies = BTreeMap::new();
    let mut around: BTreeSet<String> = touched.clone();
    let mut components = BTreeSet::new();
    // A fix changes every placement of a part: each placement's clashes count as its body's.
    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    for name in touched {
        let Some(b) = st.body(name) else {
            bodies.insert(name.clone(), BodyState { volume: None, invalid: usize::MAX, score: None });
            continue;
        };
        let m = b.mesh();
        let volume = m.triangles.iter().filter_map(|t| m.tri(t)).map(|[a, b, c]| a.dot(b.cross(c)) / 6.0).sum::<f64>().abs();
        bodies.insert(name.clone(), BodyState { volume: Some(volume), invalid: b.body.validity().len(), score: None });
        components.insert(b.component);
        for placed in st.bodies.iter().filter(|o| o.definition_name() == b.definition_name()) {
            owner.insert(placed.name.clone(), name.clone());
            let bx = placed.mesh().bounds().inflated(NEIGHBOUR_GAP);
            around.extend(st.bodies.iter().filter(|o| o.mesh().bounds().meets(&bx)).map(|o| o.name.clone()));
        }
    }
    // Scores, every touched body in one call.
    let mut ask = run.ask.clone();
    ask["bodies"] = json!(touched);
    if let Ok(r) = s.execute("inspect.polish", &ask) {
        for b in r["bodies"].as_array().into_iter().flatten() {
            if let Some(slot) = b["body"].as_str().and_then(|n| bodies.get_mut(n)) {
                slot.score = b["score"].as_f64();
            }
        }
    }
    // Joints on the touched bodies' occurrences only, with the touched bodies each holds.
    // Occurrence 0 is the root, component 0.
    let component_of = |occ: u64| if occ == 0 { Some(0) } else { s.doc.occurrences.iter().find(|o| o.id == occ).map(|o| o.component) };
    let mut joint_bodies: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for j in &s.doc.assembly.joints {
        for occ in [j.a.occurrence, j.b.occurrence] {
            let Some(c) = component_of(occ).filter(|c| components.contains(c)) else { continue };
            let held = touched.iter().filter(|n| st.body(n).is_some_and(|b| b.component == c)).cloned();
            joint_bodies.entry(j.id).or_default().extend(held);
        }
    }
    let mine: BTreeSet<u64> = joint_bodies.keys().copied().collect();
    let joint_errors = if mine.is_empty() {
        BTreeSet::new()
    } else {
        polykrom_doc::joints::solve(&s.doc, &s.model.state()).errors.iter().map(|e| e.0).filter(|id| mine.contains(id)).collect()
    };
    let clashes = s
        .execute("inspect.interference", &json!({"bodies": around, "method": "mesh"}))
        .ok()
        .and_then(|r| r["interferences"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|h| Some(((h["a"].as_str()?.to_string(), h["b"].as_str()?.to_string()), h["depth_mm"].as_f64().unwrap_or(0.0))))
        .filter(|((a, b), _)| owner.contains_key(a) || owner.contains_key(b))
        .collect();
    run.ms[2] += t.elapsed().as_secs_f64() * 1000.0;
    State { bodies, joint_errors, joint_bodies, clashes, owner }
}

/// Why the state after a batch is worse than before it (`None`: it isn't), and the touched bodies
/// at fault: every body a failed check points at (its own measures, a joint it holds that no
/// longer resolves, a new clash it is in).
fn worse(was: &State, now: &State, mass_tol: f64) -> Option<(String, BTreeSet<String>)> {
    let mut why: Option<String> = None;
    let mut at_fault = BTreeSet::new();
    let mut fault = |reason: String, bodies: &mut dyn Iterator<Item = String>| {
        why.get_or_insert(reason);
        at_fault.extend(bodies);
    };
    for (name, b) in &now.bodies {
        let Some(a) = was.bodies.get(name) else { continue };
        let reason = if b.invalid > a.invalid {
            Some(format!("{name} is no longer valid"))
        } else if b.volume.is_none() || b.score.is_none() {
            // What can't be measured after the fix isn't known to be fine.
            Some(format!("{name} could not be measured after the fix"))
        } else if let (Some(x), Some(y)) = (a.volume, b.volume)
            && x > 0.0
            && ((y - x) / x).abs() > mass_tol
        {
            Some(format!("the mass of {name} changed by {:.1}%", (y - x) / x * 100.0))
        } else if let (Some(x), Some(y)) = (a.score, b.score)
            && y < x - SCORE_SLACK
        {
            Some(format!("the score of {name} fell from {x} to {y}"))
        } else {
            None
        };
        if let Some(r) = reason {
            fault(r, &mut std::iter::once(name.clone()));
        }
    }
    for j in now.joint_errors.iter().filter(|j| !was.joint_errors.contains(j)) {
        fault("joints no longer resolve".into(), &mut now.joint_bodies.get(j).into_iter().flatten().cloned());
    }
    for ((a, b), _) in now.clashes.iter().filter(|(k, d)| was.clashes.get(*k).is_none_or(|w| **d > w + DEPTH_SLACK)) {
        fault(format!("{a} and {b} interfere"), &mut [a, b].into_iter().filter_map(|n| now.owner.get(n)).cloned());
    }
    why.map(|w| (w, at_fault))
}

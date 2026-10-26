//! Applying `inspect.polish`'s safe fixes (#193): its fillets and chamfers, one group at a time,
//! each checked before the next. A group the kernel refuses (a fillet on a curved edge, #184), or
//! one that leaves the body invalid, makes a new clash with a neighbour, breaks a joint, moves the
//! mass by more than `mass_tol` or lowers the body's polish score, is rolled back to its
//! checkpoint and listed as skipped with the reason; the rest stay. Fixes that reshape a part stay
//! recommendations. The whole run is one step to undo.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::CommandSpec;
use crate::params::{bad, num, string_list};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("polish.apply", "Apply Polish", apply).icon("polish").params(
    "fixes?: [ids from inspect.polish, such as \"F1\"] | categories?: [look|print|strength] (default every safe fix); bodies?, printed?, \
     skip?, sharp_deg? (as inspect.polish); max_fixes?: how many of inspect.polish's fixes to try, 1…100 (default 20); mass_tol?: the \
     share of a body's volume a fix may change, 0…1 (default 0.05) → before: {score, scores}, after: {score, scores}, applied: [{id, \
     body, command, params, score_before, score_after}], skipped: [{id, body, command, reason}] (refused by the kernel, invalid body, \
     new interference, joints no longer resolve, mass changed, score fell, or already fixed by an earlier fix), fixes (what inspect.polish \
     still suggests), fixes_left (how many of them are safe: run again), recommendations: [findings, most severe \
     first; fix: [ids] on one a listed safe fix still covers]. Each fix runs in a checkpoint and is rolled back \
     when it fails a check; a fix on a body an earlier fix changed is worked out again from the body as it is now. One undo step",
)];

/// The inspect.polish parameters a run passes on.
const PASSED: [&str; 4] = ["bodies", "printed", "skip", "sharp_deg"];
/// How far a fix may lower a body's score (rounding) or deepen a clash (mm) and still stay.
const SCORE_SLACK: f64 = 0.5;
const DEPTH_SLACK: f64 = 0.05;

/// What a fix is checked against: the body's volume and validity, the joints, the clashes
/// around it and its score.
struct State {
    volume: Option<f64>,
    invalid: usize,
    joint_errors: usize,
    clashes: BTreeMap<(String, String), f64>,
    score: Option<f64>,
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "polish.apply";
    let mut ask: Map<String, Value> = PASSED.iter().filter_map(|k| Some((k.to_string(), p.get(*k).filter(|v| !v.is_null())?.clone()))).collect();
    let max = match p.get("max_fixes").filter(|v| !v.is_null()) {
        None => 20.0,
        Some(_) => num(p, "max_fixes").filter(|v| (1.0..=100.0).contains(v)).ok_or_else(|| bad(cmd, "`max_fixes` must be 1 to 100"))?,
    };
    let mass_tol = match p.get("mass_tol").filter(|v| !v.is_null()) {
        None => 0.05,
        Some(_) => num(p, "mass_tol").filter(|v| (0.0..=1.0).contains(v)).ok_or_else(|| bad(cmd, "`mass_tol` must be 0 to 1"))?,
    };
    ask.insert("max_fixes".into(), json!(max));
    let ask = Value::Object(ask);
    let before = s.execute("inspect.polish", &ask)?;
    let (ids, categories) = (string_list(p, "fixes"), string_list(p, "categories"));
    let all = before["fixes"].as_array().cloned().unwrap_or_default();
    if let Some(missing) = ids.iter().find(|id| !all.iter().any(|f| f["id"] == id.as_str())) {
        return Err(bad(cmd, format!("no fix `{missing}` (inspect.polish lists them)")));
    }
    let chosen: Vec<&Value> = all
        .iter()
        .filter(|f| f["safe"] == true)
        .filter(|f| ids.is_empty() || ids.iter().any(|id| f["id"] == id.as_str()))
        .filter(|f| categories.is_empty() || categories.iter().any(|c| f["category"] == c.as_str()))
        .collect();
    // The whole run is one undo step: the fixes run with the history set aside, so they neither
    // add steps nor push the oldest ones out, and the outer command records the one step.
    let (history, redo) = (std::mem::take(&mut s.undo), std::mem::take(&mut s.redo));
    let mut failure = None;
    let (mut applied, mut skipped) = (Vec::new(), Vec::new());
    let mut changed: BTreeSet<String> = BTreeSet::new();
    for fix in chosen {
        let body = fix["body"].as_str().unwrap_or_default().to_string();
        let command = fix["command"].as_str().unwrap_or_default().to_string();
        let mut skip = |reason: String| skipped.push(json!({"id": fix["id"], "body": body, "command": command, "reason": reason}));
        // A body an earlier fix changed: the same fix worked out again on it as it is now.
        let params = if changed.contains(&body) {
            match fresh(s, &ask, &body, fix) {
                Some(params) => params,
                None => {
                    skip("already fixed by an earlier fix".into());
                    continue;
                }
            }
        } else {
            fix["params"].clone()
        };
        // Run inside this command, the fix doesn't get the mapping of world picks into the body's
        // own frame that a command run on its own gets: map them here.
        let params = crate::frames::to_local(s, &command, &params).unwrap_or(params);
        let was = state(s, &ask, &body);
        let mark = s.checkpoint();
        let (why, now) = match s.execute(&command, &params) {
            Err(e) => (Some(format!("refused: {e}")), None),
            Ok(_) => {
                let now = state(s, &ask, &body);
                (worse(&was, &now, mass_tol), now.score)
            }
        };
        match why {
            Some(reason) => {
                if let Err(e) = s.restore_checkpoint(mark) {
                    failure = Some(e);
                    break;
                }
                skip(reason);
            }
            None => {
                s.drop_checkpoint(mark);
                applied.push(
                    json!({"id": fix["id"], "body": body, "command": command, "params": params, "score_before": was.score, "score_after": now}),
                );
                changed.insert(body);
            }
        }
    }
    (s.undo, s.redo) = (history, redo);
    if let Some(e) = failure {
        return Err(e);
    }
    let after = s.execute("inspect.polish", &ask)?;
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
    recommendations.sort_by_key(rank);
    recommendations.truncate(50);
    Ok(json!({
        "before": {"score": before["score"], "scores": before["scores"]},
        "after": {"score": after["score"], "scores": after["scores"]},
        "applied": applied, "skipped": skipped, "fixes": after["fixes"], "fixes_left": left.len(), "recommendations": recommendations,
    }))
}

/// The fix of the same category, kind and size inspect.polish gives for `body` now.
fn fresh(s: &mut Session, ask: &Value, body: &str, fix: &Value) -> Option<Value> {
    let mut one = ask.clone();
    one["bodies"] = json!([body]);
    one["max_fixes"] = json!(100);
    let r = s.execute("inspect.polish", &one).ok()?;
    let size = |f: &Value| f["params"]["radius"].as_f64().or_else(|| f["params"]["distance"].as_f64());
    r["fixes"]
        .as_array()?
        .iter()
        .find(|f| f["category"] == fix["category"] && f["kind"] == fix["kind"] && size(f) == size(fix))
        .map(|f| f["params"].clone())
}

fn state(s: &mut Session, ask: &Value, body: &str) -> State {
    let st = s.world_state();
    let me = st.body(body);
    let volume = me.and_then(|b| solidcraft_kernel::measure(&b.body).ok()).map(|m| m.volume);
    let invalid = me.map_or(usize::MAX, |b| b.body.validity().len());
    // The neighbours: bodies whose boxes come within a millimetre.
    let mut around = vec![body.to_string()];
    if let Some(b) = me {
        let bx = b.mesh().bounds().inflated(1.0);
        around.extend(st.bodies.iter().filter(|o| o.name != body && o.mesh().bounds().meets(&bx)).map(|o| o.name.clone()));
    }
    let joint_errors = if s.doc.assembly.joints.is_empty() { 0 } else { solidcraft_doc::joints::solve(&s.doc, &s.model.state()).errors.len() };
    let clashes = s
        .execute("inspect.interference", &json!({"bodies": around, "method": "mesh"}))
        .ok()
        .and_then(|r| r["interferences"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|h| Some(((h["a"].as_str()?.to_string(), h["b"].as_str()?.to_string()), h["depth_mm"].as_f64().unwrap_or(0.0))))
        .collect();
    let mut one = ask.clone();
    one["bodies"] = json!([body]);
    let score = s.execute("inspect.polish", &one).ok().and_then(|r| r["bodies"][0]["score"].as_f64());
    State { volume, invalid, joint_errors, clashes, score }
}

/// Why the state after a fix is worse than before it (`None`: it isn't).
fn worse(was: &State, now: &State, mass_tol: f64) -> Option<String> {
    if now.invalid > was.invalid {
        return Some("the body is no longer valid".into());
    }
    if let (Some(a), Some(b)) = (was.volume, now.volume)
        && a > 0.0
        && ((b - a) / a).abs() > mass_tol
    {
        return Some(format!("the mass changed by {:.1}%", (b - a) / a * 100.0));
    }
    if now.joint_errors > was.joint_errors {
        return Some("joints no longer resolve".into());
    }
    if let Some(((a, b), _)) = now.clashes.iter().find(|(k, d)| was.clashes.get(*k).is_none_or(|w| **d > w + DEPTH_SLACK)) {
        return Some(format!("{a} and {b} interfere"));
    }
    match (was.score, now.score) {
        (Some(a), Some(b)) if b < a - SCORE_SLACK => Some(format!("the score fell from {a} to {b}")),
        _ => None,
    }
}

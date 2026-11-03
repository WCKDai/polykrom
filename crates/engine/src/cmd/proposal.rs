//! Propose and apply: an agent (or the Review panel) shows what some commands would do, and the
//! person decides. `engine.propose` runs the commands on a scratch copy (the preview path) and
//! reports the change without touching the design; `engine.apply_proposal` runs exactly those
//! commands as one undo step, and only while the design is as it was when it was proposed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use polykrom_doc::{Document, ModelBody, ModelState};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::bad;
use crate::{EngineError, Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("engine.propose", "Propose Change", propose)
        .icon("inspect")
        .noundo()
        .params(
            "commands: [{command, params}] (as engine.batch takes them; at most 200 design-editing steps, no file, document or engine commands); label?: text shown for the proposal \
             → {proposal: id, revision, label, replies, bodies: {created, changed, removed} (each with volume_mm3 and area_mm2 before/after and delta), totals, \
             timeline: {added, changed, removed, affected}, parameters, warnings, errors}. Runs the steps on a scratch copy: the design, its revision, selection and \
             undo history are unchanged. The proposal is held in memory (the last 8) until the design changes; engine.apply_proposal {proposal} applies it",
        ),
    CommandSpec::new("engine.apply_proposal", "Apply Proposal", apply)
        .icon("edit")
        .params(
            "proposal: id from engine.propose → {applied: true, proposal, label, commands, bodies: [{name, volume_mm3, area_mm2}]}. Runs the proposal's commands as ONE undo step. \
             Refused with a structured error (the error text is a JSON object {code, message, …}) when the design changed since the proposal (`stale_proposal`) or \
             the id is not held (`unknown_proposal`); a command failing leaves the design as it was",
        ),
];

/// Proposals held at once; the oldest is dropped for a new one.
pub const MAX_PROPOSALS: usize = 8;
/// Most steps in one proposal.
pub const MAX_STEPS: usize = 200;
/// Ids remembered after their proposals were dropped, to tell "stale" from "never held".
const MAX_RETIRED: usize = 64;

/// One held proposal: the commands, and the design they were made against.
#[derive(Clone)]
struct Proposal {
    id: String,
    label: String,
    revision: u64,
    doc: Arc<Document>,
    commands: Vec<(String, Value)>,
}

/// What a session holds of its proposals. Not part of the design, not saved, not in history.
#[derive(Clone, Default)]
pub(crate) struct Proposals {
    next: u64,
    held: VecDeque<Proposal>,
    /// Ids dropped because the design changed: `apply_proposal` answers them `stale_proposal`.
    stale: VecDeque<(String, u64)>,
}

impl Proposals {
    /// The design changed: nothing held applies any more.
    pub(crate) fn design_changed(&mut self) {
        for p in self.held.drain(..) {
            self.stale.push_back((p.id, p.revision));
        }
        while self.stale.len() > MAX_RETIRED {
            self.stale.pop_front();
        }
    }

    /// How many proposals are held.
    fn len(&self) -> usize {
        self.held.len()
    }
}

fn refused(code: &str, message: String, extra: Value) -> EngineError {
    let mut v = json!({"code": code, "message": message});
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        o.extend(e.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    EngineError::Refused(v)
}

/// The steps of a `commands` parameter, checked: known design-editing commands only.
fn steps(p: &Value) -> Result<Vec<(String, Value)>> {
    let cmd = "engine.propose";
    let list = p.get("commands").and_then(Value::as_array).ok_or_else(|| bad(cmd, "`commands` must be an array of {command, params}"))?;
    if list.is_empty() {
        return Err(bad(cmd, "`commands` is empty: nothing to propose"));
    }
    if list.len() > MAX_STEPS {
        return Err(bad(cmd, format!("at most {MAX_STEPS} steps in a proposal")));
    }
    let mut out = Vec::with_capacity(list.len());
    for (i, step) in list.iter().enumerate() {
        let id =
            step.get("command").or_else(|| step.get("id")).and_then(Value::as_str).ok_or_else(|| bad(cmd, format!("step {i}: missing `command`")))?;
        let params = step.get("params").cloned().filter(|v| !v.is_null()).unwrap_or_else(|| json!({}));
        if !params.is_object() {
            return Err(bad(cmd, format!("step {i} ({id}): `params` must be an object")));
        }
        let spec = crate::find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        let outside = ["engine.", "file.", "doc.", "render.", "diagnostics."].iter().any(|p| spec.id.starts_with(p));
        let writes = crate::control_pending::command_effect(spec.id, &params) != crate::control_pending::Effect::Changes;
        if !spec.undoable || outside || writes {
            return Err(bad(cmd, format!("step {i} ({id}) is not a design edit: a proposal holds commands that change the design and nothing else")));
        }
        out.push((spec.id.to_string(), params));
    }
    Ok(out)
}

fn label_of(p: &Value, commands: &[(String, Value)]) -> String {
    let given = p.get("label").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty());
    match given {
        Some(l) => l.chars().take(120).collect(),
        None => match commands {
            [(id, _)] => id.clone(),
            [(id, _), rest @ ..] => format!("{id} and {} more", rest.len()),
            [] => String::new(),
        },
    }
}

fn pair(volume: Option<f64>, area: Option<f64>) -> Value {
    json!({"volume_mm3": volume, "area_mm2": area})
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn delta(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    (before.is_some() || after.is_some()).then_some(after.unwrap_or(0.0) - before.unwrap_or(0.0))
}

fn same(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0),
        (None, None) => true,
        _ => false,
    }
}

/// The volume and area of a body (none for what the kernel cannot measure; no volume for a sheet).
fn measured(b: &ModelBody) -> (Option<f64>, Option<f64>) {
    let m = super::inspect::measure_json(b);
    (num(&m, "volume_mm3"), num(&m, "area_mm2"))
}

fn bodies_diff(before: &ModelState, after: &ModelState) -> (Value, (Option<f64>, Option<f64>), (Option<f64>, Option<f64>)) {
    let mut created = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let (mut tb, mut ta) = ((0.0, 0.0), (0.0, 0.0));
    let add = |t: &mut (f64, f64), m: (Option<f64>, Option<f64>)| {
        t.0 += m.0.unwrap_or(0.0);
        t.1 += m.1.unwrap_or(0.0);
    };
    for b in &before.bodies {
        let mb = measured(b);
        add(&mut tb, mb);
        match after.body(&b.name) {
            None => removed.push(json!({"name": b.name, "feature": b.feature, "before": pair(mb.0, mb.1)})),
            Some(a) => {
                let ma = measured(a);
                add(&mut ta, ma);
                let unchanged_key = b.geometry_key().is_some() && b.geometry_key() == a.geometry_key();
                if !unchanged_key && (b.geometry_key() != a.geometry_key() || !same(mb.0, ma.0) || !same(mb.1, ma.1)) {
                    changed.push(json!({
                        "name": b.name,
                        "feature": a.feature,
                        "before": pair(mb.0, mb.1),
                        "after": pair(ma.0, ma.1),
                        "delta": pair(delta(mb.0, ma.0), delta(mb.1, ma.1)),
                    }));
                }
            }
        }
    }
    for a in after.bodies.iter().filter(|a| before.body(&a.name).is_none()) {
        let ma = measured(a);
        add(&mut ta, ma);
        created.push(json!({"name": a.name, "feature": a.feature, "after": pair(ma.0, ma.1)}));
    }
    let bodies = json!({"created": created, "changed": changed, "removed": removed});
    (bodies, (Some(tb.0), Some(tb.1)), (Some(ta.0), Some(ta.1)))
}

/// Every body's geometry identity in a feature's result: what changed in it shows as a difference.
fn fingerprint(state: &ModelState) -> Vec<(String, Option<u64>)> {
    state.bodies.iter().map(|b| (b.name.clone(), b.geometry_key())).collect()
}

fn feature_json(s: &Session, id: u64, change: &str) -> Value {
    let f = s.doc.feature(id);
    let r = s.model.result(id);
    json!({
        "id": id,
        "name": f.map(|f| f.name.clone()),
        "type": f.map(|f| f.kind.type_name()),
        "change": change,
        "error": r.and_then(|r| r.error.clone()),
        "warning": r.and_then(|r| r.warning.clone()),
    })
}

/// What the commands would do: the report of `engine.propose`.
fn report(before: &Session, after: &Session) -> Value {
    let (bodies, tb, ta) = bodies_diff(&before.world_state(), &after.world_state());

    let old: BTreeMap<u64, &polykrom_doc::Feature> = before.doc.features.iter().map(|f| (f.id, f)).collect();
    let new: BTreeSet<u64> = after.doc.features.iter().map(|f| f.id).collect();
    let (mut added, mut changed, mut affected, mut warnings, mut errors) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for f in &after.doc.features {
        let kind = match old.get(&f.id) {
            None => Some("added"),
            Some(o) if *o != f => Some("changed"),
            Some(_) => {
                let (b, a) = (before.model.result(f.id), after.model.result(f.id));
                let differs = match (b, a) {
                    (Some(b), Some(a)) => fingerprint(&b.state) != fingerprint(&a.state) || b.error != a.error || b.warning != a.warning,
                    (None, None) => false,
                    _ => true,
                };
                differs.then_some("affected")
            }
        };
        let Some(kind) = kind else { continue };
        let entry = feature_json(after, f.id, kind);
        let was = before.model.result(f.id);
        if let Some(w) = entry["warning"].as_str()
            && was.and_then(|r| r.warning.as_deref()) != Some(w)
        {
            warnings.push(json!({"feature": f.id, "name": f.name, "warning": w}));
        }
        if let Some(e) = entry["error"].as_str()
            && was.and_then(|r| r.error.as_deref()) != Some(e)
        {
            errors.push(json!({"feature": f.id, "name": f.name, "error": e}));
        }
        match kind {
            "added" => added.push(entry),
            "changed" => changed.push(entry),
            _ => affected.push(entry),
        }
    }
    let removed: Vec<Value> = before
        .doc
        .features
        .iter()
        .filter(|f| !new.contains(&f.id))
        .map(|f| json!({"id": f.id, "name": f.name, "type": f.kind.type_name(), "change": "removed"}))
        .collect();

    let old_params: BTreeMap<&str, &str> = before.doc.params.iter().map(|p| (p.name.as_str(), p.expr.as_str())).collect();
    let mut parameters = Vec::new();
    for p in &after.doc.params {
        match old_params.get(p.name.as_str()) {
            Some(was) if *was == p.expr => {}
            was => parameters.push(json!({"name": p.name, "before": was, "after": p.expr})),
        }
    }

    json!({
        "bodies": bodies,
        "totals": {
            "before": pair(tb.0, tb.1),
            "after": pair(ta.0, ta.1),
            "delta": pair(delta(tb.0, ta.0), delta(tb.1, ta.1)),
        },
        "timeline": {"added": added, "changed": changed, "removed": removed, "affected": affected},
        "parameters": parameters,
        "warnings": warnings,
        "errors": errors,
    })
}

fn propose(s: &mut Session, p: &Value) -> Result<Value> {
    let commands = steps(p)?;
    let label = label_of(p, &commands);
    if s.active_sketch.is_some() {
        return Err(bad("engine.propose", "a sketch is being edited: finish it first, or start the proposal with the sketch commands that make it"));
    }
    if s.model.is_stale() {
        s.refresh();
    }
    let (after, replies) = s.scratch().preview_steps(&commands)?;
    let mut out = report(s, &after);
    s.proposals.next += 1;
    let id = format!("prop-{}", s.proposals.next);
    let (revision, doc) = (s.revision, s.doc.clone());
    s.proposals.held.push_back(Proposal { id: id.clone(), label: label.clone(), revision, doc, commands: commands.clone() });
    while s.proposals.held.len() > MAX_PROPOSALS {
        if let Some(old) = s.proposals.held.pop_front() {
            s.proposals.stale.push_back((old.id, old.revision));
        }
    }
    if let Some(o) = out.as_object_mut() {
        o.insert("proposal".into(), json!(id));
        o.insert("revision".into(), json!(revision));
        o.insert("label".into(), json!(label));
        o.insert("steps".into(), json!(commands.len()));
        o.insert("replies".into(), Value::Array(replies));
        o.insert("held".into(), json!(s.proposals.len()));
    }
    Ok(out)
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "engine.apply_proposal";
    let id =
        p.get("proposal").or_else(|| p.get("id")).and_then(Value::as_str).ok_or_else(|| bad(cmd, "`proposal` must be an id from engine.propose"))?;
    let Some(held) = s.proposals.held.iter().find(|h| h.id == id).cloned() else {
        return Err(match s.proposals.stale.iter().find(|(k, _)| k == id) {
            Some((_, made_on)) => refused(
                "stale_proposal",
                format!("the design changed since proposal {id} was made: propose the change again"),
                json!({"proposal": id, "made_on_revision": made_on, "current_revision": s.revision}),
            ),
            None => refused("unknown_proposal", format!("no proposal {id} is held (the last {MAX_PROPOSALS} are kept)"), json!({"proposal": id})),
        });
    };
    // Dropped on every change, so a held proposal is current; this guards a design replaced by a
    // path that does not run through `execute`.
    if held.revision != s.revision || !Arc::ptr_eq(&held.doc, &s.doc) {
        s.proposals.design_changed();
        return Err(refused(
            "stale_proposal",
            format!("the design changed since proposal {id} was made: propose the change again"),
            json!({"proposal": id, "made_on_revision": held.revision, "current_revision": s.revision}),
        ));
    }
    // The outer `execute` owns the undo step and puts the design back if a step fails.
    for (i, (command, params)) in held.commands.iter().enumerate() {
        s.execute(command, params).map_err(|e| EngineError::Other(format!("proposal {id}: step {i} ({command}): {e}; the design is unchanged")))?;
    }
    let state = s.world_state();
    let bodies: Vec<Value> = state
        .bodies
        .iter()
        .map(|b| {
            let (volume, area) = measured(b);
            json!({"name": b.name, "volume_mm3": volume, "area_mm2": area})
        })
        .collect();
    Ok(json!({"applied": true, "proposal": id, "label": held.label, "commands": held.commands.len(), "bodies": bodies}))
}

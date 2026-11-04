//! Propose and apply: an agent (or the Review panel) shows what some commands would do, and the
//! person decides. `engine.propose` runs the commands on a scratch copy (the preview path) and
//! reports the change without touching the design; `engine.apply_proposal` runs exactly those
//! commands as one undo step, and only while the design is as it was when it was proposed.
//! `engine.propose_candidates` (PK-88) proposes several candidates for one intent at once: each
//! is evaluated on its own scratch copy exactly as `engine.propose` evaluates one, held as an
//! ordinary proposal under one candidate-set id, and the reply compares them side by side
//! (volumes, masses, boxes, the exact IoU with the current design as `inspect.compare` finds it).

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
    CommandSpec::new("engine.propose_candidates", "Propose Candidates", propose_candidates)
        .icon("inspect")
        .noundo()
        .params(
            "candidates: [{label?, commands: [{command, params}]}] (2 to 8; each limited as engine.propose limits one proposal); label?: text for the set \
             → {set: id, revision, label, current: {volume_mm3, mass_g, bbox, bodies}, candidates: [{index, proposal: id | null, label, valid, error, errors, warnings, steps, replies, \
             bodies: [{name, change, volume_mm3, area_mm2, mass_g, material, density_g_cm3, bbox, compare: {iou, intersection_mm3, union_mm3, identical, …}}], \
             totals: {volume_mm3, area_mm2, mass_g, bbox, delta}, compare: {iou, intersection_mm3, union_mm3, identical}, timeline, parameters, ms}], held}. \
             Each candidate runs on its own scratch copy of the same design, as engine.propose runs one, and is held as an ordinary proposal with its own id; \
             a candidate whose step fails is reported invalid with its error (proposal null, not held) and the others are still evaluated. The exact IoU is \
             inspect.compare's, per body by name, and over the design. engine.apply_proposal on one candidate applies it as ONE undo step and drops the set's \
             other candidates; engine.drop_proposal with the set id drops them all; a design change makes the whole set stale. The design, its revision, \
             selection and undo history are unchanged",
        ),
    CommandSpec::new("engine.apply_proposal", "Apply Proposal", apply)
        .icon("edit")
        .params(
            "proposal: id from engine.propose or a candidate's id from engine.propose_candidates → {applied: true, proposal, label, commands, bodies: [{name, volume_mm3, area_mm2}], set?, dropped?}. \
             Runs the proposal's commands as ONE undo step; a candidate's set-mates are dropped (`dropped`). \
             Refused with a structured error (the error text is a JSON object {code, message, …}) when the design changed since the proposal (`stale_proposal`) or \
             the id is not held (`unknown_proposal`); a command failing leaves the design as it was",
        ),
    CommandSpec::new("engine.drop_proposal", "Drop Proposal", drop_proposal)
        .icon("delete")
        .noundo()
        .params(
            "proposal: id from engine.propose, a candidate's id or a set id from engine.propose_candidates → {dropped: bool, proposal, held, proposals?}. The Review panel's Cancel command: the proposal \
             (or every candidate of the set) is forgotten and the design is untouched. `dropped` is false for a proposal already gone (stale or applied); an id never held is refused (`unknown_proposal`)",
        ),
];

/// Proposals held at once; the oldest is dropped for a new one.
pub const MAX_PROPOSALS: usize = 8;
/// Most steps in one proposal.
pub const MAX_STEPS: usize = 200;
/// Fewest and most candidates in one set.
pub const MIN_CANDIDATES: usize = 2;
pub const MAX_CANDIDATES: usize = 8;
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
    /// The candidate set it belongs to (`engine.propose_candidates`), if any.
    set: Option<String>,
}

/// What a session holds of its proposals. Not part of the design, not saved, not in history.
#[derive(Clone, Default)]
pub(crate) struct Proposals {
    next: u64,
    next_set: u64,
    held: VecDeque<Proposal>,
    /// Ids dropped because the design changed (proposals and candidate sets): `apply_proposal`
    /// answers them `stale_proposal`.
    stale: VecDeque<(String, u64)>,
}

impl Proposals {
    /// The design changed: nothing held applies any more.
    pub(crate) fn design_changed(&mut self) {
        for p in self.held.drain(..).collect::<Vec<_>>() {
            if let Some(set) = &p.set {
                self.retire(set.clone(), p.revision);
            }
            self.retire(p.id, p.revision);
        }
    }

    /// Remember a dropped id, so it is answered "stale" rather than "unknown".
    fn retire(&mut self, id: String, revision: u64) {
        if !self.stale.iter().any(|(k, _)| *k == id) {
            self.stale.push_back((id, revision));
        }
        while self.stale.len() > MAX_RETIRED {
            self.stale.pop_front();
        }
    }

    /// Hold a proposal; the oldest goes when more than [`MAX_PROPOSALS`] are held.
    fn hold(&mut self, label: String, revision: u64, doc: Arc<Document>, commands: Vec<(String, Value)>, set: Option<String>) -> String {
        self.next += 1;
        let id = format!("prop-{}", self.next);
        self.held.push_back(Proposal { id: id.clone(), label, revision, doc, commands, set });
        while self.held.len() > MAX_PROPOSALS {
            if let Some(old) = self.held.pop_front() {
                self.retire(old.id, old.revision);
            }
        }
        id
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
    steps_for("engine.propose", p)
}

/// [`steps`] of a proposal or of one candidate, with `cmd` named in the errors.
fn steps_for(cmd: &str, p: &Value) -> Result<Vec<(String, Value)>> {
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

/// Ready to propose: no sketch open, and the model current (a proposal is measured against it).
fn ready(s: &mut Session, cmd: &str) -> Result<()> {
    if s.active_sketch.is_some() {
        return Err(bad(cmd, "a sketch is being edited: finish it first, or start the proposal with the sketch commands that make it"));
    }
    if s.model.is_stale() {
        s.refresh();
    }
    Ok(())
}

/// The preview path of `engine.propose`: the commands run on a scratch copy of `s`, and what
/// they did to it (the report, the scratch session after them, and each step's reply).
fn evaluate(s: &Session, commands: &[(String, Value)]) -> Result<(Value, Session, Vec<Value>)> {
    let (after, replies) = s.scratch().preview_steps(commands)?;
    let report = report(s, &after);
    Ok((report, after, replies))
}

fn propose(s: &mut Session, p: &Value) -> Result<Value> {
    let commands = steps(p)?;
    let label = label_of(p, &commands);
    ready(s, "engine.propose")?;
    let (mut out, _, replies) = evaluate(s, &commands)?;
    let (revision, doc) = (s.revision, s.doc.clone());
    let id = s.proposals.hold(label.clone(), revision, doc, commands.clone(), None);
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

// --- Candidate sets -------------------------------------------------------------------------

/// A body as a candidate's side of the comparison lists it: its measures, its mass from the
/// material its design gives it, and its box.
fn body_json(doc: &Document, b: &ModelBody, measure: Option<Value>) -> Value {
    let m = measure.unwrap_or_else(|| super::inspect::measure_json(b));
    let source = b.definition_name();
    let (volume, area) = (num(&m, "volume_mm3"), num(&m, "area_mm2"));
    let material = doc.body_material(source);
    let density = doc.density(source);
    let mut v = json!({
        "name": b.name,
        "volume_mm3": volume,
        "area_mm2": area,
        "material": material,
        "density_g_cm3": area.map(|_| density),
        "mass_g": volume.map(|v| v * density / 1000.0),
        "bbox": m.get("bbox").cloned().unwrap_or(Value::Null),
    });
    // A body the kernel could not measure says why.
    if let (Some(e), Some(o)) = (m.get("error"), v.as_object_mut()) {
        o.insert("error".into(), e.clone());
    }
    v
}

/// The box around some boxes (`{min, max}` each), none when there is none.
fn bbox_union<'a>(boxes: impl Iterator<Item = &'a Value>) -> Value {
    let mut out: Option<([f64; 3], [f64; 3])> = None;
    for b in boxes {
        let corner = |k: &str| -> Option<[f64; 3]> {
            let v = b.get(k)?.as_array()?;
            Some([v.first()?.as_f64()?, v.get(1)?.as_f64()?, v.get(2)?.as_f64()?])
        };
        let (Some(lo), Some(hi)) = (corner("min"), corner("max")) else { continue };
        out = Some(match out {
            None => (lo, hi),
            Some((a, b)) => ([a[0].min(lo[0]), a[1].min(lo[1]), a[2].min(lo[2])], [b[0].max(hi[0]), b[1].max(hi[1]), b[2].max(hi[2])]),
        });
    }
    out.map_or(Value::Null, |(lo, hi)| json!({"min": lo, "max": hi}))
}

/// The sums a side's bodies make: volume, area, mass (bodies without one count nothing) and box.
fn totals_json(bodies: &[Value]) -> Value {
    let sum = |k: &str| bodies.iter().filter_map(|b| num(b, k)).sum::<f64>() + 0.0;
    json!({
        "volume_mm3": sum("volume_mm3"),
        "area_mm2": sum("area_mm2"),
        "mass_g": sum("mass_g"),
        "bbox": bbox_union(bodies.iter().map(|b| &b["bbox"])),
    })
}

/// Running sums of the design-level intersection and union, `None` once any body's share is
/// unknown (an open sheet, a body that could not be measured or compared).
struct Overlap {
    intersection: Option<f64>,
    union: Option<f64>,
    identical: bool,
}

impl Overlap {
    fn new() -> Self {
        Overlap { intersection: Some(0.0), union: Some(0.0), identical: true }
    }
    fn add(&mut self, intersection: Option<f64>, union: Option<f64>) {
        self.intersection = self.intersection.zip(intersection).map(|(a, b)| a + b);
        self.union = self.union.zip(union).map(|(a, b)| a + b);
    }
    fn json(&self) -> Value {
        let iou = self.intersection.zip(self.union).and_then(|(i, u)| (u > 0.0).then_some(i / u));
        json!({"iou": iou, "intersection_mm3": self.intersection, "union_mm3": self.union, "identical": self.identical})
    }
}

/// One candidate's bodies against the current design's: each body of the candidate measured, with
/// its exact IoU with the body of the same name (as `inspect.compare` finds it, or 1 for one whose
/// geometry is unchanged), the bodies it removes, and the design-level overlap.
fn candidate_bodies(cmd: &str, before_doc: &Document, before: &ModelState, after: &Session, after_state: &ModelState) -> (Vec<Value>, Value) {
    let options = super::compare::Options::default();
    let mut bodies = Vec::with_capacity(after_state.bodies.len());
    let mut overlap = Overlap::new();
    for b in &after_state.bodies {
        let entry = match before.body(&b.name) {
            None => {
                let mut v = body_json(&after.doc, b, None);
                v["change"] = json!("created");
                overlap.identical = false;
                overlap.add(Some(0.0), num(&v, "volume_mm3"));
                v["compare"] = json!({"iou": 0.0, "intersection_mm3": 0.0, "union_mm3": num(&v, "volume_mm3"), "identical": false});
                v
            }
            Some(was) if was.geometry_key().is_some() && was.geometry_key() == b.geometry_key() => {
                let mut v = body_json(&after.doc, b, None);
                v["change"] = json!("unchanged");
                let volume = num(&v, "volume_mm3");
                overlap.add(volume, volume);
                v["compare"] = json!({"iou": volume.map(|_| 1.0), "intersection_mm3": volume, "union_mm3": volume, "identical": true});
                v
            }
            Some(was) => match super::compare::compare_pair(cmd, (&b.name, was), (&b.name, b), &options) {
                Ok((c, _)) => {
                    let mut v = body_json(&after.doc, b, c.get("b").cloned());
                    v["change"] = json!("changed");
                    if c["identical"] != json!(true) {
                        overlap.identical = false;
                    }
                    overlap.add(num(&c, "intersection_mm3"), num(&c, "union_mm3"));
                    v["compare"] = json!({
                        "iou": c["iou"],
                        "intersection_mm3": c["intersection_mm3"],
                        "union_mm3": c["union_mm3"],
                        "identical": c["identical"],
                        "only_in_current_mm3": c["only_in_a"]["volume_mm3"],
                        "only_in_candidate_mm3": c["only_in_b"]["volume_mm3"],
                        "regions": c["regions"].as_array().map_or(0, Vec::len),
                        "max_deviation_mm": c["deviation"]["max_mm"],
                        "verdict": c["verdict"],
                        "warnings": c["warnings"],
                    });
                    v
                }
                Err(e) => {
                    let mut v = body_json(&after.doc, b, None);
                    v["change"] = json!("changed");
                    overlap.identical = false;
                    overlap.add(None, None);
                    v["compare"] = json!({"iou": null, "intersection_mm3": null, "union_mm3": null, "identical": false, "error": e.to_string()});
                    v
                }
            },
        };
        bodies.push(entry);
    }
    // Bodies the candidate removes, with the measures they have now: all of each is only in the
    // current design.
    for was in before.bodies.iter().filter(|w| after_state.body(&w.name).is_none()) {
        let mut v = body_json(before_doc, was, None);
        v["change"] = json!("removed");
        overlap.identical = false;
        overlap.add(Some(0.0), num(&v, "volume_mm3"));
        v["compare"] = json!({"iou": 0.0, "intersection_mm3": 0.0, "union_mm3": num(&v, "volume_mm3"), "identical": false});
        bodies.push(v);
    }
    (bodies, overlap.json())
}

/// Evaluate one candidate on its own scratch copy of `s`, as `engine.propose` would: its reply
/// entry (without the proposal id) and, when every step ran, what to hold.
fn evaluate_candidate(s: &Session, cmd: &str, before: &ModelState, current: &Value, commands: &[(String, Value)]) -> (Value, bool) {
    let clock = crate::timing::Clock::start();
    match evaluate(s, commands) {
        Err(e) => (
            json!({
                "valid": false,
                "error": e.to_string(),
                "errors": [],
                "warnings": [],
                "steps": commands.len(),
                "replies": [],
                "bodies": [],
                "totals": null,
                "compare": null,
                "timeline": null,
                "parameters": [],
                "ms": crate::timing::round_ms(clock.ms()),
            }),
            false,
        ),
        Ok((report, after, replies)) => {
            let after_state = after.world_state();
            let (bodies, compare) = candidate_bodies(cmd, &s.doc, before, &after, &after_state);
            let kept: Vec<Value> = bodies.iter().filter(|b| b["change"] != json!("removed")).cloned().collect();
            let mut totals = totals_json(&kept);
            totals["delta"] = json!({
                "volume_mm3": delta(num(current, "volume_mm3"), num(&totals, "volume_mm3")),
                "area_mm2": delta(num(current, "area_mm2"), num(&totals, "area_mm2")),
                "mass_g": delta(num(current, "mass_g"), num(&totals, "mass_g")),
            });
            let errors = report["errors"].clone();
            let valid = errors.as_array().is_some_and(Vec::is_empty);
            (
                json!({
                    "valid": valid,
                    "error": null,
                    "errors": errors,
                    "warnings": report["warnings"],
                    "steps": commands.len(),
                    "replies": replies,
                    "bodies": bodies,
                    "totals": totals,
                    "compare": compare,
                    "timeline": report["timeline"],
                    "parameters": report["parameters"],
                    "ms": crate::timing::round_ms(clock.ms()),
                }),
                true,
            )
        }
    }
}

fn propose_candidates(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "engine.propose_candidates";
    let list = p
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| bad(cmd, "`candidates` must be an array of {label?, commands: [{command, params}]}"))?;
    if list.len() < MIN_CANDIDATES || list.len() > MAX_CANDIDATES {
        return Err(bad(cmd, format!("{MIN_CANDIDATES} to {MAX_CANDIDATES} candidates, not {}", list.len())));
    }
    let mut candidates = Vec::with_capacity(list.len());
    for (i, c) in list.iter().enumerate() {
        if !c.is_object() {
            return Err(bad(cmd, format!("candidate {i}: must be {{label?, commands: [{{command, params}}]}}")));
        }
        let commands = steps_for(cmd, c).map_err(|e| match e {
            EngineError::BadParams { cmd: of, msg } => EngineError::BadParams { cmd: of, msg: format!("candidate {i}: {msg}") },
            e => e,
        })?;
        let label = label_of(c, &commands);
        candidates.push((label, commands));
    }
    let set_label = p
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map_or_else(|| format!("{} candidates", candidates.len()), |l| l.chars().take(120).collect::<String>());
    ready(s, cmd)?;

    let before = s.world_state();
    let current_bodies: Vec<Value> = before.bodies.iter().map(|b| body_json(&s.doc, b, None)).collect();
    let mut current = totals_json(&current_bodies);
    current["bodies"] = Value::Array(current_bodies);

    let (revision, doc) = (s.revision, s.doc.clone());
    s.proposals.next_set += 1;
    let set = format!("cset-{}", s.proposals.next_set);
    let mut out = Vec::with_capacity(candidates.len());
    let mut held_any = false;
    for (i, (label, commands)) in candidates.into_iter().enumerate() {
        let (mut entry, ok) = evaluate_candidate(s, cmd, &before, &current, &commands);
        let id = ok.then(|| s.proposals.hold(label.clone(), revision, doc.clone(), commands, Some(set.clone())));
        held_any |= id.is_some();
        if let Some(o) = entry.as_object_mut() {
            o.insert("index".into(), json!(i));
            o.insert("proposal".into(), json!(id));
            o.insert("label".into(), json!(label));
            o.insert("set".into(), json!(set));
        }
        out.push(entry);
    }
    if !held_any {
        // Nothing to apply or drop: the set is already gone (dropping it answers `false`).
        s.proposals.retire(set.clone(), revision);
    }
    Ok(json!({
        "set": set,
        "revision": revision,
        "label": set_label,
        "current": current,
        "candidates": out,
        "held": s.proposals.len(),
    }))
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
    // One candidate chosen: the set's others go with it. The first step that changes the design
    // drops every held proposal, so they are listed first and dropped explicitly after, for a
    // candidate whose steps change nothing.
    let mates: Option<Vec<String>> =
        held.set.as_ref().map(|set| s.proposals.held.iter().filter(|h| h.set.as_ref() == Some(set) && h.id != id).map(|h| h.id.clone()).collect());
    // The outer `execute` owns the undo step and puts the design back if a step fails.
    for (i, (command, params)) in held.commands.iter().enumerate() {
        s.execute(command, params).map_err(|e| EngineError::Other(format!("proposal {id}: step {i} ({command}): {e}; the design is unchanged")))?;
    }
    let dropped = match (&held.set, mates) {
        (Some(set), Some(mates)) => {
            drop_set(&mut s.proposals, set, Some(id));
            Some(mates)
        }
        _ => None,
    };
    let state = s.world_state();
    let bodies: Vec<Value> = state
        .bodies
        .iter()
        .map(|b| {
            let (volume, area) = measured(b);
            json!({"name": b.name, "volume_mm3": volume, "area_mm2": area})
        })
        .collect();
    let mut out = json!({"applied": true, "proposal": id, "label": held.label, "commands": held.commands.len(), "bodies": bodies});
    if let (Some(set), Some(dropped)) = (held.set, dropped) {
        out["set"] = json!(set);
        out["dropped"] = json!(dropped);
    }
    Ok(out)
}

/// Drop every held proposal of the set `set` but `keep`; the ids dropped. The set id is retired
/// with them.
fn drop_set(ps: &mut Proposals, set: &str, keep: Option<&str>) -> Vec<String> {
    let mut gone = Vec::new();
    let mut revision = None;
    ps.held.retain(|h| {
        let member = h.set.as_deref() == Some(set) && Some(h.id.as_str()) != keep;
        if member {
            gone.push((h.id.clone(), h.revision));
            revision = Some(h.revision);
        }
        !member
    });
    let ids: Vec<String> = gone.iter().map(|(id, _)| id.clone()).collect();
    for (id, revision) in gone {
        ps.retire(id, revision);
    }
    if let Some(revision) = revision {
        ps.retire(set.to_string(), revision);
    }
    ids
}

fn drop_proposal(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "engine.drop_proposal";
    let id =
        p.get("proposal").or_else(|| p.get("id")).and_then(Value::as_str).ok_or_else(|| bad(cmd, "`proposal` must be an id from engine.propose"))?;
    if s.proposals.held.iter().any(|h| h.set.as_deref() == Some(id)) {
        let dropped = drop_set(&mut s.proposals, id, None);
        return Ok(json!({"dropped": true, "proposal": id, "proposals": dropped, "held": s.proposals.len()}));
    }
    if let Some(i) = s.proposals.held.iter().position(|h| h.id == id) {
        if let Some(gone) = s.proposals.held.remove(i) {
            s.proposals.retire(gone.id, gone.revision);
        }
        return Ok(json!({"dropped": true, "proposal": id, "held": s.proposals.len()}));
    }
    if s.proposals.stale.iter().any(|(k, _)| k == id) {
        return Ok(json!({"dropped": false, "proposal": id, "held": s.proposals.len()}));
    }
    Err(refused("unknown_proposal", format!("no proposal {id} is held (the last {MAX_PROPOSALS} are kept)"), json!({"proposal": id})))
}

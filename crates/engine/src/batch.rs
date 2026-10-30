//! Batches (#253): steps that only change the design leave the evaluation for the end of the
//! batch, or for the first step that reads the model, so a batch of deletes on a large design
//! evaluates it once instead of once per step (TinyRex: 18 `component.delete`s took over an
//! hour).
//!
//! A command that reads only the design ([`CommandSpec::reads_model`] false) changes it and the
//! evaluation waits. `component.delete` and `timeline.delete` read the model only to report and
//! settle what they broke (#241): in a batch they change the design, and the evaluation at the
//! end checks that no feature that worked before the first of them fails. When one does, the
//! deletes run again one at a time from where the first started, as they would have outside a
//! batch, and their results replace the ones the batch gave ([`Session::end_batch`]). The design,
//! the model and every reply are those of running the steps one by one.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};
use solidcraft_doc::Document;

use crate::cmd::CommandSpec;
use crate::{Checkpoint, Sel, Session, timing};

/// The deletes whose evaluation waits for the end of a batch.
const DEFERRED_DELETES: [&str; 2] = ["component.delete", "timeline.delete"];

/// A batch in progress ([`Session::begin_batch`]).
#[derive(Default)]
pub(crate) struct Deferral {
    /// The step running now.
    step: usize,
    /// The design the model was last evaluated for, while a later change waits for an evaluation.
    behind: Option<Arc<Document>>,
    /// The deletes waiting for it.
    deletes: Option<Deletes>,
    /// The command running now is a delete that leaves the evaluation for later.
    deleting: bool,
    /// The steps run again, with their results.
    replaced: Vec<(usize, std::result::Result<Value, String>)>,
}

/// Deletes since the model was last evaluated.
struct Deletes {
    /// The session before the first.
    start: Checkpoint,
    active_component: u64,
    active_occurrence_path: Option<Vec<u64>>,
    selection: Vec<Sel>,
    /// The features that worked then.
    worked: BTreeSet<u64>,
    /// Each one's step, command and parameters (as it ran them, in component frames).
    steps: Vec<(usize, &'static str, Value)>,
}

impl Session {
    /// Start a batch: until [`Session::end_batch`], a step that only changes the design leaves
    /// the evaluation for later (see the module docs).
    pub fn begin_batch(&mut self) {
        self.batch = Some(Box::default());
    }

    /// The batch's step `i` runs next.
    pub fn batch_step(&mut self, i: usize) {
        if let Some(b) = &mut self.batch {
            b.step = i;
        }
    }

    /// End the batch: evaluate what waited. Returns the steps that ran again (deletes that
    /// broke a feature, run one at a time) with their results, which replace the batch's.
    pub fn end_batch(&mut self) -> Vec<(usize, std::result::Result<Value, String>)> {
        self.flush_batch();
        self.batch.take().map(|b| b.replaced).unwrap_or_default()
    }

    /// The command running now is a delete in a batch: it changes the design and leaves the
    /// evaluation, and what it settles and reports from it, for later.
    pub(crate) fn defers_delete(&self) -> bool {
        self.batch.as_ref().is_some_and(|b| b.deleting)
    }

    /// Bring the model up to date with the design: evaluate what waited and check the deletes
    /// (running them again one at a time if one broke a feature).
    fn flush_batch(&mut self) {
        let Some(mut b) = self.batch.take() else { return };
        if let Some(previous) = b.behind.take() {
            if Arc::ptr_eq(&previous, &self.doc) { self.refresh() } else { self.refresh_from(&previous) }
        }
        if let Some(d) = b.deletes.take()
            && !crate::cmd::newly_failing(self, &d.worked).is_empty()
        {
            self.go_to(d.start);
            self.active_component = d.active_component;
            self.active_occurrence_path = d.active_occurrence_path;
            self.selection = d.selection;
            for (step, id, params) in d.steps {
                // Inside the batch: not journaled again, and the parameters are already in frames.
                let (_nested, _) = crate::frames::Nested::enter(id);
                b.replaced.push((step, self.execute(id, &params).map_err(|e| e.to_string())));
            }
            crate::crash::rerun(self);
        }
        self.batch = Some(b);
    }

    /// Before command `spec` runs in a batch (`outermost`: as a step, not inside another
    /// command): evaluate first if it needs the model (or, for the first delete, what worked
    /// before it). Whether it is a delete that leaves the evaluation for later.
    pub(crate) fn batch_before(&mut self, spec: &CommandSpec, params: &Value, live: bool, outermost: bool) -> bool {
        let Some(b) = self.batch.as_mut() else { return false };
        let delete = outermost && !live && DEFERRED_DELETES.contains(&spec.id);
        // (A step refused before it ran left no delete running.)
        if outermost {
            b.deleting = false;
        }
        // Going back to a checkpoint from before the deletes takes them back.
        if spec.id == "edit.restore_checkpoint" && params.get("forget").and_then(Value::as_bool) != Some(true) {
            b.deletes = None;
        }
        let flush = (b.deletes.is_some() && !delete) || (b.deletes.is_none() && b.behind.is_some() && (spec.reads_model || live));
        if flush {
            self.flush_batch();
        }
        if !delete {
            return false;
        }
        if self.batch.as_ref().is_some_and(|b| b.deletes.is_none()) {
            let deletes = Deletes {
                start: self.mark_with_history(),
                active_component: self.active_component,
                active_occurrence_path: self.active_occurrence_path.clone(),
                selection: self.selection.clone(),
                worked: crate::cmd::working_features(self),
                steps: Vec::new(),
            };
            if let Some(b) = self.batch.as_mut() {
                b.deletes = Some(deletes);
            }
        }
        if let Some(b) = self.batch.as_mut() {
            b.deleting = true;
        }
        true
    }

    /// After a command that changed the design (from `previous`) in a batch: whether its
    /// evaluation waits (a delete that left it, `deleted`, or a command that reads only the
    /// design).
    pub(crate) fn batch_defers(&mut self, spec: &CommandSpec, deleted: bool, previous: &Arc<Document>, render_only: bool) -> bool {
        let Some(b) = self.batch.as_mut() else { return false };
        // A change to how the design is drawn is applied now, unless an evaluation waits anyway.
        let defers = deleted || (!spec.reads_model && (!render_only || b.behind.is_some()));
        if defers && b.behind.is_none() {
            b.behind = Some(previous.clone());
        }
        defers
    }

    /// After command `id` ran in a batch (`ok`: it succeeded, with `params`).
    pub(crate) fn batch_after(&mut self, id: &'static str, params: &Value, ok: bool) {
        let Some(b) = self.batch.as_mut() else { return };
        if b.deleting
            && ok
            && let Some(d) = &mut b.deletes
        {
            d.steps.push((b.step, id, params.clone()));
        }
        b.deleting = false;
    }

    /// A failed command in a batch restored the design it started from: its evaluation waits too.
    pub(crate) fn batch_failed(&mut self) -> bool {
        let Some(b) = self.batch.as_mut() else { return false };
        b.behind.get_or_insert_with(|| self.doc.clone());
        true
    }

    /// The model was evaluated for the design as it is.
    pub(crate) fn batch_evaluated(&mut self) {
        if let Some(b) = &mut self.batch {
            b.behind = None;
        }
    }
}

/// How a batch runs: undone when a step fails (`rollback`), with each step's time (`timed`) and
/// kernel share (`profile`).
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub rollback: bool,
    pub timed: bool,
    pub profile: bool,
}

impl Options {
    /// From a batch request's `rollback` (default true), `timings` and `profile`.
    pub fn of(p: &Value) -> Options {
        let flag = |k: &str| p.get(k).and_then(Value::as_bool);
        let profile = flag("profile").unwrap_or(false);
        Options { rollback: flag("rollback").unwrap_or(true), timed: profile || flag("timings").unwrap_or(false), profile }
    }
}

/// Run `steps` (`{command, params}`) as one batch on the session `session(ctx)` gives, each
/// through `exec`, so the design is evaluated once (see the module docs): `{completed, results,
/// undone, failed?, rolled_back?, timings?, total_ms?}`. The first step that fails stops the
/// batch and, with `rollback`, the design goes back to where the batch started.
pub fn run<C>(
    ctx: &mut C,
    steps: &[Value],
    opts: Options,
    session: impl Fn(&mut C) -> &mut Session,
    mut exec: impl FnMut(&mut C, &str, Value) -> std::result::Result<Value, String>,
) -> Value {
    let command = |step: &Value| step.get("command").or_else(|| step.get("id")).and_then(Value::as_str).map(str::to_string);
    let mut rows = Vec::new();
    let clock = timing::Clock::start();
    let checkpoint =
        if opts.rollback { session(ctx).execute("edit.checkpoint", &json!({})).ok().and_then(|v| v["checkpoint"].as_u64()) } else { None };
    session(ctx).begin_batch();
    let mut results = Vec::with_capacity(steps.len());
    let mut failed = None;
    for (i, step) in steps.iter().enumerate() {
        session(ctx).batch_step(i);
        let c = command(step);
        let (r, took) = timing::time(opts.profile, || match &c {
            Some(id) => exec(ctx, id, step.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({}))),
            None => Err("missing `command`".to_string()),
        });
        if opts.timed {
            rows.push(took.to_json(i, c.as_deref().unwrap_or_default()));
        }
        match r {
            Ok(v) => results.push(v),
            Err(e) => {
                failed = Some((i, e));
                break;
            }
        }
    }
    // Going back evaluates the design it goes back to; the deletes' check comes first, as their
    // results are reported (a design changed by commands that read only it is not evaluated).
    let s = session(ctx);
    let back_first = failed.is_some() && checkpoint.is_some() && s.batch.as_ref().is_some_and(|b| b.deletes.is_none());
    let restored = back_first && checkpoint.is_some_and(|id| s.execute("edit.restore_checkpoint", &json!({"checkpoint": id})).is_ok());
    let mut replaced = s.end_batch();
    // A step run again that failed (which a delete run one at a time never should) is where the
    // batch failed.
    replaced.sort_by_key(|(i, _)| *i);
    for (i, r) in replaced {
        match r {
            Ok(v) => {
                if let Some(slot) = results.get_mut(i) {
                    *slot = v;
                }
            }
            Err(e) if failed.as_ref().is_none_or(|(j, _)| i < *j) => {
                results.truncate(i);
                failed = Some((i, e));
            }
            Err(_) => {}
        }
    }
    let mut reply = match failed {
        Some((i, e)) => {
            let restored = restored
                || (!back_first && checkpoint.is_some_and(|id| session(ctx).execute("edit.restore_checkpoint", &json!({"checkpoint": id})).is_ok()));
            let failed = json!({"index": i, "command": steps.get(i).and_then(command), "error": e});
            // `rolled_back`: the design really is back at its checkpoint.
            json!({"completed": i, "results": results, "failed": failed, "undone": if restored { i } else { 0 }, "rolled_back": restored})
        }
        None => {
            if let Some(id) = checkpoint {
                let _ = session(ctx).execute("edit.restore_checkpoint", &json!({"checkpoint": id, "forget": true}));
            }
            json!({"completed": results.len(), "results": results, "undone": 0})
        }
    };
    if opts.timed
        && let Some(o) = reply.as_object_mut()
    {
        o.insert("timings".into(), Value::Array(rows));
        o.insert("total_ms".into(), json!(timing::round_ms(clock.ms())));
    }
    reply
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::Options;
    use crate::Session;

    fn run(s: &mut Session, id: &str, p: Value) -> Value {
        match s.execute(id, &p) {
            Ok(v) => v,
            Err(e) => panic!("{id} {p}: {e}"),
        }
    }

    /// A plate in the design, `parts` components with a box and a cylinder joined to it, and a
    /// tail of `tail` boxes after them in the timeline (which each delete moves).
    fn design(parts: usize, tail: usize) -> Session {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 400, "width": 20, "height": 5, "body_name": "Plate"}));
        for i in 0..parts {
            let x = 30.0 * i as f64;
            run(&mut s, "component.create", json!({"name": format!("Part{i}")}));
            run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [x, 40, 0], "body_name": format!("Block{i}")}));
            run(&mut s, "solid.cylinder", json!({"base": [x + 5.0, 45, 10], "radius": 2, "height": 5, "body_name": format!("Peg{i}")}));
            run(&mut s, "solid.combine", json!({"target": format!("Block{i}"), "tools": [format!("Peg{i}")], "operation": "join"}));
            run(&mut s, "component.activate", json!({"component": "root"}));
        }
        for i in 0..tail {
            run(
                &mut s,
                "solid.box",
                json!({"length": 5, "width": 5, "height": 5, "corner": [10.0 * i as f64, -40, 0], "body_name": format!("Tail{i}"), "name": format!("Tail{i}")}),
            );
        }
        s
    }

    /// What the steps leave: the design, and each feature's result and the bodies (by name and
    /// volume, to 1e-9 relative).
    fn outcome(s: &Session) -> (String, Vec<(u64, Option<String>, bool)>, Vec<(String, f64)>) {
        assert!(s.model.is_current(&s.doc), "the model is the design's evaluation");
        let results = s.model.results.iter().map(|r| (r.id, r.error.clone(), r.skipped)).collect();
        let bodies = s
            .model
            .state()
            .bodies
            .iter()
            .map(|b| (b.name.clone(), b.body.tessellate_with(1e-3, true).map_or(f64::NAN, |m| m.measure().volume)))
            .collect();
        (s.doc.to_json(), results, bodies)
    }

    fn same(a: &Session, b: &Session) {
        let ((da, ra, ba), (db, rb, bb)) = (outcome(a), outcome(b));
        assert_eq!(da, db, "the same design");
        assert_eq!(ra, rb, "the same results");
        assert_eq!(ba.len(), bb.len(), "{ba:?} against {bb:?}");
        for ((na, va), (nb, vb)) in ba.iter().zip(&bb) {
            assert_eq!(na, nb);
            assert!((va - vb).abs() <= 1e-9 * va.abs().max(1.0), "{na}: {va} against {vb}");
        }
        assert_eq!(a.undo.len(), b.undo.len(), "the same undo steps");
    }

    /// `steps` one at a time, and as a batch on a copy: the replies, design, model and undo
    /// history agree. The batch's evaluations.
    fn batched_as_one_at_a_time(s: &Session, steps: &[Value]) -> (Value, usize) {
        let mut one = s.scratch();
        let mut each = Vec::new();
        for step in steps {
            each.push(run(&mut one, step["command"].as_str().unwrap_or_default(), step["params"].clone()));
        }
        let mut batched = s.scratch();
        batched.evaluations = 0;
        let opts = Options { rollback: true, timed: false, profile: false };
        let reply = super::run(&mut batched, steps, opts, |s| s, |s, id, p| s.execute(id, &p).map_err(|e| e.to_string()));
        assert_eq!(reply["completed"], json!(steps.len()), "{reply}");
        assert_eq!(reply["results"], json!(each), "the same replies");
        same(&batched, &one);
        (reply, batched.evaluations)
    }

    /// #253: ten component deletes in a batch evaluate the design once, and leave the design,
    /// model, replies and undo history that deleting them one at a time leaves.
    #[test]
    fn deletes_in_a_batch_evaluate_once() {
        let s = design(10, 20);
        let steps: Vec<Value> = (0..10).map(|i| json!({"command": "component.delete", "params": {"component": format!("Part{i}")}})).collect();
        let mut one = s.scratch();
        one.evaluations = 0;
        for step in &steps {
            run(&mut one, "component.delete", step["params"].clone());
        }
        assert!(one.evaluations >= 10, "one at a time, each delete evaluates: {}", one.evaluations);
        let (_, evaluations) = batched_as_one_at_a_time(&s, &steps);
        assert_eq!(evaluations, 1, "the batch evaluates once");
        // Timeline deletes and commands that only change the design, mixed in, too: a step after
        // deletes that is not one evaluates them first (it could not be run again with them).
        let steps = vec![
            json!({"command": "timeline.delete", "params": {"feature": "Tail3"}}),
            json!({"command": "document.metadata.set", "params": {"key": "description", "value": "fewer parts"}}),
            json!({"command": "component.delete", "params": {"component": "Part2"}}),
            json!({"command": "timeline.delete", "params": {"features": ["Tail4", "Tail5"]}}),
            json!({"command": "timeline.group", "params": {"name": "Rest", "features": ["Tail6", "Tail7"]}}),
        ];
        let (_, evaluations) = batched_as_one_at_a_time(&s, &steps);
        // Before each of the two other steps, before the delete after the first (what worked
        // then), and at the end.
        assert_eq!(evaluations, 4);
    }

    /// A step that reads the model sees the deletes before it: the batch evaluates there.
    #[test]
    fn a_step_reading_the_model_sees_the_deletes_before_it() {
        let s = design(3, 2);
        let steps = vec![
            json!({"command": "component.delete", "params": {"component": "Part0"}}),
            json!({"command": "inspect.measure", "params": {"bodies": ["Plate"]}}),
            json!({"command": "component.delete", "params": {"component": "Part1"}}),
            json!({"command": "component.list", "params": {}}),
        ];
        batched_as_one_at_a_time(&s, &steps);
    }

    /// Deletes that break a feature that worked (a cut using their bodies as tools) run again one
    /// at a time, so what they settle and report is what one delete after another gives (#241):
    /// the cut first drops one tool, then is suppressed with the other.
    #[test]
    fn deletes_that_break_a_feature_settle_it_as_one_at_a_time() {
        let mut s = design(2, 0);
        run(
            &mut s,
            "solid.combine",
            json!({"target": "Plate", "tools": ["Block0", "Block1"], "operation": "cut", "keep_tools": true, "name": "Cut"}),
        );
        run(&mut s, "solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [0, -40, 0], "body_name": "After"}));
        let steps = vec![
            json!({"command": "component.delete", "params": {"component": "Part0"}}),
            json!({"command": "component.delete", "params": {"component": "Part1"}}),
        ];
        let (reply, _) = batched_as_one_at_a_time(&s, &steps);
        assert_eq!(reply["results"][0]["tool_features"], json!([{"feature": "Cut", "tools_removed": ["Block0"]}]), "{reply}");
        assert_eq!(reply["results"][1]["tool_features"], json!([{"feature": "Cut", "suppressed": true, "tools_removed": ["Block1"]}]), "{reply}");
    }

    /// A failing step after deletes takes the batch back to where it started, evaluated.
    #[test]
    fn a_failing_step_after_deletes_rolls_the_batch_back() {
        let mut s = design(3, 2);
        let before = s.doc.to_json();
        let undo = s.undo.len();
        let steps = vec![
            json!({"command": "component.delete", "params": {"component": "Part0"}}),
            json!({"command": "timeline.delete", "params": {"feature": "Tail1"}}),
            json!({"command": "component.delete", "params": {"component": "NoSuchPart"}}),
        ];
        let opts = Options { rollback: true, timed: false, profile: false };
        let reply = super::run(&mut s, &steps, opts, |s| s, |s, id, p| s.execute(id, &p).map_err(|e| e.to_string()));
        assert_eq!((reply["completed"].clone(), reply["rolled_back"].clone()), (json!(2), json!(true)), "{reply}");
        assert_eq!(reply["failed"]["index"], json!(2), "{reply}");
        assert_eq!(s.doc.to_json(), before);
        assert_eq!(s.undo.len(), undo);
        assert!(s.model.is_current(&s.doc), "the model is the design's evaluation");
        assert!(s.batch.is_none(), "the batch is over");
    }
}

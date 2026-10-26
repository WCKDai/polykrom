//! Rebasing a feature's result onto a changed model (#169).
//!
//! The evaluation cache keys a feature by the whole model before it, so an edit early in a long
//! timeline evaluated every feature after it again, even those that never read what changed: each
//! part of a large assembly building its bodies from its own sketches. A feature whose own inputs
//! are unchanged, and whose last evaluation started from a model that is the same as the model
//! now in everything the feature reads, made a change to that model that it would make again:
//! the bodies it added, replaced or removed, the sketch it solved, the threads it added. That
//! change is applied to the model as it is now instead of evaluating the feature.
//!
//! Only what can be shown to hold is rebased; everything else is evaluated:
//! - the feature's own inputs: its definition, the parameters it uses and the placements it reads
//!   (the cache's local key), and the document context, are those of its last evaluation;
//! - its kind is one whose reads are known ([`reads`]): a sketch on a fixed plane without
//!   projections, a new body from its own sketch or a primitive, a join or cut (its named targets,
//!   its component's bodies, or every body when it names none, as the tool shaping then looks at
//!   every body), a split by a fixed plane, a body transfer. Never a pattern or mirror, a feature
//!   with face references (found by name over every body), To / From Object or a construction axis;
//! - what it reads is the same in both models: the same bodies (shared geometry, names, key,
//!   owner) in the same order, the same solved sketches; no sheet metal anywhere before it;
//! - the change can be told and applied by name: bodies kept in their order, bodies it made after
//!   them, named as the feature stores them (never `BodyN` from the counter, a `(k)` number or a
//!   name the model now has), a cut or join making no new bodies, threads only added;
//! - the names of the faces it made read only what it read: from the history of its operands, or
//!   a new body's own roles, never the geometry of bodies near it; named the same way however deep
//!   the chain of features naming them is now (whose depth they take), and the design has no names
//!   saved before format 14.
//!
//! `POLYKROM_REBASE_SHADOW` (tests and diagnostics only) evaluates every rebased feature as well
//! and compares the two. A difference is logged and counted ([`shadow_mismatches`]; a shadow run
//! passes only with none, and this crate's tests fail on it), and the evaluated result is kept.

use super::*;

/// What of the model before it a feature reads, besides its own sketches.
enum Reads<'a> {
    /// Nothing of the bodies.
    Nothing,
    /// These bodies, by name.
    Named(Vec<&'a str>),
    /// Its component's bodies.
    Component,
    /// Every body.
    All,
}

/// Bodies named by a feature: every body for none or `*`.
fn named(names: &[crate::BodyName]) -> Reads<'_> {
    match names.iter().any(|n| n.0 == "*") || names.is_empty() {
        true => Reads::All,
        false => Reads::Named(names.iter().map(|n| n.0.as_str()).collect()),
    }
}

/// The bodies and sketches feature `f` reads, when it is of a kind that may be rebased.
fn reads(f: &Feature) -> Option<(Reads<'_>, Option<u64>)> {
    // A new body from its own sketch, or a join or cut: its named targets, else every body (the
    // tool is shaped around every body then).
    fn solid<'a>(f: &Feature, op: &Operation, targets: &'a [crate::BodyName]) -> Option<Reads<'a>> {
        match op {
            Operation::NewBody => (targets.is_empty() && f.participants.is_empty()).then_some(Reads::Nothing),
            Operation::Cut | Operation::Join => Some(named(targets)),
            _ => None,
        }
    }
    let primitive = |op: &Operation| match (op, f.participants.as_slice()) {
        (Operation::NewBody, []) => Some(Reads::Nothing),
        (Operation::Cut | Operation::Join, []) => Some(Reads::Component),
        (Operation::Cut | Operation::Join, p) if p.iter().any(|n| n == "*") => Some(Reads::All),
        (Operation::Cut | Operation::Join, p) => Some(Reads::Named(p.iter().map(String::as_str).collect())),
        _ => None,
    };
    if f.shell_piece_order || f.face_names.iter().any(|n| !n.is_empty()) {
        return None;
    }
    match &f.kind {
        FeatureKind::Sketch { plane, sketch } if plane_is_fixed(plane) && sketch.links.is_empty() && sketch.wires.is_empty() => {
            Some((Reads::Nothing, None))
        }
        FeatureKind::Extrude { sketch, extent, operation, targets, .. } if extent.to.is_none() && extent.from.is_none() => {
            let new_through_all = *operation == Operation::NewBody && extent.through_all;
            Some((solid(f, operation, targets).filter(|_| !new_through_all)?, Some(sketch.0)))
        }
        FeatureKind::Revolve { sketch, axis, operation, targets, to: None, .. } if !matches!(axis, AxisRef::Construction { .. }) => {
            Some((solid(f, operation, targets)?, Some(sketch.0)))
        }
        FeatureKind::Box { operation, .. }
        | FeatureKind::Cylinder { operation, .. }
        | FeatureKind::Sphere { operation, .. }
        | FeatureKind::Torus { operation, .. } => Some((primitive(operation)?, None)),
        FeatureKind::Split { body, plane, tool: None, sketch: None, line: None } if plane_is_fixed(plane) => {
            Some((Reads::Named(vec![body.0.as_str()]), None))
        }
        FeatureKind::BodyTransfer { bodies, .. } => Some((named(bodies), None)),
        _ => None,
    }
}

/// A body's faces are named from geometry only over nothing but itself: a new solid of its own.
fn names_its_own(f: &Feature) -> bool {
    match &f.kind {
        FeatureKind::Box { operation, .. }
        | FeatureKind::Cylinder { operation, .. }
        | FeatureKind::Sphere { operation, .. }
        | FeatureKind::Torus { operation, .. }
        | FeatureKind::Extrude { operation, .. }
        | FeatureKind::Revolve { operation, .. } => *operation == Operation::NewBody,
        _ => false,
    }
}

/// The same body, as far as any feature can tell: shared geometry and names, the same identity
/// and owner.
fn same_body(a: &ModelBody, b: &ModelBody) -> bool {
    a.name == b.name
        && a.body.same(&b.body)
        && a.names.id() == b.names.id()
        && a.key == b.key
        && a.feature == b.feature
        && a.component == b.component
        && a.transferred == b.transferred
        && a.source_name == b.source_name
        && a.source_transform == b.source_transform
        && a.imported.as_ref().map(Arc::as_ptr) == b.imported.as_ref().map(Arc::as_ptr)
}

/// The same solved sketch `id` in both models (or in neither).
fn same_sketch(a: &ModelState, b: &ModelState, id: u64) -> bool {
    let json = |s: &SolvedSketch| serde_json::to_string(&(&s.plane, &s.sketch, &s.report)).ok();
    match (a.sketches.iter().find(|s| s.feature == id), b.sketches.iter().find(|s| s.feature == id)) {
        (Some(x), Some(y)) => Arc::ptr_eq(x, y) || (a.sketch_keys.get(&id) == b.sketch_keys.get(&id) && json(x).is_some() && json(x) == json(y)),
        (None, None) => true,
        _ => false,
    }
}

/// How deep the chain of features naming the model's bodies is (see `ModelState::stamp`).
fn depth(st: &ModelState) -> usize {
    st.bodies.iter().filter_map(|b| b.names.depth()).max().unwrap_or(0)
}

/// The bodies of `st` that `reads` takes, in their order.
fn read_bodies<'a>(doc: &Document, f: &Feature, st: &'a ModelState, reads: &Reads) -> Option<Vec<&'a ModelBody>> {
    let mine = |b: &&ModelBody| b.component == f.component || b.feature == f.id || doc.model_body_component(b) == f.component;
    Some(match reads {
        Reads::Nothing => Vec::new(),
        Reads::Named(names) => names.iter().map(|n| st.body(n)).collect::<Option<_>>()?,
        Reads::Component => st.bodies.iter().filter(mine).collect(),
        Reads::All => st.bodies.iter().collect(),
    })
}

/// Whether the operands a body's faces are named from are among `read` (bodies by name).
fn operands_read(lineage: &crate::lineage::Lineage, read: &dyn Fn(&str) -> bool) -> bool {
    use crate::lineage::Operand;
    lineage.operands.iter().all(|op| match op.as_ref() {
        Operand::Body(n) | Operand::Copy { body: n, .. } | Operand::Inner(n) => read(n),
        Operand::Stage(stage) => operands_read(&stage.lineage, read),
        Operand::Made(_) | Operand::CopiedTool { .. } => true,
    })
}

/// `f`'s result `after`, evaluated from `was`, as `f` would leave `now` instead: `None` when that
/// cannot be shown (see the module notes).
fn rebase(doc: &Document, f: &Feature, was: &ModelState, after: &ModelState, now: &ModelState) -> Option<ModelState> {
    let (reads, sketch) = reads(f)?;
    // Nothing either model holds that the change does not carry: sheets, construction geometry,
    // blends, the body counter; and names worked out as deep in both.
    let untouched = [was, after, now].iter().all(|s| s.sheets.is_empty())
        && after.body_counter == was.body_counter
        && after.construct == was.construct
        && after.blend_edges == was.blend_edges
        && after.fitted == was.fitted
        && after.tapered == was.tapered;
    // The chain of features naming what it makes is as deep as the model now makes it, named the
    // same way (past `MAX_DEPTH` by position).
    let (was_depth, now_depth) = (depth(was) + 1, depth(now) + 1);
    let deep = crate::naming::MAX_DEPTH;
    if !untouched || (was_depth < deep) != (now_depth < deep) || !sketch.is_none_or(|id| same_sketch(was, now, id)) {
        return None;
    }
    let (old, new) = (read_bodies(doc, f, was, &reads)?, read_bodies(doc, f, now, &reads)?);
    if old.len() != new.len() || !old.iter().zip(&new).all(|(a, b)| same_body(a, b)) {
        return None;
    }
    let read = |n: &str| matches!(reads, Reads::All) || old.iter().any(|b| b.name == n);
    // The bodies before and after, by name: kept in their order, changed in place, gone, or made
    // after all of them.
    let position: std::collections::HashMap<&str, usize> = was.bodies.iter().enumerate().map(|(i, b)| (b.name.as_str(), i)).collect();
    if position.len() != was.bodies.len() {
        return None;
    }
    let (mut last, mut changed, mut made) = (None, std::collections::HashMap::new(), Vec::new());
    for b in &after.bodies {
        match position.get(b.name.as_str()) {
            Some(i) if made.is_empty() && last.is_none_or(|l| *i > l) => {
                last = Some(*i);
                if !was.bodies.get(*i).is_some_and(|w| same_body(w, b)) {
                    changed.insert(b.name.as_str(), b);
                }
            }
            Some(_) => return None,
            None => made.push(b),
        }
    }
    let kept: std::collections::HashSet<&str> = after.bodies.iter().map(|b| b.name.as_str()).collect();
    let gone: Vec<&str> = was.bodies.iter().map(|b| b.name.as_str()).filter(|n| !kept.contains(n)).collect();
    let boolean = matches!(&f.kind, FeatureKind::Extrude { operation, .. } | FeatureKind::Revolve { operation, .. } | FeatureKind::Box { operation, .. }
        | FeatureKind::Cylinder { operation, .. } | FeatureKind::Sphere { operation, .. } | FeatureKind::Torus { operation, .. } if *operation != Operation::NewBody);
    // New bodies take the names the feature stores, in order, free in the model now.
    let named = made
        .iter()
        .enumerate()
        .all(|(k, b)| f.body_names.get(k).is_some_and(|n| !n.trim().is_empty() && *n == b.name) && now.body(&b.name).is_none());
    if (boolean && !made.is_empty()) || !named || !changed.keys().chain(&gone).all(|n| read(n)) {
        return None;
    }
    // The names of what it made read only what it read.
    let transfer = matches!(f.kind, FeatureKind::BodyTransfer { .. });
    let names_read = changed.values().chain(&made).all(|b| {
        transfer
            || (b.names.depth() == Some(was_depth)
                && b.lineage().is_none_or(|l| operands_read(&l, &read))
                && (names_its_own(f) || !crate::naming::names(b).geometric))
    });
    // Threads only added, none on a body the feature removes.
    let threads = after.threads.strip_prefix(was.threads.as_slice())?;
    if !names_read || now.threads.iter().any(|t| gone.contains(&t.body.as_str())) {
        return None;
    }
    // The sketch it solved, after the others.
    let others: Vec<&Arc<SolvedSketch>> = was.sketches.iter().filter(|s| s.feature != f.id).collect();
    let solved = after.sketches.get(others.len()..)?;
    if !others.iter().zip(&after.sketches).all(|(a, b)| Arc::ptr_eq(a, b)) || solved.iter().any(|s| s.feature != f.id) {
        return None;
    }
    let deepened = |b: &ModelBody| match transfer || now_depth == was_depth {
        true => b.clone(),
        false => ModelBody { names: b.names.at_depth(now_depth), ..b.clone() },
    };
    let mut st = now.clone();
    st.bodies = now
        .bodies
        .iter()
        .filter(|b| !gone.contains(&b.name.as_str()))
        .map(|b| match changed.get(b.name.as_str()) {
            Some(c) => deepened(c),
            None => ModelBody { made: crate::lineage::Made::Before, ..b.clone() },
        })
        .chain(made.into_iter().map(deepened))
        .collect();
    st.threads.extend_from_slice(threads);
    st.sketches.retain(|s| s.feature != f.id);
    st.sketches.extend(solved.iter().cloned());
    match after.sketch_keys.get(&f.id) {
        Some(k) => st.sketch_keys.insert(f.id, *k),
        None => st.sketch_keys.remove(&f.id),
    };
    Some(st)
}

/// The last evaluation's results, offered for rebasing by [`Model::evaluate`].
pub(super) struct Previous<'a> {
    results: &'a [FeatureResult],
    at: std::collections::HashMap<u64, usize>,
}

impl<'a> Previous<'a> {
    pub(super) fn of(results: &'a [FeatureResult]) -> Previous<'a> {
        Previous { results, at: results.iter().enumerate().map(|(i, r)| (r.id, i)).collect() }
    }

    /// Feature `f` (`local`: its own inputs, as the cache keys them) rebased onto `now`: the state
    /// and its last result (for its warnings and time).
    pub(super) fn rebase(&self, doc: &Document, f: &Feature, local: u64, now: &ModelState) -> Option<(ModelState, &'a FeatureResult)> {
        let i = *self.at.get(&f.id)?;
        let old = self.results.get(i).filter(|r| r.local == local && r.local != 0 && !r.skipped && !r.stale && !r.unkept && r.error.is_none())?;
        let was = self.results.get(..i)?.iter().rev().find(|r| !r.skipped).filter(|r| !r.unkept && !r.stale)?;
        Some((rebase(doc, f, &was.state, &old.state, now)?, old))
    }
}

/// `POLYKROM_REBASE_SHADOW`: evaluate rebased features as well, and compare.
pub(super) fn shadow() -> bool {
    solidcraft_geom::env::var_os("REBASE_SHADOW").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Rebased results that shadow mode found differing from an evaluation, in this process.
static MISMATCHES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many rebased results shadow mode (`POLYKROM_REBASE_SHADOW`) found differing from an
/// evaluation in this process: a shadow run passes only with none.
pub fn shadow_mismatches() -> usize {
    MISMATCHES.load(std::sync::atomic::Ordering::Relaxed)
}

/// Shadow mode: whether `rebased` is what evaluating `f` gave (`evaluated`, with its `error`,
/// `warning` and diagnostics) in every way a later feature or the user can see. A difference is
/// logged (and noted in the file the variable names, with every other check) and counted
/// ([`shadow_mismatches`]); the evaluated result is then kept. The crate's own tests fail on it.
pub(super) fn shadow_check(
    f: &Feature,
    rebased: &ModelState,
    old: &FeatureResult,
    evaluated: &ModelState,
    error: Option<&str>,
    warning: Option<&str>,
    diagnostics: &kernel::BooleanDiagnostics,
) -> bool {
    let difference = differs(rebased, evaluated).or_else(|| match () {
        _ if error.is_some() => Some(format!("evaluating it failed: {error:?}")),
        _ if old.warning.as_deref() != warning => Some(format!("warning {:?} vs {warning:?}", old.warning)),
        _ if &old.boolean_diagnostics != diagnostics => Some("boolean diagnostics".into()),
        _ => None,
    });
    let note = match &difference {
        Some(d) => format!("MISMATCH {} {} ({}): {d}", f.kind.type_name(), f.id, f.name),
        None => format!("ok {} {}", f.kind.type_name(), f.id),
    };
    if let Some(path) = solidcraft_geom::env::var("REBASE_SHADOW").filter(|v| v != "1") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{note}");
        }
    }
    if difference.is_some() {
        MISMATCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        log::error!("rebase shadow check: {note}");
        eprintln!("rebase shadow check: {note}");
    }
    // This crate's tests fail on it.
    #[cfg(test)]
    assert!(difference.is_none(), "rebase shadow check: {note}");
    difference.is_none()
}

/// What tells two models apart (`None`: nothing): bodies by name, owner, exact geometry, volume
/// and face names; sketches, threads, sheets, construction geometry and the rest. Body keys are
/// not compared: a rebased body keeps the identity of the evaluation that made it.
pub(super) fn differs(a: &ModelState, b: &ModelState) -> Option<String> {
    if a.bodies.len() != b.bodies.len() {
        return Some(format!("{} bodies vs {}", a.bodies.len(), b.bodies.len()));
    }
    for (x, y) in a.bodies.iter().zip(&b.bodies) {
        let what = |w: &str| Some(format!("body `{}`: {w}", x.name));
        if (&x.name, x.feature, x.component, x.transferred, &x.source_name) != (&y.name, y.feature, y.component, y.transferred, &y.source_name)
            || x.source_transform != y.source_transform
        {
            return Some(format!("body `{}` vs `{}`", x.name, y.name));
        }
        if x.names.depth() != y.names.depth() {
            return what(&format!("naming depth {:?} vs {:?}", x.names.depth(), y.names.depth()));
        }
        if x.body.same(&y.body) && x.names.id() == y.names.id() {
            continue;
        }
        let (ex, ey) = (kernel::encode_native_body(&x.body).ok(), kernel::encode_native_body(&y.body).ok());
        if ex.is_none() || ex != ey {
            return what("B-rep");
        }
        // Volumes to a millionth of a cubic millimetre per cubic metre.
        let volume = |m: &ModelBody| kernel::measure(&m.body).ok().map(|m| m.volume);
        match (volume(x), volume(y)) {
            (Some(v), Some(w)) if (v - w).abs() <= 1e-9 * v.abs().max(1.0) => {}
            (v, w) => return what(&format!("volume {v:?} vs {w:?}")),
        }
        let (nx, ny) = (crate::naming::names(x), crate::naming::names(y));
        let verified = |n: &crate::history_names::Names| n.info.iter().map(|i| i.verified).collect::<Vec<_>>();
        if nx.faces != ny.faces || verified(&nx) != verified(&ny) {
            return what(&format!("face names {:?} vs {:?}", nx.faces, ny.faces));
        }
    }
    let sketches = a.sketches.len() == b.sketches.len()
        && a.sketches.iter().zip(&b.sketches).all(|(x, y)| x.feature == y.feature && same_sketch(a, b, x.feature));
    let json = |s: &ModelState| serde_json::to_string(&(&s.threads, &s.sheets)).ok();
    match () {
        _ if !sketches || a.sketch_keys != b.sketch_keys => Some("sketches".into()),
        _ if json(a).is_none() || json(a) != json(b) => Some("threads or sheets".into()),
        _ if a.construct != b.construct => Some("construction geometry".into()),
        _ if (a.body_counter, &a.reserved_body_names) != (b.body_counter, &b.reserved_body_names) => Some("body counter".into()),
        _ if (&a.blend_edges, &a.fitted, &a.tapered) != (&b.blend_edges, &b.fitted, &b.tapered) => Some("blends".into()),
        _ => None,
    }
}

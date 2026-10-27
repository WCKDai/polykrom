//! Opening a design: the features of each component evaluated ahead, side by side (#234).
//!
//! A full evaluation runs every feature in timeline order, each on the model the one before it
//! left. Most of a large assembly's features read only their own component's bodies and
//! sketches (#217, #219), so the components' features can be evaluated at the same time. Each
//! component gets a lane: its features, in timeline order, evaluated on worker threads in a model
//! of their own. The timeline is still evaluated in order, on the calling thread: each feature
//! whose result can be rebased (`eval_rebase.rs`) takes its lane's result through the same proof
//! a rebase gives (what it read is the same in the lane's model as in the real one; the change it
//! made is told and applied by name), its bodies keyed as evaluating it in order keys them.
//! Anything that cannot be shown is evaluated there as before, so the model is the one serial
//! evaluation gives, to the bit: bodies, names, keys, counters, warnings and their order.
//!
//! `POLYKROM_SERIAL_EVAL=1` evaluates serially; `POLYKROM_REBASE_SHADOW` evaluates every feature
//! taken from a lane in order as well and compares the two, keys included.
//!
//! Lanes run on the kernel's worker threads (`kernel::map_with_local_pools`), each with a pool of
//! one thread, so a lane's own parallel work (triangulation) runs inside it instead of
//! multiplying the threads; one core is left for the timeline's own work.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

/// A feature its lane evaluated: the lane's model before it, the one after it (`None`: it
/// failed), its warning, boolean diagnostics and time.
pub(super) struct Ahead {
    pub before: Arc<ModelState>,
    pub after: Option<Arc<ModelState>>,
    pub warning: Option<String>,
    pub boolean_diagnostics: kernel::BooleanDiagnostics,
    pub ms: f64,
}

/// The lanes of one evaluation, running.
pub(super) struct Lanes {
    shared: Arc<Shared>,
}

struct Shared {
    /// Features evaluated ahead by id (`None`: their lane stopped before them).
    done: Mutex<std::collections::HashMap<u64, Option<Ahead>>>,
    ready: Condvar,
    stop: AtomicBool,
}

/// What a lane needs, owned: the lanes outlive no evaluation but run on threads of their own.
struct Inputs {
    doc: Document,
    vals: BTreeMap<String, Value>,
    param_errors: BTreeMap<String, String>,
    feature_component: std::collections::HashMap<u64, u64>,
}

/// `POLYKROM_SERIAL_EVAL`: evaluate every feature in timeline order on the calling thread.
fn serial() -> bool {
    solidcraft_geom::env::var_os("SERIAL_EVAL").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Fewer features than this: a lane's threads cost more than they save.
const MIN_FEATURES: usize = 32;

impl Lanes {
    /// The lanes of `doc`'s features before `marker`, started; `None` when it evaluates serially
    /// (asked to, one core, a small design, the browser).
    pub(super) fn start(
        doc: &Document,
        vals: &BTreeMap<String, Value>,
        param_errors: &BTreeMap<String, String>,
        feature_component: &std::collections::HashMap<u64, u64>,
        marker: usize,
    ) -> Option<Lanes> {
        let cores = std::thread::available_parallelism().map(usize::from).unwrap_or(1);
        if cfg!(target_arch = "wasm32") || serial() || cores < 3 || doc.features.len() < MIN_FEATURES {
            return None;
        }
        // The features of each component, in timeline order; lanes in the order they start.
        let mut lanes: Vec<(u64, Vec<usize>)> = Vec::new();
        for (i, f) in doc.features.iter().enumerate().take(marker).filter(|(_, f)| !f.suppressed) {
            match lanes.iter_mut().find(|(c, _)| *c == f.component) {
                Some((_, lane)) => lane.push(i),
                None => lanes.push((f.component, vec![i])),
            }
        }
        // A feature is waited for by its id: ids must be unique (a design's are; a document built
        // in code may not hold to it).
        let ids: Vec<u64> = lanes.iter().flat_map(|(_, lane)| lane).filter_map(|i| doc.features.get(*i)).map(|f| f.id).collect();
        if ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len() {
            return None;
        }
        let shared = Arc::new(Shared { done: Mutex::new(std::collections::HashMap::new()), ready: Condvar::new(), stop: AtomicBool::new(false) });
        let inputs =
            Inputs { doc: doc.clone(), vals: vals.clone(), param_errors: param_errors.clone(), feature_component: feature_component.clone() };
        let worker = Arc::clone(&shared);
        let started = std::thread::Builder::new().name("polykrom-lanes".into()).spawn(move || {
            // Whatever happens to the lanes (a worker that cannot start, a panic), every feature
            // gets an entry once they are done, so no one waits for one in vain.
            let _all = Fill { shared: &worker, rest: ids };
            let run = |lane: &(u64, Vec<usize>)| -> kernel::Result<()> {
                run_lane(&inputs, &worker, &lane.1);
                Ok(())
            };
            let _ = kernel::map_with_local_pools(&lanes, cores - 1, run);
        });
        started.ok().map(|_| Lanes { shared })
    }

    /// Feature `id` as its lane evaluated it, once it has (`None`: its lane stopped before it).
    pub(super) fn take(&self, id: u64) -> Option<Ahead> {
        let mut done = self.shared.done.lock().ok()?;
        loop {
            if let Some(ahead) = done.remove(&id) {
                return ahead;
            }
            done = self.shared.ready.wait(done).ok()?;
        }
    }
}

impl Drop for Lanes {
    /// The evaluation is over: the lanes stop after the feature they are on.
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

/// Evaluate the features `lane` (indices into the timeline), each on the model the one before it
/// in the lane left. Every one of them gets an entry, `None` from the first one not evaluated
/// (stopped, or a panic past the kernel's guard), so no one waits for it in vain.
fn run_lane(inputs: &Inputs, shared: &Shared, lane: &[usize]) {
    let doc = &inputs.doc;
    let features: Vec<&Feature> = lane.iter().filter_map(|i| doc.features.get(*i)).collect();
    let mut fill = Fill { shared, rest: features.iter().rev().map(|f| f.id).collect() };
    let reserved = doc.reserved_body_names.clone();
    // Names past `naming::MAX_DEPTH` features deep are positional: the lane's chain starts about
    // as deep as the timeline's is there (each feature before it that may make a body deepens it
    // by one at most), so it names its bodies the same way. Where it guesses the wrong side of
    // that depth, its results are not taken (`eval_rebase.rs`).
    let first = lane.first().copied().unwrap_or(0);
    let naming_floor = doc.features.iter().take(first).filter(|f| !f.suppressed && may_make_bodies(&f.kind)).count();
    let mut state = Arc::new(ModelState { reserved_body_names: reserved.clone(), naming_floor, ..Default::default() });
    for f in features {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        // A cosmetic thread changes no body, and the timeline evaluates it itself (its face is
        // found by name over every body): nothing for the lane to do.
        if matches!(f.kind, FeatureKind::Thread { modeled: false, .. }) {
            fill.rest.pop();
            continue;
        }
        // Keyed by the feature alone: taking it keys its bodies as the timeline does.
        let Computed { result, warning, boolean_diagnostics, ms } =
            evaluate_feature(doc, &inputs.vals, &inputs.param_errors, &inputs.feature_component, f, &state, &reserved, f.id);
        let before = state.clone();
        let after = result.ok().map(Arc::new);
        if let Some(after) = &after {
            state = after.clone();
        }
        fill.rest.pop();
        if let Ok(mut done) = shared.done.lock() {
            done.insert(f.id, Some(Ahead { before, after, warning, boolean_diagnostics, ms }));
        }
        shared.ready.notify_all();
    }
}

/// Features (by id) given no entry yet: on drop, each gets `None` unless it has one.
struct Fill<'a> {
    shared: &'a Shared,
    rest: Vec<u64>,
}

impl Drop for Fill<'_> {
    fn drop(&mut self) {
        if let Ok(mut done) = self.shared.done.lock() {
            for id in self.rest.drain(..) {
                done.entry(id).or_insert(None);
            }
        }
        self.shared.ready.notify_all();
    }
}

/// Whether a feature of this kind can make or change a body, naming it one deeper (a transfer
/// keeps the names a body has, a cosmetic thread changes no body).
fn may_make_bodies(kind: &FeatureKind) -> bool {
    !matches!(
        kind,
        FeatureKind::Sketch { .. }
            | FeatureKind::ConstructionPlane { .. }
            | FeatureKind::ConstructionAxis { .. }
            | FeatureKind::ConstructionPoint { .. }
            | FeatureKind::BodyTransfer { .. }
            | FeatureKind::Thread { modeled: false, .. }
    )
}

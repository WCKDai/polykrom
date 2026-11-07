//! The measurement behind the fillet work budget (`FILLET_WORK` in eval.rs; docs/perf.md,
//! "Fillet work budget: the measured margin"). Only a build with debug assertions has this
//! module; a release build compiles none of it and reads neither variable, so measuring changes
//! nothing it does.
//!
//! - `POLYKROM_FILLET_WORK_LOG=<file>`: every constant fillet's first attempt appends one JSON
//!   object to the file: the edges picked, the radius, the fit, the budget it ran under, the
//!   units it spent (PolyCore's `work_spent`), how it ended (`built`; `too_large`, refused for
//!   its size; `budget`, stopped by the budget; `refused` otherwise), the stage the budget ran
//!   out at, the input body's face and edge counts, the thread's name (cargo names a test's
//!   thread after it), the process's arguments (the kernel case, oracle case or script) and
//!   the process id.
//! - `POLYKROM_FILLET_WORK=<units>`: the budget of every first attempt, instead of
//!   `fillet_work`'s. High, it measures what an attempt the budget would stop costs in full;
//!   low, it shows the budget's effect without a rebuild.

use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// One first attempt, as logged.
pub(crate) struct Attempt<'a> {
    pub edges: usize,
    pub radius: f64,
    pub fit: crate::FilletFit,
    pub limit: u64,
    pub spent: u64,
    pub outcome: &'static str,
    pub stage: Option<&'static str>,
    pub body: &'a polycore::Body,
}

/// The log file named by `POLYKROM_FILLET_WORK_LOG`, opened once per process.
fn log_file() -> Option<&'static Mutex<std::fs::File>> {
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = crate::env::var("FILLET_WORK_LOG")?;
        std::fs::OpenOptions::new().create(true).append(true).open(path).ok().map(Mutex::new)
    })
    .as_ref()
}

/// The budget `POLYKROM_FILLET_WORK` gives every first attempt, if set.
pub(crate) fn budget_override() -> Option<u64> {
    static OVERRIDE: OnceLock<Option<u64>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| crate::env::var("FILLET_WORK")?.trim().parse().ok())
}

/// Append `a` to the log, when there is one.
pub(crate) fn record(a: &Attempt<'_>) {
    let Some(file) = log_file() else { return };
    let fit = match a.fit {
        crate::FilletFit::Exact => "exact",
        crate::FilletFit::Reduce => "reduce",
        crate::FilletFit::Taper => "taper",
    };
    let line = serde_json::json!({
        "edges": a.edges,
        "radius": a.radius,
        "fit": fit,
        "limit": a.limit,
        "spent": a.spent,
        "outcome": a.outcome,
        "stage": a.stage,
        "body_faces": a.body.face_count(),
        "body_edges": a.body.edge_count(),
        "thread": std::thread::current().name(),
        "argv": std::env::args().collect::<Vec<_>>(),
        "pid": std::process::id(),
    });
    if let Ok(mut f) = file.lock() {
        let _ = writeln!(f, "{line}");
    }
}

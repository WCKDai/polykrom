//! The tool call the engine is running, as the rest of the server sees it (#264).
//!
//! The server reads requests on one thread and runs them on another, one at a time. While a call
//! runs, the reader answers `ping`, `pending` and `running` from [`InFlight`] and passes
//! `notifications/cancelled` to it. A cancelled call stops at its next check: the timeline's
//! evaluation between features, the kernel's long work through its watch, and `batch` between
//! steps ([`cancelled`]). The server then takes it back.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

use polykrom_engine::doc::progress::Progress;
use serde_json::{Value, json};

/// A `tools/call` while it runs.
pub(crate) struct InFlight {
    /// The request's JSON-RPC id.
    pub id: Value,
    pub tool: String,
    pub started: Instant,
    /// Shared with the evaluation and the kernel ([`polykrom_engine::doc::progress::watch`]).
    pub progress: Arc<Progress>,
    /// The client's `_meta.progressToken`: progress notifications go out under it.
    pub progress_token: Option<Value>,
    /// Cancels the call where it really runs (a running app), when it isn't in this process.
    pub canceller: Option<crate::backend::Canceller>,
}

impl InFlight {
    /// What `pending` and `running` say while this call runs.
    pub fn status(&self) -> Value {
        let (done, total) = self.progress.features();
        let mut v = json!({
            "running": true,
            "tool": self.tool,
            "elapsed_s": (self.started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            "cancelling": self.progress.is_cancelled(),
        });
        if total > 0 {
            v["features"] = json!({"evaluated": done, "of": total});
        }
        if let Some((stage, done, total)) = self.progress.work() {
            v["work"] = json!({"stage": stage, "done": done, "of": total});
        }
        v["note"] = Value::String(if self.progress.is_cancelled() {
            "cancelling: it stops at its next check, and the design goes back to where it was before the call".into()
        } else {
            format!("`{}` is still running. Wait for its reply, or cancel it (notifications/cancelled with its request id)", self.tool)
        });
        v
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Progress>>> = const { RefCell::new(None) };
}

/// Run `f` as the call `progress` belongs to, on this thread.
pub(crate) fn running<R>(progress: Arc<Progress>, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|c| c.replace(Some(progress)));
    struct Restore(Option<Option<Arc<Progress>>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                CURRENT.with(|c| *c.borrow_mut() = previous);
            }
        }
    }
    let _restore = Restore(Some(previous));
    f()
}

/// Whether the call running on this thread was cancelled.
pub(crate) fn cancelled() -> bool {
    CURRENT.with(|c| c.borrow().as_ref().is_some_and(|p| p.is_cancelled()))
}

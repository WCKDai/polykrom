//! Whether the command running on this thread has been cancelled (#264).
//!
//! The caller watches the work: `polykrom_doc::progress::watch` for an MCP call or a file load
//! in the app, or `polycore::watch::watching`. The timeline's evaluation and the kernel's
//! watched work stop by themselves. The engine's own long loops ask [`requested`] where they
//! already check their budgets (clearance and interference pairs, contact sweeps, solver
//! processes), and stop with [`error`]. A command nobody watches is never cancelled.

/// The work on this thread was cancelled.
pub fn requested() -> bool {
    polycore::watch::current().is_some_and(|w| w.cancelled())
}

/// The error a command stops with once cancelled.
pub fn error(cmd: &str) -> crate::EngineError {
    crate::EngineError::Other(format!("{cmd}: cancelled"))
}

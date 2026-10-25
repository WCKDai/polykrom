//! Set up: install the ACP adapter of an agent whose own command line is already here (Claude
//! Code, Codex), from the agent list, so nobody needs a terminal for it. Only the user's click (or
//! the control channel's `setup`) starts it; npm runs on a worker thread, and discovery looks
//! again when it ends so the agent can be used at once.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use super::discover;
use crate::SolveApp;

/// The longest an install may run. npm has its own network time limits; this only keeps a stuck
/// install from showing as running forever.
const LIMIT: Duration = Duration::from_secs(600);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Running,
    Done,
    /// The last lines of npm's error output, or why it couldn't start.
    Failed(String),
}

impl Progress {
    /// The id on the control channel.
    pub fn id(&self) -> &'static str {
        match self {
            Progress::Running => "running",
            Progress::Done => "done",
            Progress::Failed(_) => "failed",
        }
    }
}

/// What a failed install's output says went wrong, for a line in plain words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// npm can't write to its global folder (a system-owned prefix such as /usr/local).
    Permission,
    /// The package registry can't be reached.
    Network,
    /// The install ran past [`LIMIT`] and was stopped.
    TimedOut,
    Other,
}

/// The cause of a failure, from npm's error codes in its output.
pub fn cause(why: &str) -> Cause {
    let has = |codes: &[&str]| codes.iter().any(|c| why.contains(c));
    if why.starts_with(discover::STOPPED) {
        Cause::TimedOut
    } else if has(&["EACCES", "EPERM"]) {
        Cause::Permission
    } else if has(&["ENOTFOUND", "EAI_AGAIN", "ETIMEDOUT", "ECONNREFUSED", "ECONNRESET", "ENETUNREACH"]) {
        Cause::Network
    } else {
        Cause::Other
    }
}

/// The last Set up: which agent, and how it went.
pub struct Job {
    pub id: &'static str,
    pub name: &'static str,
    pub progress: Progress,
    /// The worker's outcome, until it arrives.
    rx: Option<Receiver<Result<(), String>>>,
}

/// Whether a Set up is running (one at a time: two global npm installs can trip over each other).
pub fn running(app: &SolveApp) -> bool {
    app.agent_chat.setup.as_ref().is_some_and(|j| j.progress == Progress::Running)
}

/// Install the adapter of `id`, an agent discovery listed as ready to set up, in the background.
/// An error once the agent is known is also kept as its outcome, so the list shows it.
pub fn start(app: &mut SolveApp, ctx: &egui::Context, id: &str) -> Result<(), String> {
    if running(app) {
        return Err("a set up is already running".into());
    }
    let d = app.agent_chat.detection.as_ref().ok_or("agents haven't been looked for yet")?;
    let s = d.setup.iter().find(|s| s.id == id).ok_or_else(|| format!("`{id}` is not ready to set up"))?;
    let (id, name, package) = (s.id, s.name, s.package);
    let started = match d.npm.clone() {
        None => Err(format!("npm was not found: install Node.js, or run `{}` in a terminal", s.install)),
        Some(npm) => {
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = ctx.clone();
            std::thread::Builder::new()
                .name("agent-setup".into())
                .spawn(move || {
                    let _ = tx.send(discover::install_bridge(&npm, package, LIMIT));
                    ctx.request_repaint();
                })
                .map(|_| rx)
                .map_err(|e| e.to_string())
        }
    };
    let (progress, rx, result) = match started {
        Ok(rx) => (Progress::Running, Some(rx), Ok(())),
        Err(e) => (Progress::Failed(e.clone()), None, Err(e)),
    };
    app.agent_chat.setup = Some(Job { id, name, progress, rx });
    result
}

/// Take a finished install's outcome, then look for agents again from scratch: a search started
/// during the install may have missed the new adapter. A set-up agent is picked for the next chat
/// unless a chat is running.
pub fn poll(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(job) = app.agent_chat.setup.as_mut() else { return };
    let Some(rx) = &job.rx else { return };
    let outcome = match rx.try_recv() {
        Ok(outcome) => outcome,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err("stopped".into()),
    };
    job.rx = None;
    job.progress = match outcome {
        Ok(()) => Progress::Done,
        Err(e) => Progress::Failed(e),
    };
    let done = (job.progress == Progress::Done).then_some(job.id);
    app.agent_chat.detecting = None;
    super::refresh(app, ctx);
    if let Some(id) = done
        && !app.agent_chat.state().running()
    {
        app.agent_chat.agent_id = Some(id.to_string());
    }
}

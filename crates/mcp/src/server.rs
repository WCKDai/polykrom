//! JSON-RPC 2.0 framing and the MCP lifecycle / tools / resources methods.

use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Longest accepted request line, shared with the control server.
use polykrom_engine::MAX_LINE;
use serde_json::{Value, json};

use polykrom_engine::io::vfs::{self, PathScope};

use polykrom_engine::doc::progress::{self, Progress};

use crate::backend::Backend;
use crate::call::{self, InFlight};
use crate::tools::{call_tool, check_arguments, text_result, tool_definitions};

/// The newest protocol revision we speak; offered when the client asks for one we don't know.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SUPPORTED_VERSIONS: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const RESOURCE_NOT_FOUND: i64 = -32002;

/// Most messages in one JSON-RPC batch.
const MAX_BATCH: usize = 1000;

const INSTRUCTIONS: &str = "Polykrom is an independent parametric 3D CAD app. Units are millimetres \
and degrees in expressions (internally radians), Z is up. Model like this: `sketch.create {plane: XY}`, draw \
(`sketch.rectangle.two_point {p0, p1}`, `sketch.circle.center {center, radius}`, `sketch.line {points, closed}`), constrain \
and dimension (`sketch.dimension {entities, value}`; values may name parameters), `sketch.finish`, then features \
(`solid.extrude {distance, operation?: new|join|cut|intersect}`, `solid.revolve`, `solid.fillet {edges: [[x,y,z] point on \
edge], radius}`, `solid.chamfer`, patterns, mirror, primitives). Use `batch` for several steps at once, \
`list_commands` for ids and parameter docs, `describe_command` for a command's schemas, `inspect_design` / `measure` to verify, `body_topology` to find edge points, \
`set_parameter` to drive the design and `screenshot` to look at it. While a call runs, the `pending` and `running` tools (and a protocol ping) answer at once \
and say how far it is; to stop it, send `notifications/cancelled` with its request id: the design goes back to where it was before \
the call. A call whose error starts `still running:` was not cancelled: don't send it again; `pending` waits for its result. \
Assemblies: make each part a component (`component.create {name}`, model inside it, then `component.activate {component: root}`), \
ground one (`occurrence.ground {occurrence, grounded: true}`) and join the others with `joint.create {type, a, b, limits?}`: each origin is \
an occurrence plus a point or circle on it in world coordinates, and a joint moves about or along its origin's z (a slider's origin z \
points along its travel). Give bodies real materials (`material.list`, `material.assign {bodies, material}`) so mass and stress are right. \
Check fits with `inspect.clearance` and `inspect.interference` (a threaded shank in a hole tapped at the drill size is listed under \
threaded, not as a clash: put a cosmetic thread on it with `solid.thread`, library screws have one), find how far a joint can move with `joint.sweep {joint, from, to}`, \
run mechanisms with `motion.study {name, steps, keys}`, check them with `motion.interference` and record them with \
`motion.export_animation {study, path}`; check strength with `analysis.static_stress`, and that a robot stands and walks with `sim.mujoco {study?}`. Once it works, polish \
the finish in passes with `polish {apply: safe, views: true}` (`inspect.polish` measures sharp edges, radius family, boxiness, \
printability and tidiness; `polish.apply` applies its safe fillets and chamfers) until the score stops rising. Before modelling, state the numbers the result \
must meet (sizes, volume, mass, clearances, travel); before saying it is done, run `quality` (inspect.quality) and check each with `measure` or the inspect commands. A quality pass is structural evidence, not a proof of tolerances or safety. \
On large designs keep replies small with a select list on `execute` and on each `batch` step (dotted paths, * for list items: \
[\"total.mass_g\", \"bodies.*.name\"]), check whole assemblies with `inspect.interference {method: mesh}` (seconds) before exact \
checks of the pairs that matter, and give slow checks a budget_s.";

pub struct Server {
    backend: Box<dyn Backend>,
    initialized: bool,
    scope: Option<PathScope>,
    /// Tests: every call's evaluation waits before this feature until the call is cancelled.
    #[cfg(test)]
    pub(crate) hold_at: Option<usize>,
    /// Tests: told when a call starts running.
    #[cfg(test)]
    pub(crate) started: Option<mpsc::Sender<Value>>,
}

/// A line read from the client, for the thread that runs requests.
enum Incoming {
    Line(String),
    Oversized,
    NotUtf8,
}

/// What the reader and the thread running calls share (#264).
#[derive(Default)]
struct Shared {
    /// The `tools/call` being run.
    current: Mutex<Option<Arc<InFlight>>>,
    /// Requests cancelled before they ran (their ids as JSON text): they never run.
    cancelled: Mutex<HashSet<String>>,
}

/// Most remembered cancellations of requests that haven't run (a client may cancel ones that
/// already finished, which never come).
const MAX_CANCELLED: usize = 1024;

/// How often a running call reports progress, when the client asked for it with a
/// `progressToken`. Each notification is also a sign of life to a client waiting on the call.
const PROGRESS_EVERY: Duration = Duration::from_secs(2);

/// Largest line the reader looks into itself: the messages it answers are small.
const PEEK_LINE: usize = 4096;

fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}
fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

impl Server {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend,
            initialized: false,
            scope: None,
            #[cfg(test)]
            hold_at: None,
            #[cfg(test)]
            started: None,
        }
    }
    /// Confine the files tool calls may reach to `scope` (`None`: anywhere). `polykrom-cli mcp`
    /// always sets one unless started with `--allow-any-path` (docs/mcp.md).
    pub fn with_path_scope(mut self, scope: Option<PathScope>) -> Self {
        self.scope = scope;
        self
    }
    pub fn backend(&mut self) -> &mut dyn Backend {
        self.backend.as_mut()
    }
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Serve newline-delimited JSON-RPC until `input` closes. Only protocol messages go to
    /// `output`; diagnostics belong on stderr.
    ///
    /// Requests are read on a thread of their own and run here, one at a time. While a call runs,
    /// the reader answers `ping`, `pending` and `running` at once and passes
    /// `notifications/cancelled` to the call, which stops at its next check and is taken back
    /// (#264).
    pub fn serve(&mut self, input: impl BufRead + Send, output: impl Write + Send) -> std::io::Result<()> {
        let output = Mutex::new(output);
        let shared = Shared::default();
        let finished = AtomicBool::new(false);
        let (tx, rx) = mpsc::channel::<Incoming>();
        let (out, sh, fin) = (&output, &shared, &finished);
        std::thread::scope(|scope| {
            let reader = scope.spawn(move || read_requests(input, tx, sh, out));
            let ticker = scope.spawn(move || report_progress(sh, out, fin));
            let ran = self.run_requests(rx, sh, out);
            finished.store(true, Ordering::Relaxed);
            ticker.thread().unpark();
            let read = reader.join().unwrap_or_else(|_| Err(std::io::Error::other("the request reader stopped")));
            let _ = ticker.join();
            ran.and(read)
        })
    }

    /// Run what the reader passes on, in order, until it stops.
    fn run_requests(&mut self, rx: mpsc::Receiver<Incoming>, shared: &Shared, output: &Mutex<impl Write>) -> std::io::Result<()> {
        for incoming in rx {
            let reply = match incoming {
                Incoming::Oversized => Some(error(Value::Null, INVALID_REQUEST, "request too large").to_string()),
                Incoming::NotUtf8 => Some(error(Value::Null, PARSE_ERROR, "parse error: input is not UTF-8").to_string()),
                Incoming::Line(line) => self.run_line(&line, shared),
            };
            if let Some(reply) = reply {
                send(output, &reply)?;
            }
        }
        Ok(())
    }

    /// One request. A `tools/call` runs watched: the reader can see how far it is and cancel
    /// it; a cancelled call is taken back and gets no reply, as MCP asks.
    fn run_line(&mut self, line: &str, shared: &Shared) -> Option<String> {
        let call = serde_json::from_str::<Value>(line)
            .ok()
            .filter(|m| m.get("method").and_then(Value::as_str) == Some("tools/call") && m.get("id").is_some_and(|id| !id.is_null()));
        let Some(msg) = call else { return self.handle_line(line) };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        if shared.cancelled.lock().is_ok_and(|mut c| c.remove(&id.to_string())) {
            return None;
        }
        let params = msg.get("params");
        let progress = Arc::new(Progress::default());
        #[cfg(test)]
        if let Some(index) = self.hold_at {
            progress.hold_at(index);
        }
        let flight = Arc::new(InFlight {
            id,
            tool: params.and_then(|p| p.get("name")).and_then(Value::as_str).unwrap_or_default().to_string(),
            started: Instant::now(),
            progress: progress.clone(),
            progress_token: params.and_then(|p| p.pointer("/_meta/progressToken")).filter(|t| !t.is_null()).cloned(),
            canceller: self.backend.canceller(),
        });
        // Only this side's own session can be taken back. A running app stops and takes back a
        // cancelled request itself (control.cancel), and its reply is sent as usual.
        let point = self.backend.save_point();
        #[cfg(test)]
        let started_id = flight.id.clone();
        if let Ok(mut c) = shared.current.lock() {
            *c = Some(flight);
        }
        #[cfg(test)]
        if let Some(tx) = &self.started {
            let _ = tx.send(started_id);
        }
        let reply = call::running(progress.clone(), || progress::watch(progress.clone(), || self.handle(msg)));
        if let Ok(mut c) = shared.current.lock() {
            *c = None;
        }
        if progress.is_cancelled()
            && let Some(point) = point
        {
            self.backend.return_to(point);
            return None;
        }
        reply.map(|r| r.to_string())
    }

    /// One line in, at most one line out (notifications get no reply).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        // Check the raw input before trimming: a huge whitespace prefix is still a huge line.
        if line.len() > MAX_LINE {
            return Some(error(Value::Null, INVALID_REQUEST, "request too large").to_string());
        }
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) if batch.is_empty() => Some(error(Value::Null, INVALID_REQUEST, "empty batch")),
            Ok(Value::Array(batch)) if batch.len() > MAX_BATCH => Some(error(Value::Null, INVALID_REQUEST, "batch too large")),
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.into_iter().filter_map(|m| self.handle(m)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(msg) => self.handle(msg),
            Err(e) => Some(error(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
        };
        reply.map(|r| r.to_string())
    }

    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let Value::Object(o) = &msg else { return Some(error(Value::Null, INVALID_REQUEST, "message must be an object")) };
        let id = o.get("id").cloned();
        let Some(method) = o.get("method").and_then(Value::as_str) else {
            // A response to a request of ours (we send none) or junk.
            if o.contains_key("result") || o.contains_key("error") {
                return None;
            }
            return Some(error(id.unwrap_or(Value::Null), INVALID_REQUEST, "missing `method`"));
        };
        let params = o.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        };
        Some(match self.request(method, &params) {
            Ok(r) => response(id, r),
            Err((code, m)) => error(id, code, m),
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
                let version = if SUPPORTED_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSION };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}, "resources": {"listChanged": false}},
                    "serverInfo": {"name": "solidcraft", "title": "Polykrom", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": format!("{INSTRUCTIONS} Backend: {}.", self.backend.describe()),
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing tool `name`".to_string()))?;
                let args = match params.get("arguments") {
                    None | Some(Value::Null) => json!({}),
                    Some(a) => a.clone(),
                };
                check_arguments(name, &args).map_err(|e| (INVALID_PARAMS, e))?;
                // The open design's folder counts as it is now (a save-as or open moves it).
                let scope = self.scope.clone().map(|s| s.with_design(self.backend.design_path().as_deref()));
                let backend = self.backend.as_mut();
                Ok(vfs::scoped(scope, || call_tool(backend, name, &args)))
            }
            "resources/list" => Ok(json!({"resources": [
                {"uri": "solidcraft://design", "name": "Active design", "description": "document.inspect with measurements", "mimeType": "application/json"},
                {"uri": "solidcraft://commands", "name": "Command catalog", "description": "every command with its parameter docs", "mimeType": "application/json"},
            ]})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                let v = match uri {
                    // The solvecraft:// URIs are what earlier releases listed (compatibility shim,
                    // docs/compatibility.md); clients that saved them still read the resource.
                    "solidcraft://design" | "solvecraft://design" => self.backend.call("document.inspect", json!({"measure": true})),
                    "solidcraft://commands" | "solvecraft://commands" => self.backend.call("engine.commands", json!({})),
                    _ => return Err((RESOURCE_NOT_FOUND, format!("unknown resource `{uri}`"))),
                }
                .map_err(|e| (INTERNAL_ERROR, e))?;
                Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": v.to_string()}]}))
            }
            _ => Err((METHOD_NOT_FOUND, format!("method `{method}` not found"))),
        }
    }
}

/// Write one message line.
fn send(output: &Mutex<impl Write>, line: &str) -> std::io::Result<()> {
    let mut o = output.lock().map_err(|_| std::io::Error::other("the output is unusable after a panic"))?;
    o.write_all(line.as_bytes())?;
    o.write_all(b"\n")?;
    o.flush()
}

/// Read newline-delimited messages until `input` closes, in bounded space: an oversized line is
/// consumed and reported, so an agent cannot exhaust memory with a request that has no newline.
/// The messages that must not wait behind a running call are answered here; the rest go to the
/// thread that runs requests.
fn read_requests(mut input: impl BufRead, tx: mpsc::Sender<Incoming>, shared: &Shared, output: &Mutex<impl Write>) -> std::io::Result<()> {
    loop {
        let mut line = Vec::new();
        let mut oversized = false;
        let mut received = false;
        loop {
            let chunk = input.fill_buf()?;
            if chunk.is_empty() {
                break;
            }
            let count = chunk.iter().position(|&byte| byte == b'\n').map_or(chunk.len(), |i| i + 1);
            let newline = chunk[count - 1] == b'\n';
            let content_len = count - usize::from(newline);
            if !oversized {
                if line.len().saturating_add(content_len) > MAX_LINE {
                    oversized = true;
                    line.clear();
                } else {
                    line.extend_from_slice(&chunk[..content_len]);
                }
            }
            input.consume(count);
            received = true;
            if newline {
                break;
            }
        }
        if !received {
            return Ok(());
        }
        let incoming = if oversized {
            Incoming::Oversized
        } else {
            match String::from_utf8(line) {
                Ok(line) => {
                    if let Some(answer) =
                        (line.len() <= PEEK_LINE).then(|| serde_json::from_str::<Value>(&line).ok()).flatten().and_then(|m| answer_now(&m, shared))
                    {
                        if let Some(reply) = answer {
                            send(output, &reply.to_string())?;
                        }
                        continue;
                    }
                    Incoming::Line(line)
                }
                // Not UTF-8: answered with a parse error, and serving goes on.
                Err(_) => Incoming::NotUtf8,
            }
        };
        if tx.send(incoming).is_err() {
            return Ok(());
        }
    }
}

/// The reader's own answer to `msg`, if it doesn't wait for the running call: `Some(None)` for a
/// notification it handled, `Some(Some(reply))` for a request it answered, `None` to pass on.
fn answer_now(msg: &Value, shared: &Shared) -> Option<Option<Value>> {
    let o = msg.as_object()?;
    let method = o.get("method")?.as_str()?;
    let current = shared.current.lock().ok().and_then(|c| c.clone());
    let Some(id) = o.get("id").filter(|id| !id.is_null()).cloned() else {
        if method != "notifications/cancelled" {
            return None;
        }
        // Cancel the running call, or one still waiting to run.
        let Some(target) = msg.pointer("/params/requestId") else { return Some(None) };
        match current {
            Some(c) if c.id == *target => {
                c.progress.cancel();
                // Where the call runs in an app, it is cancelled there too, without holding up
                // the reader.
                if let Some(cancel) = c.canceller.clone() {
                    std::thread::spawn(move || cancel());
                }
            }
            _ => {
                if let Ok(mut cancelled) = shared.cancelled.lock() {
                    if cancelled.len() >= MAX_CANCELLED {
                        cancelled.clear();
                    }
                    cancelled.insert(target.to_string());
                }
            }
        }
        return Some(None);
    };
    // Nothing running: everything goes the usual way.
    let current = current?;
    match method {
        "ping" => Some(Some(response(id, json!({})))),
        "tools/call" => match msg.pointer("/params/name").and_then(Value::as_str) {
            Some("pending" | "running") => Some(Some(response(id, text_result(&current.status(), false)))),
            _ => None,
        },
        _ => None,
    }
}

/// While a call runs whose client sent a `progressToken`, say how far it is now and then.
fn report_progress(shared: &Shared, output: &Mutex<impl Write>, finished: &AtomicBool) {
    let mut sent: u64 = 0;
    let mut last_call: Option<String> = None;
    while !finished.load(Ordering::Relaxed) {
        std::thread::park_timeout(PROGRESS_EVERY);
        if finished.load(Ordering::Relaxed) {
            return;
        }
        let Some(c) = shared.current.lock().ok().and_then(|c| c.clone()) else { continue };
        let Some(token) = c.progress_token.clone() else { continue };
        // `progress` has to grow with each notification for one token: count them.
        if last_call.as_deref() != Some(&c.id.to_string()) {
            last_call = Some(c.id.to_string());
            sent = 0;
        }
        if c.started.elapsed() < PROGRESS_EVERY {
            continue;
        }
        sent += 1;
        let (done, total) = c.progress.features();
        let mut message = format!("{} running for {:.0} s", c.tool, c.started.elapsed().as_secs_f64());
        if total > 0 {
            message.push_str(&format!(", {done} of {total} features evaluated"));
        }
        if let Some((stage, d, t)) = c.progress.work() {
            message.push_str(&format!(", {stage} {d} of {t}"));
        }
        let note =
            json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {"progressToken": token, "progress": sent, "message": message}});
        if send(output, &note.to_string()).is_err() {
            return;
        }
    }
}

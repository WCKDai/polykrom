//! The AI review flow (design kit views 04 and 05): a prompt row under the ribbon, a Review
//! changes panel while a proposal is open, the proposed geometry shown in the viewport, and after
//! Apply a receipt toast with Undo. Every action is a command: `engine.propose` makes the proposal
//! (a scratch-copy preview that leaves the design alone), `engine.apply_proposal` applies it as one
//! undo step, `engine.drop_proposal` is Cancel command, and the toast's Undo is `edit.undo` of
//! exactly that step. The agent chat's design edits come through here too while "review before
//! apply" is on (the default), and always for a request from the prompt row: see [`divert`].
//!
//! Every value the panel shows comes from the proposal's report or from the model: a changed
//! feature's inputs are read from the design before and after the commands (each in its own
//! kind: lengths in the design's units, angles in degrees, counts bare), the instances and pitch
//! of a hole pattern from the solved sketch its holes sit on, the dependents from the report.

use egui::{Align2, Id, Rect, pos2, vec2};
use polykrom_engine::doc::{Document, FeatureKind, ModelState, PatternKind, expr::Kind};
use serde_json::{Value, json};

use crate::i18n::{format as trf, tr, tr_label};
use crate::theme::Tokens;
use crate::ui_kit::{self, Container, Edge, Surface};
use crate::{ControlRequest, SolveApp};

/// The panel's width (logical points), the inspector's: it stands where Properties stand.
pub const PANEL_WIDTH: f32 = 268.0;
/// The prompt row's height under the ribbon.
const PROMPT_ROW_HEIGHT: f32 = 36.0;
/// The least room (points) below the ribbon the row leaves to the modelling controls: in a window
/// shorter than this the sketch palette and dialogs come first, and the row waits for more room.
const PROMPT_MIN_ROOM: f32 = 420.0;
/// How long the receipt toast stays after Apply while nothing else changes the design (ms).
const RECEIPT_MS: f64 = 30_000.0;
/// The most names listed on one line of the panel.
const MAX_NAMES: usize = 6;
/// The most characters a prompt takes (the agent chat's own cap is larger).
const MAX_PROMPT: usize = 4000;
/// The most value rows the panel lists (a proposal that touches more says so in the scope lines).
const MAX_ROWS: usize = 40;
/// The widest the blocked reason gets in the prompt row before it truncates (its hover text and
/// the status line carry it whole).
const MAX_REASON_WIDTH: f32 = 320.0;
/// The widest the scope chip gets before its text truncates.
const MAX_SCOPE_WIDTH: f32 = 260.0;
/// The key that focuses the prompt row (shown beside the scope chip as the kit does).
const FOCUS_KEY: &str = "Ctrl+J";
/// Two sketch pitches within this are the same (mm).
const PITCH_TOL: f64 = 1e-6;

/// Who asked for the proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The prompt row (through the agent chat).
    Prompt,
    /// The agent chat's own design edit, diverted here.
    Agent,
    /// The control channel (`ui.review`), an MCP client, a test.
    Control,
}

/// What the prompt row's button says and can do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptState {
    /// "Preview changes": a request can go.
    Idle,
    /// "Preview pending": the agent is working on the request.
    Pending,
    /// "Preview ready": a proposal is in the panel.
    Ready,
    /// "Preview changes" greyed, with the reason the agent cannot act now.
    Blocked(String),
}

/// What the request is about. `Selection` follows the selection; the others are the chip's
/// own choice, which lasts until the proposal is applied or cancelled.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Selection,
    Design,
    Feature(u64),
    Body(String),
}

/// A value of the model the panel shows: a parameter of a changed feature or a design parameter,
/// as `before → after` when the proposal changes it, as its value alone when it keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The feature (or the parameters table) the value belongs to.
    pub feature: String,
    /// The engine's label of the input ("Diameter"), translated by the panel.
    pub label: String,
    pub before: Option<String>,
    pub after: String,
}

impl Row {
    /// "before → after", or the value alone.
    pub fn text(&self) -> String {
        match &self.before {
            Some(b) => format!("{b} → {}", self.after),
            None => self.after.clone(),
        }
    }
}

/// What the proposal does to the model, read from the design before and after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Described {
    /// The values the proposal changes.
    pub changes: Vec<Row>,
    /// Values of the changed features (and of the patterns around them) the proposal keeps.
    pub kept: Vec<Row>,
    /// The instances of the changed feature: a hole's points, a pattern's copies.
    pub instances: Option<usize>,
    /// A pattern is involved (a hole at several points, a pattern feature): whether its count
    /// and pitch are the same after.
    pub pattern_preserved: Option<bool>,
}

/// A proposal under review: what `engine.propose` was given and answered.
#[derive(Clone, Debug)]
pub struct Open {
    pub id: String,
    pub label: String,
    /// The design revision it was made on; another revision means it is stale.
    pub revision: u64,
    pub commands: Vec<(String, Value)>,
    /// The reply of `engine.propose`: bodies, totals, timeline, parameters, warnings, errors.
    pub report: Value,
    pub source: Source,
    /// The prompt row's request this answers, if it came from there.
    pub request: Option<String>,
    pub described: Described,
}

/// The receipt after Apply: the toast's text, and the design revision it left, so Undo undoes
/// that step only (any edit, undo or redo since moves the revision on).
#[derive(Clone, Debug)]
pub struct Receipt {
    pub what: String,
    pub value: String,
    /// "Pattern preserved" is true of the applied change.
    pub pattern_preserved: bool,
    pub revision: u64,
    pub at: f64,
}

#[derive(Default)]
pub struct Review {
    /// The prompt row's text.
    pub prompt: String,
    pub scope: Scope,
    pub open: Option<Open>,
    pub receipt: Option<Receipt>,
    /// The prompt sent to the agent, until its proposal arrives (the button reads "Preview
    /// pending") or its message ends without one.
    pub waiting: Option<String>,
    /// The prompt row's field takes the focus on the next frame (Ctrl+J, `ui.review {focus}`).
    pub focus: bool,
    /// The panel's rect this frame (it covers the viewport for input).
    pub rect: Option<Rect>,
    /// The prompt row's rect this frame (a card over the viewport in the floating layout).
    pub prompt_rect: Option<Rect>,
}

/// The colours of the proposed geometry: the after-state in place of the bodies it changes, its
/// changed faces lit the way a selected face is, removed material as the usual cut ghost.
pub(crate) fn preview_colors(t: &Tokens) -> crate::preview::Colors {
    let c = |c: egui::Color32| c.to_srgba_unmultiplied();
    crate::preview::Colors { body: c(t.body), added: c(t.sel_face), new_body: c(t.sel_face), removed: c(t.preview_cut), edge: c(t.body_edge) }
}

/// The commands the viewport previews while a proposal is open and no dialog has a preview of its
/// own; none once the design moved on (the proposal is stale then, and dropped on the next frame).
pub(crate) fn preview_commands(app: &SolveApp) -> Option<Vec<(String, Value)>> {
    let open = app.review.open.as_ref()?;
    (app.dialog.is_none() && open.revision == app.session.revision).then(|| open.commands.clone())
}

/// A command the agent may run that a proposal can hold: a design edit (the rule `engine.propose`
/// applies), so reads, views and file commands keep running live.
fn proposable(id: &str, params: &Value) -> bool {
    let Some(spec) = polykrom_engine::find_command(id) else { return false };
    let outside = ["engine.", "file.", "doc.", "render.", "diagnostics."].iter().any(|p| spec.id.starts_with(p));
    spec.undoable
        && !outside
        && polykrom_engine::control_pending::command_effect(spec.id, params) == polykrom_engine::control_pending::Effect::Changes
}

/// Propose `commands` and open the panel on the result. A proposal already open is replaced
/// (dropped once the new one is held); on an error nothing changes and the old one stays.
pub fn propose(app: &mut SolveApp, commands: Vec<(String, Value)>, label: Option<&str>, source: Source) -> Result<Value, String> {
    let steps: Vec<Value> = commands.iter().map(|(c, p)| json!({"command": c, "params": p})).collect();
    let mut params = json!({"commands": steps});
    if let Some(l) = label.map(str::trim).filter(|l| !l.is_empty()) {
        params["label"] = json!(l);
    }
    let report = app.run("engine.propose", params)?;
    if let Some(previous) = app.review.open.take() {
        let _ = app.run("engine.drop_proposal", json!({"proposal": previous.id}));
    }
    let described = describe(app, &commands, &report);
    let request = if source == Source::Prompt { app.review.waiting.take() } else { None };
    let open = Open {
        id: report["proposal"].as_str().unwrap_or_default().to_string(),
        label: report["label"].as_str().unwrap_or_default().to_string(),
        revision: report["revision"].as_u64().unwrap_or(app.session.revision),
        commands,
        report: report.clone(),
        source,
        request,
        described,
    };
    let status = trf("review.status_open", &[("feature", &target(&open).0)]);
    app.review.open = Some(open);
    app.review.waiting = None;
    app.review.receipt = None;
    log(app, &status);
    Ok(report)
}

/// Apply & rebuild: the proposal's commands run as one undo step; the panel gives way to the
/// receipt toast. A refusal (the design changed since) closes the panel and says so.
pub fn apply(app: &mut SolveApp) -> Result<Value, String> {
    let Some(open) = app.review.open.take() else { return Err("no proposal is open".into()) };
    match app.run("engine.apply_proposal", json!({"proposal": open.id})) {
        Ok(r) => {
            let (what, value) = receipt_text(app, &open);
            let pattern_preserved = open.described.pattern_preserved == Some(true);
            app.review.receipt = Some(Receipt { what, value, pattern_preserved, revision: app.session.revision, at: crate::now_ms() });
            // The request is done: the row is ready for the next one (view 05).
            if open.request.is_some() {
                app.review.prompt.clear();
            }
            app.review.scope = Scope::Selection;
            let line = status_line(app).unwrap_or_default();
            log(app, &line);
            log(app, tr("review.status_applied"));
            Ok(r)
        }
        Err(e) => {
            let why = refusal_message(&e);
            app.set_status(trf("review.apply_failed", &[("why", &why)]), true);
            Err(why)
        }
    }
}

/// Cancel command: the proposal is dropped; the design is as it was. `false`: nothing was open.
pub fn cancel(app: &mut SolveApp) -> bool {
    let Some(open) = app.review.open.take() else { return false };
    let _ = app.run("engine.drop_proposal", json!({"proposal": open.id}));
    app.review.scope = Scope::Selection;
    app.set_status(tr("review.status_cancelled"), false);
    true
}

/// The receipt's Undo: undoes the applied proposal, and only while it is still the last thing done.
pub fn undo_receipt(app: &mut SolveApp) -> Result<(), String> {
    let Some(receipt) = app.review.receipt.take() else { return Err("nothing to undo".into()) };
    if receipt.revision != app.session.revision {
        let why = tr("review.undo_too_late").to_string();
        app.set_status(why.clone(), true);
        return Err(why);
    }
    app.run("edit.undo", json!({})).map(|_| ())
}

/// The agent's message ended (answered, stopped or gone): a request still waiting for a proposal
/// gets none, so the row is free again and the status says where the answer is.
pub(crate) fn turn_ended(app: &mut SolveApp) {
    if app.review.waiting.take().is_some() && app.review.open.is_none() {
        app.set_status(tr("review.status_no_proposal"), false);
    }
}

/// A status the viewport's status line already shows: into the Messages log, without a toast.
fn log(app: &mut SolveApp, text: &str) {
    app.messages.push(text, false, crate::now_ms());
}

/// A refusal's message (`engine.apply_proposal` answers a JSON object), else the error as it is.
fn refusal_message(e: &str) -> String {
    serde_json::from_str::<Value>(e).ok().and_then(|v| v["message"].as_str().map(str::to_owned)).unwrap_or_else(|| e.to_string())
}

/// Each frame: a proposal the design has moved past is stale; it is dropped and the panel says so.
fn drop_if_stale(app: &mut SolveApp) {
    if app.review.open.as_ref().is_some_and(|o| o.revision != app.session.revision) {
        if let Some(open) = app.review.open.take() {
            let _ = app.run("engine.drop_proposal", json!({"proposal": open.id}));
        }
        app.set_status(tr("review.status_dropped"), false);
    }
    if app.review.receipt.as_ref().is_some_and(|r| r.revision != app.session.revision || crate::now_ms() - r.at > RECEIPT_MS) {
        app.review.receipt = None;
    }
}

// ---- What the model says, in values --------------------------------------------------------------

/// The design and model after `commands`, run on a scratch copy (what `engine.propose` held;
/// the evaluation is shared with it through the session's cache). `None` when a step fails:
/// the panel then shows the report alone.
fn after(app: &SolveApp, commands: &[(String, Value)]) -> Option<(std::sync::Arc<Document>, std::sync::Arc<ModelState>)> {
    let mut s = app.session.scratch();
    for (c, p) in commands {
        s.execute(c, p).ok()?;
    }
    let state = s.model.state();
    Some((s.doc.clone(), state))
}

/// How many the feature makes: a hole at sketch points (one each), a pattern (its copies).
fn instances(doc: &Document, kind: &FeatureKind) -> Option<usize> {
    match kind {
        FeatureKind::Hole { points: Some(sp), .. } => Some(sp.ids.len()),
        FeatureKind::Pattern { pattern, .. } => {
            let copies = |e: &polykrom_engine::doc::Expr| {
                doc.eval(e.as_str(), Kind::Unitless).ok().filter(|v| v.is_finite()).map(|v| v.round().max(0.0) as usize)
            };
            match pattern {
                PatternKind::Rectangular { count1, count2, .. } => Some(copies(count1)? * count2.as_ref().map_or(Some(1), copies)?),
                PatternKind::Circular { count, .. } => copies(count),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether the feature is a hole at several sketch points (what the kit calls "4 holes").
fn is_holes(kind: &FeatureKind) -> bool {
    matches!(kind, FeatureKind::Hole { points: Some(sp), .. } if sp.ids.len() > 1)
}

/// The pitch of the points a hole feature sits on, along the sketch's X and Y: the spacing of
/// their distinct coordinates when it is even (a grid, a 2 × 2 pattern), from the solved sketch.
fn pitch(state: &ModelState, kind: &FeatureKind) -> Option<[Option<f64>; 2]> {
    let FeatureKind::Hole { points: Some(sp), .. } = kind else { return None };
    let ss = state.sketch(sp.sketch.0)?;
    let points: Vec<[f64; 2]> = sp.ids.iter().filter_map(|id| ss.sketch.points.iter().find(|p| &p.id == id)).map(|p| [p.pos.x, p.pos.y]).collect();
    if points.len() < 2 {
        return None;
    }
    let even = |axis: usize| {
        let mut values: Vec<f64> = points.iter().map(|p| p[axis]).collect();
        values.sort_by(f64::total_cmp);
        values.dedup_by(|a, b| (*a - *b).abs() <= PITCH_TOL);
        let gaps: Vec<f64> = values.windows(2).map(|w| w[1] - w[0]).collect();
        let first = *gaps.first()?;
        gaps.iter().all(|g| (g - first).abs() <= PITCH_TOL).then_some(first)
    };
    Some([even(0), even(1)])
}

/// A number without trailing zeros ("3", "2.5"), at most `decimals` places.
fn plain(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s }
}

/// An input's value in its own kind: a length in the design's units (a diameter with its Ø),
/// an angle in degrees, a count bare. An expression that does not evaluate is shown as written.
fn value_text(app: &SolveApp, doc: &Document, expr: &str, kind: Kind, label: &str) -> String {
    let value = doc.eval(expr, kind).ok().filter(|v| v.is_finite());
    match (kind, value) {
        (Kind::Length, Some(v)) => {
            let prefix = if label.contains("iameter") { "Ø" } else { "" };
            format!("{prefix}{}", crate::prefs::show_mm(app, v, 1))
        }
        (Kind::Angle, Some(v)) => format!("{} deg", plain(v.to_degrees(), 2)),
        (Kind::Unitless, Some(v)) => plain(v, 3),
        (_, None) => expr.trim().to_string(),
    }
}

/// The rows of a feature's inputs after against before: changed ones as `before → after`, the
/// others as their value. A feature that is new has after values alone.
fn input_rows(app: &SolveApp, before: Option<(&Document, &FeatureKind)>, after: (&Document, &FeatureKind), name: &str) -> (Vec<Row>, Vec<Row>) {
    let (mut changes, mut kept) = (Vec::new(), Vec::new());
    let after_inputs = after.1.inputs();
    let before_inputs = before.map(|(_, k)| k.inputs()).unwrap_or_default();
    for (label, expr, kind) in &after_inputs {
        let was = before_inputs.iter().find(|(l, _, _)| l == label);
        let value = value_text(app, after.0, expr, *kind, label);
        match (before, was) {
            (Some((doc, _)), Some((_, old, old_kind))) if old != expr => {
                let before = value_text(app, doc, old, *old_kind, label);
                changes.push(Row { feature: name.to_owned(), label: (*label).to_owned(), before: Some(before), after: value });
            }
            (Some(_), Some(_)) => kept.push(Row { feature: name.to_owned(), label: (*label).to_owned(), before: None, after: value }),
            _ => changes.push(Row { feature: name.to_owned(), label: (*label).to_owned(), before: None, after: value }),
        }
    }
    // A hole's extent is an input too, in words: through all when it has no depth.
    if let FeatureKind::Hole { depth: None, .. } = after.1 {
        let through = tr("review.through_all").to_string();
        let was_depth = matches!(before, Some((_, FeatureKind::Hole { depth: Some(_), .. })));
        if was_depth {
            let old = before_inputs
                .iter()
                .find(|(l, _, _)| *l == "Depth")
                .map(|(l, e, k)| value_text(app, before.map(|b| b.0).unwrap_or(after.0), e, *k, l));
            changes.push(Row { feature: name.to_owned(), label: "Extent".into(), before: old, after: through });
        } else {
            kept.push(Row { feature: name.to_owned(), label: "Extent".into(), before: None, after: through });
        }
    }
    (changes, kept)
}

/// What the proposal does to the model: the changed features' inputs before and after, what they
/// keep, the pattern around them. From the design before (the session's) and after (a scratch
/// run of the commands); the report names the features.
fn describe(app: &SolveApp, commands: &[(String, Value)], report: &Value) -> Described {
    let mut d = Described::default();
    let Some((after_doc, after_state)) = after(app, commands) else { return d };
    let before_doc = app.session.doc.clone();
    let before_state = app.session.model.state();
    let ids = |list: &str| -> Vec<u64> { report["timeline"][list].as_array().into_iter().flatten().filter_map(|f| f["id"].as_u64()).collect() };
    let mut pattern: Option<bool> = None;
    let mut note = |preserved: bool| pattern = Some(pattern.unwrap_or(true) && preserved);
    let changed = ids("changed");
    // The instances are of the one changed feature; several changed say nothing as one.
    let single = changed.len() == 1;
    for id in changed {
        let (Some(bf), Some(af)) = (before_doc.feature(id), after_doc.feature(id)) else { continue };
        let (changes, kept) = input_rows(app, Some((&before_doc, &bf.kind)), (&after_doc, &af.kind), &af.name);
        d.changes.extend(changes);
        d.kept.extend(kept);
        let (was, now) = (instances(&before_doc, &bf.kind), instances(&after_doc, &af.kind));
        if single {
            d.instances = now;
        }
        if is_holes(&af.kind) || is_holes(&bf.kind) {
            let (p0, p1) = (pitch(&before_state, &bf.kind), pitch(&after_state, &af.kind));
            note(was == now && p0 == p1);
            if let Some([x, y]) = p1 {
                for (label, v) in [("Pitch X", x), ("Pitch Y", y)] {
                    if let Some(v) = v {
                        d.kept.push(Row { feature: af.name.clone(), label: label.into(), before: None, after: crate::prefs::show_mm(app, v, 1) });
                    }
                }
            }
        } else if matches!(af.kind, FeatureKind::Pattern { .. }) {
            note(was == now && bf.kind.inputs() == af.kind.inputs());
        }
    }
    for id in ids("added") {
        let Some(af) = after_doc.feature(id) else { continue };
        let (changes, kept) = input_rows(app, None, (&after_doc, &af.kind), &af.name);
        d.changes.extend(changes);
        d.kept.extend(kept);
    }
    // A pattern the change rebuilds: its count and pitch are what "keep the pattern" means.
    for id in ids("affected") {
        let (Some(bf), Some(af)) = (before_doc.feature(id), after_doc.feature(id)) else { continue };
        if !matches!(af.kind, FeatureKind::Pattern { .. }) {
            continue;
        }
        let (changes, kept) = input_rows(app, Some((&before_doc, &bf.kind)), (&after_doc, &af.kind), &af.name);
        note(changes.is_empty());
        d.changes.extend(changes);
        d.kept.extend(kept);
    }
    d.pattern_preserved = pattern;
    d.changes.truncate(MAX_ROWS);
    d.kept.truncate(MAX_ROWS.saturating_sub(d.changes.len()));
    d
}

// ---- What the report says, in words ------------------------------------------------------------

fn names(list: &Value) -> Vec<String> {
    list.as_array().into_iter().flatten().filter_map(|f| f["name"].as_str().map(str::to_owned)).collect()
}

fn joined(names: &[String]) -> String {
    let mut shown: Vec<&str> = names.iter().take(MAX_NAMES).map(String::as_str).collect();
    let more = names.len().saturating_sub(MAX_NAMES);
    let tail;
    if more > 0 {
        tail = format!("+{more}");
        shown.push(&tail);
    }
    shown.join(", ")
}

/// What the proposal changes directly: the parameters, else the features it adds, changes or
/// removes, else the bodies it changes. Its name (the status line's "feature") and how many.
fn target(open: &Open) -> (String, usize) {
    let r = &open.report;
    let params = names(&r["parameters"]);
    if !params.is_empty() {
        return (joined(&params), params.len());
    }
    let mut features = names(&r["timeline"]["changed"]);
    features.extend(names(&r["timeline"]["added"]));
    features.extend(names(&r["timeline"]["removed"]));
    if !features.is_empty() {
        return (joined(&features), features.len());
    }
    let bodies = names(&r["bodies"]["changed"]);
    if !bodies.is_empty() {
        return (joined(&bodies), bodies.len());
    }
    (title(open), 1)
}

/// The panel's title: the label given, or the command's name when the engine labelled the
/// proposal with its id.
fn title(open: &Open) -> String {
    match polykrom_engine::find_command(&open.label) {
        Some(c) => tr_label(c.label).to_string(),
        None => open.label.clone(),
    }
}

/// The features rebuilt because an input changed (the dependents).
fn dependents(open: &Open) -> Vec<String> {
    names(&open.report["timeline"]["affected"])
}

/// "4 holes" for a changed hole feature at several points, else the feature's name.
fn feature_words(app: &SolveApp, f: &Value) -> String {
    let name = f["name"].as_str().unwrap_or_default().to_owned();
    match f["id"].as_u64().and_then(|id| app.session.doc.feature(id)) {
        Some(feature) if is_holes(&feature.kind) => {
            trf("review.holes", &[("count", &instances(&app.session.doc, &feature.kind).unwrap_or(0).to_string())])
        }
        _ => name,
    }
}

/// "4 mounting holes": the count and the feature's name, the name's first letter lowered when it
/// is a capitalised word ("Mounting holes"), kept when it is a designation ("M6 holes").
fn instances_words(count: usize, name: &str) -> String {
    let mut chars = name.chars();
    let lowered = match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => first.to_lowercase().chain(chars_from(name, 1)).collect(),
        _ => name.to_owned(),
    };
    trf("review.instances", &[("count", &count.to_string()), ("name", &lowered)])
}

fn chars_from(s: &str, n: usize) -> impl Iterator<Item = char> + '_ {
    s.chars().skip(n)
}

/// The receipt's words: "<what> updated / <value>", the subject being the changed feature's
/// instances ("4 mounting holes") when it has them, the values the inputs it changed.
fn receipt_text(app: &SolveApp, open: &Open) -> (String, String) {
    let r = &open.report;
    let (mut what, _) = target(open);
    let d = &open.described;
    if let (Some(n), Some(first)) = (d.instances, d.changes.first())
        && n > 1
        && names(&r["timeline"]["changed"]).len() == 1
    {
        what = instances_words(n, &first.feature);
    }
    if !d.changes.is_empty() {
        let mut values: Vec<&str> = d.changes.iter().map(|c| c.after.as_str()).collect();
        values.dedup();
        return (what, values.join(", "));
    }
    if let Some(p) = r["parameters"].as_array().filter(|a| !a.is_empty()) {
        let values: Vec<String> = p.iter().filter_map(|p| p["after"].as_str().map(str::to_owned)).collect();
        return (what, values.join(", "));
    }
    if let Some(after) = r["totals"]["after"]["volume_mm3"].as_f64() {
        return (what, crate::prefs::show_mm(app, after, 3));
    }
    (what, String::new())
}

/// The status line while a proposal is open or just applied (the kit's bottom status bar):
/// "AI review / <target> / Proposed geometry shown", then "<target> updated / <summary>" with
/// "Pattern preserved" when that is true of the change.
pub(crate) fn status_line(app: &SolveApp) -> Option<String> {
    if let Some(open) = &app.review.open {
        return Some(trf("review.status_open", &[("feature", &target(open).0)]));
    }
    let r = app.review.receipt.as_ref()?;
    let mut line = trf("review.updated", &[("what", &r.what), ("value", &r.value)]);
    if r.pattern_preserved {
        line = format!("{line} / {}", tr("review.pattern_preserved"));
    }
    Some(line)
}

// ---- The scope ----------------------------------------------------------------------------------

/// A feature as the scope chip names it: "Mounting holes / 4 holes", "Vents / 12 instances".
fn feature_scope(doc: &Document, f: &polykrom_engine::doc::Feature) -> String {
    match instances(doc, &f.kind) {
        Some(n) if is_holes(&f.kind) => trf("review.scope_holes", &[("name", &f.name), ("count", &n.to_string())]),
        Some(n) if n > 1 => trf("review.scope_instances", &[("name", &f.name), ("count", &n.to_string())]),
        _ => f.name.clone(),
    }
}

/// What the selection is about: the selected feature, the selected bodies, else the whole design.
fn selection_scope(app: &SolveApp) -> String {
    use polykrom_engine::Sel;
    for s in &app.session.selection {
        if let Sel::Feature { id } = s
            && let Some(f) = app.session.doc.feature(*id)
        {
            return feature_scope(&app.session.doc, f);
        }
    }
    let bodies = crate::inspector::selected_bodies(app);
    match bodies.as_slice() {
        [] => tr("review.scope_all").to_string(),
        [one] => one.clone(),
        many => trf("review.scope_bodies", &[("count", &many.len().to_string())]),
    }
}

/// What the prompt's scope is: the chip's choice, else the selection's.
pub fn scope(app: &SolveApp) -> String {
    let doc = &app.session.doc;
    match &app.review.scope {
        Scope::Design => tr("review.scope_all").to_string(),
        Scope::Feature(id) => doc.feature(*id).map_or_else(|| selection_scope(app), |f| feature_scope(doc, f)),
        Scope::Body(name) if app.session.world_state().body(name).is_some() => name.clone(),
        _ => selection_scope(app),
    }
}

// ---- The prompt row -----------------------------------------------------------------------------

/// The prompt row is part of the AI features: without them (or in the browser) it is not shown.
fn prompt_shown(app: &SolveApp) -> bool {
    crate::agent_chat::shown(app) && !crate::render_workspace::active(app)
}

/// Why the agent cannot take a request now, if it cannot: a sketch is being edited (a proposal
/// is made on the model), no agent is set up, the agent needs to sign in, or it is busy with
/// another message.
pub fn blocked_reason(app: &SolveApp) -> Option<String> {
    use crate::agent_chat::State;
    if app.session.active_sketch.is_some() {
        return Some(tr("review.blocked_sketch").to_string());
    }
    if crate::agent_chat::agents::current(app).is_none() && !agent_running(app) {
        return Some(tr("review.blocked_no_agent").to_string());
    }
    match app.agent_chat.state() {
        State::NeedsAuth => Some(tr("review.blocked_auth").to_string()),
        State::Busy if app.review.waiting.is_none() => Some(tr("review.blocked_busy").to_string()),
        _ => None,
    }
}

/// The prompt row's state: a proposal open outranks a request pending, which outranks a reason
/// the agent cannot act.
pub fn prompt_state(app: &SolveApp) -> PromptState {
    if app.review.open.is_some() {
        return PromptState::Ready;
    }
    if app.review.waiting.is_some() {
        return PromptState::Pending;
    }
    match blocked_reason(app) {
        Some(why) => PromptState::Blocked(why),
        None => PromptState::Idle,
    }
}

/// Send the prompt to the agent; its design edits come back as a proposal ([`divert`]).
pub fn send_prompt(app: &mut SolveApp, ctx: &egui::Context) {
    let text = app.review.prompt.trim().to_string();
    if text.is_empty() {
        return;
    }
    if !crate::agent_chat::shown(app) {
        app.set_status(tr("review.no_agent"), true);
        return;
    }
    if let Some(why) = blocked_reason(app) {
        app.set_status(why, true);
        return;
    }
    let message = format!("{text}\n\n{}", trf("review.scope", &[("scope", &scope(app))]));
    let notices = app.agent_chat.items.len();
    crate::agent_chat::send(app, ctx, message);
    // The message is with an agent that runs (or starts): the request is pending until its
    // proposal arrives or the message ends. A chat that could not take it (no agent, a start that
    // failed) says why in its own transcript: the status line repeats it, as the transcript may
    // be closed.
    if agent_running(app) {
        app.review.waiting = Some(text);
        app.set_status(tr("review.status_sent"), false);
        return;
    }
    let refused = app.agent_chat.items.iter().skip(notices).rev().find_map(|i| match i {
        crate::agent_chat::Item::Notice { text, error: true } => Some(text.clone()),
        _ => None,
    });
    app.review.waiting = None;
    app.set_status(refused.unwrap_or_else(|| tr("review.blocked_no_agent").to_string()), true);
}

/// The agent is starting, ready, working or waiting for a sign-in: a message is or will be with it.
fn agent_running(app: &SolveApp) -> bool {
    use crate::agent_chat::State;
    matches!(app.agent_chat.state(), State::Starting | State::Ready | State::Busy | State::NeedsAuth)
}

/// Each frame: a request waiting on an agent that is no longer running gets no proposal.
fn settle_waiting(app: &mut SolveApp) {
    if app.review.waiting.is_some() && !agent_running(app) {
        turn_ended(app);
    }
}

/// The prompt row under the ribbon (the kit's "Native model command bar"), a top strip in both
/// layouts so the panels and the viewport's controls start below it: docked, a flush bar joining
/// the ribbon; floating, a card under the ribbon cards with the shell gap below it.
pub fn prompt_row(app: &mut SolveApp, ui: &mut egui::Ui) {
    app.review.prompt_rect = None;
    settle_waiting(app);
    if !prompt_shown(app) || ui.available_rect_before_wrap().height() < PROMPT_MIN_ROOM {
        return;
    }
    let t = Tokens::get();
    let container = Container::new(ui.ctx(), Surface::Chrome);
    let floating = container.floating();
    let gap = container.gap();
    let height = if floating { PROMPT_ROW_HEIGHT + gap } else { PROMPT_ROW_HEIGHT };
    let frame = container.edge_frame(if floating { egui::Color32::TRANSPARENT } else { t.toolbar });
    let cards = app.shell.cards.clone();
    let r =
        container.edge("sc_review_prompt", Edge::Top).exact_size(height).resizable(false).show_separator_line(false).frame(frame).show(ui, |ui| {
            let full = frame.outer_rect(ui.max_rect());
            let card = if floating {
                // As wide as the ribbon cards together.
                let (left, right) = match (cards.first(), cards.last()) {
                    (Some(a), Some(b)) => (a.left(), b.right()),
                    _ => (full.left() + gap, full.right() - gap),
                };
                let card = Rect::from_min_size(pos2(left, full.top()), vec2((right - left).max(0.0), PROMPT_ROW_HEIGHT));
                let card_frame = container.frame(ui.ctx(), t.toolbar, crate::frame::Side::Left);
                ui.painter().add(card_frame.paint(card));
                // Blank card space owns clicks, as the ribbon cards do.
                ui.interact(card, ui.id().with("review_prompt_card"), egui::Sense::click_and_drag());
                card
            } else {
                full
            };
            let inner = card.shrink2(vec2(t.space_12, 0.0));
            let mut row = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
            prompt_contents(app, &mut row);
            card
        });
    app.review.prompt_rect = Some(r.inner);
    crate::scenario::publish_rect("review:prompt_row", r.inner);
}

/// The button's label for a state.
fn button_label(state: &PromptState) -> &'static str {
    match state {
        PromptState::Ready => tr("review.preview_ready"),
        PromptState::Pending => tr("review.preview_pending"),
        PromptState::Idle | PromptState::Blocked(_) => tr("review.preview"),
    }
}

fn prompt_contents(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    ui.spacing_mut().item_spacing = vec2(t.space_12, 0.0);
    let (icon, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
    crate::icons::paint_icon(ui.painter(), icon, "assistant", 1.0);
    let state = prompt_state(app);
    let label = button_label(&state);
    let small = egui::FontId::proportional(t.font_small);
    let button_width = ui.painter().layout_no_wrap(label.to_string(), crate::fonts::medium(t.font_label), t.text).size().x + 2.0 * t.space_16;
    let spinner_width = if state == PromptState::Pending { t.space_14 + t.space_8 } else { 0.0 };
    let scope_text = trf("review.scope", &[("scope", &scope(app))]);
    let scope_width =
        (ui.painter().layout_no_wrap(scope_text.clone(), small.clone(), t.text_dim).size().x + 2.0 * t.space_8 + t.space_16).min(MAX_SCOPE_WIDTH);
    let key_text = crate::keymap::display(FOCUS_KEY);
    let key_width = ui.painter().layout_no_wrap(key_text.clone(), small.clone(), t.text_dim).size().x + 2.0 * t.space_6;
    let reason = if let PromptState::Blocked(why) = &state { Some(why.clone()) } else { None };
    let reason_width =
        reason.as_ref().map_or(0.0, |why| (ui.painter().layout_no_wrap(why.clone(), small.clone(), t.warning).size().x).min(MAX_REASON_WIDTH));
    let parts = 4 + usize::from(reason.is_some()) + usize::from(spinner_width > 0.0);
    let input_width =
        (ui.available_width() - button_width - spinner_width - scope_width - key_width - reason_width - parts as f32 * t.space_12).max(80.0);
    let response = ui
        .allocate_ui_with_layout(vec2(input_width, ui.available_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui_kit::bounded_name_field(ui, Id::new("sc_review_prompt_edit"), &mut app.review.prompt, tr("review.prompt_hint"), MAX_PROMPT)
        })
        .inner;
    if std::mem::take(&mut app.review.focus) {
        response.request_focus();
    }
    crate::scenario::publish_target("review:prompt", response.rect);
    let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    // The scope chip: a dropdown of what the request can be about.
    scope_chip(app, ui, &scope_text, scope_width);
    // The key that brings the focus here.
    let key = ui.add(egui::Label::new(egui::RichText::new(key_text).font(small.clone()).color(t.text_dim)).selectable(false));
    ui.painter().rect_stroke(
        key.rect.expand2(vec2(t.space_4, t.space_2)),
        t.control_radius,
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Outside,
    );
    key.on_hover_text(tr("review.focus_note"));
    // Blocked: the reason, a widget of its own (read by assistive technology, whole on hover).
    if let Some(why) = &reason {
        let r = ui
            .allocate_ui_with_layout(vec2(reason_width, ui.available_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(why).font(small.clone()).color(t.warning)).truncate())
            })
            .inner;
        crate::scenario::publish_rect("review:blocked", r.rect);
        r.on_hover_text(why);
    }
    if state == PromptState::Pending {
        let (rect, _) = ui.allocate_exact_size(vec2(t.space_14, t.space_14), egui::Sense::hover());
        egui::Spinner::new().size(t.space_14).paint_at(ui, rect);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
    let enabled = state == PromptState::Idle && !app.review.prompt.trim().is_empty();
    let note = match &state {
        PromptState::Ready => tr("review.preview_ready_note").to_string(),
        PromptState::Pending => tr("review.preview_pending_note").to_string(),
        PromptState::Blocked(why) => why.clone(),
        PromptState::Idle => tr("review.preview_note").to_string(),
    };
    let button = ui.add_enabled_ui(enabled, |ui| ui_kit::button(ui, label, ui_kit::Button::Normal)).inner.on_hover_text(note);
    crate::scenario::publish_target("review:preview", button.rect);
    if (button.clicked() || submitted) && enabled {
        let ctx = ui.ctx().clone();
        send_prompt(app, &ctx);
    }
}

/// The scope chip: "Scope: Hole 02 / 4 holes ▾", from the selection; its menu lists the
/// selection, the whole design, and each feature and body to choose instead.
fn scope_chip(app: &mut SolveApp, ui: &mut egui::Ui, text: &str, width: f32) {
    let t = Tokens::get();
    let doc = app.session.doc.clone();
    let bodies: Vec<String> = app.session.world_state().bodies.iter().map(|b| b.name.clone()).collect();
    let current = app.review.scope.clone();
    let mut chosen: Option<Scope> = None;
    let response = ui
        .allocate_ui_with_layout(vec2(width, ui.available_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::proportional(t.font_small));
            let r = ui_kit::dropdown_menu(ui, "sc_review_scope", text, |ui| {
                ui.style_mut().override_font_id = None;
                let mut option = |ui: &mut egui::Ui, scope: Scope, label: &str| {
                    let r = ui_kit::list_picker(ui, "review_scope", label, current == scope);
                    crate::scenario::publish_handle(&format!("review:scope:{label}"), r.rect.center());
                    if r.clicked() {
                        chosen = Some(scope);
                    }
                };
                option(ui, Scope::Selection, &trf("review.scope_selection", &[("scope", &selection_scope(app))]));
                option(ui, Scope::Design, tr("review.scope_all"));
                for f in doc.features.iter().filter(|f| !matches!(f.kind, FeatureKind::Sketch { .. })) {
                    option(ui, Scope::Feature(f.id), &feature_scope(&doc, f));
                }
                for b in &bodies {
                    option(ui, Scope::Body(b.clone()), b);
                }
            });
            r.on_hover_text(tr("review.scope_note"))
        })
        .inner;
    crate::scenario::publish_target("review:scope", response.rect);
    if let Some(scope) = chosen {
        app.review.scope = scope;
    }
}

// ---- The panel ----------------------------------------------------------------------------------

/// The Review changes panel, shown while a proposal is open, where Properties otherwise stand.
pub fn panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    app.review.rect = None;
    drop_if_stale(app);
    if app.review.open.is_none() {
        return;
    }
    let t = Tokens::get();
    let available = ui.available_rect_before_wrap();
    let container = Container::new(ui.ctx(), Surface::Panel);
    if container.floating() {
        let gap = container.gap();
        let mut right = available.right() - gap;
        if crate::agent_chat::shown(app)
            && app.agent_chat.open
            && app.preferences.agent_chat_side != "left"
            && let Some(chat) = app.agent_chat.panel_rect
        {
            right = right.min(chat.left() - gap);
        }
        let top = app.review.prompt_rect.map_or_else(|| app.shell.cards.last().map_or(available.top(), |c| c.bottom()), |r| r.bottom()) + gap;
        let mut bottom = available.bottom() - gap;
        if let Some(history) = app.shell.history.filter(|h| app.ui.show_timeline && h.right() > right - PANEL_WIDTH) {
            bottom = bottom.min(history.top() - gap);
        }
        let frame = container.frame(ui.ctx(), t.panel, crate::frame::Side::Right).inner_margin(t.space_10 as i8);
        let inner = PANEL_WIDTH - frame.total_margin().sum().x;
        let height = (bottom - top - frame.total_margin().sum().y).max(0.0);
        let card = container.area(Id::new("sc_review_card")).pivot(Align2::RIGHT_TOP).fixed_pos(pos2(right, top)).order(egui::Order::Middle).show(
            ui.ctx(),
            |ui| {
                container.show_frame(ui, frame, available, |ui| {
                    ui.set_width(inner);
                    ui.set_min_height(height);
                    ui_kit::scope(ui, |ui| contents(app, ui, height));
                });
            },
        );
        app.review.rect = Some(card.response.rect);
    } else {
        let r = container
            .edge(Id::new("sc_review_panel"), Edge::Right)
            .exact_size(PANEL_WIDTH)
            .resizable(false)
            .frame(container.edge_frame(t.panel))
            .show(ui, |ui| {
                let height = ui.available_height();
                ui_kit::scope(ui, |ui| contents(app, ui, height))
            });
        app.review.rect = Some(r.response.rect);
    }
    if let Some(rect) = app.review.rect {
        crate::scenario::publish_rect("review:panel", rect);
    }
}

fn contents(app: &mut SolveApp, ui: &mut egui::Ui, height: f32) {
    let Some(open) = app.review.open.clone() else { return };
    let t = Tokens::get();
    let top = ui.cursor().top();
    let mut close = false;
    ui_kit::header(ui, "inspect", tr("review.title"), || close = true);
    // MODEL COMMAND / title / target.
    ui.add_space(t.space_4);
    ui.label(egui::RichText::new(tr("review.model_command")).font(crate::fonts::medium(t.font_small)).color(t.accent));
    ui.add(egui::Label::new(egui::RichText::new(title(&open)).font(crate::fonts::medium(t.font_heading)).color(t.text)).wrap());
    let (target_name, _) = target(&open);
    let deps = dependents(&open);
    let d = &open.described;
    let line = match (d.instances, deps.len()) {
        (Some(n), _) if n > 1 => trf("review.target_instances", &[("target", &target_name), ("count", &n.to_string())]),
        (_, 1) => trf("review.target_one", &[("target", &target_name)]),
        (_, n) => trf("review.target_many", &[("target", &target_name), ("count", &n.to_string())]),
    };
    ui_kit::text(ui, &line, ui_kit::Text::Hint("review_target"));
    ui.add_space(t.space_4);
    ui_kit::divider(ui, "review_summary");
    // Proposed change: each changed value as before → after, then what the change keeps.
    ui_kit::text(ui, tr("review.proposed_change"), ui_kit::Text::Heading("review_proposed"));
    let r = &open.report;
    for p in r["parameters"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().unwrap_or_default();
        let before = p["before"].as_str().unwrap_or("—");
        let after = p["after"].as_str().unwrap_or("—");
        let shown = format!("{before} → {after}");
        ui_kit::row(ui, name, |ui| ui_kit::text(ui, &shown, ui_kit::Text::MonoCell));
    }
    for row in d.changes.iter().chain(&d.kept) {
        let shown = row.text();
        ui_kit::row(ui, tr_label(&row.label), |ui| ui_kit::text(ui, &shown, ui_kit::Text::MonoCell)).on_hover_text(&row.feature);
    }
    let described: Vec<&str> = d.changes.iter().map(|c| c.feature.as_str()).collect();
    for (list, change) in [("changed", "review.change.changed"), ("added", "review.change.added"), ("removed", "review.change.removed")] {
        for f in r["timeline"][list].as_array().into_iter().flatten() {
            let name = f["name"].as_str().unwrap_or_default();
            if described.contains(&name) {
                continue;
            }
            ui_kit::row(ui, name, |ui| ui_kit::text(ui, tr(change), ui_kit::Text::Cell));
        }
    }
    for b in r["bodies"]["changed"].as_array().into_iter().flatten() {
        let name = b["name"].as_str().unwrap_or_default();
        if let (Some(before), Some(after)) = (b["before"]["volume_mm3"].as_f64(), b["after"]["volume_mm3"].as_f64()) {
            let label = trf("review.body_volume", &[("body", name)]);
            let shown = volume_pair(app, before, after);
            ui_kit::row(ui, &label, |ui| ui_kit::text(ui, &shown, ui_kit::Text::MonoCell));
        }
    }
    for b in r["bodies"]["created"].as_array().into_iter().flatten() {
        let name = b["name"].as_str().unwrap_or_default();
        ui_kit::row(ui, name, |ui| ui_kit::text(ui, tr("review.change.added"), ui_kit::Text::Cell));
    }
    for b in r["bodies"]["removed"].as_array().into_iter().flatten() {
        let name = b["name"].as_str().unwrap_or_default();
        ui_kit::row(ui, name, |ui| ui_kit::text(ui, tr("review.change.removed"), ui_kit::Text::Cell));
    }
    ui.add_space(t.space_4);
    ui_kit::divider(ui, "review_scope");
    // Scope & dependencies.
    ui_kit::text(ui, tr("review.scope_heading"), ui_kit::Text::Heading("review_scope"));
    let params = names(&r["parameters"]);
    let mut features: Vec<String> = r["timeline"]["changed"].as_array().into_iter().flatten().map(|f| feature_words(app, f)).collect();
    features.extend(names(&r["timeline"]["added"]));
    features.extend(names(&r["timeline"]["removed"]));
    let mut lines = Vec::new();
    match params.len() {
        0 => {}
        1 => lines.push(trf("review.parameter_line", &[("names", &joined(&params))])),
        n => lines.push(trf("review.parameters_line", &[("count", &n.to_string()), ("names", &joined(&params))])),
    }
    match features.len() {
        0 => {}
        1 => lines.push(trf("review.feature_line", &[("names", &joined(&features))])),
        n => lines.push(trf("review.features_line", &[("count", &n.to_string()), ("names", &joined(&features))])),
    }
    match deps.len() {
        0 => lines.push(tr("review.no_dependents").to_string()),
        1 => lines.push(trf("review.dependent_line", &[("names", &joined(&deps))])),
        n => lines.push(trf("review.dependents_line", &[("count", &n.to_string()), ("names", &joined(&deps))])),
    }
    if let Some(preserved) = d.pattern_preserved {
        lines.push(tr(if preserved { "review.pattern_preserved" } else { "review.pattern_changed" }).to_string());
    }
    ui_kit::text(ui, &lines.join("\n"), ui_kit::Text::Hint("review_dependencies"));
    ui_kit::text(ui, tr("review.references"), ui_kit::Text::Note("review_references"));
    for w in r["warnings"].as_array().into_iter().flatten() {
        let line = trf("review.warning", &[("name", w["name"].as_str().unwrap_or_default()), ("text", w["warning"].as_str().unwrap_or_default())]);
        ui_kit::text(ui, &line, ui_kit::Text::Report("review_warning", false));
    }
    for e in r["errors"].as_array().into_iter().flatten() {
        let line = trf("review.warning", &[("name", e["name"].as_str().unwrap_or_default()), ("text", e["error"].as_str().unwrap_or_default())]);
        ui_kit::text(ui, &line, ui_kit::Text::Error("review_error"));
    }
    ui.add_space(t.space_8);
    // Apply & rebuild, Cancel command.
    let apply_button = ui_kit::wide_primary_button(ui, tr("review.apply"));
    crate::scenario::publish_target("review:apply", apply_button.rect);
    let cancel_button = ui
        .allocate_ui_with_layout(
            vec2(ui.available_width(), t.form_button_height),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| ui_kit::button(ui, tr("review.cancel"), ui_kit::Button::Normal),
        )
        .inner;
    crate::scenario::publish_target("review:cancel", cancel_button.rect);
    // The footer line sits at the bottom of a panel that has the room.
    let used = ui.cursor().top() - top;
    let footer = ui.painter().layout_no_wrap(tr("review.none_applied").to_string(), egui::FontId::proportional(t.font_small), t.text_dim).size().y;
    let spare = height - used - footer - 2.0 * t.space_8;
    if spare > 0.0 {
        ui.add_space(spare);
    }
    ui_kit::text(ui, tr("review.none_applied"), ui_kit::Text::Note("review_footer"));
    if apply_button.clicked() {
        let _ = apply(app);
    } else if cancel_button.clicked() || close {
        cancel(app);
    }
}

/// "before → after" of a body's volume, in the design's units.
fn volume_pair(app: &SolveApp, before: f64, after: f64) -> String {
    let (k, _) = crate::prefs::unit_scale(&app.session.doc.units);
    let shown = crate::prefs::show_mm(app, after, 3);
    // The "before" number alone, so the unit is written once.
    let decimals = shown.split(' ').next().and_then(|n| n.split('.').nth(1)).map_or(0, str::len);
    format!("{:.*} → {shown}", decimals, before * k.powi(3))
}

// ---- The receipt toast ---------------------------------------------------------------------------

/// The receipt toast's text ("<what> updated / <value>"), while it shows.
pub(crate) fn receipt_text_shown(app: &SolveApp) -> Option<String> {
    let r = app.review.receipt.as_ref()?;
    Some(trf("review.updated", &[("what", &r.what), ("value", &r.value)]))
}

// ---- The agent chat ------------------------------------------------------------------------------

/// A request of the agent's about to run: with "review before apply" on, or while the prompt row
/// waits for its preview, a design edit is proposed instead of run, the panel opens on it, and
/// the agent is answered with the preview. The design edits of one message join one proposal
/// (each request re-proposes them all), so a sketch and its extrude preview together. Anything
/// else (reads, views, files, and every request while the setting is off and no prompt waits)
/// is given back to run as before.
pub(crate) fn divert(app: &mut SolveApp, req: ControlRequest) -> Option<ControlRequest> {
    if !app.preferences.agent_review_before_apply && app.review.waiting.is_none() {
        return Some(req);
    }
    let params = |step: &Value| step.get("params").cloned().filter(|v| !v.is_null()).unwrap_or_else(|| json!({}));
    let steps: Vec<(String, Value)> = match req.method.as_str() {
        "engine.execute" | "command" => {
            let Some(id) = req.params.get("command").or_else(|| req.params.get("id")).and_then(Value::as_str) else { return Some(req) };
            let p = params(&req.params);
            if !proposable(id, &p) {
                return Some(req);
            }
            vec![(id.to_string(), p)]
        }
        "engine.batch" => {
            let Some(list) = req.params.get("commands").and_then(Value::as_array) else { return Some(req) };
            let mut steps = Vec::with_capacity(list.len());
            for step in list {
                let Some(id) = step.get("command").or_else(|| step.get("id")).and_then(Value::as_str) else { return Some(req) };
                let p = params(step);
                if !proposable(id, &p) {
                    return Some(req);
                }
                steps.push((id.to_string(), p));
            }
            if steps.is_empty() {
                return Some(req);
            }
            steps
        }
        _ => return Some(req),
    };
    let mine = steps.len();
    let mut all = match &app.review.open {
        Some(o) if o.source == Source::Agent || o.source == Source::Prompt => o.commands.clone(),
        _ => Vec::new(),
    };
    let first = all.is_empty();
    all.extend(steps);
    let label = crate::agent_chat::turn_label(app);
    let request = app.review.waiting.clone().or_else(|| app.review.open.as_ref().and_then(|o| o.request.clone()));
    let source = if request.is_some() { Source::Prompt } else { Source::Agent };
    // The request stays with the proposal it answers, through every edit of the message.
    if let Some(text) = &request
        && app.review.waiting.is_none()
    {
        app.review.waiting = Some(text.clone());
    }
    match propose(app, all, label.as_deref(), source) {
        Ok(report) => {
            let replies = report["replies"].as_array().cloned().unwrap_or_default();
            let own: Vec<Value> = replies.iter().skip(replies.len().saturating_sub(mine)).cloned().collect();
            let preview = json!({
                "bodies": report["bodies"], "totals": report["totals"], "timeline": report["timeline"],
                "parameters": report["parameters"], "warnings": report["warnings"], "errors": report["errors"],
            });
            let note = "proposed, not applied: the person reviews it in the Review changes panel and applies or cancels it. \
                        The design, and what you read from it, is unchanged until then; further design edits in this message join the same proposal";
            let result = if req.method == "engine.batch" {
                json!({"proposed": true, "proposal": report["proposal"], "completed": own.len(), "results": own, "preview": preview, "note": note})
            } else {
                json!({"proposed": true, "proposal": report["proposal"], "result": own.last().cloned().unwrap_or(Value::Null), "preview": preview, "note": note})
            };
            let _ = req.reply.send(json!({"ok": true, "result": result}));
            if first {
                app.agent_chat.notice_review_pending();
            }
            None
        }
        Err(e) => {
            // The step fails on the scratch copy (or is refused): the agent gets the error and
            // nothing ran; a proposal already open stays as it was, and a request keeps waiting
            // for the message to end or to propose again.
            let _ = req.reply.send(json!({"ok": false, "error": e}));
            None
        }
    }
}

// ---- Automation --------------------------------------------------------------------------------

fn rows_json(rows: &[Row]) -> Value {
    Value::Array(
        rows.iter().map(|r| json!({"feature": r.feature, "label": r.label, "before": r.before, "after": r.after, "text": r.text()})).collect(),
    )
}

/// The flow's state for `ui.review` and `ui.inspect`.
pub fn status(app: &SolveApp) -> Value {
    let open = app.review.open.as_ref().map(|o| {
        json!({
            "proposal": o.id, "label": o.label, "revision": o.revision, "steps": o.commands.len(),
            "source": match o.source { Source::Prompt => "prompt", Source::Agent => "agent", Source::Control => "control" },
            "request": o.request,
            "target": target(o).0, "dependents": dependents(o),
            "instances": o.described.instances, "pattern_preserved": o.described.pattern_preserved,
            "changes": rows_json(&o.described.changes), "kept": rows_json(&o.described.kept),
            "totals": o.report["totals"], "parameters": o.report["parameters"], "timeline": o.report["timeline"],
            "warnings": o.report["warnings"], "errors": o.report["errors"],
        })
    });
    let receipt = app
        .review
        .receipt
        .as_ref()
        .map(|r| json!({"what": r.what, "value": r.value, "text": receipt_text_shown(app), "pattern_preserved": r.pattern_preserved}));
    let (state, blocked) = match prompt_state(app) {
        PromptState::Idle => ("idle", None),
        PromptState::Pending => ("pending", None),
        PromptState::Ready => ("ready", None),
        PromptState::Blocked(why) => ("blocked", Some(why)),
    };
    json!({
        "open": open,
        "receipt": receipt,
        "prompt": app.review.prompt,
        "scope": scope(app),
        "state": state,
        "blocked": blocked,
        "waiting": app.review.waiting,
        "status": status_line(app),
        "review_before_apply": app.preferences.agent_review_before_apply,
    })
}

/// `ui.review`: drive the flow as the panel's buttons do. `propose: {commands, label?}` opens a
/// proposal; `prompt` sets the prompt row's text and `preview: true` sends it; `scope` picks the
/// chip's choice (`"selection"`, `"design"`, `{feature}` or `{body}`); `focus: true` puts the
/// keyboard in the prompt; `apply`, `cancel` and `undo` are the buttons; `review_before_apply`
/// sets the agent chat's setting.
pub fn control(app: &mut SolveApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    if let Some(v) = p.get("review_before_apply").and_then(Value::as_bool) {
        app.preferences.agent_review_before_apply = v;
    }
    if let Some(text) = p.get("prompt").and_then(Value::as_str) {
        app.review.prompt = text.chars().take(MAX_PROMPT).collect();
    }
    if let Some(s) = p.get("scope") {
        app.review.scope = match s {
            Value::String(k) if k == "selection" => Scope::Selection,
            Value::String(k) if k == "design" => Scope::Design,
            Value::Object(o) if o.get("feature").is_some() => {
                let key = o["feature"].as_str().map(str::to_owned).or_else(|| o["feature"].as_u64().map(|n| n.to_string())).unwrap_or_default();
                Scope::Feature(app.session.doc.find_feature(&key).map(|f| f.id).ok_or_else(|| format!("no feature `{key}`"))?)
            }
            Value::Object(o) if o.get("body").is_some() => Scope::Body(o["body"].as_str().unwrap_or_default().to_owned()),
            _ => return Err("scope must be \"selection\", \"design\", {feature} or {body}".into()),
        };
    }
    if p.get("focus").and_then(Value::as_bool) == Some(true) {
        app.review.focus = true;
    }
    if let Some(proposal) = p.get("propose") {
        let list = proposal.get("commands").and_then(Value::as_array).ok_or("propose.commands must be an array of {command, params}")?;
        let mut commands = Vec::with_capacity(list.len());
        for step in list {
            let id = step.get("command").or_else(|| step.get("id")).and_then(Value::as_str).ok_or("a step is missing `command`")?;
            commands.push((id.to_string(), step.get("params").cloned().filter(|v| !v.is_null()).unwrap_or_else(|| json!({}))));
        }
        propose(app, commands, proposal.get("label").and_then(Value::as_str), Source::Control)?;
    }
    if p.get("preview").and_then(Value::as_bool) == Some(true) {
        send_prompt(app, ctx);
    }
    if p.get("apply").and_then(Value::as_bool) == Some(true) {
        apply(app)?;
    }
    if p.get("cancel").and_then(Value::as_bool) == Some(true) && !cancel(app) {
        return Err("no proposal is open".into());
    }
    if p.get("undo").and_then(Value::as_bool) == Some(true) {
        undo_receipt(app)?;
    }
    Ok(status(app))
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;

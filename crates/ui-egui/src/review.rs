//! The AI review flow (design kit views 04 and 05): a prompt row under the ribbon, a Review
//! changes panel while a proposal is open, the proposed geometry shown in the viewport, and after
//! Apply a receipt toast with Undo. Every action is a command: `engine.propose` makes the proposal
//! (a scratch-copy preview that leaves the design alone), `engine.apply_proposal` applies it as one
//! undo step, `engine.drop_proposal` is Cancel command, and the toast's Undo is `edit.undo` of
//! exactly that step. The agent chat's design edits come through here too while "review before
//! apply" is on (the default): see [`divert`].

use egui::{Align2, Id, Rect, pos2, vec2};
use serde_json::{Value, json};

use crate::i18n::{format as trf, tr};
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
}

/// The receipt after Apply: the toast's text, and the design revision it left, so Undo undoes
/// that step only (any edit, undo or redo since moves the revision on).
#[derive(Clone, Debug)]
pub struct Receipt {
    pub what: String,
    pub value: String,
    pub revision: u64,
    pub at: f64,
}

#[derive(Default)]
pub struct Review {
    /// The prompt row's text.
    pub prompt: String,
    pub open: Option<Open>,
    pub receipt: Option<Receipt>,
    /// The prompt sent to the agent, until its proposal arrives (the button reads "Preview ready").
    pub waiting: Option<String>,
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
    let open = Open {
        id: report["proposal"].as_str().unwrap_or_default().to_string(),
        label: report["label"].as_str().unwrap_or_default().to_string(),
        revision: report["revision"].as_u64().unwrap_or(app.session.revision),
        commands,
        report: report.clone(),
        source,
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
            app.review.receipt = Some(Receipt { what, value, revision: app.session.revision, at: crate::now_ms() });
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
        Some(c) => crate::i18n::tr_label(c.label).to_string(),
        None => open.label.clone(),
    }
}

/// The features rebuilt because an input changed (the dependents).
fn dependents(open: &Open) -> Vec<String> {
    names(&open.report["timeline"]["affected"])
}

/// The receipt's words: "<what> updated / <value>".
fn receipt_text(app: &SolveApp, open: &Open) -> (String, String) {
    let r = &open.report;
    let (what, _) = target(open);
    if let Some(p) = r["parameters"].as_array().filter(|a| !a.is_empty()) {
        let values: Vec<String> = p.iter().filter_map(|p| p["after"].as_str().map(str::to_owned)).collect();
        return (what, values.join(", "));
    }
    if let Some(after) = r["totals"]["after"]["volume_mm3"].as_f64() {
        return (what, crate::prefs::show_mm(app, after, 3));
    }
    (what, String::new())
}

/// The status line while a proposal is open or just applied (the kit's bottom status bar).
pub(crate) fn status_line(app: &SolveApp) -> Option<String> {
    if let Some(open) = &app.review.open {
        return Some(trf("review.status_open", &[("feature", &target(open).0)]));
    }
    app.review.receipt.as_ref().map(|_| tr("review.status_applied").to_string())
}

/// What the prompt's scope is: the selected feature or bodies, else the whole design.
fn scope(app: &SolveApp) -> String {
    use polykrom_engine::Sel;
    for s in &app.session.selection {
        if let Sel::Feature { id } = s
            && let Some(f) = app.session.doc.feature(*id)
        {
            return f.name.clone();
        }
    }
    let bodies = crate::inspector::selected_bodies(app);
    match bodies.as_slice() {
        [] => tr("review.scope_all").to_string(),
        [one] => one.clone(),
        many => trf("review.scope_bodies", &[("count", &many.len().to_string())]),
    }
}

// ---- The prompt row -----------------------------------------------------------------------------

/// The prompt row is part of the AI features: without them (or in the browser) it is not shown.
fn prompt_shown(app: &SolveApp) -> bool {
    crate::agent_chat::shown(app) && !crate::render_workspace::active(app)
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
    let message = format!("{text}\n\n{}", trf("review.scope", &[("scope", &scope(app))]));
    let notices = app.agent_chat.items.len();
    crate::agent_chat::send(app, ctx, message);
    app.review.waiting = Some(text);
    // A chat that could not take the message (no agent) says why in its own transcript: the
    // status line repeats it, as the transcript may be closed.
    let refused = app.agent_chat.items.iter().skip(notices).find_map(|i| match i {
        crate::agent_chat::Item::Notice { text, error: true } => Some(text.clone()),
        _ => None,
    });
    match refused {
        Some(why) => {
            app.review.waiting = None;
            app.set_status(why, true);
        }
        None => app.set_status(tr("review.status_sent"), false),
    }
}

/// The prompt row under the ribbon (the kit's "Native model command bar"), a top strip in both
/// layouts so the panels and the viewport's controls start below it: docked, a flush bar joining
/// the ribbon; floating, a card under the ribbon cards with the shell gap below it.
pub fn prompt_row(app: &mut SolveApp, ui: &mut egui::Ui) {
    app.review.prompt_rect = None;
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

fn prompt_contents(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    ui.spacing_mut().item_spacing = vec2(t.space_12, 0.0);
    let (icon, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
    crate::icons::paint_icon(ui.painter(), icon, "assistant", 1.0);
    let ready = app.review.open.is_some();
    let button_label = if ready { tr("review.preview_ready") } else { tr("review.preview") };
    let button_width = ui.painter().layout_no_wrap(button_label.to_string(), crate::fonts::medium(t.font_label), t.text).size().x + 2.0 * t.space_16;
    let scope_text = trf("review.scope", &[("scope", &scope(app))]);
    let scope_width = ui.painter().layout_no_wrap(scope_text.clone(), egui::FontId::proportional(t.font_small), t.text_dim).size().x;
    let input_width = (ui.available_width() - button_width - scope_width - 3.0 * t.space_12).max(80.0);
    let response = ui
        .allocate_ui_with_layout(vec2(input_width, ui.available_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui_kit::bounded_name_field(ui, Id::new("sc_review_prompt_edit"), &mut app.review.prompt, tr("review.prompt_hint"), MAX_PROMPT)
        })
        .inner;
    crate::scenario::publish_target("review:prompt", response.rect);
    let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let chip =
        ui.add(egui::Label::new(egui::RichText::new(scope_text).font(egui::FontId::proportional(t.font_small)).color(t.text_dim)).selectable(false));
    chip.on_hover_text(tr("review.scope_note"));
    let enabled = !ready && !app.review.prompt.trim().is_empty();
    let button = ui.add_enabled_ui(enabled, |ui| ui_kit::button(ui, button_label, ui_kit::Button::Normal)).inner.on_hover_text(if ready {
        tr("review.preview_ready_note")
    } else {
        tr("review.preview_note")
    });
    crate::scenario::publish_target("review:preview", button.rect);
    if (button.clicked() || submitted) && enabled {
        let ctx = ui.ctx().clone();
        send_prompt(app, &ctx);
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
    let line = match deps.len() {
        1 => trf("review.target_one", &[("target", &target_name)]),
        n => trf("review.target_many", &[("target", &target_name), ("count", &n.to_string())]),
    };
    ui_kit::text(ui, &line, ui_kit::Text::Hint("review_target"));
    ui.add_space(t.space_4);
    ui_kit::divider(ui, "review_summary");
    // Proposed change: before → after.
    ui_kit::text(ui, tr("review.proposed_change"), ui_kit::Text::Heading("review_proposed"));
    let r = &open.report;
    for p in r["parameters"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().unwrap_or_default();
        let before = p["before"].as_str().unwrap_or("—");
        let after = p["after"].as_str().unwrap_or("—");
        let shown = format!("{before} → {after}");
        ui_kit::row(ui, name, |ui| ui_kit::text(ui, &shown, ui_kit::Text::MonoCell));
    }
    for (list, change) in [("changed", "review.change.changed"), ("added", "review.change.added"), ("removed", "review.change.removed")] {
        for f in r["timeline"][list].as_array().into_iter().flatten() {
            let name = f["name"].as_str().unwrap_or_default();
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
    let mut features = names(&r["timeline"]["changed"]);
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

/// A request of the agent's about to run: with "review before apply" on, a design edit is
/// proposed instead of run, the panel opens on it, and the agent is answered with the preview.
/// The design edits of one message join one proposal (each request re-proposes them all), so a
/// sketch and its extrude preview together. Anything else (reads, views, files, and every request
/// while the setting is off) is given back to run as before.
pub(crate) fn divert(app: &mut SolveApp, req: ControlRequest) -> Option<ControlRequest> {
    if !app.preferences.agent_review_before_apply {
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
    let source = if app.review.waiting.is_some() { Source::Prompt } else { Source::Agent };
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
            // nothing ran; a proposal already open stays as it was.
            let _ = req.reply.send(json!({"ok": false, "error": e}));
            None
        }
    }
}

// ---- Automation --------------------------------------------------------------------------------

/// The flow's state for `ui.review` and `ui.inspect`.
pub fn status(app: &SolveApp) -> Value {
    let open = app.review.open.as_ref().map(|o| {
        json!({
            "proposal": o.id, "label": o.label, "revision": o.revision, "steps": o.commands.len(),
            "source": match o.source { Source::Prompt => "prompt", Source::Agent => "agent", Source::Control => "control" },
            "target": target(o).0, "dependents": dependents(o),
            "totals": o.report["totals"], "parameters": o.report["parameters"], "timeline": o.report["timeline"],
            "warnings": o.report["warnings"], "errors": o.report["errors"],
        })
    });
    let receipt = app.review.receipt.as_ref().map(|r| json!({"what": r.what, "value": r.value, "text": receipt_text_shown(app)}));
    json!({
        "open": open,
        "receipt": receipt,
        "prompt": app.review.prompt,
        "waiting": app.review.waiting,
        "status": status_line(app),
        "review_before_apply": app.preferences.agent_review_before_apply,
    })
}

/// `ui.review`: drive the flow as the panel's buttons do. `propose: {commands, label?}` opens a
/// proposal; `prompt` sets the prompt row's text and `preview: true` sends it; `apply`, `cancel`
/// and `undo` are the buttons; `review_before_apply` sets the agent chat's setting.
pub fn control(app: &mut SolveApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    if let Some(v) = p.get("review_before_apply").and_then(Value::as_bool) {
        app.preferences.agent_review_before_apply = v;
    }
    if let Some(text) = p.get("prompt").and_then(Value::as_str) {
        app.review.prompt = text.chars().take(MAX_PROMPT).collect();
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

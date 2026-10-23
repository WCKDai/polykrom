//! The inspector: what the selected bodies are (material, mass, volume, size) and the checks run on
//! them (static stress and its safety factor). It appears when bodies are selected, at the bottom
//! right of the viewport (floating layout) or as a right-hand panel (docked layout).
//!
//! Checks are recorded wherever `analysis.static_stress` runs, from the dialog, a control request,
//! MCP or the agent chat, so the person and the agent see the same results.

use std::collections::HashMap;

use egui::{Align2, Id, Rect, pos2};
use serde_json::{Value, json};

use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{self, Container, Edge, Surface};
use solidcraft_engine::Sel;

/// The card's width (logical points).
const WIDTH: f32 = 268.0;

/// The inspector's state: the last measurement, and the checks run on each body.
#[derive(Default)]
pub struct Inspector {
    /// `inspect.measure` of the bodies at a document revision.
    measured: Option<(Vec<String>, u64, Value)>,
    /// The last static stress result for each body, with the body's fingerprint when it ran
    /// (volume, area, box and material): a check is stale when the body it judged has changed.
    stress: HashMap<String, (Value, Value)>,
    /// Hidden by its close button until the selection changes.
    dismissed: Option<Vec<String>>,
    /// The card's rect this frame (it covers the viewport for input).
    pub rect: Option<Rect>,
}

impl Inspector {
    /// Remember a command's result if it is a check the inspector shows.
    pub fn record(&mut self, id: &str, reply: &Value, session: &mut solidcraft_engine::Session) {
        if id == "analysis.static_stress"
            && let Some(body) = reply.get("body").and_then(Value::as_str)
        {
            let measured = session.execute("inspect.measure", &json!({ "bodies": [body] })).unwrap_or(Value::Null);
            self.stress.insert(body.to_owned(), (fingerprint(&measured), reply.clone()));
        }
    }
}

/// What a check depends on: the body's volume, area, box and material, rounded so that a
/// re-measurement of the same body matches.
fn fingerprint(measured: &Value) -> Value {
    let round = |v: &Value| v.as_f64().map(|x| (x * 1e6).round() / 1e6);
    let total = &measured["total"];
    let one = &measured["bodies"][0];
    let boxed = if total.get("bbox").is_some() { total } else { one };
    let corner = |k: &str| boxed["bbox"][k].as_array().map(|a| a.iter().map(round).collect::<Vec<_>>());
    json!({
        "volume": round(&total["volume_mm3"]),
        "area": round(&total["area_mm2"]),
        "min": corner("min"),
        "max": corner("max"),
        "material": one["material"],
    })
}

impl Inspector {
    fn current_fingerprint(&self, body: &str) -> Option<Value> {
        self.measured.as_ref().filter(|(b, _, _)| b.len() == 1 && b[0] == body).map(|(_, _, v)| fingerprint(v))
    }
}

/// The bodies the selection names, in selection order: picked bodies, and the bodies of picked
/// faces and edges.
pub fn selected_bodies(app: &SolveApp) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in &app.session.selection {
        let name = match s {
            Sel::Body { name } => name,
            Sel::Face { body, .. } | Sel::Edge { body, .. } => body,
            _ => continue,
        };
        if !out.contains(name) {
            out.push(name.clone());
        }
    }
    out
}

/// Draw the inspector for the current selection, if any.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui) {
    app.inspector.rect = None;
    let bodies = selected_bodies(app);
    if bodies.is_empty() || app.session.active_sketch.is_some() {
        app.inspector.dismissed = None;
        return;
    }
    if app.inspector.dismissed.as_ref() == Some(&bodies) {
        return;
    }
    app.inspector.dismissed = None;
    let revision = app.session.revision;
    let fresh = app.inspector.measured.as_ref().is_some_and(|(b, r, _)| *b == bodies && *r == revision);
    if !fresh {
        let measured = app.session.execute("inspect.measure", &json!({ "bodies": bodies })).unwrap_or(Value::Null);
        app.inspector.measured = Some((bodies.clone(), revision, measured));
    }
    let available = ui.available_rect_before_wrap();
    if Container::for_layout(app.ui.layout, Surface::Panel).floating() {
        floating(app, ui, available, &bodies);
    } else {
        let t = Tokens::get();
        let r = Container::new(ui.ctx(), Surface::Panel)
            .edge(Id::new("sc_inspector_panel"), Edge::Right)
            .exact_size(WIDTH)
            .resizable(false)
            .frame(Container::new(ui.ctx(), Surface::Panel).edge_frame(t.panel))
            .show(ui, |ui| ui_kit::scope(ui, |ui| contents(app, ui, &bodies)));
        app.inspector.rect = Some(r.response.rect);
    }
    if let Some(rect) = app.inspector.rect {
        crate::scenario::publish_rect("inspector:card", rect);
    }
}

/// The floating card, anchored at the viewport's bottom right: above History, left of a chat
/// docked on the right.
fn floating(app: &mut SolveApp, ui: &mut egui::Ui, available: Rect, bodies: &[String]) {
    let container = Container::new(ui.ctx(), Surface::Panel);
    let gap = container.gap();
    let mut right = available.right() - gap;
    if crate::agent_chat::shown(app)
        && app.agent_chat.open
        && app.preferences.agent_chat_side != "left"
        && let Some(chat) = app.agent_chat.panel_rect
    {
        right = right.min(chat.left() - gap);
    }
    let mut bottom = available.bottom() - gap;
    if let Some(history) = app.shell.history.filter(|h| app.ui.show_timeline && h.right() > right - WIDTH) {
        bottom = bottom.min(history.top() - gap);
    }
    let t = Tokens::get();
    let frame = container.frame(ui.ctx(), t.panel, crate::frame::Side::Right).inner_margin(t.space_10 as i8);
    let inner = WIDTH - frame.total_margin().sum().x;
    let card =
        container.area(Id::new("sc_inspector_card")).pivot(Align2::RIGHT_BOTTOM).fixed_pos(pos2(right, bottom)).order(egui::Order::Middle).show(
            ui.ctx(),
            |ui| {
                container.show_frame(ui, frame, available, |ui| {
                    ui.set_width(inner);
                    contents(app, ui, bodies);
                });
            },
        );
    app.inspector.rect = Some(card.response.rect);
}

fn contents(app: &mut SolveApp, ui: &mut egui::Ui, bodies: &[String]) {
    let title = if let [one] = bodies { one.clone() } else { crate::i18n::format("inspector.bodies", &[("count", &bodies.len().to_string())]) };
    let mut hide = false;
    ui_kit::header(ui, "body", &title, || hide = true);
    if hide {
        app.inspector.dismissed = Some(bodies.to_vec());
    }
    let measured = app.inspector.measured.as_ref().map(|(_, _, v)| v.clone()).unwrap_or(Value::Null);
    let total = &measured["total"];
    let number = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64);
    if let Some([one]) = measured["bodies"].as_array().map(Vec::as_slice) {
        let material = one["material"].as_str().unwrap_or("Default").to_owned();
        ui_kit::row(ui, crate::i18n::tr_label("Material"), |ui| ui_kit::text(ui, &material, ui_kit::Text::Cell));
    }
    if let Some(mass) = number(total, "mass_g") {
        ui_kit::row(ui, crate::i18n::tr_label("Mass"), |ui| ui_kit::text(ui, &format!("{mass:.1} g"), ui_kit::Text::MonoCell));
    }
    if let Some(volume) = number(total, "volume_mm3") {
        let shown = crate::prefs::show_mm(app, volume, 3);
        ui_kit::row(ui, crate::i18n::tr_label("Volume"), |ui| ui_kit::text(ui, &shown, ui_kit::Text::MonoCell));
    }
    // One body's box is on the body; several bodies' on the total.
    let boxed = if total.get("bbox").is_some() { total } else { &measured["bodies"][0] };
    let size = match (boxed["bbox"]["min"].as_array(), boxed["bbox"]["max"].as_array()) {
        (Some(a), Some(z)) => {
            let unit = crate::prefs::unit_scale(&app.session.doc.units).1;
            // Trailing zeros dropped so three sizes fit the card: 200 × 10 × 10 mm.
            let trim = |s: String| if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s };
            let d: Vec<String> =
                a.iter().zip(z).map(|(a, z)| trim(crate::prefs::show_mm_bare(app, z.as_f64().unwrap_or(0.0) - a.as_f64().unwrap_or(0.0)))).collect();
            Some(format!("{} {unit}", d.join(" × ")))
        }
        _ => None,
    };
    if let Some(size) = size {
        ui_kit::row(ui, crate::i18n::tr_label("Size"), |ui| ui_kit::text(ui, &size, ui_kit::Text::MonoCell));
    }
    ui_kit::section(ui, "sc_inspector_checks", crate::i18n::tr_label("Checks"), true, |ui| checks(app, ui, bodies));
}

fn checks(app: &mut SolveApp, ui: &mut egui::Ui, bodies: &[String]) {
    let t = Tokens::get();
    let [body] = bodies else {
        ui_kit::text(ui, crate::i18n::tr("inspector.one_body"), ui_kit::Text::Note("inspector_one_body"));
        return;
    };
    match app.inspector.stress.get(body).cloned() {
        Some((judged, r)) if r.get("error").is_none() => {
            let number = |k: &str| r[k].as_f64().unwrap_or(f64::NAN);
            ui_kit::row(ui, crate::i18n::tr_label("Max stress"), |ui| {
                ui_kit::text(ui, &format!("{:.1} MPa", number("max_von_mises_mpa")), ui_kit::Text::MonoCell)
            });
            ui_kit::row(ui, crate::i18n::tr_label("Max deflection"), |ui| {
                ui_kit::text(ui, &format!("{:.3} mm", number("max_displacement_mm")), ui_kit::Text::MonoCell)
            });
            if let Some(sf) = r["safety_factor"].as_f64() {
                // Colour says it at a glance; the words under it say it too (never colour alone).
                let (ink, word) = if sf < 1.0 {
                    (t.error, "inspector.sf_fails")
                } else if sf < 2.0 {
                    (t.warning, "inspector.sf_low")
                } else {
                    (t.form_success, "inspector.sf_ok")
                };
                ui_kit::row(ui, crate::i18n::tr_label("Safety factor"), |ui| {
                    ui.label(egui::RichText::new(format!("{sf:.2}")).font(egui::FontId::monospace(t.font_body)).color(ink))
                });
                ui_kit::caption(ui, crate::i18n::tr_label("Safety factor"), crate::i18n::tr(word));
            }
            if app.inspector.current_fingerprint(body).is_some_and(|now| now != judged) {
                ui_kit::text(ui, crate::i18n::tr("inspector.stale"), ui_kit::Text::Note("inspector_stale"));
            }
        }
        _ => {
            ui_kit::text(ui, crate::i18n::tr("inspector.unchecked"), ui_kit::Text::Note("inspector_unchecked"));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    if ui_kit::button(ui, crate::i18n::tr_label("Check stress…"), ui_kit::Button::Normal).clicked() {
        app.start("analysis.static_stress");
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::scenario::Harness;

    fn bracket() -> Harness {
        let mut h = Harness::new();
        h.app.run("solid.box", json!({"length": 40, "width": 20, "height": 10})).unwrap();
        h
    }

    #[test]
    fn selecting_a_body_shows_what_it_is_and_hiding_lasts_until_the_selection_changes() {
        let mut h = bracket();
        h.frames(2);
        assert!(crate::scenario::rect_of("inspector:card").is_none(), "nothing selected, no inspector");
        let body = h.app.session.world_state().bodies[0].name.clone();
        h.app.session.selection = vec![solidcraft_engine::Sel::Body { name: body.clone() }];
        h.frames(2);
        assert!(crate::scenario::rect_of("inspector:card").is_some());
        assert!(h.widget(&body).is_some(), "the body's name heads the card");
        let volume = crate::prefs::show_mm(&h.app, 8000.0, 3);
        assert!(h.widget(&volume).is_some(), "its volume, {volume}");
        assert!(h.widget(crate::i18n::tr_label("Size")).is_some(), "its size");
        assert!(h.widget(crate::i18n::tr("inspector.unchecked")).is_some(), "no check has run");
        h.app.inspector.dismissed = Some(vec![body.clone()]);
        h.frames(2);
        assert!(crate::scenario::rect_of("inspector:card").is_none(), "hidden");
        h.app.session.selection.clear();
        h.frames(1);
        h.app.session.selection = vec![solidcraft_engine::Sel::Body { name: body }];
        h.frames(2);
        assert!(crate::scenario::rect_of("inspector:card").is_some(), "a new selection shows it again");
    }

    #[test]
    fn a_stress_result_from_any_caller_shows_with_its_safety_factor_and_goes_stale_when_its_body_changes() {
        let mut h = bracket();
        let body = h.app.session.world_state().bodies[0].name.clone();
        // What `analysis.static_stress` replies; recorded as `SolveApp::run` records it.
        let reply = json!({"body": body, "max_von_mises_mpa": 114.8, "max_displacement_mm": 0.165, "safety_factor": 2.17});
        h.app.inspector.record("analysis.static_stress", &reply, &mut h.app.session);
        h.app.session.selection = vec![solidcraft_engine::Sel::Body { name: body.clone() }];
        h.frames(2);
        assert!(h.widget("114.8 MPa").is_some());
        assert!(h.widget("0.165 mm").is_some());
        assert!(h.widget("2.17").is_some());
        assert!(h.widget(crate::i18n::tr("inspector.stale")).is_none());
        h.app.run("select.set", json!({"items": [{"type": "body", "name": body}]})).unwrap();
        h.frames(2);
        assert!(h.widget(crate::i18n::tr("inspector.stale")).is_none(), "selecting is not an edit");
        h.app.run("solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [100, 0, 0]})).unwrap();
        h.app.session.selection = vec![solidcraft_engine::Sel::Body { name: body.clone() }];
        h.frames(2);
        assert!(h.widget(crate::i18n::tr("inspector.stale")).is_none(), "another body changed, not this one");
        h.app.run("material.assign", json!({"bodies": [body.clone()], "material": "Steel"})).unwrap();
        h.app.session.selection = vec![solidcraft_engine::Sel::Body { name: body }];
        h.frames(2);
        assert!(h.widget(crate::i18n::tr("inspector.stale")).is_some(), "its material changed after the check");
    }
}

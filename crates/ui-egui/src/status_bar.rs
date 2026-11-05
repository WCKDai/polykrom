//! The status bar along the window's bottom (design kit view 01): the selection summary and the
//! model units at the left; at the right the controls that left the title bar (the assistant,
//! the start page, Preferences, Help and Report a bug), the version, and the one discreet credit,
//! "Polykrom by WCKD". Report a bug and the version no longer share a corner.
use egui::{FontId, Rect, Sense, pos2, vec2};

use crate::{
    SolveApp, icons,
    theme::Tokens,
    ui_kit::{Container, Edge, Surface},
};

/// The bar's height (the kit's 36 px).
pub const HEIGHT: f32 = 36.0;
/// The one credit in the application chrome.
pub const CREDIT: &str = "Polykrom by WCKD";
/// From the window's edge to the first and the last text.
const INSET: f32 = 16.0;
/// The right-hand icon buttons.
const BUTTON: f32 = 26.0;
/// Below this window height the bar steps aside: a tiny window needs the room for the model.
const MIN_WINDOW_HEIGHT: f32 = 480.0;

/// What is selected, in a few words: `4 edges selected`, `1 body selected`, `Nothing selected`.
/// A count of one takes the kind's singular message (`ui.sel_kind_bodies_one`), as the browser's
/// counts do.
pub(crate) fn selection_summary(app: &SolveApp) -> String {
    use polykrom_engine::Sel;
    let selection = &app.session.selection;
    if selection.is_empty() {
        return crate::i18n::tr("ui.status_nothing_selected").to_string();
    }
    let one = selection.len() == 1;
    let kind = |s: &Sel| match (s, one) {
        (Sel::Body { .. } | Sel::Occurrence { .. }, false) => "ui.sel_kind_bodies",
        (Sel::Body { .. } | Sel::Occurrence { .. }, true) => "ui.sel_kind_bodies_one",
        (Sel::Face { .. }, false) => "ui.sel_kind_faces",
        (Sel::Face { .. }, true) => "ui.sel_kind_faces_one",
        (Sel::Edge { .. }, false) => "ui.sel_kind_edges",
        (Sel::Edge { .. }, true) => "ui.sel_kind_edges_one",
        (Sel::Feature { .. }, false) => "ui.sel_kind_features",
        (Sel::Feature { .. }, true) => "ui.sel_kind_features_one",
        (Sel::SketchCurve { .. } | Sel::SketchPoint { .. } | Sel::ExternalSketchCurve { .. }, false) => "ui.sel_kind_sketch_items",
        (Sel::SketchCurve { .. } | Sel::SketchPoint { .. } | Sel::ExternalSketchCurve { .. }, true) => "ui.sel_kind_sketch_items_one",
        (_, false) => "ui.sel_kind_items",
        (_, true) => "ui.sel_kind_items_one",
    };
    let count = selection.len().to_string();
    let first = kind(&selection[0]);
    if selection.iter().all(|s| kind(s) == first) {
        crate::i18n::format("ui.status_selected_kind", &[("count", &count), ("kind", crate::i18n::tr(first))])
    } else {
        crate::i18n::format("ui.status_selected", &[("count", &count)])
    }
}

/// One of the bar's icon buttons, `BUTTON` square, its right edge at `right`.
fn icon_button(ui: &mut egui::Ui, right: f32, cy: f32, icon: &str, name: &str, tip: &str, enabled: bool, handles: &[&str]) -> (Rect, bool) {
    let t = Tokens::get();
    let rect = Rect::from_min_size(pos2(right - BUTTON, cy - BUTTON / 2.0), vec2(BUTTON, BUTTON));
    let resp =
        ui.interact(rect, ui.id().with(("status", handles.first().copied().unwrap_or(icon))), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    for handle in handles {
        crate::scenario::publish_target(handle, rect);
    }
    if enabled && (resp.hovered() || resp.has_focus()) {
        ui.painter().rect_filled(rect, t.radius_sm, t.hover);
    }
    icons::paint_icon(ui.painter(), rect.shrink(5.0), icon, if enabled { f32::from(t.icon.a()) / 255.0 } else { 0.35 });
    (rect, enabled && resp.on_hover_text(tip).clicked())
}

/// The bar's panel along the window's bottom, reserved before the rest of the window so
/// everything else stacks above it. Its controls are drawn by [`contents`] once the title bar
/// and the ribbon have registered theirs, so keyboard focus walks the window in reading order
/// and reaches the bar last.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    if ui.ctx().content_rect().height() < MIN_WINDOW_HEIGHT {
        app.shell.status_bar = None;
        return;
    }
    let frame = egui::Frame::NONE.fill(t.window);
    Container::new(ui.ctx(), Surface::ApplicationBar)
        .edge("sc_status_bar", Edge::Bottom)
        .exact_size(HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(frame)
        .show(ui, |ui| {
            let r = ui.max_rect();
            app.shell.status_bar = Some(r);
            crate::scenario::publish_rect("status:bar", r);
            // Blank bar space owns its clicks (and is no stop for the keyboard).
            ui.interact(r, ui.id().with("status_bar_bg"), Sense::CLICK);
        });
    // Report a bug freezes the CAD input at its click (`CadInput::block` follows this), so the
    // click is taken here, before the viewport and the dialogs run, rather than where the
    // button draws at the frame's end (`contents`): an open dimension edit stays open.
    if report_clicked(ui.ctx()) {
        crate::help::run(app, &ui.ctx().clone(), "report");
    }
}

/// The Report a bug button's id, kept by [`contents`] for [`report_clicked`].
const REPORT_BUTTON: &str = "sc_status_report_button";
/// The pass in which [`show`] took the button's click, so [`contents`] leaves it.
const REPORT_TAKEN: &str = "sc_status_report_taken";

/// Whether this frame's click lands on the Report a bug button (as egui's own hit test reads it:
/// the widget drawn last frame under a press and release). A click is marked taken for this pass.
fn report_clicked(ctx: &egui::Context) -> bool {
    let Some(id) = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(REPORT_BUTTON))) else { return false };
    let clicked = ctx.input(|i| i.pointer.primary_clicked()) && ctx.viewport(|v| v.interact_widgets.clicked == Some(id));
    if clicked {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(REPORT_TAKEN), pass));
    }
    clicked
}

/// The bar's controls, over the panel [`show`] reserved this frame (nothing when the window is
/// too short for the bar).
pub fn contents(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let Some(r) = app.shell.status_bar.filter(|_| ui.ctx().content_rect().height() >= MIN_WINDOW_HEIGHT) else { return };
    let mut help_anchor: Option<Rect> = None;
    // On the start page the bar keeps only its right-hand controls (there is no selection).
    let design = !app.home.open;
    let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("status_bar_contents"));
    bar.set_clip_rect(r.intersect(ui.clip_rect()));
    let ui = &mut bar;
    let cy = r.center().y;
    let font = FontId::proportional(t.font_small);
    let painter = ui.painter().clone();
    // Left: the selection, then the model units (a click changes them: Change Design Units).
    if design {
        let summary = painter.layout_no_wrap(selection_summary(app), font.clone(), t.text_dim);
        let sel = Rect::from_min_size(pos2(r.left() + INSET, cy - summary.size().y / 2.0), summary.size());
        painter.galley(sel.min, summary, t.text_dim);
        crate::scenario::publish_rect("status:selection", sel);
        let units_text = crate::i18n::format("ui.status_units", &[("units", &app.session.doc.units)]);
        let units = painter.layout_no_wrap(units_text, font.clone(), t.text_dim);
        let ur = Rect::from_min_size(pos2(sel.right() + 30.0, cy - units.size().y / 2.0), units.size());
        let uresp = ui.interact(ur.expand2(vec2(6.0, 5.0)), ui.id().with("status_units"), Sense::click());
        uresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, crate::i18n::tr("ui.units")));
        crate::scenario::publish_target("status:units", ur);
        if uresp.hovered() || uresp.has_focus() {
            painter.rect_filled(ur.expand2(vec2(6.0, 5.0)), t.radius_sm, t.hover);
        }
        painter.galley(ur.min, units, t.text_dim);
        if uresp.on_hover_text(crate::i18n::tr_label("Change Design Units")).clicked() {
            crate::context_menu::open_for(app, ur.left_top(), crate::context_menu::Target::Units);
        }
    }
    // Right: the credit, then the version, then the controls from the title bar.
    let credit = painter.layout_no_wrap(CREDIT.to_string(), font.clone(), t.text_dim);
    let cr = Rect::from_min_size(pos2(r.right() - INSET - credit.size().x, cy - credit.size().y / 2.0), credit.size());
    painter.galley(cr.min, credit, t.text_dim);
    crate::scenario::publish_rect("status:credit", cr);
    let version_text = crate::i18n::format("ui.version_label", &[("version", crate::update::current_version())]);
    let version = painter.layout_no_wrap(version_text, font, t.text_dim.gamma_multiply(0.8));
    let vr = Rect::from_min_size(pos2(cr.left() - 24.0 - version.size().x, cy - version.size().y / 2.0), version.size());
    painter.galley(vr.min, version, t.text_dim.gamma_multiply(0.8));
    crate::scenario::publish_rect("titlebar:version", vr);
    crate::scenario::publish_rect("status:version", vr);
    let idle = !app.file_load.busy();
    let mut x = vr.left() - 18.0;
    let report_name = crate::i18n::tr("ui.report_a_bug");
    let (report, clicked) = icon_button(
        ui,
        x,
        cy,
        "bug_report",
        report_name,
        crate::i18n::tr_label("Report a bug — review a screenshot and open a GitHub draft"),
        true,
        &["titlebar:report", "status:report"],
    );
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(REPORT_BUTTON), ui.id().with(("status", "titlebar:report"))));
    let taken = ui.ctx().data(|d| d.get_temp::<u64>(egui::Id::new(REPORT_TAKEN))) == Some(ui.ctx().cumulative_pass_nr());
    if clicked && !taken {
        crate::help::run(app, &ui.ctx().clone(), "report");
    }
    x = report.left() - 4.0;
    let (help, clicked) = icon_button(ui, x, cy, "help", app.tr("ui.help"), app.tr("ui.help"), true, &["appbar:help", "status:help"]);
    if clicked {
        app.help.menu = !app.help.menu;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sc_help_opened"), true));
    }
    help_anchor = Some(help);
    x = help.left() - 4.0;
    let prefs_tip =
        if cfg!(target_os = "macos") { format!("{} (Cmd+,)", app.tr("ui.preferences")) } else { format!("{} (Ctrl+,)", app.tr("ui.preferences")) };
    let (prefs, clicked) =
        icon_button(ui, x, cy, "settings", app.tr("ui.preferences"), &prefs_tip, true, &["titlebar:preferences", "status:preferences"]);
    crate::scenario::publish_handle("titlebar:preferences", prefs.center());
    if clicked {
        crate::prefs::open_at(app, 0);
    }
    x = prefs.left() - 4.0;
    let (home, clicked) = icon_button(
        ui,
        x,
        cy,
        "home",
        app.tr("ui.start_page"),
        app.tr("ui.start_page"),
        idle && design,
        &["titlebar:home", "appbar:home", "status:home"],
    );
    // Compatibility for scenarios that used the former Home tab.
    crate::scenario::publish_target_as("tab:home", "titlebar:home", home);
    if clicked {
        app.home.show(app.session.revision);
    }
    x = home.left() - 4.0;
    if design && crate::agent_chat::app_bar_button_shown(app) {
        let at = Rect::from_min_size(pos2(x - 32.0, cy - 16.0), vec2(32.0, 32.0));
        crate::toolbar::assistant_button(app, ui, at);
    }
    // The Help menu opens above its button.
    if let Some(help) = help_anchor {
        crate::help::show(app, &ui.ctx().clone(), pos2(help.left() - 160.0, help.top() - 8.0 - 6.0 * 30.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Harness, handle_at, rect_of};
    use serde_json::json;

    /// The bar's left says what is selected; the right keeps the version clear of Report a bug
    /// (they overlapped at the top right before) and ends with the one credit.
    #[test]
    fn status_bar_summarises_the_selection_and_keeps_report_clear_of_the_version() {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Block"})).unwrap();
        h.app.run("select.clear", json!({})).unwrap();
        h.frames(3);
        assert_eq!(selection_summary(&h.app), "Nothing selected");
        h.app.run("select.set", json!({"items": [{"type": "body", "name": "Block"}]})).unwrap();
        assert_eq!(selection_summary(&h.app), "1 body selected", "a count of one reads in the singular");
        {
            let _ja = crate::i18n::scope("ja");
            assert_eq!(selection_summary(&h.app), "ボディ 1 個選択中");
        }
        {
            let _zh = crate::i18n::scope("zh");
            assert_eq!(selection_summary(&h.app), "已选择 1 个实体");
        }
        h.frames(2);
        let bar = rect_of("status:bar").unwrap();
        assert_eq!(bar.height(), HEIGHT);
        assert_eq!(bar.bottom(), h.ctx.content_rect().bottom());
        let report = rect_of("titlebar:report").unwrap();
        let version = rect_of("titlebar:version").unwrap();
        let credit = rect_of("status:credit").unwrap();
        assert!(bar.contains_rect(report) && bar.contains_rect(version) && bar.contains_rect(credit));
        assert!(!report.intersects(version), "Report a bug and the version no longer overlap: {report:?} {version:?}");
        assert!(report.right() < version.left() && version.right() < credit.left());
        assert_eq!(credit.right(), bar.right() - INSET);
        for name in ["titlebar:home", "titlebar:preferences", "appbar:help"] {
            let r = rect_of(name).unwrap();
            assert!(bar.contains_rect(r), "{name} sits in the status bar");
            assert!(r.right() <= report.left(), "{name} is left of Report a bug");
        }
        assert!(h.widget("Polykrom by WCKD").is_none(), "the credit is text, not a control");
        assert!(h.widget("Report a bug").is_some());
        h.step(&json!({"click": {"handle": "appbar:help"}})).unwrap();
        assert!(h.app.help.menu, "? opens the Help menu");
        h.step(&json!({"key": "Escape"})).unwrap();
        h.step(&json!({"click": {"handle": "titlebar:preferences"}})).unwrap();
        assert!(h.app.prefs_window.open);
        h.step(&json!({"key": "Escape"})).unwrap();
        h.step(&json!({"click": {"handle": "status:units"}})).unwrap();
        assert!(h.app.viewport.context_menu.is_some(), "the units open the Change Design Units menu");
        h.step(&json!({"key": "Escape"})).unwrap();
        assert!(handle_at("titlebar:home").is_some());
        h.step(&json!({"click": {"handle": "titlebar:home"}})).unwrap();
        assert!(h.app.home.open, "the house button shows the start page");
    }
}

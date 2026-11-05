//! Above the viewport (design kit view 01): the open documents as tabs (with close and new) on
//! the window's base surface, and under them the view bar with the view orientation, the display
//! style and the selection filter, each a drop-down over the existing view settings.
use egui::{FontId, Rect, Sense, pos2, vec2};

use crate::{SolveApp, theme::Tokens, ui_kit::Container, ui_kit::Surface};

/// The document tabs (the kit's 36 px) and the view bar (32 px).
pub const TABS_HEIGHT: f32 = 36.0;
pub const VIEW_BAR_HEIGHT: f32 = 32.0;
pub const HEIGHT: f32 = TABS_HEIGHT + VIEW_BAR_HEIGHT;
/// From the strip's edges to the first tab and the first drop-down.
const INSET: f32 = 16.0;
/// Between the view bar's drop-downs (the kit's 24 px).
const GAP: f32 = 24.0;

/// The strip's place at the top of the viewport's content area: the tabs and the view bar, or
/// on a short view the tabs alone (the view settings stay on the VIEW tab and the view menu),
/// or None when the area is too short for even the tabs to leave a usable viewport.
pub(crate) fn strip(content: Rect) -> Option<Rect> {
    if content.width() < 200.0 {
        return None;
    }
    let height = if content.height() >= HEIGHT + MIN_SCENE_HEIGHT {
        HEIGHT
    } else if content.height() >= TABS_HEIGHT + MIN_SCENE_HEIGHT {
        TABS_HEIGHT
    } else {
        return None;
    };
    Some(Rect::from_min_size(content.min, vec2(content.width(), height)))
}

/// The scene keeps at least this much height under the strip; a shorter content area (a small
/// window) drops the view bar, then the tabs, so the model keeps its room.
const MIN_SCENE_HEIGHT: f32 = 300.0;

/// The standard views the orientation drop-down offers: (message id, `animate_view` name).
const ORIENTATIONS: [(&str, &str); 9] = [
    ("ui.view_home", "home"),
    ("ui.view_isometric", "iso"),
    ("ui.view_top", "top"),
    ("ui.view_bottom", "bottom"),
    ("ui.view_front", "front"),
    ("ui.view_back", "back"),
    ("ui.view_left", "left"),
    ("ui.view_right", "right"),
    ("ui.view_fit", "fit"),
];

/// The display styles, by `UiState::visual_style`.
const STYLES: [&str; 4] = ["ui.view_style_0", "ui.view_style_1", "ui.view_style_2", "ui.view_style_3"];

/// The name of the view the camera looks from (a standard view), or the orientation label.
fn orientation_label(app: &SolveApp) -> &'static str {
    use polykrom_engine::render::StandardView;
    let view = [
        (StandardView::Iso, "ui.view_isometric"),
        (StandardView::Top, "ui.view_top"),
        (StandardView::Bottom, "ui.view_bottom"),
        (StandardView::Front, "ui.view_front"),
        (StandardView::Back, "ui.view_back"),
        (StandardView::Left, "ui.view_left"),
        (StandardView::Right, "ui.view_right"),
    ]
    .into_iter()
    .find(|(view, _)| {
        let (yaw, pitch) = view.angles();
        let wrap = |a: f64| (a + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;
        wrap(app.cam.yaw - yaw).abs() < 1e-3 && (app.cam.pitch - pitch).abs() < 1e-3
    })
    .map(|(_, id)| id);
    crate::i18n::tr(view.unwrap_or("ui.view_orientation"))
}

/// The selection filter in a word: everything, bodies (a face picks its body), or filtered.
fn selection_label(app: &SolveApp) -> String {
    let filter = if app.ui.pick_bodies {
        "ui.view_selection_bodies"
    } else if app.ui.pick_off.is_empty() {
        "ui.view_selection_all"
    } else {
        "ui.view_selection_filtered"
    };
    crate::i18n::format("ui.view_selection", &[("filter", crate::i18n::tr(filter))])
}

/// A drop-down chip of the view bar: its text and a chevron; returns its response.
fn chip(ui: &mut egui::Ui, left: f32, cy: f32, label: &str, active: bool, handle: &str) -> egui::Response {
    let t = Tokens::get();
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(t.font_small), t.text);
    let rect = Rect::from_min_size(pos2(left, cy - 11.0), vec2(galley.size().x + 16.0 + 6.0, 22.0));
    let resp = ui.interact(rect, ui.id().with(("view_chip", handle)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    crate::scenario::publish_target(handle, rect);
    if resp.hovered() || resp.has_focus() {
        ui.painter().rect_filled(rect, t.radius_sm, t.hover);
    }
    let ink = if active { t.text } else { t.text_dim };
    ui.painter().galley(pos2(rect.left() + 4.0, cy - galley.size().y / 2.0), galley, ink);
    crate::ui_kit::dropdown_indicator(ui.painter(), Rect::from_center_size(pos2(rect.right() - 9.0, cy + 0.5), vec2(10.0, 10.0)), ink);
    resp
}

/// The strip at `strip` (from [`strip`]): the document tabs, then the view bar.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui, strip: Rect) {
    let t = Tokens::get();
    let tabs = Rect::from_min_size(strip.min, vec2(strip.width(), TABS_HEIGHT.min(strip.height())));
    let view = Rect::from_min_max(tabs.left_bottom(), strip.max);
    // The strip owns its clicks: the viewport beneath never gets them.
    ui.interact(strip, ui.id().with("document_bar"), Sense::click_and_drag());
    ui.painter().rect_filled(tabs, 0.0, t.window);
    crate::scenario::publish_rect("documents:strip", tabs);
    let mut tabs_ui = ui.new_child(egui::UiBuilder::new().max_rect(tabs).id_salt("document_tabs"));
    tabs_ui.set_clip_rect(tabs.intersect(ui.clip_rect()));
    crate::documents::tabs(app, &mut tabs_ui, tabs, tabs.left() + INSET, tabs.right() - INSET);
    // A short view keeps the tabs alone.
    if view.height() >= VIEW_BAR_HEIGHT {
        ui.painter().rect_filled(view, 0.0, t.toolbar);
        crate::scenario::publish_rect("view:bar", view);
        view_bar(app, ui, view);
    }
}

fn view_bar(app: &mut SolveApp, ui: &mut egui::Ui, r: Rect) {
    let cy = r.center().y;
    let mut x = r.left() + INSET - 4.0;
    // View orientation.
    let orientation = chip(ui, x, cy, orientation_label(app), true, "view:orientation");
    let mut pick: Option<&'static str> = None;
    Container::new(ui.ctx(), Surface::Popup).menu(&orientation).show(|ui| {
        for (id, name) in ORIENTATIONS {
            let resp = crate::context_menu::button(ui, crate::i18n::tr(id), "");
            crate::scenario::publish_handle(&format!("view:orientation:{name}"), resp.rect.center());
            if resp.clicked() {
                pick = Some(name);
                ui.close();
            }
        }
    });
    if let Some(name) = pick {
        app.animate_view(name);
    }
    x = orientation.rect.right() + GAP;
    // Display style.
    let style_label = crate::i18n::tr(STYLES.get(usize::from(app.ui.visual_style)).copied().unwrap_or(STYLES[0]));
    let style = chip(ui, x, cy, style_label, false, "view:style");
    let mut picked_style: Option<u8> = None;
    Container::new(ui.ctx(), Surface::Popup).menu(&style).show(|ui| {
        for (i, id) in STYLES.iter().enumerate() {
            let resp = crate::ui_kit::list_picker(ui, &format!("view:style:{i}"), crate::i18n::tr(id), usize::from(app.ui.visual_style) == i);
            crate::scenario::publish_handle(&format!("view:style:{i}"), resp.rect.center());
            if resp.clicked() {
                picked_style = u8::try_from(i).ok();
                ui.close();
            }
        }
    });
    if let Some(s) = picked_style {
        app.ui.visual_style = s;
    }
    x = style.rect.right() + GAP;
    // Selection filter: the SELECT panel's checkboxes.
    let selection = chip(ui, x, cy, &selection_label(app), false, "view:selection");
    Container::new(ui.ctx(), Surface::Popup).menu(&selection).show(|ui| {
        ui.set_min_width(220.0);
        crate::toolbar::selection_filter(app, ui);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Harness, handle_at, rect_of};
    use serde_json::json;

    /// The strip sits at the top of the viewport's content, right of the browser, in both
    /// layouts; its tabs are the open documents and its drop-downs drive the existing view
    /// settings (orientation, display style, selection filter).
    #[test]
    fn document_tabs_and_view_bar_sit_above_the_viewport_and_drive_the_view_settings() {
        for layout in ["floating", "docked"] {
            let mut h = Harness::new();
            h.app.home.open = false;
            h.call("ui.set", json!({"layout": layout}));
            h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10})).unwrap();
            h.frames(3);
            let strip = rect_of("documents:strip").unwrap();
            let bar = rect_of("view:bar").unwrap();
            let browser = rect_of("browser:card").unwrap();
            let toolbar = rect_of("toolbar").unwrap();
            assert_eq!(strip.height(), TABS_HEIGHT, "{layout}");
            assert_eq!(bar.height(), VIEW_BAR_HEIGHT, "{layout}");
            assert_eq!(bar.top(), strip.bottom(), "{layout}");
            assert!(strip.top() >= toolbar.bottom(), "{layout}: below the ribbon");
            assert!(strip.left() >= browser.right(), "{layout}: right of the browser");
            assert!(h.app.shell.content.unwrap().top() >= bar.bottom(), "{layout}: the viewport's content starts below the bar");
            let tab = rect_of("tab:design:0").unwrap();
            assert!(strip.contains_rect(tab), "{layout}: the document tab is in the strip");
            assert!(strip.contains(handle_at("tab:new").unwrap()), "{layout}: the + is in the strip");
            // Orientation.
            h.step(&json!({"click": {"handle": "view:orientation"}})).unwrap();
            h.step(&json!({"click": {"handle": "view:orientation:top"}})).unwrap();
            h.settle();
            assert_eq!(orientation_label(&h.app), "Top", "{layout}");
            // Display style.
            h.step(&json!({"click": {"handle": "view:style"}})).unwrap();
            h.step(&json!({"click": {"handle": "view:style:2"}})).unwrap();
            assert_eq!(h.app.ui.visual_style, 2, "{layout}");
            h.frames(2);
            assert!(h.widget("Wireframe").is_some(), "{layout}: the chip says the style");
            // Selection filter: the SELECT panel's checkboxes.
            h.step(&json!({"click": {"handle": "view:selection"}})).unwrap();
            h.step(&json!({"widget": "Edges"})).unwrap();
            assert!(h.app.ui.pick_off.contains(&"edges".to_string()), "{layout}");
            h.step(&json!({"key": "Escape"})).unwrap();
            h.frames(2);
            assert!(h.widget("Selection: filtered").is_some(), "{layout}");
        }
    }
}

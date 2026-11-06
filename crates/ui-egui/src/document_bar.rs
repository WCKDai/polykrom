//! The open documents as tabs (with close and new) and the view bar: the view orientation, the
//! display style and the selection filter, each a drop-down over the existing view settings.
//!
//! Floating (the default layout), the two are ONE slim glass strip floating at the top of the
//! canvas, right of the Design Browser card and clear of the view cube ([`floating_strip`],
//! [`show_floating`]): the canvas keeps its full width and height under it. Docked, they are two
//! bands above the viewport on the window's base surface, as design kit view 01 draws them
//! ([`strip`], [`show`]).
use egui::{FontId, Rect, Sense, pos2, vec2};

use crate::{SolveApp, theme::Tokens, ui_kit::Container, ui_kit::Surface};

/// The docked document tabs (the kit's 36 px) and view bar (32 px).
pub const TABS_HEIGHT: f32 = 36.0;
pub const VIEW_BAR_HEIGHT: f32 = 32.0;
pub const HEIGHT: f32 = TABS_HEIGHT + VIEW_BAR_HEIGHT;
/// The floating strip: one slim row for the tabs and the view chips.
pub const STRIP_HEIGHT: f32 = 32.0;
/// From the docked strip's edges to the first tab and the first drop-down.
const INSET: f32 = 16.0;
/// Between the docked view bar's drop-downs (the kit's 24 px), and between the floating strip's.
const GAP: f32 = 24.0;
const FLOATING_GAP: f32 = 12.0;
/// Inside the floating card: from its edges to the first tab and after the last chip.
const CARD_INSET: f32 = 8.0;
/// On the floating strip, between the tabs' + and the first chip.
const CHIPS_GAP: f32 = 16.0;

/// The docked strip's place at the top of the viewport's content area: the tabs and the view bar,
/// or on a short view the tabs alone (the view settings stay on the VIEW tab and the view menu).
/// None only when the area is too small for even the tabs to leave a usable scene (a window
/// below the app's minimum size); the title bar's document name then lists the open designs
/// instead (`toolbar::app_bar`), so they stay reachable.
pub(crate) fn strip(content: Rect) -> Option<Rect> {
    if content.width() < 200.0 || content.height() < TABS_HEIGHT + MIN_SCENE_UNDER_TABS {
        return None;
    }
    let height = if content.height() >= HEIGHT + MIN_SCENE_HEIGHT { HEIGHT } else { TABS_HEIGHT };
    Some(Rect::from_min_size(content.min, vec2(content.width(), height)))
}

/// The scene keeps at least this much height under the full docked strip; a shorter content area
/// (a small window) drops the view bar, so the model keeps its room.
const MIN_SCENE_HEIGHT: f32 = 300.0;
/// The tabs themselves stay down to this much scene: at the app's minimum window size (960 x
/// 600) the docked layout's content area is about 320 pt high, so the open documents never
/// leave the window in a supported size.
const MIN_SCENE_UNDER_TABS: f32 = 120.0;

/// The floating strip's card: at the top of `content` (the canvas clear of the browser card, a
/// floating Assistant and the History card), inset from the canvas's left edge, as wide as the
/// tabs and the chips need and no wider than the room before the view cube. None where the
/// content is too small for the tabs to leave a usable scene (a window below the app's minimum
/// size); the title bar's document name then lists the open designs (`toolbar::app_bar`).
pub(crate) fn floating_strip(app: &SolveApp, ui: &egui::Ui, content: Rect) -> Option<Rect> {
    if !content.is_finite() || content.width() < 200.0 || content.height() < STRIP_HEIGHT + MIN_SCENE_UNDER_TABS {
        return None;
    }
    let gap = crate::shell::SHELL_GAP;
    // The content already stands a gap right of the browser card; at the canvas's own edge the
    // card is inset by the gap.
    let left = content.left().max(app.viewport.rect.map_or(content.left(), |v| v.left()) + gap);
    let mut right = content.right() - gap;
    if let Some(cube) = app.viewport.layout.as_ref().map(|l| l.axis_view).filter(|cube| cube.is_positive()) {
        right = right.min(cube.left() - gap);
    }
    let room = right - left;
    if room < 160.0 {
        return None;
    }
    let tabs = crate::documents::natural_width(app, ui.painter());
    let natural = CARD_INSET + tabs + CHIPS_GAP + chips_width(app, ui, FLOATING_GAP) + CARD_INSET;
    Some(Rect::from_min_size(pos2(left, content.top()), vec2(natural.min(room), STRIP_HEIGHT)))
}

/// The floating strip at `strip` (from [`floating_strip`]): a glass card holding the tabs and,
/// where the room allows, the view chips to their right (a narrow strip keeps the tabs alone; the
/// view settings stay on the VIEW tab and the view menu).
pub(crate) fn show_floating(app: &mut SolveApp, ui: &mut egui::Ui, strip: Rect) {
    let t = Tokens::get();
    let frame = Container::new(ui.ctx(), Surface::Panel).glass_frame(ui.ctx(), t.panel, crate::frame::Side::Left);
    Container::paint_frame(ui.painter(), frame, strip);
    // The card owns its clicks: the viewport beneath never gets them.
    ui.interact(strip, ui.id().with("document_strip"), Sense::CLICK | Sense::DRAG);
    crate::scenario::publish_rect("document_bar:card", strip);
    let inner = strip.shrink2(vec2(CARD_INSET, 0.0));
    let tabs_width = crate::documents::natural_width(app, ui.painter());
    let chips_width = chips_width(app, ui, FLOATING_GAP);
    let chips = inner.width() >= 90.0 + 34.0 + CHIPS_GAP + chips_width;
    let tabs_right = if chips { (inner.left() + tabs_width).min(inner.right() - CHIPS_GAP - chips_width) } else { inner.right() };
    let tabs = Rect::from_min_max(inner.left_top(), pos2(tabs_right, inner.bottom()));
    crate::scenario::publish_rect("documents:strip", tabs);
    let mut tabs_ui = ui.new_child(egui::UiBuilder::new().max_rect(tabs).id_salt("document_tabs"));
    tabs_ui.set_clip_rect(tabs.intersect(ui.clip_rect()));
    crate::documents::tabs(app, &mut tabs_ui, tabs, tabs.left(), tabs.right());
    if chips {
        let bar = Rect::from_min_max(pos2(tabs.right() + CHIPS_GAP, inner.top()), inner.right_bottom());
        crate::scenario::publish_rect("view:bar", bar);
        view_chips(app, ui, bar.left(), bar.center().y, FLOATING_GAP);
    }
}

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

/// The display style's label.
fn style_label(app: &SolveApp) -> &'static str {
    crate::i18n::tr(STYLES.get(usize::from(app.ui.visual_style)).copied().unwrap_or(STYLES[0]))
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

/// A chip's width for its label: the text, its padding and the chevron.
fn chip_width(ui: &egui::Ui, label: &str) -> f32 {
    let t = Tokens::get();
    ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(t.font_small), t.text).size().x + 16.0 + 6.0
}

/// The three chips side by side, `gap` apart.
fn chips_width(app: &SolveApp, ui: &egui::Ui, gap: f32) -> f32 {
    chip_width(ui, orientation_label(app)) + chip_width(ui, style_label(app)) + chip_width(ui, &selection_label(app)) + 2.0 * gap
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

/// The docked strip at `strip` (from [`strip`]): the document tabs, then the view bar.
pub fn show(app: &mut SolveApp, ui: &mut egui::Ui, strip: Rect) {
    let t = Tokens::get();
    let tabs = Rect::from_min_size(strip.min, vec2(strip.width(), TABS_HEIGHT.min(strip.height())));
    let view = Rect::from_min_max(tabs.left_bottom(), strip.max);
    // The strip owns its clicks, but yields its bottom edge to the scene below it so a model
    // point landing exactly on the scene's top row stays pickable (it abuts the strip there).
    let block = Rect::from_min_max(strip.min, pos2(strip.max.x, (strip.max.y - 1.0).max(strip.min.y)));
    ui.interact(block, ui.id().with("document_bar"), Sense::CLICK | Sense::DRAG);
    ui.painter().rect_filled(tabs, 0.0, t.window);
    crate::scenario::publish_rect("documents:strip", tabs);
    let mut tabs_ui = ui.new_child(egui::UiBuilder::new().max_rect(tabs).id_salt("document_tabs"));
    tabs_ui.set_clip_rect(tabs.intersect(ui.clip_rect()));
    crate::documents::tabs(app, &mut tabs_ui, tabs, tabs.left() + INSET, tabs.right() - INSET);
    // A short view keeps the tabs alone.
    if view.height() >= VIEW_BAR_HEIGHT {
        ui.painter().rect_filled(view, 0.0, t.toolbar);
        crate::scenario::publish_rect("view:bar", view);
        view_chips(app, ui, view.left() + INSET - 4.0, view.center().y, GAP);
    }
}

/// The view chips from `x` on, centred on `cy`, `gap` apart: the orientation, the display style
/// and the selection filter, each a drop-down over the existing view settings.
fn view_chips(app: &mut SolveApp, ui: &mut egui::Ui, mut x: f32, cy: f32, gap: f32) {
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
    x = orientation.rect.right() + gap;
    // Display style.
    let style = chip(ui, x, cy, style_label(app), false, "view:style");
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
    x = style.rect.right() + gap;
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
    /// layouts: floating, one slim glass card holding the tabs and the view chips side by side
    /// (the canvas keeps its full width and height under it); docked, the two bands above the
    /// viewport. Its tabs are the open documents and its drop-downs drive the existing view
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
            if layout == "floating" {
                // One slim card at the top of the canvas, right of the browser card and clear of
                // the view cube; the tabs then the chips on the one row.
                let card = rect_of("document_bar:card").unwrap();
                let cube = rect_of("axis_view:outer").unwrap();
                let scene = h.app.viewport.rect.unwrap();
                assert_eq!(card.height(), STRIP_HEIGHT, "{layout}");
                assert!(card.contains_rect(strip) && card.contains_rect(bar), "{layout}: tabs and chips on the one card");
                assert_eq!(strip.top(), bar.top(), "{layout}: one row");
                assert!(bar.left() >= strip.right(), "{layout}: the chips follow the tabs");
                assert!((card.left() - (browser.right() + crate::shell::SHELL_GAP)).abs() < 1.0, "{layout}: a gap right of the browser card");
                assert!((card.top() - scene.top()).abs() < 1.0, "{layout}: at the top of the canvas");
                assert!(card.right() + crate::shell::SHELL_GAP <= cube.left() + 1.0, "{layout}: clear of the view cube {cube:?}: {card:?}");
                assert!(card.width() < scene.width() / 2.0, "{layout}: a strip, not a band: {card:?}");
                assert_eq!(h.app.shell.content.unwrap().top(), scene.top(), "{layout}: the strip takes no band out of the content");
                assert!(crate::shell::covered(&h.app, card.center()), "{layout}: the card shields the scene");
            } else {
                assert_eq!(strip.height(), TABS_HEIGHT, "{layout}");
                assert_eq!(bar.height(), VIEW_BAR_HEIGHT, "{layout}");
                assert_eq!(bar.top(), strip.bottom(), "{layout}");
                assert!(h.app.shell.content.unwrap().top() >= bar.bottom(), "{layout}: the viewport's content starts below the bar");
            }
            assert!(strip.top() >= toolbar.bottom(), "{layout}: below the ribbon");
            assert!(strip.left() >= browser.right(), "{layout}: right of the browser");
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

    /// Two designs open, in both layouts, at every window height from the app's minimum (600)
    /// up: the tabs stay on the strip and a click switches the design. Below the minimum, where
    /// even the tabs would leave no scene, the title bar's document name lists the designs.
    #[test]
    fn document_tabs_stay_reachable_at_every_window_height() {
        for layout in ["docked", "floating"] {
            for height in [600, 620, 650, 660, 700, 800, 1000] {
                let mut h = Harness::new();
                h.app.home.open = false;
                h.call("ui.set", json!({"layout": layout}));
                h.call("ui.resize", json!({"width": 1366, "height": height}));
                h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10})).unwrap();
                crate::documents::open_tab(&mut h.app, polykrom_engine::Session::default()).unwrap();
                crate::documents::switch(&mut h.app, 0);
                h.frames(3);
                let strip = rect_of("documents:strip").unwrap_or_else(|| panic!("{layout} at {height}: the strip is drawn"));
                let content = h.ctx.content_rect();
                assert!(strip.bottom() <= content.bottom() - crate::status_bar::HEIGHT, "{layout} at {height}: the strip is above the status bar");
                let tab = rect_of("tab:design:1").unwrap_or_else(|| panic!("{layout} at {height}: the second tab is drawn"));
                assert!(strip.contains_rect(tab), "{layout} at {height}: the tab is on the strip");
                assert!(rect_of("appbar:documents").is_none(), "{layout} at {height}: no fallback while the tabs show");
                h.step(&json!({"click": {"handle": "tab:design:1"}})).unwrap();
                assert_eq!(h.app.docs.active, 1, "{layout} at {height}: the click switched the design");
                // The scene keeps room under the strip.
                let scene = h.app.shell.content.unwrap();
                assert!(scene.height() >= MIN_SCENE_UNDER_TABS, "{layout} at {height}: scene {scene:?}");
            }
            // Below the supported minimum the strip has no room: the title bar's document name
            // drops the list of designs.
            let mut h = Harness::new();
            h.app.home.open = false;
            h.call("ui.set", json!({"layout": layout}));
            h.call("ui.resize", json!({"width": 1366, "height": 240}));
            h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10})).unwrap();
            crate::documents::open_tab(&mut h.app, polykrom_engine::Session::default()).unwrap();
            crate::documents::switch(&mut h.app, 0);
            h.frames(3);
            assert!(rect_of("documents:strip").is_none(), "{layout}: no room for the strip at 240");
            assert!(rect_of("appbar:documents").is_some(), "{layout}: the document name lists the designs");
            assert!(h.widget("All open designs").is_some(), "{layout}: the fallback has an accessible name");
            h.step(&json!({"click": {"handle": "appbar:documents"}})).unwrap();
            h.step(&json!({"click": {"handle": "document:1"}})).unwrap();
            assert_eq!(h.app.docs.active, 1, "{layout}: the list switched the design");
            h.frames(2);
            assert!(handle_at("document:1").is_none(), "{layout}: the list closed");
        }
    }
}

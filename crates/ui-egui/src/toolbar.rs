//! Application bar, the workspace toolbar (tabs and panels) and keyboard shortcuts.

use crate::fonts::RichTextExt as _;
use crate::icons::glyphs::pin_icon;
use crate::ui_kit::{Container, Edge, Surface};
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};
use serde_json::json;

use crate::theme::Tokens;
use crate::{SolveApp, icons};

pub use crate::workspace::TABS;

/// Legacy paint arguments and non-SVG affordances follow the same semantic palette as the icon.
/// The requested icon id determines its meaning on ribbons, menus and drag previews alike.
fn command_accent(icon: &str) -> Color32 {
    crate::icon_svg::primary(icon, crate::theme::is_dark(), crate::theme::vision())
}

/// Panels shown on each tab (in order). Commands come from the registry by (tab, panel).
pub fn panels(tab: &str) -> Vec<&'static str> {
    crate::workspace::panels(tab)
}

/// How a panel is drawn on a toolbar that may be too narrow for every button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Its first buttons (all of them, or the ones that fit); the panel's drop-down lists every
    /// command.
    Buttons,
    /// One button that opens the panel's drop-down (which lists every command, pinned or not).
    Collapsed,
    /// In the » menu at the end of the toolbar.
    More,
}

/// A ribbon button's preferred and minimum widths. The constraint gallery is a fixed-width
/// block; ordinary buttons narrow as their labels wrap and truncate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonSize {
    pub width: f32,
    pub min_width: f32,
}

/// One panel as [`fit`] sees it, in button order: the pinned buttons and the gallery, then the
/// secondary small-button columns from `secondary_from` on (`usize::MAX`: none), which give way
/// before any pinned button on any panel does.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelSize {
    pub buttons: Vec<ButtonSize>,
    pub secondary_from: usize,
}

/// Where [`fit`] put a panel: its first buttons and their individual fitted widths.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub fit: Fit,
    pub shown: usize,
    pub widths: Vec<f32>,
}

impl Plan {
    fn full(panel: &PanelSize) -> Self {
        Self { fit: Fit::Buttons, shown: panel.buttons.len(), widths: panel.buttons.iter().map(|b| b.width).collect() }
    }

    fn more() -> Self {
        Self { fit: Fit::More, shown: 0, widths: Vec::new() }
    }

    /// The panel's width on the toolbar (0 in the » menu).
    pub fn width(&self) -> f32 {
        match self.fit {
            Fit::More => 0.0,
            Fit::Collapsed => COLLAPSED_WIDTH,
            Fit::Buttons => panel_width(self.widths.get(..self.shown).unwrap_or_default()),
        }
    }
}

/// A panel's width with these individual button widths.
pub fn panel_width(buttons: &[f32]) -> f32 {
    (buttons.iter().sum::<f32>() + buttons.len().saturating_sub(1) as f32 * BUTTON_GAP + 8.0).max(COLLAPSED_WIDTH)
}

/// A collapsed panel's width: one button's panel.
pub const COLLAPSED_WIDTH: f32 = 72.0;

/// The gap after each panel (its separator).
const PANEL_GAP: f32 = 1.0;
/// The workspace tab row (the kit's 34 px).
pub const TAB_ROW_HEIGHT: f32 = 34.0;
/// Distance from the ribbon card's edges to a group's label.
const RIBBON_TAB_INSET: f32 = 4.0;
/// A ribbon group (the kit's 106 px): the buttons, then the group's name along the bottom.
pub const GROUP_HEIGHT: f32 = 106.0;
pub const RIBBON_HEIGHT: f32 = TAB_ROW_HEIGHT + GROUP_HEIGHT + 1.0;
/// A large icon-over-label button (the kit's 72 px), and where its icon and label sit in it.
const BUTTON_HEIGHT: f32 = 72.0;
const LARGE_ICON: f32 = 28.0;
const LARGE_ICON_Y: f32 = 18.0;
const LARGE_LABEL_Y: f32 = 47.0;
/// From a group's top to its buttons.
const GROUP_PAD_TOP: f32 = 6.0;
const ICON_BUTTON_HEIGHT: f32 = 36.0;
const ICON_GROUP_HEIGHT: f32 = 62.0;
/// A small icon-beside-label button (the kit's 32 px): two rows of them make a column.
const SMALL_HEIGHT: f32 = 32.0;
const SMALL_ROW_GAP: f32 = 4.0;
const SMALL_ROWS: usize = 2;
/// A group shows at most this many small-button columns; the rest stay in its drop-down.
const SMALL_MAX_COLUMNS: usize = 3;
const SMALL_ICON: f32 = 20.0;
const SMALL_PAD: f32 = 6.0;
/// Where a small button's label starts: the padding, the icon and the kit's 6 px gap.
const SMALL_TEXT_X: f32 = SMALL_PAD + SMALL_ICON + 6.0;
const SMALL_MIN_WIDTH: f32 = 76.0;
const SMALL_MAX_WIDTH: f32 = 184.0;

/// The same height drives shell reservations and ribbon painting in either layout.
pub(crate) fn ribbon_height(labels: bool) -> f32 {
    if labels { RIBBON_HEIGHT } else { TAB_ROW_HEIGHT + ICON_GROUP_HEIGHT + 1.0 }
}

/// Whether the ribbon shows its labels: the preference, except in a window too short for the
/// labelled groups to leave the model its room (the icon-only ribbon takes 44 points less).
pub(crate) fn labels_shown(app: &SolveApp, ctx: &egui::Context) -> bool {
    app.preferences.ribbon_labels && ctx.content_rect().height() >= SHORT_WINDOW_HEIGHT
}

/// Below this window height the ribbon drops its labels.
const SHORT_WINDOW_HEIGHT: f32 = 480.0;
const BUTTON_GAP: f32 = 4.0;
const BUTTON_MIN_WIDTH: f32 = 54.0;
/// A large button is as wide as its one-line label needs, up to this; only a longer label wraps
/// (the kit's labels are one line each).
const BUTTON_MAX_WIDTH: f32 = 100.0;
const BUTTON_LABEL_PADDING: f32 = 12.0;
const SECTION_HEIGHT: f32 = 20.0;

/// The » button's room at the end of the panels, kept only while some panel is in it.
pub const MORE_ROOM: f32 = 40.0;

/// Fit panels into `room`, giving up as little as it takes, one small step at a time:
/// 1. buttons narrow from their measured label widths to their minimum, wrapping and truncating;
/// 2. panels hand their last buttons to their drop-down one at a time, the panel showing the
///    most buttons first (the rightmost of equals), down to one button (collapsed);
/// 3. the last panels go into the » menu, and any room that frees goes back to buttons.
///
/// Each removal frees at most one button (or the fixed-width constraint gallery), so the toolbar
/// never leaves a larger gap while something is hidden. The result depends only on `room`
/// (no state from earlier frames), so it never flickers. Nothing is dropped.
pub fn fit(panels: &[PanelSize], room: f32) -> Vec<Plan> {
    let room = if room.is_finite() { room } else { 0.0 };
    if let Some(plans) = shrink(panels, room) {
        return plans;
    }
    // 3. The last panels into the » menu (which takes its own room), as few as it takes; the
    // ones left keep as many buttons as fit.
    for kept in (0..panels.len()).rev() {
        if let Some(mut plans) = shrink(panels.get(..kept).unwrap_or_default(), room - MORE_ROOM) {
            plans.extend(panels.iter().skip(kept).map(|_| Plan::more()));
            return plans;
        }
    }
    panels.iter().map(|_| Plan::more()).collect()
}

/// The room plans take on the toolbar (each panel and its separator).
fn used(plans: &[Plan]) -> f32 {
    plans.iter().filter(|p| p.fit != Fit::More).map(|p| p.width() + PANEL_GAP).sum()
}

/// Steps 1 and 2 of [`fit`]: every panel on the toolbar, or None when even one button each
/// doesn't fit.
fn shrink(panels: &[PanelSize], room: f32) -> Option<Vec<Plan>> {
    let mut plans: Vec<Plan> = panels.iter().map(Plan::full).collect();
    if used(&plans) <= room {
        return Some(plans);
    }
    // 0. The secondary columns (a group's other commands as small buttons) go into the
    // drop-downs first, from the panel showing the most of them: the pinned buttons keep their
    // one-line labels for as long as the room allows.
    while used(&plans) > room {
        let Some(i) = plans
            .iter()
            .zip(panels)
            .enumerate()
            .filter(|(_, (p, s))| p.fit == Fit::Buttons && p.shown > s.secondary_from)
            .max_by(|(i, (a, sa)), (j, (b, sb))| (a.shown - sa.secondary_from).cmp(&(b.shown - sb.secondary_from)).then(i.cmp(j)))
            .map(|(i, _)| i)
        else {
            break;
        };
        if let Some(p) = plans.get_mut(i) {
            p.shown -= 1;
        }
    }
    if used(&plans) <= room {
        return Some(plans);
    }
    // 1. Narrow together in one-pixel steps, keeping each button's measured width until it
    // reaches its minimum. The gallery's minimum equals its full width; a small-button column
    // narrows further, so step as far as the widest button can.
    let max_reduction = panels.iter().flat_map(|p| p.buttons.iter()).map(|b| (b.width - b.min_width).ceil() as usize).max().unwrap_or(0);
    for reduction in 1..=max_reduction {
        for (plan, panel) in plans.iter_mut().zip(panels) {
            for (width, button) in plan.widths.iter_mut().zip(&panel.buttons) {
                *width = (button.width - reduction as f32).max(button.min_width);
            }
        }
        if used(&plans) <= room {
            return Some(plans);
        }
    }
    // 2. One button at a time into the drop-downs: from the panel showing the most (the
    // rightmost of equals), down to one button each.
    while used(&plans) > room {
        let pick = plans
            .iter()
            .enumerate()
            .filter(|(_, p)| p.fit == Fit::Buttons && (p.shown > 1 || p.width() > COLLAPSED_WIDTH))
            .max_by(|(i, a), (j, b)| a.shown.cmp(&b.shown).then(i.cmp(j)));
        let (i, _) = pick?;
        if let Some(p) = plans.get_mut(i) {
            p.shown = p.shown.saturating_sub(1).max(1);
            if p.shown == 1 {
                p.fit = Fit::Collapsed;
            }
        }
    }
    Some(plans)
}

/// The commands a panel's drop-down (or its » submenu) lists: its own, then any button moved
/// into it from another panel.
fn menu_commands(
    cmds: &[&'static polykrom_engine::CommandSpec],
    shown: &[&'static polykrom_engine::CommandSpec],
) -> Vec<&'static polykrom_engine::CommandSpec> {
    let mut out = cmds.to_vec();
    for c in shown {
        if !out.iter().any(|o| o.id == c.id) {
            out.push(*c);
        }
    }
    out
}

/// The height of an application-bar pill ([`offer_pill`]), a little under the bar's buttons.
pub(crate) const PILL_HEIGHT: f32 = 22.0;

/// An application-bar pill for something on offer (Update) at the right of `span` (the bar's free
/// space): accent-filled, with `icon` and the first of `labels` that fits (the icon alone when
/// none does or `labeled` is false). Every label but the last also leaves `keep` points of the
/// span free (for the document identity). None when even the icon doesn't fit: it never covers
/// the bar's other buttons. `handle` names it for scenarios.
pub(crate) fn offer_pill(
    ui: &mut egui::Ui,
    span: Rect,
    labeled: bool,
    labels: &[&str],
    keep: f32,
    icon: &str,
    handle: &str,
) -> Option<(Rect, egui::Response)> {
    let t = Tokens::get();
    let font = FontId::proportional(t.font_label);
    let (icon_x, text_x, right_pad) = (16.0, 29.0, 11.0);
    let last = labels.len().saturating_sub(1);
    let label = labels.iter().enumerate().filter(|_| labeled).find_map(|(i, text)| {
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), t.app_bar_text));
        let width = text_x + galley.size().x + right_pad;
        let room = if i < last { span.width() - keep } else { span.width() };
        (width <= room).then_some((*text, width))
    });
    let width = label.map_or(PILL_HEIGHT + 4.0, |(_, w)| w);
    if width > span.width() {
        return None;
    }
    let rect = Rect::from_min_size(pos2(span.right() - width, span.center().y - PILL_HEIGHT / 2.0), vec2(width, PILL_HEIGHT));
    let response = ui.interact(rect, ui.id().with(("sc_offer_pill", handle)), Sense::click());
    let name = label.map(|(text, _)| text).or(labels.first().copied()).unwrap_or_default().to_string();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name));
    crate::scenario::publish_target(handle, rect);
    let hot = response.hovered() || response.has_focus();
    let fill = if hot { t.app_bar_hover_button } else { t.accent };
    let radius = PILL_HEIGHT / 2.0 * f32::from(t.radius_xl) / 10.0;
    ui.painter().rect_filled(rect, radius, fill);
    if response.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), Tokens::outset_radius(radius, 2.0), Stroke::new(1.0, t.app_bar_text), egui::StrokeKind::Outside);
    }
    let glyph_x = if label.is_some() { rect.left() + icon_x } else { rect.center().x };
    let glyph = Rect::from_center_size(pos2(glyph_x, rect.center().y), vec2(16.0, 16.0));
    icons::paint_on(ui.painter(), glyph, icon, Some(true), 1.0);
    if let Some((text, _)) = label {
        ui.painter().text(pos2(rect.left() + text_x, rect.center().y), Align2::LEFT_CENTER, text, font, t.app_bar_text);
    }
    Some((rect, response))
}

/// The application bar's height: the window's title bar on desktop builds, the design kit's
/// 48 px document toolbar (view 01).
pub const APP_BAR_HEIGHT: f32 = 48.0;
/// The bar's buttons (Undo, Redo, Save, Export): the kit's 30 px.
const BAR_BUTTON_HEIGHT: f32 = 30.0;
/// Between the bar's buttons.
const BAR_GAP: f32 = 6.0;
/// From the window's edge to the logo and to the last button.
const BAR_INSET: f32 = 16.0;
/// The bar's buttons drop their text (Save, Export keep their icons) below this width.
const BAR_LABELS_MIN_WIDTH: f32 = 860.0;
/// The command search field's width in a narrow bar: its icon and its key.
const SEARCH_MIN_WIDTH: f32 = 60.0;

/// One button of the bar's right-hand group: an icon, and `label` on the kit's field surface
/// when there is one. Returns whether it was clicked (never while disabled).
#[allow(clippy::too_many_arguments)]
fn bar_button(ui: &mut egui::Ui, rect: Rect, icon: &str, label: Option<&str>, tip: &str, enabled: bool, handles: &[&str], menu_open: bool) -> bool {
    let t = Tokens::get();
    let resp =
        ui.interact(rect, ui.id().with(("ab", handles.first().copied().unwrap_or(icon))), if enabled { Sense::click() } else { Sense::hover() });
    let name = label.unwrap_or(tip);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    for handle in handles {
        crate::scenario::publish_target(handle, rect);
    }
    let hot = enabled && (resp.hovered() || resp.has_focus() || menu_open);
    if label.is_some() {
        // Save and Export: the kit's filled buttons on the field surface.
        ui.painter().rect_filled(
            rect,
            t.radius_sm,
            if hot { t.app_bar_hover_strong } else { t.app_bar_tab_active.lerp_to_gamma(t.app_bar_text, 0.08) },
        );
    } else if hot {
        ui.painter().rect_filled(rect, t.radius_sm, t.app_bar_hover_button);
    }
    let opacity = if enabled { 1.0 } else { 0.35 };
    let glyph_x = if label.is_some() { rect.left() + 7.0 + 9.0 } else { rect.center().x };
    icons::paint_app_bar(ui.painter(), Rect::from_center_size(pos2(glyph_x, rect.center().y), vec2(18.0, 18.0)), icon, opacity);
    if let Some(label) = label {
        ui.painter().text(
            pos2(rect.left() + 31.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(t.font_label),
            t.app_bar_text.gamma_multiply(opacity),
        );
    }
    enabled && resp.on_hover_text(tip).clicked()
}

/// The document's subtitle in the bar: where it is stored and its revision, from the session
/// (`Local file · rev 12`; `Not saved yet · rev 3` before the first save).
pub(crate) fn document_subtitle(app: &SolveApp) -> String {
    let place = match app.session.path.as_deref() {
        Some(path) => {
            let path = std::path::Path::new(path);
            match path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).filter(|n| !n.is_empty()) {
                Some(folder) => format!("{} · {folder}", app.tr("ui.doc_local_file")),
                None => app.tr("ui.doc_local_file").to_string(),
            }
        }
        None => app.tr("ui.doc_unsaved").to_string(),
    };
    crate::i18n::format("ui.doc_subtitle", &[("place", &place), ("revision", &app.session.revision.to_string())])
}

pub fn app_bar(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let app_bar = Container::new(ui.ctx(), Surface::ApplicationBar).edge("sc_appbar", Edge::Top);
    let frame = Container::new(ui.ctx(), Surface::ApplicationBar).edge_frame(t.app_bar);
    app_bar.exact_size(APP_BAR_HEIGHT).show_separator_line(false).frame(frame).show(ui, |ui| {
        let r = ui.max_rect();
        // The bar is the window's title bar: its empty space moves the window.
        if app.custom_titlebar || app.integrated_titlebar {
            crate::titlebar::drag_area(ui, r);
        }
        // While a file loads the design waits: no menus or commands from the bar.
        let idle = !app.file_load.busy();
        let left = r.left() + if app.integrated_titlebar { crate::titlebar::TRAFFIC_LIGHTS } else { 0.0 };
        let captions = if app.custom_titlebar { crate::titlebar::WIDTH } else { 0.0 };
        let right = r.right() - captions - BAR_INSET;
        let labelled = right - left >= BAR_LABELS_MIN_WIDTH;
        let cy = r.center().y;
        // The supplied logo alone at the left (its exact vector, no ground behind it). It is also
        // the application menu's button: the start page, File, Help and Preferences live there.
        let logo = Rect::from_center_size(pos2(left + BAR_INSET + 16.0, cy), vec2(32.0, 32.0));
        let menu_enabled = idle && !app.native_file_menu;
        let logo_resp = ui.interact(logo, ui.id().with("appbar_menu"), if menu_enabled { Sense::click() } else { Sense::hover() });
        let menu_tip = app.tr("ui.app_menu");
        logo_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, menu_enabled, menu_tip));
        crate::scenario::publish_target("appbar:menu", logo);
        crate::scenario::publish_target("appbar:logo", logo);
        if !app.native_file_menu {
            // The File menu's handle keeps its old names too, for scripts written against them.
            crate::scenario::publish_target("appbar:file", logo);
            crate::scenario::publish_target_as("appbar:folder", "appbar:file", logo);
        }
        let menu_open = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("sc_file_menu")).unwrap_or(false));
        if menu_enabled && (logo_resp.hovered() || logo_resp.has_focus() || menu_open) {
            ui.painter().rect_filled(logo.expand(3.0), t.radius_sm, t.app_bar_hover_button);
        }
        icons::paint_on(ui.painter(), logo.shrink(1.0), "logo", Some(true), if idle { 1.0 } else { 0.5 });
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sc_file_menu_x"), logo.left()));
        if menu_enabled && logo_resp.on_hover_text(menu_tip).clicked() {
            app.ui.palette_open = false;
            let ctx = ui.ctx().clone();
            let pass = ctx.cumulative_pass_nr();
            // The logo toggles the menu. Remember the pass it opened on so the same click isn't
            // also treated as a click outside the menu (which would close it at once).
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("sc_file_menu"), !menu_open);
                d.insert_temp(egui::Id::new("sc_file_menu_opened"), pass);
            });
        }
        // The right-hand group, from the edge: Export, Save, command search, Redo, Undo.
        let shown = design_shown(app) && idle;
        let tip_for =
            |label: &str, key: &str| if shown { format!("{label} ({key})") } else { crate::i18n::format("ui.needs_design", &[("command", label)]) };
        let save_tip = tip_for(app.tr("ui.save"), "Ctrl+S");
        let undo_tip = tip_for(app.tr("ui.undo"), "Ctrl+Z");
        let redo_tip = tip_for(app.tr("ui.redo"), "Ctrl+Y");
        let export_tip = if shown { crate::i18n::tr("ui.export_tip").to_string() } else { tip_for(app.tr("ui.export"), "") };
        let label_font = FontId::proportional(t.font_label);
        let text_width = |ui: &egui::Ui, text: &str| ui.painter().layout_no_wrap(text.to_string(), label_font.clone(), t.app_bar_text).size().x;
        let labelled_width = |ui: &egui::Ui, text: &str| if labelled { 31.0 + text_width(ui, text) + 14.0 } else { 32.0 };
        let button_rect =
            |right: f32, width: f32| Rect::from_min_size(pos2(right - width, cy - BAR_BUTTON_HEIGHT / 2.0), vec2(width, BAR_BUTTON_HEIGHT));
        let mut x = right;
        let export_label = app.tr("ui.export").to_string();
        let export = button_rect(x, labelled_width(ui, &export_label));
        let export_open = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("sc_export_menu")).unwrap_or(false));
        if bar_button(
            ui,
            export,
            "export",
            labelled.then_some(export_label.as_str()),
            &export_tip,
            shown,
            &["titlebar:export", "appbar:export"],
            export_open,
        ) {
            let pass = ui.ctx().cumulative_pass_nr();
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("sc_export_menu"), !export_open);
                d.insert_temp(egui::Id::new("sc_export_menu_opened"), pass);
                d.insert_temp(egui::Id::new("sc_export_menu_at"), export.left_bottom());
            });
        }
        x = export.left() - BAR_GAP;
        let save_label = app.tr("ui.save").to_string();
        let save = button_rect(x, labelled_width(ui, &save_label));
        if bar_button(ui, save, "save", labelled.then_some(save_label.as_str()), &save_tip, shown, &["titlebar:save", "appbar:save"], false) {
            save_design(app);
        }
        x = save.left() - BAR_GAP;
        // Command search with its shortcut (S), drawn on the foreground by `shortcut_box::show`
        // so it can grow over the ribbon; the bar only reserves its place.
        // A narrow bar shortens the field (its icon and key stay) rather than dropping it.
        let search_width = crate::ui_kit::CommandSearch::closed_width(ui.ctx()).min((x - left - 260.0).max(SEARCH_MIN_WIDTH));
        let search = Rect::from_min_size(pos2(x - search_width, cy - 11.0), vec2(search_width, 22.0));
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sc_ribbon_search"), search));
        x = search.left() - BAR_GAP;
        // Undo and Redo use the same actions as the application menu and the shortcuts; a native
        // menu bar (macOS) carries them itself.
        if !app.native_file_menu {
            let redo = button_rect(x, 32.0);
            if bar_button(ui, redo, "redo", None, &redo_tip, shown, &["appbar:redo", "titlebar:redo"], false) {
                FileAction::Redo.run(app);
            }
            x = redo.left() - BAR_GAP;
            let undo = button_rect(x, 32.0);
            if bar_button(ui, undo, "undo", None, &undo_tip, shown, &["appbar:undo", "titlebar:undo"], false) {
                FileAction::Undo.run(app);
            }
            x = undo.left() - BAR_GAP;
        }
        let controls_left = x;
        // The document identity: its name, then where it is stored and its revision.
        let identity_left = logo.right() + 14.0;
        let (name, dirty) = crate::documents::tabs_info(app)
            .get(app.docs.active.min(crate::documents::tabs_info(app).len().saturating_sub(1)))
            .cloned()
            .unwrap_or_default();
        let shown_name = crate::i18n::Locale::for_language(&app.preferences.language).given_name(&name);
        let title = format!("{shown_name}{}", if dirty { " •" } else { "" });
        let title_galley = ui.painter().layout_no_wrap(title, crate::fonts::semibold(t.font_heading), t.app_bar_text);
        let subtitle_galley =
            ui.painter().layout_no_wrap(document_subtitle(app), FontId::proportional(t.font_small), t.app_bar_text.gamma_multiply(0.72));
        let identity_width = title_galley.size().x.max(subtitle_galley.size().x);
        let identity_right = (identity_left + identity_width).min(controls_left - 12.0);
        let identity = Rect::from_min_max(pos2(identity_left, r.top() + 6.0), pos2(identity_right.max(identity_left), r.bottom() - 6.0));
        crate::scenario::publish_rect("appbar:document", identity);
        if identity.width() > 20.0 {
            let clip = ui.painter().with_clip_rect(identity.intersect(ui.clip_rect()));
            clip.galley(pos2(identity.left(), r.top() + 7.0), title_galley, t.app_bar_text);
            clip.galley(pos2(identity.left(), r.top() + 27.0), subtitle_galley, t.app_bar_text.gamma_multiply(0.72));
        }
        // A newer release: its Update pill sits in the free space before the right-hand group.
        let span = Rect::from_min_max(pos2(identity.right() + 12.0, cy - PILL_HEIGHT / 2.0), pos2(controls_left - 6.0, cy + PILL_HEIGHT / 2.0));
        if span.width() > PILL_HEIGHT {
            crate::update::button(app, ui, span, labelled);
        }
        if app.custom_titlebar {
            crate::titlebar::caption_buttons(app, ui, r);
        }
    });
    file_menu(app, ui.ctx());
    export_menu(app, ui.ctx());
}

/// File, edit and application actions shared by the in-window and native menus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileAction {
    /// Create a design using the document lifecycle.
    New,
    /// Pick a design or an exchange file to open.
    Open,
    /// Save the active design, asking for a path when needed.
    Save,
    /// Save the active design under another name.
    SaveAs,
    /// Undo the active design's last command.
    Undo,
    /// Redo the active design's last undone command.
    Redo,
    /// Save a JSON design using the existing save-as flow.
    SaveJson,
    /// Export through the format-specific export command.
    Export(&'static str),
    /// Open the mesh export dialog with the selected format.
    ExportMesh(&'static str),
    /// Start an existing interactive command.
    Command(&'static str),
    /// Open an entry from the recent-design list.
    OpenRecent(String),
    /// Clear recent designs and their last-opened dates.
    ClearRecent,
    /// Show the application's preferences.
    Settings,
    /// Show the keyboard shortcut editor.
    Shortcuts,
    /// Toggle the agent chat panel.
    AgentChat,
    /// Request application exit, checking for unsaved documents.
    Quit,
}

impl FileAction {
    /// Whether this action is available in the current document and picker state.
    pub fn enabled(&self, app: &SolveApp) -> bool {
        let needs_design = matches!(self, Self::Save | Self::SaveAs | Self::Undo | Self::Redo | Self::ExportMesh(_) | Self::Command(_))
            || matches!(self, Self::Export(extension) if *extension != "json");
        let available = match self {
            Self::Undo => !app.session.undo.is_empty(),
            Self::Redo => !app.session.redo.is_empty(),
            Self::ClearRecent => !app.home.recent.is_empty(),
            Self::AgentChat => crate::agent_chat::app_bar_button_shown(app),
            _ => true,
        };
        !app.file_picker_busy() && !app.file_load.busy() && (!needs_design || design_shown(app)) && available
    }

    /// Execute through the same document, save-job and preferences paths as the toolbar.
    pub fn run(&self, app: &mut SolveApp) {
        if !self.enabled(app) {
            return;
        }
        match self {
            Self::New => crate::documents::new_design(app),
            Self::Open => app.pick_open(|app, path| {
                if let Some(path) = path {
                    app.open_path_async(&path);
                }
            }),
            Self::Save => save(app),
            Self::SaveAs => save_as(app),
            Self::Undo => drop(app.run("edit.undo", json!({}))),
            Self::Redo => drop(app.run("edit.redo", json!({}))),
            Self::SaveJson => save_as_with_extension(app, "json"),
            Self::Export(extension) => export(app, extension),
            Self::ExportMesh(format) => crate::dialogs_save_mesh::start_with_format(app, format),
            Self::Command(command) => app.start(command),
            Self::OpenRecent(path) => crate::home::open_recent(app, path),
            Self::ClearRecent => {
                app.home.recent.clear();
                app.home.opened_at.clear();
            }
            Self::Settings => crate::prefs::open_at(app, 0),
            Self::Shortcuts => {
                app.keymap.open = true;
                app.keymap.read_only = false;
            }
            Self::AgentChat => crate::agent_chat::toggle(app),
            Self::Quit => crate::documents::request_quit(app),
        }
    }
}

/// The bar's Save button: the same flow as Ctrl+S and the application menu.
fn save_design(app: &mut SolveApp) {
    save(app);
}

/// One menu row with a shortcut, an icon and an enabled flag.
fn menu_item(ui: &mut egui::Ui, label: &str, key: &str, icon: &str, enabled: bool) -> bool {
    crate::context_menu::list_row(ui, &crate::context_menu::Item::action(label, label, icon).key(key).on_if(enabled)).is_some()
}

/// The export rows shared by the application menu and the bar's Export button: every format
/// Polykrom writes, each through its existing command or dialog.
fn export_rows(app: &mut SolveApp, ui: &mut egui::Ui, close: &mut bool) {
    let shown = design_shown(app);
    if menu_item(ui, "Export STL…", "", "mesh_export", shown) {
        FileAction::ExportMesh("stl").run(app);
        *close = true;
    }
    if menu_item(ui, "Export 3MF…", "", "mesh_export", shown) {
        FileAction::ExportMesh("3mf").run(app);
        *close = true;
    }
    if menu_item(ui, "Export STEP…", "", "export", shown) {
        FileAction::Export("step").run(app);
        *close = true;
    }
    if menu_item(ui, app.tr_label("Export Current View as DXF…"), "", "export", shown) {
        FileAction::Command("file.export_view_dxf").run(app);
        *close = true;
    }
    if menu_item(ui, "Export OBJ…", "", "export", shown) {
        FileAction::Export("obj").run(app);
        *close = true;
    }
    if menu_item(ui, "Export URDF…", "", "export", shown) {
        FileAction::Export("urdf").run(app);
        *close = true;
    }
    if menu_item(ui, "Export Design as JSON…", "", "export", true) {
        FileAction::Export("json").run(app);
        *close = true;
    }
}

/// The bar's Export button's menu: the export rows alone, under the button.
fn export_menu(app: &mut SolveApp, ctx: &egui::Context) {
    let id = egui::Id::new("sc_export_menu");
    if !ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
        return;
    }
    let at = ctx.data(|d| d.get_temp::<egui::Pos2>(egui::Id::new("sc_export_menu_at"))).unwrap_or(pos2(100.0, APP_BAR_HEIGHT));
    let mut close = false;
    let resp =
        Container::new(ctx, Surface::Popup).area(egui::Id::new("sc_export_area")).fixed_pos(at).constrain(true).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                crate::ui_kit::Menu::show_frame(ui, crate::ui_kit::Menu::frame(), |ui| {
                    ui.set_min_width(190.0);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = Tokens::get().hover;
                    export_rows(app, ui, &mut close);
                });
            },
        );
    crate::scenario::publish_rect("appbar:export_menu", resp.response.rect);
    let just_opened = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("sc_export_menu_opened"))) == Some(ctx.cumulative_pass_nr());
    if close || (!just_opened && resp.response.clicked_elsewhere()) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ctx.data_mut(|d| d.insert_temp(id, false));
    }
}

/// The application menu, from the logo: the start page, the File items (New, Open, Save, Save
/// As, Export…), the workspaces, Preferences, Keyboard Shortcuts, Help, Quit and the version.
fn file_menu(app: &mut SolveApp, ctx: &egui::Context) {
    let id = egui::Id::new("sc_file_menu");
    if app.native_file_menu {
        ctx.data_mut(|data| data.insert_temp(id, false));
        return;
    }
    if !ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
        return;
    }
    let mut close = false;
    let mut help: Option<&'static str> = None;
    let mut workspaces = false;
    let at = pos2(ctx.data(|d| d.get_temp::<f32>(egui::Id::new("sc_file_menu_x"))).unwrap_or(16.0), APP_BAR_HEIGHT);
    let resp = Container::new(ctx, Surface::Popup).area(egui::Id::new("sc_file_area")).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        crate::ui_kit::Menu::show_frame(ui, crate::ui_kit::Menu::frame(), |ui| {
            ui.set_min_width(220.0);
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
            ui.visuals_mut().widgets.hovered.weak_bg_fill = Tokens::get().hover;
            let item_if = |ui: &mut egui::Ui, label: &str, key: &str, icon: &str, enabled: bool| menu_item(ui, label, key, icon, enabled);
            let item = |ui: &mut egui::Ui, label: &str, key: &str, icon: &str| item_if(ui, label, key, icon, true);
            // Insert, Save and Export act on the design, which Home hides.
            let shown = design_shown(app);
            if item(ui, app.tr("ui.start_page"), "", "home") {
                app.home.show(app.session.revision);
                close = true;
            }
            ui.separator();
            if item(ui, app.tr("ui.new_design"), "Ctrl+N", "new") {
                FileAction::New.run(app);
                close = true;
            }
            if item(ui, &format!("{}…", app.tr("ui.open")), "Ctrl+O", "open") {
                FileAction::Open.run(app);
                close = true;
            }
            crate::ui_kit::Menu::button(ui, app.tr("ui.open_recent"), |ui| {
                for path in app.home.recent.clone() {
                    let missing = !crate::home::available(app, &path);
                    let label = format!("{path}{}", if missing { " (missing)" } else { "" });
                    if crate::context_menu::button(ui, &label, "open").clicked() {
                        FileAction::OpenRecent(path).run(app);
                        close = true;
                        ui.close();
                    }
                }
                ui.separator();
                if crate::context_menu::button(ui, app.tr("ui.clear_recent"), "delete").clicked() {
                    FileAction::ClearRecent.run(app);
                    close = true;
                    ui.close();
                }
            });
            if item_if(ui, "Insert STEP…", "", "import", shown) {
                FileAction::Command("file.insert_step").run(app);
                close = true;
            }
            if item_if(ui, "Insert Mesh…", "", "import", shown) {
                FileAction::Command("file.insert_mesh").run(app);
                close = true;
            }
            if item_if(ui, &format!("{}…", app.tr_label("Insert Design")), "", "component", shown) {
                FileAction::Command("component.insert_design").run(app);
                close = true;
            }
            if item_if(ui, app.tr("ui.save"), "Ctrl+S", "save", shown) {
                FileAction::Save.run(app);
                close = true;
            }
            if item_if(ui, &format!("{}…", app.tr("ui.save_as")), "", "save", shown) {
                FileAction::SaveAs.run(app);
                close = true;
            }
            if item(ui, "Save As JSON…", "", "save") {
                FileAction::SaveJson.run(app);
                close = true;
            }
            // The browser build: a copy as a downloaded file, whatever Save does.
            if app.services.download_design.is_some() && item_if(ui, "Download design", "", "export", shown) {
                download_design(app);
                close = true;
            }
            if app.services.browser_files.is_some() && item(ui, "Files in this browser…", "", "open") {
                if let Some(show) = &app.services.browser_files {
                    show();
                }
                close = true;
            }
            ui.separator();
            export_rows(app, ui, &mut close);
            ui.separator();
            if item_if(ui, &format!("{}…", app.tr_label("Design Properties")), "", "settings", shown) {
                FileAction::Command("document.metadata").run(app);
                close = true;
            }
            ui.separator();
            // The workspaces (Design, Render, and the ones planned): the tab row switches
            // between the ones that exist, this lists them all.
            if item(ui, &format!("{}…", app.tr("ui.ribbon_workspace")), "", "ws_design") {
                workspaces = true;
                close = true;
            }
            // The browser can install Polykrom as an app (`install`): only while it offers to.
            if let Some(offer) = crate::install::offered(app)
                && crate::context_menu::button(ui, crate::i18n::tr("install.menu"), "new")
                    .on_hover_text(crate::i18n::tr("install.button_tip"))
                    .clicked()
            {
                crate::install::start(app, offer);
                close = true;
            }
            if item(ui, &format!("{}…", app.tr("ui.preferences")), if cfg!(target_os = "macos") { "Cmd+," } else { "Ctrl+," }, "settings") {
                FileAction::Settings.run(app);
                close = true;
            }
            if item(ui, &format!("{}…", app.tr("ui.keyboard_shortcuts")), "", "list") {
                FileAction::Shortcuts.run(app);
                close = true;
            }
            crate::ui_kit::Menu::button(ui, app.tr("ui.help"), |ui| {
                for (id, label, icon) in crate::help::ITEMS {
                    if crate::context_menu::button(ui, crate::i18n::tr_label(label), icon).clicked() {
                        help = Some(id);
                        close = true;
                        ui.close();
                    }
                }
            });
            if item(ui, app.tr("ui.quit"), "", "close") {
                FileAction::Quit.run(app);
                close = true;
            }
            ui.separator();
            // The version, at the foot of the menu (About says the rest).
            let t = Tokens::get();
            ui.add_space(t.space_2);
            ui.label(egui::RichText::new(format!("Polykrom v{}", crate::update::current_version())).size(t.font_small).color(t.text_dim));
            ui.add_space(t.space_2);
        });
    });
    let just_opened = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("sc_file_menu_opened"))) == Some(ctx.cumulative_pass_nr());
    if close || (!just_opened && resp.response.clicked_elsewhere()) {
        ctx.data_mut(|d| d.insert_temp(id, false));
    }
    if let Some(item) = help {
        crate::help::run(app, ctx, item);
    }
    if workspaces {
        crate::context_menu::open_for(app, at, crate::context_menu::Target::Workspace);
    }
}

/// Whether the design is on screen: viewport/menu editing stays disabled behind File.
/// File owns its explicit Save/Save As/export actions and returns to the design before dispatch.
pub fn design_shown(app: &SolveApp) -> bool {
    !app.home.open
}

pub fn save(app: &mut SolveApp) {
    if crate::browser::needs_capture(app, crate::browser::SAVE) {
        app.tree.capture_prompt = Some(crate::browser::SAVE.into());
        return;
    }
    if app.session.path.is_some() {
        crate::save_job::start(app, "file.save", json!({}));
    } else {
        save_as(app);
    }
}

pub(crate) fn save_as(app: &mut SolveApp) {
    save_as_with_extension(app, polykrom_engine::io::DESIGN_EXT);
}

fn save_as_with_extension(app: &mut SolveApp, extension: &str) {
    let name = format!("{}.{extension}", app.session.doc.name);
    app.pick_save(&name, &[polykrom_engine::io::DESIGN_EXT, "json"], |app, path| {
        // A save under way answers a closing tab when it ends.
        if let Some(path) = path
            && crate::save_job::start(app, "file.save_as", json!({"path": path}))
        {
            return;
        }
        crate::documents::saved_for_close(app);
    });
}

/// File > Download design (browser build): the open design as a `.polykrom` file download.
fn download_design(app: &mut SolveApp) {
    let name = format!("{}.{}", app.session.doc.name, polykrom_engine::io::DESIGN_EXT);
    let result = app
        .session
        .design_container(polykrom_engine::io::DesignCompression::Fast, false, false)
        .map_err(|e| e.to_string())
        .and_then(|bytes| app.services.download_design.as_ref().map_or(Ok(()), |download| download(&name, &bytes)));
    match result {
        Ok(()) => app.set_status(format!("Downloaded {name}"), false),
        Err(e) => app.set_status(format!("Download design: {e}"), true),
    }
}

pub(crate) fn export(app: &mut SolveApp, ext: &str) {
    export_as(app, ext, None);
}

/// Export to a file with extension `ext`, in `format` when the extension alone doesn't name it.
fn export_as(app: &mut SolveApp, ext: &str, format: Option<&str>) {
    let name = format!("{}.{ext}", app.session.doc.name);
    let format = format.map(str::to_string);
    app.pick_save(&name, &[ext], move |app, path| {
        let Some(path) = path else { return };
        let mut params = json!({"path": path});
        if let Some(format) = format {
            params["format"] = json!(format);
        }
        if app.run("file.export", params).is_ok() {
            app.set_status(format!("Exported {path}"), false);
        }
    });
}

/// Choose at most two lines at spaces or after `/`, then truncate each line at
/// Unicode character boundaries. An oversized word stays on one line with an ellipsis.
fn ribbon_button_lines(painter: &egui::Painter, label: &str, width: f32) -> Vec<String> {
    let mut lines = ribbon_label_lines(painter, label, width);
    for line in &mut lines {
        if ribbon_label_width(painter, line) > width {
            while !line.is_empty() {
                line.pop();
                if ribbon_label_width(painter, &format!("{}…", line.trim_end())) <= width {
                    break;
                }
            }
            *line = format!("{}…", line.trim_end());
        }
    }
    lines
}

/// Ribbon command labels are set at the kit's small size (11 pt: `Polykrom / UI / Small`, the
/// size of every command label in its ribbon).
fn ribbon_label_width(painter: &egui::Painter, text: &str) -> f32 {
    let mut job =
        egui::text::LayoutJob::simple(text.to_owned(), FontId::proportional(crate::theme::Tokens::get().font_small), Color32::WHITE, f32::INFINITY);
    job.halign = egui::Align::Center;
    painter.layout_job(job).size().x
}

fn ribbon_button_width(painter: &egui::Painter, label: &str) -> f32 {
    let widest = ribbon_label_lines(painter, label, BUTTON_MAX_WIDTH - BUTTON_LABEL_PADDING)
        .iter()
        .map(|line| ribbon_label_width(painter, line))
        .fold(0.0_f32, f32::max);
    (widest + BUTTON_LABEL_PADDING).clamp(BUTTON_MIN_WIDTH, BUTTON_MAX_WIDTH)
}

fn ribbon_label_lines(painter: &egui::Painter, label: &str, width: f32) -> Vec<String> {
    let measure = |text: &str| ribbon_label_width(painter, text);
    let normalized = label.split_whitespace().collect::<Vec<_>>().join(" ");
    let label = normalized.as_str();
    let mut end = 0;
    for token in label.split_inclusive(|c: char| c.is_whitespace() || c == '/') {
        let next = end + token.len();
        if end > 0 && measure(label.get(..next).unwrap_or_default().trim()) > width {
            break;
        }
        end = next;
    }
    let (first, rest) = label.split_at(end);
    let mut lines = vec![first.trim().to_owned()];
    if !rest.trim().is_empty() {
        lines.push(rest.trim().to_owned());
    }
    lines
}

fn ribbon_button_galley(painter: &egui::Painter, label: &str, width: f32, color: Color32) -> std::sync::Arc<egui::Galley> {
    let lines = ribbon_button_lines(painter, label, width);
    // Explicit newlines only: egui must never split an oversized word.
    let mut job = egui::text::LayoutJob::simple(lines.join("\n"), FontId::proportional(crate::theme::Tokens::get().font_small), color, f32::INFINITY);
    job.halign = egui::Align::Center;
    painter.layout_job(job)
}

/// Ribbon headings use the translated labels, with normal word casing instead of registry caps.
fn ribbon_label(app: &SolveApp, label: &str) -> String {
    let label = app
        .tr_label(label)
        .split_whitespace()
        .map(|word| {
            let lower = word.to_lowercase();
            let mut chars = lower.chars();
            chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    // Partial catalogs may already translate the word-cased name (for example "Model").
    app.tr_label(&label).to_string()
}

fn ribbon_text(ui: &egui::Ui, at: egui::Pos2, text: &str, size: f32, color: Color32, bold: bool) {
    let text = egui::RichText::new(text).size(size).color(color);
    let text = if bold { text.semibold() } else { text };
    let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
    let origin = at - galley.size() / 2.0;
    let ppp = ui.ctx().pixels_per_point().max(0.5);
    let origin = pos2((origin.x * ppp).round() / ppp, (origin.y * ppp).round() / ppp);
    ui.painter().galley(origin, galley, color);
}

fn ribbon_tabs(app: &mut SolveApp, ui: &mut egui::Ui, row: Rect) {
    let t = Tokens::get();
    let tabs = crate::workspace::visible_tabs(app);
    let rendering = crate::render_workspace::active(app);
    let font = crate::fonts::semibold(t.font_body);
    let widths: Vec<_> =
        tabs.iter().map(|tab| ui.painter().layout_no_wrap(ribbon_label(app, tab), font.clone(), t.text).size().x + 2.0 * TAB_PAD).collect();
    let tabs_width: f32 = widths.iter().sum::<f32>() + tabs.len().saturating_sub(1) as f32 * TAB_GAP;
    // The tab row sits on the window's base surface; the ribbon's groups are lifted from it.
    ui.painter().rect_filled(row, 0.0, t.window);
    let changed = ui.ctx().data_mut(|d| {
        let id = egui::Id::new("sc_ribbon_visible_tab");
        let state = (app.ui.tab.clone(), row.width());
        let changed = d.get_temp::<(String, f32)>(id) != Some(state.clone());
        d.insert_temp(id, state);
        changed
    });
    let span = Rect::from_min_max(row.left_top() + vec2(TAB_INSET, 0.0), pos2((row.right() - TAB_INSET).max(row.left() + TAB_INSET), row.bottom()));
    let mut picked: Option<&'static str> = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(span), |ui| {
        ui.set_clip_rect(span);
        egui::ScrollArea::horizontal()
            .id_salt("ribbon_tabs")
            .animated(false)
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                let (all, _) = ui.allocate_exact_size(vec2(tabs_width, TAB_ROW_HEIGHT), Sense::hover());
                let mut x = all.left();
                for (tab, width) in tabs.into_iter().zip(widths) {
                    let tr = Rect::from_min_size(pos2(x, all.top()), vec2(width, TAB_ROW_HEIGHT));
                    let active = app.ui.tab == tab;
                    if active && changed {
                        ui.scroll_to_rect(tr, Some(egui::Align::Center));
                    }
                    let response = ui.interact(tr, ui.id().with(("tab", tab)), Sense::click());
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, ribbon_label(app, tab)));
                    let accent = crate::theme::TabAccent::for_tab(&t, tab);
                    // The active tab: its accent as text with a 2 px underline (the kit's lime on
                    // the base colour), no filled pill; the others in the muted text colour.
                    let ink = if active {
                        accent.color
                    } else if response.hovered() || response.has_focus() {
                        t.text
                    } else {
                        t.text_dim
                    };
                    ribbon_text(ui, pos2(tr.center().x, tr.top() + 16.0), &ribbon_label(app, tab), t.font_body, ink, active);
                    if active {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                pos2(tr.left() + TAB_PAD - 1.0, tr.bottom() - TAB_UNDERLINE - 6.0),
                                vec2(width - 2.0 * TAB_PAD + 2.0, TAB_UNDERLINE),
                            ),
                            0.0,
                            accent.color,
                        );
                    }
                    if ui.clip_rect().contains(tr.center()) {
                        crate::scenario::publish_target(&format!("tab:{tab}"), tr);
                        // The part of the tab on screen: the strip scrolls, and its last visible
                        // tab may be cut by the strip's edge.
                        crate::scenario::publish_rect(&format!("toolbar_tab:{tab}"), tr.intersect(ui.clip_rect()));
                        let aliases: &[&str] = match tab {
                            "MODEL" => &["SOLID", "PLASTIC"],
                            "SURFACE" => &["MESH"],
                            "MANAGE" => &["UTILITIES"],
                            _ => &[],
                        };
                        for alias in aliases {
                            crate::scenario::publish_handle(&format!("tab:{alias}"), tr.center());
                        }
                    }
                    if response.clicked() {
                        picked = Some(tab);
                    }
                    x += width + TAB_GAP;
                }
            });
    });
    // The tabs carry the workspace choice too: RENDER enters the Render workspace, a design
    // tab leaves it (the former workspace block).
    if let Some(tab) = picked {
        if tab == "RENDER" {
            if !rendering && let Err(e) = crate::render_workspace::enter(app, true) {
                app.set_status(e, true);
            }
        } else {
            if rendering && let Err(e) = crate::render_workspace::enter(app, false) {
                app.set_status(e, true);
            }
            app.ui.tab = tab.to_string();
        }
    }
}

/// From the window's edge to the first tab, and between tabs (the kit's 16 and 26 px).
const TAB_INSET: f32 = 16.0;
const TAB_GAP: f32 = 18.0;
/// The hit padding on either side of a tab's text.
const TAB_PAD: f32 = 4.0;
/// The active tab's underline.
const TAB_UNDERLINE: f32 = 2.0;

/// The dropdown chevron every ribbon menu shares.
fn dropdown_arrow(painter: &egui::Painter, at: egui::Pos2, ink: Color32) {
    crate::ui_kit::dropdown_indicator(painter, Rect::from_center_size(at, vec2(10.0, 10.0)), ink);
}

/// What a ribbon group draws at one place, left to right: a large icon-over-label button, a
/// column of up to two small icon-beside-label buttons, or the constraint gallery.
enum RibbonItem {
    Large(&'static polykrom_engine::CommandSpec),
    Small(Vec<&'static polykrom_engine::CommandSpec>),
    Gallery,
}

/// A small button's label, on one line: a long one ends in an ellipsis.
fn small_label_galley(painter: &egui::Painter, label: &str, width: f32, color: Color32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(label.to_owned(), FontId::proportional(Tokens::get().font_small), color, width.max(1.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    painter.layout_job(job)
}

/// A small-button column's width: its widest label beside the icon, within the kit's bounds.
fn small_column_width(app: &SolveApp, painter: &egui::Painter, column: &[&'static polykrom_engine::CommandSpec]) -> f32 {
    let widest = column.iter().map(|c| ribbon_label_width(painter, app.tr_label(crate::workspace::ribbon_button_label(c)))).fold(0.0_f32, f32::max);
    (SMALL_TEXT_X + widest + SMALL_PAD).clamp(SMALL_MIN_WIDTH, SMALL_MAX_WIDTH)
}

pub fn toolbar(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    crate::workspace::sync_context(app);
    let floating = Container::for_layout(app.ui.layout, Surface::Panel).floating();
    let labels = labels_shown(app, ui.ctx());
    let button_height = if labels { BUTTON_HEIGHT } else { ICON_BUTTON_HEIGHT };
    let group_height = if labels { GROUP_HEIGHT } else { ICON_GROUP_HEIGHT };
    let ribbon_height = ribbon_height(labels);
    let gallery_width = if labels { CONSTRAINT_GRID_WIDTH } else { 6.0 * 27.0 };
    let pad_top = if labels { GROUP_PAD_TOP } else { 2.0 };
    let cards = crate::shell::Cards::new(ui.available_rect_before_wrap(), app.ui.layout, ribbon_height);
    let height = if floating { cards.bottom - ui.available_rect_before_wrap().top() } else { ribbon_height };
    let fill = if floating { Color32::TRANSPARENT } else { t.toolbar };
    let ribbon = Container::new(ui.ctx(), Surface::Chrome).edge("sc_toolbar", Edge::Top);
    let frame = Container::new(ui.ctx(), Surface::Chrome).edge_frame(fill);
    let chrome = ribbon.exact_size(height).show_separator_line(false).frame(frame).show(ui, |ui| {
        let r = if floating { cards.ribbon } else { frame.outer_rect(ui.max_rect()) };
        let ribbon_left = r.left();
        if floating {
            let frame = Container::new(ui.ctx(), Surface::Chrome).frame(ui.ctx(), t.toolbar, crate::frame::Side::Left);
            ui.painter().add(frame.paint(cards.ribbon));
            // Blank card space owns clicks as well as its buttons.
            ui.interact(cards.ribbon, ui.id().with(("shell_card", cards.ribbon.left().to_bits())), Sense::click_and_drag());
            crate::scenario::publish_rect("ribbon:card", cards.ribbon);
        }
        app.shell.cards = vec![r];
        let painter = ui.painter().clone();
        let bg = ui.interact(r, ui.id().with("toolbar_bg"), Sense::click());
        if bg.secondary_clicked() {
            crate::context_menu::open_for(app, bg.interact_pointer_pos().unwrap_or(r.left_bottom()), crate::context_menu::Target::Toolbar);
        }
        crate::scenario::publish_rect("toolbar", r);
        ribbon_tabs(app, ui, Rect::from_min_max(r.left_top(), pos2(r.right(), r.top() + TAB_ROW_HEIGHT)));
        // Panels.
        let tab = app.ui.tab.clone();
        // The command whose tool or dialog is open right now, shown pressed in the toolbar.
        let running: Option<String> = match (&app.tool, &app.dialog) {
            (Some(tool), _) => Some(tool.cmd.to_string()),
            // Create Sketch never lands in last_command (it is not repeatable).
            (None, Some(d)) if matches!(d.kind, crate::dialogs::Kind::Sketch) => Some("sketch.create".to_string()),
            (None, Some(_)) => app.last_command.as_ref().map(|(id, _)| id.clone()),
            _ => None,
        };
        let mut px = ribbon_left;
        let top = r.top() + TAB_ROW_HEIGHT;
        let mut drop: Option<(&'static str, &'static str, Option<&'static str>)> = None;
        let mut menu: Option<(egui::Pos2, &'static str)> = None;
        // A drop-down row's pin icon clicked: pin or unpin that command.
        let mut toggle: Option<&'static str> = None;
        // The open drop-down (a button dropped on it leaves the toolbar).
        let mut popup_rect: Option<Rect> = None;
        let mut overflow = Vec::new();
        // Every panel with all its buttons (the user's pins have no cap); a toolbar too narrow
        // for them narrows the buttons, then trims panels into their drop-down and the » menu.
        let lists: Vec<_> = crate::toolbar_custom::panel_lists(&tab)
            .into_iter()
            .map(|(panel, cmds, promote)| {
                let mut shown = crate::toolbar_custom::buttons(app, &tab, panel, &cmds, promote);
                // Gallery cells are painted together after the large buttons, in reading order.
                if tab == "SKETCH" && panel == "CONSTRAIN" {
                    shown.sort_by_key(|c| {
                        if app.toolbar_custom.in_gallery(&tab, panel, c.id) {
                            crate::workspace::CONSTRAINT_GALLERY.iter().position(|id| *id == c.id).map(|i| i + 1).unwrap_or(0)
                        } else {
                            0
                        }
                    });
                }
                (panel, cmds, shown)
            })
            .collect();
        // Each panel's items: its pinned (large) buttons, then the group's other commands as
        // two-row small buttons (the kit's secondary tools), then the gallery.
        let items: Vec<Vec<RibbonItem>> = lists
            .iter()
            .map(|(panel, cmds, shown)| {
                let mut items: Vec<RibbonItem> =
                    shown.iter().copied().filter(|c| !app.toolbar_custom.in_gallery(&tab, panel, c.id)).map(RibbonItem::Large).collect();
                if shown.iter().any(|c| app.toolbar_custom.in_gallery(&tab, panel, c.id)) {
                    items.push(RibbonItem::Gallery);
                }
                // The group's other commands as small buttons: not the ones pinned as a button on
                // any group of the tab, nor a sketch family's variants (its button chooses them).
                let small: Vec<_> = if labels && tab != "SKETCH" && *panel != "SELECT" {
                    cmds.iter()
                        .copied()
                        .filter(|c| crate::toolbar_custom::pinned_on(app, &tab, c.id).is_none() && !app.toolbar_custom.in_gallery(&tab, panel, c.id))
                        .filter(|c| !(tab == "SKETCH" && *panel == "CREATE" && crate::sketch_families::family(c.id).is_some()))
                        // A label too long for a small button stays in the drop-down (no wrap, no cut).
                        .filter(|c| {
                            ribbon_label_width(&painter, app.tr_label(crate::workspace::ribbon_button_label(c))) + SMALL_TEXT_X + SMALL_PAD
                                <= SMALL_MAX_WIDTH
                        })
                        .take(SMALL_MAX_COLUMNS * SMALL_ROWS)
                        .collect()
                } else {
                    Vec::new()
                };
                items.extend(small.chunks(SMALL_ROWS).map(|column| RibbonItem::Small(column.to_vec())));
                items
            })
            .collect();
        let sizes: Vec<PanelSize> = lists
            .iter()
            .zip(&items)
            .map(|((panel, _, _), items)| PanelSize {
                secondary_from: items.iter().position(|item| matches!(item, RibbonItem::Small(_))).unwrap_or(usize::MAX),
                buttons: items
                    .iter()
                    .map(|item| match item {
                        RibbonItem::Large(c) => {
                            let family = crate::sketch_families::family(c.id)
                                .filter(|f| tab == "SKETCH" && *panel == "CREATE" && f.commands.first().copied() == Some(c.id));
                            let label = family.map_or_else(|| crate::workspace::ribbon_button_label(c), |f| f.label);
                            if labels {
                                ButtonSize { width: ribbon_button_width(&painter, app.tr_label(label)), min_width: BUTTON_MIN_WIDTH }
                            } else {
                                let width = if family.is_some() { 50.0 } else { 40.0 };
                                ButtonSize { width, min_width: width }
                            }
                        }
                        RibbonItem::Small(column) => ButtonSize { width: small_column_width(app, &painter, column), min_width: SMALL_MIN_WIDTH },
                        RibbonItem::Gallery => ButtonSize { width: gallery_width, min_width: gallery_width },
                    })
                    .collect(),
            })
            .collect();
        let right = r.right();
        // The Sketch exit group follows the last sketch panel, and is always kept room for so it
        // stays reachable at every width; when panels go into the » menu it moves to the far right.
        let finish_plan = sizes.last().filter(|_| tab == "SKETCH").map(Plan::full);
        let finish_width = finish_plan.as_ref().map_or(0.0, |p| p.width().max(80.0));
        let normal_count = sizes.len().saturating_sub(usize::from(finish_plan.is_some()));
        let mut plans = fit(sizes.get(..normal_count).unwrap_or_default(), right - 4.0 - px - finish_width);
        let finish_right = plans.iter().any(|p| p.fit == Fit::More);
        plans.extend(finish_plan);
        crate::scenario::publish_count("toolbar_collapsed", plans.iter().filter(|p| p.fit == Fit::Collapsed).count() as f64);
        crate::scenario::publish_count("toolbar_more", plans.iter().filter(|p| p.fit == Fit::More).count() as f64);
        crate::scenario::publish_count(
            "toolbar_trimmed",
            plans.iter().zip(&sizes).filter(|(p, s)| p.fit == Fit::Buttons && p.shown < s.buttons.len()).count() as f64,
        );
        for (((panel, cmds, shown), items), plan) in lists.into_iter().zip(items).zip(plans) {
            let panel: &&str = &panel;
            if plan.fit == Fit::More {
                overflow.push((*panel, menu_commands(&cmds, &shown)));
                continue;
            }
            let collapsed = plan.fit == Fit::Collapsed;
            let trimmed = plan.shown < plan.widths.len();
            let finish = tab == "SKETCH" && *panel == "FINISH SKETCH";
            let width = if finish { finish_width } else { plan.width() };
            if finish && finish_right {
                px = right - finish_width;
                painter.line_segment([pos2(px, top), pos2(px, r.bottom() - 1.0)], t.tertiary_border(t.border));
            }
            let button_widths = &plan.widths;
            let sketch_create = tab == "SKETCH" && *panel == "CREATE";
            let enabled_panel = !cmds.is_empty() || *panel == "SELECT";
            // Per tab: panels of the same name on other tabs (MODIFY, CREATE…) list other commands,
            // and the drop-down's area remembers its size by this id (#874).
            let popup_id = ui.id().with(("panel_menu", &tab, panel));
            let gallery: Vec<_> = shown.iter().copied().filter(|c| app.toolbar_custom.in_gallery(&tab, panel, c.id)).collect();
            let drawn = if collapsed { 0 } else { plan.shown.min(button_widths.len()) };
            // Buttons narrower than the panel's minimum width (a one-button panel) are centred.
            let buttons_width = button_widths.iter().take(drawn).sum::<f32>() + drawn.saturating_sub(1) as f32 * BUTTON_GAP;
            let inset = if drawn == 0 { 0.0 } else { ((width - 8.0 - buttons_width) / 2.0).max(0.0).floor() };
            let bx = px + 4.0 + inset;
            let free_x = bx + buttons_width + BUTTON_GAP;
            // Dropped on the panel's free space (all of a collapsed panel): the button goes to
            // its end.
            let free = Rect::from_min_size(pos2(free_x, top + pad_top), vec2((px + width - 4.0 - free_x).max(0.0), button_height));
            let fresp = ui.interact(free, ui.id().with(("panel_free", panel)), if collapsed { Sense::click() } else { Sense::hover() });
            let mut opened = false;
            if collapsed {
                // One button: the panel's first command's icon over a caret; it opens the
                // drop-down, which lists every command of the panel.
                let br = Rect::from_center_size(free.center(), vec2((width - 8.0).max(0.0), button_height));
                crate::scenario::publish_handle(&format!("collapsed:{tab}:{panel}"), br.center());
                if fresp.hovered() && enabled_panel {
                    painter.rect_filled(br, t.radius_sm, t.hover);
                }
                if let Some(c) = shown.first().or(cmds.first()) {
                    icons::paint_icon(
                        &painter,
                        Rect::from_center_size(br.center() - vec2(0.0, 3.0), vec2(26.0, 26.0)),
                        c.icon,
                        f32::from(t.icon.a()) / 255.0,
                    );
                }
                dropdown_arrow(&painter, pos2(br.center().x, br.bottom() - 3.0), t.text_dim);
                let tip = format!("{} ({})", app.tr_label(panel), shown.len().max(cmds.len()));
                if fresp.on_hover_text(tip).clicked() && enabled_panel {
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sc_panel_open"), popup_id));
                    opened = true;
                }
            }
            let accent = crate::theme::TabAccent::for_tab(&t, &tab);
            let mut button_x = bx;
            for (item, button_width) in items.iter().zip(button_widths.iter().copied()).take(drawn) {
                let whole = Rect::from_min_size(pos2(button_x, top + pad_top), vec2(button_width, button_height));
                button_x += button_width + BUTTON_GAP;
                match item {
                    RibbonItem::Large(c) => {
                        // Only the family's default pin follows the part's choice. Explicitly pinned
                        // variants remain exact commands, including in customised toolbars.
                        let family = crate::sketch_families::family(c.id).filter(|f| sketch_create && f.commands.first().copied() == Some(c.id));
                        let selected = family.and_then(|f| app.preferences.sketch_variants.selected(f)).unwrap_or(c);
                        let mut br = whole;
                        if family.is_some() {
                            if labels {
                                br.max.y = whole.top() + 38.0;
                            } else {
                                br.max.x -= 14.0;
                            }
                        }
                        let info = selected.info(&app.session);
                        let resp = ui.interact(br, ui.id().with(("cmd", c.id)), Sense::click_and_drag());
                        crate::scenario::publish_handle(&format!("toolbar:{}", c.id), br.center());
                        // The button itself (`toolbar:<id>` is also a drop-down row's while one is open).
                        crate::scenario::publish_handle(&format!("button:{}", c.id), br.center());
                        crate::scenario::publish_rect(&format!("button:{}", c.id), whole);

                        if selected.id != c.id {
                            crate::scenario::publish_handle(&format!("button:{}", selected.id), br.center());
                        }
                        if resp.drag_started() {
                            resp.dnd_set_drag_payload(ToolbarDrag { id: c.id, from_menu: false });
                        }
                        if let Some(d) = resp.dnd_hover_payload::<ToolbarDrag>()
                            && d.id != c.id
                        {
                            painter.line_segment(
                                [pos2(br.left() - 1.0, br.top() + 2.0), pos2(br.left() - 1.0, br.bottom() - 2.0)],
                                Stroke::new(2.0, t.accent),
                            );
                        }
                        if let Some(d) = resp.dnd_release_payload::<ToolbarDrag>()
                            && d.id != c.id
                        {
                            drop = Some((d.id, *panel, Some(c.id)));
                        }
                        if resp.secondary_clicked() {
                            menu = Some((resp.interact_pointer_pos().unwrap_or(br.left_bottom()), c.id));
                        }
                        let family_open = family.is_some_and(|f| egui::Popup::is_id_open(ui.ctx(), ui.id().with(("family", f.label)).with("popup")));
                        if !finish && (resp.is_pointer_button_down_on() || running.as_deref() == Some(selected.id) || family_open) {
                            painter.rect(whole, t.radius_sm, accent.soft, Stroke::new(1.0, accent.color), egui::StrokeKind::Inside);
                        } else if resp.hovered() && info.enabled {
                            painter.rect_filled(whole, t.radius_sm, t.hover);
                        }
                        let icon = if selected.id == "sketch.finish" { "finish" } else { selected.icon };
                        let side = if finish { 30.0 } else { LARGE_ICON };
                        let icon_rect = Rect::from_center_size(
                            pos2(br.center().x, whole.top() + if labels { LARGE_ICON_Y } else { button_height / 2.0 }),
                            vec2(side, side),
                        );
                        icons::paint_icon(&painter, icon_rect, icon, if info.enabled { 1.0 } else { 0.35 });
                        let tr_label = app.tr_label(selected.label).to_string();
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && info.enabled, &tr_label));
                        if ribbon_tooltip(app, resp, selected, info.disabled_reason.as_deref()).clicked() && info.enabled {
                            let id = selected.id;
                            app.start(id);
                        }
                        if let Some(f) = family {
                            let ar = if labels {
                                Rect::from_min_max(pos2(whole.left(), whole.top() + 38.0), pos2(whole.right(), whole.top() + 58.0))
                            } else {
                                Rect::from_min_max(pos2(br.right(), whole.top()), whole.right_bottom())
                            };
                            let arrow = ui.interact(ar, ui.id().with(("family", f.label)), Sense::click());
                            crate::scenario::publish_handle(&format!("family:{}", f.label), ar.center());
                            if arrow.hovered() {
                                painter.rect_filled(ar, t.radius_sm, t.hover);
                            }
                            // The title sits on the same line as the other buttons' labels.
                            if labels {
                                let title_y = whole.top() + LARGE_LABEL_Y;
                                let galley = ribbon_button_galley(&painter, app.tr_label(f.label), ar.width() - 16.0, t.text);
                                painter.galley(pos2(ar.center().x - 4.0, title_y - galley.size().y / 2.0), galley, t.text);
                                dropdown_arrow(&painter, pos2(ar.right() - 6.0, title_y + 0.5), t.text_dim);
                            } else {
                                dropdown_arrow(&painter, ar.center(), t.text_dim);
                            }
                            arrow.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, app.tr_label(f.label)));
                            Container::new(ui.ctx(), Surface::Popup).menu(&arrow).show(|ui| {
                                let variants: Vec<_> = f.commands.iter().filter_map(|id| polykrom_engine::find_command(id)).collect();
                                dropdown_keys(app, ui, &variants);
                                sketch_menu_rows(app, ui, &variants, &mut menu, &mut toggle);
                            });
                        } else if labels {
                            let label = app.tr_label(crate::workspace::ribbon_button_label(selected));
                            let color = if info.enabled { t.text } else { t.text_dim };
                            let galley = ribbon_button_galley(&painter, label, whole.width() - BUTTON_LABEL_PADDING, color);
                            painter.galley(pos2(whole.center().x, whole.top() + LARGE_LABEL_Y - galley.size().y / 2.0), galley, t.text);
                        }
                    }
                    RibbonItem::Small(column) => {
                        // Two rows of icon-beside-label buttons (the kit's secondary tools).
                        for (k, c) in column.iter().enumerate() {
                            let br = Rect::from_min_size(
                                pos2(whole.left(), whole.top() + k as f32 * (SMALL_HEIGHT + SMALL_ROW_GAP)),
                                vec2(whole.width(), SMALL_HEIGHT),
                            );
                            let info = c.info(&app.session);
                            let resp = ui.interact(br, ui.id().with(("cmd", c.id)), Sense::click());
                            crate::scenario::publish_handle(&format!("toolbar:{}", c.id), br.center());
                            crate::scenario::publish_rect(&format!("ribbon:small:{}", c.id), br);
                            if resp.is_pointer_button_down_on() || running.as_deref() == Some(c.id) {
                                painter.rect(br, t.radius_sm, accent.soft, Stroke::new(1.0, accent.color), egui::StrokeKind::Inside);
                            } else if resp.hovered() && info.enabled {
                                painter.rect_filled(br, t.radius_sm, t.hover);
                            }
                            let icon_rect =
                                Rect::from_center_size(pos2(br.left() + SMALL_PAD + SMALL_ICON / 2.0, br.center().y), vec2(SMALL_ICON, SMALL_ICON));
                            icons::paint_icon(&painter, icon_rect, c.icon, if info.enabled { 1.0 } else { 0.35 });
                            let label = app.tr_label(crate::workspace::ribbon_button_label(c));
                            let color = if info.enabled { t.text } else { t.text_dim };
                            let galley = small_label_galley(&painter, label, br.width() - SMALL_TEXT_X - 4.0, color);
                            painter.galley(pos2(br.left() + SMALL_TEXT_X, br.center().y - galley.size().y / 2.0), galley, color);
                            if resp.secondary_clicked() {
                                menu = Some((resp.interact_pointer_pos().unwrap_or(br.left_bottom()), c.id));
                            }
                            let tr_label = app.tr_label(c.label).to_string();
                            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && info.enabled, &tr_label));
                            if ribbon_tooltip(app, resp, c, info.disabled_reason.as_deref()).clicked() && info.enabled {
                                app.start(c.id);
                            }
                        }
                    }
                    RibbonItem::Gallery => {
                        // A quiet rule sets the grid apart from the button before it.
                        t.secondary_line(
                            &painter,
                            [pos2(whole.left() - 1.5, top + 8.0), pos2(whole.left() - 1.5, top + button_height.min(46.0))],
                            t.toolbar.lerp_to_gamma(t.border, 0.5),
                        );
                        for (index, c) in gallery.iter().enumerate() {
                            let columns = if labels { 3 } else { 6 };
                            let cell = Rect::from_min_size(
                                pos2(whole.left() + (index % columns) as f32 * 27.0, top + 6.0 + (index / columns) as f32 * 23.0),
                                vec2(24.0, 20.0),
                            );
                            let info = c.info(&app.session);
                            let resp = ui.interact(cell, ui.id().with(("cmd", c.id)), Sense::click_and_drag());
                            let active = running.as_deref() == Some(c.id);
                            painter.rect(
                                cell,
                                t.radius_sm,
                                if active {
                                    if accent.color == t.accent { t.panel.lerp_to_gamma(command_accent(c.icon), 0.18) } else { accent.soft }
                                } else if resp.hovered() && info.enabled {
                                    t.hover
                                } else {
                                    t.panel
                                },
                                Stroke::new(
                                    1.0,
                                    if active { if accent.color == t.accent { command_accent(c.icon) } else { accent.color } } else { t.border },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            icons::paint_icon(
                                &painter,
                                Rect::from_center_size(cell.center(), vec2(15.0, 15.0)),
                                c.icon,
                                if info.enabled { 1.0 } else { 0.35 },
                            );
                            for prefix in ["toolbar", "button", "ribbon:constraint"] {
                                crate::scenario::publish_target(&format!("{prefix}:{}", c.id), cell);
                            }
                            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, info.enabled, app.tr_label(c.label)));
                            if resp.drag_started() {
                                resp.dnd_set_drag_payload(ToolbarDrag { id: c.id, from_menu: false });
                            }
                            if let Some(d) = resp.dnd_release_payload::<ToolbarDrag>()
                                && d.id != c.id
                            {
                                drop = Some((d.id, *panel, Some(c.id)));
                            }
                            if resp.secondary_clicked() {
                                menu = Some((resp.interact_pointer_pos().unwrap_or(cell.left_bottom()), c.id));
                            }
                            if ribbon_tooltip(app, resp, c, info.disabled_reason.as_deref()).clicked() && info.enabled {
                                app.start(c.id);
                            }
                        }
                    }
                }
            }
            // The group's name along its bottom (the kit's muted 11 pt label) is also the
            // section's command-list button. It sits RIBBON_TAB_INSET from the card's bottom edge.
            let lr = Rect::from_min_size(
                pos2(px + RIBBON_TAB_INSET, top + group_height - SECTION_HEIGHT - (RIBBON_TAB_INSET - 1.0)),
                vec2((width - 2.0 * RIBBON_TAB_INSET).max(0.0), SECTION_HEIGHT),
            );
            let lresp = ui.interact(lr, ui.id().with(("panel", panel)), Sense::click());
            crate::scenario::publish_handle(&format!("panel:{tab}:{panel}"), lr.center());
            crate::scenario::publish_target(&format!("ribbon:section:{tab}:{panel}"), lr);
            let panel_open = ui.ctx().data(|d| d.get_temp::<egui::Id>(egui::Id::new("sc_panel_open"))) == Some(popup_id);
            let hot = lresp.hovered() && enabled_panel;
            if panel_open || hot {
                painter.rect_filled(lr, t.radius_sm, if panel_open { accent.soft } else { t.hover });
            }
            let label = if finish { app.tr("ui.ribbon_exit").to_string() } else { ribbon_label(app, panel) };
            let font = if panel_open || hot {
                crate::fonts::semibold(crate::theme::Tokens::get().font_small)
            } else {
                FontId::proportional(crate::theme::Tokens::get().font_small)
            };
            let label_width = painter.layout_no_wrap(label.clone(), font, t.text_dim).size().x;
            let ink = if panel_open { accent.ink } else { t.text_dim };
            let label_x = if finish { lr.center().x } else { (lr.left() + 8.0 + label_width / 2.0).min(lr.center().x) };
            ribbon_text(ui, pos2(label_x, lr.center().y), &label, t.font_small, ink, panel_open || hot);
            if !finish {
                dropdown_arrow(&painter, pos2(label_x + label_width / 2.0 + 7.0, lr.center().y + 0.5), ink);
            }
            if lresp.clicked() && enabled_panel {
                ui.ctx().data_mut(|d| {
                    let key = egui::Id::new("sc_panel_open");
                    if d.get_temp::<egui::Id>(key) == Some(popup_id) {
                        d.remove::<egui::Id>(key);
                    } else {
                        d.insert_temp(key, popup_id);
                    }
                });
            }
            // Labels, gaps and margins append; actual buttons keep their insert-before target.
            // Respect the current layer so a dropdown covering the panel is not a drop target.
            let panel_rect = Rect::from_min_max(pos2(px, top + 2.0), pos2(px + width, r.bottom()));
            crate::scenario::publish_rect(&format!("panel:{tab}:{panel}"), panel_rect);
            let over_button = ui.input(|input| input.pointer.hover_pos()).is_some_and(|p| {
                let mut x = bx;
                button_widths.iter().take(drawn).any(|width| {
                    let rect = Rect::from_min_size(pos2(x, top + pad_top), vec2(*width, button_height));
                    x += width + BUTTON_GAP;
                    rect.contains(p)
                })
            });
            if !over_button && ui.rect_contains_pointer(panel_rect) && egui::DragAndDrop::payload::<ToolbarDrag>(ui.ctx()).is_some() {
                painter.line_segment([pos2(px + width - 4.0, top + 4.0), pos2(px + width - 4.0, top + 38.0)], Stroke::new(2.0, t.accent));
                if ui.input(|input| input.pointer.any_released())
                    && let Some(d) = egui::DragAndDrop::take_payload::<ToolbarDrag>(ui.ctx())
                {
                    drop = Some((d.id, *panel, None));
                }
            }
            let open = ui.ctx().data(|d| d.get_temp::<egui::Id>(egui::Id::new("sc_panel_open"))) == Some(popup_id);
            if open {
                let cmds = if collapsed || trimmed { menu_commands(&cmds, &shown) } else { cmds.clone() };
                let area = Container::new(ui.ctx(), Surface::Popup).area(popup_id).fixed_pos(lr.left_bottom()).order(egui::Order::Foreground).show(
                    ui.ctx(),
                    |ui| {
                        crate::ui_kit::Menu::show_frame(ui, crate::ui_kit::Menu::frame(), |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.set_min_width(200.0);
                            if *panel == "SELECT" {
                                selection_filter(app, ui);
                            } else if cmds.is_empty() {
                                ui.label(egui::RichText::new(crate::i18n::tr_label("Not available yet")).color(t.text_dim));
                            }
                            if sketch_create {
                                sketch_create_menu(app, ui, &cmds, &mut menu, &mut toggle);
                                section_footer(ui);
                                return;
                            }
                            let grouped = crate::workspace::grouped_commands(&tab, panel, &cmds);
                            let cmds: Vec<_> = grouped.iter().map(|(_, c)| *c).collect();
                            let row_w = dropdown_width(app, ui, &cmds);
                            ui.set_width(row_w);
                            // Up and Down move the keyboard focus through the enabled rows (from none
                            // focused: the first or the last), instead of egui's own spatial move.
                            let (down, up) = ui.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp)));
                            if down || up {
                                let enabled: Vec<&str> = cmds.iter().filter(|c| c.info(&app.session).enabled).map(|c| c.id).collect();
                                let focused = ui.ctx().memory(|m| m.focused());
                                let at = enabled.iter().position(|id| focused == Some(dropdown_row_id(id)));
                                let n = enabled.len();
                                let next = match (at, down) {
                                    (None, true) => Some(0),
                                    (None, false) => n.checked_sub(1),
                                    (Some(i), true) => Some((i + 1) % n.max(1)),
                                    (Some(i), false) => Some((i + n.max(1) - 1) % n.max(1)),
                                };
                                if let Some(id) = next.and_then(|i| enabled.get(i)) {
                                    ui.ctx().memory_mut(|m| {
                                        m.request_focus(dropdown_row_id(id));
                                        m.move_focus(egui::FocusDirection::None);
                                    });
                                }
                            }
                            // The rows scroll when the drop-down would run past the window's bottom.
                            // `min_scrolled_height` lets the rows grow past the size the area kept from
                            // an earlier, shorter list (#874); they still shrink to fit their content.
                            let room = (ui.ctx().content_rect().bottom() - ui.cursor().top() - 64.0).max(ROW_H * 3.0);
                            egui::ScrollArea::vertical().id_salt("panel_menu_rows").max_height(room).min_scrolled_height(room).show(ui, |ui| {
                                for (heading, c) in &grouped {
                                    if let Some(heading) = heading {
                                        section_heading(ui, heading, row_w);
                                    }
                                    let pinned = crate::toolbar_custom::pinned_on(app, &tab, c.id).is_some();
                                    match dropdown_row(app, ui, c, row_w, pinned) {
                                        RowAction::None => {}
                                        RowAction::Menu(at) => menu = Some((at, c.id)),
                                        RowAction::TogglePin => toggle = Some(c.id),
                                        RowAction::Start => {
                                            app.start(c.id);
                                            ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
                                        }
                                    }
                                }
                            });
                            section_footer(ui);
                        });
                    },
                );
                popup_rect = Some(area.response.rect);
                crate::scenario::publish_rect("toolbar_dropdown", area.response.rect);
                // A drag out of the drop-down ends with a release elsewhere: not a click elsewhere.
                let dragging = egui::DragAndDrop::has_payload_of_type::<ToolbarDrag>(ui.ctx());
                if (area.response.clicked_elsewhere() && !lresp.clicked() && !opened && !dragging) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
                }
            }
            px += width;
            // The kit's 1 px divider between groups, inset from the group's top and bottom.
            painter.line_segment([pos2(px, top + 12.0), pos2(px, top + group_height - 12.0)], t.tertiary_border(t.border));
            px += PANEL_GAP;
        }
        if !overflow.is_empty() {
            let at = Rect::from_min_size(pos2(px.min(right - finish_width - 40.0), top + 4.0), vec2(38.0, button_height));
            ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| {
                let response = crate::ui_kit::Menu::button(ui, "»", |ui| {
                    for (panel, cmds) in &overflow {
                        crate::ui_kit::Menu::button(ui, *panel, |ui| {
                            if tab == "SKETCH" && *panel == "CREATE" {
                                sketch_create_menu(app, ui, cmds, &mut menu, &mut toggle);
                                return;
                            }
                            if *panel == "SELECT" {
                                selection_filter(app, ui);
                            }
                            for c in cmds {
                                let info = c.info(&app.session);
                                let response = crate::ui_kit::command_menu_button(
                                    ui,
                                    c.id,
                                    c.label,
                                    c.icon,
                                    &crate::keymap::display_for(app, c.id).unwrap_or_default(),
                                    info.enabled,
                                );
                                crate::scenario::publish_handle(&format!("toolbar:{}", c.id), response.rect.center());
                                if response.clicked() {
                                    app.start(c.id);
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                crate::scenario::publish_target("toolbar:overflow", response.response.rect);
                response.response.on_hover_text(crate::i18n::tr_label("More toolbar panels"));
            });
        }

        // Dragging a command: its icon follows the pointer; a toolbar button dragged off the
        // toolbar (or into the open drop-down) shows it will be removed.
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let over_popup = |p: egui::Pos2| popup_rect.is_some_and(|pr| pr.contains(p));
        let leaves = |p: egui::Pos2| !r.contains(p) || over_popup(p);
        if let (Some(d), Some(p)) = (egui::DragAndDrop::payload::<ToolbarDrag>(ui.ctx()), pointer) {
            let removing = !d.from_menu && leaves(p);
            drag_ghost(ui.ctx(), d.id, p, removing);
            if ui.input(|i| i.pointer.any_released()) && drop.is_none() {
                egui::DragAndDrop::clear_payload(ui.ctx());
                if removing {
                    crate::toolbar_custom::remove(app, &tab, d.id);
                }
            }
        }
        if let Some((id, panel, before)) = drop {
            crate::toolbar_custom::reorder(app, &tab, id, panel, before);
        }
        if let Some(id) = toggle {
            crate::toolbar_custom::toggle(app, &tab, id);
        }
        if let Some((at, id)) = menu {
            ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
            crate::context_menu::open_for(app, at, crate::context_menu::Target::ToolbarCommand { id: id.to_string() });
        }
    });
    if floating {
        let frame = Container::new(ui.ctx(), Surface::Chrome).frame(ui.ctx(), t.toolbar, crate::frame::Side::Left);
        Container::paint_inset(ui.painter(), frame.widget_rect(cards.ribbon), frame);
    } else {
        Container::paint_inset(ui.painter(), chrome.response.rect - frame.outer_margin, frame);
    }
}

/// The chat panel's compact entry point. State stays in agent_chat; the bar only paints it.
pub(crate) fn assistant_button(app: &mut SolveApp, ui: &mut egui::Ui, at: Rect) {
    use crate::agent_chat::AssistantMark;
    let resp = ui.interact(at, ui.id().with("assistant_button"), Sense::click());
    crate::scenario::publish_target("titlebar:assistant", at);
    let mark = crate::agent_chat::assistant_mark(app);
    let tip = crate::agent_chat::assistant_tip(mark);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &tip));
    let state = match mark {
        AssistantMark::Quiet => crate::ui_kit::AssistantEntryState::Quiet,
        AssistantMark::Ready => crate::ui_kit::AssistantEntryState::Ready,
        AssistantMark::Busy => crate::ui_kit::AssistantEntryState::Busy,
        AssistantMark::NeedsAuth => crate::ui_kit::AssistantEntryState::NeedsAuth,
        AssistantMark::Stopped => crate::ui_kit::AssistantEntryState::Stopped,
        AssistantMark::Waiting(count) => crate::ui_kit::AssistantEntryState::Waiting(count),
    };
    crate::ui_kit::assistant_entry(ui, &resp, state, app.agent_chat.open, ui.ctx().global_style().animation_time > 0.0);
    if resp.on_hover_text(tip).clicked() {
        crate::agent_chat::toggle(app);
    }
}

/// What a click on a panel drop-down row asks for.
enum RowAction {
    None,
    Start,
    Menu(egui::Pos2),
    TogglePin,
}

const ROW_H: f32 = 30.0;

const CONSTRAINT_GRID_WIDTH: f32 = 82.0;

/// Registry parameter syntax is not a user-facing description. Reuse existing catalog prose
/// where present; other commands intentionally show only their name and registered shortcut.
fn ribbon_tooltip(app: &SolveApp, response: egui::Response, command: &polykrom_engine::CommandSpec, disabled: Option<&str>) -> egui::Response {
    response.on_hover_ui(|ui| {
        let t = Tokens::get();
        let name = app.tr_label(command.label);
        let name_width =
            ui.painter().layout_no_wrap(name.to_string(), crate::fonts::semibold(crate::theme::Tokens::get().font_heading), t.text).size().x;
        let key_width = crate::keymap::display_for(app, command.id)
            .map(|key| {
                ui.painter().layout_no_wrap(key.to_string(), FontId::monospace(crate::theme::Tokens::get().font_body), t.text_dim).size().x + 24.0
            })
            .unwrap_or(0.0);
        ui.set_width(280.0_f32.max(name_width + key_width + 12.0));
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(name_width, 20.0), Sense::hover());
            ribbon_text(ui, rect.center(), name, t.font_heading, t.text, true);
            if let Some(key) = crate::keymap::display_for(app, command.id) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::Frame::NONE
                        .fill(t.panel_header)
                        .stroke(t.tertiary_border(t.border))
                        .corner_radius(t.radius_sm.saturating_sub(1))
                        .inner_margin(egui::Margin::symmetric(5, 1))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(key).monospace().color(t.text_dim));
                        });
                });
            }
        });
        let description = match command.id {
            "render.scene" => Some(app.tr("render.preview_help")),
            "render.start" => Some(app.tr("render.image_help")),
            _ => None,
        };
        if let Some(description) = description {
            ui.add(egui::Label::new(description).wrap());
        }
        // The tool's own help, when it has one (#2758).
        if let Some(help) = crate::command_help::help(command.id) {
            ui.add(egui::Label::new(egui::RichText::new(help).color(t.text_dim)).wrap());
        }
        if let Some(reason) = disabled {
            ui.add(egui::Label::new(egui::RichText::new(reason).color(t.text_dim)).wrap());
        }
    })
}

fn section_heading(ui: &mut egui::Ui, label: &str, width: f32) {
    let t = Tokens::get();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(6.0, 0.0),
        Align2::LEFT_CENTER,
        crate::i18n::tr_label(label).to_uppercase(),
        crate::fonts::semibold(crate::theme::Tokens::get().font_small),
        t.text_dim,
    );
}

fn section_footer(ui: &mut egui::Ui) {
    ui.separator();
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(crate::i18n::tr("ui.ribbon_pin_footer")).size(crate::theme::Tokens::get().font_small).color(Tokens::get().text_dim),
        )
        .wrap(),
    );
    ui.add_space(4.0);
}

/// Stable row id inside and outside the scrolling list, for keyboard focus.
fn dropdown_row_id(id: &str) -> egui::Id {
    egui::Id::new(("dropdown_row", id))
}

/// How wide a panel drop-down's rows are: the widest label and shortcut, at least 200.
fn dropdown_width(app: &SolveApp, ui: &egui::Ui, cmds: &[&'static polykrom_engine::CommandSpec]) -> f32 {
    let t = Tokens::get();
    let font = FontId::proportional(t.font_button);
    cmds.iter()
        .map(|c| {
            let label = ui.painter().layout_no_wrap(app.tr_label(c.label).to_string(), font.clone(), t.text).size().x;
            let key = crate::keymap::display_for(app, c.id)
                .map(|k| ui.painter().layout_no_wrap(k.to_string(), FontId::monospace(t.font_button), t.text).size().x + 24.0)
                .unwrap_or(0.0);
            6.0 + 20.0 + 8.0 + label + key + 30.0
        })
        .fold(200.0, f32::max)
}

/// One command row of a panel drop-down: icon, label and shortcut, with the hover background
/// under the pointer or keyboard focus (like every menu).
fn dropdown_row(app: &SolveApp, ui: &mut egui::Ui, c: &'static polykrom_engine::CommandSpec, width: f32, pinned: bool) -> RowAction {
    let t = Tokens::get();
    let info = c.info(&app.session);
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
    // Every row drags onto the toolbar (to pin it there); enabled rows also start their command.
    let sense = if info.enabled { Sense::click_and_drag() } else { Sense::drag() };
    let resp = ui.interact(rect, dropdown_row_id(c.id), sense);
    // Focus can settle on the following frame, after the key event has gone.
    // Keep the focused row visible even then in a drop-down taller than the window.
    if resp.has_focus() && !ui.clip_rect().contains_rect(rect) {
        resp.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
    }
    if resp.drag_started() {
        resp.dnd_set_drag_payload(ToolbarDrag { id: c.id, from_menu: true });
    }
    // The pin at the row's end (above the row, so it takes its own clicks).
    let pr = Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(20.0, 20.0));
    let pin = ui.interact(pr, ui.id().with(("dropdown_pin", c.id)), Sense::click());
    let pointer_on = (resp.hovered() || pin.hovered()) && !egui::DragAndDrop::has_any_payload(ui.ctx());
    let label = app.tr_label(c.label).to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && info.enabled, &label));
    crate::scenario::publish_handle(&format!("toolbar:{}", c.id), rect.center());
    crate::scenario::publish_handle(&format!("dropdown:{}", c.id), rect.center());
    let hot = info.enabled && (pointer_on || resp.has_focus());
    let mut row = crate::ui_kit::MenuItem::new(c.id, &label, c.icon);
    row.enabled = info.enabled;
    row.highlighted = hot;
    let shortcut = crate::keymap::display_for(app, c.id).unwrap_or_default();
    row.shortcut = &shortcut;
    row.paint(ui, rect, &resp, 26.0);
    if hot {
        crate::scenario::publish_handle(&format!("dropdown_hot:{}", c.id), rect.center());
    }
    let p = ui.painter();
    if pointer_on || resp.has_focus() {
        if pin.hovered() {
            p.rect_filled(pr, t.radius_sm.saturating_sub(1), t.border.gamma_multiply(0.6));
        }
        pin_icon(p, pr, pinned, if pinned { t.accent } else { t.text_dim });
        crate::scenario::publish_handle(&format!("dropdown_pin:{}", c.id), pr.center());
    }
    let tip = if pinned { "Remove from Toolbar" } else { "Pin to Toolbar" };
    if pin.on_hover_text(tip).clicked() {
        return RowAction::TogglePin;
    }
    // A disabled row says why; a documented command also shows its help (#2758).
    let reason = info.disabled_reason.as_deref().filter(|_| !info.enabled);
    let resp = match crate::command_help::row_tip(c.id, &label, c.shortcut, reason) {
        Some(tip) => crate::command_help::tooltip(resp, c.id, tip),
        None => resp,
    };
    if resp.secondary_clicked() {
        return RowAction::Menu(resp.interact_pointer_pos().unwrap_or(rect.left_bottom()));
    }
    if resp.clicked() && info.enabled {
        return RowAction::Start;
    }
    RowAction::None
}

/// Creation menus keep related variants together, including collapsed and overflow panels.
fn sketch_create_menu(
    app: &mut SolveApp,
    ui: &mut egui::Ui,
    cmds: &[&'static polykrom_engine::CommandSpec],
    menu: &mut Option<(egui::Pos2, &'static str)>,
    toggle: &mut Option<&'static str>,
) {
    let room = (ui.ctx().content_rect().bottom() - ui.cursor().top() - 64.0).max(ROW_H * 3.0);
    egui::ScrollArea::vertical().id_salt("sketch_create_rows").max_height(room).show(ui, |ui| {
        for f in crate::sketch_families::FAMILIES {
            let variants: Vec<_> = f.commands.iter().filter_map(|id| cmds.iter().find(|c| c.id == *id).copied()).collect();
            if !variants.is_empty() {
                let response = crate::ui_kit::Menu::button(ui, app.tr_label(f.label).to_string(), |ui| {
                    dropdown_keys(app, ui, &variants);
                    sketch_menu_rows(app, ui, &variants, menu, toggle);
                });
                crate::scenario::publish_handle(&format!("create_family:{}", f.label), response.response.rect.center());
            }
        }
        ui.separator();
        let rest: Vec<_> = cmds.iter().copied().filter(|c| crate::sketch_families::family(c.id).is_none()).collect();
        dropdown_keys(app, ui, &rest);
        sketch_menu_rows(app, ui, &rest, menu, toggle);
    });
}

/// Family popups use the same ordered arrow-key navigation as a panel's command list.
fn dropdown_keys(app: &SolveApp, ui: &egui::Ui, cmds: &[&'static polykrom_engine::CommandSpec]) {
    let (down, up) = ui.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp)));
    if !down && !up {
        return;
    }
    // A submenu renders before the remaining rows of its parent. Handle a key once, but
    // leave the event available for the focused row's scroll-into-view behaviour.
    let handled = egui::Id::new("sketch_menu_key_pass");
    let pass = ui.ctx().cumulative_pass_nr();
    if ui.ctx().data(|d| d.get_temp::<u64>(handled)) == Some(pass) {
        return;
    }
    ui.ctx().data_mut(|d| d.insert_temp(handled, pass));
    let enabled: Vec<_> = cmds.iter().filter(|c| c.info(&app.session).enabled).collect();
    if enabled.is_empty() {
        return;
    }
    let focused = ui.ctx().memory(|m| m.focused());
    let at = enabled.iter().position(|c| focused == Some(dropdown_row_id(c.id)));
    let n = enabled.len();
    let next = match (at, down) {
        (None, true) => 0,
        (None, false) => n - 1,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    };
    if let Some(c) = enabled.get(next) {
        ui.ctx().memory_mut(|m| {
            m.request_focus(dropdown_row_id(c.id));
            m.move_focus(egui::FocusDirection::None);
        });
    }
}

fn sketch_menu_rows(
    app: &mut SolveApp,
    ui: &mut egui::Ui,
    cmds: &[&'static polykrom_engine::CommandSpec],
    menu: &mut Option<(egui::Pos2, &'static str)>,
    toggle: &mut Option<&'static str>,
) {
    let width = dropdown_width(app, ui, cmds);
    for c in cmds {
        let pinned = crate::toolbar_custom::pinned_on(app, "SKETCH", c.id).is_some();
        match dropdown_row(app, ui, c, width, pinned) {
            RowAction::None => {}
            RowAction::Menu(at) => *menu = Some((at, c.id)),
            RowAction::TogglePin => *toggle = Some(c.id),
            RowAction::Start => {
                app.start(c.id);
                ui.ctx().data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sc_panel_open")));
                ui.close();
            }
        }
    }
}

/// A command being dragged: a toolbar button, or a row of a panel drop-down (`from_menu`).
#[derive(Clone, Copy, Debug)]
struct ToolbarDrag {
    id: &'static str,
    from_menu: bool,
}

/// The dragged command's icon under the pointer, crossed out when releasing removes it.
fn drag_ghost(ctx: &egui::Context, id: &str, at: egui::Pos2, removing: bool) {
    let t = Tokens::get();
    let Some(c) = polykrom_engine::find_command(id) else { return };
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("sc_toolbar_drag")));
    let r = Rect::from_center_size(at + vec2(14.0, 16.0), vec2(30.0, 30.0));
    p.rect(r, t.radius_sm, t.toolbar.gamma_multiply(0.92), Stroke::new(1.0, if removing { t.error } else { t.accent }), egui::StrokeKind::Inside);
    icons::paint_icon(&p, r.shrink(5.0), c.icon, f32::from(t.icon.a()) / 255.0);
    if removing {
        let s = Stroke::new(2.0, t.error);
        let x = Rect::from_center_size(r.right_top(), vec2(9.0, 9.0));
        p.circle_filled(x.center(), 7.0, t.toolbar);
        crate::icons::glyphs::cross(&p, x, s);
    }
    crate::scenario::publish_handle(if removing { "toolbar_drag:remove" } else { "toolbar_drag" }, at);
}

/// The SELECT panel: what a click in the view picks.
pub(crate) fn selection_filter(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    ui.label(egui::RichText::new(crate::i18n::tr_label("Selection filter")).color(t.text_dim).size(t.font_small));
    crate::ui_kit::checkbox(ui, crate::i18n::tr_label("Select bodies (a face picks its body)"), &mut app.ui.pick_bodies);
    for (k, label) in [
        ("faces", "Faces"),
        ("edges", "Edges"),
        ("vertices", "Vertices"),
        ("sketch", "Sketch geometry and profiles"),
        ("sketch_curves", "Sketch curves"),
        ("sketch_points", "Sketch points"),
        ("dimensions", "Sketch dimensions"),
        ("constraints", "Sketch constraints"),
    ] {
        let mut on = !app.ui.pick_off.iter().any(|x| x == k);
        if crate::ui_kit::checkbox(ui, crate::i18n::tr_label(label), &mut on).changed() {
            app.ui.pick_off.retain(|x| x != k);
            if !on {
                app.ui.pick_off.push(k.to_string());
            }
        }
    }
}

/// Two Esc presses within this many seconds finish the open sketch.
pub const ESCAPE_TWICE_S: f64 = 0.4;

/// Global keyboard shortcuts (when no text field has focus).
pub fn shortcuts(app: &mut SolveApp, ctx: &egui::Context) {
    if design_shown(app) {
        crate::navigation::step_key_hold(app, ctx);
    } else {
        app.viewport.key_hold = None;
    }
    if app.file_load.busy() {
        return;
    }
    // The Keyboard Shortcuts window is waiting for a key: it is not a shortcut.
    if crate::keymap::capturing(app) {
        // Parameters is drawn before Preferences; do not let its footer see this Escape.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            crate::keymap::stop_capture(app);
            app.esc_handled = true;
        }
        return;
    }
    if crate::params_dialog::cancel_on_escape(app, ctx) {
        return;
    }
    // A dimension value that is taking the keyboard: letters belong to it (a parameter name
    // starting with L or D), even on the frame before its field holds the focus.
    if egui::Popup::is_any_open(ctx) || ctx.egui_wants_keyboard_input() || crate::dim_view::editing() || crate::dialogs_save_mesh::popup_open(ctx) {
        return;
    }
    let (keys, mods) = ctx.input(|i| {
        let keys: Vec<(egui::Key, bool)> = i
            .events
            .iter()
            .filter_map(|e| if let egui::Event::Key { key, pressed: true, repeat, .. } = e { Some((*key, *repeat)) } else { None })
            .collect();
        (keys, i.modifiers)
    });
    // Home hides the design: only New, Open and Preferences work there.
    let shown = design_shown(app);
    if shown {
        crate::edit_clipboard::handle(app, ctx);
    } else {
        crate::edit_clipboard::flush(ctx);
    }
    for (k, repeat) in keys {
        use egui::Key;
        // Only Esc pressed twice in a row finishes a sketch: any other key in between resets it.
        if k != Key::Escape {
            app.last_escape = None;
        }
        if shown && crate::navigation::view_key(app, ctx, k, mods, repeat) {
            continue;
        }
        if mods.command {
            match k {
                Key::N => crate::documents::new_design(app),
                Key::Comma => crate::prefs::open_at(app, 0),
                Key::O => {
                    app.pick_open(|app, path| {
                        if let Some(path) = path {
                            app.open_path_async(&path);
                        }
                    });
                }
                _ if !shown => {}
                Key::Z => drop(app.run("edit.undo", json!({}))),
                Key::Y => drop(app.run("edit.redo", json!({}))),
                // Copy, Cut and Paste (`edit_clipboard::handle`).
                Key::C | Key::X | Key::V if !mods.shift && !mods.alt => {}
                Key::S => save(app),
                _ => crate::keymap::press(app, &crate::keymap::key_name(k, mods), repeat),
            }
            continue;
        }
        if !shown {
            continue;
        }
        match k {
            Key::Escape => {
                app.esc_handled = true;
                // End navigation before the underlying tool or dialog; sketch double-Escape
                // handling starts with the next press.
                if app.viewport.nav.take().is_some() {
                    app.last_escape = None;
                    continue;
                }
                // Two quick presses in a sketch finish it (keeping it, like Finish Sketch);
                // the first one has already cancelled whatever was active.
                let now = ctx.input(|i| i.time);
                let again = app.last_escape.is_some_and(|t| now - t <= ESCAPE_TWICE_S);
                app.last_escape = Some(now);
                // Not while a tool is still out (its first Esc only ended a chain): this press
                // puts it down, and the next quick one finishes.
                if again
                    && app.tool.is_none()
                    && app.dialog.is_none()
                    && let Some(sketch) = app.session.active_sketch
                {
                    app.last_escape = None;
                    // A sketch left blank is taken away instead, in one undo step, so no empty
                    // sketch stays behind.
                    let blank = app.session.doc.sketch(sketch).is_ok_and(crate::sketch_palette::is_blank);
                    let depth = app.session.undo.len();
                    app.finish_sketch();
                    if blank && app.session.active_sketch.is_none() && app.run("timeline.delete", json!({ "feature": sketch.to_string() })).is_ok() {
                        app.session.undo.truncate(depth + 1);
                    }
                    continue;
                }
                if crate::sketch_text_move::cancel_drag(app) || crate::occurrence_drag::cancel(app) {
                    continue;
                }
                if crate::select_other::is_open(app) {
                    crate::select_other::close(app);
                } else if app.tool.is_some() {
                    crate::tools::finish(app);
                } else if app.dialog.is_some() {
                    crate::dialogs::cancel(app);
                } else if !app.session.selection.is_empty() || app.session.active_sketch.is_none() {
                    app.tree.picked_joints.clear();
                    let _ = app.run("select.clear", json!({}));
                }
                // A single Esc with nothing left to cancel keeps the sketch open (#1804); a quick
                // second press finishes it (above).
            }
            Key::S if !mods.shift && !mods.alt => {
                app.start("ui.palette");
            }
            Key::V => crate::context_menu::toggle_visibility(app),
            Key::R if mods.shift && !mods.alt => {
                app.repeat_last();
            }
            Key::F2 => {
                let at = ctx.input(|i| i.pointer.latest_pos()).unwrap_or(ctx.content_rect().center());
                crate::context_menu::rename_selection(app, at);
            }
            Key::F6 => app.animate_view("fit"),
            Key::Delete | Key::Backspace => {
                if !crate::timeline_left::delete_selected_folders(app) {
                    crate::viewport::delete_selection(app);
                }
            }
            _ => crate::keymap::press(app, &crate::keymap::key_name(k, mods), repeat),
        }
    }
}

#[cfg(test)]
mod header_report_tests {
    use super::*;

    #[test]
    fn native_file_menu_replaces_window_button_and_preserves_fallback() {
        let mut h = crate::scenario::Harness::new();
        h.frames(3);
        assert!(crate::scenario::handle_at("appbar:file").is_some());
        h.ctx.data_mut(|data| data.insert_temp(egui::Id::new("sc_file_menu"), true));
        h.app.native_file_menu = true;
        h.frames(3);
        assert!(crate::scenario::handle_at("appbar:file").is_none());
        assert!(!h.ctx.data(|data| data.get_temp::<bool>(egui::Id::new("sc_file_menu")).unwrap_or(false)));
        assert!(crate::scenario::handle_at("appbar:save").is_some());
        assert!(crate::scenario::handle_at("appbar:undo").is_none());
        assert!(crate::scenario::handle_at("appbar:redo").is_none());
        h.app.native_file_menu = false;
        h.frames(3);
        assert!(crate::scenario::handle_at("appbar:file").is_some());
        assert!(crate::scenario::handle_at("appbar:undo").is_some());
        assert!(crate::scenario::handle_at("appbar:redo").is_some());
    }

    #[test]
    fn native_menu_actions_share_undo_and_redo_history() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        assert!(!FileAction::Undo.enabled(&h.app));
        assert!(!FileAction::Redo.enabled(&h.app));
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        let depth = h.app.session.undo.len();
        assert!(FileAction::Undo.enabled(&h.app));
        FileAction::Undo.run(&mut h.app);
        assert_eq!(h.app.session.undo.len(), depth - 1);
        assert!(FileAction::Redo.enabled(&h.app));
        FileAction::Redo.run(&mut h.app);
        assert_eq!(h.app.session.undo.len(), depth);
        assert!(!FileAction::Redo.enabled(&h.app));
    }

    #[test]
    fn native_menu_actions_share_settings_and_unsaved_quit() {
        let mut h = crate::scenario::Harness::new();
        FileAction::Settings.run(&mut h.app);
        assert!(h.app.prefs_window.open);
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        FileAction::Quit.run(&mut h.app);
        assert!(!h.app.quit_requested);
        assert!(h.app.docs.quitting.is_some());
    }

    #[test]
    fn native_menu_actions_disable_save_on_home() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = true;
        assert!(!FileAction::Save.enabled(&h.app));
        assert!(!FileAction::SaveAs.enabled(&h.app));
        assert!(FileAction::New.enabled(&h.app));
        assert!(FileAction::Open.enabled(&h.app));
        assert!(FileAction::Settings.enabled(&h.app));
        assert!(!FileAction::Export("step").enabled(&h.app));
        assert!(!FileAction::ExportMesh("stl").enabled(&h.app));
        assert!(!FileAction::Command("document.metadata").enabled(&h.app));
        h.app.home.open = false;
        assert!(FileAction::Save.enabled(&h.app));
        assert!(FileAction::Export("step").enabled(&h.app));
    }

    #[test]
    fn native_menu_actions_share_json_save_and_recent_cleanup() {
        let mut h = crate::scenario::Harness::new();
        let path = h.dir.join("native-menu.json");
        let output = path.clone();
        h.app.services.pick_save = Some(Box::new(move |_, _| Box::pin(std::future::ready(Some(output.to_string_lossy().into_owned())))));
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        FileAction::SaveJson.run(&mut h.app);
        h.frames(3);
        h.settle();
        let bytes = std::fs::read(path).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&bytes).is_ok());
        assert!(!h.app.home.recent.is_empty());
        FileAction::ClearRecent.run(&mut h.app);
        assert!(h.app.home.recent.is_empty());
        assert!(h.app.home.opened_at.is_empty());
        assert!(!FileAction::ClearRecent.enabled(&h.app));
    }

    #[test]
    fn file_menu_json_save_and_export_use_commands_and_keep_distinct_paths() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        let dir = h.dir.clone();
        h.app.services.pick_save =
            Some(Box::new(move |name, _extensions| Box::pin(std::future::ready(Some(dir.join(name).to_string_lossy().into_owned())))));
        h.frames(3);
        h.step(&json!({"click":{"handle":"appbar:folder"}})).unwrap();
        h.step(&json!({"widget":"Save As JSON…"})).unwrap();
        // Written on a worker: the window keeps drawing until it is done.
        h.settle();
        let saved = h.app.session.path.clone().unwrap();
        let bytes = std::fs::read(&saved).unwrap();
        assert!(!polykrom_engine::io::is_design_container(&bytes));
        assert_eq!(polykrom_engine::io::read_design(&bytes).unwrap(), *h.app.session.doc);
        h.app.session.doc_mut().name = "Exported copy".into();
        h.frames(3);
        h.step(&json!({"click":{"handle":"appbar:folder"}})).unwrap();
        h.step(&json!({"widget":"Export Design as JSON…"})).unwrap();
        assert_eq!(h.app.session.path.as_deref(), Some(saved.as_str()));
        assert!(h.dir.join("Exported copy.json").is_file());
        h.app.run("file.save", json!({})).unwrap();
        assert!(!polykrom_engine::io::is_design_container(&std::fs::read(saved).unwrap()), "subsequent Save keeps a .json choice");
    }

    /// File › Export STL… and Export 3MF… both open the Save as mesh dialog with their format
    /// chosen (whatever was used last), and command search finds the export by format or purpose.
    #[test]
    fn file_menu_has_export_stl_and_3mf_that_open_the_mesh_dialog_with_that_format() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        h.app.preferences.save_mesh_format = "3mf".into();
        for (item, format) in [("Export STL…", "stl"), ("Export 3MF…", "3mf")] {
            h.frames(3);
            h.step(&json!({"click":{"handle":"appbar:file"}})).unwrap();
            h.step(&json!({"widget": item})).unwrap();
            let Some(crate::dialogs::Kind::SaveMesh(k)) = h.app.dialog.as_ref().map(|d| &d.kind) else { panic!("{item}: no Save as mesh dialog") };
            assert_eq!(k.format_name(), format, "{item}");
            crate::dialogs::cancel(&mut h.app);
        }
        for q in ["3mf", "stl", "export 3mf", "print"] {
            let found = crate::shortcut_box::search(&h.app, q);
            assert!(found.iter().any(|r| r.0 == "file.save_mesh"), "{q}: {:?}", found.iter().map(|r| r.0).collect::<Vec<_>>());
        }
    }

    /// The browser build's File > Download design hands the open design to the host as a
    /// `.solidcraft` container named after it; a host that saves real files shows no such item.
    #[test]
    fn file_menu_download_design_hands_the_container_to_the_host() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        h.app.session.doc_mut().name = "Plate".into();
        let got = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(String, Vec<u8>)>::new()));
        let sink = got.clone();
        h.app.services.download_design = Some(Box::new(move |name, bytes| {
            sink.borrow_mut().push((name.to_string(), bytes.to_vec()));
            Ok(())
        }));
        h.frames(3);
        h.step(&json!({"click":{"handle":"appbar:folder"}})).unwrap();
        h.step(&json!({"widget":"Download design"})).unwrap();
        let got = got.borrow();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "Plate.polykrom");
        assert!(polykrom_engine::io::is_design_container(&got[0].1));
        assert!(h.app.session.path.is_none(), "a download is no save: the design keeps no file");
    }

    fn sizes(buttons: &[usize]) -> Vec<PanelSize> {
        buttons
            .iter()
            .map(|&b| PanelSize { buttons: vec![ButtonSize { width: BUTTON_MIN_WIDTH, min_width: BUTTON_MIN_WIDTH }; b], secondary_from: usize::MAX })
            .collect()
    }

    fn used_with_more(plans: &[Plan]) -> f32 {
        used(plans) + if plans.iter().any(|p| p.fit == Fit::More) { MORE_ROOM } else { 0.0 }
    }

    #[test]
    fn panels_give_up_one_button_at_a_time_then_move_to_more_and_none_is_dropped() {
        // A panel with 14 buttons and three default ones.
        let panels = sizes(&[14, 4, 2, 2]);
        let full: f32 = [14, 4, 2, 2].iter().map(|&b| panel_width(&vec![BUTTON_MIN_WIDTH; b]) + PANEL_GAP).sum();
        let all = |plans: &[Plan]| plans.iter().map(|p| (p.fit, p.shown)).collect::<Vec<_>>();
        assert_eq!(all(&fit(&panels, full)), [(Fit::Buttons, 14), (Fit::Buttons, 4), (Fit::Buttons, 2), (Fit::Buttons, 2)]);
        // A little short: the widest panel hands one button to its drop-down; the others keep all theirs.
        assert_eq!(all(&fit(&panels, full - 10.0)), [(Fit::Buttons, 13), (Fit::Buttons, 4), (Fit::Buttons, 2), (Fit::Buttons, 2)]);
        // Every panel down to one button (collapsed) fits.
        let collapsed = 4.0 * (COLLAPSED_WIDTH + PANEL_GAP);
        assert_eq!(all(&fit(&panels, collapsed)), [(Fit::Collapsed, 1); 4]);
        // Narrower: the last one goes into the » menu, and the room it frees goes back to buttons.
        let more = fit(&panels, collapsed - 1.0);
        assert_eq!(more.iter().map(|p| p.fit).collect::<Vec<_>>(), [Fit::Collapsed, Fit::Collapsed, Fit::Collapsed, Fit::More]);
        assert!(more.iter().take(3).all(|p| p.shown >= 1));
        let more = fit(&sizes(&[14, 4, 2, 2, 2, 2]), 5.0 * (COLLAPSED_WIDTH + PANEL_GAP) + MORE_ROOM - 1.0);
        assert_eq!(more.last().unwrap().fit, Fit::More);
        assert!(more.iter().any(|p| p.fit == Fit::Buttons && p.shown > 1), "overflow room restores buttons");
        // Then all of them.
        assert!(fit(&panels, 0.0).iter().all(|p| p.fit == Fit::More));
        assert_eq!(fit(&panels, f32::NAN).len(), 4);
        // No cap: every additional button keeps its width and gap.
        assert_eq!(panel_width(&[BUTTON_MIN_WIDTH; 30]) - panel_width(&[BUTTON_MIN_WIDTH; 20]), 10.0 * (BUTTON_MIN_WIDTH + BUTTON_GAP));
    }

    /// #2108: whatever the room, the toolbar uses it. It never leaves more than one button's width
    /// blank while something is hidden, never runs past the room, and shrinks steadily as the
    /// room does (so a few pixels never flip it back and forth).
    #[test]
    fn panels_fill_the_room_they_have_at_every_width() {
        let layouts = [
            // SOLID: CREATE, MODIFY, ASSEMBLE, CONFIGURE, CONSTRUCT, INSPECT, INSERT, SELECT.
            vec![(4, false), (4, false), (3, false), (2, false), (2, false), (2, false), (2, false), (0, false)],
            // SKETCH: labelled CREATE, then MODIFY … FINISH SKETCH.
            vec![(8, true), (2, false), (3, false), (6, false), (2, false), (2, false), (0, false), (1, false)],
            // Many pins on one panel.
            vec![(14, false), (4, false), (2, false), (2, false)],
        ];
        for layout in layouts {
            let panels: Vec<PanelSize> = layout
                .iter()
                .map(|&(buttons, labelled)| PanelSize {
                    buttons: (0..buttons)
                        .map(|i| ButtonSize {
                            width: if labelled { BUTTON_MAX_WIDTH - (i % 3) as f32 * 5.0 } else { BUTTON_MIN_WIDTH },
                            min_width: BUTTON_MIN_WIDTH,
                        })
                        .collect(),
                    secondary_from: usize::MAX,
                })
                .collect();
            let mut last: Option<(f32, usize)> = None;
            for room in (0..=2000).rev().map(|r| r as f32) {
                let plans = fit(&panels, room);
                let width = used_with_more(&plans);
                let hidden = plans
                    .iter()
                    .zip(&panels)
                    .any(|(p, s)| p.fit != Fit::Buttons || p.shown < s.buttons.len() || p.widths.iter().zip(&s.buttons).any(|(w, b)| *w < b.width));
                let more = plans.iter().filter(|p| p.fit == Fit::More).count();
                if more < panels.len() {
                    assert!(width <= room + 0.01, "{layout:?} at {room}: {width} overruns");
                }
                // One button: an icon's stride, or a collapsed panel's when the next panel waits in the » menu.
                let button = if more > 0 { COLLAPSED_WIDTH + PANEL_GAP } else { BUTTON_MIN_WIDTH + BUTTON_GAP };
                if hidden && more < panels.len() {
                    assert!(room - width < button, "{layout:?} at {room}: {} px blank while something is hidden", room - width);
                }
                // Fewer pixels never bring a panel back out of the » menu.
                if let Some((_, prev_more)) = last {
                    assert!(more >= prev_more, "{layout:?} at {room}: a panel came back out of the » menu");
                }
                last = Some((width, more));
                // A panel shows its icons: one at the very least.
                for (p, s) in plans.iter().zip(&panels) {
                    if p.fit != Fit::More && !s.buttons.is_empty() {
                        assert!(p.shown >= 1, "{layout:?} at {room}: a panel without icons");
                    }
                }
            }
        }
    }

    /// Secondary columns (a group's other commands as small buttons) leave before any pinned
    /// button on any panel does, the panel showing the most of them first.
    #[test]
    fn secondary_columns_give_way_before_pinned_buttons() {
        let pinned = |n: usize| vec![ButtonSize { width: 70.0, min_width: BUTTON_MIN_WIDTH }; n];
        let column = ButtonSize { width: 100.0, min_width: 100.0 };
        let mut a = PanelSize { buttons: pinned(3), secondary_from: 3 };
        a.buttons.extend([column; 2]);
        let mut b = PanelSize { buttons: pinned(4), secondary_from: 4 };
        b.buttons.push(column);
        let panels = vec![a, b];
        let full = used(&panels.iter().map(Plan::full).collect::<Vec<_>>());
        let shown = |room: f32| fit(&panels, room).iter().map(|p| p.shown).collect::<Vec<_>>();
        assert_eq!(shown(full), [5, 5]);
        // A column short: a secondary column of the panel with the most (the first) goes first,
        // before any pinned button leaves.
        assert_eq!(shown(full - 101.0), [4, 5]);
        // Narrower still: the other panel's column goes too.
        assert_eq!(shown(full - 101.0 - 101.0), [4, 4]);
        // Only with every secondary column gone do the pinned buttons start to leave, from the
        // panel showing the most.
        let pinned_min = panel_width(&[BUTTON_MIN_WIDTH; 3]) + panel_width(&[BUTTON_MIN_WIDTH; 4]) + 2.0 * PANEL_GAP;
        assert_eq!(shown(pinned_min), [3, 4]);
        assert_eq!(shown(pinned_min - 1.0), [3, 3], "only then a pinned button of the fullest panel");
    }

    #[test]
    fn variable_width_buttons_narrow_before_rightmost_equal_panels_lose_buttons() {
        let panels = vec![PanelSize {
            buttons: vec![
                ButtonSize { width: 76.0, min_width: BUTTON_MIN_WIDTH },
                ButtonSize { width: 63.0, min_width: BUTTON_MIN_WIDTH },
                ButtonSize { width: 54.0, min_width: BUTTON_MIN_WIDTH },
            ],
            secondary_from: usize::MAX,
        }];
        let full = used(&panels.iter().map(Plan::full).collect::<Vec<_>>());
        let narrow = fit(&panels, full - 1.0);
        assert_eq!(narrow[0].shown, 3);
        assert_eq!(narrow[0].widths, [75.0, 62.0, 54.0]);
        let minimum = panel_width(&[BUTTON_MIN_WIDTH; 3]) + PANEL_GAP;
        let narrow = fit(&panels, minimum);
        assert_eq!(narrow[0].shown, 3);
        assert_eq!(narrow[0].widths, [BUTTON_MIN_WIDTH; 3]);
        assert_eq!(fit(&panels, minimum - 1.0)[0].shown, 2);

        let equal = sizes(&[3, 3]);
        let full = used(&equal.iter().map(Plan::full).collect::<Vec<_>>());
        assert_eq!(fit(&equal, full - 1.0).iter().map(|p| p.shown).collect::<Vec<_>>(), [3, 2]);
    }

    #[test]
    fn constraint_gallery_stays_fixed_and_hands_all_cells_to_the_menu_together() {
        let panels = vec![PanelSize {
            buttons: vec![
                ButtonSize { width: BUTTON_MAX_WIDTH, min_width: BUTTON_MIN_WIDTH },
                ButtonSize { width: CONSTRAINT_GRID_WIDTH, min_width: CONSTRAINT_GRID_WIDTH },
            ],
            secondary_from: usize::MAX,
        }];
        let minimum = panel_width(&[BUTTON_MIN_WIDTH, CONSTRAINT_GRID_WIDTH]) + PANEL_GAP;
        let narrow = fit(&panels, minimum);
        assert_eq!(narrow[0].shown, 2);
        assert_eq!(narrow[0].widths, [BUTTON_MIN_WIDTH, CONSTRAINT_GRID_WIDTH]);
        let collapsed = fit(&panels, minimum - 1.0);
        assert_eq!((collapsed[0].fit, collapsed[0].shown), (Fit::Collapsed, 1));
    }

    fn captured_frame(h: &mut crate::scenario::Harness) {
        let token = h.app.shot_token;
        crate::issue_report::receive(&mut h.app, token, std::sync::Arc::new(egui::ColorImage::new([2, 2], vec![Color32::GREEN; 4])));
        h.frames(3);
    }

    /// The 48 px bar (the kit's document toolbar): the logo alone at the left, as the application
    /// menu's button; the document's name with its storage and revision line; and Undo, Redo,
    /// command search, Save and Export at the right. Help, Preferences, Report a bug, the version
    /// and the start page left the bar for the status bar and the menu.
    #[test]
    fn title_bar_keeps_the_logo_left_and_the_actions_right() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.custom_titlebar = true;
        h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
        h.call("ui.resize", json!({"width":1400,"height":720}));
        h.frames(3);
        for gone in ["appbar:sun", "appbar:moon", "toolbar:ai", "titlebar:separator:brand", "titlebar:separator:documents"] {
            assert!(crate::scenario::handle_at(gone).is_none(), "{gone} is no longer in the title bar");
        }
        let rect = |name: &str| crate::scenario::rect_of(name).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(APP_BAR_HEIGHT, 48.0);
        let logo = rect("appbar:logo");
        assert_eq!(logo.size(), vec2(32.0, 32.0));
        assert_eq!(logo.left(), BAR_INSET);
        assert_eq!(logo.center().y, APP_BAR_HEIGHT / 2.0);
        assert_eq!(rect("appbar:file"), logo, "the logo is the application menu's button");
        assert!(h.widget("Application menu").is_some(), "the logo button has an accessible name");
        let identity = rect("appbar:document");
        assert!(identity.left() > logo.right() + 8.0);
        assert!(identity.top() >= 0.0 && identity.bottom() <= APP_BAR_HEIGHT);
        // The right-hand group, left to right, inside the bar and clear of the caption buttons.
        let (undo, redo, search, save, export) =
            (rect("appbar:undo"), rect("appbar:redo"), rect("ribbon:search"), rect("titlebar:save"), rect("titlebar:export"));
        assert!(identity.right() < undo.left());
        assert!(undo.right() <= redo.left() && redo.right() <= search.left() && search.right() <= save.left() && save.right() <= export.left());
        assert_eq!(export.right(), 1400.0 - crate::titlebar::WIDTH - BAR_INSET);
        for r in [undo, redo, save, export] {
            assert_eq!(r.height(), BAR_BUTTON_HEIGHT);
            assert!(r.top() >= 0.0 && r.bottom() <= APP_BAR_HEIGHT);
        }
        assert!(search.center().y == APP_BAR_HEIGHT / 2.0 && search.top() > 0.0);
        assert!(h.widget("Save").is_some() && h.widget("Export").is_some(), "Save and Export carry their labels");
        for name in ["appbar:help", "titlebar:preferences", "titlebar:report", "titlebar:version", "titlebar:home"] {
            assert!(rect(name).top() > APP_BAR_HEIGHT, "{name} left the title bar");
        }
        // The Export button lists every export, as the menu does.
        h.step(&json!({"click":{"handle":"titlebar:export"}})).unwrap();
        for item in ["Export STL…", "Export 3MF…", "Export STEP…", "Export OBJ…", "Export URDF…", "Export Design as JSON…"] {
            assert!(h.widget(item).is_some(), "{item} under the Export button");
        }
        h.step(&json!({"key":"Escape"})).unwrap();
        h.frames(2);
        assert!(h.widget("Export STEP…").is_none(), "Escape closes the export menu");
        // The application menu from the logo holds the start page, the File items, Workspaces,
        // Preferences, Keyboard Shortcuts, Help and Quit, with the version at its foot.
        h.step(&json!({"click":{"handle":"appbar:file"}})).unwrap();
        for item in [
            "Start page",
            "New Design",
            "Open…",
            "Save",
            "Save As…",
            "Export STEP…",
            "Design Properties…",
            "Workspace…",
            "Preferences…",
            "Keyboard Shortcuts…",
            "Help",
            "Quit",
        ] {
            assert!(h.widget(item).is_some(), "{item} in the application menu");
        }
        h.step(&json!({"widget":"Start page"})).unwrap();
        assert!(h.app.home.open, "the start page is reachable from the application menu");
        let dark = h.app.ui.dark;
        h.call("ui.prefs", json!({"theme": if dark { "light" } else { "dark" }}));
        h.frames(2);
        assert_ne!(h.app.ui.dark, dark, "the theme is a preference");
    }

    /// The subtitle under the document's name comes from the session: where the design is
    /// stored and its revision.
    #[test]
    fn document_subtitle_says_where_the_design_is_and_its_revision() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        let revision = h.app.session.revision;
        assert_eq!(document_subtitle(&h.app), format!("Not saved yet · rev {revision}"));
        h.app.session.path = Some(h.dir.join("designs").join("plate.polykrom").to_string_lossy().into_owned());
        assert_eq!(document_subtitle(&h.app), format!("Local file · designs · rev {revision}"));
        h.app.session.path = Some("plate.polykrom".into());
        assert_eq!(document_subtitle(&h.app), format!("Local file · rev {revision}"));
    }

    /// Narrow bars keep every button: Save and Export drop their text, the search field shrinks.
    #[test]
    fn narrow_title_bars_keep_every_action() {
        for width in [500.0, 700.0, 900.0, 1200.0] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.app.custom_titlebar = true;
            h.call("ui.resize", json!({"width":width,"height":720}));
            h.frames(3);
            let rect_of = crate::scenario::rect_of;
            let rect = |name: &str| rect_of(name).unwrap_or_else(|| panic!("{name} at {width}"));
            let rects = ["appbar:logo", "appbar:undo", "appbar:redo", "titlebar:save", "titlebar:export"].map(rect);
            for pair in rects.windows(2) {
                assert!(pair[0].right() <= pair[1].left(), "{width}: {:?} overlaps {:?}", pair[0], pair[1]);
            }
            assert!(rects[4].right() <= width - crate::titlebar::WIDTH);
            let labelled = width - crate::titlebar::WIDTH - BAR_INSET >= BAR_LABELS_MIN_WIDTH;
            assert_eq!(rects[3].width() > 32.0, labelled, "{width}: Save keeps its text only in a wide bar: {:?}", rects[3]);
            assert!(rect_of("ribbon:search").is_some_and(|r| r.width() >= SEARCH_MIN_WIDTH), "{width}: the search field stays");
        }
    }

    #[test]
    fn header_report_actual_click_opens_capture_without_changing_healthy_design() {
        for width in [450.0, 700.0, 900.0, 1400.0] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.app.custom_titlebar = true;
            h.app.run("solid.box", json!({"length":35,"width":12,"height":4})).unwrap();
            h.call("ui.resize", json!({"width":width,"height":720}));
            h.frames(3);
            let before = (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision);
            let point = h.widget("Report a bug").expect("compact and full buttons retain an accessible name");
            h.step(&json!({"click":point})).unwrap();
            assert!(h.app.issue_report.draft.is_some(), "width {width}: header click must reach the report");
            assert!(crate::issue_report::pending(&h.app));
            assert_eq!(before, (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision));
            captured_frame(&mut h);
            // The report card takes a few frames to reach its full height.
            h.frames(16);
            // Existing review content scrolls vertically when the window is narrow.
            if width < 700.0 {
                let [x, y] = h.widget("Close draft").unwrap();
                h.call("ui.scroll", json!({"x":x,"y":y - 60.0,"delta":-1200}));
                // Let the scroll animation finish before locating the click target.
                h.frames(60);
            }
            h.step(&json!({"widget":"Close draft"})).unwrap();
            assert!(h.app.issue_report.draft.is_none(), "width {width}: review closes through its actual button");
            h.step(&json!({"click":{"handle":"tab:home"}})).unwrap();
            assert!(h.app.home.open, "width {width}: the Home tab remains reachable beside Report");
            assert_eq!(before, (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision));
        }
    }

    #[test]
    fn header_report_preserves_new_dimension_draft_and_single_commit() {
        crate::dim_view::cancel();
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.run("sketch.line", json!({"points":[[0,0],[20,0]]})).unwrap();
        h.call("ui.view", json!({"view":"top"}));
        h.app.cam.distance = 80.0;
        h.app.cam.target = polykrom_engine::geom::Vec3::new(10.0, 0.0, 0.0);
        h.settle();
        h.step(&json!({"start":"sketch.dimension"})).unwrap();
        for point in [[0, 0, 0], [20, 0, 0], [10, 5, 0]] {
            h.step(&json!({"click":{"world":point}})).unwrap();
        }
        h.step(&json!({"text":"30 mm"})).unwrap();
        let before = (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision);
        h.step(&json!({"widget":"Report a bug"})).unwrap();
        h.frames(3);
        assert!(h.app.issue_report.draft.is_some() && crate::dim_view::editing());
        captured_frame(&mut h);
        h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("issue_report_body")));
        h.step(&json!({"text":"Describe this dimension problem."})).unwrap();
        h.step(&json!({"key":"Enter"})).unwrap();
        assert_eq!(before, (h.app.session.doc.to_json(), h.app.session.undo.len(), h.app.session.revision));
        h.step(&json!({"widget":"Close draft"})).unwrap();
        h.frames(2);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(egui::Id::new("sc_dim_edit")));
        h.step(&json!({"key":"Enter"})).unwrap();
        assert!(!crate::dim_view::editing());
        assert_eq!(h.app.session.undo.len(), before.1 + 1);
        assert_eq!(h.app.session.doc.param("d1").unwrap().expr, "30 mm");
        h.app.run("edit.undo", json!({})).unwrap();
        assert_eq!(h.app.session.doc.to_json(), before.0);
    }
}

#[cfg(test)]
mod home_tests {
    use super::*;

    /// Home hides the design: its Undo and Redo buttons, the File menu's Insert, Save
    /// and Export, and CAD keyboard shortcuts leave it alone. File-view Save shortcuts
    /// have their own regression coverage; viewport actions work again after returning.
    #[test]
    fn design_actions_are_off_on_home() {
        // A design with history both ways, a selection and unsaved changes, behind Home.
        let on_home = || {
            let mut h = crate::scenario::Harness::new();
            h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Main"})).unwrap();
            h.app.run("solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [20, 0, 0]})).unwrap();
            h.app.run("edit.undo", json!({})).unwrap();
            h.app.session.selection = vec![polykrom_engine::Sel::Body { name: "Main".into() }];
            let rev = h.app.session.revision;
            h.app.home.show(rev);
            h.frames(2);
            h
        };
        let mut h = on_home();
        assert!(!design_shown(&h.app));
        // Everything an action could change: the design, its history and file, files written,
        // the selection and visibility, and whether it left Home or opened a tool, dialog or the command box.
        let state = |h: &crate::scenario::Harness| {
            let files = std::fs::read_dir(&h.dir).map_or(0, |d| d.count());
            let doc = serde_json::to_value(&*h.app.session.doc).unwrap();
            let busy = (h.app.tool.is_some(), h.app.dialog.is_some(), h.app.ui.palette_open);
            (
                doc,
                h.app.session.undo.len(),
                h.app.session.redo.len(),
                h.app.session.path.clone(),
                files,
                h.app.session.selection.clone(),
                h.app.session.visibility.hidden_bodies.clone(),
                h.app.home.open,
                busy,
            )
        };
        let click = |h: &mut crate::scenario::Harness, handle: &str| {
            let at = crate::scenario::handle_at(handle).unwrap();
            h.call("ui.click", json!({"x": at.x, "y": at.y}));
        };
        let mut actions: Vec<(String, Box<dyn Fn(&mut crate::scenario::Harness)>)> = Vec::new();
        for icon in ["undo", "redo"] {
            actions.push((format!("{icon} button"), Box::new(move |h| click(h, &format!("appbar:{icon}")))));
        }
        for (key, cmd) in [("Z", true), ("Y", true), ("Delete", false), ("V", false), ("E", false), ("S", false)] {
            let name = if cmd { format!("Ctrl+{key}") } else { key.to_string() };
            actions.push((name, Box::new(move |h| drop(h.call("ui.key", json!({"key": key, "cmd": cmd}))))));
        }
        for item in ["Insert STEP…", "Insert Mesh…", "Save", "Save As…", "Export STL…", "Export STEP…", "Export 3MF…", "Export OBJ…", "Export URDF…"]
        {
            actions.push((
                format!("File › {item}"),
                Box::new(move |h| {
                    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("sc_file_menu"), false));
                    click(h, "appbar:folder");
                    // A same-named File navigation action is now present: target the menu row explicitly.
                    if let Some(at) = crate::scenario::handle_at(&format!("menu:{item}")) {
                        h.call("ui.click", json!({"x":at.x,"y":at.y}));
                    }
                }),
            ));
        }
        // Each action on its own design, so one cannot hide another.
        let mut acted = Vec::new();
        for (name, act) in &actions {
            let mut h = on_home();
            let before = state(&h);
            act(&mut h);
            h.settle();
            if state(&h) != before {
                acted.push(name.clone());
            }
        }
        assert!(acted.is_empty(), "these acted on the hidden design from Home: {acted:?}");

        h.app.home.open = false;
        h.frames(2);
        let at = crate::scenario::handle_at("appbar:redo").unwrap();
        h.call("ui.click", json!({"x": at.x, "y": at.y}));
        assert_eq!(h.app.session.redo.len(), 0, "Redo works once Home is closed");
        h.call("ui.key", json!({"key": "Z", "cmd": true}));
        h.settle();
        assert_eq!(h.app.session.redo.len(), 1, "Ctrl+Z works once Home is closed");
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("sc_file_menu"), false));
        click(&mut h, "appbar:file");
        h.step(&json!({"widget": "Save"})).unwrap();
        h.settle();
        assert!(h.app.session.path.is_some() && !h.app.session.is_dirty(), "the design saves once Home is closed");
    }

    /// File › Export URDF… writes the robot description and its meshes next to the picked file.
    #[test]
    fn file_menu_exports_a_urdf() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10})).unwrap();
        h.frames(2);
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("sc_file_menu"), false));
        let at = crate::scenario::handle_at("appbar:folder").unwrap();
        h.call("ui.click", json!({"x": at.x, "y": at.y}));
        let _ = h.step(&json!({"widget": "Export URDF…"}));
        h.settle();
        let files: Vec<String> = std::fs::read_dir(&h.dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(files.iter().any(|f| f.ends_with(".urdf")) && files.iter().any(|f| f.ends_with("-base.stl")), "{files:?}");
    }

    /// The disabled buttons' tooltip is translated in every catalog (not English fallback).
    #[test]
    fn needs_design_tooltip_is_translated() {
        let english = crate::i18n::Locale::for_language("en").text("ui.needs_design");
        assert!(english.contains("{command}"));
        for catalog in crate::i18n::LOCALES.iter().filter(|c| !matches!(c.id, "en" | "en-GB")) {
            let text = crate::i18n::Locale::for_language(catalog.id).text("ui.needs_design");
            assert!(text != english && text.contains("{command}"), "{} must translate ui.needs_design", catalog.id);
        }
    }
}

#[cfg(test)]
mod dropdown_tests {
    use super::*;

    /// A drop-down taller than the window stops at its bottom and scrolls: the last row is
    /// reached with the arrow keys and comes into view.
    #[test]
    fn a_tall_drop_down_scrolls_inside_a_short_window() {
        for mode in [crate::shell::LayoutMode::Floating, crate::shell::LayoutMode::Docked] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.call("ui.set", json!({"layout":mode}));
            h.call("ui.resize", json!({"width":1200,"height":320}));
            h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            h.frames(4);
            h.step(&json!({"click":{"handle":"panel:SKETCH:CREATE"}})).unwrap();
            h.frames(4);
            let menu = crate::scenario::rect_of("toolbar_dropdown").expect("the drop-down is open");
            assert!(menu.bottom() <= 320.0, "the drop-down runs past the window: {menu:?}");
            assert!(crate::scenario::handle_at("dropdown:sketch.text").is_some_and(|p| !menu.contains(p)), "Text starts below the fold");
            // Up from no focus goes to the last enabled row; it scrolls into view.
            h.step(&json!({"key":"ArrowUp"})).unwrap();
            h.frames(10);
            let hot = crate::scenario::handles("dropdown_hot:");
            let [(_, at)] = hot.as_slice() else { panic!("one highlighted row: {hot:?}") };
            let menu = crate::scenario::rect_of("toolbar_dropdown").unwrap();
            assert!(menu.contains(*at), "the focused last row is out of view: {at:?} in {menu:?}");
        }
    }

    /// Every row of a drop-down is shown when the window has room, even after a shorter list was
    /// shown first: another tab's panel of the same name, or the same panel collapsed (#874).
    #[test]
    fn a_drop_down_shows_all_its_rows_after_a_shorter_one() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.call("ui.resize", json!({"width":1600,"height":1000}));
        let open = |h: &mut crate::scenario::Harness, tab: &str, panel: &str| {
            h.app.ui.tab = tab.into();
            h.frames(4);
            h.step(&json!({"click":{"handle":format!("panel:{tab}:{panel}")}})).unwrap();
            h.frames(6);
            let menu = crate::scenario::rect_of("toolbar_dropdown").expect("the drop-down is open");
            let rows = crate::scenario::handles("dropdown:");
            h.step(&json!({"key":"Escape"})).unwrap();
            h.frames(4);
            (menu, rows)
        };
        let (short, short_rows) = open(&mut h, "MANAGE", "RULES");
        let (menu, rows) = open(&mut h, "MODEL", "MODIFY");
        assert!(rows.len() > short_rows.len(), "MODEL MODIFY lists more than MANAGE RULES: {rows:?} vs {short_rows:?}");
        assert!(menu.height() > short.height(), "the longer list is taller: {menu:?} vs {short:?}");
        for (id, at) in &rows {
            assert!(menu.contains(*at), "{id} at {at:?} is cut off by the drop-down {menu:?}");
        }
    }
}

#[cfg(test)]
mod panel_drop_tests {
    use super::*;

    #[test]
    fn panel_label_gap_and_margins_append_while_buttons_insert_before() {
        for target in ["label", "gap", "left margin", "right margin", "button"] {
            let mut h = crate::scenario::Harness::new();
            h.step(&json!({"click":{"handle":"panel:MODEL:CREATE"}})).unwrap();
            let label = crate::scenario::handle_at("panel:MODEL:CREATE").unwrap();
            let panel = crate::scenario::rect_of("panel:MODEL:CREATE").unwrap();
            let destination = match target {
                "gap" => label - vec2(0.0, 10.0),
                "left margin" => pos2(panel.left() + 1.0, label.y - 30.0),
                "right margin" => pos2(panel.right() - 1.0, label.y - 30.0),
                "button" => crate::scenario::handle_at("button:solid.extrude").unwrap(),
                _ => label,
            };
            h.step(&json!({"drag":[{"handle":"dropdown:solid.cylinder"}, [destination.x,destination.y]],"steps":12})).unwrap();
            h.step(&json!({"key":"Escape"})).unwrap();
            let order = if target == "button" {
                json!(["button:solid.cylinder", "button:solid.extrude", "button:solid.revolve", "button:solid.hole"])
            } else {
                json!(["button:solid.extrude", "button:solid.revolve", "button:solid.hole", "button:solid.cylinder"])
            };
            h.step(&json!({"expect":{"handle_order":order,"tool":null,"dialog":null,"errors":0}})).unwrap();
            assert!(h.app.session.doc.features.is_empty(), "{target}: pinning must not run the command");
            h.step(&json!({"restart":"close"})).unwrap();
            h.step(&json!({"expect":{"handle_order":order}})).unwrap();
        }
    }

    #[test]
    fn panel_label_drop_moves_a_button_between_panels() {
        let mut h = crate::scenario::Harness::new();
        h.step(&json!({"drag":[{"handle":"button:solid.extrude"},{"handle":"panel:MODEL:MODIFY"}],"steps":12})).unwrap();
        h.step(&json!({"expect":{"handle_order":["button:solid.fillet","button:solid.chamfer","button:solid.shell","button:solid.extrude"],"tool":null,"dialog":null,"errors":0}})).unwrap();
        h.step(&json!({"restart":"close"})).unwrap();
        h.step(&json!({"expect":{"handle_order":["button:solid.shell","button:solid.extrude"]}})).unwrap();
    }
}

#[cfg(test)]
mod dropdown_toggle_tests {
    use crate::scenario::{Harness, rect_of};
    use serde_json::json;

    /// A second click on the button that opened a drop-down closes it (#1522): the title bar's
    /// Export menu, and a panel's command list.
    #[test]
    fn a_second_click_on_its_button_closes_a_drop_down() {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.frames(3);
        let open = |h: &Harness| h.ctx.data(|d| d.get_temp::<bool>(egui::Id::new("sc_export_menu")).unwrap_or(false));
        h.step(&json!({"click": {"handle": "titlebar:export"}})).unwrap();
        assert!(open(&h), "the first click opens the Export menu");
        h.step(&json!({"click": {"handle": "titlebar:export"}})).unwrap();
        h.frames(2);
        assert!(!open(&h), "the second click closes it instead of reopening it");
        h.step(&json!({"click": {"handle": "titlebar:export"}})).unwrap();
        assert!(open(&h), "and a third opens it again");
        h.step(&json!({"key": "Escape"})).unwrap();
        h.frames(2);

        h.step(&json!({"click": {"handle": "panel:MODEL:CREATE"}})).unwrap();
        h.frames(2);
        assert!(rect_of("toolbar_dropdown").is_some(), "the first click opens the panel's list");
        h.step(&json!({"click": {"handle": "panel:MODEL:CREATE"}})).unwrap();
        h.frames(2);
        assert!(rect_of("toolbar_dropdown").is_none(), "the second click closes it");
    }
}

#[cfg(test)]
mod app_bar_tests {
    use super::*;

    #[test]
    fn bar_hover_inks_and_dividers_remain_readable_in_both_themes() {
        fn luminance(c: Color32) -> f32 {
            let c = egui::Rgba::from(c);
            0.2126 * c.r() + 0.7152 * c.g() + 0.0722 * c.b()
        }
        fn contrast(a: Color32, b: Color32) -> f32 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for dark in [false, true] {
            for vision in crate::theme::ColorVision::ALL {
                let t = Tokens::themed(dark, vision);
                for fill in [t.app_bar_hover, t.app_bar_hover_subtle, t.app_bar_hover_button, t.app_bar_hover_strong] {
                    let channel = |fg: u8, bg: u8| fg.saturating_add((u16::from(bg) * u16::from(255 - fill.a()) / 255) as u8);
                    let bg = Color32::from_rgb(channel(fill.r(), t.app_bar.r()), channel(fill.g(), t.app_bar.g()), channel(fill.b(), t.app_bar.b()));
                    assert!(contrast(t.app_bar_text, bg) >= 4.5, "dark={dark}, {vision:?}, {fill:?}");
                }
                assert!(contrast(t.app_bar_separator, t.app_bar) >= 3.0);
                assert!(contrast(t.app_bar, t.warning) >= 4.5, "approval counts need dark ink on amber");
            }
        }
    }

    /// Save writes on a worker thread: frames until it has finished.
    fn wait_for_save(h: &mut crate::scenario::Harness) {
        for _ in 0..500 {
            if h.app.saving.is_none() {
                break;
            }
            h.frames(1);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        h.frames(1);
    }

    #[test]
    fn save_button_uses_the_existing_save_flow_for_new_and_named_designs() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        h.frames(3);
        h.step(&json!({"click":{"handle":"titlebar:save"}})).unwrap();
        wait_for_save(&mut h);
        let path = h.app.session.path.clone().expect("Save chooses a path for an untitled design");
        h.app.run("solid.box", json!({"length":1,"width":1,"height":1,"corner":[10,0,0]})).unwrap();
        h.step(&json!({"click":{"handle":"titlebar:save"}})).unwrap();
        wait_for_save(&mut h);
        assert_eq!(h.app.session.path.as_deref(), Some(path.as_str()));
        let saved = polykrom_engine::io::read_design(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(saved, *h.app.session.doc);
    }
}

#[cfg(test)]
mod ribbon_label_tests {
    use super::*;

    #[test]
    fn ribbon_weight_is_painted_once_at_a_pixel_aligned_origin() {
        for scale in [1.0, 1.5, 2.0] {
            let ctx = crate::fonts::test_context();
            ctx.set_pixels_per_point(scale);
            let ink = Tokens::get().text_dim;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ribbon_text(ui, pos2(100.3, 40.7), "Selected tab", 13.0, ink, true);
            });
            output.textures_delta.clear();
            let text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Selected tab" => Some(text),
                    _ => None,
                })
                .collect();
            assert_eq!(text.len(), 1, "ribbon labels must never be drawn twice");
            let text = text[0];
            assert_eq!(text.galley.job.sections[0].format.font_id, crate::fonts::semibold(13.0));
            assert_eq!(text.galley.job.sections[0].format.color, ink);
            assert!((text.pos.x * scale - (text.pos.x * scale).round()).abs() < 0.001);
            assert!((text.pos.y * scale - (text.pos.y * scale).round()).abs() < 0.001);
        }
    }

    #[test]
    fn every_promoted_label_wraps_only_at_word_boundaries() {
        let ctx = crate::fonts::test_context();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            let specs = polykrom_engine::command_specs();
            for section in crate::workspace::SECTIONS {
                let (commands, count) = crate::workspace::layout(section.tab, section.panel, &specs).unwrap();
                for command in commands.iter().take(count) {
                    for catalog in crate::i18n::LOCALES {
                        let locale = crate::i18n::Locale::for_language(catalog.id);
                        let label = locale.label(crate::workspace::ribbon_button_label(command));
                        let width = ribbon_button_width(&painter, label) - BUTTON_LABEL_PADDING;
                        let galley = ribbon_button_galley(&painter, label, width, Color32::WHITE);
                        assert!(galley.rows.len() <= 2, "{}: {label}", command.id);
                        // Centred rows are each rounded to the pixel grid, so two rows of
                        // different widths can span up to a pixel more than the widest one
                        // measured alone (Vietnamese "Tập hợp tiếp xúc" in Inter: half a pixel).
                        assert!(galley.size().x <= width + 1.0, "{}: {label}: {} > {width}", command.id, galley.size().x);
                        let first = galley.rows.first().unwrap().text();
                        let visible = first.strip_suffix('…').unwrap_or(&first);
                        assert!(label.starts_with(visible), "{}: {label} -> {first}", command.id);
                        if let Some(second) = galley.rows.get(1) {
                            let second = second.text();
                            let next = second.strip_suffix('…').unwrap_or(&second);
                            // The second line must start at a legal source boundary,
                            // including when the first line was ellipsized.
                            assert!(
                                label.char_indices().any(|(at, ch)| {
                                    let end = at + ch.len_utf8();
                                    (ch.is_whitespace() || ch == '/')
                                        && label[end..].trim_start().starts_with(next)
                                        && (first.ends_with('…') || label[..end].trim() == first)
                                }),
                                "{}: {label} -> {first:?}, {second:?} breaks inside a word",
                                command.id
                            );
                        }
                    }
                }
            }
            for label in ["Rectangular", "Dimension", "Coincident", "Chamfer", "Revolve"] {
                let width = ribbon_button_width(&painter, label);
                assert_eq!(ribbon_button_lines(&painter, label, width - BUTTON_LABEL_PADDING), vec![label]);
                assert!((BUTTON_MIN_WIDTH..=BUTTON_MAX_WIDTH).contains(&width));
            }
            let finish = polykrom_engine::find_command("sketch.finish").unwrap();
            let label = crate::workspace::ribbon_button_label(finish);
            let width = ribbon_button_width(&painter, label);
            // A large button is as wide as its one-line label (the kit's labels never wrap).
            assert_eq!(ribbon_button_lines(&painter, label, width - BUTTON_LABEL_PADDING), vec!["Finish Sketch"]);
            // Exercise full labels and pinned commands too, not just compact defaults.
            for (label, width, expected) in [
                ("Coincident", 50.0, vec!["Coincident"]),
                ("Rectangular Pattern", 50.0, vec!["Rectangular", "Pattern"]),
                ("Move/Copy", 40.0, vec!["Move/", "Copy"]),
            ] {
                let lines = ribbon_button_lines(&painter, label, width);
                // Exact glyph widths depend on the bundled font: verify the source
                // word stays on its own line, with only an optional suffix omitted.
                assert_eq!(lines.len(), expected.len(), "{label}: {lines:?}");
                for (actual, word) in lines.iter().zip(expected) {
                    assert!(actual == word || actual.strip_suffix('…').is_some_and(|prefix| word.starts_with(prefix)), "{label}: {lines:?}");
                }
            }
            let lines = ribbon_button_lines(&painter, "one two three four five", 50.0);
            assert_eq!(lines.len(), 2);
            assert!(lines[1].ends_with('…'));
            let lines = ribbon_button_lines(&painter, "長い日本語のラベル 続き", 50.0);
            assert_eq!(lines.len(), 2);
            assert!(lines[0].ends_with('…'));
        });
        output.textures_delta.clear();
    }
}

#[cfg(test)]
mod ribbon_wrap_tests {
    use super::*;

    /// At the default window size (1600 x 1000) and the kit's 1760 x 1100, no label on a ribbon
    /// button wraps or is cut: every large button is as wide as its one-line label (`Move / Copy`
    /// wrapped to two lines before), and every small button's label is one line.
    #[test]
    fn no_ribbon_label_wraps_at_the_default_window_size() {
        for (w, height) in [(1600.0, 1000.0), (1760.0, 1100.0)] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
            h.call("ui.resize", json!({"width":w,"height":height}));
            let mut seen = 0;
            for tab in TABS.iter().copied().chain(["SHEET METAL"]) {
                h.app.ui.tab = tab.into();
                h.frames(3);
                let painter = egui::Painter::new(h.ctx.clone(), egui::LayerId::background(), Rect::EVERYTHING);
                for (name, rect) in crate::scenario::rects("button:") {
                    let Some(id) = name.strip_prefix("button:") else { continue };
                    let Some(c) = polykrom_engine::find_command(id) else { continue };
                    let label = h.app.tr_label(crate::workspace::ribbon_button_label(c));
                    let galley = ribbon_button_galley(&painter, label, rect.width() - BUTTON_LABEL_PADDING, Color32::WHITE);
                    assert_eq!(galley.rows.len(), 1, "{tab} at {w}: `{label}` wraps on its {} px button", rect.width());
                    assert!(!galley.text().ends_with('…'), "{tab} at {w}: `{label}` is cut");
                    seen += 1;
                }
                for (name, rect) in crate::scenario::rects("ribbon:small:") {
                    let Some(id) = name.strip_prefix("ribbon:small:") else { continue };
                    let Some(c) = polykrom_engine::find_command(id) else { continue };
                    let label = h.app.tr_label(crate::workspace::ribbon_button_label(c));
                    let galley = small_label_galley(&painter, label, rect.width() - SMALL_TEXT_X - 4.0, Color32::WHITE);
                    assert!(!galley.elided, "{tab} at {w}: small `{label}` is cut on its {} px button", rect.width());
                    seen += 1;
                }
            }
            assert!(seen > 40, "{seen} labelled buttons checked at {w}");
        }
    }
}

#[cfg(test)]
mod ribbon_gallery_tests {
    use super::*;
    use crate::scenario::{Harness, rect_of};

    #[test]
    fn dragged_constraint_becomes_full_button_and_reset_restores_gallery() {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width":1800,"height":900}));
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.frames(3);
        let id = crate::workspace::CONSTRAINT_GALLERY[0];
        let cell_handle = format!("ribbon:constraint:{id}");
        let button_handle = format!("toolbar:{id}");
        let button = format!("button:{id}");
        let cell = rect_of(&cell_handle).unwrap();
        let count = || crate::scenario::handles("ribbon:constraint:").len();
        let original_count = count();
        h.step(&json!({"drag":[{"handle":button_handle},{"handle":"toolbar:sketch.dimension"}]})).unwrap();
        h.frames(3);
        assert!(rect_of(&cell_handle).is_none());
        let full = rect_of(&button).unwrap();
        assert!(full.height() > cell.height());
        assert!(full.left() < rect_of("button:sketch.dimension").unwrap().left());
        assert_eq!(count(), original_count - 1);
        let next = format!("ribbon:constraint:{}", crate::workspace::CONSTRAINT_GALLERY[1]);
        assert_eq!(rect_of(&next).unwrap().top(), cell.top());
        h.step(&json!({"drag":[{"handle":button_handle},{"handle":next}]})).unwrap();
        h.frames(3);
        assert_eq!(rect_of(&cell_handle), Some(cell));
        assert_eq!(count(), original_count);
        h.step(&json!({"drag":[{"handle":button_handle},{"handle":"toolbar:sketch.dimension"}]})).unwrap();
        h.frames(3);
        assert!(rect_of(&cell_handle).is_none());
        let saved = crate::toolbar_custom::export(&h.app);
        crate::toolbar_custom::reset(&mut h.app, "SKETCH");
        crate::toolbar_custom::import(&mut h.app, &saved).unwrap();
        h.frames(3);
        assert!(rect_of(&cell_handle).is_none());
        assert_eq!(rect_of(&button).unwrap().height(), full.height());
        // Settings saved before the gallery could change load with the default gallery.
        let mut older = h.app.toolbar_custom.prefs();
        older.as_object_mut().unwrap().remove("gallery_buttons");
        let mut loaded = crate::toolbar_custom::ToolbarCustom::default();
        loaded.load(&older);
        assert_eq!(loaded.panels, h.app.toolbar_custom.panels);
        assert!(loaded.in_gallery("SKETCH", "CONSTRAIN", id));
        crate::toolbar_custom::reset(&mut h.app, "MODEL");
        h.frames(2);
        assert!(rect_of(&cell_handle).is_none());
        crate::toolbar_custom::reset(&mut h.app, "SKETCH");
        h.frames(3);
        assert_eq!(rect_of(&cell_handle), Some(cell));
        assert_eq!(count(), original_count);
    }

    #[test]
    fn constraint_gallery_preserves_button_handles_order_and_pin_actions() {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width":1800,"height":900}));
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.frames(3);
        let dimension = rect_of("button:sketch.dimension").unwrap();
        let mut cells = Vec::new();
        for id in crate::workspace::CONSTRAINT_GALLERY {
            let cell = rect_of(&format!("ribbon:constraint:{id}")).unwrap();
            assert_eq!(cell.size(), vec2(24.0, 20.0));
            assert_eq!(rect_of(&format!("button:{id}")), Some(cell));
            assert!(cell.left() > dimension.right());
            cells.push(cell);
        }
        for row in cells.chunks(3) {
            assert!(row.windows(2).all(|pair| pair[0].right() < pair[1].left() && pair[0].top() == pair[1].top()));
        }
        for col in 0..3 {
            assert_eq!(cells[col].left(), cells[col + 3].left());
            assert!(cells[col].bottom() < cells[col + 3].top());
        }
        h.step(&json!({"click":{"handle":"ribbon:constraint:sketch.constraint.equal"}})).unwrap();
        assert_eq!(h.app.tool.as_ref().map(|t| t.cmd), Some("sketch.constraint.equal"));
        h.step(&json!({"key":"Escape"})).unwrap();
        crate::toolbar_custom::remove(&mut h.app, "SKETCH", "sketch.constraint.equal");
        h.frames(2);
        assert!(rect_of("ribbon:constraint:sketch.constraint.equal").is_none());
        crate::toolbar_custom::pin(&mut h.app, "SKETCH", "sketch.constraint.equal").unwrap();
        h.frames(2);
        assert!(rect_of("ribbon:constraint:sketch.constraint.equal").is_some());
    }
}

#[cfg(test)]
mod fill_width_tests {
    use super::*;

    /// #2108: at every common window width, in the Solid and the Sketch tab, the toolbar uses the
    /// width it has. No blank run wider than one button while something is hidden, every panel
    /// on the toolbar shows its icons, the » menu only when even one button a panel won't fit,
    /// and neither the tab row nor the title bar's own controls run into each other.
    #[test]
    fn toolbar_fills_its_width_from_wide_to_narrow_windows() {
        for sketch in [false, true] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.app.custom_titlebar = true;
            h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
            if sketch {
                h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            }
            let tab = if sketch { "SKETCH" } else { "MODEL" };
            h.app.ui.tab = tab.into();
            for w in [1920.0, 1600.0, 1440.0, 1366.0, 1280.0, 1100.0, 900.0, 700.0] {
                h.call("ui.resize", json!({"width":w,"height":900}));
                h.frames(3);
                let at = format!("{tab} at {w}");
                let bar = crate::scenario::rect_of("toolbar").unwrap();
                let finish = crate::scenario::rect_of("panel:SKETCH:FINISH SKETCH");
                // The room for the other panels: the bar less the Finish group kept for it.
                let right = bar.right() - 4.0 - finish.map_or(0.0, |f| f.width());
                let count = |name: &str| crate::scenario::count_of(name).unwrap_or(-1.0);
                let (collapsed, trimmed, more) = (count("toolbar_collapsed"), count("toolbar_trimmed"), count("toolbar_more"));
                let lists = crate::toolbar_custom::panel_lists(tab);
                let mut end = bar.left();
                let mut on_bar = 0;
                for (panel, cmds, promote) in &lists {
                    let Some(rect) = crate::scenario::rect_of(&format!("panel:{tab}:{panel}")) else { continue };
                    on_bar += 1;
                    if *panel != "FINISH SKETCH" {
                        end = end.max(rect.right() + PANEL_GAP);
                    }
                    if crate::toolbar_custom::buttons(&h.app, tab, panel, cmds, *promote).is_empty() {
                        continue;
                    }
                    let icons = crate::scenario::handles("button:").iter().filter(|(_, p)| rect.contains(*p)).count()
                        + usize::from(crate::scenario::handle_at(&format!("collapsed:{tab}:{panel}")).is_some());
                    assert!(icons >= 1, "{at}: {panel} shows no icons");
                }
                assert_eq!(on_bar + more as usize, lists.len(), "{at}: every panel is on the toolbar or in the » menu");
                let overflow = crate::scenario::rect_of("toolbar:overflow");
                assert_eq!(overflow.is_some(), more > 0.0, "{at}: the » menu shows exactly when a panel is in it");
                let gap = right - end - if more > 0.0 { MORE_ROOM } else { 0.0 };
                assert!(gap >= -0.5, "{at}: the panels run {} px past the room", -gap);
                let hidden = collapsed + trimmed + more > 0.0;
                // One button: an icon's stride, a small-button column's, or the gallery's.
                let button = if more > 0.0 {
                    COLLAPSED_WIDTH + PANEL_GAP
                } else if sketch {
                    CONSTRAINT_GRID_WIDTH.max(SMALL_MAX_WIDTH) + BUTTON_GAP
                } else {
                    SMALL_MAX_WIDTH + BUTTON_GAP
                };
                if hidden {
                    assert!(gap < button, "{at}: {gap} px left blank while buttons are hidden");
                }
                // The » menu only when one button a panel can't fit.
                if w >= 900.0 {
                    assert_eq!(more, 0.0, "{at}: the » menu while there is room");
                }
                if w >= 1600.0 {
                    // Secondary (small) columns may still wait in the drop-downs; no group collapses
                    // or leaves for the » menu, and every pinned button is on the bar.
                    assert_eq!(collapsed + more, 0.0, "{at}: buttons hidden on a wide window");
                    for (panel, cmds, promote) in &lists {
                        for c in crate::toolbar_custom::buttons(&h.app, tab, panel, cmds, *promote) {
                            assert!(crate::scenario::rect_of(&format!("button:{}", c.id)).is_some(), "{at}: pinned {} hidden", c.id);
                        }
                    }
                }
                if let Some(o) = overflow {
                    assert!(o.right() <= right + 4.0, "{at}: the » button overlaps Finish or the right edge");
                }
                if sketch {
                    let finish = finish.expect("Finish group stays visible");
                    if more > 0.0 {
                        assert_eq!(finish.right(), bar.right(), "{at}: Finish moves right of the » menu");
                    } else {
                        assert!((finish.left() - end).abs() < 0.5, "{at}: Finish follows the last panel ({} vs {end})", finish.left());
                    }
                    assert!(crate::scenario::handle_at("button:sketch.finish").is_some(), "{at}: Finish stays reachable");
                }
                // The visible ribbon tabs stay inside the bar.
                for t in TABS.iter().chain(["SKETCH"].iter()) {
                    if let Some(r) = crate::scenario::rect_of(&format!("toolbar_tab:{t}")) {
                        assert!(r.right() <= bar.right(), "{at}: the {t} tab runs past the bar");
                    }
                }
                // The title bar's own controls (the document identity, the search field, and
                // Undo/Redo/Save/Export) stay in order and inside the bar.
                for group in [["appbar:undo", "appbar:redo", "ribbon:search", "titlebar:save", "titlebar:export"]] {
                    let rects: Vec<_> = group.iter().filter_map(|n| crate::scenario::rect_of(n)).collect();
                    for pair in rects.windows(2) {
                        assert!(pair[0].right() <= pair[1].left() + 0.5, "{at}: title-bar controls overlap: {:?} {:?}", pair[0], pair[1]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod sketch_return_tab_tests {
    use super::*;

    /// #3250: finishing a sketch goes back to the tab it was started from, not always to MODEL.
    #[test]
    fn finishing_a_sketch_returns_to_the_tab_it_started_from() {
        for start in ["MODEL", "SURFACE", "ASSEMBLE", "INSERT", "INSPECT", "MANAGE", "VIEW"] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.app.ui.tab = start.into();
            h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            assert_eq!(h.app.ui.tab, "SKETCH", "{start}: the sketch tab shows while sketching");
            h.app.run("sketch.finish", json!({})).unwrap();
            assert!(h.app.session.active_sketch.is_none());
            assert_eq!(h.app.ui.tab, start, "{start}: back on the tab the sketch started from");
        }
    }

    /// A second sketch started from another tab returns to that tab, and a start tab that is
    /// not a normal toolbar tab (a stale name) falls back to MODEL.
    #[test]
    fn sketch_return_tab_follows_each_sketch_and_falls_back_to_model() {
        let mut h = crate::scenario::Harness::new();
        h.app.home.open = false;
        for start in ["SURFACE", "INSPECT"] {
            h.app.ui.tab = start.into();
            h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            h.app.run("sketch.finish", json!({})).unwrap();
            assert_eq!(h.app.ui.tab, start);
        }
        let start = "NO SUCH TAB";
        h.app.ui.tab = start.into();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
        assert_eq!(h.app.ui.tab, "MODEL", "{start}: falls back to MODEL");
    }
}

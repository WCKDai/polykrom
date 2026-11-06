//! Transient shell geometry shared by automation, drawing and viewport occlusion.
use crate::ui_kit::{Container, Surface};
use egui::{Rect, pos2};
use serde::{Deserialize, Serialize};

use crate::SolveApp;

/// Saved workspace layout; automation may override it for a session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    #[default]
    Floating,
    #[serde(rename = "compact", alias = "docked")]
    Docked,
}

impl LayoutMode {
    pub(crate) fn publish(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("sc_container_layout"), self));
    }

    pub(crate) fn current(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp(egui::Id::new("sc_container_layout"))).unwrap_or_default()
    }
}

/// The gap from the viewport edge to the floating cards and between the cards.
pub(crate) const SHELL_GAP: f32 = 7.0;

/// The ribbon card (the tab row and the groups): the window's width less the floating margin.
/// The workspace choice is on the tab row, so there is no separate workspace card.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cards {
    pub ribbon: Rect,
    pub bottom: f32,
}

impl Cards {
    pub fn new(available: Rect, mode: LayoutMode, ribbon_height: f32) -> Self {
        let margin = Container::for_layout(mode, Surface::Chrome).gap();
        let left = available.left() + margin;
        let top = available.top() + margin;
        let right = (available.right() - margin).max(left);
        let ribbon = Rect::from_min_max(pos2(left, top), pos2(right, top + ribbon_height));
        Self { ribbon, bottom: ribbon.bottom() + margin }
    }
}

/// Shell panels measured after drawing the full-bleed viewport. No design or preferences data.
#[derive(Default)]
pub(crate) struct State {
    pub cards: Vec<Rect>,
    pub panels: Vec<Rect>,
    pub browser: Option<Rect>,
    pub history: Option<Rect>,
    /// In-memory seam drag width, never serialized as a preference.
    pub history_drag_width: Option<f32>,
    /// Unreserved canvas bounds used by History docking targets.
    pub history_dock_space: Option<Rect>,
    /// Browser/history collapse progress shared by both cards for this frame.
    pub history_folding: Option<[f32; 2]>,
    pub history_fold_motion: Option<[crate::timeline_left::FoldMotion; 2]>,
    pub content: Option<Rect>,
    /// The document tabs and the view bar (`document_bar`): floating, one slim card at the top
    /// of the canvas right of the Design Browser; docked, the bands above the viewport.
    pub document_bar: Option<Rect>,
    /// The status bar along the window's bottom.
    pub status_bar: Option<Rect>,
    /// The agent activity feed's card in the canvas's bottom right corner (`agent_activity`),
    /// as painted last frame: the toasts keep clear of it.
    pub agent_activity: Option<Rect>,
}

/// One source of occlusion for navigation, picking, overlays and fit.
pub(crate) fn covered_rects(app: &SolveApp) -> Vec<Rect> {
    let mut rects = Vec::new();
    let floating = Container::for_layout(app.ui.layout, Surface::Panel).floating();
    if floating {
        rects.extend(app.shell.cards.iter().copied());
        rects.extend(app.shell.panels.iter().copied());
        if crate::agent_chat::shown(app) && app.agent_chat.open && !app.preferences.agent_chat_popped {
            rects.extend(app.agent_chat.panel_rect);
        }
    }
    rects.extend(app.shell.history.filter(|_| app.ui.show_timeline));
    if app.ui.show_timeline && app.preferences.timeline_left {
        rects.extend(app.shell.history.map(|card| crate::ui_kit::HistorySeam::new(card).pill));
    }
    rects.extend(app.shell.browser.filter(|_| app.ui.show_browser));
    if floating {
        // The document strip is a card over the canvas: all of it shields the scene.
        rects.extend(app.shell.document_bar);
    } else {
        // The docked document bar abuts the scene at its bottom edge; that shared row belongs to
        // the scene (a model point landing exactly on the scene's top must stay pickable), so the
        // bar yields it.
        rects.extend(app.shell.document_bar.map(|s| Rect::from_min_max(s.min, pos2(s.max.x, (s.max.y - 1.0).max(s.min.y)))));
    }
    rects.extend(app.inspector.rect);
    // The AI review's prompt row and panel cover the viewport only as floating cards; docked,
    // they are edge panels beside it.
    if floating {
        rects.extend(app.review.rect);
        rects.extend(app.review.prompt_rect);
    }
    rects.extend(app.viewport.palette_rect.filter(|_| app.session.active_sketch.is_some()));
    rects.extend(app.viewport.dialog_rect.filter(|_| app.dialog.is_some() || app.tree.redefine.is_some()));
    rects.retain(|r| r.is_finite() && r.is_positive());
    rects
}

pub(crate) fn covered(app: &SolveApp, p: egui::Pos2) -> bool {
    covered_rects(app).iter().any(|r| r.contains(p))
}

/// Space reserved below viewport overlays by the floating History card. Its rect is set before
/// the viewport is painted, so showing or hiding History takes effect in the same frame. History
/// docked as the left sidebar sits beside the viewport, not below it, so it reserves nothing.
pub(crate) fn viewport_bottom_inset(app: &SolveApp, viewport: Rect) -> f32 {
    if app.ui.layout != LayoutMode::Floating || (app.ui.show_timeline && app.preferences.timeline_left) {
        return 0.0;
    }
    app.shell
        .history
        .filter(|r| r.is_finite() && r.width() >= 1.0 && r.height() >= 30.0)
        .map_or(0.0, |history| (viewport.bottom() - history.top() + SHELL_GAP).clamp(0.0, viewport.height().max(0.0)))
}

/// Bottom-anchored overlays share the content area's History clearance without moving the
/// viewport's projection or its left and right anchors.
pub(crate) fn bottom_overlay_rect(app: &SolveApp, mut viewport: Rect) -> Rect {
    viewport.max.y -= viewport_bottom_inset(app, viewport);
    viewport
}

/// The former central panel remains the anchor for controls and docked panels.
pub(crate) fn content_rect(app: &SolveApp, viewport: Rect) -> Rect {
    app.shell.content.unwrap_or(viewport).intersect(viewport)
}

/// Free anchor space shared by both layouts: where the viewport's controls, the tool sheets and
/// the overlays go. The scene itself (the projection) is not changed by it. Compact edge hosts
/// have already consumed their allocation; the Design Browser (a floating card or a column) and
/// the History card reserve their measured span, so nothing is laid out over them.
pub(crate) fn workspace_content(app: &SolveApp, mut content: Rect) -> Rect {
    let gap = Container::for_layout(app.ui.layout, Surface::Panel).gap();
    if let Some(history) = app.shell.history {
        if app.preferences.timeline_left {
            content.min.x = (history.right() + gap).clamp(content.left(), content.right());
        } else {
            content.max.y = (history.top() - gap).clamp(content.top(), content.bottom());
        }
    }
    if let Some(browser) = app.shell.browser.filter(|_| app.ui.show_browser) {
        content.min.x = (browser.right() + gap).clamp(content.left(), content.right()).max(content.min.x);
    }
    content
}

/// Tool sheets may cover Browser and History, while shell chrome and Assistant stay clear.
/// Assistant's outside-rail exception stays in its owner.
pub(crate) fn tool_drag_rect(app: &SolveApp, placement: Rect) -> Rect {
    let gap = Container::for_layout(app.ui.layout, Surface::Feature).gap();
    let mut bounds = crate::agent_chat::tool_space(app, app.viewport.rect.unwrap_or(placement));
    for card in app.shell.cards.iter().filter(|r| r.is_finite() && r.is_positive()) {
        bounds.min.y = (card.bottom() + gap).clamp(bounds.top(), bounds.bottom());
    }
    bounds
}

/// The height a docked panel (a tool sheet, the sketch palette) keeps on a short view, moving up
/// over the compass rather than shrinking below it.
pub(crate) const PANEL_MIN_HEIGHT: f32 = 350.0;

pub(crate) fn panel_top(app: &SolveApp, viewport: Rect, bottom: f32, side: crate::frame::Side) -> f32 {
    let content = content_rect(app, viewport);
    if side == crate::frame::Side::Right {
        // The right-hand properties and feature dialogs dock below the measured compass, as they
        // did before the design kit shell (a later task restyles that column); on a short view
        // they reclaim the compass's space rather than shrink below it.
        app.viewport
            .layout
            .as_ref()
            .map_or(content.top(), |l| if l.axis_view.is_positive() { l.axis_view.bottom().min(bottom) } else { content.top() })
            .min((bottom - PANEL_MIN_HEIGHT).max(content.top()))
    } else {
        (viewport.top() + crate::viewport::AXIS_VIEW_CLEARANCE).min((bottom - PANEL_MIN_HEIGHT).max(viewport.top()))
    }
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;

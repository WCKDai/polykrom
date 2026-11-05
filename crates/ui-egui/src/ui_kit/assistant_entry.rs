//! Assistant entry point and its status mark, shared by application chrome and previews.
use crate::{icons, theme::Tokens};
use egui::{Color32, FontId, Rect, Response, Stroke, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssistantEntryState {
    Quiet,
    Ready,
    Busy,
    NeedsAuth,
    Stopped,
    Waiting(usize),
}

/// Where the entry point sits, which decides the ink of its sparkle and the chrome under it:
/// the application bar (its text token and translucent hovers) or a panel on the window's base
/// surface (the status bar: the icon ink and the panel hover, like the buttons beside it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssistantHost {
    AppBar,
    Window,
}

/// The ink of the waiting count on its amber badge: black or white, whichever reads better.
pub(crate) fn badge_ink(t: &Tokens) -> Color32 {
    if crate::theme::contrast_ratio(Color32::BLACK, t.warning) >= crate::theme::contrast_ratio(Color32::WHITE, t.warning) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Paint the compact entry point in its caller-owned target. The caller owns state and actions.
/// Tokens: app_bar/window, app_bar_hover_button/strong or hover, app_bar_text or icon,
/// agent_ok/busy, warning/error, radius_sm, font_badge.
pub(crate) fn assistant_entry(ui: &egui::Ui, response: &Response, state: AssistantEntryState, open: bool, animate: bool, host: AssistantHost) {
    let t = Tokens::get();
    let at = response.rect;
    let mark = state;
    let (surface, hover, hover_open, ink) = match host {
        AssistantHost::AppBar => (t.app_bar, t.app_bar_hover_button, t.app_bar_hover_strong, t.app_bar_text),
        AssistantHost::Window => (t.window, t.hover, t.hover, t.icon),
    };
    // The highlight is a 26 px square, like the other icon buttons.
    let mut ring = surface;
    if open || response.hovered() || response.has_focus() {
        let fill = if open { hover_open } else { hover };
        ui.painter().rect_filled(at.shrink(t.space_3), t.radius_sm, fill);
        ring = surface.blend(fill);
    }
    // The sparkle, in the host's ink so it shows on a light bar as on a dark one; the status
    // mark sits in the top-right corner, clear of the glyph's strokes.
    icons::paint_ink(ui.painter(), Rect::from_center_size(at.center(), vec2(t.space_16, t.space_16)), "assistant", ink);
    let c = pos2(at.right() - t.space_8 + 0.5, at.top() + t.space_8 - 0.5);
    match mark {
        AssistantEntryState::Quiet => {}
        AssistantEntryState::Ready | AssistantEntryState::NeedsAuth | AssistantEntryState::Stopped => {
            let color = match mark {
                AssistantEntryState::Ready => t.agent_ok,
                AssistantEntryState::NeedsAuth => t.warning,
                _ => t.error,
            };
            ui.painter().circle_filled(c, 5.5, ring);
            ui.painter().circle_filled(c, 4.0, color);
        }
        AssistantEntryState::Busy => {
            let phase = if animate {
                ui.ctx().request_repaint();
                ui.input(|i| i.time) as f32 * 3.0
            } else {
                0.0
            };
            ui.painter().circle_filled(c, 6.0, ring);
            let points = (0..=24)
                .map(|i| {
                    let angle = phase + i as f32 / 24.0 * std::f32::consts::TAU * 0.75;
                    c + vec2(angle.cos(), angle.sin()) * 4.0
                })
                .collect();
            ui.painter().add(egui::Shape::line(points, Stroke::new(1.8, t.agent_busy)));
        }
        AssistantEntryState::Waiting(count) => {
            let text = count.to_string();
            let font = FontId::proportional(t.font_badge);
            let ink = badge_ink(&t);
            let galley = ui.painter().layout_no_wrap(text, font, ink);
            let badge = Rect::from_center_size(c, vec2((galley.size().x + t.space_6).max(t.space_14), t.space_14));
            ui.painter().rect_filled(badge.expand(1.5), Tokens::outset_radius(f32::from(t.sketch_mode_radius), 1.5), ring);
            ui.painter().rect_filled(badge, t.sketch_mode_radius, t.warning);
            ui.painter().galley(badge.center() - galley.size() / 2.0, galley, ink);
        }
    }
}

/// The sparkle's and the status mark's places in a 32 px entry (for the tests and pictures):
/// (glyph, mark centre, mark radius).
#[cfg(test)]
pub(crate) fn geometry(at: Rect) -> (Rect, egui::Pos2, f32) {
    let t = Tokens::get();
    (Rect::from_center_size(at.center(), vec2(t.space_16, t.space_16)), pos2(at.right() - t.space_8 + 0.5, at.top() + t.space_8 - 0.5), 6.0)
}

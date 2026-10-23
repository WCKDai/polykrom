//! Collected glyph artwork. Caption geometry adapted from VectorCraft (MIT OR Apache-2.0).
use crate::titlebar::Caption;
use egui::{Color32, Pos2, Rect, Stroke, Ui, pos2, vec2};

pub(crate) fn transport_glyph(p: &egui::Painter, c: egui::Pos2, what: &str, col: Color32) {
    let tri = |dir: f32, dx: f32| {
        let pts = vec![pos2(c.x + dx - 4.0 * dir, c.y - 5.0), pos2(c.x + dx + 4.0 * dir, c.y), pos2(c.x + dx - 4.0 * dir, c.y + 5.0)];
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    };
    let bar = |dx: f32| p.line_segment([pos2(c.x + dx, c.y - 5.0), pos2(c.x + dx, c.y + 5.0)], Stroke::new(1.6, col));
    match what {
        "first" => {
            bar(-5.0);
            tri(-1.0, 1.0);
        }
        "back" => tri(-1.0, 0.0),
        "forward" => tri(1.0, 0.0),
        _ => {
            tri(1.0, -1.0);
            bar(5.0);
        }
    }
}

pub(crate) fn paint_glyph(ui: &Ui, c: Caption, maximized: bool, at: egui::Pos2, s: Stroke) {
    let p = ui.painter();
    let g = Rect::from_center_size(at, vec2(10.0, 10.0));
    match c {
        Caption::Minimize => {
            p.line_segment([g.left_center(), g.right_center()], s);
        }
        // Restore: a front square with the corner of the one behind it.
        Caption::Maximize if maximized => {
            let front = Rect::from_min_max(g.min + vec2(0.0, 2.0), g.max - vec2(2.0, 0.0));
            p.rect_stroke(front, 0.0, s, egui::StrokeKind::Middle);
            p.line(vec![g.min + vec2(2.0, 2.0), g.min + vec2(2.0, 0.0), g.right_top(), g.max - vec2(0.0, 2.0), g.max - vec2(2.0, 2.0)], s);
        }
        Caption::Maximize => {
            p.rect_stroke(g, 0.0, s, egui::StrokeKind::Middle);
        }
        Caption::Close => {
            p.line_segment([g.left_top(), g.right_bottom()], s);
            p.line_segment([g.right_top(), g.left_bottom()], s);
        }
    }
}

pub(crate) fn badge(p: &egui::Painter, r: Rect, name: &str, c: Color32) {
    let s = Stroke::new(1.3, c);
    match name {
        "lock" => {
            let body = Rect::from_min_max(pos2(r.left() + 1.5, r.center().y - 0.5), pos2(r.right() - 1.5, r.bottom() - 0.5));
            p.rect_filled(body, 1.5, c);
            let top = pos2(r.center().x, r.top() + 4.0);
            p.add(egui::Shape::line(
                (0..=8)
                    .map(|k| {
                        let a = std::f32::consts::PI * (k as f32 / 8.0);
                        pos2(top.x - 3.2 * a.cos(), top.y - 3.2 * a.sin() + 1.0)
                    })
                    .chain([pos2(top.x + 3.2, body.top())])
                    .collect(),
                s,
            ));
            p.line_segment([pos2(top.x - 3.2, top.y + 1.0), pos2(top.x - 3.2, body.top())], s);
        }
        "pin" => {
            let head = pos2(r.center().x + 2.0, r.top() + 4.0);
            p.circle_filled(head, 3.2, c);
            p.line_segment([head, pos2(r.left() + 1.5, r.bottom() - 1.0)], Stroke::new(1.6, c));
        }
        _ => {}
    }
}

pub(crate) fn pin_icon(p: &egui::Painter, r: Rect, on: bool, col: Color32) {
    let c = r.center();
    let (sin, cos) = if on { (0.0, 1.0) } else { std::f32::consts::FRAC_PI_4.sin_cos() };
    // Local coordinates: the pin points down, its head on top.
    let at = |x: f32, y: f32| c + vec2(x * cos - y * sin, x * sin + y * cos);
    let head = vec![at(-2.5, -7.0), at(2.5, -7.0), at(3.0, -2.0), at(-3.0, -2.0)];
    let stroke = Stroke::new(1.4, col);
    if on {
        p.add(egui::Shape::convex_polygon(head, col, stroke));
    } else {
        p.add(egui::Shape::closed_line(head, stroke));
    }
    p.line_segment([at(-5.5, -2.0), at(5.5, -2.0)], stroke);
    p.line_segment([at(0.0, -2.0), at(0.0, 7.0)], stroke);
}

/// A breadcrumb separator ("Sketch › Create"): a thin chevron about 3 by 6 px, centred on `c`.
/// Drawn rather than typed, because a font's "›" sits on the baseline in some languages
/// (Japanese), where it reads as a comma.
pub(crate) fn breadcrumb(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(1.0, col);
    p.line_segment([pos2(c.x - 1.5, c.y - 3.0), pos2(c.x + 1.5, c.y)], s);
    p.line_segment([pos2(c.x + 1.5, c.y), pos2(c.x - 1.5, c.y + 3.0)], s);
}

pub(crate) fn brand_mark(p: &egui::Painter, c: Pos2) {
    brand_mark_styled(p, c, 20.0, crate::theme::Tokens::get().logo, true);
}

/// The same original mark as a single quiet silhouette, without face seams or outlines.
pub(crate) fn brand_mark_silhouette(p: &egui::Painter, c: Pos2, height: f32, ink: Color32) {
    brand_mark_styled(p, c, height, [ink; 4], false);
}

/// Polykrom's mark: a faceted gem, a regular hexagon (pointy top) tiled like a polyhedron seen
/// down its axis. A light centre triangle, three mid-tone facets on its sides reaching the
/// alternate corners, and six dark corner facets. Original geometry; `height` is corner to corner.
fn brand_mark_styled(p: &egui::Painter, c: Pos2, height: f32, colours: [Color32; 4], edges: bool) {
    let r = height / 2.0;
    // Hexagon corners from the top, clockwise; the centre triangle points up at half the radius.
    let corner = |k: f32| {
        let a = std::f32::consts::FRAC_PI_2 - k * std::f32::consts::FRAC_PI_3;
        pos2(c.x + r * a.cos(), c.y - r * a.sin())
    };
    let inner = |k: f32| {
        let a = std::f32::consts::FRAC_PI_2 - k * 2.0 * std::f32::consts::FRAC_PI_3;
        pos2(c.x + 0.5 * r * a.cos(), c.y - 0.5 * r * a.sin())
    };
    let h: [Pos2; 6] = [corner(0.0), corner(1.0), corner(2.0), corner(3.0), corner(4.0), corner(5.0)];
    let t: [Pos2; 3] = [inner(0.0), inner(1.0), inner(2.0)];
    let [light, mid, dark, _] = colours;
    let tri = |col: Color32, a: Pos2, b: Pos2, d: Pos2| {
        p.add(egui::Shape::convex_polygon(vec![a, b, d], col, Stroke::NONE));
    };
    // Six dark corner facets: each hexagon side with the nearest centre-triangle corner.
    for (k, near) in [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2), (5, 0)] {
        tri(dark, h[k], h[(k + 1) % 6], t[near]);
    }
    // Three mid facets: each side of the centre triangle with the corner across from it.
    tri(mid, t[0], t[1], h[1]);
    tri(mid, t[1], t[2], h[3]);
    tri(mid, t[2], t[0], h[5]);
    tri(light, t[0], t[1], t[2]);
    if !edges {
        return;
    }
    // Seams in the bar's colour, so the facets read as separate planes.
    let seam = Stroke::new((height / 40.0).max(0.6), crate::theme::Tokens::get().app_bar);
    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        p.line_segment([a, b], seam);
    }
    for (k, near) in [(1, 0), (1, 1), (3, 1), (3, 2), (5, 2), (5, 0), (0, 0), (2, 1), (4, 2)] {
        p.line_segment([h[k], t[near]], seam);
    }
}

pub(crate) fn cross(p: &egui::Painter, r: Rect, s: Stroke) {
    p.line_segment([r.left_top(), r.right_bottom()], s);
    p.line_segment([r.right_top(), r.left_bottom()], s);
}

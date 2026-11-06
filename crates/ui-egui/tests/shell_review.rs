//! Review pictures of the application shell around the viewport (PK-90.shell): the whole main
//! window at 1366 x 768, 1600 x 1000 and 1760 x 1100 in the dark and the light theme, and each
//! shell region, painted by the real app through the test software rasteriser (no window or GPU in
//! CI). Written to `target/review-media/<region>-<stage>.png`, where the stage is
//! `SHELL_REVIEW_STAGE` (default `after`); the same test on the base commit gives `before`. The
//! canvas's share of the window at each size goes to `canvas-share-<stage>.tsv` beside them.
//! Nothing here is committed: the pictures are evidence for a review.

#[path = "support/raster.rs"]
#[allow(dead_code)]
mod raster;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{ColorImage, Rect, TextureId, pos2, vec2};
use polykrom_ui_egui::{
    scenario::{self, Harness},
    theme,
};
use serde_json::json;

/// The windows the pictures show: a laptop's, the kit icons review's, and view 01's frame.
const WINDOWS: [egui::Vec2; 3] = [vec2(1366.0, 768.0), vec2(1600.0, 1000.0), vec2(1760.0, 1100.0)];

fn crop(shot: &image::RgbaImage, bounds: Rect, ppp: f32) -> image::RgbaImage {
    let bounds = (bounds * ppp).intersect(Rect::from_min_size(egui::Pos2::ZERO, vec2(shot.width() as f32, shot.height() as f32)));
    image::imageops::crop_imm(shot, bounds.left() as u32, bounds.top() as u32, bounds.width() as u32, bounds.height() as u32).to_image()
}

#[test]
fn main_window_and_shell_regions() {
    let stage = std::env::var("SHELL_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    let mut shares = String::from("window\ttheme\tcanvas\tshare\tchrome_bottom\n");
    for window in WINDOWS {
        for dark in [true, false] {
            let mode = if dark { "dark" } else { "light" };
            let size = format!("{}x{}", window.x as u32, window.y as u32);
            let mut h = Harness::new();
            h.app.home.open = false;
            h.call("ui.set", json!({"dark": dark}));
            h.call("ui.resize", json!({"width": window.x, "height": window.y}));
            h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
            h.frames(3);
            // The last feature selected, as view 01 selects Hole 02.
            if let Some(last) = h.app.session.doc.features.last().map(|f| f.id) {
                h.app.run("select.set", json!({"items": [{"type": "feature", "id": last}]})).unwrap();
            }
            h.frames(3);
            // A fresh context transfers every texture (fonts and icons) to the software renderer.
            h.ctx = egui::Context::default();
            polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
            theme::apply(&h.ctx);
            let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
            for _ in 0..8 {
                raster::paint_sized(&mut h, &mut textures, vec![], window);
            }
            let output = raster::paint_sized(&mut h, &mut textures, vec![], window);
            let ppp = output.pixels_per_point;
            let shot = raster::raster_sized(&mut h, &textures, output, window);
            shot.save(dir.join(format!("main-window-{size}-{mode}-{stage}.png"))).unwrap();
            // The canvas (the pickable scene) and its share of the window.
            let scene = h.app.viewport.rect.expect("the scene is drawn");
            let share = f64::from(scene.area()) / f64::from(window.x * window.y);
            let chrome_bottom =
                scenario::rect_of("review:prompt_row").map_or_else(|| scenario::rect_of("toolbar").map_or(0.0, |r| r.bottom()), |r| r.bottom());
            shares += &format!(
                "{size}\t{mode}\t{:.0} {:.0} {:.0} {:.0}\t{:.1}%\t{chrome_bottom:.0}\n",
                scene.left(),
                scene.top(),
                scene.width(),
                scene.height(),
                share * 100.0
            );
            // What the pictures must show: the title bar in the theme's bar colour (light in the
            // light theme), and the assistant's sparkle drawn in the status bar in both themes.
            let t = theme::Tokens::get();
            let px = |x: f32, y: f32| {
                let p = shot.get_pixel((x * ppp) as u32, (y * ppp) as u32);
                egui::Color32::from_rgb(p[0], p[1], p[2])
            };
            // Blank bar space between the document identity and Undo, on the bar's centre line.
            let identity = scenario::rect_of("appbar:document").expect("the document identity");
            let undo = scenario::rect_of("appbar:undo").expect("Undo");
            let bar = px((identity.right() + undo.left()) / 2.0, identity.center().y);
            let close = |a: egui::Color32, b: egui::Color32| a.r().abs_diff(b.r()) <= 1 && a.g().abs_diff(b.g()) <= 1 && a.b().abs_diff(b.b()) <= 1;
            assert!(close(bar, t.app_bar), "{size} {mode}: the title bar is filled with the theme's bar colour: {bar:?} vs {:?}", t.app_bar);
            let light_bar = egui::Rgba::from(bar).intensity() > 0.5;
            assert_eq!(light_bar, !dark, "{size} {mode}: the bar is light in the light theme and dark in the dark");
            // The gaps between the cards of the top chrome (under the title bar, around the ribbon
            // card, around the prompt row) and the margins beside them show the theme's own base
            // colour, never the host's clear colour. (The base commit left them to it, white in
            // this rasteriser: that is what the before pictures show.)
            if stage != "before" {
                let ribbon = scenario::rect_of("ribbon:card").expect("the ribbon card");
                let prompt = scenario::rect_of("review:prompt_row").expect("the prompt row");
                let title_bottom = polykrom_ui_egui::toolbar::APP_BAR_HEIGHT;
                // Clear of the cards' soft shadows (4 points around a card, 2 points lower).
                let ground = |x: f32, y: f32, what: &str| {
                    let p = px(x, y);
                    assert!(close(p, t.window), "{size} {mode}: {what} at ({x}, {y}) is {p:?}, not the base {:?}", t.window);
                };
                ground(window.x / 2.0, title_bottom + 1.0, "the gap under the title bar");
                ground(1.0, ribbon.center().y, "the margin left of the ribbon card");
                ground(window.x - 2.0, ribbon.center().y, "the margin right of the ribbon card");
                ground(1.0, prompt.center().y, "the margin left of the prompt row");
                ground(1.0, (prompt.bottom() + scene.top()) / 2.0, "the margin beside the gap under the prompt row");
                // No band in any gap is lighter than the base: the shadows under the cards only
                // darken it. (The cards' own fine edges, a point either side of their bounds,
                // are not the gap.)
                let base = egui::Rgba::from(t.window).intensity();
                let between = |above: f32, below: f32| (above.ceil() as u32 + 1)..(below.floor() as u32).saturating_sub(1);
                let rows =
                    between(title_bottom, ribbon.top()).chain(between(ribbon.bottom(), prompt.top())).chain(between(prompt.bottom(), scene.top()));
                for y in rows {
                    for x in [window.x * 0.25, window.x * 0.5, window.x * 0.75] {
                        let p = px(x, y as f32);
                        assert!(
                            egui::Rgba::from(p).intensity() <= base + 0.01,
                            "{size} {mode}: ({x}, {y}) is {p:?}, lighter than the base {:?}: a bright band between the cards",
                            t.window
                        );
                    }
                }
            }
            if let Some(assistant) = scenario::rect_of("titlebar:assistant") {
                let glyph = Rect::from_center_size(assistant.center(), vec2(14.0, 14.0));
                let ground = egui::Rgba::from(t.window).intensity();
                let mut inked = 0;
                for y in (glyph.top() as u32)..(glyph.bottom() as u32) {
                    for x in (glyph.left() as u32)..(glyph.right() as u32) {
                        if (egui::Rgba::from(px(x as f32, y as f32)).intensity() - ground).abs() > 0.25 {
                            inked += 1;
                        }
                    }
                }
                assert!(inked >= 12, "{size} {mode}: the assistant's sparkle is drawn in the status bar ({inked} inked pixels in {glyph:?})");
            }
            if window != WINDOWS[2] {
                continue;
            }
            // The regions, from view 01's frame.
            let top = scenario::rect_of("toolbar").map_or(190.0, |r| r.bottom() + 4.0);
            let browser = scenario::rect_of("browser:card").unwrap_or(Rect::from_min_size(pos2(0.0, top), vec2(300.0, 600.0)));
            let history = scenario::rect_of("history:card")
                .or_else(|| scenario::rect_of("history:strip"))
                .unwrap_or(Rect::from_min_max(pos2(0.0, window.y - 110.0), pos2(window.x, window.y - 36.0)));
            let strip = scenario::rect_of("documents:strip").unwrap_or(Rect::from_min_size(pos2(browser.right(), top), vec2(600.0, 80.0)));
            let regions = [
                ("title-bar", Rect::from_min_max(pos2(0.0, 0.0), pos2(window.x, 40.0))),
                ("tabs-and-ribbon", Rect::from_min_max(pos2(0.0, 30.0), pos2(window.x, top))),
                ("design-browser", Rect::from_min_max(pos2(0.0, browser.top() - 4.0), pos2(browser.right() + 4.0, browser.bottom() + 4.0))),
                ("document-tabs-and-view-bar", Rect::from_min_max(pos2(browser.right(), strip.top() - 8.0), pos2(window.x, strip.bottom() + 8.0))),
                ("design-history", Rect::from_min_max(pos2(0.0, history.top() - 4.0), pos2(window.x, history.bottom() + 4.0))),
                ("status-bar", Rect::from_min_max(pos2(0.0, window.y - 44.0), pos2(window.x, window.y))),
            ];
            for (what, rect) in regions {
                let rect = rect.intersect(Rect::from_min_size(egui::Pos2::ZERO, window));
                assert!(rect.width() > 40.0 && rect.height() > 20.0, "{mode} {what}: {rect:?}");
                crop(&shot, rect, ppp).save(dir.join(format!("{what}-{mode}-{stage}.png"))).unwrap();
            }
        }
    }
    std::fs::write(dir.join(format!("canvas-share-{stage}.tsv")), &shares).unwrap();
    print!("{shares}");
    theme::set_dark(previous);
}

/// The window painted over the model: the headless canvas draws no model, so the sample design
/// is drawn by the CPU rasteriser as `ui.render` draws it (the grid on, the camera close, so the
/// plate runs under the cards) and put in the frame in place of the canvas's empty gradient,
/// under every card. The picture and its pixels per point.
fn painted_over_model(h: &mut Harness, dir: &Path, mode: &str, window: egui::Vec2) -> (image::RgbaImage, f32) {
    h.ctx = egui::Context::default();
    polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..8 {
        raster::paint_sized(h, &mut textures, vec![], window);
    }
    let mut output = raster::paint_sized(h, &mut textures, vec![], window);
    let ppp = output.pixels_per_point;
    let scene = h.app.viewport.rect.expect("the scene is drawn");
    // The model as the window shows it, at the scene's size.
    let (w, hgt) = ((scene.width() * ppp).round() as u32, (scene.height() * ppp).round() as u32);
    let backdrop = dir.join(format!("glass-backdrop-{mode}.png"));
    let r = h.call("ui.render", json!({"path": backdrop.to_string_lossy(), "width": w, "height": hgt, "grid": true}));
    assert_eq!(r["ok"], true, "{r}");
    let model = image::open(&backdrop).unwrap().to_rgba8();
    std::fs::remove_file(&backdrop).ok();
    let tex = TextureId::User(0xB1);
    textures.insert(tex, ColorImage::from_rgba_unmultiplied([model.width() as usize, model.height() as usize], model.as_raw()));
    // In the frame, right over the canvas's gradient (the four-vertex mesh spanning the scene)
    // and under everything painted after it.
    let gradient = output
        .shapes
        .iter()
        .position(|s| match &s.shape {
            egui::Shape::Mesh(m) if m.vertices.len() == 4 && m.texture_id == TextureId::default() => {
                let bounds = Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>());
                (bounds.min - scene.min).length() < 1.0 && (bounds.max - scene.max).length() < 1.0
            }
            _ => false,
        })
        .expect("the canvas's gradient is in the frame");
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    output
        .shapes
        .insert(gradient + 1, egui::epaint::ClippedShape { clip_rect: scene, shape: egui::Shape::image(tex, scene, uv, egui::Color32::WHITE) });
    println!("{mode}: painted over {} ({w} x {hgt} px of model, renderer {})", scene, r["result"]["renderer"]);
    (raster::raster_sized(h, &textures, output, window), ppp)
}

/// The glass cards over a busy canvas (the owner's direction, 2026-11-06: the Design Browser a
/// glass plane, the other floating cards the same look) at 1600 x 1000, dark and light, with the
/// AI review flow's card and its canvas status line (a proposal to thicken the plate), and with
/// the Properties card (the sample's last feature selected; r7: glass like the other cards):
/// the Design Browser, the document strip, the history, the Review changes card, the status
/// line and the Properties card, each over the model. Written to
/// `target/review-media/glass-<surface>-<mode>-<stage>.png`.
#[test]
fn glass_cards_over_a_busy_canvas() {
    const WINDOW: egui::Vec2 = vec2(1600.0, 1000.0);
    let stage = std::env::var("SHELL_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    // No GPU here: the CPU rasteriser draws the model.
    polykrom_engine::capture::install(std::sync::Arc::new(|scene, cam, w, h| {
        let canvas = polykrom_engine::render::render(scene, cam, w as usize, h as usize);
        Ok(polykrom_engine::capture::Picture { rgba: canvas.rgba, renderer: "cpu" })
    }));
    let previous = theme::is_dark();
    for dark in [true, false] {
        let mode = if dark { "dark" } else { "light" };
        for state in ["review", "properties"] {
            let mut h = Harness::new();
            h.app.home.open = false;
            h.call("ui.set", json!({"dark": dark}));
            h.call("ui.resize", json!({"width": WINDOW.x, "height": WINDOW.y}));
            h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
            h.frames(3);
            if state == "review" {
                // The AI review flow's card and its canvas status line: a proposal to thicken the plate.
                let steps = json!([{"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}}]);
                let r = h
                    .call("ui.review", json!({"prompt": "Make the plate 10 mm thick", "propose": {"commands": steps, "label": "Plate 10 mm thick"}}));
                assert_eq!(r["ok"], true, "{r}");
            } else if let Some(last) = h.app.session.doc.features.last().map(|f| f.id) {
                // The last feature selected, as view 01 selects Hole 02: the Properties card.
                h.app.run("select.set", json!({"items": [{"type": "feature", "id": last}]})).unwrap();
            }
            h.frames(3);
            // The camera close in: the plate runs under the cards, the grid fills the rest.
            h.app.fit_view();
            h.app.cam_anim = None;
            h.frames(2);
            h.app.cam.distance *= 0.5;
            h.app.cam_anim = None;
            h.frames(3);
            let (shot, ppp) = painted_over_model(&mut h, &dir, mode, WINDOW);
            if state == "review" {
                shot.save(dir.join(format!("glass-main-window-1600x1000-{mode}-{stage}.png"))).unwrap();
            } else {
                shot.save(dir.join(format!("glass-main-window-properties-1600x1000-{mode}-{stage}.png"))).unwrap();
            }
            let crops: &[(&str, &str)] = if state == "review" {
                &[
                    ("design-browser", "browser:card"),
                    ("document-strip", "document_bar:card"),
                    ("design-history", "history:card"),
                    ("review-card", "review:panel"),
                    ("status-line", "review:status"),
                ]
            } else {
                &[("properties-card", "properties:panel")]
            };
            for (what, handle) in crops {
                let Some(card) = scenario::rect_of(handle) else {
                    println!("{mode}: no {handle} in this shell, no glass-{what} picture");
                    continue;
                };
                let rect = card.expand(24.0).intersect(Rect::from_min_size(egui::Pos2::ZERO, WINDOW));
                crop(&shot, rect, ppp).save(dir.join(format!("glass-{what}-{mode}-{stage}.png"))).unwrap();
            }
        }
    }
    theme::set_dark(previous);
}

/// A short window (1366 x 650, a laptop with the taskbar showing) in the docked and the
/// floating layout, dark and light, with two designs open: the document tabs must stay on
/// screen (PK-90.shell-r3 defect 1). Written to `target/review-media/short-window-<layout>-<mode>-<stage>.png`.
#[test]
fn short_window_keeps_the_document_tabs() {
    const SHORT: egui::Vec2 = vec2(1366.0, 650.0);
    let stage = std::env::var("SHELL_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    for layout in ["docked", "floating"] {
        for dark in [true, false] {
            let mode = if dark { "dark" } else { "light" };
            let mut h = Harness::new();
            h.app.home.open = false;
            h.call("ui.set", json!({"dark": dark, "layout": layout}));
            h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
            h.call("ui.documents", json!({"action": "new"}));
            h.app.run("solid.box", json!({"length": 40, "width": 30, "height": 20})).unwrap();
            h.call("ui.documents", json!({"action": "switch", "index": 0}));
            h.frames(3);
            h.ctx = egui::Context::default();
            polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
            theme::apply(&h.ctx);
            let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
            for _ in 0..8 {
                raster::paint_sized(&mut h, &mut textures, vec![], SHORT);
            }
            let output = raster::paint_sized(&mut h, &mut textures, vec![], SHORT);
            let shot = raster::raster_sized(&mut h, &textures, output, SHORT);
            shot.save(dir.join(format!("short-window-{layout}-{mode}-{stage}.png"))).unwrap();
            println!("{layout} {mode}: document tabs {:?}, strip {:?}", scenario::rect_of("tab:design:0"), scenario::rect_of("documents:strip"));
        }
    }
    theme::set_dark(previous);
}

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
use std::path::PathBuf;

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

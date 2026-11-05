//! Review pictures of the application shell around the viewport (PK-90.shell): the whole main
//! window at 1760 x 1100 in the dark and the light theme, and each shell region, painted by the
//! real app through the test software rasteriser (no window or GPU in CI). Written to
//! `target/review-media/<region>-<stage>.png`, where the stage is `SHELL_REVIEW_STAGE` (default
//! `after`); the same test on the base commit gives `before`. Nothing here is committed: the
//! pictures are evidence for a review.

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

/// View 01's frame.
const WINDOW: egui::Vec2 = vec2(1760.0, 1100.0);

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
    for dark in [true, false] {
        let mode = if dark { "dark" } else { "light" };
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.set", json!({"dark": dark}));
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
            raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
        }
        let output = raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
        let ppp = output.pixels_per_point;
        let shot = raster::raster_sized(&mut h, &textures, output, WINDOW);
        shot.save(dir.join(format!("main-window-{mode}-{stage}.png"))).unwrap();
        let top = scenario::rect_of("toolbar").map_or(190.0, |r| r.bottom() + 4.0);
        let browser = scenario::rect_of("browser:card").unwrap_or(Rect::from_min_size(pos2(0.0, top), vec2(300.0, 600.0)));
        let history = scenario::rect_of("history:card")
            .or_else(|| scenario::rect_of("history:strip"))
            .unwrap_or(Rect::from_min_max(pos2(0.0, WINDOW.y - 110.0), pos2(WINDOW.x, WINDOW.y - 36.0)));
        let regions = [
            ("title-bar", Rect::from_min_max(pos2(0.0, 0.0), pos2(WINDOW.x, 40.0))),
            ("tabs-and-ribbon", Rect::from_min_max(pos2(0.0, 30.0), pos2(WINDOW.x, top))),
            ("design-browser", Rect::from_min_max(pos2(0.0, browser.top() - 4.0), pos2(browser.right() + 4.0, browser.bottom() + 4.0))),
            ("document-tabs-and-view-bar", Rect::from_min_max(pos2(browser.right(), top), pos2(WINDOW.x, top + 80.0))),
            ("design-history", Rect::from_min_max(pos2(0.0, history.top() - 4.0), pos2(WINDOW.x, history.bottom() + 4.0))),
            ("status-bar", Rect::from_min_max(pos2(0.0, WINDOW.y - 44.0), pos2(WINDOW.x, WINDOW.y))),
        ];
        for (what, rect) in regions {
            let rect = rect.intersect(Rect::from_min_size(egui::Pos2::ZERO, WINDOW));
            assert!(rect.width() > 40.0 && rect.height() > 20.0, "{mode} {what}: {rect:?}");
            crop(&shot, rect, ppp).save(dir.join(format!("{what}-{mode}-{stage}.png"))).unwrap();
        }
    }
    theme::set_dark(previous);
}

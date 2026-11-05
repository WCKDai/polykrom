//! Review pictures of the surfaces that draw command icons: the application bar and ribbon, the
//! browser and the timeline, painted by the real app through the test software rasteriser (no
//! window or GPU in CI). Written under `target/kit-icons/<stage>/`, where the stage is
//! `KIT_ICON_REVIEW_STAGE` (default `after`); the same test on the previous commit gives
//! `before`. Nothing here is committed: the pictures are evidence for a review.

#[path = "support/raster.rs"]
#[allow(dead_code)]
mod raster;

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{ColorImage, Rect, TextureId};
use polykrom_ui_egui::{
    scenario::{self, Harness},
    theme,
};
use serde_json::json;

const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);

fn crop(shot: &image::RgbaImage, bounds: Rect, ppp: f32) -> image::RgbaImage {
    let bounds = (bounds * ppp).intersect(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(shot.width() as f32, shot.height() as f32)));
    image::imageops::crop_imm(shot, bounds.left() as u32, bounds.top() as u32, bounds.width() as u32, bounds.height() as u32).to_image()
}

/// The union of every published handle rectangle whose name starts with one of `prefixes`.
fn union(prefixes: &[&str]) -> Option<Rect> {
    let mut all: Option<Rect> = None;
    for prefix in prefixes {
        for (name, _) in scenario::handles(prefix) {
            if let Some(r) = scenario::rect_of(&name) {
                all = Some(all.map_or(r, |a| a.union(r)));
            }
        }
    }
    all
}

#[test]
fn toolbar_browser_and_timeline_pictures() {
    let stage = std::env::var("KIT_ICON_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/kit-icons").join(&stage);
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    let mut handles = String::new();
    for dark in [true, false] {
        let mode = if dark { "dark" } else { "light" };
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.set", json!({"dark": dark}));
        h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
        h.frames(3);
        let sketch = h.app.session.world_state().sketches.first().unwrap().feature;
        for tab in ["MODEL", "SKETCH"] {
            if tab == "SKETCH" {
                h.app.run("sketch.edit", json!({"sketch": sketch})).unwrap();
                h.frames(3);
            }
            h.step(&json!({"click": {"handle": format!("tab:{tab}")}})).unwrap();
            h.frames(3);
            assert_eq!(h.app.ui.tab, tab, "{mode}: the {tab} tab is active");
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
            let name = format!("{mode}-{}", tab.to_ascii_lowercase());
            shot.save(dir.join(format!("{name}-full.png"))).unwrap();
            let bar = union(&["appbar:", "toolbar_tab:", "toolbar:", "button:", "ribbon:"]).expect("the application bar and ribbon publish their buttons");
            let toolbar = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(WINDOW.x, bar.bottom() + 8.0));
            let browser = scenario::rect_of("browser:card").expect("the browser card");
            let timeline = scenario::rect_of("history:card").expect("the timeline card");
            for (what, rect) in [("toolbar", toolbar), ("browser", browser), ("timeline", timeline)] {
                assert!(rect.width() > 40.0 && rect.height() > 20.0, "{name} {what}: {rect:?}");
                crop(&shot, rect, ppp).save(dir.join(format!("{name}-{what}.png"))).unwrap();
                handles += &format!("{name}\t{what}\t{:.0} {:.0} {:.0} {:.0}\n", rect.left(), rect.top(), rect.width(), rect.height());
            }
            for (handle, _) in scenario::handles("toolbar:").into_iter().chain(scenario::handles("appbar:")).chain(scenario::handles("timeline:")) {
                if let Some(r) = scenario::rect_of(&handle) {
                    handles += &format!("{name}\t{handle}\t{:.0} {:.0} {:.0} {:.0}\n", r.left(), r.top(), r.width(), r.height());
                }
            }
        }
    }
    std::fs::write(dir.join("handles.tsv"), handles).unwrap();
    theme::set_dark(previous);
}

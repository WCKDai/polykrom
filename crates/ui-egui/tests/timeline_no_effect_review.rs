//! Review pictures of a feature that changed nothing (a hole drilled into the air beside the
//! sample plate, polykrom#309): the window, the timeline and the hovered feature with its tooltip,
//! painted by the real app through the test software rasteriser (no window or GPU in CI), with
//! the command's reply beside them. Written to `target/review-media/` as
//! `<surface>-<stage>.png`, where the stage is `NO_EFFECT_REVIEW_STAGE` (default `after`); the
//! same test on the base commit gives `before`. Nothing here is committed: the pictures are
//! evidence for a review.

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

/// Every text a shape paints, one per line.
fn texts(shape: &egui::Shape, out: &mut String) {
    match shape {
        egui::Shape::Text(t) => {
            out.push_str(&t.galley.job.text);
            out.push('\n');
        }
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, out)),
        _ => {}
    }
}

#[test]
fn a_hole_in_the_air_on_the_timeline() {
    let stage = std::env::var("NO_EFFECT_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    let mut h = Harness::new();
    h.app.home.open = false;
    h.call("ui.set", json!({"dark": true}));
    h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
    // A hole sketched beside the plate, on the plane of its underside: it drills into the air.
    h.app.run("sketch.create", json!({"plane": "XY", "name": "Air"})).unwrap();
    h.app.run("sketch.circle.center", json!({"center": [100, 25], "diameter": 5})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    let reply = h.app.run("solid.hole", json!({"sketch": "Air", "diameter": 5, "depth": 5, "name": "AirHole"})).unwrap();
    std::fs::write(dir.join(format!("reply-{stage}.json")), serde_json::to_string_pretty(&reply).unwrap()).unwrap();
    h.app.run("select.clear", json!({})).unwrap();
    h.frames(3);
    // A fresh context transfers every texture (fonts and icons) to the software renderer.
    h.ctx = egui::Context::default();
    polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..8 {
        raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
    }
    // The pointer moves onto the hole's chip in the timeline and rests there (tooltips show
    // only once the pointer is still) until its tooltip shows.
    let centre = scenario::handles("timeline:").into_iter().find(|(name, _)| name == "timeline:AirHole").map(|(_, at)| at);
    let chip = Rect::from_center_size(centre.expect("the timeline shows AirHole"), egui::vec2(28.0, 28.0));
    raster::paint_sized(&mut h, &mut textures, vec![egui::Event::PointerMoved(chip.center())], WINDOW);
    for _ in 0..30 {
        raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
    }
    let output = raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
    // The tooltip is in the picture: the feature's name and type, and (after the change) its
    // warning.
    let mut painted = String::new();
    for clipped in &output.shapes {
        texts(&clipped.shape, &mut painted);
    }
    assert!(painted.contains("AirHole  ("), "the tooltip of AirHole is painted: {painted}");
    let warned = painted.contains("AirHole has no effect: `Plate` is unchanged");
    assert_eq!(warned, reply["warning"].is_string(), "the tooltip says what the reply says: {painted}");
    let ppp = output.pixels_per_point;
    let shot = raster::raster_sized(&mut h, &textures, output, WINDOW);
    shot.save(dir.join(format!("window-{stage}.png"))).unwrap();
    let timeline = scenario::rect_of("history:card").expect("the timeline card");
    crop(&shot, timeline, ppp).save(dir.join(format!("timeline-{stage}.png"))).unwrap();
    // The chip, its mark and the tooltip above it.
    let zoom = Rect::from_min_max(egui::pos2(chip.left() - 120.0, chip.top() - 160.0), egui::pos2(chip.right() + 320.0, chip.bottom() + 12.0));
    crop(&shot, zoom, ppp).save(dir.join(format!("timeline-zoom-{stage}.png"))).unwrap();
    theme::set_dark(previous);
}

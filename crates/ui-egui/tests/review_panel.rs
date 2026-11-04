//! Review pictures of the AI review flow (design kit views 04 and 05), painted by the real app
//! through the test software rasteriser (no window or GPU in CI): the Review changes panel open on
//! the sample design with the proposed geometry in the viewport, and the receipt toast after
//! Apply & rebuild. The raster draws the window's own shapes, not the viewport's GPU scene, so the
//! model itself is rendered by the engine's headless picture path (the CPU rasteriser here) as
//! `review-model-<stage>.png`: the sample design before, the applied proposal after. Written to
//! `target/review-media/review-panel-<stage>.png`, `review-toast-<stage>.png` and
//! `review-model-<stage>.png`, where the stage is `REVIEW_MEDIA_STAGE` (default `after`); the same
//! test on the base commit, where `ui.review` does not exist, paints the app as it was (`before`).
//! Nothing here is committed: the pictures are evidence for a review.

#[path = "support/raster.rs"]
#[allow(dead_code)]
mod raster;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{ColorImage, TextureId};
use polykrom_ui_egui::{scenario::Harness, theme};
use serde_json::json;

const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);
/// The PR attaches pictures of at most 3 MB.
const MAX_BYTES: u64 = 3 * 1024 * 1024;

fn paint(h: &mut Harness, path: &Path) {
    // A fresh context transfers every texture (fonts, icons, the view) to the software renderer.
    h.ctx = egui::Context::default();
    polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..8 {
        raster::paint_sized(h, &mut textures, vec![], WINDOW);
    }
    let output = raster::paint_sized(h, &mut textures, vec![], WINDOW);
    let shot = raster::raster_sized(h, &textures, output, WINDOW);
    shot.save(path).unwrap();
    let bytes = std::fs::metadata(path).unwrap().len();
    assert!(bytes <= MAX_BYTES, "{}: {bytes} bytes", path.display());
}

/// The model as the viewport's camera sees it, drawn headlessly (the viewport's pipelines on a
/// GPU, else the CPU rasteriser), as `ui.render` draws it.
fn render_model(h: &Harness, path: &Path) {
    polykrom_engine::capture::install(std::sync::Arc::new(|scene, cam, w, h| {
        let (rgba, renderer) = polykrom_gpu::capture::headless_rgba(scene, cam, w, h)?;
        Ok(polykrom_engine::capture::Picture { rgba, renderer })
    }));
    let png = polykrom_ui_egui::control::view_png(&h.app, 1000, 700).unwrap();
    std::fs::write(path, png).unwrap();
    assert!(std::fs::metadata(path).unwrap().len() <= MAX_BYTES);
}

/// Frames until the proposal's preview is up (it computes off the window thread).
fn settle_preview(h: &mut Harness) {
    for _ in 0..400 {
        if h.app.preview.active {
            break;
        }
        h.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    h.frames(3);
}

#[test]
fn review_panel_and_receipt_pictures() {
    let stage = std::env::var("REVIEW_MEDIA_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    let mut h = Harness::new();
    h.app.home.open = false;
    h.call("ui.set", json!({"dark": true}));
    h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
    h.frames(3);
    h.call("ui.camera", json!({"fit": true}));
    h.frames(3);
    let model = dir.join(format!("review-model-{stage}.png"));
    if stage == "before" {
        render_model(&h, &model);
    }

    // View 04: the proposal (the plate 10 mm thick) under review.
    let change = json!({"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}});
    let r = h.call(
        "ui.review",
        json!({"prompt": "Make the plate 10 mm thick. Keep the holes and the boss.", "propose": {"commands": [change], "label": "Plate 10 mm thick"}}),
    );
    // On the base commit there is no `ui.review`: the picture is the app as it was.
    let flow = r["ok"] == true;
    if flow {
        settle_preview(&mut h);
    }
    paint(&mut h, &dir.join(format!("review-panel-{stage}.png")));

    // View 05: applied, with the receipt and its Undo.
    if flow {
        let r = h.call("ui.review", json!({"apply": true}));
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["result"]["receipt"].is_object(), "{r}");
    } else {
        h.call("engine.execute", json!({"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}}));
    }
    h.frames(3);
    paint(&mut h, &dir.join(format!("review-toast-{stage}.png")));
    if stage != "before" {
        render_model(&h, &model);
    }
    theme::set_dark(previous);
}

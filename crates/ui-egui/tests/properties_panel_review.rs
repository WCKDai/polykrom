//! Review pictures of the Properties panel (design kit views 01 and 03), painted by the real app
//! through the test software rasteriser (no window or GPU in CI), in the dark and the light
//! theme: a feature selected (`properties-selected-<theme>-<stage>.png`), the feature edited in
//! place with its diameter changed and Preview on (`properties-editor-…`), an invalid value
//! (`properties-invalid-…`), and a counterbore hole selected with the agent activity list
//! showing (`properties-counterbore-selected-…`) and edited in place (`properties-counterbore-…`:
//! each value once, as a field). The raster draws the window's own shapes, not the viewport's GPU
//! scene (nor the preview scene), so the model is rendered by the engine's headless picture
//! path separately as `properties-model-<stage>.png`. Written to `target/review-media/`, where the stage is
//! `REVIEW_MEDIA_STAGE` (default `after`); the same test on the base commit, where the panel does
//! not exist, paints the app as it was (`before`). Nothing here is committed: the pictures are
//! evidence for a review.

#[path = "support/raster.rs"]
#[allow(dead_code)]
mod raster;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{ColorImage, Rect, TextureId};
use polykrom_ui_egui::{
    scenario::{self, Harness},
    theme,
};
use serde_json::json;

const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);
/// The PR attaches pictures of at most 3 MB.
const MAX_BYTES: u64 = 3 * 1024 * 1024;

/// The design kit's mount, reduced: a plate ("Mount") with one Ø8 hole on a sketch point, a 2 × 2
/// pattern of the hole and an R2 fillet on the hole's edge.
fn mount(h: &mut Harness) {
    for (id, p) in [
        ("sketch.create", json!({"plane": "XY", "name": "Base"})),
        ("sketch.rectangle.two_point", json!({"p0": [-80, -60], "p1": [80, 60]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance": "8 mm", "body_name": "Mount", "name": "Base plate"})),
        ("sketch.create", json!({"plane": "XY", "offset": "8 mm", "name": "Hole centres"})),
        ("sketch.point", json!({"point": [0, 0], "id": "c1"})),
        ("sketch.finish", json!({})),
        ("solid.hole", json!({"sketch": "Hole centres", "points": ["c1"], "diameter": "8 mm", "name": "Mounting holes"})),
        (
            "solid.pattern.rectangular",
            json!({"features": ["Mounting holes"], "dir1": [1, 0, 0], "count1": 2, "spacing1": "40 mm", "dir2": [0, 1, 0], "count2": 2, "spacing2": "30 mm", "name": "Bolt pattern"}),
        ),
        ("solid.fillet", json!({"edges": [[4, 0, 8]], "radius": "2 mm", "name": "Hole fillet"})),
        // A counterbore hole in a corner: its editor shows the counterbore's own values.
        (
            "solid.hole",
            json!({"position": [60, 40, 8], "diameter": "6 mm", "type": "counterbore", "cb_diameter": "11 mm", "cb_depth": "3 mm", "name": "Counterbored hole"}),
        ),
    ] {
        let r = h.call("engine.execute", json!({"command": id, "params": p}));
        assert_eq!(r["ok"], true, "{id}: {r}");
    }
    h.frames(3);
}

fn crop(shot: &image::RgbaImage, bounds: Rect, ppp: f32) -> image::RgbaImage {
    let bounds = (bounds * ppp).intersect(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(shot.width() as f32, shot.height() as f32)));
    image::imageops::crop_imm(shot, bounds.left() as u32, bounds.top() as u32, bounds.width() as u32, bounds.height() as u32).to_image()
}

/// Paint the window; the full picture as `<name>.png`, and the panel (or, without one, the
/// right edge of the window where it would stand) as `<name>-panel.png`. With `focus`, the field
/// of the row so labelled is clicked first, so it is painted with the keyboard (its lime border).
fn paint(h: &mut Harness, dir: &Path, name: &str, focus: Option<&str>) {
    // A fresh context transfers every texture (fonts and icons) to the software renderer. It
    // keeps the accessibility tree the harness finds widgets by (`h.widget`), as the harness's
    // own context has it.
    h.ctx = egui::Context::default();
    h.ctx.enable_accesskit();
    polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..8 {
        raster::paint_sized(h, &mut textures, vec![], WINDOW);
    }
    if let Some(label) = focus
        && let Some(field) = scenario::rect_of(&format!("dialog:control:{label}"))
    {
        let at = field.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        raster::paint_sized(h, &mut textures, vec![egui::Event::PointerMoved(at)], WINDOW);
        raster::paint_sized(h, &mut textures, vec![press(true)], WINDOW);
        raster::paint_sized(h, &mut textures, vec![press(false)], WINDOW);
        raster::paint_sized(h, &mut textures, vec![], WINDOW);
        assert!(h.ctx.memory(|m| m.focused().is_some()), "{name}: the {label} field has the keyboard");
    }
    let output = raster::paint_sized(h, &mut textures, vec![], WINDOW);
    let ppp = output.pixels_per_point;
    let shot = raster::raster_sized(h, &textures, output, WINDOW);
    let full = dir.join(format!("{name}.png"));
    shot.save(&full).unwrap();
    let panel = scenario::rect_of("properties:panel")
        .map(|r| r.expand(8.0))
        .unwrap_or_else(|| Rect::from_min_max(egui::pos2(WINDOW.x - 300.0, 200.0), egui::pos2(WINDOW.x, WINDOW.y)));
    let panel_file = dir.join(format!("{name}-panel.png"));
    crop(&shot, panel, ppp).save(&panel_file).unwrap();
    for file in [full, panel_file] {
        let bytes = std::fs::metadata(&file).unwrap().len();
        assert!(bytes <= MAX_BYTES, "{}: {bytes} bytes", file.display());
    }
}

/// The model as the viewport's camera sees it, drawn headlessly, as `ui.render` draws it.
fn render_model(h: &Harness, path: &Path) {
    polykrom_engine::capture::install(std::sync::Arc::new(|scene, cam, w, h| {
        let (rgba, renderer) = polykrom_gpu::capture::headless_rgba(scene, cam, w, h)?;
        Ok(polykrom_engine::capture::Picture { rgba, renderer })
    }));
    let png = polykrom_ui_egui::control::view_png(&h.app, 1000, 700).unwrap();
    std::fs::write(path, png).unwrap();
    assert!(std::fs::metadata(path).unwrap().len() <= MAX_BYTES);
}

/// Frames until the preview is up (it computes off the window thread).
fn settle_preview(h: &mut Harness) {
    for _ in 0..400 {
        if h.app.preview.active || h.app.preview.error.is_some() {
            break;
        }
        h.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    h.frames(3);
}

/// Type `text` into the field of the row labelled `label` (select all, then type).
fn type_into(h: &mut Harness, label: &str, text: &str) -> bool {
    let Some(field) = scenario::rect_of(&format!("dialog:control:{label}")) else { return false };
    h.step(&json!({"click": [field.center().x, field.center().y]})).unwrap();
    h.frames(2);
    h.step(&json!({"key": "A", "cmd": true})).unwrap();
    h.step(&json!({"text": text})).unwrap();
    h.frames(3);
    true
}

#[test]
fn properties_panel_pictures() {
    let stage = std::env::var("REVIEW_MEDIA_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    for dark in [true, false] {
        let mode = if dark { "dark" } else { "light" };
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.set", json!({"dark": dark}));
        mount(&mut h);
        h.call("ui.camera", json!({"fit": true}));
        h.frames(3);
        if dark && stage != "before" {
            render_model(&h, &dir.join(format!("properties-model-{stage}.png")));
        }
        // View 01: the hole selected.
        let hole = h.app.session.doc.find_feature("Mounting holes").unwrap().id;
        h.call("engine.execute", json!({"command": "select.set", "params": {"items": [{"type": "feature", "id": hole}]}}));
        h.frames(3);
        paint(&mut h, &dir, &format!("properties-selected-{mode}-{stage}"), None);
        // View 03: Edit feature, Ø10, Preview on, the Diameter field focused (its lime border).
        let editor = h.widget("Edit feature").is_some();
        if editor {
            h.step(&json!({"widget": "Edit feature"})).unwrap();
            h.frames(3);
            assert!(h.app.properties.editor.is_some(), "{mode}: the hole is edited in place");
            assert!(type_into(&mut h, "Diameter", "10 mm"), "{mode}: the Diameter field");
            settle_preview(&mut h);
            assert!(h.app.preview.active, "{mode}: the Ø10 holes are previewed: {:?}", h.app.preview.error);
        }
        paint(&mut h, &dir, &format!("properties-editor-{mode}-{stage}"), editor.then_some("Diameter"));
        // Validation: a value that is not a length.
        if editor {
            assert!(type_into(&mut h, "Diameter", "abc"));
        }
        paint(&mut h, &dir, &format!("properties-invalid-{mode}-{stage}"), None);
        if editor {
            h.step(&json!({"key": "Escape"})).unwrap();
            h.frames(2);
        }
        // The counterbore hole selected (`properties-counterbore-selected-…`), the agent
        // activity list showing the commands run over the control channel, then edited in place
        // (`properties-counterbore-…`): each of its values once, as a field.
        let counterbore = h.app.session.doc.find_feature("Counterbored hole").unwrap().id;
        h.call("engine.execute", json!({"command": "select.set", "params": {"items": [{"type": "feature", "id": counterbore}]}}));
        h.frames(3);
        assert!(!h.app.agent_feed.entries.is_empty(), "{mode}: the agent activity list shows");
        paint(&mut h, &dir, &format!("properties-counterbore-selected-{mode}-{stage}"), None);
        if let (Some(list), Some(panel)) = (scenario::rect_of("agent_activity"), scenario::rect_of("properties:panel")) {
            assert!(!list.intersect(panel).is_positive(), "{mode}: the activity list {list:?} is drawn under the card {panel:?}");
        }
        if editor {
            h.step(&json!({"widget": "Edit feature"})).unwrap();
            h.frames(3);
            assert!(h.app.properties.editor.is_some(), "{mode}: the counterbore hole is edited in place");
            settle_preview(&mut h);
        }
        paint(&mut h, &dir, &format!("properties-counterbore-{mode}-{stage}"), editor.then_some("Diameter"));
        if editor {
            h.step(&json!({"key": "Escape"})).unwrap();
            h.frames(2);
        }
    }
    theme::set_dark(previous);
}

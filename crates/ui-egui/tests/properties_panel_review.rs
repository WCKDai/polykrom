//! Review pictures of the Properties panel (design kit views 01 and 03), painted by the real app
//! through the test software rasteriser (no window or GPU in CI), in the dark and the light
//! theme: a feature selected (`properties-selected-<theme>-<stage>.png`), the feature edited in
//! place with its diameter changed and Preview on (`properties-editor-…`), an invalid value
//! (`properties-invalid-…`), and a counterbore hole selected with the agent activity list
//! showing (`properties-counterbore-selected-…`) and edited in place (`properties-counterbore-…`:
//! each value once, as a field), and, in a 1366 × 768 window, the hole selected with the agent
//! activity list and a status toast showing (`properties-toast-…`: a short one, and `…-long-…`
//! an error as wide as a toast gets), where the list, the toast, the card and the viewport's
//! controls must all stand clear of one another. In a 1366 × 768 and a 1280 × 720 window, the
//! hole selected (`properties-selected-<size>-…`) and edited in place (`properties-editor-<size>-…`):
//! the card's actions and footer stand at its bottom and the body above them scrolls, shown
//! scrolled to its end in `…-<size>-scrolled-…-panel.png`. The raster draws the window's own shapes, not
//! the viewport's GPU scene (nor the preview scene), so the model is rendered by the engine's
//! headless picture path separately as `properties-model-<stage>.png`. Written to
//! `target/review-media/`, where the stage is `REVIEW_MEDIA_STAGE` (default `after`); the same
//! test on the base commit, where the panel does not exist, paints the app as it was (`before`),
//! so it reads the panel's state through the scenario API only, never through app fields the
//! base commit lacks. Nothing here is committed: the pictures are evidence for a review.

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
/// A laptop window: the Properties card, the agent activity list and a toast share less room.
const LAPTOP: egui::Vec2 = egui::vec2(1366.0, 768.0);
/// A small window: the Properties card's body is shorter than its content.
const SMALL: egui::Vec2 = egui::vec2(1280.0, 720.0);
/// The toasts shown in the laptop window: a status, and an error as long as they get.
const SHORT_TOAST: &str = "Counterbored hole: 1 feature updated";
const LONG_TOAST: &str = "Hole fillet failed: the R2 blend of the counterbored hole's top edge self-intersects where the edge \
                          meets the Ø11 counterbore, so no fillet surface could be built for it. Reduce the radius to 1.5 mm or \
                          less, or fillet the hole's bottom edge instead; the dependent features were left as they were.";
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

/// What is done in the painted window before its picture is taken.
#[derive(Clone, Copy)]
enum Act<'a> {
    /// Nothing: the window as it is.
    Nothing,
    /// The field of the row so labelled is clicked, so it is painted with the keyboard (its lime
    /// border).
    Focus(&'a str),
    /// The wheel over the Properties card scrolls its body to the end, the pointer then away.
    ScrollToEnd,
}

/// Paint the window, `window` large; the full picture as `<name>.png` (when `whole`), and the
/// panel (or, without one, the right edge of the window where it would stand) as
/// `<name>-panel.png`, after `act`.
fn paint(h: &mut Harness, dir: &Path, name: &str, act: Act<'_>, window: egui::Vec2, whole: bool) {
    // A fresh context transfers every texture (fonts and icons) to the software renderer. It
    // keeps the accessibility tree the harness finds widgets by (`h.widget`), as the harness's
    // own context has it.
    h.ctx = egui::Context::default();
    h.ctx.enable_accesskit();
    polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..8 {
        raster::paint_sized(h, &mut textures, vec![], window);
    }
    match act {
        Act::Nothing => {}
        Act::Focus(label) => {
            if let Some(field) = scenario::rect_of(&format!("dialog:control:{label}")) {
                let at = field.center();
                let press =
                    |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                raster::paint_sized(h, &mut textures, vec![egui::Event::PointerMoved(at)], window);
                raster::paint_sized(h, &mut textures, vec![press(true)], window);
                raster::paint_sized(h, &mut textures, vec![press(false)], window);
                raster::paint_sized(h, &mut textures, vec![], window);
                assert!(h.ctx.memory(|m| m.focused().is_some()), "{name}: the {label} field has the keyboard");
            }
        }
        Act::ScrollToEnd => {
            let card = scenario::rect_of("properties:panel").unwrap_or_else(|| panic!("{name}: the Properties card"));
            let at = egui::pos2(card.center().x, card.top() + card.height() * 0.3);
            let wheel = egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -5000.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            };
            raster::paint_sized(h, &mut textures, vec![egui::Event::PointerMoved(at)], window);
            raster::paint_sized(h, &mut textures, vec![wheel], window);
            raster::paint_sized(h, &mut textures, vec![egui::Event::PointerGone], window);
            // The wheel's scrolling is smoothed over a few frames.
            for _ in 0..12 {
                raster::paint_sized(h, &mut textures, vec![], window);
            }
        }
    }
    let output = raster::paint_sized(h, &mut textures, vec![], window);
    let ppp = output.pixels_per_point;
    let shot = raster::raster_sized(h, &textures, output, window);
    let mut files = Vec::new();
    if whole {
        let full = dir.join(format!("{name}.png"));
        shot.save(&full).unwrap();
        files.push(full);
    }
    let panel = scenario::rect_of("properties:panel")
        .map(|r| r.expand(8.0))
        .unwrap_or_else(|| Rect::from_min_max(egui::pos2(window.x - 300.0, 200.0), egui::pos2(window.x, window.y)));
    let panel_file = dir.join(format!("{name}-panel.png"));
    crop(&shot, panel, ppp).save(&panel_file).unwrap();
    files.push(panel_file);
    for file in files {
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

/// The hole is edited in place: the editor's Apply button is there. Read through the harness's
/// widget tree, which the base commit (no panel, no editor) answers too.
fn editing(h: &Harness) -> bool {
    h.widget("Apply change").is_some()
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
        paint(&mut h, &dir, &format!("properties-selected-{mode}-{stage}"), Act::Nothing, WINDOW, true);
        // View 03: Edit feature, Ø10, Preview on, the Diameter field focused (its lime border).
        let editor = h.widget("Edit feature").is_some();
        if editor {
            h.step(&json!({"widget": "Edit feature"})).unwrap();
            h.frames(3);
            assert!(editing(&h), "{mode}: the hole is edited in place");
            assert!(type_into(&mut h, "Diameter", "10 mm"), "{mode}: the Diameter field");
            settle_preview(&mut h);
            assert!(h.app.preview.active, "{mode}: the Ø10 holes are previewed: {:?}", h.app.preview.error);
        }
        paint(&mut h, &dir, &format!("properties-editor-{mode}-{stage}"), if editor { Act::Focus("Diameter") } else { Act::Nothing }, WINDOW, true);
        // Validation: a value that is not a length.
        if editor {
            assert!(type_into(&mut h, "Diameter", "abc"));
        }
        paint(&mut h, &dir, &format!("properties-invalid-{mode}-{stage}"), Act::Nothing, WINDOW, true);
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
        paint(&mut h, &dir, &format!("properties-counterbore-selected-{mode}-{stage}"), Act::Nothing, WINDOW, true);
        if let (Some(list), Some(panel)) = (scenario::rect_of("agent_activity"), scenario::rect_of("properties:panel")) {
            assert!(!list.intersect(panel).is_positive(), "{mode}: the activity list {list:?} is drawn under the card {panel:?}");
        }
        if editor {
            h.step(&json!({"widget": "Edit feature"})).unwrap();
            h.frames(3);
            assert!(editing(&h), "{mode}: the counterbore hole is edited in place");
            settle_preview(&mut h);
        }
        paint(
            &mut h,
            &dir,
            &format!("properties-counterbore-{mode}-{stage}"),
            if editor { Act::Focus("Diameter") } else { Act::Nothing },
            WINDOW,
            true,
        );
        if editor {
            h.step(&json!({"key": "Escape"})).unwrap();
            h.frames(2);
        }
        // A laptop window, the counterbore hole selected, the activity list showing and a toast
        // up (`properties-toast-…`: a status; `…-long-…`: an error as wide as a toast gets). That
        // the list, the toast, the card, the navigation bar and the compass stand clear of one
        // another is the agent activity module's test; the pictures show it, before and after.
        h.call("ui.resize", json!({"width": LAPTOP.x, "height": LAPTOP.y}));
        h.frames(3);
        for (name, text) in [("toast", SHORT_TOAST), ("toast-long", LONG_TOAST)] {
            h.app.status = Some((text.to_string(), polykrom_ui_egui::now_ms(), text == LONG_TOAST));
            paint(&mut h, &dir, &format!("properties-{name}-{mode}-{stage}"), Act::Nothing, LAPTOP, true);
            assert!(scenario::rect_of("toast").is_some(), "{mode}: the {name} shows");
            assert!(!h.app.agent_feed.entries.is_empty(), "{mode}: the agent activity list shows");
        }
        h.app.status = None;
        // The laptop window and a small one, the hole with dependents selected
        // (`properties-selected-<size>-…`) and edited in place (`properties-editor-<size>-…`):
        // the card's actions and footer stand at its bottom, in sight, and the body scrolls; the
        // `…-scrolled-…-panel.png` crops show it scrolled to its end. That the actions and the
        // footer are inside the card and the body scrolls to its last row is the panel's own
        // test; the pictures show it, before and after.
        // Each view is scrolled after its other pictures: scrolled, the base commit's card has
        // its Edit feature button out of sight, where a click cannot reach it.
        for size in [LAPTOP, SMALL] {
            let tag = format!("{}x{}", size.x, size.y);
            h.call("ui.resize", json!({"width": size.x, "height": size.y}));
            h.call("engine.execute", json!({"command": "select.set", "params": {"items": [{"type": "feature", "id": hole}]}}));
            h.frames(3);
            paint(&mut h, &dir, &format!("properties-selected-{tag}-{mode}-{stage}"), Act::Nothing, size, true);
            if editor {
                h.step(&json!({"widget": "Edit feature"})).unwrap();
                h.frames(3);
                if !editing(&h) {
                    // The base commit's card clips the button out of the pointer's reach at
                    // 1280 × 720: the editor opens as the button would open it.
                    h.call("engine.execute", json!({"command": "select.set", "params": {"items": [{"type": "feature", "id": hole}]}}));
                    polykrom_ui_egui::design_properties::begin_edit(&mut h.app, hole);
                    h.frames(3);
                }
                assert!(editing(&h), "{mode} {tag}: the hole is edited in place");
                settle_preview(&mut h);
            }
            paint(&mut h, &dir, &format!("properties-editor-{tag}-{mode}-{stage}"), Act::Nothing, size, true);
            paint(&mut h, &dir, &format!("properties-editor-{tag}-scrolled-{mode}-{stage}"), Act::ScrollToEnd, size, false);
            if editor {
                h.step(&json!({"key": "Escape"})).unwrap();
                h.frames(2);
                assert!(!editing(&h), "{mode} {tag}: Esc cancels the edit");
            }
            paint(&mut h, &dir, &format!("properties-selected-{tag}-scrolled-{mode}-{stage}"), Act::ScrollToEnd, size, false);
        }
    }
    theme::set_dark(previous);
}

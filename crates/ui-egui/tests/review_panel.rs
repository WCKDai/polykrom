//! Review pictures of the AI review flow (design kit views 04 and 05), painted by the real app
//! through the test software rasteriser (no window or GPU in CI), in the dark and the light
//! theme, on the kit's actuator mount (four Ø8 mounting holes on an 84 × 44 pitch, an R2 fillet
//! after them): the prompt row blocked (no agent set up) with its reason, the prompt row with
//! the request typed and an agent set up, the request pending while the agent starts, the Review
//! changes card with the proposed Ø10 holes previewed, and the applied toast with Undo. The raster
//! draws the window's own shapes, not the viewport's GPU scene, so the model itself is rendered by
//! the engine's headless picture path as `review-model-before.png` (before Apply, the Ø8 holes)
//! and `review-model-after.png` (after it, Ø10). The card is also painted in 1366 × 768 and
//! 1280 × 720 windows, and at 1366 × 768 in Japanese and Chinese, where its body scrolls under
//! the pinned actions. Written to `target/review-media/<surface>-<mode>-<stage>.png`, where
//! the stage is `REVIEW_MEDIA_STAGE` (default `after`); the same test on the base commit paints
//! the app as it was (`before`). Nothing here is committed: the pictures are evidence for a review.

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
use serde_json::{Value, json};

const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);
/// The PR attaches pictures of at most 3 MB.
const MAX_BYTES: u64 = 3 * 1024 * 1024;
/// The kit's request (view 04).
const REQUEST: &str = "Increase the mounting holes to Ø10. Keep the 84 × 44 pattern.";

/// The kit's actuator mount: a 120 × 80 × 8 base, a Ø44 boss with a Ø24 bore, four Ø8 mounting
/// holes at the points of a sketch on an 84 × 44 pitch, and an R2 edge fillet after them.
fn actuator_mount() -> Value {
    json!({"commands": [
        {"command": "sketch.create", "params": {"plane": "XY", "name": "Base profile"}},
        {"command": "sketch.rectangle.two_point", "params": {"p0": [-60, -40], "p1": [60, 40]}},
        {"command": "sketch.finish", "params": {}},
        {"command": "solid.extrude", "params": {"distance": 8, "body_name": "Mount", "name": "Base"}},
        {"command": "sketch.create", "params": {"plane": "XY", "offset": 8, "name": "Boss profile"}},
        {"command": "sketch.circle.center", "params": {"center": [0, 0], "diameter": 44}},
        {"command": "sketch.finish", "params": {}},
        {"command": "solid.extrude", "params": {"distance": 14, "operation": "join", "name": "Boss"}},
        {"command": "solid.hole", "params": {"position": [0, 0, 22], "diameter": 24, "name": "Bore"}},
        {"command": "sketch.create", "params": {"plane": "XY", "offset": 8, "name": "Mounting pattern"}},
        {"command": "sketch.point", "params": {"point": [-42, -22]}},
        {"command": "sketch.point", "params": {"point": [42, -22]}},
        {"command": "sketch.point", "params": {"point": [42, 22]}},
        {"command": "sketch.point", "params": {"point": [-42, 22]}},
        {"command": "sketch.finish", "params": {}},
        {"command": "solid.hole", "params": {"sketch": "Mounting pattern", "diameter": 8, "name": "Mounting holes"}},
        {"command": "solid.fillet", "params": {"edges": [[-60, -40, 4], [60, -40, 4], [60, 40, 4], [-60, 40, 4]], "radius": "2 mm", "name": "Edge finish"}},
    ]})
}

/// One laid-out frame of the app, rasterised and saved; the frame's output for crops.
fn paint(h: &mut Harness, path: &Path) -> (image::RgbaImage, f32) {
    paint_in(h, path, WINDOW, "en")
}

/// One laid-out frame of the app in a window of `size` points, with the fonts of `language`.
fn paint_in(h: &mut Harness, path: &Path, size: egui::Vec2, language: &str) -> (image::RgbaImage, f32) {
    // A fresh context transfers every texture (fonts, icons, the view) to the software renderer.
    h.ctx = egui::Context::default();
    // The harness's widget lookups read the accessibility tree of its frames: on this context too.
    h.ctx.enable_accesskit();
    polykrom_ui_egui::fonts::setup_fonts_for(&h.ctx, language, "");
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..8 {
        raster::paint_sized(h, &mut textures, vec![], size);
    }
    let output = raster::paint_sized(h, &mut textures, vec![], size);
    let ppp = output.pixels_per_point;
    let shot = raster::raster_sized(h, &textures, output, size);
    save(&shot, path);
    (shot, ppp)
}

fn save(shot: &image::RgbaImage, path: &Path) {
    shot.save(path).unwrap();
    let bytes = std::fs::metadata(path).unwrap().len();
    assert!(bytes <= MAX_BYTES, "{}: {bytes} bytes", path.display());
}

/// The part of `shot` under `bounds` (points), grown by `margin` points, as its own picture.
fn crop(shot: &image::RgbaImage, bounds: Rect, margin: f32, ppp: f32, path: &Path) {
    let bounds =
        (bounds.expand(margin) * ppp).intersect(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(shot.width() as f32, shot.height() as f32)));
    if bounds.width() < 2.0 || bounds.height() < 2.0 {
        return;
    }
    let part = image::imageops::crop_imm(shot, bounds.left() as u32, bounds.top() as u32, bounds.width() as u32, bounds.height() as u32).to_image();
    save(&part, path);
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

/// How long a preview may take to compute before the test stops waiting for it: generous, for a
/// loaded CI runner, where a computation that takes a fraction of a second here has taken many.
const PREVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Frames until the proposal's preview has settled (it computes off the window thread): it is
/// active or its error is set, and nothing is computing. Frames are pumped until that holds or
/// `PREVIEW_TIMEOUT` of wall-clock time has passed: a fixed count of frames ran out on a slow
/// CI runner while the preview was still computing, and the pictures were painted without it
/// (PK-99). The caller asserts the state.
fn settle_preview(h: &mut Harness) {
    let deadline = std::time::Instant::now() + PREVIEW_TIMEOUT;
    loop {
        let settled = !h.app.preview.busy && (h.app.preview.active || h.app.preview.error.is_some());
        if settled || std::time::Instant::now() >= deadline {
            break;
        }
        h.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    h.frames(3);
}

/// The actuator mount open in the given theme, the mounting holes selected (the scope).
fn mount(dark: bool) -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.call("ui.set", json!({"dark": dark}));
    h.app.session.run_script(&actuator_mount()).unwrap();
    h.frames(3);
    h.call("ui.camera", json!({"fit": true}));
    let holes = h.app.session.doc.find_feature("Mounting holes").expect("the hole feature").id;
    h.app.session.selection = vec![polykrom_engine::Sel::Feature { id: holes }];
    h.frames(3);
    h
}

/// An agent set up (as a person who installed one has), `command` being what would run.
fn with_agent(h: &mut Harness, command: &str) {
    let r = h.call("ui.agent_chat", json!({"add_agent": {"id": "picture-agent", "name": "Local model", "kind": "acp", "command": command}}));
    assert_eq!(r["ok"], true, "{r}");
    h.frames(2);
}

#[test]
fn review_flow_pictures() {
    let stage = std::env::var("REVIEW_MEDIA_STAGE").unwrap_or_else(|_| "after".into());
    let after = stage != "before";
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let previous = theme::is_dark();
    let file = |surface: &str, mode: &str| dir.join(format!("{surface}-{mode}-{stage}.png"));
    let change = json!({"command": "timeline.edit", "params": {"feature": "Mounting holes", "set": {"diameter": "10 mm"}}});
    for dark in [true, false] {
        let mode = if dark { "dark" } else { "light" };

        // Blocked: the request typed, no agent set up; the reason stands in the row.
        let mut h = mount(dark);
        h.call("ui.review", json!({"prompt": REQUEST}));
        h.frames(3);
        if after {
            assert_eq!(h.call("ui.inspect", json!({}))["result"]["review"]["state"], "blocked");
            assert!(h.widget("No agent found: add one in the AI settings").is_some(), "the reason in the row");
        }
        let (shot, ppp) = paint(&mut h, &file("blocked", mode));
        if let Some(row) = scenario::rect_of("review:prompt_row") {
            crop(&shot, row, 4.0, ppp, &file("blocked-row", mode));
        }

        // The prompt with the request, an agent set up: Preview changes can go.
        with_agent(&mut h, "true");
        if after {
            assert_eq!(h.call("ui.inspect", json!({}))["result"]["review"]["state"], "idle");
        }
        let (shot, ppp) = paint(&mut h, &file("prompt", mode));
        if let Some(row) = scenario::rect_of("review:prompt_row") {
            crop(&shot, row, 4.0, ppp, &file("prompt-row", mode));
        }

        // Pending: the request sent, the agent (a process that only waits here) starting on it.
        with_agent(&mut h, "sleep 30");
        let r = h.call("ui.review", json!({"preview": true}));
        assert_eq!(r["ok"], true, "{r}");
        h.frames(3);
        // The picture is of the pending state, or there is no picture: the request must be with
        // the starting agent.
        if after {
            assert!(r["result"]["waiting"].is_string(), "{mode}: the request was not taken by the starting agent: {r}");
            assert_eq!(h.call("ui.inspect", json!({}))["result"]["review"]["state"], "pending", "{mode}");
            assert!(h.widget("Preview pending").is_some(), "the button says so");
        }
        let (shot, ppp) = paint(&mut h, &file("pending", mode));
        if let Some(row) = scenario::rect_of("review:prompt_row") {
            crop(&shot, row, 4.0, ppp, &file("pending-row", mode));
        }
        h.call("ui.agent_chat", json!({"stop": true}));
        h.frames(2);
        // The stop's status toast is not part of the next picture.
        h.app.status = None;

        // View 04: the proposal (the Ø10 holes) under review, the request still in the row.
        h.call("ui.review", json!({"prompt": REQUEST}));
        let r = h.call("ui.review", json!({"propose": {"commands": [change.clone()], "label": "Resize mounting holes"}}));
        assert_eq!(r["ok"], true, "{r}");
        if after {
            assert_eq!(r["result"]["state"], "ready");
            assert_eq!(r["result"]["open"]["changes"][0]["text"], "Ø8.00 mm → Ø10.00 mm", "{r}");
            assert_eq!(r["result"]["open"]["pattern_preserved"], true);
        }
        settle_preview(&mut h);
        // The pictures from here to Apply show the proposal previewed: never painted without it.
        assert!(h.app.preview.active, "{mode}: the proposed Ø10 holes are previewed: {:?}", h.app.preview.error);
        if after {
            assert!(h.widget("Ø8.00 mm → Ø10.00 mm").is_some() && h.widget("Preview ready").is_some(), "the card's value and the row's button");
        }
        let (shot, ppp) = paint(&mut h, &file("review-card", mode));
        assert!(scenario::rect_of("review:panel").is_some(), "the Review changes card");
        if let Some(card) = scenario::rect_of("review:panel") {
            crop(&shot, card, 4.0, ppp, &file("review-card-only", mode));
        }
        if let Some(row) = scenario::rect_of("review:prompt_row") {
            crop(&shot, row, 4.0, ppp, &file("review-card-row", mode));
        }
        // The same card in shorter windows: 1366 × 768 and 1280 × 720 in English, and 1366 × 768
        // in Japanese (dark) and Chinese (light). Every value whole; the card below the view cube;
        // its body scrolls while Apply & rebuild, Cancel command and the footer stay in view.
        let cjk = if dark { "ja" } else { "zh" };
        for (small, language) in [(egui::vec2(1366.0, 768.0), "en"), (egui::vec2(1280.0, 720.0), "en"), (egui::vec2(1366.0, 768.0), cjk)] {
            h.call("ui.resize", json!({"width": small.x, "height": small.y}));
            h.call("ui.prefs", json!({"language": language}));
            h.frames(3);
            let name = format!("review-card-{}-{language}", small.x as i32);
            let (shot, ppp) = paint_in(&mut h, &file(&name, mode), small, language);
            if after {
                let window = Rect::from_min_size(egui::Pos2::ZERO, small);
                let card = scenario::rect_of("review:panel").expect("the Review changes card");
                let cube = scenario::rect_of("axis_view:outer").expect("the view cube");
                assert!(card.top() >= cube.bottom(), "{mode} {name}: the card {card:?} covers the view cube {cube:?}");
                for handle in ["review:apply", "review:cancel", "dialog:note:review_footer"] {
                    let r = scenario::rect_of(handle).unwrap_or_else(|| panic!("{handle}"));
                    assert!(card.contains_rect(r) && window.contains_rect(r), "{mode} {name}: {handle} {r:?} is not in view in the card {card:?}");
                }
            }
            if let Some(card) = scenario::rect_of("review:panel") {
                crop(&shot, card, 4.0, ppp, &file(&format!("review-card-only-{}-{language}", small.x as i32), mode));
            }
        }
        h.call("ui.prefs", json!({"language": "en"}));
        h.call("ui.resize", json!({"width": WINDOW.x, "height": WINDOW.y}));
        h.frames(3);
        // The model before Apply: the Ø8 holes (the proposal is previewed, not applied).
        if dark {
            render_model(&h, &dir.join("review-model-before.png"));
        }

        // View 05: applied, with the receipt toast and its Undo, the status line and, on the
        // body's properties, the footer.
        let r = h.call("ui.review", json!({"apply": true}));
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["result"]["receipt"].is_object(), "{r}");
        if after {
            assert_eq!(r["result"]["receipt"]["text"], "4 mounting holes updated / Ø10.00 mm");
            assert_eq!(r["result"]["status"], "4 mounting holes updated / Ø10.00 mm / Pattern preserved");
        }
        h.frames(3);
        let (shot, ppp) = paint(&mut h, &file("applied-toast", mode));
        assert!(scenario::rect_of("toast:review").is_some(), "the receipt toast");
        if let Some(toast) = scenario::rect_of("toast:review") {
            crop(&shot, toast, 12.0, ppp, &file("applied-toast-only", mode));
        }
        h.app.session.selection = vec![polykrom_engine::Sel::Body { name: "Mount".into() }];
        h.frames(3);
        let (shot, ppp) = paint(&mut h, &file("applied-properties", mode));
        if let Some(card) = scenario::rect_of("inspector:card") {
            crop(&shot, card, 4.0, ppp, &file("applied-properties-only", mode));
        }
        // The model after Apply: the Ø10 holes, a different picture from before.
        if dark {
            render_model(&h, &dir.join("review-model-after.png"));
            let (before, after) =
                (std::fs::read(dir.join("review-model-before.png")).unwrap(), std::fs::read(dir.join("review-model-after.png")).unwrap());
            assert_ne!(before, after, "the model before and after Apply are the same picture");
        }
    }
    theme::set_dark(previous);
}

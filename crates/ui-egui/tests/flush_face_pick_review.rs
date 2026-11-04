//! Review pictures for face picks on a face flush with another body's (#297, PK-79): the Shell
//! dialog with a wall's bottom face picked where it stands on a base plate, and the Draft dialog
//! with the side of a block picked where another block touches it, painted by the real app
//! through the test software rasteriser (no window or GPU in CI). Written to
//! `target/review-media/<surface>-<stage>.png`, where the stage is `PICK_REVIEW_STAGE`
//! (default `after`); the same test on the previous commit gives `before`. Nothing here is
//! committed: the pictures are evidence for a review, and the test asserts nothing about the
//! outcome, so it runs on either commit.

#[path = "support/raster.rs"]
#[allow(dead_code)]
mod raster;

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{ColorImage, TextureId};
use polykrom_engine::{Sel, geom::Vec3};
use polykrom_ui_egui::{scenario::Harness, theme};
use serde_json::json;

const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);

/// The face of `body` whose centre is at `c`.
fn face(h: &Harness, body: &str, c: Vec3) -> Sel {
    let st = h.app.session.world_state();
    let b = st.body(body).unwrap();
    let f = b.body.faces(0.01).unwrap().into_iter().find(|f| f.centroid.dist(c) < 1e-6).unwrap();
    Sel::Face { body: body.into(), index: f.index, point: c }
}

/// A tray: a base plate with a wall standing on it, the wall's bottom flush with the plate's top.
fn tray(h: &mut Harness) {
    h.app.run("solid.box", json!({"body_name": "Base", "corner": [-32, -22, 0], "length": 64, "width": 44, "height": 4})).unwrap();
    h.app.run("solid.box", json!({"body_name": "LeftWall", "corner": [-32, -22, 4], "length": 3, "width": 44, "height": 19})).unwrap();
}

/// Two blocks side by side: A's +x face is flush with B's -x face.
fn pair(h: &mut Harness) {
    h.app.run("solid.box", json!({"body_name": "A", "length": 20, "width": 20, "height": 20})).unwrap();
    h.app.run("solid.box", json!({"body_name": "B", "corner": [20, 0, 0], "length": 20, "width": 20, "height": 20})).unwrap();
}

fn picture(h: &mut Harness, path: &PathBuf) {
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
}

/// Frames until the dialog's preview has settled (it may run off the UI thread).
fn settle(h: &mut Harness) {
    for _ in 0..60 {
        h.frames(1);
        if h.app.applying.is_none() && !h.app.preview.busy && (h.app.preview.error.is_some() || h.app.preview.active) {
            break;
        }
    }
    h.frames(3);
}

#[test]
fn shell_and_draft_dialogs_on_a_flush_face() {
    let stage = std::env::var("PICK_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media");
    std::fs::create_dir_all(&dir).unwrap();
    let mut notes = String::new();
    for (surface, command) in [("shell-dialog", "solid.shell"), ("draft-dialog", "solid.draft")] {
        let mut h = Harness::new();
        h.app.home.open = false;
        let picked = if command == "solid.shell" {
            tray(&mut h);
            // The wall's bottom face: inside it, the base plate's top face is equally near.
            face(&h, "LeftWall", Vec3::new(-30.5, 0.0, 4.0))
        } else {
            pair(&mut h);
            // A's +x face: inside it, B's -x face is equally near.
            face(&h, "A", Vec3::new(20.0, 10.0, 10.0))
        };
        h.frames(3);
        h.app.session.selection = vec![picked];
        h.app.start(command);
        // A 1 mm shell of the 3 mm wall (the dialog's default would leave no wall).
        if let Some(polykrom_ui_egui::dialogs::Kind::Shell { thickness }) = h.app.dialog.as_mut().map(|d| &mut d.kind) {
            *thickness = "1".into();
        }
        if command == "solid.draft"
            && let Some(d) = h.app.dialog.as_mut()
            && let Some(input) = d.inputs.get_mut(1)
        {
            input.items = vec![Sel::Plane { name: "XY".into() }];
        }
        settle(&mut h);
        let error = h.app.dialog.as_ref().and_then(|d| d.error.clone()).or_else(|| h.app.preview.error.clone());
        notes += &format!("{surface}\t{stage}\t{}\n", error.as_deref().unwrap_or("preview ok"));
        picture(&mut h, &dir.join(format!("{surface}-{stage}.png")));
    }
    std::fs::write(dir.join(format!("dialogs-{stage}.txt")), notes).unwrap();
}

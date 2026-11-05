//! Review picture of Help ▸ About with its Fonts section open, painted by the real app through
//! the test software rasteriser (no window or GPU in CI). Written to
//! `target/review-media/about-fonts-<stage>.png`, where the stage is `ABOUT_REVIEW_STAGE`
//! (default `after`); the same test on the previous commit gives `before`. Nothing here is
//! committed: the picture is evidence for a review of the About dialog's font notice.

#[path = "support/raster.rs"]
#[allow(dead_code)]
mod raster;

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{ColorImage, Event, PointerButton, TextureId};
use polykrom_ui_egui::{
    scenario::{self, Harness},
    theme,
};

const WINDOW: egui::Vec2 = egui::vec2(1200.0, 900.0);

#[test]
fn about_dialog_fonts_picture() {
    let stage = std::env::var("ABOUT_REVIEW_STAGE").unwrap_or_else(|_| "after".into());
    let dir = std::env::var("REVIEW_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/review-media"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.help.about = true;
    // A fresh context transfers every texture (fonts and icons) to the software renderer; the
    // section's open state lives in it, so the click goes through recorded frames too.
    h.ctx = egui::Context::default();
    h.ctx.enable_accesskit();
    polykrom_ui_egui::fonts::setup_fonts(&h.ctx);
    theme::apply(&h.ctx);
    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    for _ in 0..6 {
        raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
    }
    let at = scenario::handle_at("dialog:section:Fonts").expect("the About dialog shows its Fonts section");
    let press = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    raster::paint_sized(&mut h, &mut textures, vec![Event::PointerMoved(at), press(true)], WINDOW);
    raster::paint_sized(&mut h, &mut textures, vec![press(false)], WINDOW);
    for _ in 0..6 {
        raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
    }
    let output = raster::paint_sized(&mut h, &mut textures, vec![], WINDOW);
    // The section is open: the notice (it names the sketch font in every version) is on screen.
    let tree = output.platform_output.accesskit_update.as_ref().expect("accesskit is enabled");
    assert!(
        tree.nodes.iter().any(|(_, n)| n.label().or(n.value()).is_some_and(|t| t.contains("Liberation Sans"))),
        "the Fonts section's text is shown"
    );
    let ppp = output.pixels_per_point;
    let shot = raster::raster_sized(&mut h, &textures, output, WINDOW);
    // The dialog: from its header to its Close action, below the open Fonts section.
    let header = scenario::rect_of("dialog:header").expect("the About dialog's header");
    let bottom = scenario::rect_of("dialog:section:Fonts").map_or(WINDOW.y, |r| (r.bottom() + 280.0).min(WINDOW.y));
    let dialog = egui::Rect::from_min_max(egui::pos2(header.left() - 8.0, header.top() - 8.0), egui::pos2(header.right() + 8.0, bottom)) * ppp;
    let dialog = dialog.intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(shot.width() as f32, shot.height() as f32)));
    assert!(dialog.width() > 100.0 && dialog.height() > 100.0, "{dialog:?}");
    image::imageops::crop_imm(&shot, dialog.left() as u32, dialog.top() as u32, dialog.width() as u32, dialog.height() as u32)
        .to_image()
        .save(dir.join(format!("about-fonts-{stage}.png")))
        .unwrap();
}

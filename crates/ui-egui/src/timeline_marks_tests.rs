//! A failed feature is marked in the bottom timeline and the Feature History by a shape, not only
//! its colour (#972): the error mark where a warning's mark goes (now an icon image, so these
//! tests look for the 10-point image at the mark's place).

use egui::{Pos2, Rect, Shape, vec2};
use serde_json::json;

use crate::SolveApp;

use crate::test_support::each_shape as each;

/// The sample with Fillet1 failing (the plate made narrower than its fillets).
pub(crate) fn failing_sample() -> SolveApp {
    let mut s = polykrom_engine::Session::default();
    s.run_script(&polykrom_engine::sample::script()).unwrap();
    s.execute("parameters.change", &json!({"name": "width", "expression": "5 mm"})).unwrap();
    SolveApp::new(s, crate::Services::default())
}

/// Each feature's name and whether it failed.
fn features(app: &SolveApp) -> Vec<(String, bool)> {
    app.session.doc.features.iter().map(|f| (f.name.clone(), app.session.model.result(f.id).is_some_and(|r| r.error.is_some()))).collect()
}

/// The centres of the 10 to 12-point icon images one frame of `draw` paints: the failed-feature mark
/// (the sample has no warnings, so every such image is an error mark).
fn error_discs(app: &mut SolveApp, draw: fn(&mut SolveApp, &mut egui::Ui)) -> Vec<Pos2> {
    crate::scenario::clear_handles();
    let ctx = crate::fonts::test_context();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0));
    let out = crate::test_support::frame(&ctx, egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| draw(app, ui));
    let mut found = Vec::new();
    for clipped in &out.shapes {
        each(&clipped.shape, &mut |s| {
            if let Shape::Mesh(m) = s {
                let b = m.calc_bounds();
                if matches!(m.texture_id, egui::TextureId::Managed(_)) && (b.width() - 11.0).abs() < 1.5 && (b.height() - 11.0).abs() < 1.5 {
                    found.push(b.center());
                }
            }
        });
    }
    found
}

/// A drawn item: its name and its handle's centre.
struct Item {
    name: String,
    at: Pos2,
}

/// The features that have a disc where their mark goes (`at`: the mark's centre from the item).
fn marked(app: &mut SolveApp, draw: fn(&mut SolveApp, &mut egui::Ui), at: impl Fn(&Item, Pos2) -> bool) -> Vec<(String, bool)> {
    let discs = error_discs(app, draw);
    let handles = crate::scenario::handles("timeline:");
    features(app)
        .into_iter()
        .map(|(name, _)| {
            let item = handles.iter().find(|(h, _)| *h == format!("timeline:{name}")).map(|(_, p)| *p);
            let item = Item { at: item.unwrap_or_else(|| panic!("{name} is not drawn")), name: name.clone() };
            let has = discs.iter().any(|d| at(&item, *d));
            (name, has)
        })
        .collect()
}

#[test]
fn a_failed_feature_is_marked_in_the_bottom_timeline() {
    let mut app = failing_sample();
    let want = features(&app);
    assert!(want.iter().any(|(n, failed)| *failed && n == "Fillet1"), "the sample's Fillet1 fails: {want:?}");
    assert!(want.iter().filter(|(_, failed)| !failed).count() >= 3, "and others don't");
    // The mark sits in the step's top-right corner: 10 points square, 2 points in from the
    // step's edges (the step is as wide as its name, 48 points tall).
    // (The handle is the first step of that name, as `marked` finds it: the sample names a
    // sketch and the feature it drives alike, and the first is the sketch.)
    let got = marked(&mut app, crate::timeline::timeline, |item, d| {
        let name = format!("timeline:{}", item.name);
        let rect = crate::scenario::rects("timeline:").into_iter().find(|(n, _)| *n == name).map(|(_, r)| r).unwrap_or_else(|| panic!("{name} has no rect"));
        (d.x - (rect.right() - 7.0)).abs() < 1.5 && (d.y - (rect.top() + 7.0)).abs() < 1.5
    });
    assert_eq!(got, want, "an error mark on each failed feature and on no other");
}

#[test]
fn a_failed_feature_is_marked_in_the_feature_history() {
    let mut app = failing_sample();
    let want = features(&app);
    // The mark sits at the row's right end, on its centre line.
    let got = marked(&mut app, crate::timeline_left::left_timeline, |item, d| (d.y - item.at.y).abs() < 1.5 && d.x > item.at.x);
    assert_eq!(got, want, "an error mark on each failed feature and on no other");
}

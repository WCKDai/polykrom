use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn close(a: &Value, b: f64, tol: f64, what: &str) {
    let a = a.as_f64().unwrap_or(f64::NAN);
    assert!((a - b).abs() <= tol, "{what}: {a} against {b} (±{tol})");
}

/// A 30 x 10 bar on a 10 x 10 x 15 post: printed upright, the bar's two 10 x 10 undersides
/// overhang (200 mm²) and only the post's foot is on the bed (100 mm²).
fn tee(s: &mut Session) {
    run(s, "solid.box", json!({"length": 10, "width": 10, "height": 15, "corner": [0, 0, 0], "body_name": "Tee"}));
    run(s, "solid.box", json!({"length": 30, "width": 10, "height": 5, "corner": [-10, 0, 15], "operation": "join"}));
}

#[test]
fn a_box_on_its_face_needs_no_support() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    let r = run(&mut s, "inspect.printability", json!({}));
    let b = &r["bodies"][0];
    assert_eq!(b["body"], "Block", "{r}");
    close(&b["bed_contact_mm2"], 800.0, 1e-6, "bed contact");
    close(&b["overhang_mm2"], 0.0, 1e-9, "overhang");
    close(&b["volume_mm3"], 8000.0, 1e-6, "volume");
    assert_eq!(b["fits_bed"], true);
}

#[test]
fn a_tee_overhangs_upright_and_prints_clean_upside_down() {
    let mut s = Session::default();
    tee(&mut s);
    let r = run(&mut s, "inspect.printability", json!({"bodies": ["Tee"]}));
    let b = &r["bodies"][0];
    close(&b["bed_contact_mm2"], 100.0, 1e-6, "upright bed contact");
    close(&b["overhang_mm2"], 200.0, 1e-6, "upright overhang");
    close(&b["support_projected_mm2"], 200.0, 1e-6, "upright support footprint");
    // Of the six axis directions, upside down (the bar on the bed) needs no support.
    assert_eq!(b["best"]["direction"], json!([0.0, 0.0, -1.0]), "{b}");
    close(&b["best"]["overhang_mm2"], 0.0, 1e-9, "upside-down overhang");
    close(&b["best"]["bed_contact_mm2"], 300.0, 1e-6, "upside-down bed contact");
    assert_eq!(b["orientations"].as_array().map(Vec::len), Some(6));
    // Asked to print it that way, the same numbers.
    let r = run(&mut s, "inspect.printability", json!({"bodies": ["Tee"], "direction": [0, 0, -1]}));
    close(&r["bodies"][0]["overhang_mm2"], 0.0, 1e-9, "asked upside down");
}

/// A wedge's 45° face is at the limit and needs no support; at a 30° limit it does.
#[test]
fn the_overhang_limit_decides_a_sloped_face() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XZ"}));
    // A right triangle in XZ (the sketch's second axis is −z) with corners (0, 0), (20, 20) and
    // (0, 20) in x, z: its long side, from (0, 0) to (20, 20), faces down and out at 45°.
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [20, -20], [0, -20]], "closed": true}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 10}));
    let at = |s: &mut Session, limit: f64| run(s, "inspect.printability", json!({"overhang_deg": limit}))["bodies"][0].clone();
    let slope = 20.0 * std::f64::consts::SQRT_2 * 10.0;
    close(&at(&mut s, 45.0)["overhang_mm2"], 0.0, 1e-6, "at a 45° limit");
    close(&at(&mut s, 30.0)["overhang_mm2"], slope, 1e-6, "at a 30° limit");
}

#[test]
fn a_part_longer_than_the_bed_does_not_fit_any_way_round() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 300, "width": 10, "height": 10, "body_name": "Bar"}));
    let b = &run(&mut s, "inspect.printability", json!({"bed": [250, 250, 250]}))["bodies"][0];
    assert_eq!(b["fits_bed"], false, "{b}");
    assert!(b["orientations"].as_array().is_some_and(|o| o.iter().all(|o| o["fits_bed"] == false)), "{b}");
    // On a bigger bed it fits lying down.
    let b = &run(&mut s, "inspect.printability", json!({"bed": [350, 350, 350]}))["bodies"][0];
    assert_eq!(b["fits_bed"], true, "{b}");
}

/// PLA at 1.24 g/cm³: a solid 20 mm cube is 9.92 g. With 1.2 mm walls and 20 % infill it is
/// its surface times the wall thickness, and a fifth of the rest.
#[test]
fn filament_is_the_walls_and_the_infill() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 20, "width": 20, "height": 20, "body_name": "Cube"}));
    run(&mut s, "material.assign", json!({"bodies": ["Cube"], "material": "PLA"}));
    let b = &run(&mut s, "inspect.printability", json!({"infill": 0.2}))["bodies"][0];
    let walls = 2400.0 * 1.2;
    let grams = 1.24e-3 * (walls + 0.2 * (8000.0 - walls));
    close(&b["filament_g"], grams, 1e-6, "filament");
    let solid = &run(&mut s, "inspect.printability", json!({"infill": 1.0}))["bodies"][0];
    close(&solid["filament_g"], 9.92, 1e-6, "solid");
    assert!(s.execute("inspect.printability", &json!({"infill": 2})).is_err());
    assert!(s.execute("inspect.printability", &json!({"direction": [0, 0, 0]})).is_err());
}

//! `inspect.measure` keeps what it measured: a body that did not change or move is not measured
//! again, and one that did is.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn body<'a>(m: &'a Value, name: &str) -> &'a Value {
    m["bodies"].as_array().and_then(|b| b.iter().find(|b| b["name"] == name)).unwrap_or(&Value::Null)
}

#[test]
fn measurements_are_kept_until_the_body_changes_or_moves() {
    let mut s = Session::default();
    // Free, so it can be moved (a design's first part is grounded by default).
    let c = run(&mut s, "component.create", json!({"name": "Block", "grounded": false}));
    run(&mut s, "solid.box", json!({"length": 20, "width": 10, "height": 5, "corner": [0, 0, 0], "body_name": "B"}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    let first = run(&mut s, "inspect.measure", json!({}));
    assert_eq!(s.measure_cache.lock().map(|c| c.0.len()).unwrap_or(0), 1);
    let again = run(&mut s, "inspect.measure", json!({}));
    assert_eq!(first, again, "the same answer from the cache");
    // Moved: its box moves with it.
    let occ = c["occurrence"].as_u64().unwrap_or(0);
    run(&mut s, "occurrence.move", json!({"occurrence": occ, "translate": [100, 0, 0]}));
    let moved = run(&mut s, "inspect.measure", json!({}));
    assert!((body(&moved, "B")["bbox"]["min"][0].as_f64().unwrap_or(0.0) - 100.0).abs() < 1e-6, "{moved}");
    assert_eq!(body(&moved, "B")["volume_mm3"], body(&first, "B")["volume_mm3"]);
    // Changed: measured again.
    run(&mut s, "component.activate", json!({"component": c["component"]}));
    run(&mut s, "solid.box", json!({"length": 20, "width": 10, "height": 5, "corner": [0, 0, 5], "operation": "join"}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    let grown = run(&mut s, "inspect.measure", json!({}));
    assert!((body(&grown, "B")["volume_mm3"].as_f64().unwrap_or(0.0) - 2000.0).abs() < 1e-6, "{grown}");
}

/// An open sheet encloses no volume: it reports its area and `open`, without the meaningless
/// (often negative) volume, centre of mass and mass a closed body has (#195).
#[test]
fn an_open_sheet_reports_its_area_and_no_volume() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "S"}));
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [10, 0]], "ids": ["l"]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "surface.extrude", json!({"sketch": "S", "curves": ["l"], "distance": 5, "body_name": "Sheet"}));
    run(&mut s, "solid.box", json!({"length": 2, "width": 2, "height": 2, "corner": [20, 0, 0], "body_name": "Box"}));
    let m = run(&mut s, "inspect.measure", json!({}));
    let sheet = body(&m, "Sheet");
    assert_eq!(sheet["open"], json!(true), "{m}");
    assert!((sheet["area_mm2"].as_f64().unwrap_or(0.0) - 50.0).abs() < 1e-9, "{m}");
    for key in ["volume_mm3", "center_of_mass", "mass_g"] {
        assert!(sheet.get(key).is_none(), "{key} in {sheet}");
    }
    // Its material's density stays: it doesn't need a volume.
    assert!(sheet["density_g_cm3"].as_f64().is_some_and(|d| d > 0.0), "{sheet}");
    // The closed box keeps its volume; the totals count only it.
    assert!(body(&m, "Box").get("open").is_none());
    assert!((m["total"]["volume_mm3"].as_f64().unwrap_or(0.0) - 8.0).abs() < 1e-9, "{m}");
    assert!((m["total"]["area_mm2"].as_f64().unwrap_or(0.0) - 74.0).abs() < 1e-9, "{m}");
}

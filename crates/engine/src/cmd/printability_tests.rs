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

// inspect.polish (#193): the finish report and its fixes.

fn polish_body<'a>(r: &'a Value, name: &str) -> &'a Value {
    r["bodies"].as_array().and_then(|b| b.iter().find(|b| b["body"] == name)).unwrap_or_else(|| panic!("no {name} in {r}"))
}

#[test]
fn a_raw_box_has_twelve_sharp_edges_and_a_fillet_fix() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    let r = run(&mut s, "inspect.polish", json!({}));
    let b = polish_body(&r, "Block");
    assert_eq!(b["sharp_edges"], 12, "{b}");
    close(&b["sharp_edge_mm"], 280.0, 1e-6, "sharp length");
    close(&b["boxiness"], 1.0, 1e-9, "boxiness");
    assert!(b["score"].as_f64().unwrap() < 40.0, "{b}");
    close(&b["scores"]["look"], 15.0, 1e-9, "look: 100 − 60 (all sharp) − 25 (all boxy)");
    assert!(r["score"].as_f64().unwrap() < 40.0, "{r}");
    let fix = &r["fixes"][0];
    assert_eq!(fix["command"], "solid.fillet", "{r}");
    assert_eq!(fix["category"], "look");
    assert_eq!(fix["safe"], true);
    let edges: usize = r["fixes"].as_array().unwrap().iter().map(|f| f["params"]["edges"].as_array().unwrap().len()).sum();
    assert_eq!(edges, 12, "every sharp edge has a fix: {r}");
    // 0.15 × the narrowest face's width (the 20 x 10 end: 2·200/60), rounded down to the family.
    close(&b["edges"][0]["radius"], 1.0, 1e-9, "suggested radius");
}

#[test]
fn a_filleted_box_scores_high_with_no_sharp_edges() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    let all = run(&mut s, "inspect.polish", json!({}))["fixes"][0]["params"]["edges"].clone();
    let mut edges = all.as_array().unwrap().clone();
    for f in run(&mut s, "inspect.polish", json!({}))["fixes"].as_array().unwrap().iter().skip(1) {
        edges.extend(f["params"]["edges"].as_array().unwrap().iter().cloned());
    }
    run(&mut s, "solid.fillet", json!({"edges": edges, "radius": 2}));
    let r = run(&mut s, "inspect.polish", json!({}));
    let b = polish_body(&r, "Block");
    assert_eq!(b["sharp_edges"], 0, "{b}");
    assert_eq!(r["fixes"].as_array().map(Vec::len), Some(0), "{r}");
    assert_eq!(b["radii"][0]["radius"], 2.0, "the fillets are one family: {b}");
    close(&b["boxiness"], 0.0, 1e-9, "boxiness");
    assert!(b["score"].as_f64().unwrap() > 90.0, "{b}");
}

#[test]
fn a_box_seated_on_another_leaves_out_its_seat_edges() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 10, "body_name": "Base"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [15, 15, 10], "operation": "new_body", "body_name": "Cap"}));
    let r = run(&mut s, "inspect.polish", json!({}));
    assert_eq!(polish_body(&r, "Cap")["sharp_edges"], 8, "the four edges on the base are a seat: {r}");
    assert_eq!(polish_body(&r, "Base")["sharp_edges"], 12, "{r}");
    for f in r["fixes"].as_array().unwrap() {
        for p in f["params"]["edges"].as_array().unwrap() {
            assert!(!(f["body"] == "Cap" && p[2].as_f64() == Some(10.0)), "no fix on the seat: {f}");
        }
    }
}

#[test]
fn running_the_polish_fix_raises_the_score() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    let before = run(&mut s, "inspect.polish", json!({}));
    let fix = &before["fixes"][0];
    run(&mut s, fix["command"].as_str().unwrap(), fix["params"].clone());
    let after = run(&mut s, "inspect.polish", json!({}));
    assert!(after["score"].as_f64().unwrap() > before["score"].as_f64().unwrap() + 5.0, "{before}\n{after}");
    assert!(polish_body(&after, "Block")["sharp_edges"].as_u64().unwrap() < 12);
}

#[test]
fn a_part_placed_twice_is_fixed_once_where_it_is_shown() {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "Leg"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 30, "corner": [0, 0, 0]}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "occurrence.copy", json!({"component": "Leg", "translate": [50, 0, 0]}));
    let r = run(&mut s, "inspect.polish", json!({}));
    assert_eq!(r["bodies"].as_array().map(Vec::len), Some(2), "{r}");
    let edges: usize = r["fixes"].as_array().unwrap().iter().map(|f| f["params"]["edges"].as_array().unwrap().len()).sum();
    assert_eq!(edges, 12, "one part's edges, not both placements': {r}");
    for f in r["fixes"].as_array().unwrap().clone() {
        run(&mut s, f["command"].as_str().unwrap(), f["params"].clone());
    }
    let after = run(&mut s, "inspect.polish", json!({}));
    for b in after["bodies"].as_array().unwrap() {
        assert_eq!(b["sharp_edges"], 0, "{after}");
    }
}

#[test]
fn a_printed_box_gets_a_bed_chamfer_and_print_scores() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    run(&mut s, "material.assign", json!({"bodies": ["Block"], "material": "PLA"}));
    let r = run(&mut s, "inspect.polish", json!({}));
    let b = polish_body(&r, "Block");
    assert_eq!(b["printed"], true);
    assert_eq!(b["print"]["first_layer_edges"], 4, "{b}");
    assert_eq!(b["sharp_edges"], 8, "the bed's edges are chamfered, not rounded: {b}");
    close(&b["print"]["overhang_mm2"], 0.0, 1e-9, "overhang");
    assert!(r["fixes"].as_array().unwrap().iter().any(|f| f["command"] == "solid.chamfer" && f["category"] == "print"), "{r}");
    for k in ["look", "print", "strength", "mass", "tidy"] {
        assert!(b["scores"][k].is_number(), "{k}: {b}");
    }
    // A 3 mm hole is tight for an M3 screw; a 3.4 mm one isn't.
    run(&mut s, "solid.cylinder", json!({"diameter": 3, "height": 12, "base": [10, 10, -1], "operation": "cut", "targets": ["Block"]}));
    run(&mut s, "solid.cylinder", json!({"diameter": 3.4, "height": 12, "base": [30, 10, -1], "operation": "cut", "targets": ["Block"]}));
    let r = run(&mut s, "inspect.polish", json!({}));
    let holes = &polish_body(&r, "Block")["print"]["tight_holes"];
    assert_eq!(holes, &json!([{"diameter": 3.0, "screw": 3.0, "holes": 1}]), "{r}");
    assert!(s.execute("inspect.polish", &json!({"bodies": ["Nope"]})).is_err());
    assert!(s.execute("inspect.polish", &json!({"max_fixes": 0})).is_err());
}

// polish.apply (#193): the safe fixes, each checked and rolled back when it makes things worse.

#[test]
fn polish_apply_rounds_a_raw_box_in_one_undo_step() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    let features = s.doc.features.len();
    let r = run(&mut s, "polish.apply", json!({}));
    let (before, after) = (r["before"]["score"].as_f64().unwrap(), r["after"]["score"].as_f64().unwrap());
    assert!(after > before + 20.0, "{r}");
    assert!(!r["applied"].as_array().unwrap().is_empty(), "{r}");
    let report = run(&mut s, "inspect.polish", json!({}));
    assert_eq!(polish_body(&report, "Block")["sharp_edges"], 0, "{report}");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc.features.len(), features, "one undo takes every fix back");
}

#[test]
fn polish_apply_rolls_back_a_fix_that_fails_its_checks() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    let features = s.doc.features.len();
    // No change of mass allowed: every fillet removes some, so each is rolled back.
    let r = run(&mut s, "polish.apply", json!({"mass_tol": 0}));
    assert_eq!(r["applied"].as_array().map(Vec::len), Some(0), "{r}");
    let skipped = r["skipped"].as_array().unwrap();
    assert!(!skipped.is_empty() && skipped.iter().all(|k| k["reason"].as_str().unwrap().contains("mass")), "{r}");
    assert_eq!(s.doc.features.len(), features, "{r}");
    assert_eq!(r["after"]["score"], r["before"]["score"]);
    // Only the fixes asked for.
    let r = run(&mut s, "polish.apply", json!({"fixes": ["F1"]}));
    assert_eq!(r["applied"].as_array().map(Vec::len), Some(1), "{r}");
    assert!(s.execute("polish.apply", &json!({"fixes": ["F99"]})).is_err());
    assert!(s.execute("polish.apply", &json!({"mass_tol": 2})).is_err());
}

#[test]
fn polish_apply_rounds_a_part_placed_twice_where_it_is_placed() {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "Leg"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 30, "corner": [0, 0, 0]}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "occurrence.copy", json!({"component": "Leg", "translate": [50, 0, 0]}));
    run(&mut s, "occurrence.copy", json!({"component": "Leg", "translate": [0, 40, 0]}));
    let r = run(&mut s, "polish.apply", json!({}));
    assert_eq!(r["skipped"], json!([]), "{r}");
    let after = run(&mut s, "inspect.polish", json!({}));
    assert_eq!(after["bodies"].as_array().map(Vec::len), Some(3), "{after}");
    for b in after["bodies"].as_array().unwrap() {
        assert_eq!(b["sharp_edges"], 0, "{after}");
    }
}

#[test]
fn polish_apply_keeps_a_full_undo_history_but_its_oldest_step() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    run(&mut s, "solid.box", json!({"length": 4, "width": 4, "height": 4, "corner": [60, 0, 0], "operation": "new_body", "body_name": "Cube"}));
    for i in 0..200 {
        run(&mut s, "parameters.add", json!({"name": format!("p{i}"), "expression": "1 mm"}));
    }
    let before: Vec<std::sync::Arc<solidcraft_doc::Document>> = s.undo.iter().map(|u| u.doc.clone()).collect();
    let r = run(&mut s, "polish.apply", json!({}));
    assert!(r["applied"].as_array().is_some_and(|a| a.len() >= 2), "{r}");
    assert_eq!(s.undo.len(), before.len());
    // One step in (the run), one out (the oldest): the rest are the same steps.
    for (now, was) in s.undo.iter().zip(before.iter().skip(1)) {
        assert!(std::sync::Arc::ptr_eq(&now.doc, was), "an older undo step was lost");
    }
    let last = s.undo.last().map(|u| u.label.clone());
    assert_eq!(last.as_deref(), Some("Apply Polish"));
}

#[test]
fn polish_apply_says_which_findings_another_run_would_fix() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 20, "height": 10, "body_name": "Block"}));
    run(&mut s, "solid.box", json!({"length": 4, "width": 4, "height": 4, "corner": [60, 0, 0], "operation": "new_body", "body_name": "Cube"}));
    let r = run(&mut s, "polish.apply", json!({"max_fixes": 1}));
    assert_eq!(r["applied"].as_array().map(Vec::len), Some(1), "{r}");
    assert_eq!(r["fixes_left"], 1, "{r}");
    let cube = r["recommendations"].as_array().unwrap().iter().find(|f| f["body"] == "Cube" && f["kind"] == "sharp_edges").cloned();
    assert_eq!(cube.map(|f| f["fix"].clone()), Some(json!(["F1"])), "{r}");
}

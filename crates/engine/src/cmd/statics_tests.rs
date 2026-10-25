//! inspect.statics (#164) against closed-form statics: m·g·r about a joint's axis, m·g along a
//! slider, a centre of mass's distance to the edges of the feet's polygon. Every body is a box,
//! so masses and centres are exact; tolerances are 1e-9 N·m / N / mm, or rounding of a hand sum.

use serde_json::{Value, json};
use solidcraft_geom::Vec2;

use crate::Session;

const G: f64 = 9.81;

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

/// A component `name` holding one box (`size` from `corner`, mm) in `material`; its mass in kg,
/// density × volume.
fn part(s: &mut Session, name: &str, size: [f64; 3], corner: [f64; 3], material: &str) -> f64 {
    run(s, "component.activate", json!({"component": "root"}));
    run(s, "component.create", json!({"name": name}));
    run(s, "solid.box", json!({"length": size[0], "width": size[1], "height": size[2], "corner": corner, "body_name": name}));
    run(s, "material.assign", json!({"bodies": [name], "material": material}));
    size[0] * size[1] * size[2] * s.doc.density(name) * 1e-6
}

/// A 200 x 20 x 10 aluminium `Arm` out along +x from the shoulder at (0, 0, 100), held
/// horizontal on the revolute `Shoulder` about +y from `base` (grounded). Its mass (kg).
fn arm(s: &mut Session, base: Value) -> f64 {
    let m = part(s, "Arm", [200.0, 20.0, 10.0], [0.0, -10.0, 95.0], "Aluminum");
    run(s, "occurrence.ground", json!({"occurrence": base, "grounded": true}));
    run(
        s,
        "joint.create",
        json!({"type": "revolute", "name": "Shoulder", "flip": true,
               "a": {"occurrence": base, "point": [0, 0, 100], "z": [0, 1, 0]}, "b": {"occurrence": "Arm", "point": [0, 0, 100], "z": [0, 1, 0]}}),
    );
    m
}

/// A 20 mm steel cube rigid on the arm's tip, its centre 190 mm out from the shoulder. Its mass.
fn tip(s: &mut Session) -> f64 {
    let m = part(s, "Tip", [20.0; 3], [180.0, -10.0, 105.0], "Steel");
    run(s, "joint.as_built", json!({"type": "rigid", "a": "Arm", "b": "Tip"}));
    m
}

fn shoulder(r: &Value) -> Value {
    r["joints"].as_array().and_then(|j| j.iter().find(|j| j["name"] == "Shoulder")).cloned().unwrap_or_else(|| panic!("no Shoulder in {r}"))
}

/// Held horizontal, the arm's weight acts at L/2 = 100 mm from the shoulder: m·g·L/2, positive
/// about +y (it pulls the arm down, the way the joint turns +x toward −z). A tip mass adds its
/// m·g·190 mm; the mass and centre of mass are the boxes'.
#[test]
fn a_horizontal_arm_loads_its_shoulder_with_its_weight_times_half_its_length() {
    let mut s = Session::default();
    let base = part(&mut s, "Base", [20.0, 20.0, 100.0], [-20.0, -10.0, 0.0], "Aluminum");
    let m = arm(&mut s, json!("Base"));
    close(&json!(m), 200.0 * 20.0 * 10.0 * 2.7e-6, 1e-12, "arm mass, 2.7 g/cm³");
    let r = run(&mut s, "inspect.statics", json!({}));
    let j = shoulder(&r);
    close(&j["load_nm"], m * G * 0.1, 1e-9, "shoulder torque");
    assert_eq!(j["kind"], "revolute", "{j}");
    assert!(j["rated_nm"].is_null() && j["utilisation"].is_null() && j["over"] == false && j.get("load_n").is_none(), "{j}");
    assert!(r["held_by"].as_array().is_some_and(|h| h.len() == 1 && h[0].as_str().is_some_and(|n| n.starts_with("Base"))), "{r}");
    close(&r["mass_kg"], base + m, 1e-12, "mass");
    close(&r["com_mm"][0], (base * -10.0 + m * 100.0) / (base + m), 1e-9, "com x");
    close(&r["com_mm"][2], (base * 50.0 + m * 100.0) / (base + m), 1e-9, "com z");

    let t = tip(&mut s);
    close(&json!(t), 8000.0 * 7.85e-6, 1e-12, "tip mass, 7.85 g/cm³");
    let r = run(&mut s, "inspect.statics", json!({"gravity": G}));
    close(&shoulder(&r)["load_nm"], m * G * 0.1 + t * G * 0.19, 1e-9, "arm and tip");
    close(&r["mass_kg"], base + m + t, 1e-12, "mass with the tip");
    // On the Moon the same arm weighs a sixth as much.
    let r = run(&mut s, "inspect.statics", json!({"gravity": 1.62}));
    close(&shoulder(&r)["load_nm"], (m * 0.1 + t * 0.19) * 1.62, 1e-9, "lunar");
    for bad in [json!({"gravity": -1}), json!({"contact_tol": "x"}), json!({"support": []}), json!({"support": ["Nothing"]}), json!({"margin": 2})] {
        assert!(s.execute("inspect.statics", &bad).is_err(), "{bad}");
    }
}

/// A slider holding a block up: the load is the block's weight, m·g, along the axis (−z).
#[test]
fn a_vertical_slider_carries_the_blocks_weight() {
    let mut s = Session::default();
    part(&mut s, "Post", [20.0, 20.0, 100.0], [-10.0, -10.0, 0.0], "Aluminum");
    let m = part(&mut s, "Block", [20.0; 3], [-10.0, -10.0, 100.0], "Steel");
    run(&mut s, "occurrence.ground", json!({"occurrence": "Post", "grounded": true}));
    run(
        &mut s,
        "joint.create",
        json!({"type": "slider", "name": "Lift", "flip": true,
               "a": {"occurrence": "Post", "point": [0, 0, 100], "z": [0, 0, 1]}, "b": {"occurrence": "Block", "point": [0, 0, 100], "z": [0, 0, 1]}}),
    );
    let r = run(&mut s, "inspect.statics", json!({}));
    let j = &r["joints"][0];
    assert_eq!((&j["name"], &j["kind"]), (&json!("Lift"), &json!("slider")), "{r}");
    close(&j["load_n"], -m * G, 1e-9, "slider force along +z");
    assert!(j.get("load_nm").is_none() && j["utilisation"].is_null(), "{j}");
}

/// The table's boxes, one material: a 400 x 200 x 20 top at z 300 on 20 x 20 x 300 legs at the
/// corners in `legs` (as [x, y] of each leg's corner), each a component `Leg1`… Returns (volume,
/// centre) of each box.
fn table(s: &mut Session, legs: &[[f64; 2]]) -> Vec<(f64, [f64; 2])> {
    part(s, "Top", [400.0, 200.0, 20.0], [0.0, 0.0, 300.0], "Aluminum");
    let mut boxes = vec![(1.6e6, [200.0, 100.0])];
    for (i, [x, y]) in legs.iter().enumerate() {
        part(s, &format!("Leg{}", i + 1), [20.0, 20.0, 300.0], [*x, *y, 0.0], "Aluminum");
        boxes.push((1.2e5, [x + 10.0, y + 10.0]));
    }
    boxes
}

/// The centre of mass (x, y) of boxes of one material.
fn com(boxes: &[(f64, [f64; 2])]) -> [f64; 2] {
    let v: f64 = boxes.iter().map(|b| b.0).sum();
    [boxes.iter().map(|b| b.0 * b.1[0]).sum::<f64>() / v, boxes.iter().map(|b| b.0 * b.1[1]).sum::<f64>() / v]
}

/// The signed distance from `c` to the line through `a` and `b`, positive on the origin's side.
fn to_line(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let side = |p: [f64; 2]| (dx * (p[1] - a[1]) - dy * (p[0] - a[0])) / dx.hypot(dy);
    side(c) * side([0.0, 0.0]).signum()
}

/// Four legs: the feet span the 400 x 200 rectangle and the centre of mass is at its middle,
/// 100 mm from the long edges; naming the legs as the support gives the same.
#[test]
fn a_four_legged_table_stands_with_its_centre_over_the_middle() {
    let mut s = Session::default();
    let boxes = table(&mut s, &[[0.0, 0.0], [380.0, 0.0], [0.0, 180.0], [380.0, 180.0]]);
    assert_eq!(com(&boxes), [200.0, 100.0]);
    for p in [json!({}), json!({"support": ["Leg1", "Leg2", "Leg3", "Leg4"]}), json!({"ground": 0})] {
        let r = run(&mut s, "inspect.statics", p.clone());
        let sup = &r["support"];
        assert_eq!(sup["inside"], true, "{p}: {r}");
        close(&sup["margin_mm"], 100.0, 1e-9, "margin");
        close(&sup["ground_mm"], 0.0, 1e-12, "ground");
        let mut poly: Vec<[i64; 2]> = sup["polygon"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|q| [q[0].as_f64().unwrap_or(f64::NAN) as i64, q[1].as_f64().unwrap_or(f64::NAN) as i64])
            .collect();
        poly.sort();
        assert_eq!(poly, [[0, 0], [0, 200], [400, 0], [400, 200]], "{p}: {r}");
        assert!(r["joints"].as_array().is_some_and(Vec::is_empty), "{r}");
    }
    // Lifted off the ground: nothing touches a floor 1 mm down.
    let r = run(&mut s, "inspect.statics", json!({"ground": -1}));
    assert!(r["support"]["points"] == 0 && r["support"]["margin_mm"].is_null() && r["support"]["inside"] == false, "{r}");
}

/// Three legs: the feet span a triangle cut by the edge from (400, 20) to (20, 200), and the
/// centre of mass is inside it by its distance to that edge. A heavy block over the missing
/// corner moves it out past that edge: a negative margin, the same distance.
#[test]
fn a_three_legged_table_tips_when_its_weight_is_over_the_missing_leg() {
    let mut s = Session::default();
    let mut boxes = table(&mut s, &[[0.0, 0.0], [380.0, 0.0], [0.0, 180.0]]);
    let edge = ([400.0, 20.0], [20.0, 200.0]);
    let r = run(&mut s, "inspect.statics", json!({}));
    let want = to_line(edge.0, edge.1, com(&boxes));
    assert!(want > 20.0 && want < 30.0, "by hand: 23.3 mm, {want}");
    assert_eq!(r["support"]["inside"], true, "{r}");
    close(&r["support"]["margin_mm"], want, 1e-9, "inside");
    assert_eq!(r["support"]["polygon"].as_array().map(Vec::len), Some(5), "{r}");

    part(&mut s, "Weight", [100.0; 3], [300.0, 100.0, 320.0], "Aluminum");
    boxes.push((1e6, [350.0, 150.0]));
    let c = com(&boxes);
    let want = to_line(edge.0, edge.1, c);
    assert!(want < -15.0 && want > -20.0, "by hand: −17.0 mm, {want}");
    let r = run(&mut s, "inspect.statics", json!({}));
    assert_eq!(r["support"]["inside"], false, "{r}");
    // The nearest point of the polygon is on that edge (between its ends), so the distance is to its line.
    close(&r["support"]["margin_mm"], want, 1e-9, "outside");
    close(&r["com_mm"][0], c[0], 1e-9, "com x");
    close(&r["com_mm"][1], c[1], 1e-9, "com y");
}

/// A point or a line holds nothing up: never inside, margin ≤ 0 (the distance to it).
#[test]
fn feet_in_a_line_have_no_inside() {
    let line = super::hull(vec![Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), Vec2::new(10.0, 0.0)]);
    assert_eq!(line.len(), 2, "{line:?}");
    assert_eq!(super::margin(&line, Vec2::new(5.0, 0.0)), (false, Some(0.0)));
    assert_eq!(super::margin(&line, Vec2::new(5.0, 3.0)), (false, Some(-3.0)));
    assert_eq!(super::margin(&[Vec2::new(1.0, 1.0)], Vec2::new(4.0, 5.0)), (false, Some(-5.0)));
    assert_eq!(super::margin(&[], Vec2::ZERO), (false, None));
}

/// A stand on two legs, nothing grounded, held up by its feet: `LegA` rigid under one end of a
/// 200 x 20 x 10 top, `LegB` on the revolute `Hip` about +y at (190, 0, 100), 5 mm in from its
/// centre line. The feet push W/2 each (the centre of mass is midway); about the hip, LegB's weight
/// W/4 and its foot's push W/2 both act 5 mm out: the load is 5 mm × (W/4 − W/2) = −1.25e-3 m × W.
#[test]
fn a_leg_with_feet_on_both_sides_carries_its_feet_push() {
    let mut s = Session::default();
    part(&mut s, "Top", [200.0, 20.0, 10.0], [0.0, -10.0, 100.0], "Aluminum");
    part(&mut s, "LegA", [10.0, 20.0, 100.0], [0.0, -10.0, 0.0], "Aluminum");
    part(&mut s, "LegB", [10.0, 20.0, 100.0], [190.0, -10.0, 0.0], "Aluminum");
    run(&mut s, "joint.as_built", json!({"type": "rigid", "a": "Top", "b": "LegA"}));
    run(
        &mut s,
        "joint.create",
        json!({"type": "revolute", "name": "Hip", "flip": true,
               "a": {"occurrence": "Top", "point": [190, 0, 100], "z": [0, 1, 0]}, "b": {"occurrence": "LegB", "point": [190, 0, 100], "z": [0, 1, 0]}}),
    );
    let r = run(&mut s, "inspect.statics", json!({"support": ["LegA", "LegB"]}));
    let w = 80_000.0 * 2.7e-6 * G;
    close(&r["mass_kg"], w / G, 1e-12, "mass");
    close(&r["support"]["margin_mm"], 10.0, 1e-9, "midway between the feet, 10 mm from the long edges");
    let hip = &r["joints"][0];
    assert!(hip.get("indeterminate").is_none(), "{r}");
    close(&hip["load_nm"], -1.25e-3 * w, 1e-9, "hip");
    // Held by the grounded top instead, the hip carries only LegB's weight, 5 mm out.
    run(&mut s, "occurrence.ground", json!({"occurrence": "Top", "grounded": true}));
    let r = run(&mut s, "inspect.statics", json!({}));
    close(&r["joints"][0]["load_nm"], 5e-3 * w / 4.0, 1e-9, "hip, top grounded");
    // Grounded and standing on LegB too: the load can't be split by statics.
    run(&mut s, "occurrence.ground", json!({"occurrence": "LegB", "grounded": true}));
    let r = run(&mut s, "inspect.statics", json!({}));
    assert!(r["joints"][0]["indeterminate"] == true && r["joints"][0]["load_nm"].is_null(), "{r}");
}

/// A library stepper at the shoulder: its holding torque is the rating, the utilisation is the
/// load over it, and the tip mass takes it over.
#[test]
fn a_stepper_at_the_shoulder_rates_it() {
    let mut s = Session::default();
    let r = run(&mut s, "parts.insert", json!({"family": "stepper", "size": "NEMA14-28", "point": [-100, 0, 0]}));
    let comp = r["component"].as_u64().unwrap_or(0);
    let occ = s.doc.occurrence_of(comp).map(|o| o.id).unwrap_or(0);
    let m = arm(&mut s, json!(occ));
    let r = run(&mut s, "inspect.statics", json!({}));
    let j = shoulder(&r);
    close(&j["rated_nm"], 0.13, 1e-12, "NEMA14-28 holding torque");
    close(&j["utilisation"], m * G * 0.1 / 0.13, 1e-9, "utilisation");
    assert!(j["over"] == false && j["above_margin"] == true, "0.81 of its rating: {j}");
    assert_eq!(shoulder(&run(&mut s, "inspect.statics", json!({"margin": 0.9})))["above_margin"], false);
    let t = tip(&mut s);
    let j = shoulder(&run(&mut s, "inspect.statics", json!({})));
    close(&j["utilisation"], (m * 0.1 + t * 0.19) * G / 0.13, 1e-9, "with the tip");
    assert_eq!(j["over"], true, "{j}");
}

/// Raised from horizontal to vertical over a study: the worst step is the first, horizontal, at
/// m·g·L/2; upright, the arm's weight is over the shoulder and the load is 0. The design is not
/// moved.
#[test]
fn over_a_study_the_horizontal_arm_is_the_worst_step() {
    let mut s = Session::default();
    part(&mut s, "Base", [20.0, 20.0, 100.0], [-20.0, -10.0, 0.0], "Aluminum");
    let m = arm(&mut s, json!("Base"));
    run(&mut s, "motion.study", json!({"name": "Raise", "steps": 10, "keys": [{"joint": "Shoulder", "points": [[0, 0], [10, -90]]}]}));
    let r = run(&mut s, "inspect.statics", json!({"study": "Raise"}));
    let j = shoulder(&r);
    assert_eq!(j["worst_step"], 0.0, "{r}");
    close(&j["load_nm"], m * G * 0.1, 1e-9, "worst");
    assert_eq!(r["margin_mm"].as_array().map(Vec::len), Some(11), "{r}");
    assert!(r["min_margin_mm"].is_number() && r["stopped_at"].is_null(), "{r}");
    close(&shoulder(&run(&mut s, "inspect.statics", json!({})))["load_nm"], m * G * 0.1, 1e-9, "unchanged design");
    run(&mut s, "motion.play", json!({"study": "Raise", "step": 5}));
    close(&shoulder(&run(&mut s, "inspect.statics", json!({})))["load_nm"], m * G * 0.1 * 45f64.to_radians().cos(), 1e-9, "45°");
    run(&mut s, "motion.play", json!({"study": "Raise", "step": 10}));
    close(&shoulder(&run(&mut s, "inspect.statics", json!({})))["load_nm"], 0.0, 1e-9, "upright");
    assert!(s.execute("inspect.statics", &json!({"study": "Nope"})).is_err());
}

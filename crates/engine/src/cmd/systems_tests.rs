//! Mechanisms with analytic answers, as the agent evaluations build them: a slider-crank through a
//! full cycle, a gear pair, a rack and pinion, and a clevis whose sweep must stop where geometry
//! says the link meets its fork. Each checks kinematics and contact together, end to end.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn part(s: &mut Session, name: &str, cmds: &[(&str, Value)]) {
    run(s, "component.activate", json!({"component": "root"}));
    run(s, "component.create", json!({"name": name}));
    for (id, p) in cmds {
        run(s, id, p.clone());
    }
    run(s, "component.activate", json!({"component": "root"}));
}

fn values(s: &mut Session) -> Vec<(String, f64)> {
    let l = run(s, "joint.list", json!({}));
    l["joints"].as_array().unwrap().iter().filter_map(|j| Some((j["name"].as_str()?.to_string(), j["values"][0]["value"].as_f64()?))).collect()
}

fn value(s: &mut Session, name: &str) -> f64 {
    values(s).into_iter().find(|(n, _)| n == name).map(|(_, v)| v).unwrap_or(f64::NAN)
}

/// Crank radius 25 and rod 100: the piston strokes 2r = 50 mm, the rod swings ±asin(r/l), and a
/// full turn meets no collision.
#[test]
fn a_slider_crank_strokes_twice_its_crank_radius_through_a_clear_cycle() {
    let mut s = Session::default();
    part(&mut s, "Base", &[("solid.box", json!({"length": 240, "width": 60, "height": 10, "corner": [-40, -30, -10]}))]);
    part(&mut s, "Crank", &[("solid.box", json!({"length": 37, "width": 12, "height": 5, "corner": [-6, -6, 0]}))]);
    part(&mut s, "Rod", &[("solid.box", json!({"length": 112, "width": 12, "height": 5, "corner": [19, -6, 5]}))]);
    part(&mut s, "Piston", &[("solid.box", json!({"length": 20, "width": 20, "height": 10, "corner": [115, -10, 10]}))]);
    run(&mut s, "occurrence.ground", json!({"occurrence": "Base", "grounded": true}));
    let rev = |name: &str, a: &str, b: &str, at: [f64; 3]| json!({"type": "revolute", "name": name, "a": {"occurrence": a, "point": at}, "b": {"occurrence": b, "point": at}, "flip": true});
    run(&mut s, "joint.create", rev("Crank pivot", "Base", "Crank", [0.0, 0.0, 0.0]));
    run(&mut s, "joint.create", rev("Crank pin", "Crank", "Rod", [25.0, 0.0, 5.0]));
    run(&mut s, "joint.create", rev("Wrist pin", "Rod", "Piston", [125.0, 0.0, 10.0]));
    // A slider moves along its origin's z: here along X.
    run(
        &mut s,
        "joint.create",
        json!({"type": "slider", "name": "Bore", "a": {"occurrence": "Base", "point": [125, 0, 0], "z": [1, 0, 0]}, "b": {"occurrence": "Piston", "point": [125, 0, 0], "z": [1, 0, 0]}, "flip": true}),
    );
    run(&mut s, "motion.study", json!({"name": "Cycle", "steps": 72, "keys": [{"joint": "Crank pivot", "points": [[0, 0], [72, 360]]}]}));
    let i = run(&mut s, "motion.interference", json!({"study": "Cycle"}));
    assert_eq!(i["clear"], true, "{i}");
    assert!(i["stopped_at"].is_null() && i["stopped_by_loop"].is_null(), "the loop closes all the way round: {i}");
    let p = run(&mut s, "motion.plot", json!({"study": "Cycle"}));
    let range = |name: &str| {
        let v: Vec<f64> = p["joints"].as_array().unwrap().iter().find(|j| j["name"] == name).unwrap()["value"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    };
    let (lo, hi) = range("Bore");
    assert!((hi - lo - 50.0).abs() < 1e-6, "stroke {}", hi - lo);
    let (wlo, whi) = range("Wrist pin");
    let swing = (25.0f64 / 100.0).asin().to_degrees();
    assert!((whi - swing).abs() < 1e-3 && (wlo + swing).abs() < 1e-3, "rod swing {wlo}..{whi}, want ±{swing}");
}

/// Pitch radii 10 and 50: a turn of the pinion turns the wheel a fifth of a turn the other way.
#[test]
fn a_gear_pair_turns_at_the_ratio_of_its_pitch_radii() {
    let mut s = Session::default();
    part(&mut s, "Frame", &[("solid.box", json!({"length": 140, "width": 120, "height": 5, "corner": [-60, -60, -5]}))]);
    part(&mut s, "Wheel", &[("solid.cylinder", json!({"base": [0, 0, 0], "axis": [0, 0, 1], "radius": 49.5, "height": 8}))]);
    part(&mut s, "Pinion", &[("solid.cylinder", json!({"base": [60, 0, 0], "axis": [0, 0, 1], "radius": 9.5, "height": 8}))]);
    run(&mut s, "occurrence.ground", json!({"occurrence": "Frame", "grounded": true}));
    for (name, x) in [("Wheel", 0.0), ("Pinion", 60.0)] {
        run(
            &mut s,
            "joint.create",
            json!({"type": "revolute", "name": name, "a": {"occurrence": "Frame", "point": [x, 0, 0]}, "b": {"occurrence": name, "point": [x, 0, 0]}, "flip": true}),
        );
    }
    run(&mut s, "joint.gear", json!({"a": "Pinion", "b": "Wheel", "ra": 10, "rb": 50}));
    run(&mut s, "joint.drive", json!({"joint": "Pinion", "value": 360}));
    assert!((value(&mut s, "Wheel") + 72.0).abs() < 1e-9, "{:?}", values(&mut s));
    let sweep = run(&mut s, "joint.sweep", json!({"joint": "Pinion", "from": 0, "to": 360}));
    assert!(sweep["stopped_at"].is_null(), "1 mm between the blanks: they never touch: {sweep}");
}

/// A 10 mm pinion turned 90 degrees moves its rack r·π/2.
#[test]
fn a_rack_travels_the_pinion_radius_times_its_turn() {
    let mut s = Session::default();
    part(&mut s, "Frame", &[("solid.box", json!({"length": 120, "width": 40, "height": 5, "corner": [-60, -20, -5]}))]);
    part(&mut s, "Pinion", &[("solid.cylinder", json!({"base": [0, 0, 0], "axis": [0, 0, 1], "radius": 9.5, "height": 5}))]);
    part(&mut s, "Rack", &[("solid.box", json!({"length": 80, "width": 6, "height": 5, "corner": [-40, 10.5, 0]}))]);
    run(&mut s, "occurrence.ground", json!({"occurrence": "Frame", "grounded": true}));
    run(
        &mut s,
        "joint.create",
        json!({"type": "revolute", "name": "Pinion", "a": {"occurrence": "Frame", "point": [0, 0, 0]}, "b": {"occurrence": "Pinion", "point": [0, 0, 0]}, "flip": true}),
    );
    run(
        &mut s,
        "joint.create",
        json!({"type": "slider", "name": "Rack", "a": {"occurrence": "Frame", "point": [0, 13.5, 0], "z": [1, 0, 0]}, "b": {"occurrence": "Rack", "point": [0, 13.5, 0], "z": [1, 0, 0]}, "flip": true}),
    );
    run(&mut s, "joint.rack_pinion", json!({"a": "Pinion", "b": "Rack", "radius": 10}));
    run(&mut s, "joint.drive", json!({"joint": "Pinion", "value": 90}));
    assert!((value(&mut s, "Rack").abs() - 10.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{:?}", values(&mut s));
}

/// A link on a pin at height 35 over a 10 mm base block reaching to x = 20: turned down, its lower
/// edge (10 below its axis) meets the block's corner when the corner is 10 from the link's line,
/// at 51.34 − asin(10 / 32.02) = 33.13 degrees. The sweep must stop there, not pass through or
/// stop short; turned up, nothing is in the way.
#[test]
fn a_clevis_sweep_stops_where_the_link_meets_its_fork() {
    let mut s = Session::default();
    part(&mut s, "Fork", &[("solid.box", json!({"length": 40, "width": 32, "height": 10, "corner": [-20, -16, 0]}))]);
    part(
        &mut s,
        "Link",
        &[
            ("solid.box", json!({"length": 80, "width": 15.6, "height": 20, "corner": [0, -7.8, 25]})),
            ("solid.cylinder", json!({"base": [0, -7.8, 35], "axis": [0, 1, 0], "radius": 10, "height": 15.6, "operation": "join"})),
        ],
    );
    run(&mut s, "occurrence.ground", json!({"occurrence": "Fork", "grounded": true}));
    run(
        &mut s,
        "joint.create",
        json!({"type": "revolute", "name": "Pivot", "a": {"occurrence": "Fork", "point": [0, 0, 35], "z": [0, 1, 0]}, "b": {"occurrence": "Link", "point": [0, 0, 35], "z": [0, 1, 0]}, "flip": true}),
    );
    let corner = (20.0f64).hypot(25.0);
    let want = (25.0f64 / 20.0).atan().to_degrees() - (10.0 / corner).asin().to_degrees();
    let ways: Vec<Value> = [90, -90].iter().map(|to| run(&mut s, "joint.sweep", json!({"joint": "Pivot", "from": 0, "to": to}))).collect();
    let stops: Vec<f64> = ways.iter().filter_map(|w| w["stopped_at"].as_f64()).collect();
    assert_eq!(stops.len(), 1, "one way meets the block, the other is free: {ways:?}");
    assert!((stops[0].abs() - want).abs() < 0.05, "stops at {}, want {want}", stops[0]);
}

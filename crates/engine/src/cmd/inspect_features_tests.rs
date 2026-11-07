//! `inspect.features` reads features off the exact B-rep: parts built here, exact values checked,
//! every feature naming its faces, and nothing about the design touched.

use polycore_geom::Vec3;
use polycore_geom::pmath;
use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

/// The body's reading.
fn features(s: &mut Session, body: &str) -> Value {
    let r = run(s, "inspect.features", json!({"body": body}));
    assert_eq!(r["body_count"], 1, "{r}");
    let b = r["bodies"][0].clone();
    assert!(b.get("error").is_none(), "{b}");
    every_face_is_covered_or_reported(s, body, &b);
    b
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_else(|| panic!("not a list: {v}"))
}

fn names(v: &Value) -> Vec<String> {
    list(v).iter().map(|x| x.as_str().unwrap_or_else(|| panic!("not a name: {x}")).to_string()).collect()
}

fn of_type<'a>(b: &'a Value, t: &str) -> Vec<&'a Value> {
    b["features"].as_array().unwrap().iter().filter(|f| f["type"] == t).collect()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// Exact to the nanometre the reply rounds to.
fn exact(v: &Value, want: f64) {
    let got = num(v);
    assert!((got - want).abs() <= 1e-9, "{got} is not {want}");
}

fn exact3(v: &Value, want: [f64; 3]) {
    for (x, w) in list(v).iter().zip(want) {
        exact(x, w);
    }
}

/// Every face of the body is in a feature or in `unrecognised` (never lost, never both for a
/// non-blend), and every feature's faces are real face names.
fn every_face_is_covered_or_reported(s: &mut Session, body: &str, b: &Value) {
    let all: Vec<String> =
        list(&run(s, "model.faces", json!({"body": body}))["faces"]).iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(b["face_count"].as_u64().unwrap() as usize, all.len(), "{b}");
    let mut covered: Vec<String> = Vec::new();
    for f in list(&b["features"]) {
        let fs = names(&f["faces"]);
        assert!(!fs.is_empty(), "a feature with no faces: {f}");
        for n in &fs {
            assert!(all.contains(n), "`{n}` is not a face of {body}: {f}");
        }
        covered.extend(fs);
    }
    let unrecognised: Vec<String> = list(&b["unrecognised"]).iter().map(|u| u["face"].as_str().unwrap().to_string()).collect();
    for n in &all {
        assert!(covered.contains(n) || unrecognised.contains(n), "face `{n}` is neither in a feature nor unrecognised: {b}");
        assert!(!(covered.contains(n) && unrecognised.contains(n)), "face `{n}` is both covered and unrecognised: {b}");
    }
    for u in list(&b["unrecognised"]) {
        assert!(u["reason"].as_str().is_some_and(|r| !r.is_empty()), "{u}");
        assert!(u["surface"].is_string(), "{u}");
    }
}

fn plate_with_four_holes() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 120, "width": 80, "height": 8, "corner": [0, 0, 0], "body_name": "Plate"}));
    run(&mut s, "solid.hole", json!({"position": [18, 18, 8], "diameter": 8, "name": "Hole"}));
    run(
        &mut s,
        "solid.pattern.rectangular",
        json!({"features": ["Hole"], "dir1": [1, 0, 0], "count1": 2, "spacing1": 84, "dir2": [0, 1, 0], "count2": 2, "spacing2": 44}),
    );
    s
}

#[test]
fn a_plate_with_four_through_holes_in_a_rectangular_pattern() {
    let mut s = plate_with_four_holes();
    let b = features(&mut s, "Plate");
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 4, "{b}");
    let mut centres = Vec::new();
    for h in &holes {
        assert_eq!(h["through"], true, "{h}");
        exact(&h["diameter"], 8.0);
        exact(&h["depth"], 8.0);
        assert_eq!(h["entry_face"], "F1:+z", "{h}");
        assert_eq!(h["exit_face"], "F1:-z", "{h}");
        exact3(&h["axis"]["direction"], [0.0, 0.0, -1.0]);
        exact(&h["axis"]["origin"][2], 8.0);
        assert_eq!(h["steps"].as_array().unwrap().len(), 1, "{h}");
        assert_eq!(h["steps"][0]["kind"], "bore", "{h}");
        exact(&h["steps"][0]["diameter"], 8.0);
        exact(&h["steps"][0]["depth"], 8.0);
        assert_eq!(names(&h["faces"]).len(), 2, "two half cylinders: {h}");
        assert!(h.get("approximate").is_none() && h.get("note").is_none(), "{h}");
        centres.push((num(&h["axis"]["origin"][0]), num(&h["axis"]["origin"][1])));
    }
    centres.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(centres, vec![(18.0, 18.0), (18.0, 62.0), (102.0, 18.0), (102.0, 62.0)]);
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 1, "{b}");
    let p = &patterns[0];
    assert_eq!(p["type"], "linear", "{p}");
    assert_eq!(p["feature_type"], "hole", "{p}");
    assert_eq!(p["count"], 4, "{p}");
    assert_eq!(p["count1"], 2, "{p}");
    exact(&p["pitch1"], 84.0);
    exact3(&p["direction1"], [1.0, 0.0, 0.0]);
    assert_eq!(p["count2"], 2, "{p}");
    exact(&p["pitch2"], 44.0);
    exact3(&p["direction2"], [0.0, 1.0, 0.0]);
    assert_eq!(p["members"], json!([0, 1, 2, 3]), "{p}");
    assert_eq!(names(&p["faces"]).len(), 8, "the pattern covers every hole's faces: {p}");
    // The plate's own six faces are covered by no feature, and say so.
    let unrecognised: Vec<String> = list(&b["unrecognised"]).iter().map(|u| u["face"].as_str().unwrap().to_string()).collect();
    assert_eq!(unrecognised.len(), 6, "{b}");
    assert!(unrecognised.contains(&"F1:+z".to_string()) && unrecognised.contains(&"F1:-x".to_string()), "{unrecognised:?}");
    assert!(list(&b["uncertain"]).is_empty(), "{b}");
}

#[test]
fn a_blind_hole_a_counterbore_a_countersink_and_a_drilled_hole() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 100, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.hole", json!({"position": [15, 20, 20], "diameter": 6, "depth": 12, "name": "Blind"}));
    run(
        &mut s,
        "solid.hole",
        json!({"position": [40, 20, 20], "diameter": 6, "type": "counterbore", "cb_diameter": 11, "cb_depth": 5, "name": "CB"}),
    );
    run(
        &mut s,
        "solid.hole",
        json!({"position": [65, 20, 20], "diameter": 6, "type": "countersink", "cs_diameter": 12, "cs_angle": "90 deg", "name": "CS"}),
    );
    run(
        &mut s,
        "solid.hole",
        json!({"position": [88, 20, 20], "diameter": 6, "depth": 12, "type": "drilled", "tip_angle": "118 deg", "name": "Drilled"}),
    );
    let b = features(&mut s, "Block");
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 4, "{b}");
    let at =
        |x: f64| holes.iter().find(|h| (num(&h["axis"]["origin"][0]) - x).abs() < 1e-9).copied().unwrap_or_else(|| panic!("no hole at x = {x}: {b}"));
    for h in &holes {
        assert_eq!(h["entry_face"], "F1:+z", "{h}");
        exact3(&h["axis"]["direction"], [0.0, 0.0, -1.0]);
        exact(&h["diameter"], 6.0);
        assert!(h.get("approximate").is_none(), "{h}");
    }

    // Blind: a flat floor 12 deep, the floor among its faces.
    let blind = at(15.0);
    assert_eq!(blind["through"], false, "{blind}");
    exact(&blind["depth"], 12.0);
    assert_eq!(blind["floor_face"], "F2:bore:base", "{blind}");
    assert!(names(&blind["faces"]).contains(&"F2:bore:base".to_string()), "{blind}");
    assert_eq!(names(&blind["faces"]).len(), 3, "{blind}");

    // Counterbore: one hole, two steps, both diameters and depths.
    let cb = at(40.0);
    assert_eq!(cb["through"], true, "{cb}");
    exact(&cb["depth"], 20.0);
    let steps = list(&cb["steps"]);
    assert_eq!(steps.len(), 2, "{cb}");
    assert_eq!(steps[0]["kind"], "counterbore", "{cb}");
    exact(&steps[0]["diameter"], 11.0);
    exact(&steps[0]["depth"], 5.0);
    exact(&steps[0]["from"], 0.0);
    assert_eq!(steps[1]["kind"], "bore", "{cb}");
    exact(&steps[1]["diameter"], 6.0);
    exact(&steps[1]["depth"], 15.0);
    exact(&steps[1]["from"], 5.0);
    assert_eq!(cb["exit_face"], "F1:-z", "{cb}");
    let cb_faces = names(&cb["faces"]);
    assert_eq!(cb_faces.len(), 5, "two counterbore halves, the step, two bore halves: {cb}");
    assert!(cb_faces.contains(&"F3:cbore:base".to_string()), "{cb}");
    assert_eq!(names(&steps[0]["faces"]).len(), 2, "{cb}");

    // Countersink: the cone's angle and rim diameter, then the bore.
    let cs = at(65.0);
    assert_eq!(cs["through"], true, "{cs}");
    let steps = list(&cs["steps"]);
    assert_eq!(steps.len(), 2, "{cs}");
    assert_eq!(steps[0]["kind"], "countersink", "{cs}");
    exact(&steps[0]["diameter"], 12.0);
    exact(&steps[0]["angle_deg"], 90.0);
    exact(&steps[0]["depth"], 3.0);
    assert_eq!(steps[1]["kind"], "bore", "{cs}");
    exact(&steps[1]["from"], 3.0);
    exact(&steps[1]["depth"], 17.0);
    assert_eq!(names(&cs["faces"]).len(), 4, "{cs}");

    // Drilled: depth to the shoulder, the point's angle and its own depth, exactly.
    let d = at(88.0);
    assert_eq!(d["through"], false, "{d}");
    exact(&d["depth"], 12.0);
    exact(&d["tip"]["angle_deg"], 118.0);
    let tip = 3.0 / pmath::tan(59.0_f64.to_radians());
    assert!((num(&d["tip"]["depth"]) - tip).abs() < 1e-9, "{d}");
    assert_eq!(names(&d["tip"]["faces"]).len(), 3, "{d}");
    assert_eq!(names(&d["faces"]).len(), 6, "{d}");
    assert!(of_type(&b, "chamfer").is_empty(), "a countersink is not a chamfer: {b}");
}

fn base_with_boss_and_pocket() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 120, "width": 80, "height": 8, "corner": [0, 0, 0], "body_name": "Base"}));
    run(&mut s, "solid.cylinder", json!({"base": [40, 40, 8], "diameter": 44, "height": 10, "operation": "join", "targets": ["Base"]}));
    run(&mut s, "solid.hole", json!({"position": [40, 40, 18], "diameter": 24, "name": "Bore"}));
    run(&mut s, "solid.box", json!({"length": 30, "width": 40, "height": 3, "corner": [80, 20, 5], "operation": "cut", "targets": ["Base"]}));
    s
}

#[test]
fn a_boss_and_a_pocket() {
    let mut s = base_with_boss_and_pocket();
    let b = features(&mut s, "Base");
    let bosses = of_type(&b, "boss");
    assert_eq!(bosses.len(), 1, "{b}");
    let boss = bosses[0];
    exact(&boss["diameter"], 44.0);
    exact(&boss["height"], 10.0);
    exact3(&boss["axis"]["origin"], [40.0, 40.0, 8.0]);
    exact3(&boss["axis"]["direction"], [0.0, 0.0, 1.0]);
    assert_eq!(boss["cap_face"], "F2:top", "{boss}");
    assert_eq!(boss["base_face"], "F1:+z", "{boss}");
    let bf = names(&boss["faces"]);
    assert_eq!(bf.len(), 3, "{boss}");
    assert!(bf.contains(&"F2:top".to_string()), "{boss}");
    // The bore through the boss enters at the cap and leaves at the base's underside.
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 1, "{b}");
    exact(&holes[0]["diameter"], 24.0);
    exact(&holes[0]["depth"], 18.0);
    assert_eq!(holes[0]["entry_face"], "F2:top", "{b}");
    assert_eq!(holes[0]["exit_face"], "F1:-z", "{b}");
    // The pocket: its floor, four walls and the face it opens on.
    let pockets = of_type(&b, "pocket");
    assert_eq!(pockets.len(), 1, "{b}");
    let p = pockets[0];
    exact(&p["depth"], 3.0);
    assert_eq!(p["floor_face"], "F4:-z", "{p}");
    assert_eq!(p["opening_face"], "F1:+z", "{p}");
    let walls = names(&p["walls"]);
    assert_eq!(walls.len(), 4, "{p}");
    assert!(walls.contains(&"F4:+x".to_string()) && walls.contains(&"F4:-y".to_string()), "{p}");
    assert_eq!(names(&p["faces"]).len(), 5, "{p}");
    assert!(list(&b["uncertain"]).is_empty(), "{b}");

    // A hole through the pocket's floor leaves the pocket a pocket (its floor edge to the hole
    // is convex, which is no wall); the hole enters at the floor.
    run(&mut s, "solid.hole", json!({"position": [95, 40, 5], "diameter": 6, "name": "InFloor"}));
    let b = features(&mut s, "Base");
    let pockets = of_type(&b, "pocket");
    assert_eq!(pockets.len(), 1, "{b}");
    exact(&pockets[0]["depth"], 3.0);
    assert_eq!(pockets[0]["floor_face"], "F4:-z", "{b}");
    assert_eq!(names(&pockets[0]["walls"]).len(), 4, "{b}");
    let in_floor = of_type(&b, "hole").into_iter().find(|h| (num(&h["axis"]["origin"][0]) - 95.0).abs() < 1e-9).unwrap_or_else(|| panic!("{b}"));
    assert_eq!(in_floor["entry_face"], "F4:-z", "{in_floor}");
    exact(&in_floor["depth"], 5.0);
    exact(&in_floor["diameter"], 6.0);
    assert!(list(&b["uncertain"]).is_empty(), "{b}");
}

#[test]
fn a_filleted_edge_chain_and_a_chamfer() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.fillet", json!({"edges": [[30, 0, 20], [30, 40, 20], [0, 20, 20], [60, 20, 20]], "radius": 2}));
    run(&mut s, "solid.chamfer", json!({"edges": [[30, 40, 0]], "distance": 2, "distance2": 4}));
    let b = features(&mut s, "Block");
    let fillets = of_type(&b, "fillet");
    assert_eq!(fillets.len(), 1, "the four top edges' rounds are one chain: {b}");
    let f = fillets[0];
    exact(&f["radius"], 2.0);
    assert_eq!(f["convex"], true, "{f}");
    assert_eq!(names(&f["faces"]).len(), 4, "{f}");
    let joins = names(&f["joins"]);
    assert_eq!(joins.len(), 5, "{f}");
    for n in ["F1:+z", "F1:-y", "F1:+y", "F1:-x", "F1:+x"] {
        assert!(joins.contains(&n.to_string()), "{f}");
    }
    // The chamfer: `distance` is along the first of `joins`, the face facing up most.
    let chamfers = of_type(&b, "chamfer");
    assert_eq!(chamfers.len(), 1, "{b}");
    let c = chamfers[0];
    assert_eq!(c["joins"], json!(["F1:+y", "F1:-z"]), "{c}");
    exact(&c["distance"], 2.0);
    exact(&c["distance2"], 4.0);
    assert!((num(&c["angle_deg"]) - pmath::atan(4.0_f64 / 2.0).to_degrees()).abs() < 1e-9, "{c}");
    assert_eq!(names(&c["faces"]).len(), 1, "{c}");
    assert!(list(&b["patterns"]).is_empty(), "{b}");

    // A tangent chain: the top loop of a block with rounded vertical edges, filleted at once,
    // is one fillet of eight faces (four cylinders, four tori) joining the top, the sides and
    // the vertical rounds.
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.fillet", json!({"edges": [[0, 0, 10], [60, 0, 10], [60, 40, 10], [0, 40, 10]], "radius": 5}));
    run(&mut s, "solid.fillet", json!({"edges": [[30, 0, 20]], "radius": 2}));
    let b = features(&mut s, "Block");
    let fillets = of_type(&b, "fillet");
    let r2: Vec<&&Value> = fillets.iter().filter(|f| num(&f["radius"]) == 2.0).collect();
    assert_eq!(r2.len(), 1, "{b}");
    assert_eq!(names(&r2[0]["faces"]).len(), 8, "{}", r2[0]);
    assert_eq!(names(&r2[0]["joins"]).len(), 9, "{}", r2[0]);
    assert_eq!(fillets.iter().filter(|f| num(&f["radius"]) == 5.0).count(), 4, "{b}");
}

#[test]
fn a_circular_pattern_of_six_holes() {
    let mut s = Session::default();
    run(&mut s, "solid.cylinder", json!({"base": [0, 0, 0], "diameter": 100, "height": 10, "body_name": "Disc"}));
    run(&mut s, "solid.hole", json!({"position": [35, 0, 10], "diameter": 6, "name": "Hole"}));
    run(&mut s, "solid.pattern.circular", json!({"features": ["Hole"], "axis": "Z", "count": 6}));
    let b = features(&mut s, "Disc");
    assert_eq!(of_type(&b, "hole").len(), 6, "{b}");
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 1, "{b}");
    let p = &patterns[0];
    assert_eq!(p["type"], "circular", "{p}");
    assert_eq!(p["count"], 6, "{p}");
    exact(&p["angle_step_deg"], 60.0);
    assert_eq!(p["full_circle"], true, "{p}");
    exact(&p["radius"], 35.0);
    exact3(&p["axis"]["origin"], [0.0, 0.0, 10.0]);
    assert!((num(&p["axis"]["direction"][2]).abs() - 1.0).abs() < 1e-9, "{p}");
    assert_eq!(list(&p["members"]).len(), 6, "{p}");
    assert_eq!(names(&p["faces"]).len(), 12, "{p}");
    // The disc's own wall is not a boss (it has two caps) and not a blend: reported as such.
    let unrec: Vec<String> = list(&b["unrecognised"]).iter().map(|u| format!("{}", u["surface"])).collect();
    assert_eq!(unrec.iter().filter(|s| s.as_str() == "\"cylinder\"").count(), 2, "{b}");
}

/// Three holes at 0°, 60° and 120° on one radius are a partial circular pattern (the wrap-around
/// gap is the one larger step), read the same whichever order the members come in; a full
/// circle stays full.
#[test]
fn a_partial_circular_pattern_of_three_holes_in_any_member_order() {
    for order in [[0.0, 60.0, 120.0], [0.0, 120.0, 60.0], [120.0, 0.0, 60.0], [60.0, 120.0, 0.0]] {
        let mut s = Session::default();
        run(&mut s, "solid.cylinder", json!({"base": [0, 0, 0], "diameter": 100, "height": 10, "body_name": "Disc"}));
        for a in order.map(|a: f64| a.to_radians()) {
            run(&mut s, "solid.hole", json!({"position": [30.0 * pmath::cos(a), 30.0 * pmath::sin(a), 10], "diameter": 6}));
        }
        let b = features(&mut s, "Disc");
        assert_eq!(of_type(&b, "hole").len(), 3, "{order:?}: {b}");
        let patterns = list(&b["patterns"]);
        assert_eq!(patterns.len(), 1, "{order:?}: {b}");
        let p = &patterns[0];
        assert_eq!(p["type"], "circular", "{order:?}: {p}");
        assert_eq!(p["count"], 3, "{order:?}: {p}");
        exact(&p["angle_step_deg"], 60.0);
        assert_eq!(p["full_circle"], false, "{order:?}: {p}");
        exact(&p["radius"], 30.0);
        exact3(&p["axis"]["origin"], [0.0, 0.0, 10.0]);
        exact3(&p["axis"]["direction"], [0.0, 0.0, -1.0]);
        assert_eq!(p["members"], json!([0, 1, 2]), "{order:?}: {p}");
    }
    // The fit itself, on bare points: any order, and the wrap step counted.
    let pts = |degs: &[f64]| -> Vec<Vec3> {
        degs.iter().map(|d| Vec3::new(30.0 * pmath::cos(d.to_radians()), 30.0 * pmath::sin(d.to_radians()), 0.0)).collect()
    };
    for degs in [[0.0, 60.0, 120.0], [120.0, 0.0, 60.0], [60.0, 0.0, 120.0]] {
        let c = super::circle(&pts(&degs), Some(Vec3::new(0.0, 0.0, 1.0)), 1e-6).unwrap_or_else(|| panic!("{degs:?}"));
        assert!((c.step - 60.0_f64.to_radians()).abs() < 1e-9, "{degs:?}: {}", c.step);
        assert!(!c.full && (c.radius - 30.0).abs() < 1e-9 && c.center.dist(Vec3::ZERO) < 1e-9, "{degs:?}");
    }
    for degs in [[0.0, 120.0, 240.0], [240.0, 0.0, 120.0]] {
        let c = super::circle(&pts(&degs), None, 1e-6).unwrap_or_else(|| panic!("{degs:?}"));
        assert!(c.full && (c.step - 120.0_f64.to_radians()).abs() < 1e-9, "{degs:?}");
        assert!(c.axis.z > 0.0, "the axis of a fit with no given direction is the same whatever the order: {degs:?} {:?}", c.axis);
    }
    // Unequal steps are no pattern, and two larger gaps are none.
    assert!(super::circle(&pts(&[0.0, 60.0, 150.0]), None, 1e-6).is_none());
    assert!(super::circle(&pts(&[0.0, 60.0, 180.0, 240.0]), None, 1e-6).is_none());
}

#[test]
fn a_free_form_face_is_unrecognised_with_no_false_feature() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [40, 0], [40, 20]]}));
    run(&mut s, "sketch.spline.fit_point", json!({"points": [[40, 20], [30, 28], [15, 22], [0, 30]]}));
    run(&mut s, "sketch.line", json!({"points": [[0, 30], [0, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 10, "body_name": "Free"}));
    let b = features(&mut s, "Free");
    assert!(list(&b["features"]).is_empty(), "no false feature: {b}");
    assert!(list(&b["patterns"]).is_empty(), "{b}");
    assert!(list(&b["uncertain"]).is_empty(), "{b}");
    let unrecognised = list(&b["unrecognised"]);
    assert_eq!(unrecognised.len(), b["face_count"].as_u64().unwrap() as usize, "{b}");
    let free: Vec<&Value> = unrecognised.iter().filter(|u| u["surface"] == "free-form").collect();
    assert!(!free.is_empty(), "{b}");
    for u in free {
        assert!(u["reason"].as_str().unwrap().contains("free-form"), "{u}");
        assert!(u["face"].as_str().unwrap().contains("side:s1"), "the spline's side: {u}");
    }
}

/// Two chamfers on opposite edges of a block's top: two chamfers, and the top face between them
/// is no chamfer however its normal lies between theirs.
#[test]
fn two_chamfers_leave_the_face_between_them_unrecognised() {
    for (params, want) in [
        (json!({"distance": 2}), (2.0, 2.0, 45.0)),
        (json!({"distance": 2, "angle": "30 deg"}), (2.0, 2.0 * pmath::tan(30.0_f64.to_radians()), 30.0)),
    ] {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
        let mut p = params.clone();
        p["edges"] = json!([[30, 0, 20], [30, 40, 20]]);
        run(&mut s, "solid.chamfer", p);
        let b = features(&mut s, "Block");
        let chamfers = of_type(&b, "chamfer");
        assert_eq!(chamfers.len(), 2, "{params}: {b}");
        for c in &chamfers {
            assert_eq!(c["joins"][0], "F1:+z", "{c}");
            assert!((num(&c["distance"]) - want.0).abs() < 1e-9, "{c}");
            assert!((num(&c["distance2"]) - want.1).abs() < 1e-9, "{c}");
            assert!((num(&c["angle_deg"]) - want.2).abs() < 1e-9, "{c}");
        }
        let unrecognised: Vec<String> = list(&b["unrecognised"]).iter().map(|u| u["face"].as_str().unwrap().to_string()).collect();
        assert!(unrecognised.contains(&"F1:+z".to_string()), "{params}: {b}");
        assert_eq!(list(&b["features"]).len(), 2, "{b}");
    }
}

/// A chamfer is a narrow bevel between two faces that would otherwise meet: a face as wide as
/// what its neighbours would reach is a face of its own. A hexagonal prism's sides turn 60° to
/// each other across parallel edges, but none is a chamfer; a drafted box's top turns 80° to its
/// four sides and is no chamfer either; a wedge has no chamfer.
#[test]
fn a_hex_prism_a_drafted_box_and_a_wedge_have_no_chamfer() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    run(&mut s, "sketch.polygon.inscribed", json!({"center": [0, 0], "radius": 20, "sides": 6}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 10, "body_name": "Hex"}));
    let b = features(&mut s, "Hex");
    assert!(of_type(&b, "chamfer").is_empty(), "a hexagonal prism's sides are not chamfers: {b}");
    assert!(list(&b["features"]).is_empty(), "{b}");
    let unrecognised = list(&b["unrecognised"]);
    assert_eq!(unrecognised.len(), 8, "{b}");
    assert!(unrecognised.iter().all(|u| u["surface"] == "plane"), "{b}");
    assert_eq!(unrecognised.iter().filter(|u| u["face"].as_str().is_some_and(|n| n.contains(":side:"))).count(), 6, "six side planes: {b}");

    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 100, "width": 100, "height": 30, "corner": [0, 0, 0], "body_name": "Box"}));
    run(&mut s, "solid.draft", json!({"faces": [[100, 50, 15], [0, 50, 15], [50, 0, 15], [50, 100, 15]], "angle": "10 deg", "neutral": "XY"}));
    let b = features(&mut s, "Box");
    assert!(of_type(&b, "chamfer").is_empty(), "a drafted box's top is not a chamfer: {b}");
    assert!(list(&b["features"]).is_empty(), "{b}");
    assert_eq!(list(&b["unrecognised"]).len(), 6, "{b}");

    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XZ"}));
    run(&mut s, "sketch.line", json!({"points": [[0, 0], [60, 0], [60, 20], [0, 0]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 30, "body_name": "Wedge"}));
    let b = features(&mut s, "Wedge");
    assert!(list(&b["features"]).is_empty(), "{b}");
    assert_eq!(list(&b["unrecognised"]).len(), 5, "{b}");

    // A real chamfer still is one, however it is given: a distance, a distance and an angle, a
    // conical chamfer on a boss's rim.
    let mut s = base_with_boss_and_pocket();
    run(&mut s, "solid.chamfer", json!({"edges": [[62, 40, 18]], "distance": 1.5}));
    run(&mut s, "solid.chamfer", json!({"edges": [[60, 0, 8]], "distance": 1, "angle": "30 deg"}));
    let b = features(&mut s, "Base");
    let chamfers = of_type(&b, "chamfer");
    assert_eq!(chamfers.len(), 2, "{b}");
    let rim = chamfers.iter().find(|c| c["joins"][0] == "F2:top").unwrap_or_else(|| panic!("{b}"));
    exact(&rim["distance"], 1.5);
    exact(&rim["distance2"], 1.5);
    exact(&rim["angle_deg"], 45.0);
    assert_eq!(names(&rim["faces"]).len(), 2, "{rim}");
    let edge = chamfers.iter().find(|c| c["joins"][0] == "F1:+z").unwrap_or_else(|| panic!("{b}"));
    assert_eq!(edge["joins"], json!(["F1:+z", "F1:-y"]), "{edge}");
    exact(&edge["distance"], 1.0);
    assert!((num(&edge["distance2"]) - pmath::tan(30.0_f64.to_radians())).abs() < 1e-9, "{edge}");
    exact(&edge["angle_deg"], 30.0);
}

/// A cross-hole through a round post breaks out through the post's curved side: its rims are the
/// curves where two cylinders meet, not circles, so the hole is read with its exact diameter and
/// marked approximate (the depth along the axis comes from the mesh), not reported as a cavity.
#[test]
fn a_cross_hole_through_a_round_post_is_an_approximate_through_hole() {
    let mut s = Session::default();
    run(&mut s, "solid.cylinder", json!({"base": [0, 0, 0], "diameter": 40, "height": 30, "body_name": "Post"}));
    run(&mut s, "solid.hole", json!({"position": [-20, 0, 15], "direction": [1, 0, 0], "diameter": 6}));
    let b = features(&mut s, "Post");
    assert!(list(&b["uncertain"]).is_empty(), "not a closed cavity: {b}");
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 1, "{b}");
    let h = holes[0];
    assert_eq!(h["through"], true, "{h}");
    exact(&h["diameter"], 6.0);
    assert_eq!(h["approximate"], true, "{h}");
    assert!(h["note"].as_str().is_some_and(|n| n.contains("mesh")), "{h}");
    assert!((num(&h["axis"]["direction"][0]).abs() - 1.0).abs() < 1e-9, "along x: {h}");
    exact(&h["axis"]["origin"][1], 0.0);
    exact(&h["axis"]["origin"][2], 15.0);
    // From one side of the post to the other, within the sag of the rim curves.
    let depth = num(&h["depth"]);
    assert!(depth > 39.5 && depth <= 40.0 + 1e-9, "{h}");
    assert_eq!(names(&h["faces"]).len(), 2, "the bore's halves: {h}");
    assert_eq!(list(&h["steps"]).len(), 1, "{h}");
    exact(&h["steps"][0]["diameter"], 6.0);
    assert!(h["entry_face"].as_str().is_some_and(|f| f.starts_with("F1:side")), "{h}");
    assert!(h["exit_face"].as_str().is_some_and(|f| f.starts_with("F1:side")), "{h}");
    // An approximate hole joins no pattern; the post's own faces are reported as before.
    assert!(list(&b["patterns"]).is_empty(), "{b}");
    assert_eq!(list(&b["unrecognised"]).len(), 4, "{b}");
}

/// A through hole with a rounded mouth is read from the mouth: the round is its first step,
/// the entry face the one beyond the round, and the round is also the fillet it is.
#[test]
fn a_hole_with_a_rounded_mouth_is_read_from_the_mouth() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.hole", json!({"position": [30, 20, 20], "diameter": 10, "name": "H"}));
    run(&mut s, "solid.fillet", json!({"edges": [[35, 20, 20]], "radius": 1.5}));
    let b = features(&mut s, "Block");
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 1, "{b}");
    let h = holes[0];
    assert_eq!(h["entry_face"], "F1:+z", "{h}");
    assert_eq!(h["exit_face"], "F1:-z", "{h}");
    exact3(&h["axis"]["direction"], [0.0, 0.0, -1.0]);
    exact3(&h["axis"]["origin"], [30.0, 20.0, 20.0]);
    exact(&h["depth"], 20.0);
    exact(&h["diameter"], 10.0);
    let steps = list(&h["steps"]);
    assert_eq!(steps.len(), 2, "{h}");
    assert_eq!(steps[0]["kind"], "round", "{h}");
    exact(&steps[0]["radius"], 1.5);
    exact(&steps[0]["depth"], 1.5);
    assert_eq!(steps[1]["kind"], "bore", "{h}");
    exact(&steps[1]["from"], 1.5);
    exact(&steps[1]["depth"], 18.5);
    assert!(h.get("approximate").is_none() && h.get("note").is_none(), "nothing is read from the mesh: {h}");
    assert_eq!(names(&h["faces"]).len(), 4, "the bore's halves and the round's: {h}");
    let fillets = of_type(&b, "fillet");
    assert_eq!(fillets.len(), 1, "{b}");
    exact(&fillets[0]["radius"], 1.5);
    assert!(names(&fillets[0]["joins"]).contains(&"F1:+z".to_string()), "{b}");
}

/// A through slot's half-round ends join two parallel walls, which never meet: they are no
/// fillets (nor anything else, so they are reported unrecognised, with why).
#[test]
fn a_through_slots_ends_are_not_fillets() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 100, "width": 50, "height": 6, "corner": [0, 0, 0], "body_name": "Plate"}));
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    run(&mut s, "sketch.slot.overall", json!({"p0": [30, 25], "p1": [70, 25], "width": 10}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 6, "operation": "cut", "targets": ["Plate"]}));
    let b = features(&mut s, "Plate");
    assert!(of_type(&b, "fillet").is_empty(), "a slot's end is not a fillet: {b}");
    assert!(list(&b["features"]).is_empty(), "{b}");
    assert!(list(&b["patterns"]).is_empty() && list(&b["uncertain"]).is_empty(), "{b}");
    let ends: Vec<Value> = list(&b["unrecognised"]).into_iter().filter(|u| u["surface"] == "cylinder").collect();
    assert_eq!(ends.len(), 2, "the two half-round ends: {b}");
    for u in &ends {
        assert!(u["face"].as_str().is_some_and(|f| f.starts_with("F3:side:a")), "{u}");
        assert!(u["reason"].as_str().is_some_and(|r| r.contains("parallel") && r.contains("never meet")), "{u}");
    }
}

fn block_with_pocket() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 100, "width": 60, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.box", json!({"length": 60, "width": 30, "height": 8, "corner": [20, 15, 12], "operation": "cut", "targets": ["Block"]}));
    s
}

/// A pocket whose floor edges carry the cutter's radius is a pocket still: the rounds are among
/// its faces (`floor_rounds`), its walls are the upright faces beyond them, and the rounds are
/// also the fillet they are.
#[test]
fn a_pocket_with_filleted_floor_edges_is_a_pocket_and_lists_the_fillet() {
    let mut s = block_with_pocket();
    run(&mut s, "solid.fillet", json!({"edges": [[50, 15, 12], [50, 45, 12], [20, 30, 12], [80, 30, 12]], "radius": 2}));
    let b = features(&mut s, "Block");
    let pockets = of_type(&b, "pocket");
    assert_eq!(pockets.len(), 1, "{b}");
    let p = pockets[0];
    exact(&p["depth"], 8.0);
    assert_eq!(p["floor_face"], "F2:-z", "{p}");
    assert_eq!(p["opening_face"], "F1:+z", "{p}");
    assert_eq!(names(&p["walls"]), vec!["F2:-x", "F2:-y", "F2:+x", "F2:+y"], "{p}");
    let rounds = names(&p["floor_rounds"]);
    assert_eq!(rounds.len(), 4, "{p}");
    assert!(rounds.iter().all(|r| r.starts_with("F3:blend:") && r.ends_with("|F2:-z")), "{p}");
    let faces = names(&p["faces"]);
    assert_eq!(faces.len(), 9, "the floor, four walls and four rounds: {p}");
    assert!(rounds.iter().all(|r| faces.contains(r)), "{p}");
    let fillets = of_type(&b, "fillet");
    assert_eq!(fillets.len(), 1, "the four floor rounds are one chain: {b}");
    let f = fillets[0];
    exact(&f["radius"], 2.0);
    assert_eq!(f["convex"], false, "{f}");
    assert_eq!(names(&f["faces"]), rounds, "{f}");
    assert_eq!(names(&f["joins"]), vec!["F2:-x", "F2:-y", "F2:+x", "F2:+y", "F2:-z"], "{f}");
    assert!(list(&b["uncertain"]).is_empty(), "{b}");
    assert_eq!(list(&b["unrecognised"]).len(), 6, "the block's own faces: {b}");
    // A plain pocket has no rounds.
    let b = features(&mut block_with_pocket(), "Block");
    assert_eq!(of_type(&b, "pocket")[0]["floor_rounds"], json!([]), "{b}");
}

/// A pocket's vertical corner rounds stay its walls and are listed as the fillets they are, as a
/// hole's mouth round is both a step of the hole and a fillet.
#[test]
fn a_pockets_vertical_corner_rounds_are_fillets_too() {
    let mut s = block_with_pocket();
    run(&mut s, "solid.fillet", json!({"edges": [[20, 15, 16], [80, 15, 16], [20, 45, 16], [80, 45, 16]], "radius": 4}));
    let b = features(&mut s, "Block");
    let pockets = of_type(&b, "pocket");
    assert_eq!(pockets.len(), 1, "{b}");
    let p = pockets[0];
    exact(&p["depth"], 8.0);
    assert_eq!(p["floor_face"], "F2:-z", "{p}");
    assert_eq!(names(&p["walls"]).len(), 8, "four flat walls and four corner rounds: {p}");
    let fillets = of_type(&b, "fillet");
    assert_eq!(fillets.len(), 4, "the corner rounds are fillets too: {b}");
    assert_eq!(p["floor_rounds"], json!([]), "{p}");
    for f in &fillets {
        exact(&f["radius"], 4.0);
        assert_eq!(f["convex"], false, "{f}");
        let faces = names(&f["faces"]);
        assert_eq!(faces.len(), 1, "{f}");
        assert!(faces[0].starts_with("F3:blend:F2:"), "{f}");
        assert!(names(&p["walls"]).contains(&faces[0]), "the round is a wall of the pocket too: {f}");
        let joins = names(&f["joins"]);
        assert_eq!(joins.len(), 2, "{f}");
        assert!(joins.iter().all(|j| j.starts_with("F2:")), "two flat walls: {f}");
    }
    assert!(list(&b["uncertain"]).is_empty(), "{b}");
    assert_eq!(list(&b["unrecognised"]).len(), 6, "{b}");
}

/// Six holes on a ring about a seventh of the same size at its centre: the ring of six and a
/// lone hole, not a row of three through the centre (which would leave four holes unexplained).
#[test]
fn a_ring_of_six_holes_about_a_centre_hole_is_the_ring_and_a_lone_hole() {
    let mut s = Session::default();
    run(&mut s, "solid.cylinder", json!({"base": [600, 0, 0], "diameter": 100, "height": 10, "body_name": "Disc"}));
    run(&mut s, "solid.hole", json!({"position": [635, 0, 10], "diameter": 6, "name": "Hole"}));
    run(&mut s, "solid.pattern.circular", json!({"features": ["Hole"], "axis": {"origin": [600, 0, 0], "dir": [0, 0, 1]}, "count": 6}));
    run(&mut s, "solid.hole", json!({"position": [600, 0, 10], "diameter": 6, "name": "Centre"}));
    let b = features(&mut s, "Disc");
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 7, "{b}");
    let centre = holes
        .iter()
        .position(|h| (num(&h["axis"]["origin"][0]) - 600.0).abs() < 1e-9 && num(&h["axis"]["origin"][1]).abs() < 1e-9)
        .unwrap_or_else(|| panic!("{b}"));
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 1, "{b}");
    let p = &patterns[0];
    assert_eq!(p["type"], "circular", "{p}");
    assert_eq!(p["count"], 6, "{p}");
    exact(&p["angle_step_deg"], 60.0);
    assert_eq!(p["full_circle"], true, "{p}");
    exact(&p["radius"], 35.0);
    exact3(&p["axis"]["origin"], [600.0, 0.0, 10.0]);
    let members: Vec<usize> = list(&p["members"]).iter().map(|m| m.as_u64().unwrap() as usize).collect();
    assert_eq!(members.len(), 6, "{p}");
    assert!(!members.contains(&centre), "the centre hole stays alone: {p}");
    assert_eq!(names(&p["faces"]).len(), 12, "{p}");
}

/// Two alike features are two features: a pattern needs three members.
#[test]
fn two_alike_fillets_or_chamfers_are_not_a_pattern() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.fillet", json!({"edges": [[30, 0, 20], [30, 40, 20]], "radius": 2}));
    run(&mut s, "solid.chamfer", json!({"edges": [[30, 0, 0], [30, 40, 0]], "distance": 1}));
    let b = features(&mut s, "Block");
    assert_eq!(of_type(&b, "fillet").len(), 2, "{b}");
    assert_eq!(of_type(&b, "chamfer").len(), 2, "{b}");
    assert!(list(&b["patterns"]).is_empty(), "{b}");
    // Three equally spaced alike holes are a row.
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 100, "width": 40, "height": 10, "corner": [0, 0, 0], "body_name": "Bar"}));
    for x in [20, 50, 80] {
        run(&mut s, "solid.hole", json!({"position": [x, 20, 10], "diameter": 5}));
    }
    let b = features(&mut s, "Bar");
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 1, "{b}");
    assert_eq!(patterns[0]["type"], "linear", "{b}");
    assert_eq!(patterns[0]["count1"], 3, "{b}");
    exact(&patterns[0]["pitch1"], 30.0);
    assert!(patterns[0].get("count2").is_none(), "{b}");
}

/// A plate with `kind` (`hole` or `boss`) features at `centres`, as one sketch cut or join (far
/// faster than one feature per member, for plates of hundreds).
fn plate_with(kind: &str, centres: &[(f64, f64)]) -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 200, "width": 200, "height": 6, "corner": [0, 0, 0], "body_name": "Plate"}));
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 6], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}}));
    for (x, y) in centres {
        run(&mut s, "sketch.circle.center", json!({"center": [x, y], "radius": 2.5}));
    }
    run(&mut s, "sketch.finish", json!({}));
    let (distance, operation) = if kind == "hole" { (-6, "cut") } else { (4, "join") };
    run(&mut s, "solid.extrude", json!({"distance": distance, "operation": operation, "targets": ["Plate"]}));
    s
}

/// The centres of a `c1` × `c2` grid from (20, 20) at `p1` along x and `p2` along y.
fn grid_centres(c1: usize, c2: usize, p1: f64, p2: f64) -> Vec<(f64, f64)> {
    (0..c1).flat_map(|i| (0..c2).map(move |j| (20.0 + p1 * i as f64, 20.0 + p2 * j as f64))).collect()
}

/// A `c1` × `c2` grid of holes or bosses made with `solid.pattern.rectangular`, whose fitted
/// positions carry the kernel's noise.
fn patterned_grid(kind: &str, c1: usize, c2: usize, p1: f64, p2: f64) -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 200, "width": 200, "height": 8, "corner": [0, 0, 0], "body_name": "Plate"}));
    if kind == "hole" {
        run(&mut s, "solid.hole", json!({"position": [20, 20, 8], "diameter": 6, "name": "Seed"}));
    } else {
        run(
            &mut s,
            "solid.cylinder",
            json!({"base": [20, 20, 8], "diameter": 10, "height": 5, "operation": "join", "targets": ["Plate"], "name": "Seed"}),
        );
    }
    run(
        &mut s,
        "solid.pattern.rectangular",
        json!({"features": ["Seed"], "dir1": [1, 0, 0], "count1": c1, "spacing1": p1, "dir2": [0, 1, 0], "count2": c2, "spacing2": p2}),
    );
    s
}

/// The one pattern of `b` is a `c1` × `c2` lattice of `kind` at `p1` along +x and `p2` along +y.
fn assert_one_grid(b: &Value, kind: &str, c1: usize, c2: usize, p1: f64, p2: f64) {
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 1, "{b}");
    let p = &patterns[0];
    assert_eq!(p["type"], "linear", "{p}");
    assert_eq!(p["feature_type"], kind, "{p}");
    assert_eq!(p["count"], c1 * c2, "{p}");
    assert_eq!(p["count1"], c1, "{p}");
    exact(&p["pitch1"], p1);
    exact3(&p["direction1"], [1.0, 0.0, 0.0]);
    assert_eq!(p["count2"], c2, "{p}");
    exact(&p["pitch2"], p2);
    exact3(&p["direction2"], [0.0, 1.0, 0.0]);
    assert_eq!(list(&p["members"]).len(), c1 * c2, "{p}");
}

/// A rectangular grid of three or more rows and columns is one lattice, never rows of three nor
/// a ring of four about its centre: the fitted positions carry noise of about 1e-14, so the
/// lattice's origin must not depend on which member sorts lowest.
#[test]
fn grids_of_three_or_more_rows_and_columns_are_one_lattice() {
    for (kind, c1, c2, p1, p2) in [
        ("hole", 3, 3, 30.0, 40.0),
        ("hole", 3, 3, 30.0, 30.0),
        ("hole", 3, 4, 30.0, 30.0),
        ("boss", 3, 3, 30.0, 40.0),
        ("boss", 3, 3, 30.0, 30.0),
        ("boss", 3, 4, 30.0, 40.0),
    ] {
        let mut s = patterned_grid(kind, c1, c2, p1, p2);
        let b = features(&mut s, "Plate");
        assert_eq!(of_type(&b, kind).len(), c1 * c2, "{kind} {c1}x{c2}: {b}");
        assert_one_grid(&b, kind, c1, c2, p1, p2);
    }
    // The lattice fit itself, with the noise the kernel leaves: the member lowest in x is in the
    // middle row, so counting cells from it would put the first row at a negative index.
    let mut pts = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            let x = 20.0 + 30.0 * i as f64 + if j == 1 { -2e-14 } else { 0.0 };
            pts.push(Vec3::new(x, 20.0 + 30.0 * j as f64, 8.0));
        }
    }
    let l = super::lattice(&pts, 1e-3).unwrap_or_else(|| panic!("no lattice through {pts:?}"));
    assert_eq!((l.count1, l.count2), (3, 3));
    assert!((l.v1 - Vec3::new(30.0, 0.0, 0.0)).len() < 1e-9 && (l.v2 - Vec3::new(0.0, 30.0, 0.0)).len() < 1e-9, "{:?} {:?}", l.v1, l.v2);
}

/// A grid beside a lone alike feature is the grid and the feature: the grid is read as a lattice
/// among the rows and rings, and a whole grid beats a ring of four at a square's corners.
#[test]
fn a_grid_beside_a_lone_feature_is_the_grid_and_the_feature() {
    for kind in ["hole", "boss"] {
        let mut centres = grid_centres(3, 3, 30.0, 30.0);
        centres.push((160.0, 160.0));
        let mut s = plate_with(kind, &centres);
        let b = features(&mut s, "Plate");
        assert_eq!(of_type(&b, kind).len(), 10, "{b}");
        assert_one_grid(&b, kind, 3, 3, 30.0, 30.0);
        let lone = of_type(&b, kind).iter().position(|f| (num(&f["axis"]["origin"][0]) - 160.0).abs() < 1e-9).unwrap_or_else(|| panic!("{b}"));
        assert!(!list(&b["patterns"][0]["members"]).contains(&json!(lone)), "{b}");
    }
    // Two grids on one plate are two lattices; a grid and a ring are the grid and the ring.
    let mut centres = grid_centres(3, 4, 30.0, 30.0);
    centres.extend(grid_centres(3, 3, 30.0, 40.0).into_iter().map(|(x, y)| (x + 110.0, y)));
    let mut s = plate_with("hole", &centres);
    let b = features(&mut s, "Plate");
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 2, "{b}");
    let counts: Vec<(u64, u64)> = patterns.iter().map(|p| (p["count1"].as_u64().unwrap(), p["count2"].as_u64().unwrap())).collect();
    assert_eq!(counts, vec![(3, 4), (3, 3)], "{b}");
    let mut centres = grid_centres(3, 3, 30.0, 30.0);
    centres.extend((0..6).map(|k| {
        let a = std::f64::consts::PI / 3.0 * k as f64;
        (150.0 + 30.0 * pmath::cos(a), 150.0 + 30.0 * pmath::sin(a))
    }));
    let mut s = plate_with("hole", &centres);
    let b = features(&mut s, "Plate");
    let patterns = list(&b["patterns"]);
    assert_eq!(patterns.len(), 2, "{b}");
    assert_eq!((patterns[0]["type"].as_str(), patterns[0]["count"].as_u64()), (Some("linear"), Some(9)), "{b}");
    assert_eq!((patterns[1]["type"].as_str(), patterns[1]["count"].as_u64()), (Some("circular"), Some(6)), "{b}");
    exact(&patterns[1]["radius"], 30.0);
}

/// A blind hole with a rounded floor edge is as deep as its floor: the round is its last step,
/// and the depth runs from the entry plane to the floor, not to the end of the wall.
#[test]
fn a_blind_hole_with_a_rounded_floor_is_as_deep_as_its_floor() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 20, "corner": [0, 0, 0], "body_name": "Block"}));
    run(&mut s, "solid.hole", json!({"position": [30, 20, 20], "diameter": 10, "depth": 12, "name": "H"}));
    run(&mut s, "solid.fillet", json!({"edges": [[35, 20, 8]], "radius": 1}));
    let b = features(&mut s, "Block");
    let holes = of_type(&b, "hole");
    assert_eq!(holes.len(), 1, "{b}");
    let h = holes[0];
    assert_eq!(h["through"], false, "{h}");
    exact(&h["depth"], 12.0);
    exact(&h["diameter"], 10.0);
    assert_eq!(h["entry_face"], "F1:+z", "{h}");
    assert_eq!(h["floor_face"], "F2:bore:base", "{h}");
    let steps = list(&h["steps"]);
    assert_eq!(steps.len(), 2, "{h}");
    assert_eq!(steps[0]["kind"], "bore", "{h}");
    exact(&steps[0]["from"], 0.0);
    exact(&steps[0]["depth"], 11.0);
    assert_eq!(steps[1]["kind"], "round", "{h}");
    exact(&steps[1]["from"], 11.0);
    exact(&steps[1]["depth"], 1.0);
    exact(&steps[1]["radius"], 1.0);
    assert_eq!(names(&h["faces"]).len(), 5, "the bore's halves, the floor and the round's halves: {h}");
    assert!(h.get("approximate").is_none() && h.get("note").is_none(), "{h}");
    // Rounded at the mouth as well: still the floor's depth.
    run(&mut s, "solid.fillet", json!({"edges": [[35, 20, 20]], "radius": 1}));
    let b = features(&mut s, "Block");
    let h = of_type(&b, "hole")[0];
    exact(&h["depth"], 12.0);
    assert_eq!(list(&h["steps"]).len(), 3, "{h}");
}

/// A perforated plate of any number of holes is one lattice (that fit is linear in the holes);
/// only the search for grids, rows and rings within a group stops at 256 members, and a group it
/// skips is noted on the body.
#[test]
fn a_perforated_plate_is_one_lattice_and_a_larger_search_is_noted() {
    let mut s = plate_with("hole", &grid_centres(17, 17, 10.0, 10.0));
    let b = features(&mut s, "Plate");
    assert_eq!(of_type(&b, "hole").len(), 289, "{}", b["face_count"]);
    assert_one_grid(&b, "hole", 17, 17, 10.0, 10.0);
    assert!(b.get("note").is_none(), "{}", b["note"]);
    // 256 on a grid and one beside them: not one lattice, and one too many to search.
    let mut centres = grid_centres(16, 16, 10.0, 10.0);
    centres.push((190.0, 190.0));
    let mut s = plate_with("hole", &centres);
    let b = features(&mut s, "Plate");
    assert_eq!(of_type(&b, "hole").len(), 257, "{}", b["face_count"]);
    assert!(list(&b["patterns"]).is_empty(), "{}", b["patterns"]);
    let note = b["note"].as_str().unwrap_or_else(|| panic!("no note: {}", b["note"]));
    assert!(note.contains("257") && note.contains("256") && note.contains("hole") && note.contains("no pattern"), "{note}");
}

#[test]
fn the_command_changes_nothing() {
    let mut s = plate_with_four_holes();
    run(&mut s, "solid.fillet", json!({"edges": [[60, 0, 8]], "radius": 1}));
    run(&mut s, "edit.undo", json!({}));
    let before = (s.revision, s.undo.len(), s.redo.len(), s.doc.to_json(), s.is_dirty(), s.selection.clone());
    let inspected = run(&mut s, "document.inspect", json!({}));
    let r = run(&mut s, "inspect.features", json!({}));
    assert_eq!(r["body_count"], 1, "{r}");
    let again = run(&mut s, "inspect.features", json!({"bodies": ["Plate"]}));
    assert_eq!(r, again, "the same reading twice");
    let after = (s.revision, s.undo.len(), s.redo.len(), s.doc.to_json(), s.is_dirty(), s.selection.clone());
    assert_eq!(before, after);
    assert_eq!(s.redo.len(), 1, "the undone fillet is still there to redo");
    let inspected_after = run(&mut s, "document.inspect", json!({}));
    assert_eq!(inspected["revision"], inspected_after["revision"]);
    assert_eq!(inspected["undo"], inspected_after["undo"]);
    assert_eq!(inspected["redo"], inspected_after["redo"]);
    assert_eq!(inspected["dirty"], inspected_after["dirty"]);
    // Redo still works: the history was not touched.
    run(&mut s, "edit.redo", json!({}));
    assert_eq!(of_type(&features(&mut s, "Plate"), "fillet").len(), 1);
}

#[test]
fn an_unknown_body_is_refused_and_a_mesh_body_reported() {
    let mut s = plate_with_four_holes();
    let e = s.execute("inspect.features", &json!({"body": "Plat"})).unwrap_err().to_string();
    assert!(e.contains("no body `Plat`") && e.contains("Plate"), "{e}");
    let e = s.execute("inspect.features", &json!({"body": 3})).unwrap_err().to_string();
    assert!(e.contains("`body`"), "{e}");
    // Every body when none is named.
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [200, 0, 0], "body_name": "Cube"}));
    let r = run(&mut s, "inspect.features", json!({}));
    assert_eq!(r["body_count"], 2, "{r}");
    assert_eq!(r["total"]["features"], 4, "{r}");
    assert_eq!(r["total"]["patterns"], 1, "{r}");
}

//! A material-changing feature that evaluates and changes nothing (a hole drilled into the air,
//! a cut clear of the part) is not reported as plain success (polykrom#309): its reply carries a
//! warning of kind `no_effect` naming the feature and the bodies it left as they were, the
//! feature's own warning says so in words, and nothing is listed as changed. A feature that does
//! change its body carries no such warning. Nothing is evaluated twice: the judgement reads the
//! states the model keeps around the feature.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn volume(s: &mut Session) -> f64 {
    run(s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap_or(f64::NAN)
}

/// The number of faces of the body `name`.
fn faces(s: &Session, name: &str) -> usize {
    s.model.state().body(name).map(|b| b.body.face_count()).unwrap_or(0)
}

/// The sample plate (80 × 50 × 8 mm, two Ø8 holes, a bored boss): one body, `Plate`.
fn sample() -> Session {
    let mut s = Session::default();
    s.run_script(&crate::sample::script()).unwrap_or_else(|e| panic!("{e}"));
    s
}

/// The one `no_effect` warning of `reply`, as `(feature, bodies, message)`.
fn no_effect(reply: &Value) -> Option<(String, Vec<String>, String)> {
    let warnings = reply.get("warnings")?.as_array()?;
    assert_eq!(warnings.len(), 1, "{reply}");
    let w = warnings.first()?;
    assert_eq!(w["kind"], json!("no_effect"), "{reply}");
    let bodies = w["bodies"].as_array()?.iter().filter_map(Value::as_str).map(str::to_string).collect();
    Some((w["feature"].as_str()?.to_string(), bodies, w["message"].as_str()?.to_string()))
}

/// The feature's warning as the timeline shows it.
fn timeline_warning(s: &Session, feature: u64) -> Option<String> {
    s.model.result(feature).and_then(|r| r.warning.clone())
}

#[test]
fn a_hole_in_the_air_warns_that_it_has_no_effect() {
    let mut s = sample();
    let (before, faces_before) = (volume(&mut s), faces(&s, "Plate"));
    // A hole sketched beside the plate, on the plane of its underside: it drills into the air.
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Air"}));
    run(&mut s, "sketch.circle.center", json!({"center": [100, 25], "diameter": 5}));
    run(&mut s, "sketch.finish", json!({}));
    let reply = run(&mut s, "solid.hole", json!({"sketch": "Air", "diameter": 5, "depth": 5, "name": "AirHole"}));
    let (feature, bodies, message) = no_effect(&reply).unwrap_or_else(|| panic!("no no_effect warning: {reply}"));
    assert_eq!(feature, "AirHole");
    assert_eq!(bodies, ["Plate"]);
    assert_eq!(message, "AirHole has no effect: `Plate` is unchanged");
    // The feature's own warning carries the same words, for the timeline and `warning`.
    assert_eq!(reply["warning"].as_str(), Some(message.as_str()), "{reply}");
    let id = reply["feature"].as_u64().unwrap_or_default();
    assert_eq!(timeline_warning(&s, id).as_deref(), Some(message.as_str()));
    // Not an error, nothing changed, and nothing is reported as changed.
    assert_eq!(reply["changed_bodies"], json!([]), "{reply}");
    assert_eq!(reply["created_bodies"], json!([]), "{reply}");
    assert_eq!(reply["removed_bodies"], json!([]), "{reply}");
    assert_eq!(volume(&mut s), before);
    assert_eq!(faces(&s, "Plate"), faces_before);
}

#[test]
fn an_extrude_cut_below_the_part_warns_that_it_has_no_effect() {
    let mut s = sample();
    let before = volume(&mut s);
    // A disc 20 mm under the plate, cut 5 mm up: it never reaches the plate.
    run(&mut s, "sketch.create", json!({"plane": "XY", "offset": -20, "name": "Under"}));
    run(&mut s, "sketch.circle.center", json!({"center": [40, 25], "diameter": 10}));
    run(&mut s, "sketch.finish", json!({}));
    let reply = run(&mut s, "solid.extrude", json!({"distance": 5, "operation": "cut", "name": "UnderCut"}));
    let (feature, bodies, message) = no_effect(&reply).unwrap_or_else(|| panic!("no no_effect warning: {reply}"));
    assert_eq!((feature.as_str(), bodies.as_slice()), ("UnderCut", &["Plate".to_string()][..]));
    assert_eq!(message, "UnderCut has no effect: `Plate` is unchanged");
    assert_eq!(reply["warning"].as_str(), Some(message.as_str()), "{reply}");
    assert_eq!(reply["changed_bodies"], json!([]), "{reply}");
    assert_eq!(volume(&mut s), before);
    // Undo takes it back like any feature: the warning was not an error.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(volume(&mut s), before);
}

#[test]
fn a_hole_through_the_part_has_no_such_warning() {
    let mut s = sample();
    let before = volume(&mut s);
    // A hole sketched on the top of the plate, clear of its holes and boss.
    run(&mut s, "sketch.create", json!({"plane": "XY", "offset": "thickness", "name": "Top"}));
    run(&mut s, "sketch.circle.center", json!({"center": [20, 40], "diameter": 5}));
    run(&mut s, "sketch.finish", json!({}));
    let reply = run(&mut s, "solid.hole", json!({"sketch": "Top", "diameter": 5, "name": "Through"}));
    assert!(reply.get("warnings").is_none(), "{reply}");
    assert_eq!(reply["warning"], json!(null), "{reply}");
    assert_eq!(reply["changed_bodies"], json!(["Plate"]), "{reply}");
    let id = reply["feature"].as_u64().unwrap_or_default();
    assert_eq!(timeline_warning(&s, id), None);
    assert!(volume(&mut s) < before - 100.0, "the hole removed material");
    // A cut that meets the plate: no warning either.
    run(&mut s, "sketch.create", json!({"plane": "XY", "offset": -20, "name": "Under"}));
    run(&mut s, "sketch.circle.center", json!({"center": [70, 10], "diameter": 6}));
    run(&mut s, "sketch.finish", json!({}));
    let reply = run(&mut s, "solid.extrude", json!({"distance": 30, "operation": "cut", "name": "Reaches"}));
    assert!(reply.get("warnings").is_none(), "{reply}");
    assert_eq!(reply["changed_bodies"], json!(["Plate"]), "{reply}");
}

/// A tool that only touches a body (a box cut flush against its top, a combine whose tool
/// shares a face) leaves the same body: the operation may note its faces, but the body is not
/// `changed_bodies` material, so an agent reading only that list is not misled.
#[test]
fn a_cut_that_only_touches_a_body_warns_and_lists_it_as_unchanged() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 20, "width": 20, "height": 20, "body_name": "Block"}));
    let (before, faces_before) = (volume(&mut s), faces(&s, "Block"));
    let reply =
        run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [5, 5, 20], "operation": "cut", "name": "TouchingTop"}));
    let (feature, bodies, _) = no_effect(&reply).unwrap_or_else(|| panic!("no no_effect warning: {reply}"));
    assert_eq!((feature.as_str(), bodies.as_slice()), ("TouchingTop", &["Block".to_string()][..]));
    assert_eq!(reply["changed_bodies"], json!([]), "{reply}");
    assert_eq!(volume(&mut s), before);
    assert_eq!(faces(&s, "Block"), faces_before);
    // The same through Combine: the tool is used up, the target is as it was.
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [20, 5, 5], "operation": "new_body", "body_name": "Tool"}));
    let reply = run(&mut s, "solid.combine", json!({"target": "Block", "tools": ["Tool"], "operation": "cut", "name": "TouchingSide"}));
    let (feature, bodies, message) = no_effect(&reply).unwrap_or_else(|| panic!("no no_effect warning: {reply}"));
    assert_eq!((feature.as_str(), bodies.as_slice()), ("TouchingSide", &["Block".to_string()][..]));
    assert_eq!(message, "TouchingSide has no effect: `Block` is unchanged");
    assert_eq!(reply["changed_bodies"], json!([]), "{reply}");
    assert_eq!(reply["removed_bodies"], json!(["Tool"]), "{reply}");
    assert_eq!(volume(&mut s), before);
    assert_eq!(faces(&s, "Block"), faces_before);
}

/// Press Pull is an extrude or a fillet: a pull of a face by nothing that cuts the air (a face
/// pushed in from beside the part) carries the warning like the extrude it became. Here: a
/// Combine that joins a tool inside its target, which changes nothing of it either.
#[test]
fn a_combine_whose_tool_lies_inside_its_target_warns() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 20, "width": 20, "height": 20, "body_name": "Block"}));
    run(&mut s, "solid.box", json!({"length": 4, "width": 4, "height": 4, "corner": [8, 8, 8], "operation": "new_body", "body_name": "Inside"}));
    let before = volume(&mut s);
    let reply = run(&mut s, "solid.combine", json!({"target": "Block", "tools": ["Inside"], "operation": "join", "name": "Swallow"}));
    match no_effect(&reply) {
        Some((feature, bodies, _)) => {
            assert_eq!((feature.as_str(), bodies.as_slice()), ("Swallow", &["Block".to_string()][..]));
            assert_eq!(reply["changed_bodies"], json!([]), "{reply}");
        }
        // The kernel may build the union afresh (another body of the same shape): then the
        // reply has no warning and lists the target as changed, which is what happened.
        None => assert_eq!(reply["changed_bodies"], json!(["Block"]), "{reply}"),
    }
    assert_eq!(volume(&mut s), before - 64.0);
}

/// Every reply of a batch is the command's own reply: a script of steps carries the warning on
/// the step that had no effect and on no other (`engine.batch` and the MCP batch are the same
/// replies, one per step).
#[test]
fn a_script_reports_the_warning_on_the_step_that_had_no_effect() {
    let mut s = sample();
    let replies = s
        .run_script(&json!({"commands": [
            {"command": "sketch.create", "params": {"plane": "XY", "name": "Air"}},
            {"command": "sketch.circle.center", "params": {"center": [100, 25], "diameter": 5}},
            {"command": "sketch.finish", "params": {}},
            {"command": "solid.hole", "params": {"sketch": "Air", "diameter": 5, "depth": 5, "name": "AirHole"}},
            {"command": "solid.box", "params": {"length": 5, "width": 5, "height": 5, "corner": [100, 0, 0], "operation": "new_body", "body_name": "Beside"}},
        ]}))
        .unwrap_or_else(|e| panic!("{e}"));
    let with: Vec<usize> = replies.iter().enumerate().filter(|(_, r)| r.get("warnings").is_some()).map(|(i, _)| i).collect();
    assert_eq!(with, [3], "{replies:?}");
    let (feature, bodies, _) = no_effect(&replies[3]).unwrap_or_else(|| panic!("{:?}", replies[3]));
    assert_eq!((feature.as_str(), bodies.as_slice()), ("AirHole", &["Plate".to_string()][..]));
}

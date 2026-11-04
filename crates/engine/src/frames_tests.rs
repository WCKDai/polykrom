//! Components as frames: what a component holds moves with its occurrence, and picks in the
//! world land in the component's own frame (user bug #4).

use polycore_geom::Vec3;
use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn near(a: Vec3, b: Vec3) -> bool {
    a.dist(b) < 1e-6
}

fn world_bbox(s: &Session, body: &str) -> (Vec3, Vec3) {
    let w = s.world_state();
    let b = w.body(body).unwrap_or_else(|| panic!("no body {body}")).mesh().bounds();
    (b.min, b.max)
}

fn sketch_point_world(s: &mut Session, sketch: &str, point: &str) -> Vec3 {
    let si = run(s, "sketch.inspect", json!({ "sketch": sketch }));
    let p = si["points"].as_array().unwrap().iter().find(|p| p["id"] == point).unwrap()["world"].clone();
    Vec3::new(p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap())
}

fn volume(s: &mut Session) -> f64 {
    run(s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap()
}

/// A component "Part" with a 30 x 20 x 10 block (sketch "Base" on XY, "Top" on its top face)
/// and a construction plane, moved 50 mm along X.
fn moved_part() -> Session {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "Part", "grounded": false}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Base"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [30, 20]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 10}));
    run(&mut s, "sketch.create", json!({"plane": {"face": [15, 10, 10]}, "name": "Top"}));
    run(&mut s, "sketch.circle.center", json!({"center": [15, 10], "radius": 3}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "construct.plane.offset", json!({"base": "XY", "offset": 25, "name": "Shelf"}));
    run(&mut s, "occurrence.move", json!({"occurrence": "Part:1", "translate": [50, 0, 0]}));
    s
}

/// (a) Sketches and construction planes move with their component's occurrence.
#[test]
fn sketches_and_planes_move_with_the_component() {
    let mut s = moved_part();
    let (lo, hi) = world_bbox(&s, "Body1");
    assert!(near(lo, Vec3::new(50.0, 0.0, 0.0)) && near(hi, Vec3::new(80.0, 20.0, 10.0)), "{lo:?} {hi:?}");
    assert!(near(sketch_point_world(&mut s, "Base", "p1"), Vec3::new(50.0, 0.0, 0.0)));
    let c = sketch_point_world(&mut s, "Top", "p1");
    assert!((c.z - 10.0).abs() < 1e-6 && c.x >= 50.0 - 1e-6, "{c:?}");
    let shelf = crate::view::construction_planes(&s).into_iter().find(|(_, n, _)| n == "Shelf").unwrap().2;
    assert!(near(shelf.origin, Vec3::new(50.0, 0.0, 25.0)), "{:?}", shelf.origin);
    // A turn about Z carries them too.
    run(&mut s, "occurrence.move", json!({"occurrence": "Part:1", "axis": [0, 0, 1], "angle": "90 deg", "origin": [50, 0, 0]}));
    assert!(near(sketch_point_world(&mut s, "Base", "p3"), Vec3::new(30.0, 30.0, 0.0)), "{:?}", sketch_point_world(&mut s, "Base", "p3"));
}

/// (b) Picks on a moved component, given in the world, land on its geometry: fillet edges, a
/// sketch on a face, a hole, a shell; new geometry of the active component goes where asked.
#[test]
fn world_picks_map_into_the_component() {
    let mut s = moved_part();
    let v0 = volume(&mut s);
    // The edge along X at the top front: world (65, 0, 10).
    run(&mut s, "solid.fillet", json!({"edges": [[65, 0, 10]], "radius": 2}));
    assert!(volume(&mut s) < v0 - 1.0, "the fillet cut material");
    // The modified face has no certified lineage; explicitly freeze its picked plane.
    // Its world pick still maps into the moved component exactly once.
    run(&mut s, "sketch.create", json!({"plane": {"face": [70, 10, 10],"fixed":true}, "name": "Pad"}));
    run(&mut s, "sketch.circle.center", json!({"center": [20, 10], "radius": 2, "id": "PadCircle"}));
    let sketch = run(&mut s, "sketch.inspect", json!({"sketch": "Pad"}));
    let center = sketch["curves"].as_array().unwrap().iter().find(|c| c["id"] == "PadCircle").unwrap()["center"].as_str().unwrap().to_string();
    run(&mut s, "sketch.finish", json!({}));
    // A projected boundary's first point is an enumeration detail; check the circle
    // that was actually authored in the moved component's local sketch coordinates.
    let pad = sketch_point_world(&mut s, "Pad", &center);
    assert!(near(pad, Vec3::new(70.0, 10.0, 10.0)), "{pad:?}");
    let v1 = volume(&mut s);
    let h = run(&mut s, "solid.hole", json!({"position": [60, 10, 10], "diameter": 4, "depth": 5}));
    assert!(volume(&mut s) < v1 - 1.0, "the hole cut material: {h}");
    // A box made at world (100, 0, 0) in the active (moved) component shows there.
    run(&mut s, "solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [100, 0, 0], "operation": "new"}));
    let last = s.model.state().bodies.last().map(|b| b.name.clone()).unwrap();
    let (lo, _) = world_bbox(&s, &last);
    assert!(near(lo, Vec3::new(100.0, 0.0, 0.0)), "{lo:?}");
}

/// (b) A pasted instance's pick fillets the component (every instance).
#[test]
fn picks_on_an_instance_map_through_its_placement() {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "Pin"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "occurrence.copy", json!({"component": "Pin", "translate": [0, 40, 0]}));
    let v0 = volume(&mut s);
    // An edge of the second instance only (world y = 40).
    run(&mut s, "solid.fillet", json!({"edges": [[5, 40, 10]], "radius": 2}));
    let v1 = volume(&mut s);
    assert!(v1 < v0 - 1.0, "{v0} {v1}");
}

/// (c) Moving a body into a moved component keeps it where it is in the world.
#[test]
fn moving_bodies_between_components_keeps_their_place() {
    let mut s = moved_part();
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "solid.box", json!({"length": 4, "width": 4, "height": 4, "corner": [0, 40, 0], "body_name": "Loose"}));
    let before = world_bbox(&s, "Loose");
    run(&mut s, "component.move_bodies", json!({"bodies": ["Loose"], "component": "Part"}));
    let after = world_bbox(&s, "Loose");
    assert!(near(before.0, after.0) && near(before.1, after.1), "{before:?} {after:?}");
    // It moves with the component from now on, and a pick on it still lands.
    run(&mut s, "occurrence.move", json!({"occurrence": "Part:1", "translate": [0, 0, 10]}));
    let moved = world_bbox(&s, "Loose");
    assert!(near(moved.0, before.0 + Vec3::new(0.0, 0.0, 10.0)), "{moved:?}");
    let v0 = volume(&mut s);
    run(&mut s, "solid.fillet", json!({"edges": [[2, 40, 14]], "radius": 1}));
    assert!(volume(&mut s) < v0 - 0.1);
    // Back to the root: still in place.
    run(&mut s, "component.move_bodies", json!({"bodies": ["Loose"], "component": "root"}));
    let back = world_bbox(&s, "Loose");
    assert!(near(back.0, moved.0) && near(back.1, moved.1), "{back:?} {moved:?}");
}

/// (d) A sketch on a body's face follows the body when a Move feature moves it.
#[test]
fn sketch_on_a_face_follows_a_moved_body() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 20, "width": 20, "height": 10}));
    run(&mut s, "sketch.create", json!({"plane": {"face": [10, 10, 10]}, "name": "OnTop"}));
    run(&mut s, "sketch.circle.center", json!({"center": [10, 10], "radius": 2}));
    run(&mut s, "sketch.finish", json!({}));
    // The move goes before the sketch in the timeline: the face the sketch is on moves up.
    run(&mut s, "timeline.roll_to", json!({"position": 1}));
    run(&mut s, "solid.move", json!({"bodies": ["Body1"], "translate": [0, 0, 15]}));
    run(&mut s, "timeline.roll_to", json!({}));
    let c = sketch_point_world(&mut s, "OnTop", "p1");
    assert!((c.z - 25.0).abs() < 1e-6, "{c:?}");
}

/// A 1 x 1 PNG for canvases.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Canvases belong to the active component and move with its occurrence; one put on a moved
/// component's face from the root sits on the face (and stays the root's).
#[test]
fn canvases_move_with_their_component() {
    let mut s = moved_part();
    let part = s.active_component;
    assert_ne!(part, 0);
    run(&mut s, "canvas.insert", json!({"data": PNG, "plane": "XY", "name": "Plan"}));
    // From the root, on the moved block's top face (world).
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "canvas.insert", json!({"data": PNG, "plane": {"face": [65, 10, 10]}, "name": "Lid", "width": 10}));
    run(&mut s, "canvas.insert", json!({"data": PNG, "plane": "XZ", "name": "Root"}));
    let (vals, _) = s.doc.param_values();
    let world = |s: &Session, name: &str| {
        let c = s.doc.canvases.iter().find(|c| c.name == name).unwrap();
        (c.component, s.doc.canvas_plane(&vals, c).unwrap())
    };
    let (c, p) = world(&s, "Plan");
    assert_eq!(c, part);
    assert!(near(p.origin, Vec3::new(50.0, 0.0, 0.0)), "{:?}", p.origin);
    let (c, p) = world(&s, "Lid");
    assert_eq!(c, 0, "the active component's");
    let lid = s.doc.canvases.iter().find(|c| c.name == "Lid").unwrap();
    let centre = p.to_world(lid.center);
    assert!(near(centre, Vec3::new(65.0, 10.0, 10.0)), "{centre:?}");
    assert_eq!(world(&s, "Root").0, 0);
    // Moving the occurrence carries its canvases, not the root's.
    run(&mut s, "occurrence.move", json!({"occurrence": "Part:1", "translate": [0, 40, 0]}));
    let (vals, _) = s.doc.param_values();
    let plan = s.doc.canvases.iter().find(|c| c.name == "Plan").unwrap();
    assert!(near(s.doc.canvas_plane(&vals, plan).unwrap().origin, Vec3::new(50.0, 40.0, 0.0)));
    let lid = s.doc.canvases.iter().find(|c| c.name == "Lid").unwrap();
    let centre = s.doc.canvas_plane(&vals, lid).unwrap().to_world(lid.center);
    assert!(near(centre, Vec3::new(65.0, 10.0, 10.0)), "{centre:?}");
    let root = s.doc.canvases.iter().find(|c| c.name == "Root").unwrap();
    assert!(near(s.doc.canvas_plane(&vals, root).unwrap().origin, Vec3::ZERO));
    // The component survives a save and reopen.
    let back = polykrom_doc::Document::from_json(&s.doc.to_json()).unwrap();
    assert_eq!(back.canvases, s.doc.canvases);
}

/// World points of a sketch's curves' points (sorted).
fn sketch_world_points(s: &mut Session, sketch: &str) -> Vec<Vec3> {
    let si = run(s, "sketch.inspect", json!({ "sketch": sketch }));
    si["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let w = &p["world"];
            Vec3::new(w[0].as_f64().unwrap(), w[1].as_f64().unwrap(), w[2].as_f64().unwrap())
        })
        .collect()
}

/// A block "C" (40 x 30 x 20) in its own component, the root active again.
fn block_in_component() -> Session {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "C", "grounded": false}));
    run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 20}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    s
}

/// The user's "big square": a sketch made in the root on a moved component's face lies on the
/// face where it is shown, its projected loop on the face's edges (moved and turned), and it
/// follows when the occurrence moves on.
#[test]
fn root_sketch_on_a_moved_components_face() {
    let mut s = block_in_component();
    run(&mut s, "occurrence.move", json!({"occurrence": "C:1", "translate": [100, 0, 0]}));
    let r = run(&mut s, "sketch.create", json!({"plane": {"face": [120, 15, 20]}, "name": "Top"}));
    assert!(!r["projected"].is_null(), "the face's edges are projected: {r}");
    run(&mut s, "sketch.finish", json!({}));
    let pts = sketch_world_points(&mut s, "Top");
    assert!(pts.len() >= 4, "{pts:?}");
    // Every point but the sketch's origin (the model origin on the face plane).
    for p in pts.iter().filter(|p| !near(**p, Vec3::new(0.0, 0.0, 20.0))) {
        assert!((p.z - 20.0).abs() < 1e-6 && p.x > 100.0 - 1e-6 && p.x < 140.0 + 1e-6 && p.y > -1e-6 && p.y < 30.0 + 1e-6, "{p:?}");
    }
    for corner in [Vec3::new(100.0, 0.0, 20.0), Vec3::new(140.0, 30.0, 20.0)] {
        assert!(pts.iter().any(|p| near(*p, corner)), "{corner:?} in {pts:?}");
    }
    assert_eq!(s.doc.find_feature("Top").unwrap().component, 0, "the sketch is the root's");

    // Turned 90 degrees about Z (around the world origin): the top face is now x -30..0, y 0..40.
    let mut s = block_in_component();
    run(&mut s, "occurrence.move", json!({"occurrence": "C:1", "axis": [0, 0, 1], "angle": "90 deg", "origin": [0, 0, 0]}));
    run(&mut s, "sketch.create", json!({"plane": {"face": [-15, 20, 20]}, "name": "Top"}));
    run(&mut s, "sketch.finish", json!({}));
    let pts = sketch_world_points(&mut s, "Top");
    for corner in [Vec3::new(0.0, 0.0, 20.0), Vec3::new(-30.0, 40.0, 20.0)] {
        assert!(pts.iter().any(|p| near(*p, corner)), "{corner:?} in {pts:?}");
    }
    // Projected geometry stays on the face when the occurrence moves on (the link re-resolves).
    run(&mut s, "occurrence.move", json!({"occurrence": "C:1", "translate": [0, 0, 5]}));
    let pts = sketch_world_points(&mut s, "Top");
    assert!(pts.iter().any(|p| near(*p, Vec3::new(-30.0, 40.0, 25.0))), "{pts:?}");
}

/// (#558) With the root active, construction geometry, a hole and Press Pull picked on a moved
/// component land on it where it is shown, and so do picks on a pasted instance.
#[test]
fn root_picks_on_a_moved_component_build_where_it_is_shown() {
    let mut s = moved_part();
    run(&mut s, "component.activate", json!({"component": "root"}));
    // The block spans world x 50…80, y 0…20, z 0…10.
    run(&mut s, "construct.axis.edge", json!({"edge": {"edge": [65, 0, 0]}, "name": "FrontEdge"}));
    run(
        &mut s,
        "construct.plane.three_points",
        json!({"a": {"vertex": [50, 0, 10]}, "b": {"vertex": [80, 0, 10]}, "c": {"vertex": [50, 20, 10]}, "name": "Lid"}),
    );
    let lid = crate::view::construction_planes(&s).into_iter().find(|(_, n, _)| n == "Lid").unwrap().2;
    assert!((lid.origin.z - 10.0).abs() < 1e-6 && lid.normal().z.abs() > 1.0 - 1e-9, "{lid:?}");
    let st = s.model.state();
    let axis = st.construct.iter().find(|c| c.name == "FrontEdge").map(|c| c.geom.clone()).unwrap();
    let polykrom_doc::construct::ConstructGeom::Axis { origin, dir } = axis else { panic!("{axis:?}") };
    assert!(origin.y.abs() < 1e-6 && origin.z.abs() < 1e-6 && dir.x.abs() > 1.0 - 1e-9, "{origin:?} {dir:?}");
    let v0 = volume(&mut s);
    run(&mut s, "solid.hole", json!({"position": [60, 10, 10], "diameter": 4, "depth": 5}));
    let v1 = volume(&mut s);
    assert!(v1 < v0 - 1.0, "the hole cut the moved block: {v0} {v1}");
    run(&mut s, "solid.press_pull", json!({"face": [80, 10, 5], "distance": 2}));
    assert!(volume(&mut s) > v1 + 1.0, "press pull grew the moved block");
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);

    // A pasted instance 40 mm further along y.
    run(&mut s, "occurrence.copy", json!({"component": "Part", "translate": [0, 40, 0]}));
    run(&mut s, "construct.plane.along_path", json!({"path": {"edge": [65, 40, 0]}, "t": 0.5, "name": "Across"}));
    let across = crate::view::construction_planes(&s).into_iter().find(|(_, n, _)| n == "Across").unwrap().2;
    assert!((across.origin.y - 40.0).abs() < 1e-6, "{across:?}");
}

/// (#297) Face picks on a component turned 90° about Z are given in the world, a `normal` too:
/// it turns with its pick and settles an edge pick. Without one the error names the point as
/// given, not the point in the component's frame.
#[test]
fn face_picks_on_a_turned_component_take_a_world_normal_and_report_the_given_point() {
    let mut s = block_in_component();
    run(&mut s, "occurrence.move", json!({"occurrence": "C:1", "axis": [0, 0, 1], "angle": "90 deg", "origin": [0, 0, 0]}));
    // The block (40 x 30 x 20) now spans world x -30…0, y 0…40, z 0…20: its +y face faces -x.
    let edge = json!([-30, 20, 20]);
    let err = s.execute("solid.shell", &json!({"faces": [edge], "thickness": 2})).unwrap_err().to_string();
    assert!(err.contains("ambiguous pick at [-30, 20, 20]"), "{err}");
    assert!(err.contains(":+y` of `Body1`") && err.contains(":+z` of `Body1`"), "{err}");
    assert_eq!(s.doc.features.len(), 1, "nothing was added");
    let used = |reply: &Value| reply["faces_used"][0].as_str().unwrap_or_default().to_string();
    let reply = run(&mut s, "solid.shell", json!({"faces": [{"point": edge, "normal": [-1, 0, 0]}], "thickness": 2}));
    assert!(used(&reply).ends_with(":+y"), "{reply}");
    // The +y face open: a 36 x 28 x 16 cavity in the 24 000 mm³ block.
    let v = volume(&mut s);
    assert!((v - (24_000.0 - 36.0 * 28.0 * 16.0)).abs() < 1e-6, "{v}");
    run(&mut s, "edit.undo", json!({}));
    let reply = run(&mut s, "solid.shell", json!({"faces": [{"point": edge, "normal": [0, 0, 1]}], "thickness": 2}));
    assert!(used(&reply).ends_with(":+z"), "{reply}");
    let v = volume(&mut s);
    assert!((v - (24_000.0 - 36.0 * 26.0 * 18.0)).abs() < 1e-6, "{v}");
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
}

/// (#297 review) `sheet.convert`'s `face` is a face pick like the others: on a turned component
/// its ambiguous-pick error names the point as given, in the world, not the point in the
/// component's frame; inside the face the plate converts, and the reply names the face.
#[test]
fn sheet_convert_on_a_turned_component_reports_the_given_point() {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "C", "grounded": false}));
    run(&mut s, "solid.box", json!({"length": 100, "width": 60, "height": 2, "body_name": "Plate"}));
    run(&mut s, "component.activate", json!({"component": "root"}));
    run(&mut s, "occurrence.move", json!({"occurrence": "C:1", "axis": [0, 0, 1], "angle": "90 deg", "origin": [0, 0, 0]}));
    // The plate (100 x 60 x 2) now spans world x -60…0, y 0…100: the top face's edge along its
    // +y side is at x = -60. In the component's frame that pick is [50, 60, 2].
    let err = s.execute("sheet.convert", &json!({"body": "Plate", "face": [-60, 50, 2]})).unwrap_err().to_string();
    assert!(err.contains("ambiguous pick at [-60, 50, 2]"), "{err}");
    assert!(err.contains(":+y` of `Plate`") && err.contains(":+z` of `Plate`"), "{err}");
    assert_eq!(s.doc.features.len(), 1, "nothing was added");
    let reply = run(&mut s, "sheet.convert", json!({"body": "Plate", "face": [-30, 50, 2]}));
    assert!(reply["faces_used"][0].as_str().unwrap_or_default().ends_with(":+z"), "{reply}");
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
}

/// (#297 review) In a moved active component, an entry of a face list that lies off the body
/// (0.1 mm out from a face: within a pick's reach of the face, beyond the body's own) is read in
/// the active component's frame, as every pick was before face lists were mapped by body, and
/// resolves to its face: the shell opens both faces, and `faces_used` names both.
#[test]
fn off_body_face_picks_in_a_moved_active_component_fall_back_to_its_frame() {
    let mut s = Session::default();
    run(&mut s, "component.create", json!({"name": "Part", "grounded": false}));
    run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 20}));
    run(&mut s, "occurrence.move", json!({"occurrence": "Part:1", "translate": [50, 0, 0]}));
    // The box spans world x 50…90, y 0…30, z 0…20.
    let reply = run(&mut s, "solid.shell", json!({"faces": [[70, 30, 10], [90.1, 15, 10]], "thickness": 2}));
    let used: Vec<&str> = reply["faces_used"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    assert!(used.len() == 2 && used[0].ends_with(":+y") && used[1].ends_with(":+x"), "{reply}");
    // Both faces open: a 38 x 28 x 16 cavity in the 24 000 mm³ box.
    let v = volume(&mut s);
    assert!((v - 6_976.0).abs() < 1e-6, "{v}");
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
}

/// (#297 review) Delete Face and Split Face picks on a moved component, given in the world with
/// the root active, land on the component's body like every other face-taking command's.
#[test]
fn delete_face_and_split_face_picks_land_on_a_moved_component() {
    let mut s = block_in_component();
    run(&mut s, "occurrence.move", json!({"occurrence": "C:1", "translate": [100, 0, 0]}));
    // The block (40 x 30 x 20) spans world x 100…140, y 0…30, z 0…20.
    let reply = run(&mut s, "solid.split_face", json!({"faces": [[120, 15, 20]], "plane": {"origin": [120, 15, 20], "normal": [1, 0, 0]}}));
    assert!(reply["faces_used"][0].as_str().unwrap_or_default().ends_with(":+z"), "{reply}");
    let m = run(&mut s, "inspect.measure", json!({}));
    assert_eq!(m["bodies"][0]["kernel"]["faces"].as_u64(), Some(7), "the top is split in two: {m}");
    let v = volume(&mut s);
    assert!((v - 24_000.0).abs() < 1e-6, "{v}");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "solid.fillet", json!({"edges": [[140, 15, 20]], "radius": 5}));
    assert!(volume(&mut s) < 24_000.0 - 1.0, "the fillet cut material");
    // A point on the fillet's surface: 45° round from its axis (world (135, ·, 15)).
    let h = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
    let reply = run(&mut s, "face.delete", json!({"faces": [[135.0 + h, 15.0, 15.0 + h]]}));
    assert!(reply["faces_used"][0].as_str().is_some_and(|n| !n.is_empty()), "{reply}");
    let v = volume(&mut s);
    assert!((v - 24_000.0).abs() < 1e-3 * 24_000.0, "the fillet is gone and the block healed: {v}");
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
}

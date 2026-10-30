//! Tests for the hard model-quality gate. A pass is structural evidence, not a proof of
//! correctness: these tests cover the cases the gate must refuse.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn constrained_box() -> Session {
    let mut s = Session::default();
    run(&mut s, "parameters.change", json!({"name": "width", "expression": "40 mm"}));
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    let r = run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [38, 29]}));
    let lines: Vec<String> = r["curves"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let sketch = s.doc.sketch(s.active_sketch.unwrap()).unwrap();
    let start = match sketch.curves[0].kind {
        polykrom_sketch::CurveKind::Line { a, .. } => sketch.points[a].id.clone(),
        _ => panic!("rectangle has a line"),
    };
    run(&mut s, "sketch.constraint.coincident", json!({"a": start, "b": "origin"}));
    run(&mut s, "sketch.dimension", json!({"entities": [lines[0]], "value": "width"}));
    run(&mut s, "sketch.dimension", json!({"entities": [lines[1]], "value": 30}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 20}));
    s
}

#[test]
fn a_fully_constrained_parametric_solid_passes() {
    let mut s = constrained_box();
    let q = run(&mut s, "inspect.quality", json!({}));
    assert_eq!(q["verdict"], "pass", "{q}");
    assert!(q["hard_failures"].as_array().unwrap().is_empty(), "{q}");
    assert_eq!(q["counts"]["sketches"], 1, "{q}");
    assert_eq!(q["counts"]["bodies"], 1, "{q}");
    assert!(q["score"].as_i64().unwrap() >= 80, "{q}");
}

#[test]
fn an_under_constrained_sketch_fails() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [40, 30]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 20}));
    let q = run(&mut s, "inspect.quality", json!({}));
    assert_eq!(q["verdict"], "fail", "{q}");
    assert!(q["hard_failures"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("under-constrained")), "{q}");
}

#[test]
fn the_open_surface_and_parametric_warnings_are_explicit() {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [40, 30]}));
    let q = run(&mut s, "inspect.quality", json!({"bodies": []}));
    assert_eq!(q["verdict"], "fail", "{q}");
    assert!(q["warnings"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("no user parameters")), "{q}");
}

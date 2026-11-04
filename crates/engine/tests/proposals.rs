//! Propose and apply (`engine.propose`, `engine.apply_proposal`): an agent shows a change, the
//! person decides. On the sample design (the actuator plate with holes, boss and bore).

use std::sync::Arc;

use polykrom_engine::{EngineError, Session, sample};
use serde_json::{Value, json};

fn sample_session() -> Session {
    let mut s = Session::default();
    s.run_script(&sample::script()).expect("the sample builds");
    s
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn volume(s: &mut Session) -> f64 {
    run(s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().expect("a total volume")
}

fn thicker() -> Value {
    json!({"commands": [{"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}}], "label": "Plate 10 mm thick"})
}

/// The error text of a refusal is a JSON object with a stable `code`.
fn refusal(r: Result<Value, EngineError>) -> Value {
    match r {
        Err(e @ EngineError::Refused(_)) => serde_json::from_str(&e.to_string()).expect("a JSON object"),
        other => panic!("expected a structured refusal, got {other:?}"),
    }
}

#[test]
fn a_proposal_reports_the_change_and_changes_nothing() {
    let mut s = sample_session();
    let (doc, revision, undo, redo, volume0) = (s.doc.clone(), s.revision, s.undo.len(), s.redo.len(), volume(&mut s));
    let selection = s.selection.clone();

    let p = run(&mut s, "engine.propose", thicker());

    // Volume and area of the plate before and after, and the delta: the plate got thicker.
    let plate = p["bodies"]["changed"].as_array().expect("changed bodies").iter().find(|b| b["name"] == "Plate").expect("the plate changed");
    let (before, after, d) = (
        plate["before"]["volume_mm3"].as_f64().unwrap(),
        plate["after"]["volume_mm3"].as_f64().unwrap(),
        plate["delta"]["volume_mm3"].as_f64().unwrap(),
    );
    assert!((before - volume0).abs() < 1e-6, "{before} {volume0}");
    assert!(d > 100.0 && (after - before - d).abs() < 1e-6, "{plate}");
    assert!(plate["delta"]["area_mm2"].as_f64().unwrap().abs() > 1.0, "{plate}");
    assert_eq!(p["totals"]["delta"]["volume_mm3"].as_f64().unwrap(), d);
    // The parameter, and the features it rebuilds.
    assert_eq!(p["parameters"][0]["name"], "thickness");
    assert_eq!(p["parameters"][0]["after"], "10 mm");
    let affected = p["timeline"]["affected"].as_array().unwrap();
    assert!(affected.iter().any(|f| f["type"] == "ExtrudeFeature"), "{}", p["timeline"]);
    assert_eq!(p["replies"].as_array().unwrap().len(), 1);
    assert_eq!(p["label"], "Plate 10 mm thick");
    assert!(p["proposal"].as_str().is_some_and(|id| id.starts_with("prop-")));

    // Nothing in the session moved.
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len(), s.redo.len()), (revision, undo, redo));
    assert_eq!(s.selection, selection);
    assert_eq!(volume(&mut s), volume0);
    assert_eq!(p["revision"], revision);
}

#[test]
fn applying_a_proposal_is_the_proposed_state_in_one_undo_step() {
    let mut s = sample_session();
    let volume0 = volume(&mut s);
    let undo = s.undo.len();
    let p = run(&mut s, "engine.propose", thicker());
    let proposed: f64 = p["totals"]["after"]["volume_mm3"].as_f64().unwrap();
    assert_ne!(proposed, volume0);

    let a = run(&mut s, "engine.apply_proposal", json!({"proposal": p["proposal"]}));
    assert_eq!(a["applied"], true);
    assert_eq!(volume(&mut s), proposed, "the design is exactly the proposed after-state");
    let applied: f64 = a["bodies"].as_array().unwrap().iter().filter_map(|b| b["volume_mm3"].as_f64()).sum();
    assert_eq!(applied, proposed);
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    assert_eq!(s.undo.last().map(|u| u.label.as_str()), Some("Apply Proposal"));

    // A single undo restores the original volumes.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(volume(&mut s), volume0);
    assert_eq!(s.undo.len(), undo);
    // The applied proposal is used up.
    let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": p["proposal"]})));
    assert_eq!(r["code"], "stale_proposal", "{r}");
}

#[test]
fn a_multi_step_proposal_applies_as_one_undo_step() {
    let mut s = sample_session();
    let (volume0, undo) = (volume(&mut s), s.undo.len());
    let steps = json!({"commands": [
        {"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}},
        {"command": "parameters.change", "params": {"name": "width", "expression": "90 mm"}},
    ]});
    let p = run(&mut s, "engine.propose", steps);
    assert_eq!(p["steps"], 2);
    assert_eq!(p["label"], "parameters.change and 1 more");
    run(&mut s, "engine.apply_proposal", json!({"proposal": p["proposal"]}));
    assert_eq!(volume(&mut s), p["totals"]["after"]["volume_mm3"].as_f64().unwrap());
    assert_eq!(s.undo.len(), undo + 1);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(volume(&mut s), volume0);
}

#[test]
fn a_proposal_is_refused_after_any_other_edit() {
    for edit in ["parameter", "undo", "redo-able undo"] {
        let mut s = sample_session();
        let p = run(&mut s, "engine.propose", thicker());
        let id = p["proposal"].clone();
        match edit {
            "parameter" => drop(run(&mut s, "parameters.change", json!({"name": "width", "expression": "81 mm"}))),
            _ => drop(run(&mut s, "edit.undo", json!({}))),
        }
        let (doc, revision, undo, redo) = (s.doc.clone(), s.revision, s.undo.len(), s.redo.len());
        let v = volume(&mut s);

        let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": id})));
        assert_eq!(r["code"], "stale_proposal", "{edit}: {r}");
        assert_eq!(r["proposal"], id);
        assert_eq!(r["made_on_revision"], p["revision"]);
        assert!(r["message"].as_str().unwrap().contains("changed"), "{r}");
        // Nothing changed.
        assert!(Arc::ptr_eq(&doc, &s.doc));
        assert_eq!((s.revision, s.undo.len(), s.redo.len()), (revision, undo, redo));
        assert_eq!(volume(&mut s), v);
    }
}

#[test]
fn proposals_are_bounded_and_unknown_ids_are_refused() {
    let mut s = sample_session();
    let first = run(&mut s, "engine.propose", thicker())["proposal"].clone();
    let mut last = Value::Null;
    for _ in 0..polykrom_engine::cmd::proposal::MAX_PROPOSALS {
        last = run(&mut s, "engine.propose", thicker())["proposal"].clone();
    }
    // The oldest was dropped for the newest.
    let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": first})));
    assert_eq!(r["code"], "stale_proposal");
    let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": "prop-9999"})));
    assert_eq!(r["code"], "unknown_proposal");
    run(&mut s, "engine.apply_proposal", json!({"proposal": last}));
    // Applying changed the design: every other proposal is gone.
    let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": "prop-2"})));
    assert_eq!(r["code"], "stale_proposal");
}

#[test]
fn a_proposal_that_cannot_run_or_is_not_an_edit_holds_nothing() {
    let mut s = sample_session();
    let (doc, revision, undo) = (s.doc.clone(), s.revision, s.undo.len());
    for commands in [
        json!([]),
        json!([{"command": "no.such.command"}]),
        json!([{"command": "file.export", "params": {"path": "/tmp/never-written.step"}}]),
        json!([{"command": "engine.propose", "params": {"commands": []}}]),
        json!([{"command": "inspect.measure"}]),
        json!([{"command": "parameters.change", "params": {"name": "thickness", "expression": "nonsense +"}}]),
    ] {
        assert!(s.execute("engine.propose", &json!({"commands": commands})).is_err(), "{commands}");
    }
    assert!(!std::path::Path::new("/tmp/never-written.step").exists());
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len()), (revision, undo));
    let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": "prop-1"})));
    assert_eq!(r["code"], "unknown_proposal");
}

/// Cancel in the Review panel: the proposal is dropped, nothing in the design moves, and the id
/// can't be applied afterwards.
#[test]
fn dropping_a_proposal_changes_nothing_and_retires_its_id() {
    let mut s = sample_session();
    let (doc, revision, undo, redo, volume0) = (s.doc.clone(), s.revision, s.undo.len(), s.redo.len(), volume(&mut s));
    let p = run(&mut s, "engine.propose", thicker());
    let id = p["proposal"].clone();
    assert_eq!(p["held"], 1);

    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": id}));
    assert_eq!(d["dropped"], true, "{d}");
    assert_eq!(d["held"], 0);
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len(), s.redo.len()), (revision, undo, redo));
    assert_eq!(volume(&mut s), volume0);

    // Dropped: applying it is refused as stale, and dropping it again reports it already gone.
    let r = refusal(s.execute("engine.apply_proposal", &json!({"proposal": id})));
    assert_eq!(r["code"], "stale_proposal", "{r}");
    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": id}));
    assert_eq!(d["dropped"], false, "{d}");
    // An id never held is refused.
    let r = refusal(s.execute("engine.drop_proposal", &json!({"proposal": "prop-9999"})));
    assert_eq!(r["code"], "unknown_proposal", "{r}");
    assert!(s.execute("engine.drop_proposal", &json!({})).is_err(), "the id is required");
}

#[test]
fn a_proposal_can_remove_and_add_bodies() {
    let mut s = sample_session();
    let p = run(
        &mut s,
        "engine.propose",
        json!({"commands": [{"command": "solid.box", "params": {"length": 10, "width": 10, "height": 10, "corner": [200, 0, 0], "operation": "new", "body_name": "Spare"}}]}),
    );
    let created = p["bodies"]["created"].as_array().unwrap();
    assert_eq!(created.len(), 1, "{p}");
    assert_eq!(created[0]["name"], "Spare");
    assert!((created[0]["after"]["volume_mm3"].as_f64().unwrap() - 1000.0).abs() < 1e-6);
    assert_eq!(p["timeline"]["added"].as_array().unwrap().len(), 1);
    assert!(p["bodies"]["changed"].as_array().unwrap().is_empty());
    assert!((p["totals"]["delta"]["volume_mm3"].as_f64().unwrap() - 1000.0).abs() < 1e-6);
}

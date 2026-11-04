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

// --- Candidate sets (engine.propose_candidates, PK-88) ----------------------------------------

/// A 40 × 30 × 10 plate, the design the candidates are proposed on.
fn plate_session() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 10, "body_name": "Plate"}));
    s
}

/// A hole of diameter `d` through the plate's centre, cut into `target`.
fn hole(d: f64, target: &str) -> Value {
    json!({"command": "solid.cylinder", "params": {"diameter": d, "height": 10, "base": [20, 15, 0], "operation": "cut", "targets": [target]}})
}

/// One intent, three candidates: the hole at three diameters.
fn three_holes() -> Value {
    json!({"label": "Centre hole", "candidates": [
        {"label": "Ø4", "commands": [hole(4.0, "Plate")]},
        {"label": "Ø6", "commands": [hole(6.0, "Plate")]},
        {"label": "Ø8", "commands": [hole(8.0, "Plate")]},
    ]})
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("a number, not {v}"))
}

#[test]
fn three_candidates_are_compared_side_by_side_and_match_each_alone() {
    let mut s = plate_session();
    let (doc, revision, undo, redo, volume0) = (s.doc.clone(), s.revision, s.undo.len(), s.redo.len(), volume(&mut s));
    let selection = s.selection.clone();
    let density = run(&mut s, "inspect.measure", json!({}))["bodies"][0]["density_g_cm3"].as_f64().unwrap();

    let r = run(&mut s, "engine.propose_candidates", three_holes());
    assert!(r["set"].as_str().is_some_and(|id| id.starts_with("cset-")), "{r}");
    assert_eq!(r["label"], "Centre hole");
    assert_eq!(r["revision"], revision);
    assert_eq!(r["held"], 3);
    assert_eq!(f(&r["current"]["volume_mm3"]), 12_000.0);
    assert_eq!(r["current"]["bbox"], json!({"min": [0.0, 0.0, 0.0], "max": [40.0, 30.0, 10.0]}));
    let cs = r["candidates"].as_array().expect("candidates");
    assert_eq!(cs.len(), 3);
    let ids: std::collections::BTreeSet<&str> = cs.iter().map(|c| c["proposal"].as_str().expect("a proposal id")).collect();
    assert_eq!(ids.len(), 3, "three distinct ids: {ids:?}");
    assert!(ids.iter().all(|id| id.starts_with("prop-")));
    for (i, (c, d)) in cs.iter().zip([4.0f64, 6.0, 8.0]).enumerate() {
        assert_eq!(c["index"], i);
        assert_eq!(c["label"], format!("Ø{d}"));
        assert_eq!(c["set"], r["set"]);
        assert_eq!(c["valid"], true, "{c}");
        assert_eq!(c["error"], Value::Null);
        assert_eq!((c["errors"].as_array().map(Vec::len), c["warnings"].as_array().map(Vec::len)), (Some(0), Some(0)));
        assert!(f(&c["ms"]) >= 0.0, "{}", c["ms"]);
        assert_eq!(c["steps"], 1);
        assert_eq!(c["replies"].as_array().map(Vec::len), Some(1));
        // The plate with the hole: its volume, mass from the material density, and box.
        let hole_volume = std::f64::consts::PI * (d / 2.0).powi(2) * 10.0;
        let b = &c["bodies"][0];
        assert_eq!(b["name"], "Plate");
        assert_eq!(b["change"], "changed");
        let v = f(&b["volume_mm3"]);
        assert!((v - (12_000.0 - hole_volume)).abs() < 0.5, "Ø{d}: {v} vs {}", 12_000.0 - hole_volume);
        assert!((f(&b["mass_g"]) - v * density / 1000.0).abs() < 1e-9, "{b}");
        assert_eq!(b["material"], "Default");
        let bbox = &b["bbox"];
        for k in 0..3 {
            assert!(f(&bbox["min"][k]).abs() < 1e-6, "{bbox}");
            assert!((f(&bbox["max"][k]) - [40.0, 30.0, 10.0][k]).abs() < 1e-6, "{bbox}");
        }
        assert_eq!(f(&c["totals"]["volume_mm3"]), v);
        assert!((f(&c["totals"]["delta"]["volume_mm3"]) - (v - 12_000.0)).abs() < 1e-9, "{}", c["totals"]);
        assert_eq!(c["totals"]["bbox"], *bbox);
        // The exact IoU with the current design: the plate less the hole, over the plate.
        let iou = f(&c["compare"]["iou"]);
        assert!((iou - (12_000.0 - hole_volume) / 12_000.0).abs() < 2e-3, "Ø{d}: iou {iou}");
        assert_eq!(c["compare"]["identical"], false);
        assert_eq!(c["bodies"][0]["compare"]["iou"], c["compare"]["iou"]);
        assert!((f(&c["compare"]["union_mm3"]) - 12_000.0).abs() < 1e-6, "{}", c["compare"]);
        assert!(f(&c["bodies"][0]["compare"]["only_in_current_mm3"]) > 0.0);
        // The timeline: the cut was added.
        assert_eq!(c["timeline"]["added"].as_array().map(Vec::len), Some(1), "{}", c["timeline"]);
    }
    // A bigger hole leaves less: the candidates order by volume as by diameter.
    let volumes: Vec<f64> = cs.iter().map(|c| f(&c["totals"]["volume_mm3"])).collect();
    assert!(volumes[0] > volumes[1] && volumes[1] > volumes[2], "{volumes:?}");

    // Nothing in the session moved.
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len(), s.redo.len()), (revision, undo, redo));
    assert_eq!(s.selection, selection);
    assert_eq!(volume(&mut s), volume0);

    // Each candidate alone: engine.propose reports the same volume, and inspect.compare on the
    // plate and a copy with that hole the same IoU.
    for (c, d) in cs.iter().zip([4.0f64, 6.0, 8.0]) {
        let alone = run(&mut s, "engine.propose", json!({"commands": [hole(d, "Plate")]}));
        assert!(
            (f(&alone["totals"]["after"]["volume_mm3"]) - f(&c["totals"]["volume_mm3"])).abs() < 1e-9,
            "Ø{d}: {} vs {}",
            alone["totals"],
            c["totals"]
        );
        let mut t = plate_session();
        run(&mut t, "solid.move", json!({"bodies": ["Plate"], "translate": [0, 0, 0], "copy": true}));
        let step = hole(d, "Plate (2)");
        t.execute(step["command"].as_str().unwrap(), &step["params"]).unwrap();
        let cmp = run(&mut t, "inspect.compare", json!({"a": "Plate", "b": "Plate (2)"}));
        assert!((f(&cmp["iou"]) - f(&c["compare"]["iou"])).abs() < 1e-9, "Ø{d}: {} vs {}", cmp["iou"], c["compare"]["iou"]);
        assert!((f(&cmp["b"]["volume_mm3"]) - f(&c["bodies"][0]["volume_mm3"])).abs() < 1e-9);
    }
}

#[test]
fn a_candidate_whose_step_fails_is_invalid_and_the_others_are_still_evaluated() {
    let mut s = plate_session();
    let (doc, revision, undo) = (s.doc.clone(), s.revision, s.undo.len());
    let r = run(
        &mut s,
        "engine.propose_candidates",
        json!({"candidates": [
            {"label": "Ø6", "commands": [hole(6.0, "Plate")]},
            {"label": "negative", "commands": [hole(-1.0, "Plate")]},
            {"label": "no such body", "commands": [hole(6.0, "Nowhere")]},
            {"label": "Ø8", "commands": [hole(8.0, "Plate")]},
        ]}),
    );
    let cs = r["candidates"].as_array().unwrap();
    assert_eq!(cs.len(), 4);
    for i in [1, 2] {
        let c = &cs[i];
        assert_eq!(c["valid"], false, "{c}");
        assert!(c["error"].as_str().is_some_and(|e| !e.is_empty()), "{c}");
        assert_eq!(c["proposal"], Value::Null, "an invalid candidate is not held: {c}");
        assert!(c["bodies"].as_array().is_some_and(Vec::is_empty));
        assert!(f(&c["ms"]) >= 0.0);
    }
    assert!(cs[2]["error"].as_str().unwrap().contains("targets"), "{}", cs[2]["error"]);
    assert!(cs[1]["error"].as_str().unwrap().contains("positive"), "{}", cs[1]["error"]);
    for i in [0, 3] {
        let c = &cs[i];
        assert_eq!(c["valid"], true, "{c}");
        assert!(c["proposal"].as_str().is_some_and(|id| id.starts_with("prop-")), "{c}");
        assert!(f(&c["compare"]["iou"]) < 1.0);
    }
    assert_eq!(r["held"], 2);
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len()), (revision, undo));
    // The valid ones apply; the whole set can be dropped.
    run(&mut s, "engine.apply_proposal", json!({"proposal": cs[3]["proposal"]}));
    assert!((volume(&mut s) - f(&cs[3]["totals"]["volume_mm3"])).abs() < 1e-9);

    // Every candidate failing: the reply still compares them, nothing is held, and the set is gone.
    let mut s = plate_session();
    let r =
        run(&mut s, "engine.propose_candidates", json!({"candidates": [{"commands": [hole(-1.0, "Plate")]}, {"commands": [hole(-2.0, "Plate")]}]}));
    assert_eq!(r["held"], 0);
    assert!(r["candidates"].as_array().unwrap().iter().all(|c| c["valid"] == false && c["proposal"].is_null()), "{r}");
    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": r["set"]}));
    assert_eq!(d["dropped"], false, "{d}");
}

#[test]
fn applying_one_candidate_is_one_undo_step_and_drops_the_others() {
    let mut s = plate_session();
    let (doc0, volume0, undo) = (s.doc.clone(), volume(&mut s), s.undo.len());
    let r = run(&mut s, "engine.propose_candidates", three_holes());
    let cs = r["candidates"].as_array().unwrap().clone();
    let chosen = cs[1]["proposal"].as_str().unwrap().to_string();
    let others: Vec<&str> = [0, 2].iter().map(|&i| cs[i]["proposal"].as_str().unwrap()).collect();

    let a = run(&mut s, "engine.apply_proposal", json!({"proposal": chosen}));
    assert_eq!(a["applied"], true, "{a}");
    assert_eq!(a["label"], "Ø6");
    assert_eq!(a["set"], r["set"]);
    let dropped: Vec<&str> = a["dropped"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(dropped, others, "{a}");
    assert!((volume(&mut s) - f(&cs[1]["totals"]["volume_mm3"])).abs() < 1e-9, "the design is the chosen candidate's after-state");
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    assert_eq!(s.undo.last().map(|u| u.label.as_str()), Some("Apply Proposal"));

    // The other candidates are gone (stale, not unknown), and so is the set.
    for id in &others {
        let e = refusal(s.execute("engine.apply_proposal", &json!({"proposal": id})));
        assert_eq!(e["code"], "stale_proposal", "{e}");
    }
    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": r["set"]}));
    assert_eq!(d["dropped"], false, "{d}");
    let e = refusal(s.execute("engine.apply_proposal", &json!({"proposal": chosen})));
    assert_eq!(e["code"], "stale_proposal", "{e}");

    // A single undo restores the design exactly.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(volume(&mut s), volume0);
    assert_eq!(*s.doc, *doc0, "the design is exactly as it was");
    assert_eq!(s.undo.len(), undo);
}

#[test]
fn dropping_the_set_drops_every_candidate_and_changes_nothing() {
    let mut s = plate_session();
    let (doc, revision, undo, redo, volume0) = (s.doc.clone(), s.revision, s.undo.len(), s.redo.len(), volume(&mut s));
    let r = run(&mut s, "engine.propose_candidates", three_holes());
    let ids: Vec<String> = r["candidates"].as_array().unwrap().iter().map(|c| c["proposal"].as_str().unwrap().to_string()).collect();
    assert_eq!(r["held"], 3);

    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": r["set"]}));
    assert_eq!(d["dropped"], true, "{d}");
    assert_eq!(d["held"], 0);
    assert_eq!(d["proposals"], json!(ids));
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len(), s.redo.len()), (revision, undo, redo));
    assert_eq!(volume(&mut s), volume0);
    for id in &ids {
        let e = refusal(s.execute("engine.apply_proposal", &json!({"proposal": id})));
        assert_eq!(e["code"], "stale_proposal", "{e}");
        assert_eq!(run(&mut s, "engine.drop_proposal", json!({"proposal": id}))["dropped"], false);
    }
    assert_eq!(run(&mut s, "engine.drop_proposal", json!({"proposal": r["set"]}))["dropped"], false);
    let e = refusal(s.execute("engine.drop_proposal", &json!({"proposal": "cset-9999"})));
    assert_eq!(e["code"], "unknown_proposal", "{e}");

    // One candidate can be dropped on its own; the rest of the set stays.
    let r = run(&mut s, "engine.propose_candidates", three_holes());
    let first = r["candidates"][0]["proposal"].clone();
    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": first}));
    assert_eq!((d["dropped"].clone(), d["held"].clone()), (json!(true), json!(2)));
    let d = run(&mut s, "engine.drop_proposal", json!({"proposal": r["set"]}));
    assert_eq!((d["dropped"].clone(), d["held"].clone()), (json!(true), json!(0)));
    assert_eq!(d["proposals"].as_array().map(Vec::len), Some(2));
}

#[test]
fn a_design_change_makes_the_whole_set_stale() {
    for edit in ["edit", "undo"] {
        let mut s = plate_session();
        let r = run(&mut s, "engine.propose_candidates", three_holes());
        match edit {
            "edit" => drop(run(
                &mut s,
                "solid.box",
                json!({"length": 5, "width": 5, "height": 5, "corner": [100, 0, 0], "operation": "new", "body_name": "Spare"}),
            )),
            _ => drop(run(&mut s, "edit.undo", json!({}))),
        }
        let (doc, revision, undo, redo) = (s.doc.clone(), s.revision, s.undo.len(), s.redo.len());
        for c in r["candidates"].as_array().unwrap() {
            let e = refusal(s.execute("engine.apply_proposal", &json!({"proposal": c["proposal"]})));
            assert_eq!(e["code"], "stale_proposal", "{edit}: {e}");
            assert_eq!(e["made_on_revision"], r["revision"]);
        }
        let d = run(&mut s, "engine.drop_proposal", json!({"proposal": r["set"]}));
        assert_eq!(d["dropped"], false, "{edit}: the set is gone, not unknown: {d}");
        assert!(Arc::ptr_eq(&doc, &s.doc));
        assert_eq!((s.revision, s.undo.len(), s.redo.len()), (revision, undo, redo));
    }
}

#[test]
fn candidate_sets_are_checked_before_anything_runs() {
    let mut s = plate_session();
    let (doc, revision, undo) = (s.doc.clone(), s.revision, s.undo.len());
    let one = json!([{"commands": [hole(6.0, "Plate")]}]);
    let nine: Vec<Value> = (0..9).map(|i| json!({"commands": [hole(1.0 + i as f64, "Plate")]})).collect();
    for params in [
        json!({}),
        json!({"candidates": []}),
        json!({"candidates": one}),
        json!({"candidates": nine}),
        json!({"candidates": [{"commands": [hole(6.0, "Plate")]}, {"commands": []}]}),
        json!({"candidates": [{"commands": [hole(6.0, "Plate")]}, {"commands": [{"command": "inspect.measure"}]}]}),
        json!({"candidates": [{"commands": [hole(6.0, "Plate")]}, {"commands": [{"command": "engine.propose", "params": {"commands": []}}]}]}),
        json!({"candidates": [{"commands": [hole(6.0, "Plate")]}, "not an object"]}),
    ] {
        let e = s.execute("engine.propose_candidates", &params).expect_err(&params.to_string());
        assert!(!matches!(e, EngineError::Refused(_)), "{e}");
    }
    // A bad candidate is named.
    let e = s.execute(
        "engine.propose_candidates",
        &json!({"candidates": [{"commands": [hole(6.0, "Plate")]}, {"commands": [{"command": "inspect.measure"}]}]}),
    );
    assert!(e.as_ref().is_err_and(|e| e.to_string().contains("candidate 1")), "{e:?}");
    assert!(Arc::ptr_eq(&doc, &s.doc));
    assert_eq!((s.revision, s.undo.len()), (revision, undo));
    let e = refusal(s.execute("engine.apply_proposal", &json!({"proposal": "prop-1"})));
    assert_eq!(e["code"], "unknown_proposal");
    // A set is not a step of a proposal.
    assert!(s.execute("engine.propose", &json!({"commands": [{"command": "engine.propose_candidates", "params": three_holes()}]})).is_err());
}

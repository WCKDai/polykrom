//! The AI review flow on the sample design: a proposal opens the Review changes panel with its
//! before → after values and dependents; Apply & rebuild makes the design the proposed after-state
//! in one undo step, the receipt's Undo takes it back; Cancel command changes nothing.

use std::sync::Arc;

use serde_json::{Value, json};

use super::Source;
use crate::scenario::{self, Harness};

fn sample() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
    h.frames(3);
    h
}

fn volume(h: &mut Harness) -> f64 {
    h.app.session.execute("inspect.measure", &json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap()
}

/// The plate 10 mm thick instead of 8: a parameter change every extrude of the sample depends on.
fn thicker() -> Vec<(String, Value)> {
    vec![("parameters.change".into(), json!({"name": "thickness", "expression": "10 mm"}))]
}

/// Whether some widget's accessible label contains `text`.
fn shows(h: &Harness, text: &str) -> bool {
    h.output
        .accesskit_update
        .as_ref()
        .is_some_and(|tree| tree.nodes.iter().any(|(_, n)| n.label().into_iter().chain(n.value()).any(|l| l.contains(text))))
}

/// Frames until the proposal's preview is up (it computes off the window thread).
fn wait_for_preview(h: &mut Harness) {
    for _ in 0..400 {
        if h.app.preview.active {
            return;
        }
        h.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("the proposed geometry was not previewed");
}

#[test]
fn apply_and_rebuild_makes_the_proposed_design_and_the_receipts_undo_takes_it_back() {
    let mut h = sample();
    let volume0 = volume(&mut h);
    let (doc0, revision0, undo0) = (h.app.session.doc.clone(), h.app.session.revision, h.app.session.undo.len());
    assert!(h.widget("Preview changes").is_some(), "the prompt row under the ribbon");

    let report = super::propose(&mut h.app, thicker(), Some("Plate 10 mm thick"), Source::Control).unwrap();
    let after = report["totals"]["after"]["volume_mm3"].as_f64().unwrap();
    assert!((after - volume0).abs() > 1.0, "{after} {volume0}");
    let dependents = report["timeline"]["affected"].as_array().unwrap().len();
    assert!(dependents >= 2, "{}", report["timeline"]);
    h.frames(3);

    // The panel, with the before → after value and the dependents count.
    assert!(scenario::rect_of("review:panel").is_some(), "the Review changes panel is open");
    assert!(scenario::rect_of("inspector:card").is_none(), "it stands where Properties stand");
    assert!(h.widget("8 mm → 10 mm").is_some(), "the before → after value");
    assert!(shows(&h, "Plate 10 mm thick"), "the title");
    assert!(shows(&h, &format!("{dependents} dependent features")), "the dependents count");
    assert!(h.widget("Preview ready").is_some(), "the prompt row's button reads Preview ready");
    assert_eq!(super::status_line(&h.app).as_deref(), Some("AI review / thickness / Proposed geometry shown"));
    assert!(scenario::rect_of("review:status").is_some());

    // Nothing in the design moved; the viewport shows the after-state in place of the plate.
    assert!(Arc::ptr_eq(&doc0, &h.app.session.doc));
    assert_eq!((h.app.session.revision, h.app.session.undo.len()), (revision0, undo0));
    assert_eq!(volume(&mut h), volume0);
    wait_for_preview(&mut h);
    assert!(h.app.preview.replaced.iter().any(|b| b == "Plate"), "{:?}", h.app.preview.replaced);

    // Apply & rebuild.
    h.step(&json!({"click": {"handle": "review:apply"}})).unwrap();
    h.frames(3);
    assert!(h.app.review.open.is_none());
    assert!(scenario::rect_of("review:panel").is_none(), "the panel returns to Properties");
    assert_eq!(volume(&mut h), after, "the design is the proposal's after-state");
    assert_eq!(h.app.session.undo.len(), undo0 + 1, "one undo step");
    assert!(!h.app.preview.active, "the preview is gone with the proposal");
    assert_eq!(super::status_line(&h.app).as_deref(), Some("Model updated / Rebuild complete"));
    assert!(scenario::rect_of("toast:review").is_some(), "the receipt toast");
    assert_eq!(super::status(&h.app)["receipt"]["text"], "thickness updated / 10 mm", "the receipt's text");
    assert!(h.widget("Undo").is_some(), "the receipt's Undo button");

    // The toast's Undo undoes exactly that step.
    h.step(&json!({"click": {"handle": "dialog:button:Undo"}})).unwrap();
    h.frames(3);
    assert_eq!(volume(&mut h), volume0, "the original volume is back");
    assert_eq!(h.app.session.undo.len(), undo0);
    assert!(h.app.review.receipt.is_none());
    assert!(scenario::rect_of("toast:review").is_none());
}

#[test]
fn cancel_command_drops_the_proposal_and_changes_nothing() {
    let mut h = sample();
    let volume0 = volume(&mut h);
    let (doc0, revision0, undo0) = (h.app.session.doc.clone(), h.app.session.revision, h.app.session.undo.len());
    let report = super::propose(&mut h.app, thicker(), None, Source::Control).unwrap();
    let id = report["proposal"].as_str().unwrap().to_string();
    h.frames(3);
    assert!(scenario::rect_of("review:panel").is_some());
    assert!(h.widget("Change Parameters").is_some(), "a proposal without a label is titled after its command");

    h.step(&json!({"click": {"handle": "review:cancel"}})).unwrap();
    h.frames(3);
    assert!(h.app.review.open.is_none());
    assert!(scenario::rect_of("review:panel").is_none());
    assert!(Arc::ptr_eq(&doc0, &h.app.session.doc), "the document is unchanged");
    assert_eq!((h.app.session.revision, h.app.session.undo.len()), (revision0, undo0));
    assert_eq!(volume(&mut h), volume0);
    assert!(h.app.review.receipt.is_none(), "no receipt: nothing was applied");
    // The proposal is gone from the engine too.
    let r = h.app.session.execute("engine.apply_proposal", &json!({"proposal": id}));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn a_proposal_the_design_moved_past_is_dropped() {
    let mut h = sample();
    super::propose(&mut h.app, thicker(), None, Source::Control).unwrap();
    h.frames(2);
    assert!(h.app.review.open.is_some());
    h.app.run("parameters.change", json!({"name": "width", "expression": "81 mm"})).unwrap();
    h.frames(2);
    assert!(h.app.review.open.is_none(), "stale: dropped");
    assert!(scenario::rect_of("review:panel").is_none());
    assert!(h.app.status.as_ref().is_some_and(|(s, _, _)| s.contains("dropped")), "{:?}", h.app.status);
}

#[test]
fn the_control_channel_drives_the_flow() {
    let mut h = sample();
    let volume0 = volume(&mut h);
    let steps = json!([{"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}}]);
    let r = h.call("ui.review", json!({"prompt": "Make the plate 10 mm thick", "propose": {"commands": steps, "label": "Plate 10 mm thick"}}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["result"]["open"]["label"], "Plate 10 mm thick");
    assert_eq!(r["result"]["open"]["target"], "thickness");
    assert_eq!(r["result"]["prompt"], "Make the plate 10 mm thick");
    assert!(r["result"]["open"]["dependents"].as_array().is_some_and(|d| !d.is_empty()));
    let after = r["result"]["open"]["totals"]["after"]["volume_mm3"].as_f64().unwrap();
    assert_eq!(volume(&mut h), volume0);
    let inspect = h.call("ui.inspect", json!({}));
    assert_eq!(inspect["result"]["review"]["open"]["label"], "Plate 10 mm thick");

    let r = h.call("ui.review", json!({"apply": true}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r["result"]["open"].is_null());
    assert_eq!(r["result"]["receipt"]["text"], "thickness updated / 10 mm");
    assert_eq!(volume(&mut h), after);
    let r = h.call("ui.review", json!({"undo": true}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(volume(&mut h), volume0);
    // Nothing open: cancel says so.
    let r = h.call("ui.review", json!({"cancel": true}));
    assert_eq!(r["ok"], false);
}

#[test]
fn preview_changes_without_an_agent_says_so_and_proposes_nothing() {
    let mut h = sample();
    let (doc0, revision0) = (h.app.session.doc.clone(), h.app.session.revision);
    let r = h.call("ui.review", json!({"prompt": "Make the plate 10 mm thick", "preview": true}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r["result"]["open"].is_null());
    assert!(r["result"]["waiting"].is_null(), "no agent took the prompt");
    assert!(h.app.status.as_ref().is_some_and(|(_, _, error)| *error), "{:?}", h.app.status);
    assert!(Arc::ptr_eq(&doc0, &h.app.session.doc));
    assert_eq!(h.app.session.revision, revision0);
}

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
    assert_eq!(super::status_line(&h.app).as_deref(), Some("thickness updated / 10 mm"), "the status bar: what was updated");
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

// ---- Design kit views 04 and 05: the actuator mount's mounting holes ----------------------------

/// The kit's actuator mount: a 120 × 80 × 8 base, a Ø44 boss with a Ø24 bore, four Ø8 mounting
/// holes at the points of a sketch on an 84 × 44 pitch, and an R2 edge fillet after them.
fn actuator_mount() -> Value {
    json!({"commands": [
        {"command": "sketch.create", "params": {"plane": "XY", "name": "Base profile"}},
        {"command": "sketch.rectangle.two_point", "params": {"p0": [-60, -40], "p1": [60, 40]}},
        {"command": "sketch.finish", "params": {}},
        {"command": "solid.extrude", "params": {"distance": 8, "body_name": "Mount", "name": "Base"}},
        {"command": "sketch.create", "params": {"plane": "XY", "offset": 8, "name": "Boss profile"}},
        {"command": "sketch.circle.center", "params": {"center": [0, 0], "diameter": 44}},
        {"command": "sketch.finish", "params": {}},
        {"command": "solid.extrude", "params": {"distance": 14, "operation": "join", "name": "Boss"}},
        {"command": "solid.hole", "params": {"position": [0, 0, 22], "diameter": 24, "name": "Bore"}},
        {"command": "sketch.create", "params": {"plane": "XY", "offset": 8, "name": "Mounting pattern"}},
        {"command": "sketch.point", "params": {"point": [-42, -22]}},
        {"command": "sketch.point", "params": {"point": [42, -22]}},
        {"command": "sketch.point", "params": {"point": [42, 22]}},
        {"command": "sketch.point", "params": {"point": [-42, 22]}},
        {"command": "sketch.finish", "params": {}},
        {"command": "solid.hole", "params": {"sketch": "Mounting pattern", "diameter": 8, "name": "Mounting holes"}},
        {"command": "solid.fillet", "params": {"edges": [[-60, -40, 4], [60, -40, 4], [60, 40, 4], [-60, 40, 4]], "radius": "2 mm", "name": "Edge finish"}},
    ]})
}

fn mount() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.session.run_script(&actuator_mount()).unwrap();
    h.frames(3);
    h
}

fn feature_id(h: &Harness, name: &str) -> u64 {
    h.app.session.doc.find_feature(name).unwrap_or_else(|| panic!("no feature {name}")).id
}

/// The resized holes: the kit's AI request, as one `timeline.edit` of the hole feature.
fn resize_holes() -> Vec<(String, Value)> {
    vec![("timeline.edit".into(), json!({"feature": "Mounting holes", "set": {"diameter": "10 mm"}}))]
}

#[test]
fn a_hole_patterns_values_come_from_the_model_with_its_pitch_and_dependent_fillet_and_apply_says_what_changed() {
    let mut h = mount();
    let holes = feature_id(&h, "Mounting holes");
    h.app.session.selection = vec![polykrom_engine::Sel::Feature { id: holes }];
    h.frames(2);
    assert_eq!(super::scope(&h.app), "Mounting holes / 4 holes", "the scope chip, from the selection");
    assert!(h.widget("Scope: Mounting holes / 4 holes").is_some(), "the chip is a widget");
    let volume0 = volume(&mut h);
    let undo0 = h.app.session.undo.len();

    super::propose(&mut h.app, resize_holes(), Some("Resize mounting holes"), Source::Control).unwrap();
    h.frames(3);
    let open = h.app.review.open.clone().expect("open");
    let d = &open.described;
    // Every row is read from the design before and after: the diameter changes, the extent, the
    // pitch of the sketch points and the instances are kept.
    assert_eq!(
        d.changes,
        vec![super::Row { feature: "Mounting holes".into(), label: "Diameter".into(), before: Some("Ø8.00 mm".into()), after: "Ø10.00 mm".into() }]
    );
    let kept: Vec<(&str, &str)> = d.kept.iter().map(|r| (r.label.as_str(), r.after.as_str())).collect();
    assert_eq!(kept, vec![("Extent", "Through all"), ("Pitch X", "84.00 mm"), ("Pitch Y", "44.00 mm")]);
    assert_eq!(d.instances, Some(4));
    assert_eq!(d.pattern_preserved, Some(true));
    assert_eq!(super::dependents(&open), vec!["Edge finish"], "the fillet after the holes rebuilds");
    // The card.
    assert!(shows(&h, "MODEL COMMAND"));
    assert!(shows(&h, "Resize mounting holes"));
    assert!(shows(&h, "Mounting holes / 4 instances"), "the subtitle");
    assert!(h.widget("Ø8.00 mm → Ø10.00 mm").is_some(), "old → new in the value cell");
    assert!(h.widget("84.00 mm").is_some() && h.widget("Through all").is_some(), "the kept values");
    assert!(shows(&h, "1 feature / 4 holes\n1 dependent feature / Edge finish\nPattern preserved"), "scope & dependencies");
    assert!(shows(&h, "References are preserved. Geometry and downstream features rebuild on apply."));
    assert!(shows(&h, "No model changes applied yet"));
    assert_eq!(super::status_line(&h.app).as_deref(), Some("AI review / Mounting holes / Proposed geometry shown"));
    assert!(h.widget("Preview ready").is_some());
    let s = super::status(&h.app);
    assert_eq!(s["state"], "ready");
    assert_eq!(s["open"]["changes"][0]["text"], "Ø8.00 mm → Ø10.00 mm");
    assert_eq!(s["open"]["kept"][1]["label"], "Pitch X");
    wait_for_preview(&mut h);
    assert!(h.app.preview.replaced.iter().any(|b| b == "Mount"), "{:?}", h.app.preview.replaced);

    // Apply & rebuild: exactly the previewed command, one undo step; the toast and the status.
    h.step(&json!({"click": {"handle": "review:apply"}})).unwrap();
    h.frames(3);
    assert_eq!(h.app.session.undo.len(), undo0 + 1);
    let f = h.app.session.doc.find_feature("Mounting holes").unwrap();
    assert!(matches!(&f.kind, polykrom_engine::doc::FeatureKind::Hole { diameter, .. } if diameter.as_str() == "10 mm"), "{:?}", f.kind);
    assert!(volume(&mut h) < volume0, "bigger holes, less material");
    assert_eq!(super::status(&h.app)["receipt"]["text"], "4 mounting holes updated / Ø10.00 mm");
    assert_eq!(super::status_line(&h.app).as_deref(), Some("4 mounting holes updated / Ø10.00 mm / Pattern preserved"));
    assert!(scenario::rect_of("toast:review").is_some());
    assert!(scenario::rect_of("review:panel").is_none(), "the right-hand area is free for the properties");
    // The properties' footer, on the body's card.
    h.app.session.selection = vec![polykrom_engine::Sel::Body { name: "Mount".into() }];
    h.frames(2);
    assert!(scenario::rect_of("inspector:card").is_some());
    assert!(h.widget("Model updated / Rebuild complete").is_some(), "the properties footer");
    // Undo undoes exactly that step.
    h.step(&json!({"click": {"handle": "dialog:button:Undo"}})).unwrap();
    h.frames(3);
    assert_eq!(h.app.session.undo.len(), undo0);
    assert_eq!(volume(&mut h), volume0);
    let f = h.app.session.doc.find_feature("Mounting holes").unwrap();
    assert!(matches!(&f.kind, polykrom_engine::doc::FeatureKind::Hole { diameter, .. } if diameter.as_str() == "8"), "{:?}", f.kind);
}

#[test]
fn cancel_leaves_the_hole_pattern_its_revision_and_its_history_as_they_were() {
    let mut h = mount();
    let (doc0, revision0, undo0, redo0) = (h.app.session.doc.clone(), h.app.session.revision, h.app.session.undo.len(), h.app.session.redo.len());
    let volume0 = volume(&mut h);
    super::propose(&mut h.app, resize_holes(), Some("Resize mounting holes"), Source::Control).unwrap();
    h.frames(3);
    h.step(&json!({"click": {"handle": "review:cancel"}})).unwrap();
    h.frames(3);
    assert!(h.app.review.open.is_none() && h.app.review.receipt.is_none());
    assert!(Arc::ptr_eq(&doc0, &h.app.session.doc), "the document is the same object");
    assert_eq!((h.app.session.revision, h.app.session.undo.len(), h.app.session.redo.len()), (revision0, undo0, redo0));
    assert_eq!(volume(&mut h), volume0);
    assert_eq!(super::status_line(&h.app), None);
}

/// Each value keeps its kind: a length in the design's units, a count bare, an angle in degrees;
/// nothing is scaled by the length unit that is not a length.
#[test]
fn values_keep_their_kinds_in_an_inch_design() {
    let mut h = Harness::new();
    h.app.home.open = false;
    for (c, p) in [
        ("document.units", json!({"units": "in"})),
        // Lengths with their unit: a bare number in an inch design is inches.
        ("solid.box", json!({"length": "60 mm", "width": "60 mm", "height": "10 mm", "corner": [0, 0, 0]})),
        ("solid.hole", json!({"position": [10, 30, 10], "diameter": "8 mm", "depth": "6 mm", "name": "Pin"})),
        (
            "solid.pattern.circular",
            json!({"features": ["Pin"], "axis": {"origin": [30, 30, 0], "dir": [0, 0, 1]}, "count": 3, "angle": "360 deg", "name": "Ring"}),
        ),
    ] {
        h.app.run(c, p).unwrap();
    }
    h.frames(2);
    let commands = vec![
        ("timeline.edit".to_string(), json!({"feature": "Pin", "set": {"diameter": "10 mm"}})),
        ("timeline.edit".to_string(), json!({"feature": "Ring", "set": {"pattern.count": "4"}})),
    ];
    super::propose(&mut h.app, commands, Some("Bigger pins, one more"), Source::Control).unwrap();
    let d = h.app.review.open.as_ref().unwrap().described.clone();
    let rows: Vec<(String, String)> = d.changes.iter().chain(&d.kept).map(|r| (r.label.clone(), r.text())).collect();
    assert_eq!(
        rows,
        vec![
            ("Diameter".to_string(), "Ø0.315 in → Ø0.3937 in".to_string()),
            ("Quantity".to_string(), "3 → 4".to_string()),
            ("Depth".to_string(), "0.2362 in".to_string()),
            ("Total angle".to_string(), "360 deg".to_string()),
        ]
    );
    assert_eq!(d.pattern_preserved, Some(false), "one more copy: the pattern changes");
    assert_eq!(d.instances, None, "a single hole has no instances; the pattern is a dependent of its own");
    h.frames(2);
    assert!(h.widget("3 → 4").is_some());
    assert!(h.widget("360 deg").is_some());
    assert!(shows(&h, "Pattern changed"));
}

// ---- The prompt row's states and keys ----------------------------------------------------------

/// An agent set up, as a person who installed one has (none runs here).
fn with_agent(h: &mut Harness) {
    let r = h.call("ui.agent_chat", json!({"add_agent": {"id": "test-agent", "name": "Test agent", "kind": "acp", "command": "true"}}));
    assert_eq!(r["ok"], true, "{r}");
    h.frames(2);
}

#[test]
fn the_prompt_row_is_blocked_with_its_reason_pending_while_the_agent_works_and_ready_with_a_proposal() {
    use super::PromptState;
    let mut h = mount();
    // No agent set up: blocked, and the reason is a widget of its own.
    h.call("ui.review", json!({"prompt": "Increase the mounting holes to Ø10. Keep the 84 × 44 pattern."}));
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Blocked("No agent found: add one in the AI settings".into()));
    assert!(h.widget("No agent found: add one in the AI settings").is_some(), "the reason is read by assistive technology");
    assert!(scenario::rect_of("review:blocked").is_some());
    assert_eq!(super::status(&h.app)["blocked"], "No agent found: add one in the AI settings");
    assert!(h.widget("Preview changes").is_some());
    assert!(h.widget("Increase the mounting holes to Ø10. Keep the 84 × 44 pattern.").is_some(), "the request stays visible");
    // With an agent: idle, and the row can send.
    with_agent(&mut h);
    assert_eq!(super::prompt_state(&h.app), PromptState::Idle);
    assert!(scenario::rect_of("review:blocked").is_none());
    // A sketch being edited blocks it: a proposal is made on the model.
    let sketch = feature_id(&h, "Mounting pattern");
    h.app.run("sketch.edit", json!({"sketch": sketch})).unwrap();
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Blocked("Finish the sketch first: a change is proposed on the model".into()));
    let r = h.call("ui.review", json!({"preview": true}));
    assert_eq!(r["result"]["state"], "blocked", "{r}");
    assert!(r["result"]["waiting"].is_null());
    assert!(h.app.status.as_ref().is_some_and(|(s, _, error)| *error && s.contains("Finish the sketch")), "{:?}", h.app.status);
    h.app.finish_sketch();
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Idle);
    // Pending: the request is with the agent, which is working on it.
    h.app.review.waiting = Some("Increase the mounting holes to Ø10.".into());
    h.app.agent_chat.state = Some(crate::agent_chat::State::Busy);
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Pending);
    assert!(h.widget("Preview pending").is_some());
    assert_eq!(super::status(&h.app)["state"], "pending");
    // The agent gone (stopped, exited) while a request waits: the row is free again.
    h.app.agent_chat.state = Some(crate::agent_chat::State::Exited);
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Idle);
    assert!(h.app.status.as_ref().is_some_and(|(s, _, _)| s.contains("without a change")), "{:?}", h.app.status);
    h.app.review.waiting = Some("Increase the mounting holes to Ø10.".into());
    h.app.agent_chat.state = Some(crate::agent_chat::State::Busy);
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Pending);
    // Ready: the proposal is in.
    super::propose(&mut h.app, resize_holes(), Some("Resize mounting holes"), Source::Prompt).unwrap();
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), PromptState::Ready);
    assert!(h.app.review.waiting.is_none());
    assert_eq!(h.app.review.open.as_ref().and_then(|o| o.request.clone()).as_deref(), Some("Increase the mounting holes to Ø10."));
    // Applied: the request is done, the row is empty for the next one (view 05).
    h.app.agent_chat.state = Some(crate::agent_chat::State::Ready);
    h.call("ui.review", json!({"apply": true}));
    h.frames(2);
    assert_eq!(h.app.review.prompt, "");
    assert_eq!(super::prompt_state(&h.app), PromptState::Idle);
}

#[test]
fn the_keyboard_reaches_the_prompt_and_the_scope_is_a_choice() {
    let mut h = mount();
    with_agent(&mut h);
    // Ctrl+J (Cmd+J) puts the keyboard in the prompt; Enter sends it.
    h.step(&json!({"key": "J", "cmd": true})).unwrap();
    h.frames(2);
    h.step(&json!({"text": "Make the holes 10 mm"})).unwrap();
    h.frames(2);
    assert_eq!(h.app.review.prompt, "Make the holes 10 mm");
    assert!(h.widget("Ctrl+J").is_some() || h.widget("Cmd+J").is_some(), "the key is shown beside the scope");
    let sent = h.app.agent_chat.items.len();
    h.step(&json!({"key": "Enter"})).unwrap();
    h.frames(2);
    // The message went to the chat with the scope (no agent runs here, so it waits to start).
    assert!(h.app.agent_chat.items.len() > sent || h.app.review.waiting.is_some(), "{:?}", h.app.status);
    // The scope: from the selection, or chosen.
    h.app.review.waiting = None;
    assert_eq!(super::scope(&h.app), "Whole design");
    let holes = feature_id(&h, "Mounting holes");
    h.app.session.selection = vec![polykrom_engine::Sel::Feature { id: holes }];
    assert_eq!(super::scope(&h.app), "Mounting holes / 4 holes");
    let r = h.call("ui.review", json!({"scope": "design"}));
    assert_eq!(r["result"]["scope"], "Whole design", "{r}");
    let r = h.call("ui.review", json!({"scope": {"feature": "Edge finish"}}));
    assert_eq!(r["result"]["scope"], "Edge finish");
    let r = h.call("ui.review", json!({"scope": {"body": "Mount"}}));
    assert_eq!(r["result"]["scope"], "Mount");
    let r = h.call("ui.review", json!({"scope": {"feature": "Nowhere"}}));
    assert_eq!(r["ok"], false);
    // The chip's menu offers them.
    h.frames(2);
    h.step(&json!({"click": {"handle": "review:scope"}})).unwrap();
    h.frames(2);
    assert!(h.widget("Mounting holes / 4 holes").is_some(), "a feature in the menu");
    h.step(&json!({"click": {"handle": "review:scope:Whole design"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.review.scope, super::Scope::Design);
    assert_eq!(super::scope(&h.app), "Whole design");
    // Applying a proposal returns the chip to the selection.
    super::propose(&mut h.app, resize_holes(), None, Source::Control).unwrap();
    super::apply(&mut h.app).unwrap();
    assert_eq!(h.app.review.scope, super::Scope::Selection);
}

// ---- The card's geometry (PK-90.ai-flow-r2) ---------------------------------------------------

/// The window sizes the card is checked at.
const WINDOWS: [(f64, f64); 2] = [(1600.0, 1000.0), (1366.0, 768.0)];

/// The text shapes one more frame paints at the harness's window size, with whether each was
/// cut short with an ellipsis.
fn painted_texts(h: &mut Harness, size: (f64, f64)) -> Vec<(egui::Rect, String, bool)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(egui::Rect, String, bool)>) {
        match shape {
            egui::Shape::Text(t) => out.push((shape.visual_bounding_rect(), t.galley.text().to_string(), t.galley.elided)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size.0 as f32, size.1 as f32));
    let mut out = h.ctx.run_ui(egui::RawInput { screen_rect: Some(screen), focused: true, ..Default::default() }, |ui| {
        h.app.logic(ui.ctx());
        h.app.ui(ui);
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    out.shapes.iter().for_each(|s| walk(&s.shape, &mut texts));
    texts
}

/// Every value of the card is painted whole, with its unit, at both window sizes in every
/// language: a pair too wide for its column stacks before and after instead of ending in an
/// ellipsis, and a body's volume reads `… mm³ → … mm³`.
#[test]
fn no_review_card_value_is_cut_short_at_either_window_size_in_any_language() {
    for (w, hgt) in WINDOWS {
        for language in ["en", "ja", "zh"] {
            let mut h = mount();
            h.call("ui.resize", json!({"width": w, "height": hgt}));
            h.call("ui.prefs", json!({"language": language}));
            h.frames(2);
            super::propose(&mut h.app, resize_holes(), Some("Resize mounting holes"), Source::Control).unwrap();
            h.frames(3);
            let open = h.app.review.open.clone().unwrap();
            let mut expected: Vec<String> = open.described.changes.iter().chain(&open.described.kept).map(super::Row::text).collect();
            let body = &open.report["bodies"]["changed"][0];
            let (before, after) =
                super::volume_pair(&h.app, body["before"]["volume_mm3"].as_f64().unwrap(), body["after"]["volume_mm3"].as_f64().unwrap());
            assert!(before.ends_with(" mm³") && after.ends_with(" mm³"), "{language} {w}×{hgt}: volumes in the design's units: {before} → {after}");
            expected.push(format!("{before} → {after}"));
            assert!(expected.iter().any(|t| t.contains("Ø8.00 mm → Ø10.00 mm")), "{expected:?}");
            let panel = scenario::rect_of("review:panel").expect("the card");
            let texts = painted_texts(&mut h, (w, hgt));
            let in_card: Vec<&(egui::Rect, String, bool)> = texts.iter().filter(|(r, _, _)| panel.contains_rect(*r)).collect();
            for value in &expected {
                let painted: Vec<&&(egui::Rect, String, bool)> = in_card.iter().filter(|(_, text, _)| text.replace('\n', " ") == *value).collect();
                assert!(!painted.is_empty(), "{language} {w}×{hgt}: `{value}` is not painted whole in the card: {in_card:?}");
                assert!(painted.iter().all(|(_, _, elided)| !elided), "{language} {w}×{hgt}: `{value}` is cut short");
            }
            // A value takes at most two lines, and keeps to its column.
            for (rect, text, _) in &in_card {
                if expected.iter().any(|v| text.replace('\n', " ") == *v) {
                    assert!(text.matches('\n').count() <= 1, "{language} {w}×{hgt}: `{text}` on more than two lines");
                    assert!(rect.right() <= panel.right(), "{language} {w}×{hgt}: `{text}` runs out of the card");
                }
            }
            h.call("ui.prefs", json!({"language": "en"}));
        }
    }
}

/// The card stands under the prompt row and below the view cube, so the cube and its home button
/// stay visible and clickable (the covered rects leave them out), at both window sizes.
#[test]
fn the_review_card_leaves_the_view_cube_and_its_home_button_clear() {
    for (w, hgt) in WINDOWS {
        let mut h = mount();
        h.call("ui.resize", json!({"width": w, "height": hgt}));
        h.frames(2);
        super::propose(&mut h.app, resize_holes(), Some("Resize mounting holes"), Source::Control).unwrap();
        h.frames(3);
        let panel = scenario::rect_of("review:panel").expect("the card");
        let cube = scenario::rect_of("axis_view:outer").expect("the view cube");
        let home = scenario::rect_of("axis_view:home").expect("the home button");
        let row = scenario::rect_of("review:prompt_row").expect("the prompt row");
        assert!(!panel.intersect(cube).is_positive(), "{w}×{hgt}: the card {panel:?} covers the view cube {cube:?}");
        assert!(!panel.intersect(home).is_positive(), "{w}×{hgt}: the card {panel:?} covers the home button {home:?}");
        assert!(!crate::shell::covered(&h.app, home.center()), "{w}×{hgt}: the home button is clickable");
        assert!(!crate::shell::covered(&h.app, cube.center()), "{w}×{hgt}: the cube is clickable");
        assert!(panel.top() >= cube.bottom() && panel.top() > row.bottom(), "{w}×{hgt}: below the cube and the row: {panel:?}");
        assert!((panel.right() - row.right()).abs() <= 1.0, "{w}×{hgt}: the same right edge as the row: {panel:?} {row:?}");
        assert!(crate::shell::covered(&h.app, panel.center()), "the card shields the viewport");
    }
}

/// Pending: the spinner is an item of the row with the row's spacing on both sides, the key chip
/// (its border included) before it, the button after it.
#[test]
fn the_pending_spinner_keeps_the_rows_item_spacing() {
    let mut h = mount();
    with_agent(&mut h);
    h.app.review.waiting = Some("Increase the mounting holes to Ø10.".into());
    h.app.agent_chat.state = Some(crate::agent_chat::State::Busy);
    h.frames(2);
    assert_eq!(super::prompt_state(&h.app), super::PromptState::Pending);
    let t = crate::theme::Tokens::get();
    let scope = scenario::rect_of("review:scope").expect("the scope chip");
    let key = scenario::rect_of("review:key").expect("the key chip");
    let spinner = scenario::rect_of("review:spinner").expect("the spinner");
    let button = scenario::rect_of("review:preview").expect("the button");
    let gaps = [key.left() - scope.right(), spinner.left() - key.right(), button.left() - spinner.right()];
    assert!(gaps.iter().all(|g| (g - t.space_12).abs() <= 1.0), "the row's item spacing around the spinner: {gaps:?}");
    assert!(h.widget("Ctrl+J").is_some() || h.widget("Cmd+J").is_some(), "the key is still read by assistive technology");
}

/// Ctrl+J is the window's: the keymap refuses it for a command, as the shortcut would never fire.
#[test]
fn the_prompt_rows_key_is_reserved_in_the_keymap() {
    let mut h = mount();
    assert!(crate::keymap::RESERVED.contains(&super::FOCUS_KEY));
    assert_eq!(crate::keymap::key_name(egui::Key::J, egui::Modifiers::COMMAND), super::FOCUS_KEY);
    let r = crate::keymap::bind(&mut h.app, "solid.box", super::FOCUS_KEY, true);
    assert_eq!(r, Err("Ctrl+J is used by Polykrom itself".into()));
    assert!(h.app.keymap.custom.is_empty(), "nothing was bound");
}

/// Ctrl+J while the prompt row is hidden (the Render workspace) asks nothing: the request is
/// dropped, so the row does not take the keyboard later, unasked, when it shows again.
#[test]
fn a_focus_request_while_the_row_is_hidden_is_dropped() {
    let mut h = mount();
    with_agent(&mut h);
    crate::render_workspace::enter(&mut h.app, true).unwrap();
    h.frames(2);
    assert!(scenario::rect_of("review:prompt_row").is_none(), "the row is hidden in Render");
    h.step(&json!({"key": "J", "cmd": true})).unwrap();
    h.frames(2);
    assert!(!h.app.review.focus, "the request does not wait for the row");
    crate::render_workspace::enter(&mut h.app, false).unwrap();
    h.frames(3);
    assert!(scenario::rect_of("review:prompt_row").is_some(), "the row is back");
    assert!(!h.ctx.egui_wants_keyboard_input(), "the prompt did not take the keyboard unasked");
    // Asked while the row shows, the keyboard goes to the prompt.
    h.step(&json!({"key": "J", "cmd": true})).unwrap();
    h.frames(2);
    assert!(h.ctx.egui_wants_keyboard_input());
}

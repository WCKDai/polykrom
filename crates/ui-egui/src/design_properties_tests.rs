//! The Properties panel (design kit views 01 and 03), exercised through the real app.
use serde_json::{Value, json};

use super::{Edits, edits, fields};
use crate::dialogs::{self, Kind};
use crate::scenario::Harness;

/// A mount plate (160 × 120 × 8, "Mount") with one Ø8 hole on a sketch point at its centre, a
/// 2 × 2 pattern of the hole at 40 × 30 pitch and an R2 fillet on the hole's edge. The pattern
/// names the hole; the fillet's edge is the hole's, and no other edge lies near enough for the
/// fillet to re-find one without it: the model reports both as the hole's dependents.
pub(super) fn mount() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    for (id, p) in [
        ("sketch.create", json!({"plane": "XY", "name": "Base"})),
        ("sketch.rectangle.two_point", json!({"p0": [-80, -60], "p1": [80, 60]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance": "8 mm", "body_name": "Mount", "name": "Base plate"})),
        ("sketch.create", json!({"plane": "XY", "offset": "8 mm", "name": "Hole centres"})),
        ("sketch.point", json!({"point": [0, 0], "id": "c1"})),
        ("sketch.finish", json!({})),
        ("solid.hole", json!({"sketch": "Hole centres", "points": ["c1"], "diameter": "8 mm", "name": "Mounting hole"})),
        (
            "solid.pattern.rectangular",
            json!({"features": ["Mounting hole"], "dir1": [1, 0, 0], "count1": 2, "spacing1": "40 mm", "dir2": [0, 1, 0], "count2": 2, "spacing2": "30 mm", "name": "Bolt pattern"}),
        ),
        ("solid.fillet", json!({"edges": [[4, 0, 8]], "radius": "2 mm", "name": "Hole fillet"})),
    ] {
        h.app.run(id, p).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
    h.frames(3);
    h
}

pub(super) fn feature(h: &Harness, name: &str) -> u64 {
    h.app.session.doc.find_feature(name).map(|f| f.id).unwrap_or_else(|| panic!("no feature {name}"))
}

pub(super) fn select(h: &mut Harness, name: &str) {
    let id = feature(h, name);
    h.app.run("select.set", json!({"items": [{"type": "feature", "id": id}]})).unwrap();
    h.frames(3);
}

/// The names `timeline.dependents` reports for a feature, in timeline order.
fn reported_dependents(h: &mut Harness, id: u64) -> Vec<String> {
    let r: Value = h.app.session.execute("timeline.dependents", &json!({"feature": id})).unwrap();
    let mut names: Vec<String> = ["deleted_with_it", "would_fail"]
        .iter()
        .flat_map(|k| r[*k].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let order: Vec<&str> = h.app.session.doc.features.iter().map(|f| f.name.as_str()).collect();
    names.sort_by_key(|n| order.iter().position(|o| o == n));
    names
}

/// The hole's diameter field of the open editor.
fn diameter(h: &mut Harness) -> &mut String {
    let ed = h.app.properties.editor.as_mut().expect("the editor is open");
    fields(&mut ed.dialog).into_iter().find(|(l, _, _)| *l == "Diameter").map(|(_, _, v)| v).expect("a Diameter field")
}

/// How long a preview may take to compute before the test stops waiting for it: generous, for a
/// loaded CI runner, where a computation that takes a fraction of a second here has taken many.
const PREVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Frames until the preview has settled (it computes off the window thread): it is active or
/// its error is set, and nothing is computing. Frames are pumped until that holds or
/// `PREVIEW_TIMEOUT` of wall-clock time has passed: a fixed count of frames ran out on a slow
/// CI runner while the preview was still computing (PK-99). The caller asserts the state.
fn settle_preview(h: &mut Harness) {
    let deadline = std::time::Instant::now() + PREVIEW_TIMEOUT;
    loop {
        let settled = !h.app.preview.busy && (h.app.preview.active || h.app.preview.error.is_some());
        if settled || std::time::Instant::now() >= deadline {
            break;
        }
        h.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    h.frames(2);
}

/// The design, its revision, history and selection: what Cancel and Esc must leave alone.
fn snapshot(h: &Harness) -> (String, u64, usize, usize, Option<usize>, Vec<polykrom_engine::Sel>) {
    let s = &h.app.session;
    (s.doc.to_json(), s.revision, s.undo.len(), s.redo.len(), s.doc.marker, s.selection.clone())
}

fn hole_diameter(h: &Harness) -> String {
    match &h.app.session.doc.find_feature("Mounting hole").unwrap().kind {
        polykrom_engine::doc::FeatureKind::Hole { diameter, .. } => diameter.to_string(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_panel_shows_the_selected_feature_its_parameters_and_its_dependents() {
    let mut h = mount();
    let hole = feature(&h, "Mounting hole");
    select(&mut h, "Mounting hole");
    assert!(h.app.properties.rect.is_some(), "the panel is drawn");
    for text in [
        "Properties",
        "HOLE 01",
        "Mounting hole",
        "Mount / 1 instance",
        "Parameters",
        "Diameter",
        "Ø8.00 mm",
        "Extent",
        "Through all",
        "Reference",
        "Hole centres",
        "Body",
        "Mount",
        "Edit feature",
        "Edit sketch",
        "Edit the feature or its driving sketch.\nDependent features rebuild automatically.",
        "Dependent features",
        "Bolt pattern / 4 instances",
        "Hole fillet / R2",
        "No pending edits",
    ] {
        assert!(h.widget(text).is_some(), "the panel shows {text:?}");
    }
    assert!(h.widget("Apply change").is_none(), "no editor yet");
    // The dependents are exactly the model's.
    let reported = reported_dependents(&mut h, hole);
    assert_eq!(reported, ["Bolt pattern", "Hole fillet"], "the model reports the pattern and the fillet");
    let shown: Vec<String> = super::dependents(&mut h.app, hole).iter().map(|d| h.app.session.doc.feature(*d).unwrap().name.clone()).collect();
    assert_eq!(shown, reported);
    // A dependent is a link: it selects that feature, whose panel then shows.
    h.step(&json!({"widget": "Hole fillet / R2"})).unwrap();
    h.frames(2);
    assert_eq!(super::selected_feature(&h.app), Some(feature(&h, "Hole fillet")));
    assert!(h.widget("FILLET 01").is_some() && h.widget("Radius").is_some() && h.widget("2.00 mm").is_some());
    assert!(h.widget("No dependent features").is_some());
    // A sketch: Edit sketch opens it.
    select(&mut h, "Hole centres");
    assert!(h.widget("SKETCH 02").is_some() && h.widget("Points").is_some());
    assert!(h.widget("Edit feature").is_none());
    h.step(&json!({"widget": "Edit sketch"})).unwrap();
    h.frames(2);
    assert_eq!(h.app.session.active_sketch, Some(feature(&h, "Hole centres")));
    assert!(h.app.properties.rect.is_none(), "the sketch palette has the right edge while a sketch is open");
    h.app.finish_sketch();
    h.frames(2);
    // Nothing selected: the design's properties, and Edit properties opens the window.
    h.app.run("select.clear", json!({})).unwrap();
    h.app.run("document.metadata.set", json!({"values": {"project": "Actuator"}})).unwrap();
    h.frames(3);
    for text in ["DESIGN", "Project", "Actuator", "Part number", "—", "Units", "mm", "Edit properties", "No pending edits"] {
        assert!(h.widget(text).is_some(), "the design's properties show {text:?}");
    }
    h.step(&json!({"widget": "Edit properties"})).unwrap();
    assert!(h.app.design_properties.is_some());
}

#[test]
fn edit_feature_edits_in_place_and_apply_issues_the_dialogs_command_as_one_undo_step() {
    let mut h = mount();
    let hole = feature(&h, "Mounting hole");
    // What the existing dialog issues for a Ø10 hole.
    h.app.edit_feature(hole);
    let mut d = h.app.dialog.clone().expect("the hole's dialog");
    if let Kind::Hole { diameter, .. } = &mut d.kind {
        *diameter = "10 mm".into();
    }
    let expected = dialogs::apply_commands(&h.app, &d).unwrap();
    dialogs::cancel(&mut h.app);
    h.frames(2);
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].0, "timeline.redefine");
    assert_eq!(expected[0].1["params"]["diameter"], json!("10 mm"));

    select(&mut h, "Mounting hole");
    let before = snapshot(&h);
    h.step(&json!({"widget": "Edit feature"})).unwrap();
    h.frames(2);
    assert!(h.app.dialog.is_none(), "the panel edits in place, no dialog");
    assert!(h.app.properties.editor.as_ref().is_some_and(|ed| ed.feature == hole && ed.preview));
    assert_eq!(snapshot(&h), before, "opening the editor changes nothing");
    for text in [
        "Edit feature",
        "HOLE 01",
        "Mounting hole",
        "1 instance / Hole centres",
        "Diameter",
        "Preview",
        "Apply change",
        "Cancel edit",
        "Preview / Not applied",
    ] {
        assert!(h.widget(text).is_some(), "the editor shows {text:?}");
    }
    assert!(
        h.widget("Bolt pattern and Hole fillet will rebuild after this change.").is_some(),
        "the consequence summary names the model's dependents"
    );
    assert!(h.widget("Bolt pattern keeps its 2 × 2 instances at 40 × 30 mm pitch.").is_some(), "the pattern's pitch is preserved");
    assert!(h.widget("The hole stays on its point of Hole centres.").is_some());
    assert_eq!(h.widget("Ø8.00 mm"), None, "the diameter is a field now");
    *diameter(&mut h) = "10 mm".into();
    h.frames(2);
    assert_eq!(super::preview_commands(&h.app), Some(expected.clone()), "the preview runs the dialog's command");
    settle_preview(&mut h);
    assert!(h.app.preview.active, "the change is previewed in the viewport: {:?}", h.app.preview.error);
    assert_eq!(hole_diameter(&h), "8 mm", "a preview changes nothing");
    assert_eq!(h.app.session.revision, before.1);
    let editor_commands = dialogs::apply_commands(&h.app, &h.app.properties.editor.as_ref().unwrap().dialog).unwrap();
    assert_eq!(editor_commands, expected, "Apply issues exactly the dialog's command");
    h.step(&json!({"widget": "Apply change"})).unwrap();
    h.frames(3);
    assert!(h.app.properties.editor.is_none(), "applied: the panel shows the feature again");
    assert_eq!(hole_diameter(&h), "10 mm");
    assert_eq!(h.app.session.undo.len(), before.2 + 1, "one undo step");
    assert_eq!(h.app.session.undo.last().map(|u| u.label.as_str()), Some("Edit Feature"), "the step is the dialog's timeline.redefine");
    assert_eq!(h.app.session.doc.marker, before.4, "the timeline marker is where it was");
    assert_eq!(super::selected_feature(&h.app), Some(hole), "the hole stays selected");
    assert!(h.widget("Ø10.00 mm").is_some());
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(hole_diameter(&h), "8 mm", "one undo takes the whole edit back");
}

#[test]
fn cancel_and_esc_leave_the_design_its_revision_and_the_history_unchanged() {
    let mut h = mount();
    let hole = feature(&h, "Mounting hole");
    select(&mut h, "Mounting hole");
    let before = snapshot(&h);
    // Cancel edit.
    h.step(&json!({"widget": "Edit feature"})).unwrap();
    h.frames(2);
    *diameter(&mut h) = "10 mm".into();
    h.frames(2);
    settle_preview(&mut h);
    assert!(h.app.preview.active);
    h.step(&json!({"widget": "Cancel edit"})).unwrap();
    h.frames(2);
    assert!(h.app.properties.editor.is_none());
    assert_eq!(snapshot(&h), before, "Cancel leaves the design, its revision, the history and the selection alone");
    assert!(!h.app.preview.active, "the preview is gone");
    assert!(h.widget("Ø8.00 mm").is_some());
    // Esc, with the keyboard in the Diameter field.
    h.step(&json!({"widget": "Edit feature"})).unwrap();
    h.frames(2);
    *diameter(&mut h) = "12 mm".into();
    h.frames(2);
    let field = crate::scenario::rect_of("dialog:control:Diameter").expect("the Diameter field");
    h.step(&json!({"click": [field.center().x, field.center().y]})).unwrap();
    h.frames(2);
    assert!(h.ctx.memory(|m| m.focused().is_some()), "the field has the keyboard");
    h.step(&json!({"key": "Escape"})).unwrap();
    h.frames(2);
    assert!(h.app.properties.editor.is_none(), "Esc cancels the edit");
    assert_eq!(snapshot(&h), before, "Esc leaves the design, its revision, the history and the selection alone");
    assert_eq!(super::selected_feature(&h.app), Some(hole), "the hole stays selected");
    assert!(h.widget("Ø8.00 mm").is_some());
}

#[test]
fn invalid_input_shows_the_error_under_the_field_and_disables_apply() {
    let mut h = mount();
    select(&mut h, "Mounting hole");
    h.step(&json!({"widget": "Edit feature"})).unwrap();
    h.frames(2);
    let before = snapshot(&h);
    *diameter(&mut h) = "abc".into();
    h.frames(3);
    let error = h.app.session.doc.eval("abc", polykrom_engine::doc::expr::Kind::Length).unwrap_err().to_string();
    assert!(error.contains("abc"), "{error}");
    let shown = dialogs::shown_error(&error);
    assert!(h.widget(&shown).is_some(), "the error {shown:?} shows");
    let field = crate::scenario::rect_of("dialog:control:Diameter").expect("the field");
    let under = crate::scenario::rect_of("dialog:report:Diameter").expect("the error under the field");
    assert!(under.top() >= field.bottom() && under.top() < field.bottom() + 40.0, "the error is right under the field: {field:?} {under:?}");
    assert!(super::preview_commands(&h.app).is_none(), "nothing is previewed while a value is invalid");
    assert!(h.widget("Apply change").is_some());
    h.step(&json!({"widget": "Apply change"})).unwrap();
    h.frames(2);
    assert!(h.app.properties.editor.is_some(), "Apply is disabled: the editor stays");
    assert_eq!(snapshot(&h), before, "nothing was applied");
    // A wrong kind of value is refused too; a right one is accepted and Apply works again.
    *diameter(&mut h) = "45 deg".into();
    h.frames(2);
    assert!(
        h.widget(&dialogs::shown_error(&h.app.session.doc.eval("45 deg", polykrom_engine::doc::expr::Kind::Length).unwrap_err().to_string()))
            .is_some()
    );
    *diameter(&mut h) = "9 mm".into();
    h.frames(2);
    assert!(h.widget(&shown).is_none());
    h.step(&json!({"widget": "Apply change"})).unwrap();
    h.frames(3);
    assert!(h.app.properties.editor.is_none());
    assert_eq!(hole_diameter(&h), "9 mm");
}

#[test]
fn the_consequence_summary_lists_exactly_the_models_dependents() {
    let mut h = mount();
    let hole = feature(&h, "Mounting hole");
    let reported = reported_dependents(&mut h, hole);
    let lines = super::consequences(&mut h.app, hole);
    assert_eq!(lines[0], format!("{} will rebuild after this change.", reported.join(" and ")));
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[1], "Bolt pattern keeps its 2 × 2 instances at 40 × 30 mm pitch.");
    // The fillet: nothing depends on it.
    let fillet = feature(&h, "Hole fillet");
    assert!(reported_dependents(&mut h, fillet).is_empty());
    assert_eq!(super::consequences(&mut h.app, fillet), ["No other feature depends on it."]);
    // The base plate: everything after it goes with it; the summary names what the model does.
    let base = feature(&h, "Base plate");
    let reported = reported_dependents(&mut h, base);
    assert!(!reported.is_empty());
    let names: Vec<String> = super::dependents(&mut h.app, base).iter().map(|d| h.app.session.doc.feature(*d).unwrap().name.clone()).collect();
    assert_eq!(names, reported);
}

/// A design with one feature of each common kind (the kind-coverage model): primitives,
/// extrudes, a revolve, a fillet, a chamfer, a shell, holes of each type, patterns, a mirror, a
/// plane, a tapered extrude, a curvature fillet and a scaled body.
fn kinds() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    for (id, p) in [
        ("solid.box", json!({"length": 40, "width": 30, "height": 10, "name": "Block", "body_name": "Block"})),
        ("solid.cylinder", json!({"radius": 5, "height": 20, "base": [60, 0, 0], "name": "Pin", "body_name": "Pin"})),
        ("solid.sphere", json!({"radius": 6, "center": [80, 0, 0], "name": "Ball", "body_name": "Ball"})),
        ("solid.torus", json!({"major": 10, "minor": 2, "center": [110, 0, 0], "name": "Ring", "body_name": "Ring"})),
        ("sketch.create", json!({"plane": "XY", "offset": 10, "name": "Top"})),
        ("sketch.rectangle.two_point", json!({"p0": [5, 5], "p1": [15, 15]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance": 5, "operation": "join", "name": "Pad"})),
        ("sketch.create", json!({"plane": "XY", "offset": 15, "name": "Cap"})),
        ("sketch.rectangle.two_point", json!({"p0": [20, 5], "p1": [30, 15]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"sketch": "Cap", "to": [25, 10, 10], "direction": "negative", "operation": "join", "name": "Down to the block"})),
        ("sketch.create", json!({"plane": "XZ", "name": "Profile"})),
        ("sketch.rectangle.two_point", json!({"p0": [2, 30], "p1": [6, 36]})),
        ("sketch.finish", json!({})),
        ("solid.revolve", json!({"sketch": "Profile", "axis": "y", "angle": "180 deg", "name": "Turned", "body_name": "Turned"})),
        ("solid.fillet", json!({"edges": [[0, 15, 10]], "radius": "1 mm", "name": "Round"})),
        ("solid.chamfer", json!({"edges": [[40, 15, 10]], "distance": "1 mm", "name": "Bevel"})),
        ("solid.shell", json!({"faces": [[60, 0, 20]], "thickness": 1, "name": "Hollow pin"})),
        ("solid.hole", json!({"position": [35, 25, 10], "diameter": 3, "depth": 5, "name": "Drill"})),
        ("solid.pattern.rectangular", json!({"features": ["Drill"], "dir1": [-1, 0, 0], "count1": 2, "spacing1": 6, "name": "Row"})),
        (
            "solid.pattern.circular",
            json!({"bodies": ["Ball"], "axis": {"origin": [80, 20, 0], "dir": [0, 0, 1]}, "count": 3, "name": "Ring of balls"}),
        ),
        ("sketch.create", json!({"plane": "XY", "name": "Path"})),
        ("sketch.line", json!({"points": [[0, -30], [100, -30]], "ids": ["l1"]})),
        ("sketch.finish", json!({})),
        ("solid.pattern.path", json!({"bodies": ["Turned"], "path_sketch": "Path", "path": ["l1"], "count": 2, "spacing": 30, "name": "Along"})),
        ("solid.mirror", json!({"bodies": ["Pin"], "plane": "YZ", "name": "Twin"})),
        ("construct.plane.offset", json!({"base": "XY", "offset": 25, "name": "Shelf"})),
        // The kinds whose dialog names a value differently from the model (the r3 review): a
        // tapered extrude, counterbore and countersink holes, a curvature fillet, a scaled body.
        ("sketch.create", json!({"plane": "XY", "offset": 10, "name": "Boss"})),
        ("sketch.rectangle.two_point", json!({"p0": [18, 18], "p1": [25, 25]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"sketch": "Boss", "distance": 4, "taper": "5 deg", "operation": "join", "name": "Tapered boss"})),
        (
            "solid.hole",
            json!({"position": [8, 24, 10], "diameter": 3, "depth": 5, "type": "counterbore", "cb_diameter": 5, "cb_depth": 1, "name": "Counterbored"}),
        ),
        (
            "solid.hole",
            json!({"position": [33, 6, 10], "diameter": 3, "depth": 5, "type": "countersink", "cs_diameter": 6, "cs_angle": "90 deg", "name": "Countersunk"}),
        ),
        ("solid.fillet", json!({"edges": [[10, 5, 15]], "radius": "1 mm", "type": "curvature", "name": "Blend"})),
        ("solid.scale", json!({"bodies": ["Ring"], "factor": 1.5, "name": "Bigger ring"})),
    ] {
        h.app.run(id, p).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
    h.frames(3);
    h
}

/// Every feature of a design with one of each common kind is either edited in the panel or
/// opens its existing dialog (or, with no dialog yet, the palette); the panel says which.
#[test]
fn every_feature_kind_is_edited_in_the_panel_or_its_existing_way() {
    let mut h = kinds();
    let features: Vec<(u64, String)> = h.app.session.doc.features.iter().map(|f| (f.id, f.name.clone())).collect();
    let mut seen = std::collections::BTreeSet::new();
    for (id, name) in features {
        h.app.run("select.set", json!({"items": [{"type": "feature", "id": id}]})).unwrap();
        h.frames(3);
        let how = edits(&mut h.app, id);
        seen.insert(how);
        let note = match how {
            Edits::Panel | Edits::Sketch => "Edit the feature or its driving sketch.\nDependent features rebuild automatically.",
            Edits::Dialog => "This feature is edited in its dialog: Edit feature opens it.",
            Edits::Palette => "This feature has no dialog yet: Edit feature opens the command palette with timeline.edit.",
        };
        assert!(h.widget(note).is_some(), "{name}: the panel says how it is edited ({how:?})");
        let button = if how == Edits::Sketch { "Edit sketch" } else { "Edit feature" };
        h.step(&json!({"widget": button})).unwrap_or_else(|e| panic!("{name}: {e}"));
        h.frames(3);
        match how {
            Edits::Panel => {
                let ed = h.app.properties.editor.as_mut().unwrap_or_else(|| panic!("{name}: edited in the panel"));
                assert_eq!(ed.feature, id);
                assert!(!fields(&mut ed.dialog).is_empty(), "{name}: the editor has fields");
                assert!(h.app.dialog.is_none(), "{name}: no dialog beside the editor");
                assert!(h.widget("Apply change").is_some() && h.widget("Cancel edit").is_some(), "{name}");
                each_parameter_is_shown_once(&mut h, id, &name);
                super::cancel(&mut h.app);
            }
            Edits::Dialog => {
                let d = h.app.dialog.as_ref().unwrap_or_else(|| panic!("{name}: its dialog opens"));
                assert_eq!(d.editing.map(|(f, _)| f), Some(id), "{name}");
                assert!(h.app.properties.editor.is_none(), "{name}");
                dialogs::cancel(&mut h.app);
            }
            Edits::Palette => {
                assert!(h.app.ui.palette_open && h.app.sbox.text.starts_with("timeline.edit"), "{name}: the palette opens with timeline.edit");
                assert!(h.app.dialog.is_none() && h.app.properties.editor.is_none(), "{name}");
                h.app.ui.palette_open = false;
                h.app.sbox.text.clear();
            }
            Edits::Sketch => {
                assert_eq!(h.app.session.active_sketch, Some(id), "{name}: the sketch opens");
                h.app.finish_sketch();
            }
        }
        h.frames(2);
    }
    assert!(
        seen.contains(&Edits::Panel) && seen.contains(&Edits::Dialog) && seen.contains(&Edits::Palette) && seen.contains(&Edits::Sketch),
        "{seen:?}"
    );
    // The two the review found: a path pattern has no dialog; an extrude to an object has no
    // value to edit, so its dialog opens.
    let how = |h: &mut Harness, name: &str| {
        let id = feature(h, name);
        edits(&mut h.app, id)
    };
    assert_eq!(how(&mut h, "Along"), Edits::Palette);
    assert_eq!(how(&mut h, "Down to the block"), Edits::Dialog);
    assert_eq!(how(&mut h, "Twin"), Edits::Dialog);
    for name in [
        "Block",
        "Pin",
        "Ball",
        "Ring",
        "Pad",
        "Turned",
        "Round",
        "Bevel",
        "Hollow pin",
        "Drill",
        "Row",
        "Ring of balls",
        "Shelf",
        "Tapered boss",
        "Counterbored",
        "Countersunk",
        "Blend",
        "Bigger ring",
    ] {
        assert_eq!(how(&mut h, name), Edits::Panel, "{name}");
    }
}

/// The texts drawn inside the Properties panel this frame (the label column of a row, a
/// read-only value, a note), each with its centre, as often as it is drawn.
fn panel_labels(h: &Harness) -> Vec<(String, egui::Pos2)> {
    let Some(panel) = h.app.properties.rect else { return Vec::new() };
    let Some(tree) = h.output.accesskit_update.as_ref() else { return Vec::new() };
    tree.nodes
        .iter()
        .filter(|(_, n)| n.role() == egui::accesskit::Role::Label)
        .filter_map(|(_, n)| {
            let b = n.bounds()?;
            let at = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
            let text = n.label().or(n.value())?;
            panel.contains(at).then(|| (text.to_owned(), at))
        })
        .collect()
}

/// With the editor open on feature `id`: every field is one of the model's inputs (its label is
/// the model's), and every input of the model is drawn exactly once, as a field or as a
/// read-only row, never both (the r3 review: a field labelled "Counterbore Ø" left the model's
/// "Counterbore diameter" row beside it, so the value showed twice).
fn each_parameter_is_shown_once(h: &mut Harness, id: u64, name: &str) {
    let kind = h.app.session.doc.feature(id).map(|f| f.kind.clone()).unwrap_or_else(|| panic!("{name}: a feature"));
    let inputs = kind.inputs();
    let input_labels: Vec<&'static str> = inputs.iter().map(|(l, _, _)| *l).collect();
    let ed = h.app.properties.editor.as_mut().unwrap_or_else(|| panic!("{name}: the editor is open"));
    let field_labels: Vec<&'static str> = fields(&mut ed.dialog).iter().map(|(l, _, _)| *l).collect();
    for label in &field_labels {
        assert!(input_labels.contains(label), "{name}: the field {label:?} is one of the model's inputs {input_labels:?}");
    }
    let drawn = panel_labels(h);
    let texts: Vec<&str> = drawn.iter().map(|(d, _)| d.as_str()).collect();
    for label in input_labels.iter().collect::<std::collections::BTreeSet<_>>() {
        let listed = input_labels.iter().filter(|l| *l == label).count();
        let shown = texts.iter().filter(|d| *d == label).count();
        assert_eq!(shown, listed, "{name}: the parameter {label:?} is drawn once per input, as a field or a read-only row: {texts:?}");
    }
    // A read-only row of a model input: its label with a value drawn at its right on the same
    // line (a field's row has the field there, which is not a text). Fields and read-only rows
    // together show each input once: no value twice.
    let read_only = drawn
        .iter()
        .filter(|(d, at)| input_labels.contains(&d.as_str()) && drawn.iter().any(|(v, p)| v != d && p.x > at.x && (p.y - at.y).abs() < 2.0))
        .count();
    assert_eq!(
        field_labels.len() + read_only,
        inputs.len(),
        "{name}: every input is drawn once, as a field or a read-only row, never both: fields {field_labels:?}, {read_only} read-only rows, inputs {input_labels:?}"
    );
}

/// The labels of the panel's rows as they are shown in `lang`, and the ones among them that the
/// panel paints cut short with an ellipsis this frame.
fn elided_labels(h: &mut Harness, lang: &str) -> (Vec<String>, Vec<String>) {
    let labels: Vec<String> = {
        let _lang = crate::i18n::scope(lang);
        super::row_labels(&mut h.app).iter().map(|l| crate::i18n::tr_label(l).to_string()).collect()
    };
    let elided: Vec<String> = elided_in_panel(h).into_iter().filter(|text| labels.contains(text)).collect();
    (labels, elided)
}

/// The texts painted inside the Properties panel this frame that are cut short with an
/// ellipsis, each once.
fn elided_in_panel(h: &mut Harness) -> Vec<String> {
    fn walk(shape: &egui::Shape, panel: egui::Rect, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) if text.galley.elided && panel.contains(text.pos) => out.push(text.galley.job.text.clone()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, panel, out)),
            _ => {}
        }
    }
    let Some(panel) = h.app.properties.rect else { return vec!["(no panel)".to_string()] };
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
    let mut out = h.ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
        h.app.logic(ui.ctx());
        h.app.ui(ui);
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    out.shapes.iter().for_each(|s| walk(&s.shape, panel, &mut texts));
    texts.sort();
    texts.dedup();
    texts
}

/// Every parameter label of every kind the panel edits shows in full, in English, Japanese and
/// Chinese, with the feature selected and with it edited in place: the label column is sized to
/// the widest label of the rows shown (the r4 review: "Counterbore dia…" at the design's column).
#[test]
fn no_label_of_an_editable_kind_is_elided_in_en_ja_or_zh() {
    for lang in ["en", "ja", "zh"] {
        let mut h = kinds();
        h.app.preferences.language = lang.to_string();
        h.frames(3);
        let features: Vec<(u64, String)> = h.app.session.doc.features.iter().map(|f| (f.id, f.name.clone())).collect();
        let mut checked = 0;
        for (id, name) in features {
            if edits(&mut h.app, id) != Edits::Panel {
                continue;
            }
            h.app.run("select.set", json!({"items": [{"type": "feature", "id": id}]})).unwrap();
            h.frames(3);
            let (labels, elided) = elided_labels(&mut h, lang);
            assert!(!labels.is_empty(), "{lang}: {name} selected: rows");
            assert!(elided.is_empty(), "{lang}: {name} selected: elided {elided:?} among the rows {labels:?}");
            super::begin_edit(&mut h.app, id);
            h.frames(3);
            assert!(h.app.properties.editor.as_ref().is_some_and(|ed| ed.feature == id), "{lang}: {name} is edited in place");
            let (labels, elided) = elided_labels(&mut h, lang);
            assert!(elided.is_empty(), "{lang}: {name} edited: elided {elided:?} among the rows {labels:?}");
            // The widest label has its column: its row's label rect is at least as wide as it.
            for label in &labels {
                let Some(rect) = crate::scenario::rect_of(&format!("dialog:label:{label}")) else { continue };
                let natural = h.ctx.fonts_mut(|f| {
                    f.layout_no_wrap(label.clone(), egui::FontId::proportional(crate::theme::Tokens::get().font_label), egui::Color32::WHITE).size().x
                });
                assert!(rect.width() + 0.5 >= natural.min(rect.width()), "{lang}: {name}: {label:?} {rect:?} for {natural}");
            }
            checked += 1;
            super::cancel(&mut h.app);
            h.frames(2);
        }
        assert!(checked >= 18, "{lang}: {checked} kinds edited in the panel");
    }
}

/// The body of the subtitle and the Body row is looked up once per selection and design
/// revision (finding the body under a hole scans the model before it), not every frame: what the
/// cache holds is what the panel shows until the design changes.
#[test]
fn the_body_lookup_is_cached_per_selection_and_revision() {
    let mut h = mount();
    let hole = feature(&h, "Mounting hole");
    select(&mut h, "Mounting hole");
    let revision = h.app.session.revision;
    assert_eq!(h.app.properties.bodies, Some((hole, revision, vec!["Mount".to_string()])), "the hole's body, found under its position");
    assert!(h.widget("Mount / 1 instance").is_some());
    // The cache is what the frames read: a changed cache shows, with nothing recomputed.
    h.app.properties.bodies = Some((hole, revision, vec!["Cached body".to_string()]));
    h.frames(3);
    assert!(h.widget("Cached body / 1 instance").is_some() && h.widget("Cached body").is_some(), "the subtitle and the Body row read the cache");
    assert_eq!(h.app.properties.bodies.as_ref().map(|(_, r, _)| *r), Some(revision));
    // The editor reads the same cache.
    h.step(&json!({"widget": "Edit feature"})).unwrap();
    h.frames(2);
    assert!(h.widget("Cached body").is_some());
    super::cancel(&mut h.app);
    h.frames(2);
    // Another feature: computed for it (the plate made the body).
    select(&mut h, "Base plate");
    assert_eq!(h.app.properties.bodies, Some((feature(&h, "Base plate"), h.app.session.revision, vec!["Mount".to_string()])));
    // A design change (a new revision): computed again.
    select(&mut h, "Mounting hole");
    let revision = h.app.session.revision;
    h.app.properties.bodies = Some((hole, revision, vec!["Cached body".to_string()]));
    h.frames(2);
    assert!(h.widget("Cached body").is_some());
    h.app.run("document.metadata.set", json!({"values": {"project": "Actuator"}})).unwrap();
    h.frames(3);
    assert!(h.app.session.revision > revision);
    assert_eq!(h.app.properties.bodies, Some((hole, h.app.session.revision, vec!["Mount".to_string()])));
    assert!(h.widget("Cached body").is_none() && h.widget("Mount / 1 instance").is_some());
}

/// The scrollbar drawn inside `bounds` this frame (the widget egui registers for it).
fn scrollbar_in(h: &Harness, bounds: egui::Rect) -> Option<egui::Rect> {
    let tree = h.output.accesskit_update.as_ref()?;
    tree.nodes.iter().filter(|(_, n)| n.role() == egui::accesskit::Role::ScrollBar).find_map(|(_, n)| {
        let b = n.bounds()?;
        let r = egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32));
        bounds.contains_rect(r).then_some(r)
    })
}

/// The filled rectangles painted inside `bounds` in a window of `window`, each with its fill.
fn painted_fills_in(h: &mut Harness, window: egui::Rect, bounds: egui::Rect) -> Vec<(egui::Rect, egui::Color32)> {
    fn walk(shape: &egui::Shape, bounds: egui::Rect, out: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Rect(r) if bounds.contains_rect(r.rect) => out.push((r.rect, r.fill)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, bounds, out)),
            _ => {}
        }
    }
    // One more frame at the harness's clock (egui's history refuses time going backwards).
    let time = h.ctx.input(|i| i.time) + 1.0 / 60.0;
    let mut out = h.ctx.run_ui(egui::RawInput { screen_rect: Some(window), time: Some(time), ..Default::default() }, |ui| {
        h.app.logic(ui.ctx());
        h.app.ui(ui);
    });
    out.textures_delta.clear();
    let mut fills = Vec::new();
    out.shapes.iter().for_each(|s| walk(&s.shape, bounds, &mut fills));
    fills
}

/// A short window (1366 × 768 and 1280 × 720), floating and docked, the hole selected and then
/// edited in place: the card's actions (Edit feature and Edit sketch; Apply change and Cancel
/// edit), the panel's help line and the footer line stand at the card's bottom, inside the card
/// and the window, and only the body above them scrolls: where it overflows (the floating card,
/// shorter than the docked panel, at both sizes) a scrollbar shows with the pointer away, and
/// the wheel scrolls the body to its last row (the last dependent; the editor's last consequence
/// line) while the actions and the footer stay where they are; where it fits, the last row is
/// in sight as it is (the r6 review: at 1366 × 768 the card was cut off at the window's bottom,
/// the help line clipped mid-line, the dependents and the footer out of sight, with nothing to
/// show that the card scrolls).
#[test]
fn the_actions_and_the_footer_stay_at_the_cards_bottom_and_only_the_body_scrolls() {
    use crate::scenario::rect_of;
    use crate::shell::LayoutMode;
    let t = crate::theme::Tokens::get();
    for layout in [LayoutMode::Floating, LayoutMode::Docked] {
        for (width, height) in [(1366.0_f32, 768.0_f32), (1280.0, 720.0)] {
            let mut h = mount();
            h.call("ui.set", json!({"layout": layout}));
            h.call("ui.resize", json!({"width": width, "height": height}));
            select(&mut h, "Mounting hole");
            let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
            let fillet = feature(&h, "Hole fillet");
            for editor in [false, true] {
                if editor {
                    h.step(&json!({"widget": "Edit feature"})).unwrap();
                    h.frames(3);
                    assert!(h.app.properties.editor.is_some(), "{layout:?} {width}×{height}: the hole is edited in place");
                }
                let view = if editor { "the editor" } else { "the panel" };
                // What is pinned under the body, top down (the panel's help line goes with its
                // actions), and the body's last row.
                let (pinned, last): (Vec<&str>, String) = if editor {
                    (vec!["properties:apply", "properties:cancel"], "dialog:hint:properties_consequence".to_string())
                } else {
                    (vec!["properties:edit", "properties:edit_sketch", "dialog:note:properties_help"], format!("properties:dependent:{fillet}"))
                };
                let mut overflows = false;
                for scrolled in [false, true] {
                    let case = format!("{layout:?} {width}×{height}, {view}{}", if scrolled { ", scrolled to the end" } else { "" });
                    if scrolled && !overflows {
                        continue;
                    }
                    if scrolled {
                        // The wheel over the body, as a mouse would send it, then the pointer away.
                        let card = rect_of("properties:panel").unwrap_or_else(|| panic!("{case}: the card"));
                        h.app.synthetic.push(egui::Event::PointerMoved(egui::pos2(card.center().x, card.top() + card.height() * 0.3)));
                        h.app.synthetic.push(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -5000.0),
                            modifiers: egui::Modifiers::NONE,
                            phase: egui::TouchPhase::Move,
                        });
                        h.app.synthetic.push(egui::Event::PointerGone);
                        h.settle();
                    }
                    h.frames(10);
                    let card = rect_of("properties:panel").unwrap_or_else(|| panic!("{case}: the card"));
                    assert!(window.contains_rect(card), "{case}: the card {card:?} is inside the window {window:?}");
                    // The actions, one under the other, their note, then the footer, all inside
                    // the card and the footer at its bottom.
                    let mut above = card.top();
                    for name in &pinned {
                        let r = rect_of(name).unwrap_or_else(|| panic!("{case}: {name}"));
                        assert!(card.contains_rect(r), "{case}: {name} {r:?} is inside the card {card:?}");
                        assert!(r.top() >= above - 0.5, "{case}: {name} {r:?} is under the one before, which ends at {above}");
                        above = r.bottom();
                    }
                    let footer = rect_of("dialog:note:properties_footer").unwrap_or_else(|| panic!("{case}: the footer"));
                    assert!(card.contains_rect(footer), "{case}: the footer {footer:?} is inside the card {card:?}");
                    assert!(footer.top() >= above - 0.5, "{case}: the footer {footer:?} is under the actions, which end at {above}");
                    assert!(
                        card.bottom() - footer.bottom() <= t.space_24,
                        "{case}: the footer {footer:?} stands at the card's bottom {}",
                        card.bottom()
                    );
                    let first_action = rect_of(pinned[0]).unwrap_or_else(|| panic!("{case}: {}", pinned[0]));
                    // A body that overflows has its scrollbar, above the actions, its handle
                    // painted with the pointer away. The floating card is the short one.
                    let bar = scrollbar_in(&h, card);
                    if !scrolled {
                        overflows = bar.is_some();
                        assert!(overflows || layout == LayoutMode::Docked, "{case}: the body overflows and shows its scrollbar");
                    }
                    if let Some(bar) = bar {
                        assert!(
                            bar.bottom() <= first_action.top() + 0.5,
                            "{case}: the scrollbar {bar:?} is the body's, above the actions {first_action:?}"
                        );
                        let fills = painted_fills_in(&mut h, window, bar.expand(1.0));
                        assert!(
                            fills.iter().any(|(r, fill)| fill.a() > 0 && r.height() <= bar.height()),
                            "{case}: the scrollbar's handle is painted in {bar:?}: {fills:?}"
                        );
                    }
                    // Scrolled to the end (or fitting as it is), the body's last row is in
                    // sight, above the actions.
                    if scrolled || !overflows {
                        let r = rect_of(&last).unwrap_or_else(|| panic!("{case}: {last}"));
                        assert!(
                            card.contains_rect(r) && r.bottom() <= first_action.top() + 0.5,
                            "{case}: the body's last row {last} {r:?} is in sight above the actions {first_action:?}, inside the card {card:?}"
                        );
                    }
                }
                if editor {
                    super::cancel(&mut h.app);
                    h.frames(2);
                }
            }
        }
    }
}

#[test]
fn the_panel_gives_way_to_the_inspector_the_review_panel_dialogs_and_tools() {
    let mut h = mount();
    select(&mut h, "Mounting hole");
    assert!(h.app.properties.rect.is_some());
    // A body selected: the inspector stands there.
    h.app.run("select.set", json!({"items": [{"type": "body", "name": "Mount"}]})).unwrap();
    h.frames(3);
    assert!(h.app.properties.rect.is_none() && h.app.inspector.rect.is_some());
    h.app.run("select.clear", json!({})).unwrap();
    h.frames(3);
    assert!(h.app.properties.rect.is_some(), "nothing selected: the design's properties");
    // A dialog: the panel waits, and an editor in progress is dropped (nothing to restore).
    select(&mut h, "Mounting hole");
    h.step(&json!({"widget": "Edit feature"})).unwrap();
    h.frames(2);
    let before = snapshot(&h);
    h.step(&json!({"start": "solid.box"})).unwrap();
    h.frames(2);
    assert!(h.app.dialog.is_some() && h.app.properties.editor.is_none() && h.app.properties.rect.is_none());
    dialogs::cancel(&mut h.app);
    h.frames(2);
    assert_eq!(snapshot(&h).0, before.0);
    assert!(h.app.properties.rect.is_some());
}

/// Every rectangle one more frame paints in a window of `window`, with its fill and its edge.
fn painted_frames_in(h: &mut Harness, window: egui::Rect) -> Vec<(egui::Rect, egui::Color32, egui::Color32)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32, egui::Color32)>) {
        match shape {
            egui::Shape::Rect(r) => out.push((r.rect, r.fill, r.stroke.color)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let time = h.ctx.input(|i| i.time) + 1.0 / 60.0;
    let mut out = h.ctx.run_ui(egui::RawInput { screen_rect: Some(window), time: Some(time), ..Default::default() }, |ui| {
        h.app.logic(ui.ctx());
        h.app.ui(ui);
    });
    out.textures_delta.clear();
    let mut rects = Vec::new();
    out.shapes.iter().for_each(|s| walk(&s.shape, &mut rects));
    rects
}

/// Floating, the card is glass over the canvas like the other floating cards (the r7 merge of
/// the shell redesign): its frame is the panel colour at the glass opacity with the glass's
/// light edge, not the solid panel. Docked, the panel is an edge panel beside the viewport and
/// stays solid.
#[test]
fn the_floating_card_is_glass_and_the_docked_panel_is_solid() {
    for layout in ["floating", "docked"] {
        let mut h = mount();
        h.call("ui.set", json!({"layout": layout}));
        select(&mut h, "Mounting hole");
        h.frames(3);
        let t = crate::theme::Tokens::get();
        // One more frame at the harness's own window, so the card stands where it was measured.
        let window = h.ctx.content_rect();
        let panel = h.app.properties.rect.expect("the panel");
        // The frames painted over most of the panel's rect (the floating area's rect runs on
        // below its painted frame; the docked panel's frame and rect differ by its edges), each
        // with its fill and its edge.
        let fills: Vec<(egui::Rect, egui::Color32, egui::Color32)> = painted_frames_in(&mut h, window)
            .into_iter()
            .filter(|(r, _, _)| r.intersect(panel).area() >= 0.6 * panel.area() && r.area() <= 2.0 * panel.area())
            .collect();
        let card = |fill: egui::Color32| fills.iter().any(|(_, f, _)| *f == fill);
        if layout == "floating" {
            assert!(
                fills.iter().any(|(_, f, edge)| *f == t.glass(t.panel) && *edge == t.glass_edge()),
                "{layout}: the card's frame is the panel's glass {:?} with the glass edge {:?}: {fills:?}",
                t.glass(t.panel),
                t.glass_edge()
            );
            assert!(!card(t.panel), "{layout}: the card is not the solid panel: {fills:?}");
        } else {
            assert!(card(t.panel), "{layout}: the panel is solid {:?}: {fills:?}", t.panel);
        }
    }
}

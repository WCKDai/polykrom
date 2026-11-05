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

/// Frames until the preview is up (it computes off the window thread).
fn settle_preview(h: &mut Harness) {
    for _ in 0..400 {
        if h.app.preview.active || h.app.preview.error.is_some() {
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

/// Every feature of a design with one of each common kind is either edited in the panel or
/// opens its existing dialog (or, with no dialog yet, the palette); the panel says which.
#[test]
fn every_feature_kind_is_edited_in_the_panel_or_its_existing_way() {
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

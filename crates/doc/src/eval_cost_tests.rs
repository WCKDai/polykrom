//! What an evaluation costs after an edit at the end of a large design: the features before
//! the edit are reused, so the edit's cost must not grow with the bodies and imported files
//! the design already holds. A sketch on a fixed plane once moved every body of every placed
//! component into its frame, and every evaluation wrote every imported STEP file out again to
//! fingerprint it, so each sketch command took longer the more parts were inserted.

use std::cell::Cell;
use std::sync::Arc;

use solidcraft_geom::{Plane, Vec2, Vec3};
use solidcraft_kernel as kernel;
use solidcraft_sketch::Sketch;

use crate::*;

/// Work counted on this thread: bodies moved, STEP texts digested, joint faces measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Counts {
    pub moved: usize,
    pub digested: usize,
    pub joint_faces: usize,
}

thread_local! {
    static COUNTS: Cell<Counts> = const { Cell::new(Counts { moved: 0, digested: 0, joint_faces: 0 }) };
}

pub(super) fn count(f: impl FnOnce(&mut Counts)) {
    COUNTS.with(|c| {
        let mut n = c.get();
        f(&mut n);
        c.set(n);
    });
}

/// The work `f` does on this thread.
fn counted(f: impl FnOnce()) -> Counts {
    COUNTS.with(|c| c.set(Counts::default()));
    f();
    COUNTS.with(Cell::get)
}

#[test]
fn joint_face_measurements_are_shared_across_motion_samples_and_snap_kinds() {
    use crate::joints::{Joint, JointKind, JointOrigin, Snap, XRef};
    let mut doc = Document::new("joint face work");
    let part = doc.add_component(Some("Crank"), 0).unwrap();
    let occurrence = doc.occurrences.iter_mut().find(|o| o.component == part).unwrap();
    occurrence.grounded = false;
    let occurrence = occurrence.id;
    let id = doc
        .add_feature(
            FeatureKind::Cylinder {
                base: point_expr(Vec3::ZERO),
                axis: Dir(Vec3::Z),
                radius: "2".into(),
                height: "10".into(),
                operation: Operation::NewBody,
            },
            None,
        )
        .unwrap();
    doc.feature_mut(id).unwrap().component = part;
    let mut model = Model::new();
    model.evaluate(&doc);
    let state = model.state();
    let pick = Vec3::new(0.0, 0.0, 10.0);
    let face = joints::face_name_at(&doc, &state, part, pick, false).unwrap();
    doc.assembly.joints.push(Joint {
        id: 100,
        name: "Pivot".into(),
        kind: JointKind::Revolute,
        a: JointOrigin { occurrence: 0, snap: Snap::Frame { origin: pick, z: Vec3::Z, x: Some(Vec3::X) } },
        b: JointOrigin { occurrence, snap: Snap::FaceCenter { pick, face: None, x: Some(XRef::Dir(Vec3::X)) } },
        values: vec![0.0],
        limits: Vec::new(),
        flip: true,
        offset: 0.0,
        angle: 0.0,
        suppressed: false,
        exprs: Default::default(),
    });
    // One body supplies both old nearest-face snaps and persistent named-face snaps.
    // A scratch model shares its measurements, even though each sample changes the pose.
    let snapshot = model.clone();
    let work = counted(|| {
        for name in [None, Some(face)] {
            doc.assembly.joints[0].b.snap = Snap::FaceCenter { pick, face: name, x: Some(XRef::Dir(Vec3::X)) };
            for k in 0..145 {
                let angle = (k as f64 * 2.5).to_radians();
                doc.assembly.joints[0].values[0] = angle;
                let solution = joints::solve(&doc, &snapshot.state());
                assert!(solution.errors.is_empty() && solution.conflicts.is_empty(), "{solution:?}");
                let point = apply_point(&solution.world[&occurrence], Vec3::new(5.0, 0.0, 10.0));
                assert!(point.dist(Vec3::new(5.0 * angle.cos(), 5.0 * angle.sin(), 10.0)) < 1e-9);
                doc.apply_joint_solution(&solution);
            }
        }
    });
    assert_eq!(work.joint_faces, 1, "pose-only samples must not remeasure unchanged faces: {work:?}");

    // Editing geometry creates a fresh measurement without changing a retained snapshot.
    let FeatureKind::Cylinder { height, .. } = &mut doc.feature_mut(id).unwrap().kind else { panic!("cylinder") };
    *height = "12".into();
    model.evaluate(&doc);
    let origin = &doc.assembly.joints[0].b;
    let work = counted(|| {
        let changed = joints::resolve_origin(&doc, &model.state(), origin).unwrap();
        let retained = joints::resolve_origin(&doc, &snapshot.state(), origin).unwrap();
        assert!((changed[3][2] - 12.0).abs() < 1e-9);
        assert!((retained[3][2] - 10.0).abs() < 1e-9);
    });
    assert_eq!(work.joint_faces, 1, "only the edited body must be measured: {work:?}");
}

#[test]
fn joint_face_measurements_never_follow_a_replaced_body_or_stale_offset() {
    use crate::joints::{JointOrigin, Snap};
    let mut doc = Document::new("replaced joint geometry");
    let mut state = ModelState::default();
    state.bodies.push(ModelBody::new("Pin".into(), kernel::cylinder(Vec3::ZERO, Vec3::Z, 2.0, 10.0).unwrap(), 0));
    let origin = JointOrigin { occurrence: 0, snap: Snap::FaceCenter { pick: Vec3::new(0.0, 0.0, 10.0), face: None, x: None } };
    let work = counted(|| {
        joints::resolve_origin(&doc, &state, &origin).unwrap();
    });
    assert_eq!(work.joint_faces, 1);
    let retained = state.clone();
    // Body is public; a clone's old measurement must never mask a direct replacement.
    state.bodies[0].body = kernel::cylinder(Vec3::ZERO, Vec3::Z, 2.0, 12.0).unwrap();
    let changed = joints::resolve_origin(&doc, &state, &origin).unwrap();
    assert!((changed[3][2] - 12.0).abs() < 1e-9);
    let work = counted(|| {
        let old = joints::resolve_origin(&doc, &retained, &origin).unwrap();
        assert!((old[3][2] - 10.0).abs() < 1e-9);
        doc.body_offsets.insert("Pin".into(), rigid(Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO, Vec3::Z, 0.0));
        let moved = joints::resolve_origin(&doc, &retained, &origin).unwrap();
        assert!((moved[3][2] - 11.0).abs() < 1e-9);
    });
    assert_eq!(work.joint_faces, 0, "offsets are applied after the shared face measurements");
}

/// `parts` imported files of two boxes each, every one in its own component placed away from
/// the origin (as inserted parts are).
fn placed_imports(parts: usize) -> Document {
    let mut doc = Document::new("placed imports");
    for i in 0..parts {
        let x = 30.0 * i as f64;
        let a = kernel::box_solid(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 10.0, 10.0, 10.0)).unwrap();
        let b = kernel::box_solid(Vec3::new(x, 20.0, 0.0), Vec3::new(x + 5.0, 25.0, 5.0)).unwrap();
        let step: Arc<str> = kernel::step_export(&[&a, &b], "test").unwrap().into();
        let comp = doc.add_component(Some(&format!("Part {i}")), 0).unwrap();
        let o = doc.occurrences.iter_mut().find(|o| o.component == comp).unwrap();
        o.transform = rigid(Vec3::new(0.0, 0.0, 5.0 * i as f64), Vec3::ZERO, Vec3::Z, 0.05 * i as f64);
        let kind = FeatureKind::Import { file: format!("part{i}.step"), step, components: Vec::new(), skip: Vec::new() };
        let id = doc.add_feature(kind, Some(&format!("Part {i}"))).unwrap();
        doc.feature_mut(id).unwrap().component = comp;
    }
    doc
}

fn line_sketch(plane: PlaneRef, to: Vec2) -> FeatureKind {
    let mut sk = Sketch::new();
    let a = sk.add_point(Vec2::new(0.0, 0.0), None).unwrap();
    let b = sk.add_point(to, None).unwrap();
    sk.add_line_pts(a, b, None).unwrap();
    FeatureKind::Sketch { plane, sketch: sk }
}

#[test]
fn a_sketch_on_a_fixed_plane_costs_the_same_in_a_large_placed_design() {
    let parts = 12;
    let mut doc = placed_imports(parts);
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().find(|r| r.error.is_some()));
    assert_eq!(model.state().bodies.len(), parts * 2);

    // A construction plane, then sketches on the origin planes, on an offset of one and on the
    // construction plane, as commands make them one after another: no body is moved and no
    // file digested again.
    let plane = doc.add_feature(FeatureKind::ConstructionPlane { plane: PlaneRef::Origin { name: "XZ".into() } }, Some("Datum")).unwrap();
    assert_eq!(counted(|| model.evaluate(&doc)), Counts::default(), "construction plane");
    assert!(model.result(plane).unwrap().error.is_none());
    let datum = doc.feature(plane).unwrap().name.clone();
    let planes = [
        PlaneRef::Origin { name: "XY".into() },
        PlaneRef::Offset { base: Box::new(PlaneRef::Origin { name: "YZ".into() }), distance: "5 mm".into() },
        PlaneRef::Construction { name: datum.into() },
    ];
    for (k, plane) in planes.into_iter().enumerate() {
        let id = doc.add_feature(line_sketch(plane, Vec2::new(10.0, 1.0 + k as f64)), None).unwrap();
        let work = counted(|| model.evaluate(&doc));
        assert_eq!(work, Counts::default(), "sketch {k}");
        assert_eq!(model.last_recomputed, 1);
        let r = model.result(id).unwrap();
        assert!(r.error.is_none(), "{:?}", r.error);
        assert_eq!(model.state().bodies.len(), parts * 2);
        // Editing the sketch again recomputes it alone, for the same cost.
        if let Ok(sk) = doc.sketch_mut(id)
            && let Some(p) = sk.points.get_mut(1)
        {
            p.pos = Vec2::new(12.0, 2.0);
        }
        let work = counted(|| model.evaluate(&doc));
        assert_eq!(work, Counts::default(), "sketch {k} edited");
        assert_eq!(model.last_recomputed, 1);
    }
}

#[test]
fn a_sketch_on_a_face_still_sees_the_placed_bodies() {
    let parts = 3;
    let mut doc = placed_imports(parts);
    let mut model = Model::new();
    model.evaluate(&doc);
    // A face pick reads the model as the sketch's component sees it: the other components'
    // bodies are moved into its frame, as before.
    let top = Plane::new(Vec3::new(0.0, 0.0, 10.0), Vec3::X, Vec3::Y).unwrap();
    let face = PlaneRef::Face { plane: top, at: Vec3::new(5.0, 5.0, 10.0), name: None, anchor: None };
    doc.add_feature(line_sketch(face, Vec2::new(3.0, 3.0)), None).unwrap();
    let work = counted(|| model.evaluate(&doc));
    assert!(work.moved >= parts * 2 - 2, "{work:?}");
    assert_eq!(work.digested, 0);
}

#[test]
fn an_imported_file_is_fingerprinted_by_its_text() {
    let doc = placed_imports(2);
    let mut model = Model::new();
    let work = counted(|| model.evaluate(&doc));
    assert_eq!(work.digested, 2);
    let fingerprints = |m: &Model| m.results.iter().map(|r| r.fingerprint).collect::<Vec<_>>();
    let before = fingerprints(&model);

    // The same text held by another copy (an undo state, a design opened again): the same
    // fingerprints, and nothing evaluated again.
    let mut copy = doc.clone();
    for f in &mut copy.features {
        if let FeatureKind::Import { step, .. } = &mut f.kind {
            *step = Arc::from(step.to_string());
        }
    }
    let work = counted(|| model.evaluate(&copy));
    assert_eq!(work.digested, 2);
    assert_eq!(fingerprints(&model), before);
    assert_eq!(model.last_recomputed, 0);

    // Other text: that feature and the ones after it are evaluated again.
    let other = kernel::box_solid(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0)).unwrap();
    let text: Arc<str> = kernel::step_export(&[&other], "test").unwrap().into();
    if let Some(FeatureKind::Import { step, .. }) = copy.features.last_mut().map(|f| &mut f.kind) {
        *step = text;
    }
    let work = counted(|| model.evaluate(&copy));
    assert_eq!(work.digested, 1);
    let after = fingerprints(&model);
    assert_eq!(after.first(), before.first());
    assert_ne!(after.last(), before.last());
    assert_eq!(model.last_recomputed, 1);
    assert_eq!(model.state().bodies.len(), 3);
}

/// A 10 mm square plate per hole along X, 4 mm thick, in component `comp`, with `holes` square
/// holes cut through it by boxes drawn in component `tools` (the plate's own, usually). The
/// plate's body name.
fn plate_with_holes(doc: &mut Document, comp: u64, tools: u64, holes: usize) -> String {
    let length = format!("{}", 10 * holes.max(1));
    let kind = FeatureKind::Box {
        corner: point_expr(Vec3::ZERO),
        length: length.into(),
        width: "10".into(),
        height: "4".into(),
        operation: Operation::NewBody,
    };
    let plate = doc.add_feature(kind, None).unwrap();
    doc.feature_mut(plate).unwrap().component = comp;
    let mut model = Model::new();
    model.evaluate(doc);
    let body = model.state().bodies.iter().find(|b| b.feature == plate).unwrap().name.clone();
    for k in 0..holes {
        let corner = point_expr(Vec3::new(10.0 * k as f64 + 4.0, 4.0, -1.0));
        let kind = FeatureKind::Box { corner, length: "2".into(), width: "2".into(), height: "6".into(), operation: Operation::Cut };
        let id = doc.add_feature(kind, None).unwrap();
        let f = doc.feature_mut(id).unwrap();
        f.component = tools;
        f.participants = vec![body.clone()];
    }
    body
}

fn place(doc: &mut Document, comp: u64, m: Mat) {
    doc.occurrences.iter_mut().find(|o| o.component == comp).unwrap().transform = m;
}

fn volume(model: &Model, body: &str) -> f64 {
    kernel::measure(&model.state().body(body).unwrap().body).unwrap().volume
}

#[test]
fn turning_a_joint_between_parts_cuts_no_hole_again() {
    let mut doc = Document::new("two parts");
    let base = doc.add_component(Some("Base"), 0).unwrap();
    let arm = doc.add_component(Some("Arm"), 0).unwrap();
    let base_plate = plate_with_holes(&mut doc, base, base, 3);
    let arm_plate = plate_with_holes(&mut doc, arm, arm, 3);
    place(&mut doc, arm, rigid(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Z, 0.0));
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().find(|r| r.error.is_some()));
    assert!((volume(&model, &base_plate) - (1200.0 - 3.0 * 16.0)).abs() < 1e-6);
    // Turning the arm (as a revolute joint does) moves it and nothing else: each part's holes
    // stay cut in its own frame, so no hole is cut again.
    for step in 1..=3 {
        place(&mut doc, arm, rigid(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Z, 0.3 * f64::from(step)));
        model.evaluate(&doc);
        assert_eq!(model.last_recomputed, 0, "step {step}");
        assert!((volume(&model, &arm_plate) - (1200.0 - 3.0 * 16.0)).abs() < 1e-6);
    }
    // Moving both parts together changes nothing between them either.
    for comp in [base, arm] {
        place(&mut doc, comp, rigid(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::Z, 0.0));
    }
    model.evaluate(&doc);
    assert_eq!(model.last_recomputed, 0);
}

#[test]
fn a_tool_cutting_another_part_cuts_again_when_that_part_moves() {
    let mut doc = Document::new("cross part");
    let tools = doc.add_component(Some("Tools"), 0).unwrap();
    let plate = doc.add_component(Some("Plate"), 0).unwrap();
    let other = doc.add_component(Some("Other"), 0).unwrap();
    // The other part first, so the cut across parts is the last feature: it alone is evaluated
    // again when it changes.
    plate_with_holes(&mut doc, other, other, 1);
    let body = plate_with_holes(&mut doc, plate, tools, 1);
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().find(|r| r.error.is_some()));
    assert!((volume(&model, &body) - (400.0 - 16.0)).abs() < 1e-6);
    // An unrelated part moving: the cut keeps its result.
    place(&mut doc, other, rigid(Vec3::new(0.0, 40.0, 0.0), Vec3::ZERO, Vec3::Z, 0.0));
    model.evaluate(&doc);
    assert_eq!(model.last_recomputed, 0);
    // The plate moving away from the tool by 5 mm: the hole is cut again where the tool now meets
    // it, half of it on the plate.
    place(&mut doc, plate, rigid(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::Z, 0.0));
    model.evaluate(&doc);
    assert_eq!(model.last_recomputed, 1);
    assert!((volume(&model, &body) - (400.0 - 8.0)).abs() < 1e-6, "{}", volume(&model, &body));
    // The tool's part following it: the hole is where it was at first, the first result again.
    place(&mut doc, tools, rigid(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::Z, 0.0));
    model.evaluate(&doc);
    assert_eq!(model.last_recomputed, 0);
    assert!((volume(&model, &body) - (400.0 - 16.0)).abs() < 1e-6, "{}", volume(&model, &body));
}

/// A closed rectangle from `a` to `b`.
fn rect_sketch(plane: PlaneRef, a: Vec2, b: Vec2) -> FeatureKind {
    let mut sk = Sketch::new();
    let pts: Vec<usize> = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)].iter().map(|p| sk.add_point(*p, None).unwrap()).collect();
    for k in 0..4 {
        sk.add_line_pts(pts[k], pts[(k + 1) % 4], None).unwrap();
    }
    FeatureKind::Sketch { plane, sketch: sk }
}

fn extrude(sketch: u64, distance: &str, direction: Direction, operation: Operation) -> FeatureKind {
    let extent = Extent {
        convention: ExtrudeConvention::LegacyPerSide,
        distance: distance.into(),
        direction,
        distance2: None,
        start_offset: None,
        through_all: false,
        taper: None,
        to: None,
        to_offset: None,
        from: None,
    };
    FeatureKind::Extrude { thin: None, sketch: SketchId(sketch), profiles: ProfileSel::All, extent, operation, targets: vec![] }
}

/// Part `i` in its own component, as a library part is built: a plate from its sketch, a bore cut
/// by a box naming the plate, a pin, a socket cut by an extrude naming no target (which shapes its
/// tool around every body), then a split. The id of the plate's extrude.
fn library_part(doc: &mut Document, i: usize) -> u64 {
    let comp = doc.add_component(Some(&format!("Part {i}")), 0).unwrap();
    place(doc, comp, rigid(Vec3::new(40.0 * i as f64, 0.0, 0.0), Vec3::ZERO, Vec3::Z, 0.0));
    let plate = format!("Plate {i}");
    let mut add = |kind: FeatureKind, names: &[String], participants: &[String]| {
        let id = doc.add_feature(kind, None).unwrap();
        let f = doc.feature_mut(id).unwrap();
        (f.component, f.body_names, f.participants) = (comp, names.to_vec(), participants.to_vec());
        id
    };
    let xy = || PlaneRef::Origin { name: "XY".into() };
    let sketch = add(rect_sketch(xy(), Vec2::ZERO, Vec2::new(20.0, 10.0)), &[], &[]);
    let body = add(extrude(sketch, "4", Direction::Positive, Operation::NewBody), std::slice::from_ref(&plate), &[]);
    let bore = |x: f64, op| FeatureKind::Box {
        corner: point_expr(Vec3::new(x, 4.0, -1.0)),
        length: "2".into(),
        width: "2".into(),
        height: "6".into(),
        operation: op,
    };
    add(bore(4.0, Operation::Cut), &[], std::slice::from_ref(&plate));
    add(bore(30.0, Operation::NewBody), &[format!("Pin {i}")], &[]);
    let below = PlaneRef::Offset { base: Box::new(xy()), distance: "-1".into() };
    let socket = add(rect_sketch(below, Vec2::new(2.0, 2.0), Vec2::new(4.0, 4.0)), &[], &[]);
    add(extrude(socket, "7", Direction::Positive, Operation::Cut), &[], &[]);
    let split = Plane::new(Vec3::new(10.0, 0.0, 0.0), Vec3::Y, Vec3::Z).unwrap();
    let split = FeatureKind::Split { body: BodyName(plate.clone()), plane: PlaneRef::Custom { plane: split }, tool: None, sketch: None, line: None };
    add(split, &[format!("{plate} half")], &[]);
    body
}

/// Features the last evaluation of `model` evaluated (in shadow mode it evaluates the rebased
/// ones too).
fn evaluated(model: &Model) -> usize {
    model.last_recomputed - if super::rebase::shadow() { model.last_rebased } else { 0 }
}

/// Every feature's result in `model` is what evaluating `doc` from scratch gives.
fn assert_full_evaluation(doc: &Document, model: &Model) {
    let mut full = Model { no_cache: true, ..Model::new() };
    full.evaluate(doc);
    assert_eq!(model.results.len(), full.results.len());
    for (a, b) in model.results.iter().zip(&full.results) {
        assert_eq!((a.id, &a.error, &a.warning), (b.id, &b.error, &b.warning));
        assert_eq!(super::rebase::differs(&a.state, &b.state), None, "after {}", a.name);
    }
}

/// Library parts after an unused sketch, their cuts made now (`scoped`) or as older designs made
/// them; evaluated.
fn library_parts(parts: usize, scoped: bool) -> (Document, u64, u64, Model) {
    let mut doc = Document::new("library parts");
    let unused = doc.add_feature(line_sketch(PlaneRef::Origin { name: "XZ".into() }, Vec2::new(5.0, 5.0)), None).unwrap();
    let first = library_part(&mut doc, 0);
    for i in 1..parts {
        library_part(&mut doc, i);
    }
    for f in &mut doc.features {
        f.scoped_surroundings &= scoped;
    }
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().find(|r| r.error.is_some()));
    assert_eq!(model.state().bodies.len(), parts * 3);
    (doc, unused, first, model)
}

#[test]
fn an_edit_in_one_library_part_rebases_the_parts_after_it() {
    let parts = 4;
    let (mut doc, unused, first, mut model) = library_parts(parts, false);
    // As older designs made the socket cuts, naming no target, each shapes its tool around every
    // body. The first part's plate made thicker: that part is evaluated again, and the socket cut
    // of every other part with the split of what it cut; the rest is rebased.
    let FeatureKind::Extrude { extent, .. } = &mut doc.feature_mut(first).unwrap().kind else { panic!("extrude") };
    extent.distance = "5".into();
    model.evaluate(&doc);
    assert_eq!(evaluated(&model), 4 + 2 * (parts - 1), "rebased {}", model.last_rebased);
    // The two features before the edit are taken as they were.
    assert_eq!(model.last_rebased, doc.features.len() - 2 - evaluated(&model));
    assert_full_evaluation(&doc, &model);
    assert!((volume(&model, "Pin 0") - 24.0).abs() < 1e-9);

    // A change that leaves every body as it was rebases even the socket cuts.
    let Ok(sketch) = doc.sketch_mut(unused) else { panic!("sketch") };
    sketch.points[1].pos = Vec2::new(6.0, 5.0);
    model.evaluate(&doc);
    assert_eq!((evaluated(&model), model.last_rebased), (1, doc.features.len() - 1));
    assert_full_evaluation(&doc, &model);
}

#[test]
fn a_scoped_cut_is_rebased_when_another_part_changes() {
    let parts = 4;
    let (mut doc, _, first, mut model) = library_parts(parts, true);
    // Socket cuts made now shape their tools around their own part's bodies (#219): only the
    // edited part is evaluated again.
    let FeatureKind::Extrude { extent, .. } = &mut doc.feature_mut(first).unwrap().kind else { panic!("extrude") };
    extent.distance = "5".into();
    model.evaluate(&doc);
    assert_eq!((evaluated(&model), model.last_rebased), (4, doc.features.len() - 2 - 4));
    assert_full_evaluation(&doc, &model);
}

#[test]
fn a_cut_named_from_the_geometry_around_it_is_evaluated_again() {
    let mut doc = Document::new("dimples");
    let unused = doc.add_feature(line_sketch(PlaneRef::Origin { name: "XZ".into() }, Vec2::new(5.0, 5.0)), None).unwrap();
    let mut add = |kind: FeatureKind, names: &[&str], participants: &[&str]| {
        let id = doc.add_feature(kind, None).unwrap();
        let f = doc.feature_mut(id).unwrap();
        f.body_names = names.iter().map(|n| n.to_string()).collect();
        f.participants = participants.iter().map(|n| n.to_string()).collect();
        id
    };
    let block = |x: f64, operation| FeatureKind::Box {
        corner: point_expr(Vec3::new(x, 0.0, 0.0)),
        length: "10".into(),
        width: "10".into(),
        height: "4".into(),
        operation,
    };
    add(block(0.0, Operation::NewBody), &["A"], &[]);
    add(block(20.0, Operation::NewBody), &["B"], &[]);
    // A sphere's faces have no roles: those it leaves in the cut are named from the geometry of
    // the bodies near them, which the cut does not read.
    let dimple =
        add(FeatureKind::Sphere { center: point_expr(Vec3::new(5.0, 5.0, 4.0)), radius: "2".into(), operation: Operation::Cut }, &[], &["A"]);
    let bore = FeatureKind::Box {
        corner: point_expr(Vec3::new(24.0, 4.0, -1.0)),
        length: "2".into(),
        width: "2".into(),
        height: "6".into(),
        operation: Operation::Cut,
    };
    add(bore, &[], &["B"]);
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results.iter().find(|r| r.error.is_some()));
    assert!(crate::naming::names(model.result(dimple).unwrap().state.body("A").unwrap()).geometric);
    // An edit before them, which changes no body: the dimple is evaluated again, the bore rebased.
    let Ok(sketch) = doc.sketch_mut(unused) else { panic!("sketch") };
    sketch.points[1].pos = Vec2::new(6.0, 5.0);
    model.evaluate(&doc);
    assert_eq!((evaluated(&model), model.last_rebased), (2, doc.features.len() - 2));
    assert_full_evaluation(&doc, &model);
}

#[test]
#[should_panic(expected = "rebase shadow check: MISMATCH")]
fn a_shadow_mismatch_is_counted_and_fails_the_tests() {
    let mut doc = Document::new("shadow");
    let kind =
        FeatureKind::Box { corner: point_expr(Vec3::ZERO), length: "1".into(), width: "1".into(), height: "1".into(), operation: Operation::NewBody };
    let id = doc.add_feature(kind, None).unwrap();
    let mut model = Model::new();
    model.evaluate(&doc);
    let (f, old) = (doc.feature(id).unwrap(), model.result(id).unwrap());
    let before = super::rebase::shadow_mismatches();
    let same = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::rebase::shadow_check(f, &old.state, old, &old.state, None, old.warning.as_deref(), &old.boolean_diagnostics)
    }));
    assert!(matches!(same, Ok(true)));
    // A rebased model without the box: logged, counted, and a failure in this crate's tests.
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::rebase::shadow_check(f, &ModelState::default(), old, &old.state, None, None, &old.boolean_diagnostics)
    }));
    assert!(super::rebase::shadow_mismatches() > before);
    if let Err(e) = failed {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn a_saved_model_keeps_which_names_came_from_geometry() {
    let mut doc = Document::new("dimple");
    let id = doc
        .add_feature(
            FeatureKind::Box {
                corner: point_expr(Vec3::ZERO),
                length: "10".into(),
                width: "10".into(),
                height: "4".into(),
                operation: Operation::NewBody,
            },
            None,
        )
        .unwrap();
    doc.feature_mut(id).unwrap().body_names = vec!["A".into()];
    doc.add_feature(FeatureKind::Sphere { center: point_expr(Vec3::new(5.0, 5.0, 4.0)), radius: "2".into(), operation: Operation::Cut }, None)
        .unwrap();
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(crate::naming::names(model.state().body("A").unwrap()).geometric);
    let mut saved = model.saved(&doc).unwrap();
    // Even where every face's name reads as verified, the flag is taken as saved (#217 review).
    for n in saved.records["names"].as_array_mut().unwrap() {
        for info in n["info"].as_array_mut().unwrap() {
            info[2] = true.into();
        }
    }
    let adopted = Model::adopt(&doc, &saved).unwrap();
    assert!(crate::naming::names(adopted.state().body("A").unwrap()).geometric);
}

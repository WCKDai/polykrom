//! Shell regressions found by the robustness fuzzer (tests/robustness/goldens.json).
//!
//! The box-on-box designs below have a saddle corner: where the top box's left face meets the
//! lower box's back face, four planar faces meet with convex and concave edges in turn (the
//! top box overhangs the lower box's back face). A shell splits that corner into two corners
//! joined by a new edge.
//!
//! Lower box `[0, 25] × [0, 17.5] × [0, 35]`, top box `[7.5, 22.5] × [0, 17.5 + o] × [35, 42.5]`.
//! With the bottom face open, a shell of thickness `t` leaves this cavity (for `o < 17.5 − 2t`):
//!
//! - in the lower box: `[t, 25 − t] × [t, 17.5 − t] × [0, 35 − t]`
//! - in the top box, over the lower box: `[7.5 + t, 22.5 − t] × [t, 17.5 − t] × [35 − t, 42.5 − t]`
//! - in the overhang: `[7.5 + t, 22.5 − t] × [17.5 − t, 17.5 + o − t] × [35 + t, 42.5 − t]`
//!
//! The shell's volume is the solid's volume less those three boxes.
use polykrom_engine::Session;
use serde_json::json;

fn run(s: &mut Session, id: &str, p: serde_json::Value) {
    if let Err(e) = s.execute(id, &p) {
        panic!("{id} {p}: {e}");
    }
}

/// The box on a box, overhanging the lower box's back face by `o`.
fn box_on_box(o: f64) -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"height": 35.0, "length": 25.0, "width": 17.5}));
    run(&mut s, "solid.box", json!({"corner": [7.5, 0.0, 35.0], "height": 7.5, "length": 15.0, "operation": "join", "width": 17.5 + o}));
    s
}

/// The shell's volume with the bottom face open (see the module comment).
fn open_bottom_shell_volume(o: f64, t: f64) -> f64 {
    let solid = 25.0 * 17.5 * 35.0 + 15.0 * (17.5 + o) * 7.5;
    let lower = (25.0 - 2.0 * t) * (17.5 - 2.0 * t) * (35.0 - t);
    let above = (15.0 - 2.0 * t) * (17.5 - 2.0 * t) * 7.5;
    let overhang = (15.0 - 2.0 * t) * o * (7.5 - 2.0 * t);
    solid - lower - above - overhang
}

/// Seed 83 (minimised): the overhang is 2.5 mm, and the shell opens the top box's left face.
/// The shell refused the saddle corner (`a corner whose faces don't meet the same way`).
#[test]
fn shell_box_on_box_overhanging_saddle_corner() {
    let mut s = box_on_box(2.5);
    run(&mut s, "solid.shell", json!({"faces": [[7.5, 17.5, 38.75]], "thickness": 1.5}));
    sound(&s);
}

/// The same saddle corner with the bottom open: every face round the corner moves, and the
/// volume is exact.
#[test]
fn shell_saddle_corner_volume_is_exact() {
    for t in [1.0, 1.5] {
        let mut s = box_on_box(2.5);
        run(&mut s, "solid.shell", json!({"faces": [[12.5, 8.75, 0.0]], "thickness": t}));
        sound(&s);
        let (got, want) = (volume(&s), open_bottom_shell_volume(2.5, t));
        assert!((got - want).abs() < 1e-6 * want, "thickness {t}: shell volume {got}, want {want}");
    }
}

/// An overhang (1 mm) thinner than the wall (1.5 mm): the overhang's cavity is only 1 mm deep.
/// A corner landing outside the solid, or a wall left thin, would change the volume.
#[test]
fn shell_saddle_corner_with_an_overhang_thinner_than_the_wall() {
    let mut s = box_on_box(1.0);
    run(&mut s, "solid.shell", json!({"faces": [[12.5, 8.75, 0.0]], "thickness": 1.5}));
    sound(&s);
    let (got, want) = (volume(&s), open_bottom_shell_volume(1.0, 1.5));
    assert!((got - want).abs() < 1e-6 * want, "shell volume {got}, want {want}");
}

fn volume(s: &Session) -> f64 {
    s.model.state().bodies.iter().map(|b| polycore::measure(&b.body).map(|m| m.volume).unwrap_or(f64::NAN)).sum()
}

fn sound(s: &Session) {
    let errors: Vec<_> = s.model.results.iter().filter_map(|r| r.error.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let st = s.model.state();
    assert_eq!(st.bodies.len(), 1, "one body");
    for b in &st.bodies {
        assert_eq!(b.body.shell_count(), 1, "one shell");
        assert!(b.body.validity().is_empty(), "{:?}", b.body.validity());
    }
}

/// A 60 × 40 × 30 box, the design of #297.
fn box_60_40_30() -> Session {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60.0, "width": 40.0, "height": 30.0}));
    s
}

/// The volume of the box shelled 2 mm thick with the faces whose extents are given left open:
/// the cavity reaches the open faces and stops 2 mm short of the closed ones.
fn shell_volume_open(open: &[&str]) -> f64 {
    let side = |lo: &str, hi: &str, len: f64| len - if open.contains(&lo) { 0.0 } else { 2.0 } - if open.contains(&hi) { 0.0 } else { 2.0 };
    60.0 * 40.0 * 30.0 - side("-x", "+x", 60.0) * side("-y", "+y", 40.0) * side("-z", "+z", 30.0)
}

/// #297: picks on the bottom edges of the back and left walls lie on two faces each. The shell
/// used to open the bottom as well (10 288 mm³ instead of 14 696 mm³) and say nothing; now the
/// command refuses the pick, naming both faces, and the design is as it was.
#[test]
fn shell_picks_on_edges_are_refused_as_ambiguous() {
    let mut s = box_60_40_30();
    let features = s.doc.features.len();
    let err = s.execute("solid.shell", &json!({"faces": [[30.0, 40.0, 0.0], [0.0, 20.0, 0.0]], "thickness": 2.0})).unwrap_err().to_string();
    assert!(err.contains("ambiguous pick at [30, 40, 0]"), "{err}");
    assert!(err.contains("`F1:+y`") && err.contains("`F1:-z`") && err.contains("`Body1`"), "{err}");
    assert_eq!(s.doc.features.len(), features, "no feature was added");
    sound(&s);
    let got = volume(&s);
    assert!((got - 72_000.0).abs() < 1e-9, "the box is untouched: {got}");
}

/// The same shell picked inside the back and left faces: 14 696 mm³ as before, and the reply
/// names the faces it used.
#[test]
fn shell_interior_picks_keep_their_volume_and_name_the_faces_used() {
    let mut s = box_60_40_30();
    let reply =
        s.execute("solid.shell", &json!({"faces": [[30.0, 40.0, 15.0], [0.0, 20.0, 15.0]], "thickness": 2.0})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reply["faces_used"], json!(["F1:+y", "F1:-x"]), "{reply}");
    assert_eq!(reply["warning"], json!(null), "{reply}");
    sound(&s);
    let (got, want) = (volume(&s), shell_volume_open(&["+y", "-x"]));
    assert!((got - want).abs() < 1e-9 && (want - 14_696.0).abs() < 1e-9, "shell volume {got}, want {want}");
}

/// Two picks on one face are not deduplicated in silence: the reply says so, and the face is
/// opened once.
#[test]
fn shell_two_picks_on_one_face_warn() {
    let mut s = box_60_40_30();
    let reply =
        s.execute("solid.shell", &json!({"faces": [[30.0, 40.0, 15.0], [10.0, 40.0, 5.0]], "thickness": 2.0})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reply["faces_used"], json!(["F1:+y", "F1:+y"]), "{reply}");
    let warning = reply["warning"].as_str().unwrap_or_default();
    assert!(warning.contains("faces 1 and 2") && warning.contains("`F1:+y`"), "{reply}");
    sound(&s);
    let (got, want) = (volume(&s), shell_volume_open(&["+y"]));
    assert!((got - want).abs() < 1e-9, "shell volume {got}, want {want}");
}

/// An edge pick with a `normal` is the face facing that way: the same shell as the interior
/// picks make.
#[test]
fn shell_edge_picks_with_a_normal_are_unambiguous() {
    let mut s = box_60_40_30();
    let faces = json!([{"point": [30.0, 40.0, 0.0], "normal": [0.0, 1.0, 0.0]}, {"point": [0.0, 20.0, 0.0], "normal": [-1.0, 0.0, 0.0]}]);
    let reply = s.execute("solid.shell", &json!({"faces": faces, "thickness": 2.0})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reply["faces_used"], json!(["F1:+y", "F1:-x"]), "{reply}");
    assert_eq!(reply["warning"], json!(null), "{reply}");
    sound(&s);
    let (got, want) = (volume(&s), shell_volume_open(&["+y", "-x"]));
    assert!((got - want).abs() < 1e-9, "shell volume {got}, want {want}");
    // A normal that tells the two faces apart no better than the point did is still refused.
    let mut s = box_60_40_30();
    let err = s
        .execute("solid.shell", &json!({"faces": [{"point": [30.0, 40.0, 0.0], "normal": [0.0, 1.0, -1.0]}], "thickness": 2.0}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("ambiguous pick at [30, 40, 0]"), "{err}");
}

/// Faces given by name are never ambiguous, and the reply lists the names it used.
#[test]
fn shell_by_face_names_lists_the_faces_used() {
    let mut s = box_60_40_30();
    let reply = s.execute("solid.shell", &json!({"faces": ["F1:+y", "F1:-x"], "thickness": 2.0})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reply["faces_used"], json!(["F1:+y", "F1:-x"]), "{reply}");
    sound(&s);
    let (got, want) = (volume(&s), shell_volume_open(&["+y", "-x"]));
    assert!((got - want).abs() < 1e-9, "shell volume {got}, want {want}");
}

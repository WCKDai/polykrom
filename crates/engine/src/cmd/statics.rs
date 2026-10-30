//! Does it stand, and can its actuators hold it (#164; the robot dog, #101): the design's mass and
//! centre of mass over its support polygon, and the static gravity load at each revolute, slider
//! and cylindrical joint against the rated actuator driving it, now or over a motion study.
//! Statics only (dynamics: #32). A joint carries its far side, away from what holds the design
//! up: the grounded parts (and the root), or the feet named as `support`, each pushing up at the
//! centre of its contact. Occurrences hang on their parent's unless a joint places them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use polycore_geom::{Vec2, Vec3};
use polykrom_doc::joints::{self, JointKind};
use polykrom_doc::{Document, IDENTITY, Mat, apply_point, apply_vector, mat_mul};
use polykrom_sketch::linalg;
use serde_json::{Value, json};

use super::CommandSpec;
use super::joints::occurrence_id;
use crate::params::{bad, num};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("inspect.statics", "Statics", statics).icon("center_of_mass").noundo().params(
    "support?: [occurrences] (the feet: they stand on the ground and hold the design up in place of the grounded parts; default \
     none: the grounded parts hold it and every body may touch the ground); ground?: z of the ground in mm (default the lowest \
     point of the support, or of the design); contact_tol?: mm above the ground a point still touches it (default 0.5); gravity?: \
     m/s² along −Z (default 9.81); margin?: the share of its rating a joint should stay under (default 0.5); study?: id|name (every \
     sample of a motion study, played on a copy as motion.play plays it), with samples? (2…1000, default the study's steps + 1) → \
     mass_kg, com_mm, held_by: [occurrences], support: {ground_mm, points (touching), polygon: [[x,y]…] (their convex hull), inside \
     (the centre of mass over it, seen from above), margin_mm (to its nearest edge, negative outside; a point or a line holds \
     nothing up: inside false, margin ≤ 0)}, joints: [{joint, name, kind, load_nm (revolute, cylindrical: the torque gravity puts \
     on the joint's b side about a's z, N·m, right-handed; the actuator holds the opposite) | load_n (slider, cylindrical: the force \
     along it, N), rated_nm (the stall or holding torque of the library actuator at an end of the joint, as the MJCF export finds \
     it, else of one fixed to either end (by rigid joints or seats, or hanging on its parent's occurrence) with its output axis \
     on the joint's, within 0.5 mm and 0.5°; null: none), utilisation (|load_nm| / rated_nm), over (above 1), above_margin, \
     indeterminate? (load null: the joint closes a loop, or what holds the design is on both its sides)}] (worst first). A joint \
     carries the occurrences on its far side from what holds the design; with support, each foot pushes up at the centre of its \
     contact, the pushes balancing the weight about the centre of mass (the least-squares share with four feet or more), and a \
     joint with feet on both sides carries its b side's weight less their push. With study (the design is not changed): steps, margin_mm: [per sample], min_margin_mm, \
     min_margin_step, com_mm there, each joint at its worst_step, stopped_at, stopped_by_loop, stopped_by_contact. Mesh bodies \
     weigh nothing",
)];

/// Most support occurrences, ground contact candidates (tessellation vertices) one pose looks at,
/// and steps finding the joints' sides.
const MAX_SUPPORT: usize = 1000;
const MAX_VERTICES: usize = 2_000_000;
const MAX_SIDE_WORK: usize = 50_000_000;
/// How near an actuator's output axis must be to a joint's axis to drive it: within this
/// distance (mm) of its line, and within this angle (0.5°, in radians) of parallel either way.
const AXIS_TOL_MM: f64 = 0.5;
const AXIS_TOL_RAD: f64 = 0.5 * std::f64::consts::PI / 180.0;

/// A joint whose load is worked out.
struct Load {
    id: u64,
    name: String,
    kind: JointKind,
    /// A's occurrence and its joint frame there (component coordinates): the axis is its z.
    a: u64,
    frame: Mat,
    /// The occurrences that hang on the joint, with +1 when they are B's side (−1: A's, so the
    /// load on B is the opposite). None: statics alone can't split it.
    side: Option<(BTreeSet<u64>, f64)>,
    rated: Option<f64>,
}

/// What stays the same from pose to pose.
struct Setup {
    /// Per component: its bodies' mass (kg) and centre of mass (component coordinates).
    mass: BTreeMap<u64, (f64, Vec3)>,
    /// Per component that may touch the ground: its bodies' tessellation vertices.
    verts: BTreeMap<u64, Vec<Vec3>>,
    support: Option<BTreeSet<u64>>,
    held: BTreeSet<u64>,
    loads: Vec<Load>,
}

struct Options {
    g: f64,
    ground: Option<f64>,
    tol: f64,
}

/// One pose's answer.
struct Pose {
    mass: f64,
    com: Vec3,
    ground: Option<f64>,
    points: usize,
    polygon: Vec<Vec2>,
    inside: bool,
    margin: Option<f64>,
    /// Per joint: the torque (N·m) and force (N) on B about and along the axis; None when
    /// indeterminate.
    loads: Vec<Option<(f64, f64)>>,
}

fn statics(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "inspect.statics";
    let in_range = |k: &str, default: f64, lo: f64, hi: f64| -> Result<f64> {
        match p.get(k).filter(|v| !v.is_null()) {
            None => Ok(default),
            Some(_) => num(p, k).filter(|v| v.is_finite() && (lo..=hi).contains(v)).ok_or_else(|| bad(cmd, format!("`{k}` must be {lo} to {hi}"))),
        }
    };
    let o = Options {
        g: in_range("gravity", 9.81, 0.0, 1000.0)?,
        tol: in_range("contact_tol", 0.5, 0.0, 1000.0)?,
        ground: if p.get("ground").is_some_and(|v| !v.is_null()) { Some(in_range("ground", 0.0, -1e9, 1e9)?) } else { None },
    };
    let margin = in_range("margin", 0.5, 0.0, 1.0)?;
    let support = match p.get("support").filter(|v| !v.is_null()) {
        None => None,
        Some(Value::Array(list)) if !list.is_empty() && list.len() <= MAX_SUPPORT => {
            Some(list.iter().map(|v| occurrence_id(s, Some(v), cmd)).collect::<Result<BTreeSet<u64>>>()?)
        }
        Some(_) => return Err(bad(cmd, format!("`support` must list 1…{MAX_SUPPORT} occurrences (the feet)"))),
    };
    let setup = setup(s, support, cmd)?;
    let name = |o: u64| {
        if o == 0 { "root".to_string() } else { s.doc.occurrences.iter().find(|x| x.id == o).map_or_else(|| o.to_string(), |x| x.name.clone()) }
    };
    let held_by: Vec<String> = setup.held.iter().filter(|o| **o != 0 || setup.support.is_some()).map(|o| name(*o)).collect();
    if p.get("study").is_none_or(Value::is_null) {
        let pose = pose(&setup, &s.doc, &o, cmd)?;
        let worst = pose.loads.iter().map(|l| l.map(|l| (l, None))).collect();
        let mut out = answer(&setup, &pose, worst, margin);
        out["held_by"] = json!(held_by);
        return Ok(out);
    }
    let m = s.doc.assembly.studies.get(super::motion::study_index(s, p, cmd)?).cloned().ok_or_else(|| bad(cmd, "study"))?;
    let n = super::motion::study_samples(p, &m, 2, cmd)?;
    let (mut margins, mut steps) = (Vec::new(), Vec::new());
    let mut lowest: Option<(f64, f64, Pose)> = None;
    let mut worst: Vec<Option<((f64, f64), Option<f64>)>> = vec![None; setup.loads.len()];
    let stop = super::motion::play_study(s, &m, n, true, |t, k| {
        let pose = pose(&setup, &t.doc, &o, cmd)?;
        let step = super::motion::study_step(&m, k, n);
        for ((now, w), l) in pose.loads.iter().zip(worst.iter_mut()).zip(&setup.loads) {
            if let Some(now) = now
                && w.is_none_or(|(was, _)| size(l.kind, *now) > size(l.kind, was))
            {
                *w = Some((*now, Some(step)));
            }
        }
        steps.push(step);
        margins.push(pose.margin);
        if let Some(mm) = pose.margin
            && lowest.as_ref().is_none_or(|(low, _, _)| mm < *low)
        {
            lowest = Some((mm, step, pose));
        }
        Ok(true)
    })?;
    // The centre of mass and support where the margin is least, else as the design stands.
    let (min_step, at) = match lowest {
        Some((mm, step, pose)) => (Some((mm, step)), pose),
        None => (None, pose(&setup, &s.doc, &o, cmd)?),
    };
    let mut out = answer(&setup, &at, worst, margin);
    out["held_by"] = json!(held_by);
    out["study"] = json!(m.name);
    out["steps"] = json!(steps);
    out["margin_mm"] = json!(margins);
    out["min_margin_mm"] = json!(min_step.map(|x| x.0));
    out["min_margin_step"] = json!(min_step.map(|x| x.1));
    for k in ["stopped_at", "stopped_by_loop", "stopped_by_contact"] {
        out[k] = stop[k].clone();
    }
    Ok(out)
}

/// How large a load is for its joint: the torque of a turning joint, the force of a slider.
fn size(kind: JointKind, (torque, force): (f64, f64)) -> f64 {
    if kind == JointKind::Slider { force.abs() } else { torque.abs() }
}

/// The reply: the pose's mass and support, and each joint's load (`loads`: with the study step it
/// is worst at), worst first.
fn answer(setup: &Setup, pose: &Pose, loads: Vec<Option<((f64, f64), Option<f64>)>>, margin: f64) -> Value {
    let mut rows: Vec<(Option<f64>, f64, Value)> = setup
        .loads
        .iter()
        .zip(loads)
        .map(|(l, load)| {
            let turns = matches!(l.kind, JointKind::Revolute | JointKind::Cylindrical);
            let util = load.zip(l.rated).filter(|_| turns).map(|((x, _), r)| x.0.abs() / r);
            let mut v = json!({"joint": l.id, "name": l.name, "kind": l.kind, "utilisation": util,
                "over": util.is_some_and(|u| u > 1.0), "above_margin": util.is_some_and(|u| u > margin)});
            if turns {
                v["load_nm"] = json!(load.map(|x| x.0.0));
                v["rated_nm"] = json!(l.rated);
            }
            if l.kind != JointKind::Revolute {
                v["load_n"] = json!(load.map(|x| x.0.1));
            }
            match load {
                Some((_, Some(step))) => v["worst_step"] = json!(step),
                None => v["indeterminate"] = json!(true),
                _ => {}
            }
            (util, load.map_or(-1.0, |x| size(l.kind, x.0)), v)
        })
        .collect();
    rows.sort_by(|a, b| b.0.unwrap_or(-1.0).total_cmp(&a.0.unwrap_or(-1.0)).then(b.1.total_cmp(&a.1)));
    json!({
        "mass_kg": pose.mass, "com_mm": [pose.com.x, pose.com.y, pose.com.z],
        "support": {"ground_mm": pose.ground, "points": pose.points, "polygon": pose.polygon.iter().map(|q| [q.x, q.y]).collect::<Vec<_>>(),
            "inside": pose.inside, "margin_mm": pose.margin},
        "joints": rows.into_iter().map(|r| r.2).collect::<Vec<_>>(),
    })
}

/// Masses, contact candidates, what holds the design and each joint's far side.
fn setup(s: &Session, support: Option<BTreeSet<u64>>, cmd: &str) -> Result<Setup> {
    let (doc, st) = (&s.doc, s.model.state());
    // The components that may touch the ground: those placed under a support occurrence, or all.
    let touching: BTreeSet<u64> = doc
        .placements()
        .into_iter()
        .filter(|(_, path, _)| support.as_ref().is_none_or(|sup| path.iter().any(|o| sup.contains(o))))
        .map(|(comp, _, _)| comp)
        .collect();
    let mut mass: BTreeMap<u64, (f64, Vec3)> = BTreeMap::new();
    let mut verts: BTreeMap<u64, Vec<Vec3>> = BTreeMap::new();
    for b in st.bodies.iter().filter(|b| !b.body.is_mesh()) {
        let comp = doc.model_body_component(b);
        let offset = doc.model_body_offset(b).copied().unwrap_or(IDENTITY);
        let measured = polycore::measure(&b.body)?;
        // g/cm³ × mm³ → kg.
        let kg = measured.volume * doc.density(&b.name) * 1e-6;
        let c = apply_point(&offset, measured.centroid);
        if !(kg.is_finite() && c.is_finite()) {
            return Err(bad(cmd, format!("body `{}` has no finite mass", b.name)));
        }
        let e = mass.entry(comp).or_insert((0.0, Vec3::ZERO));
        e.0 += kg;
        e.1 += c * kg;
        if touching.contains(&comp) {
            verts.entry(comp).or_default().extend(b.mesh().positions.iter().map(|v| apply_point(&offset, *v)));
        }
    }
    for e in mass.values_mut().filter(|e| e.0 > 0.0) {
        e.1 = e.1 / e.0;
    }
    // The joint graph, with occurrences no joint places hanging on their parent's.
    let live: Vec<&joints::Joint> =
        doc.assembly.joints.iter().filter(|j| !j.suppressed && j.a.occurrence != j.b.occurrence).take(joints::MAX_JOINTS).collect();
    let jointed: BTreeSet<u64> = live.iter().flat_map(|j| [j.a.occurrence, j.b.occurrence]).collect();
    let mut graph: BTreeMap<u64, Vec<(u64, Option<u64>)>> = BTreeMap::new();
    let mut link = |a: u64, b: u64, joint: Option<u64>| {
        graph.entry(a).or_default().push((b, joint));
        graph.entry(b).or_default().push((a, joint));
    };
    for j in &live {
        link(j.a.occurrence, j.b.occurrence, Some(j.id));
    }
    for o in doc.occurrences.iter().filter(|o| o.parent != 0 && !jointed.contains(&o.id)) {
        if let Some(parent) = doc.occurrence_of(o.parent) {
            link(o.id, parent.id, None);
        }
    }
    let held: BTreeSet<u64> = match &support {
        Some(sup) => sup.clone(),
        None => {
            let mut h: BTreeSet<u64> = std::iter::once(0).chain(doc.occurrences.iter().filter(|o| o.grounded).map(|o| o.id)).collect();
            // As the solver places them: with nothing held among the joints, the first one's A side stays.
            if !live.iter().any(|j| h.contains(&j.a.occurrence) || h.contains(&j.b.occurrence))
                && let Some(j) = live.first()
            {
                h.insert(j.a.occurrence);
            }
            h
        }
    };
    let rated = super::parts::rated_torques(doc);
    let rigid: BTreeSet<u64> = live.iter().filter(|j| j.kind == JointKind::Rigid).map(|j| j.id).collect();
    let mut work = MAX_SIDE_WORK;
    let mut loads = Vec::new();
    for j in live.iter().filter(|j| matches!(j.kind, JointKind::Revolute | JointKind::Slider | JointKind::Cylindrical)) {
        // A joint whose origin is lost is left out, as the solver leaves it out.
        let Ok(frame) = joints::resolve_origin(doc, &st, &j.a) else { continue };
        let too_big = || bad(cmd, "too many joints and occurrences to work out which side of each joint hangs on it");
        let b_side = side(&graph, j.b.occurrence, |e| e != Some(j.id), &mut work).ok_or_else(too_big)?;
        // A slider's actuator would be rated in newtons, which no library part gives yet.
        let rated = match rated.get(&j.id) {
            _ if j.kind == JointKind::Slider => None,
            Some(t) => Some(*t),
            None => {
                let fixed = |e: Option<u64>| e.is_none_or(|id| rigid.contains(&id));
                let mut group = side(&graph, j.a.occurrence, fixed, &mut work).ok_or_else(too_big)?;
                group.extend(side(&graph, j.b.occurrence, fixed, &mut work).ok_or_else(too_big)?);
                driving(doc, &group, j.a.occurrence, &frame)
            }
        };
        let side = if b_side.contains(&j.a.occurrence) {
            None
        } else {
            let a_side = side(&graph, j.a.occurrence, |e| e != Some(j.id), &mut work).ok_or_else(too_big)?;
            match (!b_side.is_disjoint(&held), !a_side.is_disjoint(&held)) {
                (false, true) => Some((b_side, 1.0)),
                (true, false) => Some((a_side, -1.0)),
                // Feet on both sides: B's carries its weight less their push.
                (true, true) if support.is_some() => Some((b_side, 1.0)),
                _ => None,
            }
        };
        loads.push(Load { id: j.id, name: j.name.clone(), kind: j.kind, a: j.a.occurrence, frame, side, rated });
    }
    Ok(Setup { mass, verts, support, held, loads })
}

/// The rated torque of the library actuator among `group` (the occurrences fixed to a joint's
/// ends) whose output axis lies on the joint's axis, the z of `frame` on occurrence `a`: the
/// nearest to it, within `AXIS_TOL_MM` and `AXIS_TOL_RAD`.
fn driving(doc: &Document, group: &BTreeSet<u64>, a: u64, frame: &Mat) -> Option<f64> {
    let world = mat_mul(&if a == 0 { IDENTITY } else { joints::occurrence_world(doc, a) }, frame);
    let (at, axis) = (apply_point(&world, Vec3::ZERO), apply_vector(&world, Vec3::Z).normalized()?);
    group
        .iter()
        .filter_map(|o| {
            let occ = doc.occurrences.iter().find(|x| x.id == *o)?;
            let (torque, point, direction) = super::parts::rated_output(doc, occ.component)?;
            let place = joints::occurrence_world(doc, *o);
            let (p, d) = (apply_point(&place, point) - at, apply_vector(&place, direction).normalized()?);
            let off = (p - axis * p.dot(axis)).len();
            (off <= AXIS_TOL_MM && d.cross(axis).len() <= AXIS_TOL_RAD.sin()).then_some((off, torque))
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|x| x.1)
}

/// The occurrences reached from `from` along the links `cross` lets through (a joint's id, None
/// for an occurrence hanging on its parent's; None: out of `work`).
fn side(graph: &BTreeMap<u64, Vec<(u64, Option<u64>)>>, from: u64, cross: impl Fn(Option<u64>) -> bool, work: &mut usize) -> Option<BTreeSet<u64>> {
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(n) = queue.pop_front() {
        for (next, joint) in graph.get(&n).into_iter().flatten() {
            *work = work.checked_sub(1)?;
            if cross(*joint) && seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    Some(seen)
}

/// The design as `doc` places it: its centre of mass, support and joint loads.
fn pose(setup: &Setup, doc: &Document, o: &Options, cmd: &str) -> Result<Pose> {
    // Per occurrence: mass (kg) and its first moment (kg·mm, world).
    let mut nodes: BTreeMap<u64, (f64, Vec3)> = BTreeMap::new();
    // Contact candidates: (the foot they belong to, or 0 without support; world point).
    let mut candidates: Vec<(u64, Vec3)> = Vec::new();
    for (comp, path, world) in doc.placements() {
        if let Some((kg, c)) = setup.mass.get(&comp) {
            let e = nodes.entry(path.last().copied().unwrap_or(0)).or_insert((0.0, Vec3::ZERO));
            e.0 += kg;
            e.1 += apply_point(&world, *c) * *kg;
        }
        let foot = match &setup.support {
            None => Some(0),
            Some(sup) => path.iter().rev().find(|x| sup.contains(x)).copied(),
        };
        if let (Some(foot), Some(vs)) = (foot, setup.verts.get(&comp)) {
            if candidates.len().saturating_add(vs.len()) > MAX_VERTICES {
                return Err(bad(cmd, format!("more than {MAX_VERTICES} points may touch the ground: name the feet in `support`")));
            }
            candidates.extend(vs.iter().map(|v| (foot, apply_point(&world, *v))));
        }
    }
    let mass: f64 = nodes.values().map(|n| n.0).sum();
    if mass <= 0.0 || !mass.is_finite() {
        return Err(bad(cmd, "the design has no solid bodies to weigh"));
    }
    let com = nodes.values().fold(Vec3::ZERO, |a, n| a + n.1) / mass;
    let ground = o.ground.or_else(|| candidates.iter().map(|(_, q)| q.z).min_by(f64::total_cmp));
    let contact: Vec<(u64, Vec3)> = match ground {
        Some(z) => candidates.into_iter().filter(|(_, q)| q.z <= z + o.tol).collect(),
        None => Vec::new(),
    };
    let polygon = hull(contact.iter().map(|(_, q)| Vec2::new(q.x, q.y)).collect());
    let (inside, margin) = margin(&polygon, Vec2::new(com.x, com.y));
    // Each foot's push, at the centre of its contact.
    let mut feet: BTreeMap<u64, (Vec3, f64)> = BTreeMap::new();
    if setup.support.is_some() {
        for (foot, q) in &contact {
            let e = feet.entry(*foot).or_insert((Vec3::ZERO, 0.0));
            e.0 += Vec3::new(q.x, q.y, ground.unwrap_or(q.z));
            e.1 += 1.0;
        }
    }
    let feet: Vec<(u64, Vec3)> = feet.into_iter().map(|(f, (sum, n))| (f, sum / n)).collect();
    let push = reactions(&feet, com, mass * o.g);
    let down = Vec3::new(0.0, 0.0, -o.g);
    let loads = setup
        .loads
        .iter()
        .map(|l| {
            let (set, sign) = l.side.as_ref()?;
            let frame = mat_mul(&if l.a == 0 { IDENTITY } else { joints::occurrence_world(doc, l.a) }, &l.frame);
            let (at, axis) = (apply_point(&frame, Vec3::ZERO), apply_vector(&frame, Vec3::Z).normalized()?);
            let (kg, first) = set.iter().filter_map(|n| nodes.get(n)).fold((0.0, Vec3::ZERO), |(m, f), n| (m + n.0, f + n.1));
            // Σ (c − at) × m g, in N·mm, and Σ m g in N; the feet on this side push up.
            let (mut torque, mut force) = ((first - at * kg).cross(down), down * kg);
            for ((_, q), up) in feet.iter().zip(&push).filter(|((f, _), _)| set.contains(f)) {
                let up = Vec3::new(0.0, 0.0, *up);
                torque += (*q - at).cross(up);
                force += up;
            }
            Some((sign * torque.dot(axis) * 1e-3, sign * force.dot(axis)))
        })
        .collect();
    Ok(Pose { mass, com, ground, points: contact.len(), polygon, inside, margin, loads })
}

/// Each foot's upward push (N) holding weight `w` (N) up at `com`: pushes that add up to it with
/// no moment about the centre of mass, the least-squares ones with more than three feet or with
/// feet in a line.
fn reactions(feet: &[(u64, Vec3)], com: Vec3, w: f64) -> Vec<f64> {
    let rows: Vec<[f64; 3]> = feet.iter().map(|(_, q)| [1.0, q.x - com.x, q.y - com.y]).collect();
    let mut m = linalg::Mat::zeros(3, 3);
    for r in &rows {
        for (i, ri) in r.iter().enumerate() {
            for (k, rk) in r.iter().enumerate() {
                m.add(i, k, ri * rk);
            }
        }
    }
    // Pushes = rowsᵀ λ with (rows rowsᵀ + ε) λ = (w, 0, 0): the least-norm solution, which the
    // small ε (relative to each row's scale: 1 against mm²) keeps finite where the feet leave a
    // direction free.
    let tiny = 1e-12 * (0..3).map(|i| m.get(i, i)).fold(1.0, f64::max);
    for i in 0..3 {
        m.add(i, i, 1e-10 * m.get(i, i) + tiny);
    }
    let l = linalg::solve(m, vec![w, 0.0, 0.0]).unwrap_or_default();
    rows.iter().map(|r| r.iter().zip(&l).map(|(a, b)| a * b).sum()).collect()
}

/// The convex hull of `p`, anticlockwise (fewer than three points: the distinct ones, or a line's
/// two ends).
fn hull(mut p: Vec<Vec2>) -> Vec<Vec2> {
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
    if p.len() < 3 {
        return p;
    }
    let chain = |pts: &mut dyn Iterator<Item = &Vec2>| {
        let mut out: Vec<Vec2> = Vec::new();
        for q in pts {
            while let [.., a, b] = out.as_slice()
                && (*b - *a).cross(*q - *a) <= 0.0
            {
                out.pop();
            }
            out.push(*q);
        }
        out.pop();
        out
    };
    let mut lower = chain(&mut p.iter());
    lower.extend(chain(&mut p.iter().rev()));
    lower
}

/// Whether `c` is over the polygon (convex, anticlockwise) and how far inside its nearest edge it
/// is (negative: how far outside the polygon). A point or a line has no inside.
fn margin(poly: &[Vec2], c: Vec2) -> (bool, Option<f64>) {
    if poly.is_empty() {
        return (false, None);
    }
    let edges: Vec<(Vec2, Vec2)> = poly.iter().zip(poly.iter().cycle().skip(1)).map(|(a, b)| (*a, *b)).collect();
    let outside = edges
        .iter()
        .map(|(a, b)| {
            let d = *b - *a;
            let t = if d.len2() > 0.0 { ((c - *a).dot(d) / d.len2()).clamp(0.0, 1.0) } else { 0.0 };
            (c - (*a + d * t)).len()
        })
        .fold(f64::INFINITY, f64::min);
    let area = edges.iter().map(|(a, b)| a.cross(*b)).sum::<f64>() / 2.0;
    let span = edges.iter().map(|(a, b)| (*b - *a).len()).fold(0.0, f64::max);
    let inner = edges.iter().filter_map(|(a, b)| Some((*b - *a).normalized()?.cross(c - *a))).fold(f64::INFINITY, f64::min);
    if area > 1e-9 * span * span && inner >= 0.0 { (true, Some(inner)) } else { (false, Some(if outside > 0.0 { -outside } else { 0.0 })) }
}

#[cfg(test)]
#[path = "statics_tests.rs"]
mod tests;

//! MJCF (MuJoCo) model of a design's assembly (#27, docs/mjcf.md). The bodies, joints, meshes,
//! masses and motion links are the link tree the URDF export builds ([`crate::urdf()`]), written
//! as nested `<body>` elements in metres, with a position actuator per moving joint.
//!
//! Each body is written where Polykrom shows it: a body's frame is its parent's composed with
//! the joint's origin and its motion at the design's current value, and the joint's `ref` is
//! that value. So the model loads in the design's pose, and every joint value (range, `ref`,
//! control) equals Polykrom's, in radians or metres.

use std::collections::BTreeMap;

use solidcraft_doc::{Document, IDENTITY, Mat, ModelState, joints, mat_mul};

use crate::Result;
use crate::urdf::{self, Inertial, Link, Step, UrdfJoint, UrdfModel, UrdfOptions, attr, num, triple};

/// Joint damping when not given (N m s/rad turning, N s/m sliding): the passive losses of a
/// small geared joint. The actuators damp their own motion (`dampratio`), so this only slows
/// joints no actuator holds (a motion link's follower) and keeps them from swinging forever.
pub const DEFAULT_DAMPING: f64 = 0.1;
/// Joint armature when not given (kg m² turning, kg sliding): the rotor inertia a gearbox
/// reflects onto the joint, of the order of a hobby servo's. It also keeps a joint chain's
/// massless in-between bodies stable at MuJoCo's default 2 ms step.
pub const DEFAULT_ARMATURE: f64 = 0.01;
/// Position actuator gain when not given (N m/rad turning, N/m sliding): a stiff servo, which
/// holds its target within a degree until its force limit (the part's rating) is reached. Its
/// damping is critical (`dampratio="1"`), so the stiffness never rings.
pub const DEFAULT_KP: f64 = 1000.0;
/// Most body pairs a fixed base's contact exclusions may list.
const MAX_EXCLUDES: usize = 10_000;

/// Choices for [`mjcf()`] (`file.export` parameters).
#[derive(Clone, Debug)]
pub struct MjcfOptions {
    /// A free root (`<freejoint/>`), so the robot falls onto the floor. Default: when nothing
    /// is grounded.
    pub floating: Option<bool>,
    /// A floor plane at z = 0 and a light.
    pub scene: bool,
    pub kp: Option<f64>,
    pub damping: Option<f64>,
    pub armature: Option<f64>,
    /// Actuator force limit (N or N m) of a joint no rated actuator drives.
    pub effort: Option<f64>,
    /// The rated torque (N m) of the actuator driving a turning joint, by Polykrom joint id.
    pub torques: BTreeMap<u64, f64>,
}

impl Default for MjcfOptions {
    fn default() -> Self {
        Self { floating: None, scene: true, kp: None, damping: None, armature: None, effort: None, torques: BTreeMap::new() }
    }
}

/// The MJCF, its mesh files (name next to the `.xml`, bytes) and what was approximated.
#[derive(Clone, Debug)]
pub struct MjcfPackage {
    pub xml: String,
    pub meshes: Vec<(String, Vec<u8>)>,
    pub model: UrdfModel,
    pub warnings: Vec<String>,
    pub summary: MjcfSummary,
}

/// What the model holds beyond its bodies, for the export's reply (#171): an agent can't read
/// the file to check it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MjcfSummary {
    /// Hinge and slide joints (the free root's joint not counted).
    pub joints: usize,
    /// The root body has a `<freejoint/>`.
    pub free_root: bool,
    /// Every body's mass together (kg), as MuJoCo reads it.
    pub mass: f64,
    pub actuators: Vec<MjcfActuator>,
    /// The `design` keyframe (the design's pose, actuators holding it) was written.
    pub keyframe: bool,
}

/// A position actuator: its joint, force limit (N m turning, N sliding) and control range
/// (rad or m; `None` when the joint has no limits).
#[derive(Clone, Debug, PartialEq)]
pub struct MjcfActuator {
    pub name: String,
    pub joint: String,
    pub force: (f64, f64),
    pub ctrl: Option<(f64, f64)>,
}

fn positive(x: Option<f64>) -> Option<f64> {
    x.filter(|x| x.is_finite() && *x > 0.0)
}

/// `pos` (m) and `quat` (w x y z) of a frame (mm).
fn pos_quat(m: &Mat) -> String {
    let r = |row: usize, col: usize| m.get(col).and_then(|c| c.get(row)).copied().unwrap_or(0.0);
    let t = m.get(3).map_or([0.0; 3], |c| [c[0], c[1], c[2]]);
    // Shepperd: from the largest of w, x, y, z, so the square root never nears zero.
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    let q = if trace > 0.0 {
        let s = 2.0 * (trace + 1.0).sqrt();
        [s / 4.0, (r(2, 1) - r(1, 2)) / s, (r(0, 2) - r(2, 0)) / s, (r(1, 0) - r(0, 1)) / s]
    } else if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
        let s = 2.0 * (1.0 + r(0, 0) - r(1, 1) - r(2, 2)).sqrt();
        [(r(2, 1) - r(1, 2)) / s, s / 4.0, (r(0, 1) + r(1, 0)) / s, (r(0, 2) + r(2, 0)) / s]
    } else if r(1, 1) > r(2, 2) {
        let s = 2.0 * (1.0 + r(1, 1) - r(0, 0) - r(2, 2)).sqrt();
        [(r(0, 2) - r(2, 0)) / s, (r(0, 1) + r(1, 0)) / s, s / 4.0, (r(1, 2) + r(2, 1)) / s]
    } else {
        let s = 2.0 * (1.0 + r(2, 2) - r(0, 0) - r(1, 1)).sqrt();
        [(r(1, 0) - r(0, 1)) / s, (r(0, 2) + r(2, 0)) / s, (r(1, 2) + r(2, 1)) / s, s / 4.0]
    };
    let q = if q.iter().all(|x| x.is_finite()) { q } else { [1.0, 0.0, 0.0, 0.0] };
    format!("pos=\"{} {} {}\" quat=\"{} {} {} {}\"", num(t[0] * 1e-3), num(t[1] * 1e-3), num(t[2] * 1e-3), num(q[0]), num(q[1]), num(q[2]), num(q[3]))
}

fn inertial_xml(i: &Inertial) -> String {
    let t = &i.inertia;
    format!(
        "<inertial pos=\"{}\" mass=\"{}\" fullinertia=\"{} {} {} {} {} {}\"/>",
        triple(i.com * 1e-3),
        num(i.mass),
        num(t[0][0]),
        num(t[1][1]),
        num(t[2][2]),
        num(t[0][1]),
        num(t[0][2]),
        num(t[1][2])
    )
}

/// Build the MJCF of a design: the arguments are [`crate::urdf()`]'s; `stem` names the model and
/// prefixes the mesh files.
pub fn mjcf(doc: &Document, st: &ModelState, stem: &str, options: &MjcfOptions) -> Result<MjcfPackage> {
    let effort = positive(options.effort);
    let urdf_options =
        UrdfOptions { mesh_prefix: String::new(), effort: Some(effort.unwrap_or(urdf::DEFAULT_EFFORT)), velocity: Some(urdf::DEFAULT_VELOCITY) };
    let pkg = urdf::urdf(doc, st, stem, &urdf_options)?;
    let mut warnings = pkg.warnings;
    let model = pkg.model;
    let floating = options.floating.unwrap_or(!doc.occurrences.iter().any(|o| o.grounded));
    let values = joints::joint_values(&doc.assembly);
    // The joint's value at the design's state, in MJCF units.
    let value = |j: &UrdfJoint| {
        let q = j.source.and_then(|(id, k)| values.get(&id)?.get(k).copied()).filter(|q| q.is_finite()).unwrap_or(0.0);
        if j.step == Step::Prismatic { q * 1e-3 } else { q }
    };

    let mut s = String::from("<?xml version=\"1.0\"?>\n<!-- Exported by Polykrom; units: m, kg, s, rad -->\n");
    s += &format!("<mujoco model=\"{}\">\n", attr(stem));
    s += "  <compiler angle=\"radian\" meshdir=\".\" balanceinertia=\"true\"/>\n  <option integrator=\"implicitfast\"/>\n";
    s += &format!(
        "  <default>\n    <joint damping=\"{}\" armature=\"{}\"/>\n  </default>\n",
        num(positive(options.damping).unwrap_or(DEFAULT_DAMPING)),
        num(positive(options.armature).unwrap_or(DEFAULT_ARMATURE))
    );
    s += "  <asset>\n";
    for (file, _) in &pkg.meshes {
        s += &format!("    <mesh name=\"{}\" file=\"{}\"/>\n", attr(file.trim_end_matches(".stl")), attr(file));
    }
    s += "  </asset>\n  <worldbody>\n";
    if options.scene {
        s += "    <light name=\"sun\" pos=\"0 0 3\" dir=\"0 0 -1\" directional=\"true\"/>\n";
        s += "    <geom name=\"floor\" type=\"plane\" size=\"0 0 0.05\" rgba=\"0.8 0.8 0.8 1\"/>\n";
    }

    // Each link's joint from its parent; links in the order MuJoCo numbers bodies and joints.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); model.links.len()];
    for (k, j) in model.joints.iter().enumerate() {
        if let Some(c) = children.get_mut(j.parent) {
            c.push(k);
        }
    }
    let mut qpos: Vec<f64> = if floating { vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0] } else { Vec::new() };
    let mut moving: Vec<&UrdfJoint> = Vec::new();
    let mut stack = vec![Item::Open { link: 0, joint: None, depth: 2 }];
    while let Some(item) = stack.pop() {
        let (l, jk, depth) = match item {
            Item::Close(depth) => {
                s += &format!("{}</body>\n", "  ".repeat(depth));
                continue;
            }
            Item::Open { link, joint, depth } => (link, joint, depth),
        };
        let Some(link) = model.links.get(l) else { continue };
        let pad = "  ".repeat(depth);
        let joint = jk.and_then(|k| model.joints.get(k));
        let frame = joint.map_or(IDENTITY, |j| {
            let q = value(j);
            let m = match j.step {
                Step::Fixed => IDENTITY,
                Step::Revolute => urdf::rotation(j.axis, q),
                Step::Prismatic => urdf::translation(j.axis * (q * 1e3)),
            };
            mat_mul(&j.origin, &m)
        });
        s += &format!("{pad}<body name=\"{}\" {}>\n", attr(&link.name), pos_quat(&frame));
        if jk.is_none() && floating {
            s += &format!("{pad}  <freejoint name=\"root\"/>\n");
        }
        if let Some(j) = joint.filter(|j| j.step != Step::Fixed) {
            let kind = if j.step == Step::Revolute { "hinge" } else { "slide" };
            let q = value(j);
            s += &format!("{pad}  <joint name=\"{}\" type=\"{kind}\" axis=\"{}\" ref=\"{}\"", attr(&j.name), triple(j.axis), num(q));
            let scale = if j.step == Step::Prismatic { 1e-3 } else { 1.0 };
            match j.limit {
                Some((lo, hi)) => s += &format!(" range=\"{} {}\" limited=\"true\"/>\n", num(lo * scale), num(hi * scale)),
                None => s += " limited=\"false\"/>\n",
            }
            qpos.push(q);
            moving.push(j);
        }
        s += &format!("{pad}  {}\n", inertial_xml(&link.inertial.unwrap_or_else(urdf::token)));
        s += &geoms(link, &pad);
        stack.push(Item::Close(depth));
        // Reversed onto the stack, so they come off in the tree's order.
        for &k in children.get(l).map(Vec::as_slice).unwrap_or(&[]).iter().rev() {
            if let Some(c) = model.joints.get(k) {
                stack.push(Item::Open { link: c.child, joint: Some(k), depth: depth + 1 });
            }
        }
    }
    s += "  </worldbody>\n";

    // MuJoCo keeps a body from colliding with its parent, except where the parent is welded to
    // the world: there each joint's two sides (with what is fixed to them) are excluded here.
    if !floating {
        let ground = welded(&model, &children, 0);
        let drawn = |l: &usize| model.links.get(*l).is_some_and(|l| l.collision.is_some());
        let mut pairs = Vec::new();
        for j in model.joints.iter().filter(|j| j.step != Step::Fixed && ground.contains(&j.parent)) {
            let side = welded(&model, &children, j.child);
            for a in ground.iter().filter(|l| drawn(l)) {
                pairs.extend(side.iter().filter(|l| drawn(l)).map(|b| (*a, *b)));
            }
        }
        if pairs.len() > MAX_EXCLUDES {
            warnings
                .push(format!("the fixed base meets its first joints in {} body pairs: only {MAX_EXCLUDES} are kept from colliding", pairs.len()));
            pairs.truncate(MAX_EXCLUDES);
        }
        if !pairs.is_empty() {
            s += "  <contact>\n";
            for (a, b) in pairs {
                if let (Some(a), Some(b)) = (model.links.get(a), model.links.get(b)) {
                    s += &format!("    <exclude body1=\"{}\" body2=\"{}\"/>\n", attr(&a.name), attr(&b.name));
                }
            }
            s += "  </contact>\n";
        }
    }

    // A joint another follows (a gear, rack, screw) is driven; its follower is held to it.
    // MuJoCo measures both from the pose the model loads in (`ref`), where the link already
    // holds, so URDF's offset is already in it: q1 - ref1 = ratio (q2 - ref2).
    let followers: Vec<&UrdfJoint> = moving.iter().copied().filter(|j| j.mimic.is_some()).collect();
    if !followers.is_empty() {
        s += "  <equality>\n";
        for j in &followers {
            if let Some(m) = &j.mimic {
                s += &format!("    <joint joint1=\"{}\" joint2=\"{}\" polycoef=\"0 {} 0 0 0\"/>\n", attr(&j.name), attr(&m.joint), num(m.multiplier));
            }
        }
        s += "  </equality>\n";
    }
    let kp = positive(options.kp).unwrap_or(DEFAULT_KP);
    let mut ctrl = Vec::new();
    let mut unrated = Vec::new();
    let mut actuators = Vec::new();
    let driven: Vec<&UrdfJoint> = moving.iter().copied().filter(|j| j.mimic.is_none()).collect();
    if !driven.is_empty() {
        s += "  <actuator>\n";
        for j in driven {
            let rated = j.source.filter(|_| j.step == Step::Revolute).and_then(|(id, _)| positive(options.torques.get(&id).copied()));
            let force = rated.or(effort).unwrap_or_else(|| {
                unrated.push(j.name.clone());
                urdf::DEFAULT_EFFORT
            });
            s += &format!("    <position name=\"{}\" joint=\"{}\" kp=\"{}\" dampratio=\"1\"", attr(&j.name), attr(&j.name), num(kp));
            let scale = if j.step == Step::Prismatic { 1e-3 } else { 1.0 };
            let range = j.limit.map(|(lo, hi)| (lo * scale, hi * scale));
            if let Some((lo, hi)) = range {
                s += &format!(" ctrlrange=\"{} {}\" ctrllimited=\"true\"", num(lo), num(hi));
            }
            s += &format!(" forcerange=\"{} {}\" forcelimited=\"true\"/>\n", num(-force), num(force));
            ctrl.push(value(j));
            actuators.push(MjcfActuator { name: j.name.clone(), joint: j.name.clone(), force: (-force, force), ctrl: range });
        }
        s += "  </actuator>\n";
    }
    if !unrated.is_empty() {
        warnings.push(format!(
            "no rated actuator drives {} and `effort` is not given: their force limit is {}",
            unrated.join(", "),
            num(urdf::DEFAULT_EFFORT)
        ));
    }
    // The design's pose with every actuator holding it: `mj_resetDataKeyframe(model, data, 0)`.
    if !qpos.is_empty() {
        let list = |v: &[f64]| v.iter().map(|x| num(*x)).collect::<Vec<_>>().join(" ");
        s += &format!("  <keyframe>\n    <key name=\"design\" qpos=\"{}\"", list(&qpos));
        if !ctrl.is_empty() {
            s += &format!(" ctrl=\"{}\"", list(&ctrl));
        }
        s += "/>\n  </keyframe>\n";
    }
    s += "</mujoco>\n";
    let mass = model.links.iter().map(|l| l.inertial.unwrap_or_else(urdf::token).mass).sum();
    let summary = MjcfSummary { joints: moving.len(), free_root: floating, mass, actuators, keyframe: !qpos.is_empty() };
    Ok(MjcfPackage { xml: s, meshes: pkg.meshes, model, warnings, summary })
}

/// `from` and the links fixed to it, through fixed joints.
fn welded(model: &UrdfModel, children: &[Vec<usize>], from: usize) -> Vec<usize> {
    let mut out = vec![from];
    let mut k = 0;
    while let Some(&l) = out.get(k) {
        for j in children.get(l).map(Vec::as_slice).unwrap_or(&[]).iter().filter_map(|&j| model.joints.get(j)) {
            if j.step == Step::Fixed {
                out.push(j.child);
            }
        }
        k += 1;
    }
    out
}

enum Item {
    Open { link: usize, joint: Option<usize>, depth: usize },
    Close(usize),
}

/// A link's mesh geoms, placed in its body and coloured.
fn geoms(link: &Link, pad: &str) -> String {
    let rgba = link
        .color
        .map(|[r, g, b, a]| format!(" rgba=\"{} {} {} {}\"", num(r as f64), num(g as f64), num(b as f64), num(a as f64)))
        .unwrap_or_default();
    let at = pos_quat(&link.mesh_origin);
    let mesh = |file: &str| attr(file.trim_end_matches(".stl"));
    let name = attr(&link.name);
    match (&link.visual, &link.collision) {
        (Some(v), Some(c)) if v == c => format!("{pad}  <geom name=\"{name}\" type=\"mesh\" mesh=\"{}\" {at}{rgba}/>\n", mesh(v)),
        (v, c) => {
            // Open surfaces are drawn but don't collide; the closed solids collide unseen.
            let mut s = String::new();
            if let Some(v) = v {
                s += &format!("{pad}  <geom name=\"{name}\" type=\"mesh\" mesh=\"{}\" {at} contype=\"0\" conaffinity=\"0\"{rgba}/>\n", mesh(v));
            }
            if let Some(c) = c {
                s += &format!("{pad}  <geom name=\"{name}_collision\" type=\"mesh\" mesh=\"{}\" {at} group=\"3\"/>\n", mesh(c));
            }
            s
        }
    }
}

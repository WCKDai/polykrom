//! Feature recognition: `inspect.features` reads a body's exact B-rep (PolyCore's planes,
//! cylinders, cones, spheres and tori, with the body's persistent face names) and says which
//! machining features it holds: holes (through or blind, with their counterbore, countersink,
//! drill point and rounds as steps), bosses, pockets, fillets, chamfers, and the linear and
//! circular patterns they form. Every value is taken from the surfaces and the exact edge
//! curves, never from the display mesh; the mesh only tells which side of a face is outside and
//! which faces an edge joins. Where a reading needs the mesh after all (a bore that breaks out
//! through a curved face) it is marked `approximate`, with the reason.
//!
//! Read-only: it never touches the design, its revision or its undo history.

use polycore_geom::pmath;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use polycore::FaceSurface;
use polycore_geom::{Mesh, Vec3};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{bad, str_, string_list};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("inspect.features", "Recognise Features", features).icon("inspect").noundo().params(
    "body?: name | bodies?: [names] (default all) → per body: features (each with type, its exact sizes from the surfaces and the persistent names of the faces it covers: \
     hole {through, diameter, depth, axis: {origin, direction}, entry_face, exit_face | floor_face | tip: {angle_deg, depth}, steps: [{kind: counterbore|countersink|bore|round, diameter, depth, from, …}]}, \
     boss {diameter, height, axis, cap_face, base_face}, pocket {depth, floor_face, opening_face, walls}, fillet {radius, convex, joins}, chamfer {distance (along the first of joins), distance2, angle_deg, joins}), \
     patterns (linear: count1, pitch1, direction1, count2?, pitch2?, direction2?; circular: count, angle_step_deg, axis, radius; each with its members and faces), \
     unrecognised (the faces no feature covers, each with its surface and the reason) and uncertain (shapes that are nearly a feature, with why). Read-only: the design, its revision and its undo history are untouched",
)];

/// Most faces a body may have before recognition is refused (the edge walk is quadratic in a
/// body's edges).
const MAX_FACES: usize = 20_000;
const MAX_EDGES: usize = 40_000;
/// Two directions within this angle (radians) are parallel; two faces whose normals are within
/// it along an edge are tangent there.
const ANG: f64 = 1e-5;
/// A chamfer's turn across each of its two edges lies this far inside (0, 90°).
const CHAMFER_ANG: f64 = 1e-3;
/// Fewest members of a pattern: two alike features are just two features.
const MIN_PATTERN: usize = 3;
/// Most members of a group the search for the grids, rows and rings within it looks at (that
/// search is cubic); a group of any size is still read as one lattice or one circle.
const MAX_PATTERN_GROUP: usize = 256;

fn features(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "inspect.features";
    let st = s.world_state();
    let mut want = string_list(p, "bodies");
    if let Some(b) = str_(p, "body") {
        want.push(b.to_string());
    } else if p.get("body").is_some_and(|v| !v.is_null()) {
        return Err(bad(cmd, "`body` must be a body name"));
    }
    let mut unknown = Vec::new();
    for w in &want {
        if st.body(w).is_none() {
            unknown.push(format!("no body `{w}`{}", super::closest_hint(w, st.bodies.iter().map(|b| b.name.as_str()))));
        }
    }
    if !unknown.is_empty() {
        return Err(bad(cmd, unknown.join("; ")));
    }
    let selected: Vec<&polykrom_doc::ModelBody> = st.bodies.iter().filter(|b| want.is_empty() || want.contains(&b.name)).collect();
    // Each body on its own, on all cores; a body whose recognition panics is reported, not
    // dropped (the guard turns the panic into `None`).
    let results = polycore::map_on_threads(&selected, 16, |b| body_features(b));
    let bodies: Vec<Value> = selected
        .iter()
        .zip(results)
        .map(|(b, r)| r.unwrap_or_else(|| json!({"body": b.name, "error": "recognising this body's features failed"})))
        .collect();
    let count = |k: &str| -> usize { bodies.iter().filter_map(|b| b[k].as_array()).map(Vec::len).sum() };
    Ok(json!({
        "body_count": bodies.len(),
        "bodies": bodies,
        "total": {"features": count("features"), "patterns": count("patterns"), "unrecognised": count("unrecognised")},
    }))
}

/// One body's features.
fn body_features(b: &polykrom_doc::ModelBody) -> Value {
    if b.body.is_mesh() {
        return json!({"body": b.name, "error": "a mesh body has no B-rep faces to recognise features on"});
    }
    let topo = match Topo::new(b) {
        Ok(t) => t,
        Err(e) => return json!({"body": b.name, "error": e}),
    };
    let mut r = Recognised::default();
    holes_and_bosses(&topo, &mut r);
    pockets(&topo, &mut r);
    blends(&topo, &mut r);
    chamfers(&topo, &mut r);
    let (patterns, notes) = patterns(&topo, &r.features);
    let unrecognised = unrecognised(&topo, &r);
    let names = |fs: &[usize]| topo.names_of(fs);
    let mut v = json!({
        "body": b.name,
        "face_count": topo.faces.len(),
        "features": r.features.iter().enumerate().map(|(i, f)| f.json(i, &topo)).collect::<Vec<_>>(),
        "patterns": patterns,
        "unrecognised": unrecognised,
        "uncertain": r.uncertain.iter().map(|u| json!({"could_be": u.could_be, "faces": names(&u.faces), "reason": u.reason})).collect::<Vec<_>>(),
    });
    if !notes.is_empty() {
        v["note"] = json!(notes.join("; "));
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Topology: faces with their exact surfaces, edges with their exact curves and convexity.

struct Face {
    surface: Option<FaceSurface>,
    /// `1.0` when the body's outward normal is the surface's own ([`raw_normal`]), `-1.0`
    /// when it is the reverse (a hole's wall points at its axis).
    sign: f64,
    area: f64,
    centroid: Vec3,
    /// Triangles of the display mesh on this face.
    tris: Vec<usize>,
    edges: Vec<usize>,
    /// The faces cut from the very same surface (a bore's two halves), this one included.
    group: Vec<usize>,
}

struct Edge {
    faces: Vec<usize>,
    pts: Vec<Vec3>,
    circle: Option<polycore::EdgeCircle>,
    /// The ends of a straight edge.
    line: Option<(Vec3, Vec3)>,
    /// The angle (radians) the outward normal turns across the edge; `None` when a face has no
    /// analytic surface.
    turn: Option<f64>,
    /// `Some(true)`: convex (less than 180° of material), `Some(false)`: concave; `None`: tangent
    /// or unknown.
    convex: Option<bool>,
}

impl Edge {
    fn tangent(&self) -> bool {
        self.turn.is_some_and(|t| t < ANG)
    }
    fn other(&self, f: usize) -> Option<usize> {
        self.faces.iter().copied().find(|&g| g != f)
    }
    fn length(&self) -> f64 {
        self.pts.windows(2).map(|w| w.first().zip(w.get(1)).map_or(0.0, |(a, b)| a.dist(*b))).sum()
    }
}

struct Topo {
    faces: Vec<Face>,
    edges: Vec<Edge>,
    names: Arc<Vec<String>>,
    mesh: Arc<Mesh>,
    /// Lengths within this agree (a fraction of the body's size, as the kernel's own fits).
    tol: f64,
}

/// The surface's own normal at `p` (a point on it): a plane's normal, away from a cylinder's or
/// cone's axis, out of a sphere or a torus's tube.
fn raw_normal(s: &FaceSurface, p: Vec3) -> Option<Vec3> {
    match *s {
        FaceSurface::Plane { normal, .. } => Some(normal.vec()),
        FaceSurface::Cylinder { origin, axis, .. } => {
            let w = p - origin;
            (w - axis.vec() * w.dot(axis)).normalized()
        }
        FaceSurface::Cone { apex, axis, half_angle } => {
            let w = p - apex;
            let out = (w - axis.vec() * w.dot(axis)).normalized()?;
            (out * pmath::cos(half_angle) - axis.vec() * pmath::sin(half_angle)).normalized()
        }
        FaceSurface::Sphere { center, .. } => (p - center).normalized(),
        FaceSurface::Torus { center, axis, major, .. } => {
            let w = p - center;
            let radial = (w - axis.vec() * w.dot(axis)).normalized()?;
            (p - (center + radial * major)).normalized()
        }
    }
}

fn surface_kind(s: Option<&FaceSurface>) -> &'static str {
    match s {
        Some(FaceSurface::Plane { .. }) => "plane",
        Some(FaceSurface::Cylinder { .. }) => "cylinder",
        Some(FaceSurface::Cone { .. }) => "cone",
        Some(FaceSurface::Sphere { .. }) => "sphere",
        Some(FaceSurface::Torus { .. }) => "torus",
        None => "free-form",
    }
}

fn parallel(a: Vec3, b: Vec3) -> bool {
    a.cross(b).len() < ANG && a.dot(b) > 0.0
}

fn parallel_either_way(a: Vec3, b: Vec3) -> bool {
    a.cross(b).len() < ANG
}

/// Distance from `p` to the line through `o` along the unit vector `a`.
fn dist_to_line(p: Vec3, o: Vec3, a: Vec3) -> f64 {
    let w = p - o;
    (w - a * w.dot(a)).len()
}

/// Whether two surfaces are the very same (so the faces are pieces of one).
fn same_surface(a: &FaceSurface, b: &FaceSurface, tol: f64) -> bool {
    match (*a, *b) {
        (FaceSurface::Plane { normal: n1, point: p1 }, FaceSurface::Plane { normal: n2, point: p2 }) => {
            parallel(n1.vec(), n2.vec()) && (p2 - p1).dot(n1).abs() < tol
        }
        (FaceSurface::Cylinder { origin: o1, axis: a1, radius: r1 }, FaceSurface::Cylinder { origin: o2, axis: a2, radius: r2 }) => {
            parallel_either_way(a1.vec(), a2.vec()) && (r1 - r2).abs() < tol && dist_to_line(o2, o1, a1.vec()) < tol
        }
        (FaceSurface::Cone { apex: v1, axis: a1, half_angle: h1 }, FaceSurface::Cone { apex: v2, axis: a2, half_angle: h2 }) => {
            parallel(a1.vec(), a2.vec()) && (h1 - h2).abs() < ANG && v1.dist(v2) < tol
        }
        (FaceSurface::Sphere { center: c1, radius: r1 }, FaceSurface::Sphere { center: c2, radius: r2 }) => {
            c1.dist(c2) < tol && (r1 - r2).abs() < tol
        }
        (FaceSurface::Torus { center: c1, axis: a1, major: m1, minor: n1 }, FaceSurface::Torus { center: c2, axis: a2, major: m2, minor: n2 }) => {
            parallel_either_way(a1.vec(), a2.vec()) && c1.dist(c2) < tol && (m1 - m2).abs() < tol && (n1 - n2).abs() < tol
        }
        _ => false,
    }
}

impl Topo {
    fn new(b: &polykrom_doc::ModelBody) -> std::result::Result<Topo, String> {
        let body = &b.body;
        let nf = body.face_count();
        if nf > MAX_FACES {
            return Err(format!("too many faces to recognise features on ({nf}; the most is {MAX_FACES})"));
        }
        let mesh = b.mesh();
        if mesh.edges.len() > MAX_EDGES {
            return Err(format!("too many edges to recognise features on ({}; the most is {MAX_EDGES})", mesh.edges.len()));
        }
        if mesh.edge_faces.len() != mesh.edges.len() {
            return Err("the body's mesh does not say which faces its edges bound".into());
        }
        let size = body.size();
        let tol = (size * 1e-5).max(1e-9);
        let names = polykrom_doc::naming::face_names(b);
        let mut faces: Vec<Face> = (0..nf)
            .map(|i| Face {
                surface: polycore::face_surface(body, i),
                sign: 1.0,
                area: 0.0,
                centroid: Vec3::ZERO,
                tris: Vec::new(),
                edges: Vec::new(),
                group: vec![i],
            })
            .collect();
        // Which side of each face is outside: the mesh's triangles wind counter-clockwise seen
        // from outside, so the largest one's winding normal says whether the surface's own
        // normal points out or in.
        let mut largest: Vec<(f64, Vec3, Vec3)> = vec![(0.0, Vec3::ZERO, Vec3::ZERO); nf];
        for (ti, (t, f)) in mesh.triangles.iter().zip(&mesh.tri_face).enumerate() {
            let Some([a, bb, c]) = mesh.tri(t) else { continue };
            let Some(face) = faces.get_mut(*f as usize) else { continue };
            let cr = (bb - a).cross(c - a);
            let area = cr.len() * 0.5;
            face.tris.push(ti);
            face.area += area;
            face.centroid += (a + bb + c) * (area / 3.0);
            if let Some(slot) = largest.get_mut(*f as usize)
                && area > slot.0
                && let Some(n) = cr.normalized()
            {
                *slot = (area, (a + bb + c) / 3.0, n);
            }
        }
        for (f, (area, centre, n)) in faces.iter_mut().zip(largest) {
            if f.area > 0.0 {
                f.centroid = f.centroid / f.area;
            }
            if area > 0.0
                && let Some(s) = &f.surface
                && let Some(raw) = raw_normal(s, centre)
                && raw.dot(n) < 0.0
            {
                f.sign = -1.0;
            }
        }
        // Faces cut from one surface.
        let mut parent: Vec<usize> = (0..nf).collect();
        fn root(p: &mut [usize], mut i: usize) -> usize {
            while let Some(&q) = p.get(i) {
                if q == i {
                    break;
                }
                i = q;
            }
            i
        }
        let mut edges: Vec<Edge> = Vec::with_capacity(mesh.edges.len());
        for (ei, (pts, fs)) in mesh.edges.iter().zip(&mesh.edge_faces).enumerate() {
            let mut fcs: Vec<usize> = fs.iter().map(|f| *f as usize).filter(|f| *f < nf).collect();
            fcs.dedup();
            for f in &fcs {
                if let Some(face) = faces.get_mut(*f) {
                    face.edges.push(ei);
                }
            }
            let circle = polycore::edge_circle(body, ei);
            let line = match (pts.first(), pts.last()) {
                (Some(a), Some(z)) if circle.is_none() && a.dist(*z) > tol => {
                    let d = (*z - *a).normalized();
                    d.filter(|d| pts.iter().all(|p| dist_to_line(*p, *a, *d) < tol)).map(|_| (*a, *z))
                }
                _ => None,
            };
            edges.push(Edge { faces: fcs, pts: pts.clone(), circle, line, turn: None, convex: None });
            if let [a, bb] = edges.get(ei).map(|e| e.faces.as_slice()).unwrap_or(&[])
                && let (Some(sa), Some(sb)) = (faces.get(*a).and_then(|f| f.surface.as_ref()), faces.get(*bb).and_then(|f| f.surface.as_ref()))
                && same_surface(sa, sb, tol)
                && faces.get(*a).map(|f| f.sign) == faces.get(*bb).map(|f| f.sign)
            {
                let (ra, rb) = (root(&mut parent, *a), root(&mut parent, *bb));
                if let Some(slot) = parent.get_mut(ra) {
                    *slot = rb;
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in 0..nf {
            groups.entry(root(&mut parent, i)).or_default().push(i);
        }
        for g in groups.values() {
            for &i in g {
                if let Some(f) = faces.get_mut(i) {
                    f.group = g.clone();
                }
            }
        }
        let mut topo = Topo { faces, edges, names, mesh, tol };
        topo.classify_edges();
        Ok(topo)
    }

    /// The body's outward normal of face `f` at `p`, a point on it.
    fn normal(&self, f: usize, p: Vec3) -> Option<Vec3> {
        let face = self.faces.get(f)?;
        raw_normal(face.surface.as_ref()?, p).map(|n| n * face.sign)
    }

    /// A point inside face `f` close to `p`: the centre of its triangle nearest `p`.
    fn inside_near(&self, f: usize, p: Vec3) -> Option<Vec3> {
        let face = self.faces.get(f)?;
        let mut best: Option<(f64, Vec3)> = None;
        for ti in &face.tris {
            let Some([a, b, c]) = self.mesh.triangles.get(*ti).and_then(|t| self.mesh.tri(t)) else { continue };
            let centre = (a + b + c) / 3.0;
            let d = centre.dist(p);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, centre));
            }
        }
        best.map(|(_, c)| c)
    }

    /// A point of face `f` just inside the edge segment `p`–`q`: the third corner of the face's
    /// triangle on that segment (the faces share their edges' points). A triangle on the segment
    /// lies on the face's side of it however the face is shaped around it, which a triangle
    /// merely near `p` need not (a face with an opening has triangles across it).
    fn beside_segment(&self, f: usize, p: Vec3, q: Vec3) -> Option<Vec3> {
        let face = self.faces.get(f)?;
        let close = self.tol;
        for ti in &face.tris {
            let Some(tri) = self.mesh.triangles.get(*ti).and_then(|t| self.mesh.tri(t)) else { continue };
            let at = |x: Vec3| tri.iter().position(|v| v.dist(x) < close);
            if let (Some(i), Some(j)) = (at(p), at(q))
                && i != j
            {
                return tri.iter().enumerate().find(|(k, _)| *k != i && *k != j).map(|(_, v)| *v);
            }
        }
        None
    }

    /// Each edge's turn and convexity, from the exact normals at a point of the edge and the
    /// side each face lies on.
    fn classify_edges(&mut self) {
        for ei in 0..self.edges.len() {
            let Some([a, b]) = self.edges.get(ei).map(|e| e.faces.as_slice()).and_then(|f| <[usize; 2]>::try_from(f).ok()) else { continue };
            let Some((p, q)) = self.edges.get(ei).and_then(|e| {
                let i = e.pts.len() / 2;
                let p = *e.pts.get(i)?;
                let q = e.pts.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| e.pts.get(j))).copied()?;
                Some((p, q))
            }) else {
                continue;
            };
            let (Some(na), Some(nb)) = (self.normal(a, p), self.normal(b, p)) else { continue };
            let turn = pmath::acos(na.dot(nb).clamp(-1.0, 1.0));
            let convex = if turn < ANG {
                None
            } else {
                let ia = self.beside_segment(a, p, q).or_else(|| self.inside_near(a, p));
                let ib = self.beside_segment(b, p, q).or_else(|| self.inside_near(b, p));
                let (Some(ia), Some(ib)) = (ia, ib) else { continue };
                let (da, db) = ((ia - p).dot(nb), (ib - p).dot(na));
                let eps = self.tol * 1e-3;
                if da < -eps && db < -eps {
                    Some(true)
                } else if da > eps && db > eps {
                    Some(false)
                } else {
                    None
                }
            };
            if let Some(e) = self.edges.get_mut(ei) {
                e.turn = Some(turn);
                e.convex = convex;
            }
        }
    }

    fn name(&self, f: usize) -> String {
        self.names.get(f).cloned().unwrap_or_else(|| format!("face {f}"))
    }

    fn names_of(&self, fs: &[usize]) -> Vec<String> {
        let mut v: Vec<usize> = fs.to_vec();
        v.sort_unstable();
        v.dedup();
        v.iter().map(|f| self.name(*f)).collect()
    }

    fn surface(&self, f: usize) -> Option<&FaceSurface> {
        self.faces.get(f).and_then(|x| x.surface.as_ref())
    }

    fn plane(&self, f: usize) -> Option<(Vec3, Vec3)> {
        match (self.surface(f), self.faces.get(f)) {
            (Some(FaceSurface::Plane { normal, point }), Some(face)) => Some((normal.vec() * face.sign, *point)),
            _ => None,
        }
    }

    /// How far the planar face `f` reaches across the direction `dir` lying in it: the extent of
    /// its edges' points along the in-plane direction perpendicular to `dir`.
    fn extent_across(&self, f: usize, dir: Vec3) -> Option<f64> {
        let (n, _) = self.plane(f)?;
        let u = n.cross(dir).normalized()?;
        let face = self.faces.get(f)?;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for &ei in &face.edges {
            for p in self.edges.get(ei).map(|e| e.pts.as_slice()).unwrap_or(&[]) {
                let h = p.dot(u);
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        (hi >= lo).then_some(hi - lo)
    }

    /// The edges of a group of faces that lead out of it, with the face they lead to.
    fn boundary(&self, group: &[usize]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for &f in group {
            let Some(face) = self.faces.get(f) else { continue };
            for &ei in &face.edges {
                let Some(e) = self.edges.get(ei) else { continue };
                for &g in &e.faces {
                    if !group.contains(&g) {
                        out.push((ei, g));
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Recognised features.

#[derive(Clone, Debug)]
struct Step {
    kind: &'static str,
    faces: Vec<usize>,
    /// Axial distance from the entry plane to where the step begins.
    from: f64,
    /// Axial length.
    depth: f64,
    diameter: Option<f64>,
    angle_deg: Option<f64>,
    radius: Option<f64>,
}

#[derive(Clone, Debug)]
enum Bottom {
    Through { exit: Option<usize> },
    Flat { floor: usize },
    Tip { faces: Vec<usize>, angle_deg: f64, depth: f64 },
}

#[derive(Clone, Debug)]
enum Kind {
    Hole {
        diameter: f64,
        depth: f64,
        origin: Vec3,
        direction: Vec3,
        entry: Option<usize>,
        bottom: Bottom,
        steps: Vec<Step>,
    },
    Boss {
        diameter: f64,
        height: f64,
        origin: Vec3,
        direction: Vec3,
        cap: usize,
        base: Option<usize>,
    },
    Pocket {
        depth: f64,
        floor: usize,
        opening: Vec<usize>,
        walls: Vec<usize>,
        /// The rounds along the floor's edges (a cutter-radius fillet), between it and the walls.
        rounds: Vec<usize>,
        normal: Vec3,
    },
    Fillet {
        radius: f64,
        convex: Option<bool>,
        joins: Vec<usize>,
        /// A direction the blend runs along, when it has one (a cylinder's axis).
        along: Option<Vec3>,
    },
    Chamfer {
        distance: f64,
        distance2: f64,
        angle_deg: f64,
        joins: Vec<usize>,
        along: Option<Vec3>,
    },
}

#[derive(Clone, Debug)]
struct Feature {
    kind: Kind,
    faces: Vec<usize>,
    /// Where the feature is, for patterns.
    position: Vec3,
    approximate: bool,
    notes: Vec<String>,
}

struct Uncertain {
    could_be: &'static str,
    faces: Vec<usize>,
    reason: String,
}

#[derive(Default)]
struct Recognised {
    features: Vec<Feature>,
    uncertain: Vec<Uncertain>,
    /// Faces a hole or boss owns as its wall, cap, step or floor (blends may still be blends).
    owned: BTreeSet<usize>,
    /// The cylinders that are a bore's or a boss's wall: tangent to the rounds at their ends,
    /// they are never blends themselves.
    walls: BTreeSet<usize>,
}

impl Recognised {
    fn owns(&self, f: usize) -> bool {
        self.owned.contains(&f)
    }
    fn claim(&mut self, fs: &[usize]) {
        self.owned.extend(fs.iter().copied());
    }
}

/// Numbers are rounded to a nanometre so an exact value reads as one.
fn r9(x: f64) -> f64 {
    if x.is_finite() { (x * 1e9).round() / 1e9 + 0.0 } else { x }
}

fn v9(v: Vec3) -> Value {
    json!([r9(v.x), r9(v.y), r9(v.z)])
}

impl Feature {
    fn type_name(&self) -> &'static str {
        match self.kind {
            Kind::Hole { .. } => "hole",
            Kind::Boss { .. } => "boss",
            Kind::Pocket { .. } => "pocket",
            Kind::Fillet { .. } => "fillet",
            Kind::Chamfer { .. } => "chamfer",
        }
    }

    /// The direction the feature is aligned along (a hole's axis, a pocket's normal, a blend's
    /// run), for patterns: members must share it.
    fn direction(&self) -> Option<Vec3> {
        match &self.kind {
            Kind::Hole { direction, .. } | Kind::Boss { direction, .. } => Some(*direction),
            Kind::Pocket { normal, .. } => Some(*normal),
            Kind::Fillet { along, .. } | Kind::Chamfer { along, .. } => *along,
        }
    }

    /// What must agree between pattern members, as text.
    fn signature(&self) -> String {
        let k = |x: f64| format!("{:.6}", x);
        match &self.kind {
            Kind::Hole { diameter, depth, bottom, steps, .. } => {
                let b = match bottom {
                    Bottom::Through { .. } => "through".to_string(),
                    Bottom::Flat { .. } => "flat".to_string(),
                    Bottom::Tip { angle_deg, depth, .. } => format!("tip {} {}", k(*angle_deg), k(*depth)),
                };
                let st: Vec<String> = steps
                    .iter()
                    .map(|s| {
                        format!(
                            "{} {} {} {} {}",
                            s.kind,
                            k(s.depth),
                            s.diameter.map_or_else(String::new, k),
                            s.angle_deg.map_or_else(String::new, k),
                            s.radius.map_or_else(String::new, k)
                        )
                    })
                    .collect();
                format!("hole {} {} {b} [{}]", k(*diameter), k(*depth), st.join(", "))
            }
            Kind::Boss { diameter, height, .. } => format!("boss {} {}", k(*diameter), k(*height)),
            Kind::Pocket { depth, walls, .. } => format!("pocket {} {} walls", k(*depth), walls.len()),
            Kind::Fillet { radius, convex, .. } => format!("fillet {} {convex:?} {}", k(*radius), self.faces.len()),
            Kind::Chamfer { distance, distance2, .. } => format!("chamfer {} {}", k(*distance), k(*distance2)),
        }
    }

    fn json(&self, index: usize, t: &Topo) -> Value {
        let name = |f: Option<usize>| f.map(|f| t.name(f));
        let axis = |origin: Vec3, direction: Vec3| json!({"origin": v9(origin), "direction": v9(direction)});
        let mut v = match &self.kind {
            Kind::Hole { diameter, depth, origin, direction, entry, bottom, steps } => {
                let mut v = json!({
                    "type": "hole",
                    "through": matches!(bottom, Bottom::Through { .. }),
                    "diameter": r9(*diameter),
                    "depth": r9(*depth),
                    "axis": axis(*origin, *direction),
                    "entry_face": name(*entry),
                    "steps": steps.iter().map(|s| {
                        let mut v = json!({"kind": s.kind, "from": r9(s.from), "depth": r9(s.depth), "faces": t.names_of(&s.faces)});
                        if let Some(d) = s.diameter { v["diameter"] = json!(r9(d)); }
                        if let Some(a) = s.angle_deg { v["angle_deg"] = json!(r9(a)); }
                        if let Some(r) = s.radius { v["radius"] = json!(r9(r)); }
                        v
                    }).collect::<Vec<_>>(),
                });
                match bottom {
                    Bottom::Through { exit } => v["exit_face"] = json!(name(*exit)),
                    Bottom::Flat { floor } => v["floor_face"] = json!(t.name(*floor)),
                    Bottom::Tip { faces, angle_deg, depth } => {
                        v["tip"] = json!({"faces": t.names_of(faces), "angle_deg": r9(*angle_deg), "depth": r9(*depth)})
                    }
                }
                v
            }
            Kind::Boss { diameter, height, origin, direction, cap, base } => json!({
                "type": "boss", "diameter": r9(*diameter), "height": r9(*height), "axis": axis(*origin, *direction),
                "cap_face": t.name(*cap), "base_face": name(*base),
            }),
            Kind::Pocket { depth, floor, opening, walls, rounds, .. } => json!({
                "type": "pocket", "depth": r9(*depth), "floor_face": t.name(*floor),
                "opening_face": opening.first().map(|f| t.name(*f)), "opening_faces": t.names_of(opening), "walls": t.names_of(walls),
                "floor_rounds": t.names_of(rounds),
            }),
            Kind::Fillet { radius, convex, joins, .. } => {
                json!({"type": "fillet", "radius": r9(*radius), "convex": convex, "joins": t.names_of(joins)})
            }
            Kind::Chamfer { distance, distance2, angle_deg, joins, .. } => json!({
                "type": "chamfer", "distance": r9(*distance), "distance2": r9(*distance2), "angle_deg": r9(*angle_deg),
                "joins": joins.iter().map(|f| t.name(*f)).collect::<Vec<_>>(),
            }),
        };
        v["id"] = json!(index);
        v["faces"] = json!(t.names_of(&self.faces));
        if self.approximate {
            v["approximate"] = json!(true);
        }
        if !self.notes.is_empty() {
            v["note"] = json!(self.notes.join("; "));
        }
        v
    }
}

// ---------------------------------------------------------------------------------------------
// Holes and bosses: coaxial stacks about one cylinder axis.

/// Pieces of one cylinder, with its axis and the span it covers along it.
struct Segment {
    faces: Vec<usize>,
    origin: Vec3,
    axis: Vec3,
    radius: f64,
    concave: bool,
    lo: f64,
    hi: f64,
    approximate: bool,
}

/// `p`'s position along the axis.
fn along(seg_origin: Vec3, axis: Vec3, p: Vec3) -> f64 {
    (p - seg_origin).dot(axis)
}

/// Is the surface `s` coaxial with the line (`o`, `a`)? Planes count when perpendicular to it.
fn coaxial(s: &FaceSurface, o: Vec3, a: Vec3, tol: f64) -> bool {
    match *s {
        FaceSurface::Plane { normal, .. } => parallel_either_way(normal.vec(), a),
        FaceSurface::Cylinder { origin, axis, .. } => parallel_either_way(axis.vec(), a) && dist_to_line(origin, o, a) < tol,
        FaceSurface::Cone { apex, axis, .. } => parallel_either_way(axis.vec(), a) && dist_to_line(apex, o, a) < tol,
        FaceSurface::Sphere { center, .. } => dist_to_line(center, o, a) < tol,
        FaceSurface::Torus { center, axis, .. } => parallel_either_way(axis.vec(), a) && dist_to_line(center, o, a) < tol,
    }
}

/// What an edge covers along the axis (`o`, `a`), exactly from its curve: a circle about the
/// axis sits at its centre, any other arc reaches its exact extremes, a line its ends. Any other
/// curve is read from its polyline, and `false` says so.
fn edge_extent(e: &Edge, o: Vec3, a: Vec3) -> (f64, f64, bool) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut take = |p: Vec3| {
        let h = along(o, a, p);
        lo = lo.min(h);
        hi = hi.max(h);
    };
    match (e.circle, e.line) {
        (Some(c), _) if parallel_either_way(c.axis.vec(), a) => {
            take(c.center);
            (lo, hi, true)
        }
        (Some(c), _) => {
            // p(θ) = centre + r (x cos θ + y sin θ): its projection on `a` is extreme at θ*.
            take(c.start);
            take(c.end);
            if let Some(x) = (c.start - c.center).normalized() {
                let y = c.axis.vec().cross(x);
                let (cx, cy) = (x.dot(a), y.dot(a));
                let theta = pmath::atan2(cy, cx);
                for th in [theta, theta + PI] {
                    let th = th.rem_euclid(TAU);
                    if th <= c.angle {
                        take(c.center + (x * pmath::cos(th) + y * pmath::sin(th)) * c.radius);
                    }
                }
            }
            (lo, hi, true)
        }
        (None, Some((p, q))) => {
            take(p);
            take(q);
            (lo, hi, true)
        }
        (None, None) => {
            for p in &e.pts {
                take(*p);
            }
            (lo, hi, false)
        }
    }
}

/// The span along the axis of a group of faces, from the exact extents of its edges; `true`
/// when some edge had to be read from its polyline (the span is then approximate).
fn span(t: &Topo, faces: &[usize], o: Vec3, a: Vec3) -> (f64, f64, bool) {
    let (mut lo, mut hi, mut approx) = (f64::INFINITY, f64::NEG_INFINITY, false);
    let mut seen = BTreeSet::new();
    for &f in faces {
        let Some(face) = t.faces.get(f) else { continue };
        for &ei in &face.edges {
            if !seen.insert(ei) {
                continue;
            }
            let Some(e) = t.edges.get(ei) else { continue };
            let (elo, ehi, exact) = edge_extent(e, o, a);
            lo = lo.min(elo);
            hi = hi.max(ehi);
            approx |= !exact;
        }
    }
    (lo, hi, approx)
}

/// Cylinder faces that are walls of a bore or a boss (not blends), gathered per cylinder.
fn segments(t: &Topo) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut done = BTreeSet::new();
    for (i, f) in t.faces.iter().enumerate() {
        let Some(FaceSurface::Cylinder { origin, axis, radius }) = f.surface else { continue };
        if done.contains(&i) {
            continue;
        }
        let group = f.group.clone();
        done.extend(group.iter().copied());
        let (o, a) = (origin, axis.vec());
        // A wall's tangent neighbours are only its own other pieces and coaxial rounds; a face
        // tangent to anything else is a blend.
        let wall = group.iter().all(|&g| {
            t.faces.get(g).is_some_and(|face| {
                face.edges.iter().all(|&ei| {
                    let Some(e) = t.edges.get(ei) else { return true };
                    if !e.tangent() {
                        return true;
                    }
                    e.faces
                        .iter()
                        .all(|&h| group.contains(&h) || matches!(t.surface(h), Some(s @ FaceSurface::Torus { .. }) if coaxial(s, o, a, t.tol)))
                })
            })
        });
        if !wall {
            continue;
        }
        let (lo, hi, approximate) = span(t, &group, o, a);
        if !(hi - lo > t.tol) {
            continue;
        }
        out.push(Segment { faces: group, origin: o, axis: a, radius, concave: f.sign < 0.0, lo, hi, approximate });
    }
    out
}

/// What a walk along a hole's or boss's axis meets. Positions along the axis are in the frame
/// of the segment the walk started from (coaxial cylinders may carry opposite axis directions).
#[derive(Clone, Debug)]
enum Elem {
    /// A wall: the segment's index and its span.
    Wall { seg: usize, lo: f64, hi: f64 },
    /// A flat step between two walls (an annulus), or a blind hole's floor (a disc).
    Flat { faces: Vec<usize>, floor: bool },
    /// A coaxial cone: a countersink, a chamfer or a drill point (`tip`: it ends at its apex).
    Cone { faces: Vec<usize>, half: f64, tip: bool, apex: f64, lo: f64, hi: f64 },
    /// A coaxial round (a fillet on a rim or at a floor).
    Round { faces: Vec<usize>, minor: f64, lo: f64, hi: f64 },
}

/// How a walk ends.
#[derive(Clone, Debug)]
enum End {
    /// Open to the outside: the faces at the rim and where the rim is along the axis.
    Open {
        faces: Vec<usize>,
        at: f64,
        approximate: bool,
    },
    Closed,
}

/// The coaxial elements met walking from segment `s` toward `dir` (`+1`: toward `hi`), and how
/// the walk ends. `walls` are the body's wall segments; `want_concave` says whether the stack is
/// a hole (true) or a boss. Rounds and (for holes) steps, cones and floors are walked through.
fn walk(t: &Topo, walls: &[Segment], s: usize, dir: f64, want_concave: bool) -> (Vec<Elem>, End) {
    let Some(seg) = walls.get(s) else { return (Vec::new(), End::Closed) };
    let (o, a) = (seg.origin, seg.axis);
    let mut elems = Vec::new();
    let mut visited: BTreeSet<usize> = seg.faces.iter().copied().collect();
    // The current element's faces and the axial positions of its far end (`at`) and its near
    // end (`back`) along the walk.
    let mut current: Vec<usize> = seg.faces.clone();
    let (mut at, mut back) = if dir > 0.0 { (seg.hi, seg.lo) } else { (seg.lo, seg.hi) };
    let mut approx = seg.approximate;
    for _ in 0..64 {
        // The faces beyond the current element at its far end: across the edges lying in the
        // plane there (an edge along the axis, a seam, is no end edge). An exact rim (a circle
        // or a line about the axis) lies in that plane; a rim read from its polyline (where the
        // bore breaks out through a curved face: two perpendicular cylinders meet along a curve
        // spanning about r²/2R along the bore) reaches the far end and stays short of the near
        // one, and makes the reading approximate.
        let mut next: Vec<usize> = Vec::new();
        let mut rim_approx = approx;
        for (ei, g) in t.boundary(&current) {
            let Some(e) = t.edges.get(ei) else { continue };
            let (lo, hi, exact) = edge_extent(e, o, a);
            let (far, near) = if dir > 0.0 { (hi, lo) } else { (lo, hi) };
            let rim = if exact {
                (lo - at).abs() <= t.tol && (hi - at).abs() <= t.tol
            } else {
                (far - at).abs() <= t.tol * 100.0 && (near - back) * dir > t.tol
            };
            if !rim {
                continue;
            }
            if !exact {
                rim_approx = true;
            }
            if !visited.contains(&g) && !next.contains(&g) {
                next.push(g);
            }
        }
        if next.is_empty() {
            return (elems, End::Closed);
        }
        let open = |faces: Vec<usize>| End::Open { faces, at, approximate: rim_approx };
        // One surface beyond (several faces only when they are pieces of one)?
        let Some(&f0) = next.first() else { return (elems, End::Closed) };
        let group = t.faces.get(f0).map(|f| f.group.clone()).unwrap_or_default();
        if !next.iter().all(|g| group.contains(g)) {
            return (elems, open(next));
        }
        let Some(surface) = t.surface(f0) else { return (elems, open(next)) };
        if !coaxial(surface, o, a, t.tol) {
            return (elems, open(next));
        }
        let sign = t.faces.get(f0).map_or(1.0, |f| f.sign);
        match *surface {
            FaceSurface::Plane { normal, point } => {
                // A flat step or floor: every edge of it is a coaxial circle. A plane with any
                // other edge is the face the hole is drilled into (or the boss stands on).
                let face = t.faces.get(f0);
                let circles_only = face.is_some_and(|f| {
                    f.edges.iter().all(|&ei| {
                        t.edges
                            .get(ei)
                            .is_some_and(|e| e.circle.is_some_and(|c| parallel_either_way(c.axis.vec(), a) && dist_to_line(c.center, o, a) < t.tol))
                    })
                });
                let facing = normal.vec() * sign;
                if !want_concave || !circles_only || group.len() != 1 {
                    return (elems, open(next));
                }
                let h = along(o, a, point);
                // The other circles of the step lead to the next wall, concave and coaxial.
                let onward: Vec<usize> =
                    t.boundary(&group).into_iter().map(|(_, g)| g).filter(|g| !visited.contains(g) && !current.contains(g)).collect();
                if onward.is_empty() {
                    // A floor: it faces back along the walk.
                    if facing.dot(a * dir) < 0.0 {
                        elems.push(Elem::Flat { faces: group, floor: true });
                        return (elems, End::Closed);
                    }
                    return (elems, open(next));
                }
                // The next wall, in this walk's frame.
                let seg_next = walls.iter().enumerate().find_map(|(i, w)| {
                    if !w.concave || !onward.iter().all(|g| w.faces.contains(g)) {
                        return None;
                    }
                    let (lo, hi, ap) = span(t, &w.faces, o, a);
                    ((lo - h).abs() < t.tol || (hi - h).abs() < t.tol).then_some((i, lo, hi, ap))
                });
                let Some((sn, lo, hi, ap)) = seg_next else { return (elems, open(next)) };
                elems.push(Elem::Flat { faces: group, floor: false });
                visited.insert(f0);
                let Some(w) = walls.get(sn) else { return (elems, End::Closed) };
                elems.push(Elem::Wall { seg: sn, lo, hi });
                visited.extend(w.faces.iter().copied());
                current = w.faces.clone();
                approx = ap;
                (at, back) = if dir > 0.0 { (hi, lo) } else { (lo, hi) };
            }
            FaceSurface::Cone { apex, axis, half_angle } => {
                // Concave for a hole (countersink, drill point), convex for a boss (chamfer):
                // the cone is walked through; what lies beyond decides.
                if (sign < 0.0) != want_concave {
                    return (elems, open(next));
                }
                let (lo, hi, ap) = span(t, &group, o, a);
                let apex_h = along(o, a, apex);
                // Does it end at its apex (a drill point) rather than at another circle?
                let (far, near) = if dir > 0.0 { (hi, lo) } else { (lo, hi) };
                let tip = (far - apex_h).abs() < t.tol && axis.vec().dot(a * dir) < 0.0;
                visited.extend(group.iter().copied());
                elems.push(Elem::Cone { faces: group.clone(), half: half_angle, tip, apex: apex_h, lo, hi });
                if tip {
                    return (elems, End::Closed);
                }
                current = group;
                approx = ap;
                (at, back) = (far, near);
            }
            FaceSurface::Torus { minor, .. } => {
                let (lo, hi, ap) = span(t, &group, o, a);
                visited.extend(group.iter().copied());
                elems.push(Elem::Round { faces: group.clone(), minor, lo, hi });
                current = group;
                approx = ap;
                (at, back) = if dir > 0.0 { (hi, lo) } else { (lo, hi) };
            }
            FaceSurface::Cylinder { .. } => {
                // Another wall straight on (a hole whose bore changes without a step is not
                // one; a boss's wall meeting another wall is a shoulder): the rim is here.
                return (elems, open(next));
            }
            FaceSurface::Sphere { .. } => return (elems, open(next)),
        }
    }
    (elems, End::Closed)
}

fn deg(rad: f64) -> f64 {
    rad * 180.0 / PI
}

/// A rim's radius: how wide the stack opens at that end.
fn rim_radius(walls: &[Segment], elems: &[Elem], seg: usize, end_elem: Option<&Elem>) -> f64 {
    let base = walls.get(seg).map_or(0.0, |w| w.radius);
    let wall_r = |e: &Elem| match e {
        Elem::Wall { seg, .. } => walls.get(*seg).map(|w| w.radius),
        _ => None,
    };
    let last_wall = elems.iter().rev().find_map(wall_r).unwrap_or(base);
    match end_elem {
        Some(Elem::Cone { half, apex, lo, hi, .. }) => (lo - apex).abs().max((hi - apex).abs()) * pmath::tan(*half),
        Some(Elem::Round { minor, .. }) => last_wall + minor,
        _ => last_wall,
    }
}

/// How much the faces at a rim face up (+z): the convention that picks a through hole's entry
/// when nothing else does.
fn faces_up(t: &Topo, faces: &[usize], near: Vec3) -> f64 {
    faces.iter().filter_map(|&f| t.normal(f, t.inside_near(f, near).unwrap_or(near))).map(|n| n.z).fold(f64::NEG_INFINITY, f64::max)
}

fn holes_and_bosses(t: &Topo, r: &mut Recognised) {
    let walls = segments(t);
    let mut used: BTreeSet<usize> = BTreeSet::new();
    // Holes first: the concave walls, each stack walked once from any of its walls.
    for (si, seg) in walls.iter().enumerate() {
        if !seg.concave || used.contains(&si) {
            continue;
        }
        let (up, end_up) = walk(t, &walls, si, 1.0, true);
        let (down, end_down) = walk(t, &walls, si, -1.0, true);
        // The whole stack from the low end to the high end.
        let mut stack: Vec<Elem> = down.iter().rev().cloned().collect();
        stack.push(Elem::Wall { seg: si, lo: seg.lo, hi: seg.hi });
        stack.extend(up.iter().cloned());
        for e in &stack {
            if let Elem::Wall { seg, .. } = e {
                used.insert(*seg);
            }
        }
        let (o, a) = (seg.origin, seg.axis);
        // Which end is the entry.
        let mut notes = Vec::new();
        let (entry_hi, entry_end, exit_end) = match (&end_down, &end_up) {
            (End::Closed, End::Closed) => {
                r.uncertain.push(Uncertain {
                    could_be: "hole",
                    faces: stack_faces(&walls, &stack),
                    reason: "a cylindrical cavity closed at both ends: not reachable from outside".into(),
                });
                continue;
            }
            (End::Open { .. }, End::Closed) => (false, &end_down, &end_up),
            (End::Closed, End::Open { .. }) => (true, &end_up, &end_down),
            (End::Open { faces: fd, at: hd, .. }, End::Open { faces: fu, at: hu, .. }) => {
                let r_up = rim_radius(&walls, &up, si, up.last());
                let r_down = rim_radius(&walls, &down, si, down.last());
                if (r_up - r_down).abs() > t.tol {
                    (r_up > r_down, if r_up > r_down { &end_up } else { &end_down }, if r_up > r_down { &end_down } else { &end_up })
                } else {
                    let (zu, zd) = (faces_up(t, fu, o + a * *hu), faces_up(t, fd, o + a * *hd));
                    if (zu - zd).abs() > 1e-9 {
                        (zu > zd, if zu > zd { &end_up } else { &end_down }, if zu > zd { &end_down } else { &end_up })
                    } else {
                        notes.push(
                            "a through hole with alike ends: the entry is taken at the end along the axis direction given; either would do".into(),
                        );
                        (true, &end_up, &end_down)
                    }
                }
            }
        };
        let End::Open { faces: entry_faces, at: entry_at, approximate: entry_approx } = entry_end else { continue };
        let dir = if entry_hi { -a } else { a };
        let ordered: Vec<Elem> = if entry_hi { stack.iter().rev().cloned().collect() } else { stack.clone() };
        let depth_of = |h: f64| (h - entry_at) * if entry_hi { -1.0 } else { 1.0 };
        let mut approximate = *entry_approx;
        if entry_faces.len() != 1 {
            notes.push(format!(
                "the entry rim meets {} faces (the hole breaks out across an edge), so the entry face is not one face",
                entry_faces.len()
            ));
        }
        // Steps, from the entry inward.
        let mut steps = Vec::new();
        let mut faces = Vec::new();
        let mut bottom = None;
        let mut bore_diameter = seg.radius * 2.0;
        let mut bottom_at = None;
        for (k, e) in ordered.iter().enumerate() {
            match e {
                Elem::Wall { seg: i, lo, hi } => {
                    let Some(w) = walls.get(*i) else { continue };
                    approximate |= w.approximate;
                    r.walls.extend(w.faces.iter().copied());
                    let (near, far) = if entry_hi { (*hi, *lo) } else { (*lo, *hi) };
                    // A wall followed (further in) by a narrower one is a counterbore.
                    let narrower_follows = ordered
                        .iter()
                        .skip(k + 1)
                        .any(|x| matches!(x, Elem::Wall { seg: j, .. } if walls.get(*j).is_some_and(|v| v.radius < w.radius - t.tol)));
                    let kind = if narrower_follows { "counterbore" } else { "bore" };
                    if kind == "bore" {
                        bore_diameter = w.radius * 2.0;
                    }
                    steps.push(Step {
                        kind,
                        faces: w.faces.clone(),
                        from: depth_of(near),
                        depth: (far - near).abs(),
                        diameter: Some(w.radius * 2.0),
                        angle_deg: None,
                        radius: None,
                    });
                    faces.extend(w.faces.iter().copied());
                    bottom_at = Some(far);
                }
                Elem::Flat { faces: fs, floor } => {
                    faces.extend(fs.iter().copied());
                    if *floor && let Some(&f) = fs.first() {
                        bottom = Some(Bottom::Flat { floor: f });
                    }
                }
                Elem::Cone { faces: fs, half, tip, apex, lo, hi } => {
                    faces.extend(fs.iter().copied());
                    let (near, far) = if entry_hi { (*hi, *lo) } else { (*lo, *hi) };
                    if *tip {
                        bottom = Some(Bottom::Tip { faces: fs.clone(), angle_deg: deg(2.0 * half), depth: (apex - near).abs() });
                    } else {
                        // A countersink: its diameter is the cone's at its end nearest the entry.
                        let diameter = 2.0 * (near - apex).abs() * pmath::tan(*half);
                        steps.push(Step {
                            kind: "countersink",
                            faces: fs.clone(),
                            from: depth_of(near),
                            depth: (far - near).abs(),
                            diameter: Some(diameter),
                            angle_deg: Some(deg(2.0 * half)),
                            radius: None,
                        });
                        bottom_at = Some(far);
                    }
                }
                Elem::Round { faces: fs, minor, lo, hi } => {
                    faces.extend(fs.iter().copied());
                    let (near, far) = if entry_hi { (*hi, *lo) } else { (*lo, *hi) };
                    steps.push(Step {
                        kind: "round",
                        faces: fs.clone(),
                        from: depth_of(near),
                        depth: (far - near).abs(),
                        diameter: None,
                        angle_deg: None,
                        radius: Some(*minor),
                    });
                    // A round at the floor reaches the floor: the depth goes with it.
                    bottom_at = Some(far);
                }
            }
        }
        let bottom = bottom.unwrap_or_else(|| Bottom::Through {
            exit: match exit_end {
                End::Open { faces, .. } if faces.len() == 1 => faces.first().copied(),
                _ => None,
            },
        });
        let depth = match (&bottom, exit_end) {
            (Bottom::Through { .. }, End::Open { at, approximate: ap, .. }) => {
                approximate |= ap;
                depth_of(*at)
            }
            _ => bottom_at.map_or(0.0, depth_of),
        };
        if approximate {
            notes.push("a rim or end is not a circle or a line about the axis, so the depths along the axis are read from the mesh".into());
        }
        let origin = o + a * *entry_at;
        r.claim(&faces);
        r.features.push(Feature {
            kind: Kind::Hole {
                diameter: bore_diameter,
                depth,
                origin,
                direction: dir,
                entry: if entry_faces.len() == 1 { entry_faces.first().copied() } else { None },
                bottom,
                steps,
            },
            faces,
            position: origin,
            approximate,
            notes,
        });
    }
    // Bosses: convex walls with a flat cap at one end and a base at the other.
    for (si, seg) in walls.iter().enumerate() {
        if seg.concave || used.contains(&si) {
            continue;
        }
        let (_, end_up) = walk(t, &walls, si, 1.0, false);
        let (_, end_down) = walk(t, &walls, si, -1.0, false);
        let (o, a) = (seg.origin, seg.axis);
        // A cap: one plane across the axis at a rim, facing along the walk; a base: whatever the
        // other rim meets, facing back.
        let cap_at = |end: &End, dir: f64| -> Option<(usize, f64)> {
            let End::Open { faces, .. } = end else { return None };
            let [f] = faces.as_slice() else { return None };
            let (n, p) = t.plane(*f)?;
            (parallel_either_way(n, a) && n.dot(a * dir) > 0.0).then(|| (*f, along(o, a, p)))
        };
        let base_at = |end: &End, dir: f64| -> Option<(Option<usize>, f64, bool)> {
            let End::Open { faces, at, approximate } = end else { return None };
            let one = if let [f] = faces.as_slice() { Some(*f) } else { None };
            // A flat base across the axis gives the exact position; it faces back along the walk.
            if let Some(f) = one
                && let Some((n, p)) = t.plane(f)
            {
                return (parallel_either_way(n, a) && n.dot(a * dir) < 0.0).then(|| (Some(f), along(o, a, p), false));
            }
            Some((one, *at, *approximate || one.is_none()))
        };
        let found = match (cap_at(&end_up, 1.0), cap_at(&end_down, -1.0)) {
            (Some(cap), None) => base_at(&end_down, -1.0).map(|base| (cap, base, true)),
            (None, Some(cap)) => base_at(&end_up, 1.0).map(|base| (cap, base, false)),
            _ => None,
        };
        let Some(((cap, cap_h), (base, base_h, base_approx), cap_hi)) = found else { continue };
        used.insert(si);
        let height = (cap_h - base_h).abs();
        if !(height > t.tol) {
            continue;
        }
        let dir = if cap_hi { a } else { -a };
        let mut faces = seg.faces.clone();
        faces.push(cap);
        let mut notes = Vec::new();
        let approximate = seg.approximate || base_approx;
        if approximate {
            notes.push("the base is not a flat face across the axis, so the height is read from the wall's mesh".into());
        }
        r.claim(&faces);
        r.walls.extend(seg.faces.iter().copied());
        let origin = o + a * base_h;
        r.features.push(Feature {
            kind: Kind::Boss { diameter: seg.radius * 2.0, height, origin, direction: dir, cap, base },
            faces,
            position: origin,
            approximate,
            notes,
        });
    }
}

fn stack_faces(walls: &[Segment], stack: &[Elem]) -> Vec<usize> {
    stack
        .iter()
        .flat_map(|e| match e {
            Elem::Wall { seg, .. } => walls.get(*seg).map(|w| w.faces.clone()).unwrap_or_default(),
            Elem::Flat { faces, .. } | Elem::Cone { faces, .. } | Elem::Round { faces, .. } => faces.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Pockets: a flat floor whose walls rise to one opening face.

/// What a flat face with upright walls on it turns out to be.
enum Floor {
    Pocket(Feature),
    /// Nearly a pocket: its faces and why not.
    Nearly(Vec<usize>, String),
    No,
}

/// Whether face `g` stands upright on a floor whose outward normal is `n`: a plane along `n`
/// or a cylinder about it.
fn upright(t: &Topo, g: usize, n: Vec3) -> bool {
    match t.surface(g) {
        Some(FaceSurface::Plane { normal, .. }) => normal.vec().dot(n).abs() < ANG,
        Some(FaceSurface::Cylinder { axis, .. }) => parallel_either_way(axis.vec(), n),
        _ => false,
    }
}

/// Follows the round `g` on the edge of the floor `fi` (outward normal `n`) through tangent
/// edges to the upright faces beyond it: a cutter-radius fillet along a pocket's floor, and the
/// corner patches where such rounds meet. The round's faces go to `rounds`, the upright faces
/// it reaches to `walls`. `false` when `g` is no concave round lying on the floor.
fn round_to_walls(t: &Topo, r: &Recognised, fi: usize, g: usize, n: Vec3, rounds: &mut Vec<usize>, walls: &mut Vec<usize>) -> bool {
    let is_round =
        |f: usize| t.faces.get(f).is_some_and(|x| x.sign < 0.0 && x.surface.as_ref().and_then(blend_radius).is_some()) && !upright(t, f, n);
    if !is_round(g) {
        return false;
    }
    let mut queue = vec![g];
    let mut seen: BTreeSet<usize> = BTreeSet::from([fi]);
    while let Some(f) = queue.pop() {
        if !seen.insert(f) {
            continue;
        }
        if !rounds.contains(&f) {
            rounds.push(f);
        }
        let Some(face) = t.faces.get(f) else { continue };
        for &ei in &face.edges {
            let Some(e) = t.edges.get(ei) else { continue };
            if !e.tangent() {
                continue;
            }
            let Some(h) = e.other(f) else { continue };
            if seen.contains(&h) || r.owns(h) {
                continue;
            }
            if upright(t, h, n) {
                seen.insert(h);
                if !walls.contains(&h) {
                    walls.push(h);
                }
            } else if is_round(h) {
                queue.push(h);
            }
        }
    }
    true
}

/// Reads face `fi` as a pocket floor: every edge of it concave to an upright wall (or to a hole
/// or boss in the floor), or tangent to a round that leads to one, every wall rising at a convex
/// edge to one flat opening face above.
fn floor(t: &Topo, r: &Recognised, fi: usize) -> Floor {
    let Some(face) = t.faces.get(fi) else { return Floor::No };
    let Some((n, p)) = t.plane(fi) else { return Floor::No };
    let mut walls: Vec<usize> = Vec::new();
    let mut rounds: Vec<usize> = Vec::new();
    let mut open_sides: Vec<usize> = Vec::new();
    for &ei in &face.edges {
        let Some(e) = t.edges.get(ei) else { continue };
        let Some(g) = e.other(fi) else { continue };
        if r.owns(g) {
            continue;
        }
        if e.tangent() {
            // A round along the floor's edge (the commonest milled pocket has its cutter's
            // radius there): the walls are the upright faces beyond it.
            if !round_to_walls(t, r, fi, g, n, &mut rounds, &mut walls) {
                return Floor::No;
            }
            continue;
        }
        let upright = upright(t, g, n);
        match (e.convex, upright) {
            (Some(false), true) => {
                if !walls.contains(&g) {
                    walls.push(g);
                }
            }
            (Some(true), true) => open_sides.push(g),
            _ => return Floor::No,
        }
    }
    if walls.is_empty() {
        return Floor::No;
    }
    // Each wall rises to a flat opening face above the floor, across a convex edge.
    let mut opening: Vec<usize> = Vec::new();
    let mut depth: Option<f64> = None;
    for &w in &walls {
        let Some(wf) = t.faces.get(w) else { continue };
        let mut reached = false;
        for &ei in &wf.edges {
            let Some(e) = t.edges.get(ei) else { continue };
            let Some(g) = e.other(w) else { continue };
            if g == fi || walls.contains(&g) || rounds.contains(&g) || open_sides.contains(&g) || r.owns(g) || e.tangent() {
                continue;
            }
            let Some((gn, gp)) = t.plane(g) else { return Floor::No };
            let h = (gp - p).dot(n);
            if !(parallel(gn, n) && h > t.tol && e.convex == Some(true)) {
                return Floor::No;
            }
            if depth.is_some_and(|d| (d - h).abs() > t.tol) {
                let mut fs = vec![fi];
                fs.extend(walls.iter().copied());
                return Floor::Nearly(
                    fs,
                    format!(
                        "a flat floor with upright walls, but they reach opening faces at two heights ({} and {})",
                        r9(depth.unwrap_or(h)),
                        r9(h)
                    ),
                );
            }
            depth = Some(h);
            if !opening.contains(&g) {
                opening.push(g);
            }
            reached = true;
        }
        if !reached && open_sides.is_empty() {
            return Floor::No;
        }
    }
    let mut faces = vec![fi];
    faces.extend(walls.iter().copied());
    faces.extend(rounds.iter().copied());
    if !open_sides.is_empty() {
        return Floor::Nearly(
            faces,
            format!(
                "a recess open at its side: its floor meets {} at a convex edge, so the walls do not close around it",
                t.names_of(&open_sides).join(", ")
            ),
        );
    }
    let Some(depth) = depth else { return Floor::No };
    Floor::Pocket(Feature {
        kind: Kind::Pocket { depth, floor: fi, opening, walls, rounds, normal: n },
        faces,
        position: face.centroid,
        approximate: false,
        notes: Vec::new(),
    })
}

fn pockets(t: &Topo, r: &mut Recognised) {
    let mut nearly = Vec::new();
    for fi in 0..t.faces.len() {
        if r.owns(fi) {
            continue;
        }
        match floor(t, r, fi) {
            Floor::Pocket(f) => {
                r.claim(&f.faces);
                r.features.push(f);
            }
            Floor::Nearly(faces, reason) => nearly.push((faces, reason)),
            Floor::No => {}
        }
    }
    // A wall of a pocket read as a floor of its own is not nearly a pocket.
    for (faces, reason) in nearly {
        if faces.iter().any(|f| r.owns(*f)) {
            continue;
        }
        r.uncertain.push(Uncertain { could_be: "pocket", faces, reason });
    }
}

// ---------------------------------------------------------------------------------------------
// Fillets: constant-radius blends tangent to the faces they join, chained where they meet.

fn blend_radius(s: &FaceSurface) -> Option<f64> {
    match *s {
        FaceSurface::Cylinder { radius, .. } => Some(radius),
        FaceSurface::Torus { minor, .. } => Some(minor.abs()),
        FaceSurface::Sphere { radius, .. } => Some(radius),
        _ => None,
    }
}

/// The surfaces face `i` is tangent to (its own pieces not counted, pieces of one neighbouring
/// surface once), each with the body's outward normal where they meet.
fn tangent_neighbours(t: &Topo, i: usize) -> Vec<(usize, Vec3)> {
    let Some(f) = t.faces.get(i) else { return Vec::new() };
    let mut out: Vec<(usize, Vec3)> = Vec::new();
    for &ei in &f.edges {
        let Some(e) = t.edges.get(ei) else { continue };
        if !e.tangent() {
            continue;
        }
        let Some(&p) = e.pts.get(e.pts.len() / 2) else { continue };
        for &g in &e.faces {
            if f.group.contains(&g) {
                continue;
            }
            let key = t.faces.get(g).and_then(|x| x.group.first().copied()).unwrap_or(g);
            if out.iter().any(|(k, _)| *k == key) {
                continue;
            }
            let Some(n) = t.normal(i, p) else { continue };
            out.push((key, n));
        }
    }
    out
}

/// Whether the faces a round is tangent to could have met at an edge the round replaces: two of
/// them turn toward each other across it. A half-round between two parallel faces (a slot's end,
/// a full round on a plate's edge) joins faces that never meet, so it is no blend.
fn joins_could_meet(neighbours: &[(usize, Vec3)]) -> bool {
    neighbours.iter().enumerate().any(|(i, (_, a))| neighbours.iter().skip(i + 1).any(|(_, b)| !parallel_either_way(*a, *b)))
}

fn blends(t: &Topo, r: &mut Recognised) {
    // A blend face is tangent to at least two other surfaces (its own pieces not counted) that
    // could have met at the edge it replaces. A bore or boss wall tangent to the rounds at its
    // ends is not a blend; any other face a feature owns (a pocket's corner round, a hole's
    // mouth round) is also the fillet it is.
    let is_blend: Vec<bool> = t
        .faces
        .iter()
        .enumerate()
        .map(|(i, f)| {
            blend_radius(f.surface.as_ref()?)?;
            if r.walls.contains(&i) {
                return Some(false);
            }
            let neighbours = tangent_neighbours(t, i);
            Some(neighbours.len() >= 2 && joins_could_meet(&neighbours))
        })
        .map(|b| b.unwrap_or(false))
        .collect();
    // Chains: blends of one radius and convexity that meet (tangent along a loop of edges, or
    // mitred at a corner where the third edge is not blended).
    let n = t.faces.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while let Some(&q) = p.get(i) {
            if q == i {
                break;
            }
            i = q;
        }
        i
    }
    for e in &t.edges {
        let [a, b] = e.faces.as_slice() else { continue };
        if !(is_blend.get(*a) == Some(&true) && is_blend.get(*b) == Some(&true)) {
            continue;
        }
        let (Some(ra), Some(rb)) = (t.surface(*a).and_then(blend_radius), t.surface(*b).and_then(blend_radius)) else { continue };
        let same_side = t.faces.get(*a).map(|f| f.sign) == t.faces.get(*b).map(|f| f.sign);
        if (ra - rb).abs() < t.tol && same_side {
            let (x, y) = (root(&mut parent, *a), root(&mut parent, *b));
            if let Some(slot) = parent.get_mut(x) {
                *slot = y;
            }
        }
    }
    let mut chains: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        if is_blend.get(i) == Some(&true) {
            chains.entry(root(&mut parent, i)).or_default().push(i);
        }
    }
    for members in chains.values() {
        let Some(&first) = members.first() else { continue };
        let Some(radius) = t.surface(first).and_then(blend_radius) else { continue };
        // What the chain joins: the faces tangent to it that are not blends of the chain.
        let mut joins: Vec<usize> = Vec::new();
        let mut convex: Option<Option<bool>> = None;
        let mut along: Option<Option<Vec3>> = None;
        for &m in members {
            let Some(f) = t.faces.get(m) else { continue };
            for &ei in &f.edges {
                let Some(e) = t.edges.get(ei) else { continue };
                if !e.tangent() {
                    continue;
                }
                for &g in &e.faces {
                    if !members.contains(&g) && !joins.contains(&g) {
                        joins.push(g);
                    }
                }
            }
            let c = Some(f.sign > 0.0);
            convex = Some(match convex {
                None => c,
                Some(prev) if prev == c => c,
                Some(_) => None,
            });
            let d = match f.surface {
                Some(FaceSurface::Cylinder { axis, .. }) => Some(axis.vec()),
                _ => None,
            };
            along = Some(match along {
                None => d,
                Some(Some(prev)) if d.is_some_and(|d| parallel_either_way(prev, d)) => Some(prev),
                Some(_) => None,
            });
        }
        // Where the chain is: its blends' centres (a cylinder's axis midpoint, a torus's or
        // sphere's centre), averaged.
        let mut centres = Vec::new();
        for &m in members {
            match t.surface(m) {
                Some(FaceSurface::Cylinder { origin, axis, .. }) => {
                    let (lo, hi, _) = span(t, &[m], *origin, axis.vec());
                    centres.push(*origin + axis.vec() * ((lo + hi) * 0.5));
                }
                Some(FaceSurface::Torus { center, .. }) | Some(FaceSurface::Sphere { center, .. }) => centres.push(*center),
                _ => {}
            }
        }
        let position = centres.iter().fold(Vec3::ZERO, |a, c| a + *c) / centres.len().max(1) as f64;
        joins.sort_unstable();
        r.features.push(Feature {
            kind: Kind::Fillet { radius, convex: convex.flatten(), joins, along: along.flatten() },
            faces: members.clone(),
            position,
            approximate: false,
            notes: Vec::new(),
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Chamfers: a flat (or conical) face cutting a convex edge at an angle to both its faces, and
// narrower than the faces it joins: a bevel in place of the edge, not a face of its own.

/// A planar chamfer candidate: its two joined faces, the lines of its two long edges, its width
/// across.
struct ChamferCandidate {
    face: usize,
    joins: [usize; 2],
    lines: [(Vec3, Vec3); 2],
    width: f64,
}

fn planar_candidate(t: &Topo, fi: usize, r: &Recognised) -> Option<ChamferCandidate> {
    if r.owns(fi) {
        return None;
    }
    let face = t.faces.get(fi)?;
    t.plane(fi)?;
    // Straight convex edges turning strictly between 0 and 90°, with the face beyond.
    let mut joins: Vec<(usize, Vec3, Vec3, f64)> = Vec::new();
    for &ei in &face.edges {
        let e = t.edges.get(ei)?;
        let (Some((p, q)), Some(turn), Some(true)) = (e.line, e.turn, e.convex) else { continue };
        if !(turn > CHAMFER_ANG && turn < FRAC_PI_2 - CHAMFER_ANG) {
            continue;
        }
        let g = e.other(fi)?;
        joins.push((g, p, q, e.length()));
    }
    // The pair of parallel lines to different faces with the greatest length.
    let mut best: Option<(f64, usize, usize)> = None;
    for i in 0..joins.len() {
        for j in i + 1..joins.len() {
            let (Some(a), Some(b)) = (joins.get(i), joins.get(j)) else { continue };
            if a.0 == b.0 {
                continue;
            }
            let (da, db) = ((a.2 - a.1).normalized(), (b.2 - b.1).normalized());
            let (Some(da), Some(db)) = (da, db) else { continue };
            if !parallel_either_way(da, db) || dist_to_line(b.1, a.1, da) < t.tol {
                continue;
            }
            let len = a.3 + b.3;
            if best.is_none_or(|(l, ..)| len > l) {
                best = Some((len, i, j));
            }
        }
    }
    let (_, i, j) = best?;
    let (a, b) = (joins.get(i)?, joins.get(j)?);
    let da = (a.2 - a.1).normalized()?;
    Some(ChamferCandidate { face: fi, joins: [a.0, b.0], lines: [(a.1, a.2), (b.1, b.2)], width: dist_to_line(b.1, a.1, da) })
}

fn chamfers(t: &Topo, r: &mut Recognised) {
    let candidates: Vec<ChamferCandidate> = (0..t.faces.len()).filter_map(|fi| planar_candidate(t, fi, r)).collect();
    for c in &candidates {
        // A candidate whose joined face is a narrower candidate is the face that chamfer cuts
        // into, not a chamfer itself (a plate's top between two chamfers).
        if candidates.iter().any(|o| c.joins.contains(&o.face) && o.joins.contains(&c.face) && o.width < c.width - t.tol) {
            continue;
        }
        let (Some((n_c, _)), Some((n_a, p_a)), Some((n_b, p_b))) = (t.plane(c.face), t.plane(c.joins[0]), t.plane(c.joins[1])) else { continue };
        let sin_ab = n_a.cross(n_b).len();
        if sin_ab < ANG {
            continue;
        }
        // The distance along A: from the chamfer's edge on A to where A and B would meet.
        let d_along = |p_on: Vec3, n_other: Vec3, p_other: Vec3| ((p_on - p_other).dot(n_other)).abs() / sin_ab;
        let d_a = d_along(c.lines[0].0, n_b, p_b);
        let d_b = d_along(c.lines[1].0, n_a, p_a);
        // A chamfer is a narrow bevel in place of the edge where the two faces would meet: it
        // reaches less far along each face than the face keeps beyond it. A face as wide as
        // what its neighbours would reach is a face of its own (a hexagonal prism's sides turn
        // 60° to each other across parallel edges, a drafted box's top turns 80° to its sides).
        let Some(along) = (c.lines[0].1 - c.lines[0].0).normalized() else { continue };
        let (Some(keep_a), Some(keep_b)) = (t.extent_across(c.joins[0], along), t.extent_across(c.joins[1], along)) else { continue };
        if d_a >= keep_a - t.tol || d_b >= keep_b - t.tol {
            continue;
        }
        // The first face is the one facing up most (as solid.chamfer takes it); ties by name.
        let first_is_a = match n_a.z.partial_cmp(&n_b.z) {
            Some(std::cmp::Ordering::Greater) => true,
            Some(std::cmp::Ordering::Less) => false,
            _ => t.name(c.joins[0]) <= t.name(c.joins[1]),
        };
        let (joins, distance, distance2, n_first) =
            if first_is_a { (vec![c.joins[0], c.joins[1]], d_a, d_b, n_a) } else { (vec![c.joins[1], c.joins[0]], d_b, d_a, n_b) };
        let angle_deg = deg(pmath::acos(n_c.dot(n_first).clamp(-1.0, 1.0)));
        let along = Some(along);
        r.claim(&[c.face]);
        let position = t.faces.get(c.face).map_or(Vec3::ZERO, |f| f.centroid);
        r.features.push(Feature {
            kind: Kind::Chamfer { distance, distance2, angle_deg, joins, along },
            faces: vec![c.face],
            position,
            approximate: false,
            notes: Vec::new(),
        });
    }
    // Conical chamfers: a cone between a flat face across its axis and a coaxial cylinder,
    // turning less than 90° to each.
    for fi in 0..t.faces.len() {
        let Some(face) = t.faces.get(fi) else { continue };
        if r.owns(fi) || face.group.first() != Some(&fi) {
            continue;
        }
        let Some(&FaceSurface::Cone { apex, axis, half_angle }) = t.surface(fi) else { continue };
        let (o, a) = (apex, axis.vec());
        let mut plane: Option<(usize, f64)> = None;
        let mut cyl: Option<(usize, f64)> = None;
        let mut ok = true;
        for (ei, g) in t.boundary(&face.group) {
            let Some(e) = t.edges.get(ei) else { continue };
            let Some(c) = e.circle.filter(|c| parallel_either_way(c.axis.vec(), a)) else {
                ok = false;
                break;
            };
            let turning = e.turn.is_some_and(|tn| tn > CHAMFER_ANG && tn < FRAC_PI_2 - CHAMFER_ANG) && e.convex == Some(true);
            match t.surface(g) {
                Some(s @ FaceSurface::Plane { .. }) if turning && coaxial(s, o, a, t.tol) => plane = Some((g, c.radius)),
                Some(s @ FaceSurface::Cylinder { .. }) if turning && coaxial(s, o, a, t.tol) => cyl = Some((g, c.radius)),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        let (true, Some((pf, r_plane)), Some((cf, r_cyl))) = (ok, plane, cyl) else { continue };
        let radial = (r_plane - r_cyl).abs();
        let axial = radial / pmath::tan(half_angle);
        // Along the flat face first (it faces along the axis), then along the cylinder.
        let angle_deg = deg(FRAC_PI_2 - half_angle);
        let cyl_group = t.faces.get(cf).map(|f| f.group.clone()).unwrap_or_else(|| vec![cf]);
        let mut joins = vec![pf];
        joins.extend(cyl_group);
        r.claim(&face.group);
        let position = o + a * ((r_plane.min(r_cyl) + radial * 0.5) / pmath::tan(half_angle));
        r.features.push(Feature {
            kind: Kind::Chamfer { distance: radial, distance2: axial, angle_deg, joins, along: None },
            faces: face.group.clone(),
            position,
            approximate: false,
            notes: Vec::new(),
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Patterns: alike features on a lattice or a circle.

/// A lattice fit: the members at `p0 + i * v1 + j * v2`, every cell filled.
struct Lattice {
    count1: usize,
    v1: Vec3,
    count2: usize,
    v2: Vec3,
}

/// The points as one row (`count2` 1) or one grid, every cell filled, else `None`. Linear in the
/// points.
fn lattice(points: &[Vec3], tol: f64) -> Option<Lattice> {
    let n = points.len();
    if n < MIN_PATTERN {
        return None;
    }
    // The vectors are the steps to the nearest member and the nearest one off that line, from
    // any member (the first): the fitted positions carry noise of about 1e-14, so the member
    // lowest in x, y, z may well lie mid-row, and the cells are counted from the least along
    // each vector afterwards instead. The vectors point up (+z, then +y, then +x), the same
    // whichever member they were taken from.
    let p0 = *points.first()?;
    let mut by_dist: Vec<Vec3> = points.iter().filter(|p| p.dist(p0) > tol).map(|p| *p - p0).collect();
    by_dist.sort_by(|a, b| a.len().total_cmp(&b.len()));
    let v1 = upward(*by_dist.first()?);
    let d1 = v1.normalized()?;
    let collinear = points.iter().all(|p| dist_to_line(*p, p0, d1) < tol);
    let v2 = if collinear { Vec3::ZERO } else { upward(by_dist.iter().copied().find(|v| v.cross(d1).len() > tol)?) };
    let nrm = if collinear { None } else { Some(v1.cross(v2).normalized()?) };
    // Integer coordinates of each member in (v1, v2); every member must sit on a cell.
    let mut cells: Vec<(i64, i64)> = Vec::with_capacity(n);
    for p in points {
        let w = *p - p0;
        let (i, j) = match nrm {
            Some(nrm) => {
                let det = v1.cross(v2).dot(nrm);
                (w.cross(v2).dot(nrm) / det, v1.cross(w).dot(nrm) / det)
            }
            None => (w.dot(d1) / v1.len(), 0.0),
        };
        let (ri, rj) = (i.round(), j.round());
        if !(ri.is_finite() && rj.is_finite()) || (p0 + v1 * ri + v2 * rj).dist(*p) > tol {
            return None;
        }
        cells.push((ri as i64, rj as i64));
    }
    let i0 = cells.iter().map(|c| c.0).min()?;
    let j0 = cells.iter().map(|c| c.1).min()?;
    for c in &mut cells {
        *c = (c.0 - i0, c.1 - j0);
    }
    let c1 = cells.iter().map(|c| c.0).max()? + 1;
    let c2 = cells.iter().map(|c| c.1).max()? + 1;
    if c1.checked_mul(c2).and_then(|c| usize::try_from(c).ok()) != Some(n) {
        return None;
    }
    cells.sort_unstable();
    cells.dedup();
    if cells.len() != n {
        return None;
    }
    if collinear {
        return Some(Lattice { count1: n, v1, count2: 1, v2: Vec3::ZERO });
    }
    // The first direction is the one nearest the x axis (then y).
    let key = |v: Vec3| (-(v.x.abs() / v.len()), -(v.y.abs() / v.len()));
    let (v1, c1, v2, c2) = if key(v1) <= key(v2) { (v1, c1, v2, c2) } else { (v2, c2, v1, c1) };
    Some(Lattice { count1: c1 as usize, v1, count2: c2 as usize, v2 })
}

/// Every maximal run of `min` or more points equally stepped along a line, from every pair's
/// direction and pitch, as sorted indices into `points`. Cubic in the points.
fn runs(points: &[Vec3], tol: f64, min: usize) -> Vec<Vec<usize>> {
    let mut out: BTreeSet<Vec<usize>> = BTreeSet::new();
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let (Some(&pi), Some(&pj)) = (points.get(i), points.get(j)) else { continue };
            let v = pj - pi;
            let Some(d) = v.normalized() else { continue };
            // The points on the line at whole steps from `pi`, by step.
            let mut on: Vec<(i64, usize)> = Vec::new();
            for (k, pk) in points.iter().enumerate() {
                if dist_to_line(*pk, pi, d) > tol {
                    continue;
                }
                let m = (*pk - pi).dot(d) / v.len();
                if (m - m.round()).abs() * v.len() < tol && m.round().is_finite() {
                    on.push((m.round() as i64, k));
                }
            }
            on.sort_unstable();
            let mut run: Vec<usize> = Vec::new();
            let mut prev: Option<i64> = None;
            for (idx, &(m, k)) in on.iter().enumerate() {
                if prev.is_some_and(|pm| pm + 1 != m) {
                    run.clear();
                }
                run.push(k);
                prev = Some(m);
                let last = on.get(idx + 1).is_none_or(|(nm, _)| *nm != m + 1);
                if last && run.len() >= min {
                    let mut r = run.clone();
                    r.sort_unstable();
                    out.insert(r);
                }
            }
        }
    }
    out.into_iter().collect()
}

/// A pattern read from part of a group.
enum Fit {
    Linear(Lattice),
    Ring(Circle),
}

/// The grids, rows and rings in a group that is not one lattice or one circle, as sorted indices
/// into `pts`: every row of three or more equally stepped on a line; every grid of such rows
/// alike in direction, pitch and count whose starts are equally stepped themselves (two rows
/// only when side by side, straight across their direction); every rectangle of four; and every
/// circle through three with the members on it. The largest first, a grid or row before a ring
/// of the same size. Cubic in the points.
fn partial_patterns(pts: &[Vec3], dir: Option<Vec3>, tol: f64) -> Vec<(Vec<usize>, Fit)> {
    let mut out: Vec<(Vec<usize>, Fit)> = Vec::new();
    let mut seen: BTreeSet<Vec<usize>> = BTreeSet::new();
    let at = |set: &[usize]| -> Vec<Vec3> { set.iter().filter_map(|k| pts.get(*k).copied()).collect() };
    let rows = runs(pts, tol, MIN_PATTERN);
    // Rows alike in count, direction and pitch, each with its start (the member lowest along
    // the direction).
    let mut alike: Vec<(usize, Vec3, f64, Vec<(usize, Vec3)>)> = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        let on = at(row);
        let (Some(&a), Some(&b)) = (on.first(), on.get(1)) else { continue };
        let Some(d) = upward(b - a).normalized() else { continue };
        let Some(lo) = on.iter().copied().min_by(|p, q| p.dot(d).total_cmp(&q.dot(d))) else { continue };
        let Some(hi) = on.iter().copied().max_by(|p, q| p.dot(d).total_cmp(&q.dot(d))) else { continue };
        let pitch = (hi - lo).dot(d) / on.len().saturating_sub(1).max(1) as f64;
        match alike.iter_mut().find(|(c, ad, ap, _)| *c == on.len() && parallel(*ad, d) && (*ap - pitch).abs() < tol) {
            Some((_, _, _, rs)) => rs.push((ri, lo)),
            None => alike.push((on.len(), d, pitch, vec![(ri, lo)])),
        }
    }
    for (_, d, _, rs) in &alike {
        if rs.len() < 2 {
            continue;
        }
        let starts: Vec<Vec3> = rs.iter().map(|(_, s)| *s).collect();
        for run in runs(&starts, tol, 2) {
            if let [a, b] = run.as_slice()
                && let (Some(sa), Some(sb)) = (starts.get(*a), starts.get(*b))
                && (*sb - *sa).dot(*d).abs() > tol
            {
                // Two rows alone make a grid only straight across their direction: offset,
                // they are two rows.
                continue;
            }
            let mut set: Vec<usize> = run.iter().filter_map(|i| rs.get(*i)).flat_map(|(ri, _)| rows.get(*ri).cloned().unwrap_or_default()).collect();
            set.sort_unstable();
            set.dedup();
            if let Some(l) = lattice(&at(&set), tol)
                && l.count2 > 1
                && seen.insert(set.clone())
            {
                out.push((set, Fit::Linear(l)));
            }
        }
    }
    // Rectangles: two members straight across from two others are a 2 × 2 grid (four alone
    // are one lattice, so four among others are one too, before a ring of four).
    for i in 0..pts.len() {
        for j in 0..pts.len() {
            let (Some(&pi), Some(&pj)) = (pts.get(i), pts.get(j)) else { continue };
            let v = pj - pi;
            let Some(d) = v.normalized() else { continue };
            for k in j + 1..pts.len() {
                let Some(&pk) = pts.get(k).filter(|_| k != i) else { continue };
                let w = pk - pi;
                if w.len() <= tol || w.dot(d).abs() > tol {
                    continue;
                }
                let far = pi + v + w;
                let Some(m) = pts.iter().position(|p| p.dist(far) < tol) else { continue };
                let mut set = vec![i, j, k, m];
                set.sort_unstable();
                set.dedup();
                if set.len() == 4
                    && !seen.contains(&set)
                    && let Some(l) = lattice(&at(&set), tol)
                    && l.count2 > 1
                    && seen.insert(set.clone())
                {
                    out.push((set, Fit::Linear(l)));
                }
            }
        }
    }
    for row in rows {
        if let Some(l) = lattice(&at(&row), tol)
            && seen.insert(row.clone())
        {
            out.push((row, Fit::Linear(l)));
        }
    }
    // Rings: every triple's circle, the members on it.
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            for k in j + 1..pts.len() {
                let (Some(&pi), Some(&pj), Some(&pk)) = (pts.get(i), pts.get(j), pts.get(k)) else { continue };
                let Some(c) = circle(&[pi, pj, pk], dir, tol) else { continue };
                let on: Vec<usize> = pts
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| (p.dist(c.center) - c.radius).abs() < tol && (**p - c.center).dot(c.axis).abs() < tol)
                    .map(|(m, _)| m)
                    .collect();
                if on.len() < MIN_PATTERN || seen.contains(&on) {
                    continue;
                }
                if let Some(c) = circle(&at(&on), dir, tol)
                    && seen.insert(on.clone())
                {
                    out.push((on, Fit::Ring(c)));
                }
            }
        }
    }
    out.sort_by_key(|(set, fit)| (std::cmp::Reverse(set.len()), matches!(fit, Fit::Ring(_))));
    out
}

/// A circle fit about an axis parallel to `axis_dir`: centre, radius, angle step, full circle.
struct Circle {
    center: Vec3,
    axis: Vec3,
    radius: f64,
    step: f64,
    full: bool,
}

fn circle(points: &[Vec3], axis_dir: Option<Vec3>, tol: f64) -> Option<Circle> {
    let n = points.len();
    if n < MIN_PATTERN {
        return None;
    }
    let (a, b, c) = (*points.first()?, *points.get(1)?, *points.get(2)?);
    let (u, v) = (b - a, c - a);
    let w = u.cross(v);
    if w.len2() <= u.len2() * v.len2() * 1e-16 {
        return None;
    }
    let normal = w.normalized()?;
    let axis = match axis_dir {
        Some(d) => {
            if !parallel_either_way(d, normal) {
                return None;
            }
            d
        }
        // The three points' normal depends on their order; the same circle gets the same axis.
        None => upward(normal),
    };
    let center = a + (v * u.len2() - u * v.len2()).cross(w) / (2.0 * w.len2());
    let radius = center.dist(a);
    if points.iter().any(|p| (p.dist(center) - radius).abs() > tol || (*p - center).dot(axis).abs() > tol) {
        return None;
    }
    let x = (a - center).normalized()?;
    let y = axis.cross(x);
    let mut angles: Vec<f64> = points
        .iter()
        .map(|p| {
            let d = *p - center;
            let t = pmath::atan2(d.dot(y), d.dot(x));
            if t < 0.0 { t + TAU } else { t }
        })
        .collect();
    angles.sort_by(f64::total_cmp);
    // The steps between neighbours around the circle, the one from the last member back to the
    // first included: the members are equally stepped, with at most one larger gap (a partial
    // arc; none on a full circle), whichever member the angles were measured from.
    let mut steps: Vec<f64> = angles.windows(2).filter_map(|w| w.first().zip(w.get(1)).map(|(a, b)| b - a)).collect();
    steps.push(TAU - (angles.last()? - angles.first()?));
    let step = steps.iter().copied().fold(f64::INFINITY, f64::min);
    let ang_tol = tol / radius.max(tol);
    if !(step > ang_tol) {
        return None;
    }
    let larger = steps.iter().filter(|s| (*s - step).abs() > ang_tol).count();
    if larger > 1 {
        return None;
    }
    Some(Circle { center, axis, radius, step, full: larger == 0 })
}

/// `v` or its reverse, whichever points up (+z; then +y, then +x when level).
fn upward(v: Vec3) -> Vec3 {
    let up = if v.z.abs() > ANG {
        v.z > 0.0
    } else if v.y.abs() > ANG {
        v.y > 0.0
    } else {
        v.x >= 0.0
    };
    if up { v } else { -v }
}

/// The body's patterns, and notes on what was not searched.
fn patterns(t: &Topo, features: &[Feature]) -> (Vec<Value>, Vec<String>) {
    // Alike features: same type, sizes and direction. An approximate feature (read partly from
    // the mesh) joins none.
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, f) in features.iter().enumerate() {
        if f.approximate {
            continue;
        }
        groups.entry(f.signature()).or_default().push(i);
    }
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let faces_of = |members: &[usize]| -> Vec<String> {
        let fs: Vec<usize> = members.iter().filter_map(|m| features.get(*m)).flat_map(|f| f.faces.iter().copied()).collect();
        t.names_of(&fs)
    };
    let emit = |members: &[usize], f: &Feature, fit: &Fit| {
        let mut v = json!({"feature_type": f.type_name(), "members": members, "count": members.len(), "faces": faces_of(members)});
        match fit {
            Fit::Linear(l) => {
                v["type"] = json!("linear");
                v["count1"] = json!(l.count1);
                v["pitch1"] = json!(r9(l.v1.len()));
                v["direction1"] = v9(l.v1.normalized().unwrap_or(Vec3::ZERO));
                if l.count2 > 1 {
                    v["count2"] = json!(l.count2);
                    v["pitch2"] = json!(r9(l.v2.len()));
                    v["direction2"] = v9(l.v2.normalized().unwrap_or(Vec3::ZERO));
                }
            }
            Fit::Ring(c) => {
                v["type"] = json!("circular");
                v["angle_step_deg"] = json!(r9(deg(c.step)));
                v["full_circle"] = json!(c.full);
                v["radius"] = json!(r9(c.radius));
                v["axis"] = json!({"origin": v9(c.center), "direction": v9(c.axis)});
            }
        }
        if let Some(d) = f.direction() {
            v["member_direction"] = v9(d);
        }
        v
    };
    for members in groups.values() {
        if members.len() < MIN_PATTERN {
            continue;
        }
        let Some(first) = members.first().and_then(|m| features.get(*m)) else { continue };
        let dir = first.direction();
        // Members must be aligned alike.
        if members.iter().filter_map(|m| features.get(*m)).any(|f| match (f.direction(), dir) {
            (Some(a), Some(b)) => !parallel(a, b),
            (None, None) => false,
            _ => true,
        }) {
            continue;
        }
        let pts: Vec<Vec3> = members.iter().filter_map(|m| features.get(*m)).map(|f| f.position).collect();
        // The whole group as one lattice or one circle (linear in the members, so a plate of
        // any number of holes is read), else its grids, rows and rings (cubic, so only up to
        // MAX_PATTERN_GROUP members; a larger group is noted).
        if let Some(l) = lattice(&pts, t.tol) {
            out.push(emit(members, first, &Fit::Linear(l)));
            continue;
        }
        if let Some(c) = circle(&pts, dir, t.tol) {
            out.push(emit(members, first, &Fit::Ring(c)));
            continue;
        }
        if members.len() > MAX_PATTERN_GROUP {
            notes.push(format!(
                "{} alike `{}` features are not one lattice or one circle, and more than {MAX_PATTERN_GROUP} are too many to search for the grids, rows and rings among them, so they form no pattern",
                members.len(),
                first.type_name()
            ));
            continue;
        }
        // The reading that explains the most members with the fewest patterns: grids, rows and
        // rings together, the largest first (a grid or row before a ring of the same size), each
        // taken when none of its members is in a pattern already. A 3 × 3 grid beside a lone
        // hole is the grid and the hole, not a ring of four about its centre; a ring of six
        // holes about a seventh at its centre is the ring and a lone hole, not a row of three
        // through the centre.
        let mut taken: BTreeSet<usize> = BTreeSet::new();
        for (set, fit) in partial_patterns(&pts, dir, t.tol) {
            let ms: Vec<usize> = set.iter().filter_map(|k| members.get(*k).copied()).collect();
            if ms.len() != set.len() || ms.iter().any(|m| taken.contains(m)) {
                continue;
            }
            out.push(emit(&ms, first, &fit));
            taken.extend(ms);
        }
    }
    (out, notes)
}

// ---------------------------------------------------------------------------------------------
// What no feature covers, and why.

fn unrecognised(t: &Topo, r: &Recognised) -> Vec<Value> {
    let covered: BTreeSet<usize> = r.features.iter().flat_map(|f| f.faces.iter().copied()).collect();
    (0..t.faces.len())
        .filter(|f| !covered.contains(f))
        .map(|f| {
            let s = t.surface(f);
            let face = t.faces.get(f);
            // Tangent to another surface (its own other pieces do not count).
            let tangent_count = face.map_or(0, |x| {
                x.edges.iter().filter(|&&ei| t.edges.get(ei).is_some_and(|e| e.tangent() && e.faces.iter().any(|g| !x.group.contains(g)))).count()
            });
            let reason = match s {
                None => "a free-form face: not a plane, cylinder, cone, sphere or torus, so no feature reads it".to_string(),
                Some(FaceSurface::Plane { .. }) => {
                    "a planar face no feature covers (the face holes are drilled into, pockets are cut into and bosses stand on is not part of them)"
                        .to_string()
                }
                Some(FaceSurface::Cylinder { .. }) => {
                    let concave = face.is_some_and(|x| x.sign < 0.0);
                    let neighbours = tangent_neighbours(t, f);
                    match (concave, tangent_count) {
                        (true, 0) => {
                            "a concave cylindrical face that is not a bore: its ends do not meet a rim, step or floor about its axis".to_string()
                        }
                        (false, 0) => "a convex cylindrical face that is not a boss: it has no flat cap at one end and base at the other".to_string(),
                        _ if neighbours.len() >= 2 && !joins_could_meet(&neighbours) => {
                            "a half-round between parallel faces (a slot's end, a full round on an edge): the faces it joins never meet, so it is no fillet"
                                .to_string()
                        }
                        _ => "a cylindrical face tangent to a neighbour but not a constant-radius blend between two faces".to_string(),
                    }
                }
                Some(FaceSurface::Cone { .. }) => {
                    "a conical face that is neither a countersink or drill point of a hole nor a chamfer between a flat face and a coaxial cylinder"
                        .to_string()
                }
                Some(FaceSurface::Sphere { .. }) => "a spherical face not tangent to two faces (not a blend corner)".to_string(),
                Some(FaceSurface::Torus { .. }) => "a toroidal face not tangent to two faces (not a fillet)".to_string(),
            };
            json!({"face": t.name(f), "index": f, "surface": surface_kind(s), "reason": reason})
        })
        .collect()
}

#[cfg(test)]
#[path = "inspect_features_tests.rs"]
mod tests;

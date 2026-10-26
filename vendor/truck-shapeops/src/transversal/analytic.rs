//! SolidCraft: face pairs whose intersection has a closed form (#125), intersected exactly rather
//! than by colliding their meshes and projecting the collision's points back onto both surfaces.
//!
//! Two planar faces bounded by straight edges: their intersection is the planes' line,
//! inside both faces: each face's boundary crosses the other plane at points along the line, and
//! between alternate crossings the line runs inside the face (a vertex on the plane decides its
//! side once, so a corner the plane only touches is crossed twice at one point or not at all).
//! The pieces inside both faces are the segments the mesh collision finds (a planar face's mesh
//! is the face itself), exactly.
//!
//! A planar face bounded by straight edges and a cylindrical face square to it (a hole's tool
//! through a plate): their intersection is the circle where the plane cuts the cylinder, taken
//! here where all of it lies inside the planar face (a circle crossing its edges is left to the
//! mesh collision). The planar face's edges cross the circle, and the cylindrical face's edges cross
//! the plane (only straight ones, its seams; a curved edge must stay clear of the plane), at
//! angles round the circle; between them the circle runs inside a face or outside it all along,
//! which its middle tells (inside the planar face's polygon, and on the cylindrical face's mesh).
//!
//! A pair this cannot decide (a face of another surface or with another edge, parallel planes
//! that coincide, an edge lying in the other plane or touching it, a plane oblique to the
//! cylinder) is `None`, and the mesh collision takes it as before.

use std::any::Any;
use truck_base::{cgmath64::*, tolerance::*};
use truck_geotrait::BoundedCurve;
use truck_meshalgo::prelude::{PolygonMesh, PolylineCurve};
use truck_topology::Face;

#[cfg(any(test, feature = "test-hooks"))]
thread_local! {
    /// Set by the test hooks: every pair goes to the mesh collision, as before.
    pub(super) static MESH_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// How many pairs were intersected exactly (for the test hooks to see that they were).
    pub(super) static EXACT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// How many of them were a plane and a cylinder.
    pub(super) static CIRCLES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Whether exact intersections are taken (always, but in the test hooks' comparisons).
fn enabled() -> bool {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        !MESH_ONLY.with(|c| c.get())
    }
    #[cfg(not(any(test, feature = "test-hooks")))]
    {
        true
    }
}

/// The plane of a planar surface: a point on it and its unit normal.
fn plane_of<S: 'static>(s: &S) -> Option<(Point3, Vector3)> {
    let any: &dyn Any = s;
    match any.downcast_ref::<truck_modeling::Surface>()? {
        truck_modeling::Surface::Plane(p) => {
            let n = p.normal();
            (n.magnitude2().is_finite() && n.magnitude2() > 0.5).then(|| (p.origin(), n.normalize()))
        }
        _ => None,
    }
}

/// Whether an edge curve is a straight segment between its ends: a line, or a B-spline whose
/// control points all lie on one line (its points lie in their hull).
fn straight<C: 'static>(c: &C) -> bool {
    let any: &dyn Any = c;
    match any.downcast_ref::<truck_modeling::Curve>() {
        Some(truck_modeling::Curve::Line(_)) => true,
        Some(truck_modeling::Curve::BSplineCurve(b)) => {
            let pts = b.control_points();
            match (pts.first(), pts.last()) {
                (Some(a), Some(z)) if !a.near(z) => {
                    let d = (z - a).normalize();
                    pts.iter().all(|p| (p - a).cross(d).magnitude() <= TOLERANCE)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Where the line `o + t d` runs inside `face` (on a plane through the line), as sorted parameter
/// intervals: from the face's straight edges' crossings of the plane `(p, n)` the face's own
/// plane meets it in. `None` when an edge is not straight, lies in that plane, or the crossings
/// don't pair up.
fn inside_intervals<C: Clone + 'static, S>(face: &Face<Point3, C, S>, (p, n): (Point3, Vector3), o: Point3, d: Vector3) -> Option<Vec<(f64, f64)>> {
    let mut ts = Vec::new();
    for wire in face.boundaries() {
        for e in wire.edge_iter() {
            if !straight(&e.curve()) {
                return None;
            }
            let (a, b) = (e.front().point(), e.back().point());
            let (ga, gb) = ((a - p).dot(n), (b - p).dot(n));
            if !ga.is_finite() || !gb.is_finite() || (ga.abs() <= TOLERANCE && gb.abs() <= TOLERANCE) {
                return None;
            }
            if (ga >= 0.0) != (gb >= 0.0) {
                let q = a + (b - a) * (ga / (ga - gb)).clamp(0.0, 1.0);
                ts.push((q - o).dot(d));
            }
        }
    }
    if ts.len() % 2 != 0 {
        return None;
    }
    ts.sort_by(f64::total_cmp);
    Some(ts.chunks(2).filter_map(|c| match c {
        [a, b] => Some((*a, *b)),
        _ => None,
    }).collect())
}

/// Below this |n0 × n1| (the sine of the planes' angle) a plane pair is left to the mesh collision.
const NEAR_PARALLEL: f64 = 1e-6;

/// The exact intersection of two planar faces as the polylines the mesh collision would chain,
/// or `None` for a pair it leaves to the rest (neither needs the faces' meshes).
pub(super) fn plane_pair<C, S>(face0: &Face<Point3, C, S>, face1: &Face<Point3, C, S>) -> Option<Vec<PolylineCurve<Point3>>>
where
    C: Clone + 'static,
    S: Clone + 'static,
{
    if !enabled() {
        return None;
    }
    let found = plane_plane(face0, plane_of(&face0.surface())?, face1, plane_of(&face1.surface())?);
    #[cfg(any(test, feature = "test-hooks"))]
    if found.is_some() {
        EXACT.with(|c| c.set(c.get() + 1));
    }
    found
}

/// The exact intersection of a planar face and a cylindrical one (in either order) as the
/// polylines the mesh collision would chain, or `None` for a pair it leaves to the mesh
/// collision. `mesh0` and `mesh1` are the faces' meshes.
pub(super) fn plane_cylinder_pair<C, S>(face0: &Face<Point3, C, S>, mesh0: &PolygonMesh, face1: &Face<Point3, C, S>, mesh1: &PolygonMesh, tol: f64) -> Option<Vec<PolylineCurve<Point3>>>
where
    C: Clone + BoundedCurve<Point = Point3> + 'static,
    S: Clone + 'static,
{
    if !enabled() {
        return None;
    }
    let found = match (plane_of(&face0.surface()), plane_of(&face1.surface())) {
        (Some(p0), None) => plane_cylinder(face0, p0, face1, mesh1, tol),
        (None, Some(p1)) => plane_cylinder(face1, p1, face0, mesh0, tol),
        _ => None,
    };
    #[cfg(any(test, feature = "test-hooks"))]
    if found.is_some() {
        EXACT.with(|c| c.set(c.get() + 1));
        CIRCLES.with(|c| c.set(c.get() + 1));
    }
    found
}

/// Two planar faces: their planes' line inside both.
fn plane_plane<C: Clone + 'static, S>(face0: &Face<Point3, C, S>, (p0, n0): (Point3, Vector3), face1: &Face<Point3, C, S>, (p1, n1): (Point3, Vector3)) -> Option<Vec<PolylineCurve<Point3>>> {
    let d = n0.cross(n1);
    // Parallel or nearly so (their line far off or ill-conditioned): the mesh collision's, which
    // finds what meets within the faces either way.
    if d.magnitude() < NEAR_PARALLEL {
        return None;
    }
    let d = d.normalize();
    // A point of the line, solved relative to p0 so faces far from the origin keep their digits:
    // o = p0 + a n0 + b n1 with n0·(o - p0) = 0 and n1·(o - p1) = 0.
    let c1 = n1.dot(p1 - p0);
    let n01 = n0.dot(n1);
    let det = 1.0 - n01 * n01;
    let (a, b) = ((-c1 * n01) / det, c1 / det);
    let o = p0 + n0 * a + n1 * b;
    if !o.to_vec().magnitude2().is_finite() {
        return None;
    }
    let in0 = inside_intervals(face0, (p1, n1), o, d)?;
    let in1 = inside_intervals(face1, (p0, n0), o, d)?;
    // Where both run inside, segments touching end to start taken as one (the collision chains them).
    let mut segments: Vec<(f64, f64)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while let (Some(&(a0, a1)), Some(&(b0, b1))) = (in0.get(i), in1.get(j)) {
        let (lo, hi) = (a0.max(b0), a1.min(b1));
        if hi - lo > TOLERANCE {
            match segments.last_mut() {
                Some(last) if (lo - last.1).abs() <= TOLERANCE => last.1 = hi,
                _ => segments.push((lo, hi)),
            }
        }
        if a1 < b1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(segments.into_iter().map(|(lo, hi)| PolylineCurve(vec![o + d * lo, o + d * hi])).collect())
}

/// A cylinder: a point on its axis, the unit axis and the radius.
#[derive(Clone, Copy, Debug)]
struct Cylinder {
    o: Point3,
    a: Vector3,
    r: f64,
}

/// The cylinder a face's mesh lies on, from its vertices and their (surface) normals: the
/// normals all square to one axis and their lines all through it, every vertex at one distance.
/// `None` for any other surface.
fn cylinder_of(mesh: &PolygonMesh) -> Option<Cylinder> {
    let (pos, nor) = (mesh.positions(), mesh.normals());
    let samples: Vec<(Point3, Vector3)> = mesh
        .faces()
        .triangle_iter()
        .flat_map(|t| t.into_iter())
        .filter_map(|v| {
            let n = *nor.get(v.nor?)?;
            (n.magnitude2() > 0.5).then(|| (*pos.get(v.pos).unwrap_or(&Point3::origin()), n.normalize()))
        })
        .collect();
    let &(p0, n0) = samples.first()?;
    let &(p1, n1) = samples.iter().max_by(|x, y| n0.cross(x.1).magnitude2().total_cmp(&n0.cross(y.1).magnitude2()))?;
    let a = n0.cross(n1);
    if a.magnitude() < 1e-3 {
        return None;
    }
    let a = a.normalize();
    // Where the normal lines through p0 and p1 meet (seen along a).
    let (w, c) = (p1 - p0, n0.dot(n1));
    let den = 1.0 - c * c;
    let t0 = (w.dot(n0) - c * w.dot(n1)) / den;
    let o = p0 + n0 * t0;
    let r = t0.abs();
    let cyl = Cylinder { o, a, r };
    let eps = 1e-9 * r.max(1.0);
    let fits = r.is_finite()
        && r > TOLERANCE
        && samples.iter().all(|&(p, n)| {
            let radial = cyl.radial(p);
            n.dot(a).abs() <= 1e-9 && (radial.magnitude() - r).abs() <= eps && n.cross(radial).magnitude() <= 1e-9 * radial.magnitude()
        });
    fits.then_some(cyl)
}

impl Cylinder {
    /// The part of `p - o` square to the axis.
    fn radial(&self, p: Point3) -> Vector3 {
        let w = p - self.o;
        w - self.a * w.dot(self.a)
    }
}

/// The edges of a face, each as its curve's ends and whether it is straight.
fn edges_of<C: Clone + 'static, S>(face: &Face<Point3, C, S>) -> Vec<(Point3, Point3, C, bool)> {
    face.boundaries()
        .iter()
        .flat_map(|w| w.edge_iter().map(|e| (e.front().point(), e.back().point(), e.curve(), straight(&e.curve()))).collect::<Vec<_>>())
        .collect()
}

/// A planar face (with straight edges) and a cylindrical face square to it: the circle the plane
/// cuts from the cylinder, inside both faces, as polylines within `tol` of it (closed where all of
/// it is inside both).
fn plane_cylinder<C, S>(plane_face: &Face<Point3, C, S>, (p, n): (Point3, Vector3), cyl_face: &Face<Point3, C, S>, mesh: &PolygonMesh, tol: f64) -> Option<Vec<PolylineCurve<Point3>>>
where
    C: Clone + BoundedCurve<Point = Point3> + 'static,
    S: Clone,
{
    let cyl = cylinder_of(mesh)?;
    // Square to the axis only (an oblique plane cuts an ellipse, the mesh collision's for now).
    if n.cross(cyl.a).magnitude() > 1e-12 {
        return None;
    }
    let c = cyl.o + cyl.a * ((p - cyl.o).dot(n) / cyl.a.dot(n));
    let e1 = {
        let x = if n.x.abs() < 0.9 { Vector3::unit_x() } else { Vector3::unit_y() };
        (x - n * x.dot(n)).normalize()
    };
    let e2 = n.cross(e1);
    let at = |t: f64| c + (e1 * t.cos() + e2 * t.sin()) * cyl.r;
    let angle = |q: Point3| (q - c).dot(e2).atan2((q - c).dot(e1));
    let mut cuts: Vec<f64> = Vec::new();
    // The planar face's edges across the circle (each from its start, so a corner on the circle
    // counts once).
    let plane_edges = edges_of(plane_face);
    for (a, b, _, line) in &plane_edges {
        if !line {
            return None;
        }
        let (d, f) = (b - a, a - c);
        let (qa, qb, qc) = (d.dot(d), 2.0 * f.dot(d), f.dot(f) - cyl.r * cyl.r);
        let disc = qb * qb - 4.0 * qa * qc;
        if qa <= 0.0 || !disc.is_finite() {
            return None;
        }
        // Grazing the circle: the mesh collision's.
        if disc.abs() <= 1e-9 * qb * qb + TOLERANCE * TOLERANCE * qa {
            return None;
        }
        if disc < 0.0 {
            continue;
        }
        for s in [(-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa)] {
            if (0.0..1.0).contains(&s) {
                cuts.push(angle(a + d * s));
            }
        }
    }
    // Only a circle inside the planar face all round (a hole's tool through a plate): an arc that
    // ended on its edges would end at a corner where the mesh collision's curves of the
    // neighbouring faces end too, and the loops stores merge those ends by the collision's
    // points, so the corner came out up to the mesh's error off the edges' true meeting point
    // (#125, kernel case 1647). Arcs between the cylinder's seams meet only arcs of this circle
    // (the cylinder's other faces in this plane, intersected here alike).
    if !cuts.is_empty() {
        return None;
    }
    // The cylindrical face's edges across the plane: straight ones (seams) cross it once; a curved
    // one must stay clear of it by more than the mesh's sag (or the mesh test below could not tell).
    for (a, b, curve, line) in edges_of(cyl_face) {
        let (ga, gb) = ((a - p).dot(n), (b - p).dot(n));
        if line {
            if ga.abs() <= TOLERANCE || gb.abs() <= TOLERANCE {
                return None;
            }
            if (ga > 0.0) != (gb > 0.0) {
                cuts.push(angle(a + (b - a) * (ga / (ga - gb))));
            }
            continue;
        }
        let (t0, t1) = curve.try_range_tuple()?;
        let side = |k: usize| (curve.subs(t0 + (t1 - t0) * k as f64 / 32.0) - p).dot(n);
        let first = side(0);
        if !(0..=32).all(|k| {
            let g = side(k);
            g.is_finite() && g.abs() > 2.0 * tol && (g > 0.0) == (first > 0.0)
        }) {
            return None;
        }
    }
    cuts.sort_by(f64::total_cmp);
    let tau = std::f64::consts::TAU;
    // The arcs between consecutive cuts (all the circle when there are none), each kept when its
    // middle is inside both faces; kept arcs that meet are one.
    let arcs: Vec<(f64, f64)> = match cuts.first() {
        None => vec![(0.0, tau)],
        Some(&first) => cuts.iter().zip(cuts.iter().skip(1).chain(std::iter::once(&(first + tau)))).map(|(&s, &e)| (s, e)).collect(),
    };
    let mut kept: Vec<(f64, f64)> = Vec::new();
    for (s, e) in arcs {
        if e - s <= TOLERANCE / cyl.r {
            continue;
        }
        // An arc so short its middle lies within the mesh test's reach of a cut: the mesh
        // collision's.
        if (e - s) * cyl.r <= 8.0 * tol {
            return None;
        }
        let m = at(0.5 * (s + e));
        if !inside_polygon(&plane_edges, m, n) || !on_mesh(mesh, m, 2.0 * tol) {
            continue;
        }
        match kept.last_mut() {
            Some(last) if (s - last.1).abs() <= TOLERANCE / cyl.r => last.1 = e,
            _ => kept.push((s, e)),
        }
    }
    // The last arc running on into the first, round the start.
    if kept.len() > 1 {
        if let (Some(&(fs, _)), Some(&(_, le))) = (kept.first(), kept.last()) {
            if (le - (fs + tau)).abs() <= TOLERANCE / cyl.r {
                let (_, fe) = kept.remove(0);
                if let Some(last) = kept.last_mut() {
                    last.1 = fe + tau;
                }
            }
        }
    }
    // Points no more than `tol` off the circle between them.
    let step = 2.0 * (1.0 - (tol / cyl.r).min(1.0)).acos().max(1e-3);
    Some(
        kept.into_iter()
            .map(|(s, e)| {
                let k = (((e - s) / step).ceil() as usize).clamp(4, 4096);
                let mut pts: Vec<Point3> = (0..=k).map(|i| at(s + (e - s) * i as f64 / k as f64)).collect();
                // All the circle: a closed loop, its last point its first.
                if (e - s - tau).abs() <= TOLERANCE / cyl.r {
                    if let Some(first) = pts.first().copied() {
                        if let Some(last) = pts.last_mut() {
                            *last = first;
                        }
                    }
                }
                PolylineCurve(pts)
            })
            .collect(),
    )
}

/// Whether `m` (in the planar face's plane, normal `n`) is inside the polygon of its straight
/// edges: crossings of a ray from it, along a direction in the plane.
fn inside_polygon<C>(edges: &[(Point3, Point3, C, bool)], m: Point3, n: Vector3) -> bool {
    let x = if n.x.abs() < 0.9 { Vector3::unit_x() } else { Vector3::unit_y() };
    let u = (x - n * x.dot(n)).normalize();
    let v = n.cross(u);
    let to2 = |q: Point3| ((q - m).dot(u), (q - m).dot(v));
    let mut inside = false;
    for (a, b, _, _) in edges {
        let ((ax, ay), (bx, by)) = (to2(*a), to2(*b));
        if (ay > 0.0) != (by > 0.0) {
            let xc = ax + (bx - ax) * (0.0 - ay) / (by - ay);
            if xc > 0.0 {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether `m` lies on the mesh: within `dist` of one of its triangles.
fn on_mesh(mesh: &PolygonMesh, m: Point3, dist: f64) -> bool {
    let pos = mesh.positions();
    mesh.faces().triangle_iter().any(|t| {
        let (Some(a), Some(b), Some(c)) = (pos.get(t[0].pos), pos.get(t[1].pos), pos.get(t[2].pos)) else { return false };
        nearest_on_triangle(m, *a, *b, *c).distance(m) <= dist
    })
}

/// The point of the triangle `abc` nearest `p` (Ericson, "Real-Time Collision Detection", 5.1.5).
fn nearest_on_triangle(p: Point3, a: Point3, b: Point3, c: Point3) -> Point3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let den = va + vb + vc;
    if !den.is_finite() || den.abs() <= f64::MIN_POSITIVE {
        return a;
    }
    a + ab * (vb / den) + ac * (vc / den)
}

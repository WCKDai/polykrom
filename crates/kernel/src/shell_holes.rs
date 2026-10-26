//! Shells of bodies whose holes come closer to another wall than the shell is thick (a hole
//! drilled next to a chamfer): moving every face in by the wall keeps the topology only where
//! each face keeps some of its area, and there the ring of a face round the hole vanishes.
//!
//! A body with holes is the body with the holes filled less the holes, and its cavity is the
//! filled body's cavity less the holes grown by the wall (a point is a wall away from the body
//! exactly when it is that far inside the filled body and that far from every hole). So the
//! holes are filled (Delete Face heals them), the filled body's cavity is made by the
//! topology-preserving offset (`offset_free`), each hole grown by the wall is cut from it, and
//! the shell is the body less that cavity. A hole is grown with sharp edges, so the wall round
//! the edge where a hole meets its face is a little thicker than asked, never thinner.

use std::collections::{HashMap, HashSet};

use mt::{BoundedCurve, ParametricCurve};
use solidcraft_geom::Vec3;
use truck_modeling as mt;

use crate::body::{Body, Solid, from_p3};
use crate::offset::{Surf, surf_of};
use crate::{KernelError, Result, guard};

/// Most holes a shell fills and grows.
const MAX_HOLES: usize = 64;

/// A hole: the faces that line it (indices in the body's face order) and the inner loops of
/// planar faces where it opens (each with the plane's face index).
struct Hole {
    faces: Vec<usize>,
    rims: Vec<(usize, mt::Wire)>,
}

/// Points sampled along a wire, as it runs.
fn wire_points(w: &mt::Wire) -> Vec<Vec3> {
    let mut pts = Vec::new();
    for e in w.edge_iter() {
        let c = e.oriented_curve();
        let (t0, t1) = c.range_tuple();
        pts.extend((0..8).map(|k| from_p3(c.subs(t0 + (t1 - t0) * k as f64 / 8.0))));
    }
    pts
}

/// The signed area a wire encloses, seen along `n`.
fn wire_area(w: &mt::Wire, n: Vec3) -> f64 {
    let pts = wire_points(w);
    let Some(&first) = pts.first() else { return 0.0 };
    let mut sum = Vec3::ZERO;
    for (a, b) in pts.iter().zip(pts.iter().skip(1).chain(std::iter::once(&first))) {
        sum += (*a - first).cross(*b - first);
    }
    sum.dot(n) * 0.5
}

/// The holes of `b`: from each inner loop of a planar face, the faces reached across it without
/// crossing another inner loop of a planar face, where they lie behind that face (inside the
/// body: a hole, not a boss standing on the face).
fn holes(b: &Body) -> Vec<Hole> {
    let faces: Vec<mt::Face> = b.solid.face_iter().cloned().collect();
    let tol = (b.size() * 1e-7).max(1e-9) * 100.0;
    let planes: Vec<Option<(Vec3, f64)>> =
        faces.iter().map(|f| if let Some(Surf::Plane { n, d }) = surf_of(f, tol) { Some((n, d)) } else { None }).collect();
    let mut efaces: HashMap<mt::EdgeID, Vec<usize>> = HashMap::new();
    for (i, f) in faces.iter().enumerate() {
        for e in f.edge_iter() {
            let list = efaces.entry(e.id()).or_default();
            if !list.contains(&i) {
                list.push(i);
            }
        }
    }
    // Every inner loop of a planar face (all but the loop enclosing the most area).
    let mut inner: Vec<(usize, mt::Wire)> = Vec::new();
    for (i, (f, plane)) in faces.iter().zip(&planes).enumerate() {
        let Some((n, _)) = *plane else { continue };
        let wires = f.absolute_boundaries().clone();
        let outer = wires.iter().enumerate().max_by(|a, c| wire_area(a.1, n).abs().total_cmp(&wire_area(c.1, n).abs())).map(|(k, _)| k);
        inner.extend(wires.into_iter().enumerate().filter(|(k, _)| Some(*k) != outer).map(|(_, w)| (i, w)));
    }
    let stops: HashSet<mt::EdgeID> = inner.iter().flat_map(|(_, w)| w.edge_iter().map(|e| e.id())).collect();
    let mut found: Vec<Hole> = Vec::new();
    for (p, w) in &inner {
        let Some((n, d)) = planes.get(*p).copied().flatten() else { continue };
        if found.iter().any(|h| h.rims.iter().any(|(_, r)| r.edge_iter().zip(w.edge_iter()).all(|(x, y)| x.id() == y.id()))) {
            continue;
        }
        // Flood from the faces across the loop.
        let mut part: Vec<usize> = Vec::new();
        let mut todo: Vec<usize> = w.edge_iter().flat_map(|e| efaces.get(&e.id()).into_iter().flatten().copied()).filter(|f| f != p).collect();
        while let Some(f) = todo.pop() {
            if part.contains(&f) || f == *p {
                continue;
            }
            part.push(f);
            if part.len() * 2 > faces.len() {
                break;
            }
            let Some(face) = faces.get(f) else { continue };
            for e in face.edge_iter() {
                if stops.contains(&e.id()) {
                    continue;
                }
                todo.extend(efaces.get(&e.id()).into_iter().flatten().copied());
            }
        }
        if part.is_empty() || part.len() * 2 > faces.len() {
            continue;
        }
        // Behind the face: every vertex of the hole not on the face's plane lies inside it.
        let size = b.size();
        let behind =
            part.iter().filter_map(|f| faces.get(*f)).flat_map(|f| f.vertex_iter().map(|v| n.dot(from_p3(v.point())) - d)).all(|h| h < size * 1e-6);
        let deep =
            part.iter().filter_map(|f| faces.get(*f)).flat_map(|f| f.vertex_iter().map(|v| n.dot(from_p3(v.point())) - d)).any(|h| h < -size * 1e-6);
        if !(behind && deep) {
            continue;
        }
        // Every loop where the hole opens: the inner loops whose edges border it.
        let rims: Vec<(usize, mt::Wire)> = inner
            .iter()
            .filter(|(_, r)| r.edge_iter().all(|e| efaces.get(&e.id()).is_some_and(|fs| fs.iter().any(|f| part.contains(f)))))
            .cloned()
            .collect();
        found.push(Hole { faces: part, rims });
    }
    found
}

/// The solid a hole removed: its faces turned inside out, capped by its rims on their planes.
fn hole_solid(b: &Body, hole: &Hole) -> Result<Body> {
    let faces: Vec<mt::Face> = b.solid.face_iter().cloned().collect();
    guard("shell", || {
        let mut out: Vec<mt::Face> = hole.faces.iter().filter_map(|i| faces.get(*i)).map(|f| f.inverse()).collect();
        for (p, w) in &hole.rims {
            let plane = faces.get(*p).ok_or_else(|| KernelError::Failed("shell: a hole's face".into()))?;
            let mut cap =
                mt::Face::try_new(vec![w.inverse()], plane.surface()).map_err(|e| KernelError::Failed(format!("shell: a hole's cap: {e}")))?;
            if !plane.orientation() {
                cap.invert();
            }
            out.push(cap);
        }
        let solid = Solid::try_new(vec![out.into()]).map_err(|e| KernelError::Failed(format!("shell: a hole's solid: {e}")))?;
        Body::new(solid)
    })
}

/// Shell of `b` (`opened`: its open faces) through its holes filled (see the module comment);
/// `None` where it has no hole. Open faces move out past the body (by `margin`), so the cavity
/// passes through them and the body less the cavity is open there.
pub(crate) fn shell_round_holes(b: &Body, opened: &[usize], thickness: f64, tolerance: f64) -> Result<Option<Body>> {
    let found = holes(b);
    if found.is_empty() || found.len() > MAX_HOLES {
        return Ok(None);
    }
    let lining: Vec<usize> = found.iter().flat_map(|h| h.faces.iter().copied()).collect();
    if opened.iter().any(|i| lining.contains(i)) {
        return Ok(None);
    }
    // A point inside each open face, found again on the filled body.
    let mesh = b.tessellate((b.size() * 2e-3).max(1e-3))?;
    let mut at: Vec<Vec3> = Vec::new();
    for o in opened {
        let best = mesh
            .triangles
            .iter()
            .zip(&mesh.tri_face)
            .filter(|(_, f)| **f as usize == *o)
            .filter_map(|(t, _)| mesh.tri(t))
            .max_by(|x, y| tri_area(x).total_cmp(&tri_area(y)));
        let [x, y, z] = best.ok_or_else(|| KernelError::Failed("shell: an open face with no mesh".into()))?;
        at.push((x + y + z) / 3.0);
    }
    let filled = crate::delete_face::delete_faces(b, &lining)?;
    let mut open_filled: Vec<usize> = Vec::new();
    for p in &at {
        let f = crate::surfaces::nearest_face(&filled, *p)?;
        if !open_filled.contains(&f) {
            open_filled.push(f);
        }
    }
    let margin = thickness + b.size() * 0.05;
    let mut cavity = crate::offset_free::offset_free(&filled, |i| if open_filled.contains(&i) { margin } else { -thickness }, tolerance)?;
    if let Some(p) = crate::offset_free::too_thin(&filled, &cavity, &open_filled, thickness, tolerance)? {
        return Err(KernelError::Failed(format!(
            "not supported yet: the shell's walls would meet near [{:.2}, {:.2}, {:.2}] (the body is thinner there than twice the wall)",
            p.x, p.y, p.z
        )));
    }
    for hole in &found {
        let solid = hole_solid(b, hole)?;
        let grown = crate::offset_free::offset_free(&solid, |_| thickness, tolerance)?;
        cavity = crate::ops::boolean(&cavity, &grown, crate::BoolOp::Cut)?
            .ok_or_else(|| KernelError::Failed("shell: the holes fill the cavity".into()))?;
    }
    crate::ops::boolean(b, &cavity, crate::BoolOp::Cut)?.ok_or_else(|| KernelError::Failed("the shell removed everything".into())).map(Some)
}

fn tri_area(t: &[Vec3; 3]) -> f64 {
    let [a, b, c] = *t;
    (b - a).cross(c - a).len()
}

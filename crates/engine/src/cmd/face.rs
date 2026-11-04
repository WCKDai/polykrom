//! Sketches made from a planar body face, so a face can be extruded like a profile (Extrude
//! with `face`, Press Pull). The face boundary is copied into a new sketch on the face: straight
//! edges become lines, circular edges circles or arcs, anything else a chain of lines.

use polycore_geom::{Mesh, Vec2, Vec3};
use polykrom_doc::{EmbossFace, apply_point, mat_inverse, mat_mul};
use serde_json::{Value, json};

use crate::params::{bad, vec3};
use crate::{Result, Session};

fn point_triangle_dist(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let n = (b - a).cross(c - a);
    let Some(nn) = n.normalized() else { return f64::INFINITY };
    let h = (p - a).dot(nn);
    let q = p - nn * h;
    let inside = [(a, b), (b, c), (c, a)].iter().all(|(u, v)| (*v - *u).cross(q - *u).dot(n) >= -1e-12);
    if inside { h.abs() } else { [(a, b), (b, c), (c, a)].iter().map(|(u, v)| p.dist_to_segment(*u, *v)).fold(f64::INFINITY, f64::min) }
}

/// The outward normal of the planar body face at `p`.
pub(super) fn face_normal(s: &Session, p: Vec3) -> Option<Vec3> {
    face_normal_in(s, p, None)
}

/// The outward normal of the planar face at `p` of `body` (any body when `None`).
pub(super) fn face_normal_in(s: &Session, p: Vec3, body: Option<&str>) -> Option<Vec3> {
    let (m, f, d, _) = face_at_in(s, p, body).ok().flatten()?;
    let tol = (m.bounds().diagonal() * 1e-3).max(1e-3);
    if d > tol * 10.0 {
        return None;
    }
    m.triangles
        .iter()
        .zip(&m.tri_face)
        .filter(|(_, tf)| **tf == f)
        .filter_map(|(t, _)| m.tri(t))
        .find_map(|[a, b, c]| (b - a).cross(c - a).normalized())
}

/// Is the body face at `p` planar (`None`: no face there)?
pub(super) fn face_is_planar(s: &Session, p: Vec3) -> Option<bool> {
    let (m, f, d, _) = face_at(s, p)?;
    let tol = (m.bounds().diagonal() * 1e-3).max(1e-3);
    if d > tol * 10.0 {
        return None;
    }
    let ns: Vec<Vec3> = m
        .triangles
        .iter()
        .zip(&m.tri_face)
        .filter(|(_, tf)| **tf == f)
        .filter_map(|(t, _)| m.tri(t))
        .filter_map(|[a, b, c]| (b - a).cross(c - a).normalized())
        .collect();
    let n0 = *ns.first()?;
    Some(ns.iter().all(|n| n.dot(n0) > 1.0 - 1e-6))
}

/// The faces a command's `faces` list picked, resolved before the kernel sees them (#297): the
/// kernel takes the face nearest each point, so a point on an edge used to open whichever of
/// the two faces came first, in silence.
///
/// A face given to a command is a point `[x, y, z]` on it (or `{point, normal?, body?}`), or
/// its name as `model.faces` lists it, looked up on `body` (any body without one). A name
/// becomes a point on that face, which the feature keeps like a picked one, so it also keeps the
/// face's name. The point is in the frame of the command's other picks: the component's own when
/// they were mapped into it (`crate::frames` then names the body under each point and keeps the
/// point as given as `world`), else the world's.
pub(super) struct PickedFaces {
    /// The point the feature stores for each entry, in the frame of the command's picks: the
    /// point as given where it lies on the face it resolved to, else a point inside that face
    /// (a name, a `normal`-resolved pick, a pick within reach of the face but off it; see
    /// [`settled_point`]).
    pub points: Vec<Vec3>,
    /// The persistent name (`model.faces`) of each entry's face, for the reply's `faces_used`;
    /// empty where no face is within the pick tolerance (the kernel reports that itself).
    pub names: Vec<String>,
    /// Entries that resolved to one face, and bodies too big for the tie check, for the reply's
    /// `warning`.
    pub warning: Option<String>,
    /// The body the picks settled on where an entry lay on faces of several bodies (bodies
    /// touching flush) or named its body, and the command named none: the feature keeps it, so
    /// it acts on that body and not on whichever body the kernel would take by its bounds.
    /// `None` otherwise: the feature then finds the body as before.
    pub body: Option<String>,
}

impl PickedFaces {
    /// The feature's `body`: the command's `given` one, else [`PickedFaces::body`].
    pub fn feature_body(&self, given: Option<&str>) -> Option<polykrom_doc::BodyName> {
        given.map(polykrom_doc::BodyName::from).or_else(|| self.body.clone().map(polykrom_doc::BodyName))
    }

    /// `reply`, a feature command's, with the `faces_used` names and the picks' warning joined
    /// to the evaluation's.
    pub fn into_reply(self, reply: Result<Value>) -> Result<Value> {
        let mut reply = reply?;
        if let Some(o) = reply.as_object_mut() {
            o.insert("faces_used".into(), json!(self.names));
            if let Some(w) = self.warning {
                let joined = match o.get("warning").and_then(Value::as_str) {
                    Some(old) if !old.is_empty() => format!("{old}; {w}"),
                    _ => w,
                };
                o.insert("warning".into(), json!(joined));
            }
        }
        Ok(reply)
    }
}

/// The biggest body the tie check takes on: (display triangles, faces). A bigger one is not
/// checked (see [`FaceTriangles::new`]).
const PICK_CHECK_LIMIT: (usize, usize) = (2_000_000, 50_000);

/// A face's triangles with their bounds, for the distance from a point to each face of a body.
struct FaceTriangles {
    faces: Vec<(polycore_geom::Aabb3, Vec<[Vec3; 3]>)>,
}

impl FaceTriangles {
    /// The faces of `b`, or `None` for a mesh body or one bigger than `limit` (its triangles
    /// are not copied; the pick takes its nearest face unchecked, and the reply says so).
    fn new(b: &polykrom_doc::ModelBody, limit: (usize, usize)) -> Option<Self> {
        if b.body.is_mesh() {
            return None;
        }
        let m = b.mesh();
        let count = m.tri_face.iter().map(|f| *f as usize + 1).max().unwrap_or(0);
        if m.triangles.len() > limit.0 || count > limit.1 {
            return None;
        }
        let mut faces = vec![(polycore_geom::Aabb3::EMPTY, Vec::new()); count];
        for (t, f) in m.triangles.iter().zip(&m.tri_face) {
            if let (Some(slot), Some(tri)) = (faces.get_mut(*f as usize), m.tri(t)) {
                slot.0.extend(tri);
                slot.1.push(tri);
            }
        }
        Some(Self { faces })
    }

    /// Each face within `reach` of `p`: (distance, face index, the outward normal of its nearest triangle).
    fn near(&self, p: Vec3, reach: f64) -> Vec<(f64, usize, Vec3)> {
        let mut out = Vec::new();
        for (i, (bounds, tris)) in self.faces.iter().enumerate() {
            if tris.is_empty() || !bounds.inflated(reach).contains(p) {
                continue;
            }
            let mut best: Option<(f64, Vec3)> = None;
            for [a, b, c] in tris {
                let d = point_triangle_dist(p, *a, *b, *c);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, (*b - *a).cross(*c - *a).normalized().unwrap_or(Vec3::ZERO)));
                }
            }
            if let Some((d, n)) = best
                && d <= reach
            {
                out.push((d, i, n));
            }
        }
        out
    }
}

/// One face a pick may mean: how far, which body and face, the face's outward normal there,
/// and whether the body was checked for other faces as near (a body too big for the check is
/// not: its nearest face is taken).
struct Candidate {
    distance: f64,
    body: usize,
    face: usize,
    normal: Vec3,
    checked: bool,
}

/// The face of `b` (body `body`) nearest `p`, within `reach`, without the check for other faces
/// as near: for a body too big for [`FaceTriangles`].
fn nearest_face_unchecked(b: &polykrom_doc::ModelBody, body: usize, p: Vec3, reach: f64) -> Option<Candidate> {
    let (distance, face) = polykrom_doc::appearance::nearest_face(b, p)?;
    if distance > reach {
        return None;
    }
    let m = b.mesh();
    let normal = m
        .triangles
        .iter()
        .zip(&m.tri_face)
        .filter(|(_, f)| **f as usize == face)
        .filter_map(|(t, _)| m.tri(t))
        .find_map(|[a, b, c]| (b - a).cross(c - a).normalized())
        .unwrap_or(Vec3::ZERO);
    Some(Candidate { distance, body, face, normal, checked: false })
}

/// An entry of a command's face list, read: what the feature stores, what the caller gave, and
/// the faces it may mean.
struct Entry {
    /// The point the feature stores: the point as given, or a point inside the face for a name.
    point: Vec3,
    /// The point as the caller gave it (in the world, where the command's picks were mapped into
    /// a component's frame), for the error.
    given: Vec3,
    normal: Option<Vec3>,
    /// The faces the entry may mean, nearest first: one for a name, none where no face is within
    /// reach of the point.
    near: Vec<Candidate>,
}

/// `[x, y, z]` as the error names a pick, without needless decimals.
fn point_text(p: Vec3) -> String {
    format!("[{}, {}, {}]", p.x, p.y, p.z)
}

/// The command's `faces` entries resolved: each a point `[x, y, z]`, `{point, normal?, body?}`
/// or a face name. A point that two or more faces are equally near to, within the pick tolerance
/// (a millionth of the body's size, at least 1 nm: a point on an edge, a seam or a corner, where
/// the kernel would take whichever face comes first), is refused as ambiguous, with the faces
/// named, unless `normal` picks the one facing that way (within 45°). The tolerance is a tie
/// tolerance, not a reach: the top of a plate a thousand times longer than it is thick is not
/// confused with its bottom. Points of unambiguous picks that lie on their face are kept as
/// given, so their features build exactly as before; a pick within reach of a face but off it
/// becomes a point inside the face, so the kernel acts on the face `faces_used` names (see
/// [`settled_point`]).
///
/// A pick is only ambiguous between faces the operation can act on. The command's `body` and an
/// entry's own `body` keep the pick to that body, so a face flush with another body's face does
/// not compete with it. Without one, the faces of a command all belong to one body: an entry on
/// faces of several bodies is settled by the body the other entries lie on, and only when every
/// entry lies on faces of the same several bodies are those faces listed as ambiguous.
pub(super) fn picked_faces(s: &Session, list: &[Value], body: Option<&str>, cmd: &str) -> Result<PickedFaces> {
    picked_faces_within(s, list, body, cmd, PICK_CHECK_LIMIT)
}

/// [`picked_faces`] with the tie check's limit on a body's size given (tests lower it).
fn picked_faces_within(s: &Session, list: &[Value], body: Option<&str>, cmd: &str, limit: (usize, usize)) -> Result<PickedFaces> {
    let st = if crate::frames::mapped() { s.model.state() } else { s.world_state() };
    let solids: Vec<usize> = (0..st.bodies.len()).filter(|i| st.bodies.get(*i).is_some_and(|b| !b.body.is_mesh())).collect();
    // The solids called `name` (by either of a body's names).
    let named = |name: &str| -> Vec<usize> {
        solids.iter().copied().filter(|i| st.bodies.get(*i).is_some_and(|b| b.name == name || b.definition_name() == name)).collect()
    };
    // The bodies the command's `body` keeps the picks to: every solid without one (or with one
    // no body is called: the evaluation reports that).
    let commands: Vec<usize> = body.map(named).filter(|named| !named.is_empty()).unwrap_or_else(|| solids.clone());
    let triangles: Vec<std::cell::OnceCell<Option<FaceTriangles>>> = st.bodies.iter().map(|_| std::cell::OnceCell::new()).collect();
    let mut entries: Vec<Entry> = Vec::with_capacity(list.len());
    // Did an entry name its body? The body is then kept with the feature, as when an entry lay
    // on faces of several bodies.
    let mut pinned = false;
    for (k, v) in list.iter().enumerate() {
        let entry = match face_ref_form(v).map_err(|why| bad(cmd, why))? {
            FaceRefForm::Name(name) => {
                let hit = polykrom_doc::naming::locate_one(&st, body, name).map_err(|why| bad(cmd, why))?;
                let b = st.bodies.get(hit.body).ok_or_else(|| bad(cmd, format!("no face is named `{name}`")))?;
                let spot = polykrom_doc::naming::face_spot(b, hit.face).ok_or_else(|| bad(cmd, format!("face `{name}` has no surface to pick")))?;
                let one = Candidate { distance: 0.0, body: hit.body, face: hit.face, normal: Vec3::ZERO, checked: true };
                Entry { point: spot.point, given: spot.point, normal: None, near: vec![one] }
            }
            FaceRefForm::Point(p) => {
                let normal = match v.get("normal") {
                    None | Some(Value::Null) => None,
                    Some(n) => Some(vec3(n).and_then(|n| n.normalized()).ok_or_else(|| bad(cmd, "a face's `normal` must be a direction [x, y, z]"))?),
                };
                // The point as the caller gave it, kept by the frame mapper (`crate::frames`).
                let given = v.get("world").and_then(vec3).unwrap_or(p);
                let bodies: Vec<usize> = match v.get("body") {
                    None | Some(Value::Null) => commands.clone(),
                    Some(b) => {
                        let name = b.as_str().ok_or_else(|| bad(cmd, format!("face {}: `body` must be a body name", k + 1)))?;
                        let named = named(name);
                        if named.is_empty() {
                            return Err(bad(cmd, format!("face {}: no body `{name}`", k + 1)));
                        }
                        pinned = true;
                        named
                    }
                };
                let mut candidates: Vec<Candidate> = Vec::new();
                for &i in &bodies {
                    let Some(b) = st.bodies.get(i) else { continue };
                    let reach = polykrom_doc::appearance::pick_tolerance(b);
                    match triangles.get(i).and_then(|cell| cell.get_or_init(|| FaceTriangles::new(b, limit)).as_ref()) {
                        Some(tris) => candidates.extend(tris.near(p, reach).into_iter().map(|(distance, face, normal)| Candidate {
                            distance,
                            body: i,
                            face,
                            normal,
                            checked: true,
                        })),
                        None => candidates.extend(nearest_face_unchecked(b, i, p, reach)),
                    }
                }
                candidates.sort_by(|a, b| a.distance.total_cmp(&b.distance));
                let near = match candidates.first() {
                    Some(nearest) => {
                        let tol = st.bodies.get(nearest.body).map(|b| b.body.size() * 1e-6).unwrap_or(1e-6).max(1e-6);
                        let within = nearest.distance + tol;
                        candidates.into_iter().take_while(|c| c.distance <= within).collect()
                    }
                    None => Vec::new(),
                };
                Entry { point: p, given, normal, near }
            }
        };
        entries.push(entry);
    }
    // The faces a command acts on belong to one body: the bodies every entry may lie on.
    let mut common: Option<std::collections::BTreeSet<usize>> = None;
    for e in entries.iter().filter(|e| !e.near.is_empty()) {
        let bodies: std::collections::BTreeSet<usize> = e.near.iter().map(|c| c.body).collect();
        common = Some(match common {
            None => bodies,
            Some(c) => c.intersection(&bodies).copied().collect(),
        });
    }
    let common = common.unwrap_or_default();
    // Did an entry lie on faces of several bodies? Its body is then kept with the feature.
    let spanned = entries.iter().any(|e| e.near.iter().any(|c| c.body != e.near.first().map_or(c.body, |n| n.body)));
    let mut out = PickedFaces { points: Vec::with_capacity(list.len()), names: Vec::with_capacity(list.len()), warning: None, body: None };
    let mut seen: Vec<((usize, usize), usize)> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let name_of =
        |body: usize, face: usize| st.bodies.get(body).and_then(|b| polykrom_doc::naming::face_names(b).get(face).cloned()).unwrap_or_default();
    let body_name = |body: usize| st.bodies.get(body).map(|b| b.name.clone()).unwrap_or_default();
    for (k, e) in entries.iter().enumerate() {
        // The faces of the operation's body, where that settles which body the entry means.
        let own: Vec<&Candidate> = e.near.iter().filter(|c| common.contains(&c.body)).collect();
        let near: Vec<&Candidate> = if own.is_empty() { e.near.iter().collect() } else { own };
        let (point, hit) = match near.as_slice() {
            [] => (e.point, None),
            [one] => {
                if !one.checked {
                    warnings.push(format!(
                        "face {}: `{}` is too big for the pick check; its nearest face `{}` was taken without checking for other faces as near",
                        k + 1,
                        body_name(one.body),
                        name_of(one.body, one.face)
                    ));
                }
                (settled_point(&st, e.point, one, cmd)?, Some((one.body, one.face)))
            }
            _ => {
                let Some(one) = e.normal.and_then(|n| facing(&near, n)) else {
                    return Err(bad(cmd, ambiguous_text(&st, e.given, &near)));
                };
                // Settled by `normal`: the stored point moves inside that face, as a named
                // face's does, so the kernel finds the same face.
                let b = st.bodies.get(one.body).ok_or_else(|| bad(cmd, "the picked body disappeared"))?;
                let spot = polykrom_doc::naming::face_spot(b, one.face).ok_or_else(|| bad(cmd, "the picked face has no surface to pick"))?;
                (spot.point, Some((one.body, one.face)))
            }
        };
        out.points.push(point);
        let Some(key) = hit else {
            out.names.push(String::new());
            continue;
        };
        let name = name_of(key.0, key.1);
        if let Some((_, first)) = seen.iter().find(|(k, _)| *k == key) {
            warnings.push(format!("faces {} and {} are the same face (`{name}` of `{}`)", first + 1, k + 1, body_name(key.0)));
        } else {
            seen.push((key, k));
        }
        out.names.push(name);
    }
    if !warnings.is_empty() {
        out.warning = Some(warnings.join("; "));
    }
    let bodies: std::collections::BTreeSet<usize> = seen.iter().map(|((body, _), _)| *body).collect();
    if body.is_none()
        && (spanned || pinned)
        && let [one] = bodies.into_iter().collect::<Vec<_>>().as_slice()
    {
        out.body = Some(body_name(*one));
    }
    Ok(out)
}

/// The point the feature keeps for a pick `p` settled on the face `c`: `p` where it lies on the
/// face, else a point inside the face (`naming::face_spot`, as for a face given by name).
///
/// The pick tolerance reaches 5 % of the body's size from a face, but a kernel finds the face
/// it is given by a far tighter test of its own (a shell opens the planar faces within a
/// ten-thousandth of the body's size of a point, `polycore::shell`) and used to drop a point
/// that passed neither without a word: `faces_used` named a face that stayed as it was (#297
/// review). On the face means on its analytic surface (within a millionth of the body's size),
/// and within the display mesh's chord tolerance of the mesh (so not out over a hole in the
/// face); a face of no analytic surface is read by its mesh alone, as the kernel reads it.
fn settled_point(st: &polykrom_doc::ModelState, p: Vec3, c: &Candidate, cmd: &str) -> Result<Vec3> {
    let Some(b) = st.bodies.get(c.body) else { return Ok(p) };
    let size = b.body.size();
    let mesh_tol = (size * 1e-3).max(1e-3);
    let on_face = match polycore::face_surface(&b.body, c.face).and_then(|s| s.nearest(p)) {
        Some(q) => q.dist(p) <= (size * 1e-6).max(1e-6) && c.distance <= mesh_tol,
        None => c.distance <= mesh_tol,
    };
    if on_face {
        return Ok(p);
    }
    let spot = polykrom_doc::naming::face_spot(b, c.face).ok_or_else(|| bad(cmd, "the picked face has no surface to pick"))?;
    Ok(spot.point)
}

/// Of the faces a pick may mean, the one facing `n` (within 45°), when one faces it clearly
/// more than the others.
fn facing<'a>(near: &[&'a Candidate], n: Vec3) -> Option<&'a Candidate> {
    let mut dots: Vec<(f64, &Candidate)> = near.iter().map(|c| (c.normal.dot(n), *c)).collect();
    dots.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (best, face) = dots.first().copied()?;
    let next = dots.get(1).map(|(d, _)| *d).unwrap_or(-1.0);
    (best >= std::f64::consts::FRAC_1_SQRT_2 - 1e-9 && best - next > 1e-6).then_some(face)
}

/// The ambiguous-pick error: the faces (nearest first, with their body) and the ways out.
fn ambiguous_text(st: &polykrom_doc::ModelState, p: Vec3, near: &[&Candidate]) -> String {
    let name = |c: &Candidate| {
        let body = st.bodies.get(c.body);
        let face = body
            .and_then(|b| polykrom_doc::naming::face_names(b).get(c.face).cloned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("#{}", c.face));
        format!("`{face}` of `{}`", body.map(|b| b.name.as_str()).unwrap_or(""))
    };
    let mut names: Vec<String> = near.iter().map(|c| name(c)).collect();
    let last = names.pop().unwrap_or_default();
    let listed = if names.is_empty() { last } else { format!("{} and {last}", names.join(", ")) };
    format!(
        "ambiguous pick at {}: it lies on faces {listed}; pick a point inside one face, give its name from model.faces, or give {{\"point\": [x, y, z], \"normal\": [x, y, z]}} with the face's outward normal",
        point_text(p)
    )
}

/// A face reference as given, before it is looked up: see [`picked_faces`].
pub(crate) enum FaceRefForm<'a> {
    Point(Vec3),
    Name(&'a str),
}

/// What [`picked_faces`] reads `v` as, without the design: a point, or a name of 1…512 bytes.
pub(crate) fn face_ref_form(v: &Value) -> std::result::Result<FaceRefForm<'_>, &'static str> {
    if let Some(name) = v.as_str() {
        if name.is_empty() || name.len() > 512 {
            return Err("a face name is 1…512 characters");
        }
        return Ok(FaceRefForm::Name(name));
    }
    vec3(v).or_else(|| v.get("point").and_then(vec3)).map(FaceRefForm::Point).ok_or("a face is a point [x, y, z] on it, or its name from model.faces")
}

/// The body face under a point: (mesh, face index, distance, body).
fn face_at(s: &Session, p: Vec3) -> Option<(std::sync::Arc<Mesh>, u32, f64, String)> {
    face_at_in(s, p, None).ok().flatten()
}

type FacePick = (std::sync::Arc<Mesh>, u32, f64, String);

/// The face under a point of `only` (any body when `None`). Where faces of several bodies are
/// equally near (bodies touching flush), the pick is ambiguous: `Err` names those bodies.
fn face_at_in(s: &Session, p: Vec3, only: Option<&str>) -> std::result::Result<Option<FacePick>, Vec<String>> {
    let st = s.model.state();
    face_at_state(&st, p, only)
}

pub(super) fn face_at_state(st: &polykrom_doc::ModelState, p: Vec3, only: Option<&str>) -> std::result::Result<Option<FacePick>, Vec<String>> {
    let mut best: Option<FacePick> = None;
    let mut tied: Vec<String> = Vec::new();
    for b in st.bodies.iter().filter(|b| only.is_none_or(|n| b.name == n)) {
        let m = b.mesh();
        let mut near: Option<(u32, f64)> = None;
        for (t, f) in m.triangles.iter().zip(&m.tri_face) {
            let Some([a, bb, c]) = m.tri(t) else { continue };
            let d = point_triangle_dist(p, a, bb, c);
            if near.is_none_or(|(_, nd)| d < nd) {
                near = Some((*f, d));
            }
        }
        if let Some((f, d)) = near {
            if best.as_ref().is_none_or(|x| d < x.2 - 1e-8) {
                best = Some((m.clone(), f, d, b.name.clone()));
                tied.clear();
            } else if best.as_ref().is_some_and(|x| (d - x.2).abs() <= 1e-8) {
                tied.push(b.name.clone());
            }
        }
    }
    match best {
        Some(b) if !tied.is_empty() => Err(std::iter::once(b.3).chain(tied).collect()),
        b => Ok(b),
    }
}

/// Circle through three points.
fn circumcircle(a: Vec2, b: Vec2, c: Vec2) -> Option<(Vec2, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (a.len2(), b.len2(), c.len2());
    let o = Vec2::new((a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d, (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d);
    let r = o.dist(a);
    (o.is_finite() && r.is_finite() && r < 1e8).then_some((o, r))
}

fn xy(v: Vec2) -> Value {
    json!([v.x, v.y])
}

/// Coplanar faces share one linked profile sketch and one Extrude feature. Resolve every
/// pick before editing the document, so later picks never search an already modified body.
pub(super) fn sketch_of_faces(s: &mut Session, points: &[Vec3], cmd: &str, face_body: Option<&str>) -> Result<(u64, Vec<Vec2>, String)> {
    let first = points.first().copied().ok_or_else(|| bad(cmd, "faces must not be empty"))?;
    if points.len() > 1024 {
        return Err(bad(cmd, "faces exceed the 1024-face limit"));
    }
    let state = s.model.state();
    let mut picked = Vec::new();
    let mut common: Option<(String, polycore_geom::Plane)> = None;
    for &point in points {
        if !point.is_finite() {
            return Err(bad(cmd, "faces must be finite points"));
        }
        let (mesh, index, distance, name) = face_at_in(s, point, face_body)
            .map_err(|_| bad(cmd, "ambiguous face; specify face_body"))?
            .ok_or_else(|| bad(cmd, "no body face at a selected point"))?;
        let body = state.body(&name).ok_or_else(|| bad(cmd, "selected body disappeared"))?;
        let plane = body.body.planar_face(index as usize).ok_or_else(|| bad(cmd, "faces must be planar"))?;
        let tolerance = (body.body.size() * 1e-9).max(1e-8);
        if distance > (mesh.bounds().diagonal() * 1e-3).max(1e-3) * 10.0 {
            return Err(bad(cmd, "no body face at a selected point"));
        }
        if let Some((previous, base)) = &common {
            if *previous != name || base.normal().dot(plane.normal()) < 1.0 - 1e-9 || base.height(plane.origin).abs() > tolerance {
                return Err(bad(cmd, "faces must be coplanar, equally oriented faces of one body"));
            }
        } else {
            common = Some((name.clone(), plane));
        }
        if picked.iter().any(|(face, _)| *face == index as usize) {
            continue;
        }
        let inside = mesh
            .triangles
            .iter()
            .zip(&mesh.tri_face)
            .filter(|(_, face)| **face == index)
            .filter_map(|(triangle, _)| mesh.tri(triangle))
            .max_by(|[a, b, c], [x, y, z]| ((*b - *a).cross(*c - *a).len()).total_cmp(&((*y - *x).cross(*z - *x).len())))
            .map(|[a, b, c]| (a + b + c) / 3.0)
            .ok_or_else(|| bad(cmd, "degenerate face"))?;
        picked.push((index as usize, inside));
    }
    let (id, first_inside, _) = sketch_of_face(s, first, cmd, face_body)?;
    let (body, _) = common.ok_or_else(|| bad(cmd, "no faces selected"))?;
    let doc = s.placed_document();
    let source = state.body(&body).ok_or_else(|| bad(cmd, "source body disappeared"))?;
    let component = doc.model_body_component(source);
    let path = if component == s.active_component {
        s.active_occurrence_path.clone().unwrap_or_else(|| doc.first_path(component))
    } else {
        let placements = super::component::properties::placements(&doc, cmd)?;
        if placements.iter().filter(|(owner, _, _)| *owner == component).count() != 1 {
            return Err(bad(cmd, "activate the selected occurrence before extruding its faces"));
        }
        doc.first_path(component)
    };
    let placement = EmbossFace::component_frame(&doc, component, &path)?;
    let placement = doc.model_body_offset(source).map(|offset| mat_mul(&placement, offset)).unwrap_or(placement);
    let author_path = s.active_occurrence_path.clone().unwrap_or_else(|| doc.first_path(s.active_component));
    let author = EmbossFace::component_frame(&doc, s.active_component, &author_path)?;
    let inverse = mat_inverse(&author).ok_or_else(|| bad(cmd, "invalid face profile author frame"))?;
    let into_author = mat_mul(&inverse, &placement);
    drop(doc);
    let indices: Vec<_> = picked.iter().skip(1).map(|(index, _)| *index).collect();
    super::sketch_project::project_internal_faces(s, id, &body, &indices)?;
    s.refresh();
    let plane = s.model.state().sketch(id).map(|sketch| sketch.plane).ok_or_else(|| bad(cmd, "face profile sketch disappeared"))?;
    let mut selected = vec![first_inside];
    selected.extend(picked.iter().skip(1).map(|(_, point)| plane.to_local(apply_point(&into_author, *point))));
    Ok((id, selected, body))
}

/// Start a sketch on the planar face at `point` holding a copy of the face boundary. Returns
/// the sketch id, a point inside the face (sketch coordinates), and the picked body's name.
/// The sketch is left active.
pub(super) fn sketch_of_face(s: &mut Session, point: Vec3, cmd: &str, face_body: Option<&str>) -> Result<(u64, Vec2, String)> {
    let none = || match face_body {
        Some(b) => bad(cmd, format!("no face of `{b}` at `face`")),
        None => bad(cmd, "no body face at `face`"),
    };
    let (m, f, d, body) = face_at_in(s, point, face_body)
        .map_err(|names| bad(cmd, format!("faces of several bodies meet at `face` ({}): name one with `face_body`", names.join(", "))))?
        .ok_or_else(none)?;
    let tol = (m.bounds().diagonal() * 1e-3).max(1e-3);
    if d > tol * 10.0 {
        return Err(none());
    }
    let tris: Vec<[Vec3; 3]> = m.triangles.iter().zip(&m.tri_face).filter(|(_, tf)| **tf == f).filter_map(|(t, _)| m.tri(t)).collect();
    let normal = tris.iter().find_map(|[a, b, c]| (*b - *a).cross(*c - *a).normalized()).ok_or_else(|| bad(cmd, "degenerate face"))?;
    if tris.iter().any(|[a, b, c]| (*b - *a).cross(*c - *a).normalized().is_some_and(|n| n.dot(normal) < 1.0 - 1e-6)) {
        return Err(bad(cmd, "only planar faces can be extruded"));
    }
    // A point well inside the face: the centroid of its largest triangle.
    let inside = tris
        .iter()
        .max_by(|x, y| (x[1] - x[0]).cross(x[2] - x[0]).len().total_cmp(&(y[1] - y[0]).cross(y[2] - y[0]).len()))
        .map(|[a, b, c]| (*a + *b + *c) / 3.0)
        .unwrap_or(point);
    let edges: Vec<Vec<Vec3>> =
        m.face_edges(f).into_iter().filter(|e| !m.seams.get(*e).copied().unwrap_or(false)).filter_map(|e| m.edges.get(e).cloned()).collect();
    if edges.is_empty() || edges.len() > 5000 {
        return Err(bad(cmd, "the face has no usable boundary"));
    }
    let run = |s: &mut Session, id: &str, p: Value| -> Result<Value> {
        let spec = crate::find_command(id).ok_or_else(|| bad(cmd, id))?;
        (spec.run)(s, &p)
    };
    // Command frame normalization already made `point` definition-local. Capture takes
    // world coordinates; convert exactly once using the picked body's actual occurrence.
    let state = s.model.state();
    let doc = s.placed_document();
    let picked = state.body(&body).ok_or_else(|| bad(cmd, "picked body disappeared"))?;
    let component = doc.model_body_component(picked);
    let path = if component == s.active_component {
        s.active_occurrence_path.clone().unwrap_or_else(|| doc.first_path(component))
    } else {
        // The command mapper discarded the original placement when making this pick
        // definition-local. A repeated cross-owner source cannot be re-found safely.
        let placements = super::component::properties::placements(&doc, cmd)?;
        if placements.iter().filter(|(owner, _, _)| *owner == component).count() != 1 {
            return Err(bad(cmd, "activate the picked occurrence before generating a face profile from a repeated component"));
        }
        doc.first_path(component)
    };
    let placement = EmbossFace::component_frame(&doc, component, &path)?;
    let placement = doc.model_body_offset(picked).map(|offset| mat_mul(&placement, offset)).unwrap_or(placement);
    let world_inside = apply_point(&placement, inside);
    let world_body = doc.world_body_name(&state, &body, component, &path);
    let author_path = s.active_occurrence_path.clone().unwrap_or_else(|| doc.first_path(s.active_component));
    let author = EmbossFace::component_frame(&doc, s.active_component, &author_path)?;
    let author_inverse = mat_inverse(&author).ok_or_else(|| bad(cmd, "invalid face profile author frame"))?;
    let author_inside = apply_point(&author_inverse, world_inside);
    drop(doc);
    let r = super::sketch::create_face_profile(
        s,
        &json!({"plane": {"face": [world_inside.x, world_inside.y, world_inside.z], "body": world_body, "index": f, "path":path}, "project_edges": true}),
        &body,
        f as usize,
    )?;
    let sid = r.get("sketch").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "could not start a sketch on the face"))?;
    s.refresh();
    let plane = s.model.state().sketch(sid).map(|ss| ss.plane).ok_or_else(|| bad(cmd, "could not start a sketch on the face"))?;
    // The face's edges came in as linked projections: they follow the face when earlier
    // features change, so nothing is copied.
    if r.get("projected").is_some_and(|x| !x.is_null()) {
        return Ok((sid, plane.to_local(author_inside), body));
    }
    for e in &edges {
        let pts: Vec<Vec2> = e.iter().map(|p| plane.to_local(*p)).collect();
        let (Some(first), Some(last)) = (pts.first().copied(), pts.last().copied()) else { continue };
        let closed = pts.len() > 3 && first.dist(last) < tol;
        let circle = if pts.len() >= 4 {
            let n = pts.len() - usize::from(closed);
            let (a, b, c) = (pts.first(), pts.get(n / 3), pts.get(2 * n / 3));
            match (a, b, c) {
                (Some(a), Some(b), Some(c)) => circumcircle(*a, *b, *c).filter(|(o, r)| pts.iter().all(|q| (q.dist(*o) - r).abs() < tol)),
                _ => None,
            }
        } else {
            None
        };
        match circle {
            Some((o, r)) if closed => run(s, "sketch.circle.center", json!({"center": xy(o), "radius": r}))?,
            Some(_) => {
                let mid = pts.get(pts.len() / 2).copied().unwrap_or(first);
                run(s, "sketch.arc.three_point", json!({"start": xy(first), "end": xy(last), "through": xy(mid)}))?
            }
            None => {
                let mut list: Vec<Value> = Vec::new();
                for q in &pts {
                    // Drop points that repeat (a closed polyline ends where it starts).
                    if list.last().is_none_or(|l| l != &xy(*q)) {
                        list.push(xy(*q));
                    }
                }
                let closed_poly = list.len() > 2 && list.first() == list.last();
                if closed_poly {
                    list.pop();
                }
                run(s, "sketch.line", json!({"points": list, "closed": closed_poly}))?
            }
        };
    }
    Ok((sid, plane.to_local(author_inside), body))
}

#[cfg(test)]
#[path = "face_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "face_join_tests.rs"]
mod join_tests;

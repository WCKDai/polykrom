//! How finished a design is (#193), measured so an agent can act on it the way `inspect.statics`
//! measures torque. Per body, by category: sharp exposed edges with a suggested fillet radius, the
//! radius family, boxiness and slivers (`look`); first-layer edges, overhangs, thin walls and tight
//! screw holes of printed bodies (`print`); sharp inside corners of printed bodies (`strength`);
//! solid printed bulk (`mass`); default names and missing materials or appearances (`tidy`); and
//! occurrences nothing holds (`assembly`). Each category scores 0–100, each body and the design
//! get one overall score, and `fixes` lists ready-to-run fillets and chamfers by visual impact.
//! Everything is read from the bodies' cached display meshes (edge polylines, the faces each
//! edge bounds, vertex normals) in world placement, so it is quick and the same every time.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{Value, json};
use solidcraft_doc::ModelBody;
use solidcraft_geom::compare::closest_on_triangle;
use solidcraft_geom::{Aabb3, Bvh, Mesh, MeshIndex, Vec3};

use super::CommandSpec;
use super::printability::{AXES, measure};
use crate::params::{bad, num, string_list};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec::new("inspect.polish", "Polish", polish).icon("inspect").noundo().params(
    "bodies?: [names] (default every visible solid body; fasteners, bodies with an outside thread, and bought-part envelopes, bodies whose material name says envelope, are left out); max_fixes?: 1…100 \
     (default 10); sharp_deg?: the bend between two faces above which an edge is sharp (default 20); printed?: [names] (printed \
     whatever their material; PLA, PETG and TPU bodies are printed); skip?: [names]; style?: auto|organic|faceted|technical (the \
     finish aimed at, default auto: faceted when the parts carry more chamfer faces than fillet faces, else organic; organic: \
     fillets in a radius family, flat faces meeting square read as boxy; faceted: hard-surface facets with crisp chamfers in a \
     few sizes, flat facets are good, fixes are chamfers; technical: edges just broken, small fillets, flat faces fine) → style: \
     {asked, used, chamfer_faces, fillet_faces, flat_share}, colours: {palette: [{color, bodies}], near_duplicates: [[a, b]]}, score (0–100, the design: the categories' \
     weighted mean, look 4, print 2, strength 1.5, tidy 1, assembly 1, mass 0.5), scores: {look, print, strength, mass, tidy, \
     assembly} (area-weighted over the bodies), bodies: [{body, definition, score, scores, printed, sharp_edges (convex, a bend of \
     sharp_deg or more, not on a fillet or chamfer face and not lying on another body: a seat or a mate), sharp_edge_mm, edges: [the \
     longest 8: {name, mid, length, angle_deg, radius (suggested: 0.15 × the narrower face's width (2·area/perimeter, the wall \
     thickness at a rim), at most 3 mm on a part under 150 mm and 2% of a larger one, rounded down to 0.3, 0.5, 0.8, 1, 1.5, 2, 2.5, \
     3, 4, 5, 6, 8, 10…)}], radii: [{radius, faces}] (fillets: cylinder, torus and sphere faces tangent to two neighbours or more), \
     radius_outliers, chamfers, boxiness (the share of area on planar faces whose edges all bend 80–100°), slivers: {faces (narrower \
     than max(0.2 mm, size/500)), edges (shorter than max(0.1 mm, size/1000))}, print?: {up (the best of the six axis directions), \
     first_layer_edges (sharp edges of the face on the bed), overhang_mm2 (facing down more than 45° from vertical), thin_mm2 \
     (walls under 1.2 mm, sampled), min_wall_mm, tight_holes: [{diameter, screw, holes}] (untapped holes within nominal…nominal + 0.2 mm of an M2…M10 \
     screw)}, inside_corners (printed: sharp concave edges), mean_wall_mm (2 × volume / area), findings: [{category, severity: \
     high|medium|low, kind, message, fix?}]}] (lowest score first), assembly: [findings], fixes: [{id, category, kind: fillet|chamfer, \
     safe, body, edges, length_mm, impact, command, params}] (ready for engine.execute, most visual impact first: a fillet's edge \
     length, inside corners × 0.6, bed chamfers × 0.4), skipped: [{body, why}], contact_pairs_unchecked? (past 200000 pairs of \
     touching boxes, contacts aren't all looked for). Scores: look (organic) 100 − 60 × sharp share of edge \
     length − 25 × boxiness − 5 per radius past three (≤ 10) − 25 × the chamfers' share of its rounds and chamfers − 10 for both on one part − 5 per \
     sliver (≤ 15); (faceted) 100 − 50 × sharp share − 5 per chamfer size past two (≤ 15) − 25 × the fillets' share − 15 for both \
     on one part − slivers; \
     (technical) 100 − 40 × sharp share − 10 × boxiness − 20 × the share of rounds and chamfers past max(1 mm, 1% of the part) − slivers; bed chamfers don't count as a part's edge treatment; print 100 − 20 × sharp share of the bed's edges \
     − 200 × overhang share (≤ 40) − 100 × thin share (≤ 25) − 5 per tight hole (≤ 15); strength 100 − 8 per inside corner (≤ 60); \
     mass 70 for a printed body over 20 cm³ whose mean wall is over 6 mm (shell or pocket it), else 100; tidy 100 − 10 for a default \
     name, − 10 without a material when others have one, − 5 for the default colour, − 5 for a colour \
     within 30 (RGB) of another part's, − 5 for a printed part alone in its colour, − 5 for faces in other colours; assembly 100 − 10 per \
     occurrence neither grounded nor jointed (≤ 60)",
)];

const SHARP_DEG: f64 = 20.0;
/// Below this bend two faces are tangent: a fillet's sides.
const TANGENT_DEG: f64 = 5.0;
/// Bends of a chamfer face's long edges.
const CHAMFER_DEG: (f64, f64) = (25.0, 65.0);
const MAX_BODIES: usize = 2000;
const MAX_EDGES: usize = 50_000;
/// Triangles sampled per printed body for wall thickness.
const WALL_SAMPLES: usize = 600;
const MIN_WALL: f64 = 1.2;
const LISTED: usize = 8;
const MAX_FIX_EDGES: usize = 200;
/// Most pairs of bodies whose boxes meet that are looked at for contact.
const MAX_PAIRS: usize = 200_000;
const RADII: [f64; 17] = [0.3, 0.5, 0.8, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 25.0];
const SCREWS: [f64; 8] = [2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];
/// Category weights in the overall score.
const WEIGHTS: [(&str, f64); 6] = [("look", 4.0), ("print", 2.0), ("strength", 1.5), ("tidy", 1.0), ("assembly", 1.0), ("mass", 0.5)];

/// Another body's surface, to tell whether an edge lies on it.
struct Near {
    tris: Vec<[Vec3; 3]>,
    bvh: Bvh,
    tol: f64,
}

impl Near {
    fn new(m: &Mesh, tol: f64) -> Near {
        let tris: Vec<[Vec3; 3]> = m.triangles.iter().filter_map(|t| m.tri(t)).filter(|p| p.iter().all(|v| v.is_finite())).collect();
        let boxes: Vec<Aabb3> = tris.iter().map(|p| p.iter().copied().collect()).collect();
        Near { bvh: Bvh::new(&boxes), tris, tol }
    }

    fn touches(&self, p: Vec3, d: f64) -> bool {
        let hit = std::cell::Cell::new(false);
        self.bvh.visit(
            |bx| !hit.get() && bx.inflated(d).contains(p),
            |i| {
                if !hit.get() && self.tris.get(i as usize).is_some_and(|[a, b, c]| closest_on_triangle(p, *a, *b, *c).dist(p) <= d) {
                    hit.set(true);
                }
            },
        );
        hit.get()
    }
}

/// The finish a design aims at: its edge treatment and how flat faces count.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Style {
    /// Rounded: a family of fillet radii; flat faces meeting square read as boxy.
    Organic,
    /// Hard-surface facets: crisp chamfers in a few sizes; flat facets are the point.
    Faceted,
    /// Machined: edges just broken (small fillets or chamfers, either); flat faces are fine.
    Technical,
}

impl Style {
    fn name(self) -> &'static str {
        match self {
            Style::Organic => "organic",
            Style::Faceted => "faceted",
            Style::Technical => "technical",
        }
    }
}

/// What one body comes to.
struct Report {
    json: Value,
    scores: BTreeMap<&'static str, f64>,
    area: f64,
    /// For `style: auto`: its chamfer faces (bed chamfers left out), fillet faces, and flat area.
    treatment: (usize, usize, f64),
    /// (kind, radius or distance, edge midpoints with their length and where they are in the
    /// body's own coordinates, which placements of one part share).
    fixes: Vec<(&'static str, f64, Vec<(Vec3, f64, Vec3)>)>,
}

/// One B-rep edge between two faces, as the mesh shows it.
struct Edge {
    index: usize,
    faces: [usize; 2],
    angle: f64,
    convex: bool,
    length: f64,
    mid: Vec3,
    quarter: [Vec3; 2],
}

fn polish(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "inspect.polish";
    let in_range = |k: &str, default: f64, lo: f64, hi: f64| -> Result<f64> {
        match p.get(k).filter(|v| !v.is_null()) {
            None => Ok(default),
            Some(_) => num(p, k).filter(|v| v.is_finite() && (lo..=hi).contains(v)).ok_or_else(|| bad(cmd, format!("`{k}` must be {lo} to {hi}"))),
        }
    };
    let max_fixes = in_range("max_fixes", 10.0, 1.0, 100.0)? as usize;
    let asked_style = match p.get("style").and_then(Value::as_str) {
        None | Some("auto") => None,
        Some("organic") => Some(Style::Organic),
        Some("faceted") => Some(Style::Faceted),
        Some("technical") => Some(Style::Technical),
        Some(o) => return Err(bad(cmd, format!("`style` is auto, faceted, organic or technical, not `{o}`"))),
    };
    if p.get("style").is_some_and(|v| !v.is_null() && !v.is_string()) {
        return Err(bad(cmd, "`style` is auto, faceted, organic or technical"));
    }
    let sharp = in_range("sharp_deg", SHARP_DEG, TANGENT_DEG, 90.0)?;
    let (asked, printed, skip) = (string_list(p, "bodies"), string_list(p, "printed"), string_list(p, "skip"));
    let st = s.world_state();
    // Every shown body can hide another's edges (fasteners too); the asked ones are judged.
    let shown: Vec<&ModelBody> = st.bodies.iter().filter(|b| asked.contains(&b.name) || s.body_visible(&b.name)).collect();
    if let Some(missing) = asked.iter().chain(&printed).find(|n| !st.bodies.iter().any(|b| &b.name == *n)) {
        return Err(bad(cmd, format!("no body `{missing}`")));
    }
    if shown.len() > MAX_BODIES {
        return Err(bad(cmd, format!("at most {MAX_BODIES} bodies")));
    }
    let threaded: BTreeSet<&str> = st.threads.iter().filter(|t| !t.internal).map(|t| t.body.as_str()).collect();
    let mut skipped = Vec::new();
    let mut todo = Vec::new();
    for (i, b) in shown.iter().enumerate() {
        let why = if b.body.is_mesh() {
            Some("mesh body")
        } else if threaded.contains(b.name.as_str()) || threaded.contains(b.definition_name()) {
            Some("fastener (outside thread)")
        } else if s.doc.body_material(b.definition_name()).to_ascii_lowercase().contains("envelope") {
            Some("bought-part envelope (its material says so)")
        } else if skip.contains(&b.name) {
            Some("skipped")
        } else {
            None
        };
        match why {
            _ if !asked.is_empty() && !asked.contains(&b.name) => {}
            Some(why) => skipped.push(json!({"body": b.name, "why": why})),
            None => todo.push(i),
        }
    }
    // The pairs whose boxes meet.
    // A body whose mesh can't be made is listed as skipped, not judged on nothing.
    let meshes: Vec<std::sync::Arc<Mesh>> =
        solidcraft_kernel::map_with_local_pools(&shown, usize::MAX, |b| Ok(b.mesh())).into_iter().map(|m| m.unwrap_or_default()).collect();
    let names: Vec<&str> = shown.iter().map(|b| b.name.as_str()).collect();
    skipped.extend(unmeshed(&mut todo, &meshes, &names));
    let tols: Vec<f64> = shown.iter().map(|b| b.display_tol()).collect();
    let boxes: Vec<Aabb3> = meshes.iter().zip(&tols).map(|(m, t)| m.bounds().inflated(4.0 * t + 0.02)).collect();
    // Past the cap, contacts aren't all looked for: the reply says how many pairs were left out.
    let (pairs, unpaired) = super::joints::overlapping_pairs(&boxes, MAX_PAIRS);
    let mut partners: Vec<Vec<usize>> = vec![Vec::new(); shown.len()];
    for (i, j) in pairs {
        partners.get_mut(i).into_iter().for_each(|v| v.push(j));
        partners.get_mut(j).into_iter().for_each(|v| v.push(i));
    }
    let used: Vec<usize> =
        todo.iter().flat_map(|i| partners.get(*i).into_iter().flatten().copied()).collect::<BTreeSet<usize>>().into_iter().collect();
    let built =
        solidcraft_kernel::map_with_local_pools(&used, usize::MAX, |&i| Ok(meshes.get(i).map(|m| Near::new(m, tols.get(i).copied().unwrap_or(0.0)))));
    let mut near: HashMap<usize, Near> = HashMap::new();
    for (i, n) in used.iter().zip(built) {
        if let Some(n) = n? {
            near.insert(*i, n);
        }
    }
    let filament = |b: &ModelBody| {
        let m = s.doc.body_material(b.definition_name()).to_ascii_lowercase();
        printed.contains(&b.name) || ["pla", "petg", "tpu"].iter().any(|f| m.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == *f))
    };
    let jobs: Vec<(usize, bool)> = todo.iter().filter_map(|i| Some((*i, filament(shown.get(*i)?)))).collect();
    let report_all = |style: Style| {
        solidcraft_kernel::map_with_local_pools(&jobs, usize::MAX, |&(i, printed)| {
            let (Some(b), Some(m)) = (shown.get(i), meshes.get(i)) else { return Ok(None) };
            let others: Vec<&Near> = partners.get(i).into_iter().flatten().filter_map(|j| near.get(j)).collect();
            let taps: Vec<(Vec3, Vec3)> = st
                .threads
                .iter()
                .filter(|t| t.internal && (t.body == b.name || t.body == b.definition_name()))
                .map(|t| (t.axis_point, *t.axis))
                .collect();
            Ok(Some(body_report(b, m, &others, printed, sharp, &taps, style)))
        })
    };
    // `auto`: the style the design already leans to, from the edge treatments its parts have
    // (chamfers against fillets); a design with neither is taken as organic.
    let mut reports = report_all(asked_style.unwrap_or(Style::Organic));
    let (bevels, rounds, flat, total) = reports
        .iter()
        .filter_map(|r| r.as_ref().ok()?.as_ref())
        .fold((0, 0, 0.0, 0.0), |acc, r| (acc.0 + r.treatment.0, acc.1 + r.treatment.1, acc.2 + r.treatment.2, acc.3 + r.area));
    let style = asked_style.unwrap_or(if bevels > rounds { Style::Faceted } else { Style::Organic });
    if style != asked_style.unwrap_or(Style::Organic) {
        reports = report_all(style);
    }
    let style_json = json!({"asked": asked_style.map_or("auto", Style::name), "used": style.name(), "chamfer_faces": bevels, "fillet_faces": rounds,
        "flat_share": if total > 0.0 { round(flat / total) } else { 0.0 }});
    let mut bodies: Vec<Report> = Vec::new();
    for (r, (i, _)) in reports.into_iter().zip(&jobs) {
        match r {
            Ok(Some(r)) => bodies.push(r),
            Ok(None) => {}
            Err(e) => skipped.push(json!({"body": shown.get(*i).map(|b| b.name.clone()), "why": format!("not measured: {e}")})),
        }
    }
    // Colours: each judged body's, and whether its faces carry others.
    let colour = |name: &str| st.body(name).map(|b| (s.doc.body_color(b).map(to8), s.doc.face_colors(b)));
    let looks: BTreeMap<String, (Option<[u8; 3]>, bool)> = bodies
        .iter()
        .filter_map(|r| {
            let name = r.json["body"].as_str()?.to_string();
            let (c, faces) = colour(&name)?;
            let several = faces.iter().any(|(_, l)| Some(l.color) != c);
            Some((name, (c, several)))
        })
        .collect();
    let palette = Palette::of(&looks, &bodies);
    for r in &mut bodies {
        tidy(s, r, &looks, &palette);
    }
    let mut out = design(s, &mut bodies, max_fixes);
    out["style"] = style_json;
    out["colours"] = palette.json();
    out["skipped"] = json!(skipped);
    if unpaired > 0 {
        out["contact_pairs_unchecked"] = json!(unpaired);
        if let Some(list) = out["not_checked"].as_array_mut() {
            list.push(json!(format!(
                "seats and mates of {unpaired} pairs of bodies past the first {MAX_PAIRS}: their edges may be listed as exposed"
            )));
        }
    }
    Ok(out)
}

/// Drops from `todo` the bodies whose mesh is empty (tessellation failed): they are listed as
/// skipped, never scored as if clean.
fn unmeshed(todo: &mut Vec<usize>, meshes: &[std::sync::Arc<Mesh>], names: &[&str]) -> Vec<Value> {
    let mut out = Vec::new();
    todo.retain(|i| {
        let meshed = meshes.get(*i).is_some_and(|m| !m.triangles.is_empty());
        if !meshed {
            out.push(json!({"body": names.get(*i), "why": "no mesh (tessellation failed)"}));
        }
        meshed
    });
    out
}

/// Overall score from category scores, weighted.
fn overall(scores: &BTreeMap<&'static str, f64>) -> f64 {
    let (sum, w) = WEIGHTS.iter().filter_map(|(k, w)| Some((scores.get(k)? * w, *w))).fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
    if w > 0.0 { (sum / w).clamp(0.0, 100.0) } else { 100.0 }
}

fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0 + 0.0
}

/// The polyline's length and the points at a quarter, half and three quarters of it.
fn along(points: &[Vec3]) -> (f64, [Vec3; 3]) {
    let total: f64 = points.windows(2).map(|w| w.first().zip(w.get(1)).map_or(0.0, |(a, b)| a.dist(*b))).sum();
    let at = |f: f64| {
        let mut left = total * f;
        for w in points.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else { continue };
            let d = a.dist(*b);
            if d >= left && d > 0.0 {
                return a.lerp(*b, left / d);
            }
            left -= d;
        }
        points.last().copied().unwrap_or(Vec3::ZERO)
    };
    (total, [at(0.25), at(0.5), at(0.75)])
}

/// The bends of a body's edges: at a vertex of each edge's polyline, each face's normal there and
/// which side of the other face's tangent plane it turns to.
fn edges(m: &Mesh, planar: &[Option<Vec3>], tol: f64) -> Vec<Edge> {
    let cell = tol.max(1e-6);
    let key = |p: Vec3| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64, (p.z / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64, i64), Vec<(u32, u32)>> = HashMap::new();
    for (t, tri) in m.triangles.iter().enumerate() {
        for v in tri {
            if let Some(p) = m.positions.get(*v as usize) {
                grid.entry(key(*p)).or_default().push((t as u32, *v));
            }
        }
    }
    // Face `f`'s normal at its vertex nearest `p`, and the middle of a triangle there.
    let side = |f: u32, p: Vec3| -> Option<(Vec3, Vec3)> {
        let (kx, ky, kz) = key(p);
        let mut best: Option<(f64, u32, u32)> = None;
        for c in (-1..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| (kx + x, ky + y, kz + z)))) {
            for &(t, v) in grid.get(&c).into_iter().flatten() {
                let d = m.positions.get(v as usize)?.dist(p);
                if m.tri_face.get(t as usize) == Some(&f) && best.is_none_or(|b| d < b.0) {
                    best = Some((d, t, v));
                }
            }
        }
        let (_, t, v) = best?;
        let [a, b, c] = m.tri(m.triangles.get(t as usize)?)?;
        let flat = (b - a).cross(c - a).normalized()?;
        let n = match planar.get(f as usize).copied().flatten() {
            Some(n) => n,
            None => m.normals.get(v as usize).and_then(|n| n.normalized()).filter(|n| n.dot(flat) > 0.0).unwrap_or(flat),
        };
        Some((n, (a + b + c) / 3.0))
    };
    let mut out = Vec::new();
    for (index, (line, faces)) in m.edges.iter().zip(&m.edge_faces).enumerate().take(MAX_EDGES) {
        let (Some(&f1), Some(&f2)) = (faces.first(), faces.get(1)) else { continue };
        if f1 == f2 || m.seams.get(index).copied().unwrap_or(false) {
            continue;
        }
        let (length, [q1, mid, q3]) = along(line);
        let Some(&p) = line.get(line.len() / 2) else { continue };
        let (Some((n1, c1)), Some((n2, c2))) = (side(f1, p), side(f2, p)) else { continue };
        let angle = n1.dot(n2).clamp(-1.0, 1.0).acos().to_degrees();
        let convex = (c1 - p).dot(n2) + (c2 - p).dot(n1) < 0.0;
        out.push(Edge { index, faces: [f1 as usize, f2 as usize], angle, convex, length, mid, quarter: [q1, q3] });
    }
    out
}

/// The radius to suggest for an edge between faces `w` wide, on a part `size` across.
fn suggest(w: f64, size: f64) -> Option<f64> {
    let cap = if size < 150.0 { 3.0 } else { 0.02 * size };
    let r = (0.15 * w).min(cap);
    RADII.iter().rev().find(|x| **x <= r).copied().filter(|x| *x <= 0.4 * w)
}

/// Chamfer sizes a faceted part keeps to.
const BEVELS: [f64; 6] = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0];

/// The chamfer to suggest for an edge between faces `w` wide on a part `size` across: the size
/// the part already uses most when it fits, else the largest of [`BEVELS`] within 0.3 × `w`, at
/// most 3 mm on a part under 150 mm and 2% of a larger one.
fn bevel(w: f64, size: f64, usual: Option<f64>) -> Option<f64> {
    let cap = if size < 150.0 { 3.0 } else { 0.02 * size };
    let snapped = usual.and_then(|u| BEVELS.iter().copied().min_by(|a, b| (a - u).abs().total_cmp(&(b - u).abs())));
    if let Some(u) = snapped.filter(|u| *u <= (0.4 * w).min(cap)) {
        return Some(u);
    }
    let d = (0.3 * w).min(cap);
    BEVELS.iter().rev().find(|x| **x <= d).copied()
}

fn body_report(b: &ModelBody, m: &Mesh, others: &[&Near], printed: bool, sharp: f64, taps: &[(Vec3, Vec3)], style: Style) -> Report {
    let tol = b.display_tol();
    let bounds = m.bounds();
    let size = {
        let d = bounds.size();
        d.x.max(d.y).max(d.z)
    };
    let nf = m.tri_face.iter().map(|f| *f as usize + 1).max().unwrap_or(0);
    // Per face: area, and its one normal when it is flat.
    let mut area = vec![0.0; nf];
    let mut normal: Vec<Option<Vec3>> = vec![None; nf];
    let mut flat = vec![true; nf];
    let tris: Vec<[Vec3; 3]> = m.triangles.iter().filter_map(|t| m.tri(t)).collect();
    for (t, f) in m.triangles.iter().zip(&m.tri_face) {
        let (Some([a, b, c]), f) = (m.tri(t), *f as usize) else { continue };
        let cr = (b - a).cross(c - a);
        area.get_mut(f).into_iter().for_each(|x| *x += cr.len() / 2.0);
        if let (Some(n), Some(slot), Some(ok)) = (cr.normalized(), normal.get_mut(f), flat.get_mut(f)) {
            match slot {
                None => *slot = Some(n),
                Some(was) if was.dot(n) < 1.0 - 1e-6 => *ok = false,
                _ => {}
            }
        }
    }
    let planar: Vec<Option<Vec3>> = normal.iter().zip(&flat).map(|(n, ok)| n.filter(|_| *ok)).collect();
    let all = edges(m, &planar, tol);
    let mut perimeter = vec![0.0; nf];
    let mut tangent = vec![0usize; nf];
    for e in &all {
        for f in e.faces {
            perimeter.get_mut(f).into_iter().for_each(|x| *x += e.length);
            if e.angle < TANGENT_DEG {
                tangent.get_mut(f).into_iter().for_each(|x| *x += 1);
            }
        }
    }
    let width: Vec<f64> = area.iter().zip(&perimeter).map(|(a, p)| if *p > 0.0 { 2.0 * a / p } else { a.sqrt() }).collect();
    let w = |f: usize| width.get(f).copied().unwrap_or(0.0);
    // Fillets: round faces tangent to two neighbours or more. Chamfers: narrow flat faces with
    // two convex edges bent 25–65°.
    let mut radii: Vec<f64> = Vec::new();
    let mut blend = vec![false; nf];
    for f in 0..nf {
        if planar.get(f).copied().flatten().is_none() && tangent.get(f).copied().unwrap_or(0) >= 2 {
            let r = match solidcraft_kernel::face_surface(&b.body, f) {
                Some(solidcraft_kernel::FaceSurface::Cylinder { radius, .. }) | Some(solidcraft_kernel::FaceSurface::Sphere { radius, .. }) => {
                    Some(radius)
                }
                Some(solidcraft_kernel::FaceSurface::Torus { minor, .. }) => Some(minor),
                _ => None,
            };
            if let (Some(r), Some(x)) = (r, blend.get_mut(f)) {
                *x = true;
                radii.push(r);
            }
        }
    }
    let mut chamfers = Vec::new();
    let mut chamfer_faces = Vec::new();
    // A chamfer is a strip: narrower than the widest face it bevels to (a facet whose every edge
    // is chamfered is bevelled, not a chamfer).
    let longest = |f: usize| all.iter().filter(|e| e.faces.contains(&f)).map(|e| e.length).fold(0.0, f64::max);
    let strip = |f: usize| if longest(f) > 0.0 { area.get(f).copied().unwrap_or(0.0) / longest(f) } else { w(f) };
    for f in 0..nf {
        let bevel_edges: Vec<&Edge> =
            all.iter().filter(|e| e.convex && e.faces.contains(&f) && (CHAMFER_DEG.0..=CHAMFER_DEG.1).contains(&e.angle)).collect();
        let across = bevel_edges.iter().filter_map(|e| e.faces.iter().find(|g| **g != f)).map(|g| strip(*g)).fold(0.0, f64::max);
        // Flat, or a cone: the chamfer round a hole's or a cylinder's rim.
        let flat_or_cone = || {
            planar.get(f).copied().flatten().is_some()
                || matches!(solidcraft_kernel::face_surface(&b.body, f), Some(solidcraft_kernel::FaceSurface::Cone { .. }))
        };
        if bevel_edges.len() >= 2 && w(f) < 0.2 * size && strip(f) < 0.5 * across && flat_or_cone() {
            blend.get_mut(f).into_iter().for_each(|x| *x = true);
            chamfers.push(round(strip(f)));
            chamfer_faces.push(f);
        }
    }
    let family = family(&radii);
    let is_blend = |f: usize| blend.get(f).copied().unwrap_or(false);
    let on_other = |e: &Edge| {
        let d = |n: &Near| (2.0 * (tol + n.tol)).max(0.02);
        [e.quarter[0], e.mid, e.quarter[1]].iter().filter(|p| others.iter().any(|n| n.touches(**p, d(n)))).count() >= 2
    };
    // Print: the best of the six axis directions, its bed faces, overhang and walls.
    let total_area: f64 = area.iter().sum();
    let mut up = Vec3::Z;
    let mut print = Value::Null;
    let mut bed: Vec<bool> = vec![false; nf];
    let mut print_score = None;
    let mut findings: Vec<Value> = Vec::new();
    let mut find = |category: &str, severity: &str, kind: &str, message: String| {
        findings.push(json!({"category": category, "severity": severity, "kind": kind, "message": message}));
    };
    let sin45 = std::f64::consts::FRAC_1_SQRT_2;
    if printed {
        let ways: Vec<(Vec3, _)> = AXES.iter().map(|a| Vec3::new(a[0], a[1], a[2])).map(|d| (d, measure(&tris, d, sin45))).collect();
        let best = ways.iter().min_by(|(_, x), (_, y)| x.projected.total_cmp(&y.projected).then(y.bed_contact.total_cmp(&x.bed_contact)));
        if let Some((d, _)) = best {
            up = *d;
        }
        let low = m.positions.iter().map(|p| p.dot(up)).fold(f64::INFINITY, f64::min);
        let mut high = vec![f64::NEG_INFINITY; nf];
        for (t, f) in m.triangles.iter().zip(&m.tri_face) {
            for p in m.tri(t).into_iter().flatten() {
                high.get_mut(*f as usize).into_iter().for_each(|h| *h = h.max(p.dot(up)));
            }
        }
        for (f, x) in bed.iter_mut().enumerate() {
            *x = planar.get(f).copied().flatten().is_some_and(|n| n.dot(up) < -0.999) && high.get(f).is_some_and(|h| h - low <= 1e-6 * size.max(1.0));
        }
        let way = measure(&tris, up, sin45);
        let (thin, min_wall) = walls(m, &tris, tol);
        let holes = tight_holes(b, m, nf, &planar, taps);
        let bed_edges: Vec<&Edge> = all.iter().filter(|e| e.faces.iter().any(|f| bed.get(*f).copied().unwrap_or(false))).collect();
        let bed_sharp: Vec<&&Edge> = bed_edges.iter().filter(|e| e.convex && e.angle >= sharp && !e.faces.iter().any(|f| is_blend(*f))).collect();
        let bed_len: f64 = bed_edges.iter().map(|e| e.length).sum();
        let bed_share = if bed_len > 0.0 { bed_sharp.iter().map(|e| e.length).sum::<f64>() / bed_len } else { 0.0 };
        let over = if total_area > 0.0 { way.overhang / total_area } else { 0.0 };
        let thin_share = if total_area > 0.0 { thin / total_area } else { 0.0 };
        print_score = Some(
            100.0
                - 20.0 * bed_share
                - (200.0 * over).min(40.0)
                - (100.0 * thin_share).min(25.0)
                - (5.0 * holes.iter().map(|h| h.2).sum::<usize>() as f64).min(15.0),
        );
        if !bed_sharp.is_empty() {
            find(
                "print",
                "medium",
                "first_layer_edges",
                format!("{} sharp edges on the bed face: chamfer them (0.5 mm) against elephant's foot", bed_sharp.len()),
            );
        }
        if over > 0.02 {
            find(
                "print",
                if over > 0.1 { "high" } else { "medium" },
                "overhang",
                format!("{:.0} mm² overhangs past 45° printed {:?} up", way.overhang, [up.x, up.y, up.z]),
            );
        }
        if thin > 0.0 {
            find("print", "medium", "thin_walls", format!("about {thin:.0} mm² of wall under {MIN_WALL} mm (thinnest {min_wall:.2} mm)"));
        }
        for (d, screw, n) in &holes {
            find("print", "medium", "tight_hole", format!("{n} × {d:.2} mm holes are tight for an M{screw} screw: make them {:.1} mm", screw + 0.3));
        }
        print = json!({"up": [up.x, up.y, up.z], "first_layer_edges": bed_sharp.len(), "overhang_mm2": round(way.overhang), "thin_mm2": round(thin),
            "min_wall_mm": min_wall.is_finite().then(|| round(min_wall)), "tight_holes": holes.iter().map(|(d, s, n)| json!({"diameter": d, "screw": s, "holes": n})).collect::<Vec<_>>()});
    }
    let on_bed = |e: &Edge| e.faces.iter().any(|f| bed.get(*f).copied().unwrap_or(false));
    let feature_len: f64 = all.iter().filter(|e| e.angle >= TANGENT_DEG).map(|e| e.length).sum::<f64>().max(1e-9);
    let exposed: Vec<&Edge> =
        all.iter().filter(|e| e.convex && e.angle >= sharp && !e.faces.iter().any(|f| is_blend(*f)) && !on_bed(e) && !on_other(e)).collect();
    let inside: Vec<&Edge> = if printed {
        all.iter().filter(|e| !e.convex && e.angle >= sharp && !e.faces.iter().any(|f| is_blend(*f)) && !on_other(e)).collect()
    } else {
        Vec::new()
    };
    let sharp_len: f64 = exposed.iter().map(|e| e.length).sum();
    let edge_radius = |e: &Edge| suggest(w(e.faces[0]).min(w(e.faces[1])), size);
    // Boxiness: area of flat faces whose edges all bend 80–100°.
    let boxy: f64 = (0..nf)
        .filter(|f| planar.get(*f).copied().flatten().is_some())
        .filter(|f| all.iter().filter(|e| e.faces.contains(f)).all(|e| (80.0..=100.0).contains(&e.angle)))
        .map(|f| area.get(f).copied().unwrap_or(0.0))
        .sum();
    let boxiness = if total_area > 0.0 { boxy / total_area } else { 0.0 };
    let sliver_faces = (0..nf).filter(|f| area.get(*f).is_some_and(|a| *a > 0.0) && w(*f) < (0.002 * size).max(0.2)).count();
    let sliver_edges = all.iter().filter(|e| e.length < (0.001 * size).max(0.1)).count();
    let extra_radii = family.len().saturating_sub(3);
    let outliers: Vec<f64> = if family.len() > 3 { family.iter().filter(|(_, n)| *n == 1).map(|(r, _)| *r).collect() } else { Vec::new() };
    // Chamfers a print needs on the bed's edges aren't the part's edge treatment.
    let styled_bevels: Vec<usize> = chamfer_faces
        .iter()
        .copied()
        .filter(|f| {
            !all.iter().any(|e| {
                e.faces.contains(f)
                    && (CHAMFER_DEG.0..=CHAMFER_DEG.1).contains(&e.angle)
                    && e.faces.iter().any(|g| bed.get(*g).copied().unwrap_or(false))
            })
        })
        .collect();
    // A 45° chamfer's face is √2 times as wide as the chamfer's distance.
    let bevel_family = self::family(&styled_bevels.iter().map(|f| strip(*f) / std::f64::consts::SQRT_2).collect::<Vec<_>>());
    let mixed = !radii.is_empty() && !styled_bevels.is_empty();
    let share = (sharp_len / feature_len).min(1.0);
    let slivers = (5.0 * (sliver_faces + sliver_edges) as f64).min(15.0);
    // The share of the part's rounds and chamfers that are the other style's.
    let blends = (radii.len() + styled_bevels.len()).max(1) as f64;
    let (bevel_share, round_share) = (styled_bevels.len() as f64 / blends, radii.len() as f64 / blends);
    // Technical parts have their edges broken lightly: rounds and chamfers past this are heavy.
    let light = (0.01 * size).max(1.0);
    let heavy: Vec<f64> =
        radii.iter().copied().chain(styled_bevels.iter().map(|f| strip(*f) / std::f64::consts::SQRT_2)).filter(|x| *x > light + 1e-6).collect();
    let look = match style {
        Style::Organic => {
            100.0
                - 60.0 * share
                - 25.0 * boxiness
                - (5.0 * extra_radii as f64).min(10.0)
                - 25.0 * bevel_share
                - if mixed { 10.0 } else { 0.0 }
                - slivers
        }
        Style::Faceted => {
            100.0
                - 50.0 * share
                - (5.0 * bevel_family.len().saturating_sub(2) as f64).min(15.0)
                - 25.0 * round_share
                - if mixed { 15.0 } else { 0.0 }
                - slivers
        }
        Style::Technical => 100.0 - 40.0 * share - 10.0 * boxiness - 20.0 * heavy.len() as f64 / blends - slivers,
    };
    if !exposed.is_empty() {
        let what = match style {
            Style::Organic => "fillet them",
            Style::Faceted => "chamfer them (faceted: crisp chamfers in a few sizes)",
            Style::Technical => "break them with a small fillet or chamfer",
        };
        find(
            "look",
            if share > 0.5 { "high" } else { "medium" },
            "sharp_edges",
            format!("{} sharp exposed edges ({sharp_len:.0} mm): {what}", exposed.len()),
        );
    }
    if style == Style::Organic && boxiness > 0.5 {
        find("look", "medium", "boxy", format!("{:.0}% of the area is flat faces meeting square: round, taper or loft the form", boxiness * 100.0));
    }
    if style == Style::Organic && extra_radii > 0 {
        find("look", "low", "radius_family", format!("{} distinct fillet radii: keep to three (small, medium, large)", family.len()));
    }
    if style == Style::Technical && !heavy.is_empty() {
        find(
            "look",
            "low",
            "heavy_edges",
            format!("{} rounds or chamfers larger than {light:.1} mm: a technical part's edges are just broken", heavy.len()),
        );
    }
    if style == Style::Faceted && bevel_family.len() > 2 {
        find("look", "low", "chamfer_family", format!("{} distinct chamfer sizes: keep to two", bevel_family.len()));
    }
    if mixed && style != Style::Technical {
        let keep = if style == Style::Faceted { "chamfers" } else { "fillets" };
        find(
            "look",
            "medium",
            "mixed_edges",
            format!("{} fillet and {} chamfer faces on one part: keep to {keep} in the {} style", radii.len(), styled_bevels.len(), style.name()),
        );
    }
    if sliver_faces + sliver_edges > 0 {
        find("look", "low", "slivers", format!("{sliver_faces} sliver faces and {sliver_edges} tiny edges"));
    }
    if !inside.is_empty() {
        find(
            "strength",
            "medium",
            "inside_corners",
            format!("{} sharp inside corners on a printed part: fillet them to spread the load", inside.len()),
        );
    }
    let volume = tris.iter().map(|[a, b, c]| a.dot(b.cross(*c)) / 6.0).sum::<f64>().abs();
    let mean_wall = if total_area > 0.0 { 2.0 * volume / total_area } else { 0.0 };
    let bulky = printed && volume > 20_000.0 && mean_wall > 6.0;
    if bulky {
        find("mass", "low", "solid", format!("{:.0} cm³ printed solid (mean wall {mean_wall:.1} mm): shell or pocket it", volume / 1000.0));
    }
    let mut scores = BTreeMap::from([("look", look.clamp(0.0, 100.0))]);
    if let Some(p) = print_score {
        scores.insert("print", p.clamp(0.0, 100.0));
        scores.insert("strength", (100.0 - (8.0 * inside.len() as f64).min(60.0)).clamp(0.0, 100.0));
        scores.insert("mass", if bulky { 70.0 } else { 100.0 });
    }
    // Fixes pick edges where they are shown, as an agent sees them; a command run on its own maps
    // them into the body's own frame (crate::frames). Placements of one part share `own`.
    let to_own = solidcraft_doc::mat_inverse(&b.source_transform).unwrap_or(solidcraft_doc::IDENTITY);
    let own = |p: Vec3| solidcraft_doc::apply_point(&to_own, p);
    let mut groups: BTreeMap<(&'static str, i64), Vec<(Vec3, f64, Vec3)>> = BTreeMap::new();
    // The chamfer size a faceted body already uses most, to keep to it.
    let usual = bevel_family.iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.total_cmp(&a.0))).map(|(c, _)| *c);
    let treat = |e: &Edge| -> Option<(&'static str, f64)> {
        let narrow = w(e.faces[0]).min(w(e.faces[1]));
        match style {
            Style::Organic => edge_radius(e).map(|r| ("fillet", r)),
            Style::Technical => edge_radius(e).map(|r| ("fillet", r.min(1.0))),
            Style::Faceted => bevel(narrow, size, usual).map(|d| ("bevel", d)),
        }
    };
    for e in exposed.iter() {
        if let Some((kind, d)) = treat(e) {
            groups.entry((kind, (d * 1000.0).round() as i64)).or_default().push((e.mid, e.length, own(e.mid)));
        }
    }
    for e in inside.iter() {
        if let Some(r) = edge_radius(e).map(|r| r.max(0.5)) {
            groups.entry(("inside", (r * 1000.0).round() as i64)).or_default().push((e.mid, e.length, own(e.mid)));
        }
    }
    for e in all.iter().filter(|e| on_bed(e) && e.convex && e.angle >= sharp && !e.faces.iter().any(|f| is_blend(*f))) {
        let d = if w(e.faces[0]).min(w(e.faces[1])) > 2.5 { 0.5 } else { 0.3 };
        groups.entry(("chamfer", (d * 1000.0) as i64)).or_default().push((e.mid, e.length, own(e.mid)));
    }
    let names = if exposed.is_empty() { Vec::new() } else { solidcraft_doc::naming::edge_names(b) };
    let mut longest = exposed.clone();
    longest.sort_by(|x, y| y.length.total_cmp(&x.length).then(x.index.cmp(&y.index)));
    let listed: Vec<Value> = longest
        .iter()
        .take(LISTED)
        .map(|e| {
            let mut v = json!({"name": names.get(e.index), "mid": [round(e.mid.x), round(e.mid.y), round(e.mid.z)], "length": round(e.length), "angle_deg": round(e.angle)});
            match treat(e) {
                Some(("bevel", d)) => v["chamfer"] = json!(d),
                Some((_, r)) => v["radius"] = json!(r),
                None => v["radius"] = Value::Null,
            }
            v
        })
        .collect();
    let json = json!({
        "body": b.name, "definition": b.definition_name(), "printed": printed, "area_mm2": round(total_area),
        "sharp_edges": exposed.len(), "sharp_edge_mm": round(sharp_len), "edges": listed,
        "radii": family.iter().map(|(r, n)| json!({"radius": r, "faces": n})).collect::<Vec<_>>(), "radius_outliers": outliers, "chamfers": chamfers,
        "boxiness": round(boxiness), "slivers": {"faces": sliver_faces, "edges": sliver_edges}, "print": print,
        "inside_corners": inside.len(), "mean_wall_mm": round(mean_wall), "findings": findings,
    });
    let flat: f64 = (0..nf).filter(|f| planar.get(*f).copied().flatten().is_some()).map(|f| area.get(f).copied().unwrap_or(0.0)).sum();
    Report {
        json,
        scores,
        area: total_area,
        treatment: (styled_bevels.len(), radii.len(), flat),
        fixes: groups.into_iter().map(|((k, r), v)| (k, r as f64 / 1000.0, v)).collect(),
    }
}

/// Distinct radii (within 2% or 0.05 mm of each other), with how many faces have each.
fn family(radii: &[f64]) -> Vec<(f64, usize)> {
    let mut sorted: Vec<f64> = radii.iter().copied().filter(|r| r.is_finite() && *r > 0.0).collect();
    sorted.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, usize)> = Vec::new();
    for r in sorted {
        match out.last_mut() {
            Some((x, n)) if r - *x <= (0.02 * *x).max(0.05) => *n += 1,
            _ => out.push((round(r), 1)),
        }
    }
    out
}

/// Wall thickness from a sample of triangles: a ray from each, straight in, to the far side. The
/// area under [`MIN_WALL`] (scaled up by the sampling) and the thinnest wall.
fn walls(m: &Mesh, tris: &[[Vec3; 3]], tol: f64) -> (f64, f64) {
    let index = MeshIndex::new(m);
    let step = (tris.len() / WALL_SAMPLES).max(1);
    // From the chord tolerance in, so a facet's sag never reads as a wall.
    let lift = tol.max(1e-6);
    let (mut thin, mut least) = (0.0, f64::INFINITY);
    for [a, b, c] in tris.iter().step_by(step) {
        let cr = (*b - *a).cross(*c - *a);
        let Some(n) = cr.normalized() else { continue };
        let o = (*a + *b + *c) / 3.0 - n * lift;
        if let Some((t, _)) = index.raycast(m, o, -n) {
            let wall = t + lift;
            least = least.min(wall);
            if wall < MIN_WALL {
                thin += cr.len() / 2.0 * step as f64;
            }
        }
    }
    (thin, least)
}

/// Holes (round faces whose normals turn toward each other) within nominal … nominal + 0.2 mm of
/// a metric screw, untapped (no inside thread on their axis): (diameter, screw, how many holes).
fn tight_holes(b: &ModelBody, m: &Mesh, nf: usize, planar: &[Option<Vec3>], taps: &[(Vec3, Vec3)]) -> Vec<(f64, f64, usize)> {
    let mut corners: Vec<Vec<u32>> = vec![Vec::new(); nf];
    for (t, f) in m.triangles.iter().zip(&m.tri_face) {
        if planar.get(*f as usize).copied().flatten().is_none() {
            corners.get_mut(*f as usize).into_iter().for_each(|c| c.extend_from_slice(t));
        }
    }
    // By size, the holes' axes (a hole in two halves is one hole).
    let mut found: BTreeMap<(i64, i64), BTreeSet<[i64; 3]>> = BTreeMap::new();
    for (f, vs) in corners.iter().enumerate().filter(|(_, v)| !v.is_empty()) {
        let Some(solidcraft_kernel::FaceSurface::Cylinder { origin, axis, radius }) = solidcraft_kernel::face_surface(&b.body, f) else { continue };
        let at = |v: &u32| Some((*m.positions.get(*v as usize)?, *m.normals.get(*v as usize)?));
        let Some((p, np)) = vs.first().and_then(at) else { continue };
        let far = vs.iter().filter_map(at).max_by(|x, y| x.0.dist(p).total_cmp(&y.0.dist(p)));
        let inward = far.is_some_and(|(q, nq)| (q - p).dot(nq - np) < 0.0);
        let (a, d) = (*axis, 2.0 * radius);
        let off = |q: Vec3| (q - origin) - a * (q - origin).dot(a);
        let tapped = taps.iter().any(|(q, dir)| dir.cross(a).len() < 1e-3 && off(*q).len() < 0.05);
        if let Some(screw) = SCREWS.iter().find(|s| inward && !tapped && d >= **s - 0.05 && d < **s + 0.2) {
            let foot = origin - a * origin.dot(a);
            found
                .entry(((d * 100.0).round() as i64, (screw * 10.0) as i64))
                .or_default()
                .insert([foot.x, foot.y, foot.z].map(|x| (x * 100.0).round() as i64));
        }
    }
    found.into_iter().map(|((d, s), axes)| (d as f64 / 100.0, s as f64 / 10.0, axes.len())).collect()
}

/// The design's colours: how many judged bodies show each, and the pairs near enough to be one
/// colour meant twice.
struct Palette {
    used: BTreeMap<[u8; 3], usize>,
    near: Vec<([u8; 3], [u8; 3])>,
}

/// Colours closer than this (Euclidean, 0–255 per channel) read as one.
const NEAR_COLOUR: f64 = 30.0;

impl Palette {
    fn of(looks: &BTreeMap<String, (Option<[u8; 3]>, bool)>, bodies: &[Report]) -> Palette {
        let mut used: BTreeMap<[u8; 3], usize> = BTreeMap::new();
        for r in bodies {
            if let Some((Some(c), _)) = r.json["body"].as_str().and_then(|n| looks.get(n)) {
                *used.entry(*c).or_default() += 1;
            }
        }
        let colours: Vec<[u8; 3]> = used.keys().copied().collect();
        let mut near = Vec::new();
        for (i, a) in colours.iter().enumerate() {
            for b in colours.iter().skip(i + 1) {
                let d: f64 = a.iter().zip(b).map(|(x, y)| (f64::from(*x) - f64::from(*y)).powi(2)).sum::<f64>().sqrt();
                if d < NEAR_COLOUR {
                    near.push((*a, *b));
                }
            }
        }
        Palette { used, near }
    }

    fn json(&self) -> Value {
        json!({"palette": self.used.iter().map(|(c, n)| json!({"color": hex(*c), "bodies": n})).collect::<Vec<_>>(),
            "near_duplicates": self.near.iter().map(|(a, b)| json!([hex(*a), hex(*b)])).collect::<Vec<_>>()})
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn to8(c: [f32; 3]) -> [u8; 3] {
    c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn default_name(n: &str) -> bool {
    ["Body", "Component", "Solid"].iter().any(|p| n.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit())))
}

/// Names, materials and appearances.
fn tidy(s: &Session, r: &mut Report, looks: &BTreeMap<String, (Option<[u8; 3]>, bool)>, palette: &Palette) {
    let def = r.json["definition"].as_str().unwrap_or_default().to_string();
    let name = r.json["body"].as_str().unwrap_or_default().to_string();
    let mut score = 100.0;
    let mut notes = Vec::new();
    if default_name(&def) {
        score -= 10.0;
        notes.push(("default_name", format!("`{def}` has a default name: rename it for what it is")));
    }
    if !s.doc.materials.is_empty() && !s.doc.materials.contains_key(&def) {
        score -= 10.0;
        notes.push(("no_material", format!("`{def}` has no material while others do (material.assign)")));
    }
    // Colour blocking: one deliberate colour per part, from the design's palette.
    let (colour, several) = looks.get(&name).copied().unwrap_or((None, false));
    let printed = r.json["printed"] == true;
    match colour {
        None => {
            score -= 5.0;
            let why = if palette.used.is_empty() { "give each part a deliberate colour" } else { "give it a palette colour" };
            notes.push(("default_colour", format!("`{def}` shows the default colour: {why} (appearance.assign)")));
        }
        Some(c) => {
            if let Some(other) = palette.near.iter().find_map(|(a, b)| {
                if *a == c {
                    Some(*b)
                } else if *b == c {
                    Some(*a)
                } else {
                    None
                }
            }) {
                score -= 5.0;
                notes.push(("near_duplicate_colour", format!("`{def}` is {} and another part {}: near the same, use one", hex(c), hex(other))));
            } else if printed && palette.used.get(&c) == Some(&1) && palette.used.values().any(|n| *n > 1) {
                score -= 5.0;
                notes.push(("off_palette", format!("`{def}` is the only part in {}: print it in a palette colour", hex(c))));
            }
        }
    }
    if several {
        score -= 5.0;
        notes.push(("several_colours", format!("`{def}`'s faces carry other colours: keep one colour per part, colour by parts")));
    }
    r.scores.insert("tidy", score);
    if let Some(list) = r.json["findings"].as_array_mut() {
        list.extend(notes.into_iter().map(|(kind, m)| json!({"category": "tidy", "severity": "low", "kind": kind, "message": m})));
    }
}

/// The design's scores, the bodies worst first and the fixes by impact.
fn design(s: &Session, bodies: &mut [Report], max_fixes: usize) -> Value {
    let mut cats: BTreeMap<&'static str, (f64, f64)> = BTreeMap::new();
    for r in bodies.iter_mut() {
        let score = overall(&r.scores);
        r.json["score"] = json!(round(score));
        r.json["scores"] = json!(r.scores.iter().map(|(k, v)| (k.to_string(), round(*v))).collect::<BTreeMap<_, _>>());
        let w = r.area.max(1.0);
        for (k, v) in &r.scores {
            let c = cats.entry(k).or_default();
            *c = (c.0 + v * w, c.1 + w);
        }
    }
    let held: BTreeSet<u64> = s.doc.assembly.joints.iter().filter(|j| !j.suppressed).flat_map(|j| [j.a.occurrence, j.b.occurrence]).collect();
    let loose: Vec<Value> = s
        .doc
        .occurrences
        .iter()
        .filter(|o| !o.grounded && !held.contains(&o.id))
        .map(|o| json!({"category": "assembly", "severity": "low", "kind": "not_held", "occurrence": o.name, "message": format!("`{}` is neither grounded nor jointed", o.name)}))
        .collect();
    let mut scores: BTreeMap<&'static str, f64> = cats.into_iter().map(|(k, (sum, w))| (k, sum / w)).collect();
    if !s.doc.occurrences.is_empty() {
        scores.insert("assembly", (100.0 - (10.0 * loose.len() as f64).min(60.0)).max(0.0));
    }
    // Fixes: one per body, kind and radius, by edge length weighed for what it does to the look.
    // An edge of a part placed several times is fixed once, at its first placement.
    let mut fixes: Vec<(f64, Value)> = Vec::new();
    let mut seen = BTreeSet::new();
    for r in bodies.iter() {
        let (body, def) = (r.json["body"].as_str().unwrap_or_default(), r.json["definition"].as_str().unwrap_or_default());
        for (kind, size, edges) in &r.fixes {
            let unique: Vec<&(Vec3, f64, Vec3)> =
                edges.iter().filter(|(_, _, q)| seen.insert((def.to_string(), *kind, [q.x, q.y, q.z].map(|x| (x * 1e3).round() as i64)))).collect();
            if unique.is_empty() {
                continue;
            }
            // A command takes at most MAX_FIX_EDGES edges: more make several fixes.
            for part in unique.chunks(MAX_FIX_EDGES) {
                let length: f64 = part.iter().map(|(_, l, _)| l).sum();
                let points: Vec<[f64; 3]> = part.iter().map(|(p, _, _)| [p.x, p.y, p.z].map(|x| (x * 1e4).round() / 1e4)).collect();
                let (category, weight, command, params) = match *kind {
                    "chamfer" => ("print", 0.4, "solid.chamfer", json!({"body": body, "edges": points, "distance": size})),
                    "inside" => ("strength", 0.6, "solid.fillet", json!({"body": body, "edges": points, "radius": size, "fit": "reduce"})),
                    "bevel" => ("look", 1.0, "solid.chamfer", json!({"body": body, "edges": points, "distance": size})),
                    _ => ("look", 1.0, "solid.fillet", json!({"body": body, "edges": points, "radius": size, "fit": "reduce"})),
                };
                let kind = if matches!(*kind, "chamfer" | "bevel") { "chamfer" } else { "fillet" };
                fixes.push((
                    length * weight,
                    json!({"category": category, "kind": kind, "safe": true, "body": body, "edges": points.len(), "length_mm": round(length),
                    "impact": round(length * weight), "command": command, "params": params}),
                ));
            }
        }
    }
    fixes.sort_by(|(a, x), (b, y)| {
        b.total_cmp(a).then_with(|| x["body"].as_str().cmp(&y["body"].as_str())).then_with(|| x["params"].to_string().cmp(&y["params"].to_string()))
    });
    let fixes: Vec<Value> = fixes
        .into_iter()
        .take(max_fixes)
        .enumerate()
        .map(|(i, (_, mut f))| {
            f["id"] = json!(format!("F{}", i + 1));
            f
        })
        .collect();
    bodies.sort_by(|a, b| {
        a.json["score"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&b.json["score"].as_f64().unwrap_or(0.0))
            .then_with(|| a.json["body"].as_str().cmp(&b.json["body"].as_str()))
    });
    json!({
        "score": round(overall(&scores)),
        "scores": scores.iter().map(|(k, v)| (k.to_string(), round(*v))).collect::<BTreeMap<_, _>>(),
        "bodies": bodies.iter().map(|r| r.json.clone()).collect::<Vec<_>>(),
        "assembly": loose,
        "fixes": fixes,
        "not_checked": ["load paths next to joints, horns and bosses (inside corners are listed without them)", "fasteners without a seat", "walls between sampled points"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_without_a_mesh_is_skipped_not_scored() {
        let cube = solidcraft_kernel::box_solid(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0)).and_then(|b| b.tessellate(0.01)).unwrap();
        let meshes = vec![std::sync::Arc::new(cube), std::sync::Arc::new(Mesh::default())];
        let mut todo = vec![0, 1];
        let skipped = unmeshed(&mut todo, &meshes, &["Good", "Broken"]);
        assert_eq!(todo, vec![0]);
        assert_eq!(skipped, vec![json!({"body": "Broken", "why": "no mesh (tessellation failed)"})]);
    }

    #[test]
    fn a_fix_over_the_edge_limit_is_split_with_its_own_lengths() {
        let edges: Vec<(Vec3, f64, Vec3)> = (0..450).map(|i| (Vec3::new(i as f64, 0.0, 0.0), 2.0, Vec3::new(i as f64, 0.0, 0.0))).collect();
        let report = Report {
            json: json!({"body": "Plate", "definition": "Plate"}),
            scores: BTreeMap::new(),
            area: 1.0,
            treatment: (0, 0, 0.0),
            fixes: vec![("fillet", 1.0, edges)],
        };
        let r = design(&Session::default(), &mut [report], 10);
        let fixes = r["fixes"].as_array().unwrap();
        let sizes: Vec<(u64, f64)> = fixes.iter().map(|f| (f["edges"].as_u64().unwrap(), f["length_mm"].as_f64().unwrap())).collect();
        assert_eq!(sizes, vec![(200, 400.0), (200, 400.0), (50, 100.0)], "{r}");
        for f in fixes {
            assert_eq!(f["params"]["edges"].as_array().unwrap().len() as u64, f["edges"].as_u64().unwrap());
        }
    }
}

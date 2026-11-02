//! Does the design match the concept (#193): a reference body (an inserted concept mesh, say)
//! against the design's visible bodies, seen from several views. Per view, the silhouettes'
//! intersection over union, the regions where the design sticks out past the reference or falls
//! short of it by more than a tolerance, each named by the design part nearest it, and, when the
//! reference has colours, how much of the overlap is the same colour. `inspect.fit_reference`
//! scales and places the reference on the design first: uniform scale and translation, by
//! bounding box or by principal axes.

use std::collections::{BTreeMap, VecDeque};

use serde_json::{Value, json};
use solidcraft_doc::ModelBody;
use solidcraft_geom::Vec3;

use super::CommandSpec;
use crate::params::{bad, bool_, num, str_};
use crate::render::{Camera, StandardView};
use crate::{Result, Session};

const CMD: &str = "inspect.compare_reference";
const FIT: &str = "inspect.fit_reference";
const DEFAULT_VIEWS: [&str; 4] = ["left", "front", "top", "iso"];
/// Regions listed per view, largest first.
const REGIONS: usize = 6;
/// Colours closer than this (0–255 per channel, Euclidean) are the same colour.
const SAME_COLOUR: f64 = 60.0;
const MAX_PIXELS: usize = 1 << 20;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new(CMD, "Compare to Reference", compare).noundo().icon("compare").params(
        "reference: body name | [names] (a component's name takes its bodies; an inserted concept mesh, say, scaled and placed \
         on the design first: inspect.fit_reference); bodies?: [names] (default every visible body but the reference); views?: \
         [left|right|front|back|top|bottom|iso] (default left, front, top, iso: the three-quarter view); tolerance_mm?: how far the \
         design may stick out of the reference or fall short of it before a region is listed (default 2); rows?: pixels across \
         the view's height (default 200, 32…600) → score (0–100: the views' mean IoU × 100), views: [{view, iou, design_mm2, \
         reference_mm2, overlap_mm2, mm_per_px, regions: [{kind: sticks_out|short, body (the design part nearest it), area_mm2, \
         max_mm, at: [x,y,z] (its middle, at the design's depth), toward (the world direction from that part's middle), \
         message}], colour?: {match (the share of the overlap where the two are within 60 RGB of each other), mismatches: \
         [{body, design, reference, area_mm2}]}}], colour_score? (0–100, when the reference has colours). Silhouettes, parallel \
         projection; colours are the bodies' (a 3MF's base materials per object)",
    ),
    CommandSpec::new(FIT, "Fit Reference", fit).icon("scale").params(
        "reference: body name | [names] (as inspect.compare_reference); bodies?: [names] (the design to fit it to, default every \
         visible body but the reference); method?: bbox|principal (default bbox: the bounding boxes' diagonals and centres; \
         principal: the spread about the area-weighted centroids, and the principal axes, reported to turn by hand); apply?: \
         bool (default false: only report) → turn? ({axis, angle_deg: 90, residual_unturned}: a quarter turn first, when the \
         reference was modelled with another axis up and turning it matches the proportions better), scale, translate, origin \
         (what the scale is about, after any turn), residual: per-axis extent ratios left after the uniform scale (1 = matches), axes? (principal: each design axis with the reference's nearest and the angle \
         between them), steps: [{command, params}] (any turn about the world origin, solid.scale about origin, then solid.move), \
         applied",
    ),
];

/// The world bodies a reference names: bodies by name, or a component's.
fn reference_bodies<'a>(s: &Session, st: &'a solidcraft_doc::ModelState, p: &Value, cmd: &str) -> Result<Vec<&'a ModelBody>> {
    let names: Vec<String> = match p.get("reference") {
        Some(Value::String(n)) => vec![n.clone()],
        Some(Value::Array(a)) if !a.is_empty() && a.len() <= 10_000 => {
            a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(cmd, "`reference` must name bodies"))).collect::<Result<_>>()?
        }
        _ => return Err(bad(cmd, "`reference` is required: a body name or a list of them")),
    };
    let mut out: Vec<&ModelBody> = Vec::new();
    for n in &names {
        let found: Vec<&ModelBody> = match st.body(n) {
            Some(b) => vec![b],
            None => {
                let comp = s.doc.find_component(n).ok_or_else(|| bad(cmd, format!("no body or component `{n}`")))?;
                st.bodies.iter().filter(|b| b.component == comp).collect()
            }
        };
        for b in found {
            if !out.iter().any(|o| o.name == b.name) {
                out.push(b);
            }
        }
    }
    if out.is_empty() {
        return Err(bad(cmd, "the reference has no bodies"));
    }
    Ok(out)
}

/// The design's bodies: those named, else every visible body but the reference.
fn design_bodies<'a>(s: &Session, st: &'a solidcraft_doc::ModelState, p: &Value, reference: &[&ModelBody], cmd: &str) -> Result<Vec<&'a ModelBody>> {
    let asked: Option<Vec<String>> = match p.get("bodies").filter(|v| !v.is_null()) {
        None => None,
        Some(Value::Array(a)) => {
            Some(a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(cmd, "`bodies` must name bodies"))).collect::<Result<_>>()?)
        }
        Some(_) => return Err(bad(cmd, "`bodies` must be a list of names")),
    };
    if let Some(missing) = asked.iter().flatten().find(|n| st.body(n).is_none()) {
        return Err(bad(cmd, format!("no body `{missing}`")));
    }
    let out: Vec<&ModelBody> = st
        .bodies
        .iter()
        .filter(|b| !reference.iter().any(|r| r.name == b.name))
        .filter(|b| asked.as_ref().map_or_else(|| s.body_visible(&b.name), |a| a.contains(&b.name)))
        .collect();
    if out.is_empty() {
        return Err(bad(cmd, "there is no design to compare: name `bodies`"));
    }
    Ok(out)
}

/// One view's rasters: per pixel, the nearest body's index (u32::MAX: none), for the design and
/// the reference, on one grid.
struct Raster {
    width: usize,
    height: usize,
    mm_per_px: f64,
    lo: [f64; 2],
    hi: [f64; 2],
    design: Vec<u32>,
    reference: Vec<u32>,
}

fn raster(design: &[&ModelBody], reference: &[&ModelBody], right: Vec3, up: Vec3, back: Vec3, rows: usize) -> Result<Raster> {
    let project = |p: Vec3| [p.dot(right), p.dot(up), p.dot(back)];
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let meshes: Vec<(bool, u32, std::sync::Arc<solidcraft_geom::Mesh>)> = design
        .iter()
        .enumerate()
        .map(|(i, b)| (true, i as u32, b.mesh()))
        .chain(reference.iter().enumerate().map(|(i, b)| (false, i as u32, b.mesh())))
        .collect();
    for (_, _, m) in &meshes {
        for p in &m.positions {
            let q = project(*p);
            for k in 0..2 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
    }
    let (us, vs) = (hi[0] - lo[0], hi[1] - lo[1]);
    if !(us.is_finite() && vs.is_finite()) || (us <= 0.0 && vs <= 0.0) {
        return Err(bad(CMD, "nothing to compare in this view"));
    }
    let mm_per_px = (vs / rows as f64).max(us / (4 * rows) as f64).max(1e-6);
    let width = ((us / mm_per_px).ceil() as usize).max(1);
    let height = ((vs / mm_per_px).ceil() as usize).max(1);
    let pixels = width.checked_mul(height).filter(|n| *n <= MAX_PIXELS).ok_or_else(|| bad(CMD, "the view exceeds the pixel limit"))?;
    let mut owner = [vec![u32::MAX; pixels], vec![u32::MAX; pixels]];
    let mut depth = [vec![f64::NEG_INFINITY; pixels], vec![f64::NEG_INFINITY; pixels]];
    let edge = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    for (is_design, index, m) in &meshes {
        let side = usize::from(!*is_design);
        for t in &m.triangles {
            let Some(tri) = m.tri(t) else { continue };
            let q = tri.map(project);
            let px = q.map(|v| [(v[0] - lo[0]) / mm_per_px, (hi[1] - v[1]) / mm_per_px]);
            if px.iter().flatten().any(|v| !v.is_finite()) {
                continue;
            }
            let [a, b, c] = px;
            let area = edge(a, b, c);
            if area == 0.0 {
                continue;
            }
            let x0 = a[0].min(b[0]).min(c[0]).floor().clamp(0.0, width as f64) as usize;
            let x1 = a[0].max(b[0]).max(c[0]).ceil().clamp(0.0, width as f64) as usize;
            let y0 = a[1].min(b[1]).min(c[1]).floor().clamp(0.0, height as f64) as usize;
            let y1 = a[1].max(b[1]).max(c[1]).ceil().clamp(0.0, height as f64) as usize;
            let z = (q[0][2] + q[1][2] + q[2][2]) / 3.0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let centre = [x as f64 + 0.5, y as f64 + 0.5];
                    let e = [edge(a, b, centre), edge(b, c, centre), edge(c, a, centre)];
                    if !(e.iter().all(|v| *v >= -1e-9) || e.iter().all(|v| *v <= 1e-9)) {
                        continue;
                    }
                    let i = y * width + x;
                    if let (Some(d), Some(o)) = (depth[side].get_mut(i), owner[side].get_mut(i))
                        && z > *d
                    {
                        *d = z;
                        *o = *index;
                    }
                }
            }
        }
    }
    let [design, reference] = owner;
    Ok(Raster { width, height, mm_per_px, lo, hi, design, reference })
}

/// Per pixel, the distance (pixels) to the nearest pixel of `mask`, and that pixel's label: a
/// two-pass chamfer transform (3-4 weights) carrying labels along.
fn nearest(width: usize, height: usize, labels: &[u32]) -> (Vec<f64>, Vec<u32>) {
    let mut dist: Vec<f64> = labels.iter().map(|l| if *l == u32::MAX { f64::INFINITY } else { 0.0 }).collect();
    let mut label = labels.to_vec();
    let relax = |i: usize, j: usize, w: f64, dist: &mut Vec<f64>, label: &mut Vec<u32>| {
        if let (Some(&dj), Some(&lj)) = (dist.get(j), label.get(j))
            && let Some(di) = dist.get_mut(i)
            && dj + w < *di
        {
            *di = dj + w;
            if let Some(li) = label.get_mut(i) {
                *li = lj;
            }
        }
    };
    let (a, d) = (1.0, std::f64::consts::SQRT_2);
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if x > 0 {
                relax(i, i - 1, a, &mut dist, &mut label);
            }
            if y > 0 {
                relax(i, i - width, a, &mut dist, &mut label);
                if x > 0 {
                    relax(i, i - width - 1, d, &mut dist, &mut label);
                }
                if x + 1 < width {
                    relax(i, i - width + 1, d, &mut dist, &mut label);
                }
            }
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let i = y * width + x;
            if x + 1 < width {
                relax(i, i + 1, a, &mut dist, &mut label);
            }
            if y + 1 < height {
                relax(i, i + width, a, &mut dist, &mut label);
                if x + 1 < width {
                    relax(i, i + width + 1, d, &mut dist, &mut label);
                }
                if x > 0 {
                    relax(i, i + width - 1, d, &mut dist, &mut label);
                }
            }
        }
    }
    (dist, label)
}

/// The world direction a vector points most along, as `+x`, `-z`…
fn toward(v: Vec3) -> &'static str {
    let a = [v.x.abs(), v.y.abs(), v.z.abs()];
    match (a[0] >= a[1] && a[0] >= a[2], a[1] >= a[2]) {
        (true, _) => {
            if v.x >= 0.0 {
                "+x"
            } else {
                "-x"
            }
        }
        (false, true) => {
            if v.y >= 0.0 {
                "+y"
            } else {
                "-y"
            }
        }
        (false, false) => {
            if v.z >= 0.0 {
                "+z"
            } else {
                "-z"
            }
        }
    }
}

fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0 + 0.0
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn compare(s: &mut Session, p: &Value) -> Result<Value> {
    let tolerance = match p.get("tolerance_mm").filter(|v| !v.is_null()) {
        None => 2.0,
        Some(_) => num(p, "tolerance_mm")
            .filter(|v| v.is_finite() && (0.0..=1000.0).contains(v))
            .ok_or_else(|| bad(CMD, "`tolerance_mm` must be 0 to 1000"))?,
    };
    let rows = match p.get("rows").filter(|v| !v.is_null()) {
        None => 200,
        Some(_) => num(p, "rows").filter(|v| (32.0..=600.0).contains(v)).ok_or_else(|| bad(CMD, "`rows` must be 32 to 600"))? as usize,
    };
    let views: Vec<String> = match p.get("views").filter(|v| !v.is_null()) {
        None => DEFAULT_VIEWS.iter().map(|v| v.to_string()).collect(),
        Some(Value::Array(a)) if !a.is_empty() && a.len() <= 7 => {
            a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(CMD, "`views` must name views"))).collect::<Result<_>>()?
        }
        Some(_) => return Err(bad(CMD, "`views` must list 1 to 7 views")),
    };
    let st = s.world_state();
    let reference = reference_bodies(s, &st, p, CMD)?;
    let design = design_bodies(s, &st, p, &reference, CMD)?;
    let to8 = |c: [f32; 3]| c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8);
    let design_colour: Vec<Option<[u8; 3]>> = design.iter().map(|b| s.doc.body_color(b).map(to8)).collect();
    let reference_colour: Vec<Option<[u8; 3]>> = reference.iter().map(|b| s.doc.body_color(b).map(to8)).collect();
    let coloured = reference_colour.iter().any(Option::is_some);
    // Each design body's middle, to say which way a region lies from it.
    let middle: Vec<Vec3> = design.iter().map(|b| b.mesh().bounds().center()).collect();
    let design_box = design.iter().map(|b| b.mesh().bounds()).fold(solidcraft_geom::Aabb3::EMPTY, |a, b| a.union(&b));
    let mut out = Vec::new();
    let (mut iou_sum, mut colour_sum, mut colour_views) = (0.0, 0.0, 0usize);
    for name in &views {
        let view = StandardView::parse(name).ok_or_else(|| bad(CMD, format!("unknown view `{name}`")))?;
        let (yaw, pitch) = view.angles();
        let camera = Camera { target: Vec3::ZERO, yaw, pitch, distance: 200.0, fov: 0.0, ..Default::default() };
        let (right, up, back) = camera.basis();
        let r = raster(&design, &reference, right, up, back, rows)?;
        let px_area = r.mm_per_px * r.mm_per_px;
        let on = |v: &[u32], i: usize| v.get(i).is_some_and(|o| *o != u32::MAX);
        let n = r.width * r.height;
        let (mut both, mut either, mut d_count, mut r_count) = (0usize, 0usize, 0usize, 0usize);
        for i in 0..n {
            let (d, f) = (on(&r.design, i), on(&r.reference, i));
            both += usize::from(d && f);
            either += usize::from(d || f);
            d_count += usize::from(d);
            r_count += usize::from(f);
        }
        let iou = if either > 0 { both as f64 / either as f64 } else { 0.0 };
        iou_sum += iou;
        // Regions past the tolerance: the design outside the reference, the reference outside
        // the design, each by its distance from the other.
        let (to_ref, _) = nearest(r.width, r.height, &r.reference);
        let (to_design, nearest_part) = nearest(r.width, r.height, &r.design);
        let limit = tolerance / r.mm_per_px;
        let flagged: Vec<u8> = (0..n)
            .map(|i| {
                if on(&r.design, i) && !on(&r.reference, i) && to_ref.get(i).is_some_and(|d| *d > limit) {
                    1
                } else if on(&r.reference, i) && !on(&r.design, i) && to_design.get(i).is_some_and(|d| *d > limit) {
                    2
                } else {
                    0
                }
            })
            .collect();
        let mut seen = vec![false; n];
        let mut regions: Vec<(usize, Value)> = Vec::new();
        for start in 0..n {
            let kind = flagged.get(start).copied().unwrap_or(0);
            if kind == 0 || seen.get(start).copied().unwrap_or(true) {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            if let Some(x) = seen.get_mut(start) {
                *x = true;
            }
            let (mut count, mut sum_u, mut sum_v, mut max_d) = (0usize, 0.0, 0.0, 0.0f64);
            let mut votes: BTreeMap<u32, usize> = BTreeMap::new();
            while let Some(i) = queue.pop_front() {
                let (x, y) = (i % r.width, i / r.width);
                count += 1;
                sum_u += x as f64 + 0.5;
                sum_v += y as f64 + 0.5;
                let (d, part) = if kind == 1 {
                    (to_ref.get(i).copied().unwrap_or(0.0), r.design.get(i).copied().unwrap_or(u32::MAX))
                } else {
                    (to_design.get(i).copied().unwrap_or(0.0), nearest_part.get(i).copied().unwrap_or(u32::MAX))
                };
                max_d = max_d.max(d);
                *votes.entry(part).or_default() += 1;
                let neighbours =
                    [(x > 0).then(|| i - 1), (x + 1 < r.width).then(|| i + 1), (y > 0).then(|| i - r.width), (y + 1 < r.height).then(|| i + r.width)];
                for j in neighbours.into_iter().flatten() {
                    if flagged.get(j) == Some(&kind) && seen.get(j) == Some(&false) {
                        if let Some(x) = seen.get_mut(j) {
                            *x = true;
                        }
                        queue.push_back(j);
                    }
                }
            }
            let part = votes.iter().max_by_key(|(_, c)| **c).map(|(p, _)| *p).unwrap_or(u32::MAX);
            let body = design.get(part as usize);
            let (u, v) = (r.lo[0] + sum_u / count as f64 * r.mm_per_px, r.hi[1] - sum_v / count as f64 * r.mm_per_px);
            let at = right * u + up * v + back * design_box.center().dot(back);
            let from = middle.get(part as usize).copied().unwrap_or(design_box.center());
            let dir = toward((at - from) - back * (at - from).dot(back));
            let max_mm = max_d * r.mm_per_px;
            let name = body.map_or("the design", |b| b.name.as_str());
            let seen_from = view_name(view);
            let message = if kind == 1 {
                format!("{name} sticks out {max_mm:.0} mm past the reference toward {dir} ({seen_from} view)")
            } else {
                format!("{name} is {max_mm:.0} mm short of the reference toward {dir} ({seen_from} view)")
            };
            regions.push((
                count,
                json!({"kind": if kind == 1 { "sticks_out" } else { "short" }, "body": body.map(|b| b.name.clone()), "area_mm2": round(count as f64 * px_area),
                    "max_mm": round(max_mm), "at": [round(at.x), round(at.y), round(at.z)], "toward": dir, "message": message}),
            ));
        }
        regions.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1["message"].as_str().cmp(&b.1["message"].as_str())));
        let mut v = json!({"view": name, "iou": round(iou), "design_mm2": round(d_count as f64 * px_area), "reference_mm2": round(r_count as f64 * px_area),
            "overlap_mm2": round(both as f64 * px_area), "mm_per_px": round(r.mm_per_px * 1000.0) / 1000.0,
            "regions": regions.into_iter().take(REGIONS).map(|(_, v)| v).collect::<Vec<_>>()});
        if coloured {
            // Where both show, are they the same colour?
            let (mut compared, mut same) = (0usize, 0usize);
            let mut off: BTreeMap<(u32, [u8; 3], [u8; 3]), usize> = BTreeMap::new();
            for i in 0..n {
                let (Some(&d), Some(&f)) = (r.design.get(i), r.reference.get(i)) else { continue };
                let (Some(Some(dc)), Some(Some(rc))) = (design_colour.get(d as usize), reference_colour.get(f as usize)) else { continue };
                compared += 1;
                let gap: f64 = dc.iter().zip(rc).map(|(a, b)| (f64::from(*a) - f64::from(*b)).powi(2)).sum::<f64>().sqrt();
                if gap <= SAME_COLOUR {
                    same += 1;
                } else {
                    *off.entry((d, *dc, *rc)).or_default() += 1;
                }
            }
            let share = if compared > 0 { same as f64 / compared as f64 } else { 0.0 };
            colour_sum += share;
            colour_views += 1;
            let mut mismatches: Vec<(usize, Value)> = off
                .into_iter()
                .map(|((d, dc, rc), c)| (c, json!({"body": design.get(d as usize).map(|b| b.name.clone()), "design": hex(dc), "reference": hex(rc), "area_mm2": round(c as f64 * px_area)})))
                .collect();
            mismatches.sort_by_key(|m| std::cmp::Reverse(m.0));
            v["colour"] = json!({"match": round(share), "mismatches": mismatches.into_iter().take(REGIONS).map(|(_, v)| v).collect::<Vec<_>>()});
        }
        out.push(v);
    }
    let mut reply = json!({"score": round(100.0 * iou_sum / views.len() as f64), "views": out, "tolerance_mm": tolerance,
        "reference": reference.iter().map(|b| b.name.clone()).collect::<Vec<_>>(), "bodies": design.len()});
    if colour_views > 0 {
        reply["colour_score"] = json!(round(100.0 * colour_sum / colour_views as f64));
    }
    Ok(reply)
}

fn view_name(view: StandardView) -> &'static str {
    match view {
        StandardView::Front => "front",
        StandardView::Back => "back",
        StandardView::Top => "top",
        StandardView::Bottom => "bottom",
        StandardView::Left => "left",
        StandardView::Right => "right",
        StandardView::Iso => "three-quarter",
    }
}

/// Area-weighted centroid and spread (covariance) of bodies' surfaces.
fn moments(bodies: &[&ModelBody]) -> Option<(Vec3, [[f64; 3]; 3])> {
    let mut total = 0.0;
    let mut sum = Vec3::ZERO;
    let mut tris = Vec::new();
    for b in bodies {
        let m = b.mesh();
        for t in &m.triangles {
            let Some([a, c, d]) = m.tri(t) else { continue };
            let area = (c - a).cross(d - a).len() / 2.0;
            if area.is_finite() && area > 0.0 {
                let mid = (a + c + d) / 3.0;
                total += area;
                sum += mid * area;
                tris.push((mid, area));
            }
        }
    }
    if !(total > 0.0) {
        return None;
    }
    let c = sum / total;
    let mut cov = [[0.0; 3]; 3];
    for (mid, area) in tris {
        let d = [mid.x - c.x, mid.y - c.y, mid.z - c.z];
        for (i, row) in cov.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x += area * d[i] * d[j] / total;
            }
        }
    }
    Some((c, cov))
}

/// A symmetric 3×3 matrix's eigenvectors (columns), by Jacobi rotations, largest eigenvalue first.
fn principal(m: [[f64; 3]; 3]) -> [(f64, Vec3); 3] {
    let mut a = m;
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let (mut p, mut q, mut big) = (0, 1, 0.0);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if a[i][j].abs() > big {
                (p, q, big) = (i, j, a[i][j].abs());
            }
        }
        if big < 1e-12 {
            break;
        }
        let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (sn, cs) = theta.sin_cos();
        for row in &mut a {
            let (akp, akq) = (row[p], row[q]);
            row[p] = cs * akp - sn * akq;
            row[q] = sn * akp + cs * akq;
        }
        let (row_p, row_q) = (a[p], a[q]);
        a[p] = std::array::from_fn(|k| cs * row_p[k] - sn * row_q[k]);
        a[q] = std::array::from_fn(|k| sn * row_p[k] + cs * row_q[k]);
        for row in &mut v {
            let (vp, vq) = (row[p], row[q]);
            row[p] = cs * vp - sn * vq;
            row[q] = sn * vp + cs * vq;
        }
    }
    let mut out = [0, 1, 2].map(|k| (a[k][k], Vec3::new(v[0][k], v[1][k], v[2][k])));
    out.sort_by(|x, y| y.0.total_cmp(&x.0));
    out
}

fn fit(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.world_state();
    let reference = reference_bodies(s, &st, p, FIT)?;
    let design = design_bodies(s, &st, p, &reference, FIT)?;
    let method = str_(p, "method").unwrap_or("bbox");
    let bounds = |bs: &[&ModelBody]| bs.iter().map(|b| b.mesh().bounds()).fold(solidcraft_geom::Aabb3::EMPTY, |a, b| a.union(&b));
    let (db, rb) = (bounds(&design), bounds(&reference));
    if db.is_empty() || rb.is_empty() {
        return Err(bad(FIT, "the design or the reference has no extent"));
    }
    let mut out = json!({"method": method});
    let (scale, origin, target) = match method {
        "bbox" => {
            let (dd, rd) = (db.diagonal(), rb.diagonal());
            if !(rd > 0.0) {
                return Err(bad(FIT, "the reference has no extent"));
            }
            (dd / rd, rb.center(), db.center())
        }
        "principal" => {
            let (Some((dc, dcov)), Some((rc, rcov))) = (moments(&design), moments(&reference)) else {
                return Err(bad(FIT, "the design or the reference has no surface"));
            };
            let trace = |m: [[f64; 3]; 3]| m[0][0] + m[1][1] + m[2][2];
            if !(trace(rcov) > 0.0) {
                return Err(bad(FIT, "the reference has no extent"));
            }
            let (da, ra) = (principal(dcov), principal(rcov));
            out["axes"] = json!(da
                .iter()
                .map(|(_, d)| {
                    let (_, r) = ra.iter().max_by(|x, y| x.1.dot(*d).abs().total_cmp(&y.1.dot(*d).abs())).copied().unwrap_or((0.0, *d));
                    let angle = r.dot(*d).abs().clamp(-1.0, 1.0).acos().to_degrees();
                    json!({"design": [round(d.x), round(d.y), round(d.z)], "reference": [round(r.x), round(r.y), round(r.z)], "angle_deg": round(angle)})
                })
                .collect::<Vec<_>>());
            ((trace(dcov) / trace(rcov)).sqrt(), rc, dc)
        }
        other => return Err(bad(FIT, format!("`method` is bbox or principal, not `{other}`"))),
    };
    if !(scale.is_finite() && scale > 0.0) {
        return Err(bad(FIT, "no scale fits"));
    }
    // A concept modelled with another axis up: try a quarter turn about each axis and keep the
    // one whose proportions match best (the bounding boxes' per-axis ratios nearest 1).
    let (ds, rs) = (db.size(), rb.size());
    let turned = |axis: usize, v: Vec3| match axis {
        1 => Vec3::new(v.x, -v.z, v.y),
        2 => Vec3::new(v.z, v.y, -v.x),
        3 => Vec3::new(-v.y, v.x, v.z),
        _ => v,
    };
    let extents = |axis: usize| {
        let t = turned(axis, rs);
        Vec3::new(t.x.abs(), t.y.abs(), t.z.abs())
    };
    let ratios = |e: Vec3| [(ds.x, e.x), (ds.y, e.y), (ds.z, e.z)].map(|(d, r)| if r > 0.0 { d / (r * scale) } else { 0.0 });
    let badness = |axis: usize| ratios(extents(axis)).iter().filter(|r| **r > 0.0).map(|r| r.ln().abs()).sum::<f64>();
    let best = (0..4).min_by(|a, b| badness(*a).total_cmp(&badness(*b))).unwrap_or(0);
    let turn = (best != 0 && badness(best) + 0.2 < badness(0)).then_some(best);
    let origin = turn.map_or(origin, |t| turned(t, origin));
    let translate = target - origin;
    let r3 = |v: Vec3| [round(v.x), round(v.y), round(v.z)];
    let names: Vec<String> = reference.iter().map(|b| b.name.clone()).collect();
    let mut steps = Vec::new();
    if let Some(t) = turn {
        let axis = [[1, 0, 0], [0, 1, 0], [0, 0, 1]].get(t - 1).copied().unwrap_or([0, 0, 1]);
        steps.push(json!({"command": "solid.move", "params": {"bodies": names, "axis": axis, "angle": 90}}));
        let axis_name = ["x", "y", "z"].get(t - 1).copied();
        out["turn"] = json!({"axis": axis_name, "angle_deg": 90, "residual_unturned": ratios(rs).map(round)});
    }
    steps
        .push(json!({"command": "solid.scale", "params": {"bodies": names, "factor": round(scale * 10000.0) / 100.0 / 100.0, "origin": r3(origin)}}));
    steps.push(json!({"command": "solid.move", "params": {"bodies": names, "translate": r3(translate)}}));
    let steps = Value::Array(steps);
    let residual = ratios(extents(turn.unwrap_or(0))).map(round);
    out["scale"] = json!(round(scale * 10000.0) / 10000.0);
    out["translate"] = json!(r3(translate));
    out["origin"] = json!(r3(origin));
    out["residual"] = json!({"x": residual[0], "y": residual[1], "z": residual[2]});
    let mut applied = false;
    if bool_(p, "apply").unwrap_or(false) {
        for step in steps.as_array().into_iter().flatten() {
            let command = step["command"].as_str().unwrap_or_default();
            let params = crate::frames::to_local(s, command, &step["params"]).unwrap_or_else(|| step["params"].clone());
            s.execute(command, &params)?;
        }
        applied = true;
    }
    out["steps"] = steps;
    out["applied"] = json!(applied);
    Ok(out)
}

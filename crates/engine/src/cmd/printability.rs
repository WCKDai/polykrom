//! Can it be 3D-printed, and which way up (#121, the loop's "make" stage, L4 #111): for each body,
//! the overhang that would need support at a build direction, the support's footprint, the area
//! on the bed, whether it fits the build volume, the six axis directions ranked, and the
//! filament it takes. Measured on the body's tessellation in world placement, so an agent can
//! choose an orientation and say what a print costs before slicing it. Thin walls and bridges
//! are not judged yet; the reply says so.

use polycore_geom::Vec3;
use serde_json::{Value, json};

use super::CommandSpec;
use crate::params::{bad, num, string_list, vec3};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] =
    &[CommandSpec::new("inspect.printability", "Printability", printability).icon("draft_analysis").noundo().params(
        "bodies?: [names] (default every solid body); direction?: [x,y,z] the build direction, up from the bed (default +Z); \
     overhang_deg?: the steepest overhang printed without support, from vertical (default 45); bed?: [x,y,z] the build volume in mm \
     (default [250,250,250]); wall?: mm of perimeters and skins (default 1.2); infill?: 0–1 (default 0.2) → bodies: [{body, \
     volume_mm3, area_mm2, size_mm (across the bed, then up), fits_bed, bed_contact_mm2, overhang_mm2, support_projected_mm2 (the \
     overhang's footprint on the bed), filament_g, filament, orientations: [the same for ±X, ±Y, ±Z up], best}]; not_checked lists \
     what isn't judged (thin walls, bridges)",
    )];

/// The six ways a part can stand on the bed, its axes up.
pub(super) const AXES: [[f64; 3]; 6] = [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]];
/// Filaments by the material names Polykrom's library uses; anything else prints as PLA.
const FILAMENTS: [&str; 7] = ["PLA", "PETG", "ABS", "ASA", "Nylon", "TPU", "Polycarbonate"];
const PLA_DENSITY: f64 = 1.24;
const MAX_BODIES: usize = 2000;

/// One way up: the measures of one body's triangles with `up` as the build direction.
pub(super) struct Way {
    size: [f64; 3],
    pub(super) bed_contact: f64,
    pub(super) overhang: f64,
    pub(super) projected: f64,
}

fn printability(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "inspect.printability";
    let up = match p.get("direction").filter(|v| !v.is_null()) {
        None => Vec3::Z,
        Some(v) => vec3(v).and_then(|d| d.normalized()).ok_or_else(|| bad(cmd, "`direction` must be a non-zero [x,y,z]"))?,
    };
    let in_range = |k: &str, default: f64, lo: f64, hi: f64| -> Result<f64> {
        match p.get(k).filter(|v| !v.is_null()) {
            None => Ok(default),
            Some(_) => num(p, k).filter(|v| v.is_finite() && (lo..=hi).contains(v)).ok_or_else(|| bad(cmd, format!("`{k}` must be {lo} to {hi}"))),
        }
    };
    let limit = in_range("overhang_deg", 45.0, 0.0, 89.0)?;
    let wall = in_range("wall", 1.2, 0.0, 100.0)?;
    let infill = in_range("infill", 0.2, 0.0, 1.0)?;
    let bed = match p.get("bed").filter(|v| !v.is_null()) {
        None => [250.0, 250.0, 250.0],
        Some(v) => {
            let b = vec3(v).filter(|b| b.x > 0.0 && b.y > 0.0 && b.z > 0.0).ok_or_else(|| bad(cmd, "`bed` must be [x,y,z] in mm, each above 0"))?;
            [b.x, b.y, b.z]
        }
    };
    let st = s.world_state();
    let asked = string_list(p, "bodies");
    let bodies: Vec<_> = st.bodies.iter().filter(|b| !b.body.is_mesh() && (asked.is_empty() || asked.contains(&b.name))).collect();
    if let Some(missing) = asked.iter().find(|n| !bodies.iter().any(|b| &b.name == *n)) {
        return Err(bad(cmd, format!("no solid body `{missing}`")));
    }
    if bodies.len() > MAX_BODIES {
        return Err(bad(cmd, format!("at most {MAX_BODIES} bodies")));
    }
    let sin = limit.to_radians().sin();
    let out: Vec<Value> = bodies
        .iter()
        .map(|b| {
            let mesh = b.mesh();
            let tris: Vec<[Vec3; 3]> = mesh
                .triangles
                .iter()
                .filter_map(|t| Some([*mesh.positions.get(t[0] as usize)?, *mesh.positions.get(t[1] as usize)?, *mesh.positions.get(t[2] as usize)?]))
                .collect();
            // Volume by the divergence theorem, area by summing: the tessellation's own.
            let volume = tris.iter().map(|[a, b, c]| a.dot(b.cross(*c)) / 6.0).sum::<f64>().abs();
            let area: f64 = tris.iter().map(|[a, b, c]| (*b - *a).cross(*c - *a).len() / 2.0).sum();
            let way = |d: Vec3| measure(&tris, d, sin);
            let fits = |w: &Way| (w.size[0] <= bed[0] && w.size[1] <= bed[1] || w.size[0] <= bed[1] && w.size[1] <= bed[0]) && w.size[2] <= bed[2];
            let json_of = |d: Vec3, w: &Way| {
                json!({"direction": [d.x, d.y, d.z], "size_mm": w.size, "fits_bed": fits(w), "bed_contact_mm2": w.bed_contact,
                       "overhang_mm2": w.overhang, "support_projected_mm2": w.projected})
            };
            let ways: Vec<(Vec3, Way)> = AXES.iter().map(|a| Vec3::new(a[0], a[1], a[2])).map(|d| (d, way(d))).collect();
            // The best way up: fitting first, then the least support, then the most on the bed.
            let best = ways.iter().min_by(|(_, x), (_, y)| {
                (!fits(x)).cmp(&!fits(y)).then(x.projected.total_cmp(&y.projected)).then(y.bed_contact.total_cmp(&x.bed_contact))
            });
            let asked = way(up);
            // The filament: perimeters and skins over the whole surface, infill inside them.
            let material = s.doc.body_material(&b.name);
            let filament = FILAMENTS.iter().find(|f| material.to_ascii_lowercase().contains(&f.to_ascii_lowercase())).copied().unwrap_or("PLA");
            let density = if filament == "PLA" && !material.to_ascii_lowercase().contains("pla") { PLA_DENSITY } else { s.doc.density(&b.name) };
            let walls = (area * wall).min(volume);
            let grams = density * 1e-3 * (walls + infill * (volume - walls));
            let mut v = json_of(up, &asked);
            if let Some(o) = v.as_object_mut() {
                o.insert("body".into(), json!(b.name));
                o.insert("volume_mm3".into(), json!(volume));
                o.insert("area_mm2".into(), json!(area));
                o.insert("filament_g".into(), json!(grams));
                o.insert("filament".into(), json!(filament));
                o.insert("orientations".into(), json!(ways.iter().map(|(d, w)| json_of(*d, w)).collect::<Vec<_>>()));
                o.insert("best".into(), best.map(|(d, w)| json_of(*d, w)).unwrap_or(Value::Null));
            }
            v
        })
        .collect();
    Ok(json!({
        "bodies": out, "direction": [up.x, up.y, up.z], "overhang_deg": limit, "bed": bed, "wall": wall, "infill": infill,
        "not_checked": ["thin walls", "bridges", "trapped volumes"],
    }))
}

/// One body's triangles printed with `up` as the build direction: its size across the bed and up,
/// the area lying on the bed, and the area hanging over steeper than the limit (`sin` of it), with
/// that overhang's footprint on the bed.
pub(super) fn measure(tris: &[[Vec3; 3]], up: Vec3, sin: f64) -> Way {
    let u = up.cross(if up.x.abs() < 0.9 { Vec3::X } else { Vec3::Y }).normalized().unwrap_or(Vec3::X);
    let v = up.cross(u);
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in tris.iter().flatten() {
        for (k, axis) in [u, v, up].iter().enumerate() {
            let x = p.dot(*axis);
            lo[k] = lo[k].min(x);
            hi[k] = hi[k].max(x);
        }
    }
    let size = [(hi[0] - lo[0]).max(0.0), (hi[1] - lo[1]).max(0.0), (hi[2] - lo[2]).max(0.0)];
    // On the bed: within a hair of the lowest point, relative to the part's size.
    let hair = 1e-6 * size.iter().fold(1.0f64, |m, s| m.max(*s));
    let (mut bed_contact, mut overhang, mut projected) = (0.0, 0.0, 0.0);
    for [a, b, c] in tris {
        let cross = (*b - *a).cross(*c - *a);
        let twice = cross.len();
        if twice <= 0.0 {
            continue;
        }
        let down = -cross.dot(up) / twice;
        let on_bed = [a, b, c].iter().all(|p| p.dot(up) - lo[2] <= hair);
        if on_bed && down > 1.0 - 1e-9 {
            bed_contact += twice / 2.0;
        } else if down > sin + 1e-9 {
            overhang += twice / 2.0;
            projected += twice / 2.0 * down;
        }
    }
    Way { size, bed_contact, overhang, projected }
}

#[cfg(test)]
#[path = "printability_tests.rs"]
mod tests;

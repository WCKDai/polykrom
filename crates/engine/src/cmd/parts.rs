//! Standard parts: ISO metric fasteners, dowel pins and ball bearings, generated in code from
//! the dimensions in the public standards tables (no vendor geometry), and motion hardware
//! (T-slot extrusions, NEMA stepper motors, GT2 pulleys, miniature linear rails, T8 lead
//! screws) from the common dimensions their makers share (docs/parts-library-plan.md). Each part is inserted as
//! its own component, built by ordinary features (so it stays parametric and editable), with a
//! cosmetic thread where it has one; at a hole it gets a rigid joint that puts its axis on the
//! hole's and seats it on the face.
//!
//! Part frame: the seating plane is z = 0 (the underside of a head, the bottom of a nut or
//! washer, the top of a set screw or pin, a stepper's mounting face); the shank runs down −z (a
//! stepper's pilot and shaft too, its body up +z; a lead screw nut's body too). An extrusion, a
//! rail and a lead screw run up +z from z = 0; a rail's mounting face is y = 0, and its
//! carriage, inserted at the same point and direction, sits on it at z = 0. A pulley's bore is
//! along z, its hub from z = 0.

use serde_json::{Value, json};
use solidcraft_geom::Vec3;

use super::CommandSpec;
use crate::params::{bad, num, str_, vec3};
use crate::{EngineError, Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("parts.library", "Standard Parts", library).icon("library")
        .noundo()
        .params("family? → families (name, standard, sizes, lengths) and, with family, each size's dimensions (mm)"),
    CommandSpec::new("parts.fastener", "Insert Fastener", insert).at("SOLID", "INSERT").icon("fastener").params(
        "the Insert Part dialog's command (same as parts.insert): family, size, length?, at?|point?+direction?, name?",
    ),
    CommandSpec::new("parts.insert", "Insert Part", insert).icon("fastener").params(
        "family (socket_head_cap_screw | hex_bolt | hex_nut | washer | set_screw | dowel_pin | bearing | extrusion | stepper | gt2_pulley | \
         linear_rail | linear_carriage | lead_screw | lead_nut); size (\"M6\", \"608\" for bearings, \"2020\" for extrusions, \"NEMA17-40\", \"20T-5\", \
         \"MGN12\", \"MGN12H\", \"T8\": parts.library lists them); \
         length? (mm; screws, bolts, set screws, pins, extrusions, rails, lead screws; default a standard length near 2.5 d, 200 for the others); \
         at?: [x,y,z] on a hole's rim or wall (the part seats on the hole's face with a rigid joint) | point?: [x,y,z] with direction?: [x,y,z] (no joint); name?",
    ),
    CommandSpec::new("parts.resize", "Change Part Size", resize_atomic).icon("scale")
        .params("component: id|name of an inserted part; size?; length? — rebuilds it (joints keep it seated)"),
];

/// A family: (id, name, standard).
const FAMILIES: [(&str, &str, &str); 14] = [
    ("socket_head_cap_screw", "Socket head cap screw", "ISO 4762"),
    ("hex_bolt", "Hex head bolt / screw", "ISO 4014 / ISO 4017"),
    ("hex_nut", "Hex nut", "ISO 4032"),
    ("washer", "Plain washer", "ISO 7089"),
    ("set_screw", "Hex socket set screw, flat point", "ISO 4026"),
    ("dowel_pin", "Parallel pin (dowel)", "ISO 8734"),
    ("bearing", "Deep groove ball bearing (envelope)", "ISO 15 (6000/6200/600 series)"),
    ("extrusion", "Aluminium T-slot extrusion, 20 mm series (slot 6)", "series 5 / B-type"),
    ("stepper", "Hybrid stepper motor (body, pilot, D-shaft)", "NEMA ICS 16"),
    ("gt2_pulley", "GT2 timing pulley, 2 mm pitch, 6 mm belt", "GT2 (2 mm)"),
    ("linear_rail", "Miniature linear guide rail", "MGN series"),
    ("linear_carriage", "Miniature linear guide carriage (standard, H)", "MGN series"),
    ("lead_screw", "Trapezoidal lead screw, 8 mm lead (4 starts)", "T8x8"),
    ("lead_nut", "Flanged lead screw nut", "T8x8"),
];

/// GT2 pulleys: (size, teeth, bore). Pitch diameter = teeth · 2 mm / π; the outside diameter
/// is 0.254 mm in from it each side; the 6 mm belt runs on 7 mm of teeth between 1 mm flanges,
/// over a 7 mm hub.
const PULLEYS: [(&str, u32, f64); 4] = [("16T-5", 16, 5.0), ("20T-5", 20, 5.0), ("20T-8", 20, 8.0), ("40T-8", 40, 8.0)];
/// Rails: (size, width, height, hole Ø, counterbore Ø, counterbore depth, hole pitch); and their
/// standard (H) carriages: (size, width, length, height over the rail's base, clearance under
/// it, hole spacing across, along). As the MGN catalogues list them (HIWIN, and the makers that
/// follow it).
const RAILS: [(&str, f64, f64, f64, f64, f64, f64); 3] =
    [("MGN9", 9.0, 6.5, 3.5, 6.0, 3.5, 20.0), ("MGN12", 12.0, 8.0, 3.5, 6.0, 4.5, 25.0), ("MGN15", 15.0, 10.0, 4.5, 7.5, 5.3, 40.0)];
const CARRIAGES: [(&str, f64, f64, f64, f64, f64, f64); 3] =
    [("MGN9H", 20.0, 39.9, 10.0, 2.0, 15.0, 16.0), ("MGN12H", 27.0, 45.4, 13.0, 3.0, 20.0, 20.0), ("MGN15H", 32.0, 58.8, 16.0, 4.0, 25.0, 25.0)];

/// A 20 mm T-slot extrusion module (series 5, slot 6): slot opening, lip, chamber width under
/// the lip, depth where the chamber starts to narrow, slot depth from the face, core bore. The
/// opening, lip, depth and bore are the makers' common 2020 dimensions (Misumi HFS5, Bosch,
/// Item); the chamber narrows at 45° below the lip, as theirs do, which also keeps the four
/// slots apart. Their hollow core is left solid: 1.9 cm² here against about 1.6.
const SLOT: (f64, f64, f64, f64, f64, f64) = (6.2, 1.8, 11.0, 3.0, 6.1, 4.2);
/// Extrusion sizes: (size, modules side by side along y).
const EXTRUSIONS: [(&str, u8); 2] = [("2020", 1), ("2040", 2)];

/// A stepper: (size, body square, body length, chamfer leg, hole pitch, hole Ø, hole depth,
/// pilot Ø, pilot height, shaft Ø, shaft length from the face, holding torque N·m, rated current
/// A, mass kg). Body, pilot, holes and shaft from NEMA ICS 16; torque, current and mass from
/// common 1.8° datasheets of that body length.
#[allow(clippy::type_complexity)]
const STEPPERS: [(&str, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64); 7] = [
    ("NEMA14-28", 35.2, 28.0, 3.5, 26.0, 2.5, 3.5, 22.0, 2.0, 5.0, 20.0, 0.13, 0.4, 0.14),
    ("NEMA14-34", 35.2, 34.0, 3.5, 26.0, 2.5, 3.5, 22.0, 2.0, 5.0, 20.0, 0.18, 0.8, 0.18),
    ("NEMA17-34", 42.3, 34.0, 4.5, 31.0, 2.5, 4.5, 22.0, 2.0, 5.0, 24.0, 0.28, 1.3, 0.23),
    ("NEMA17-40", 42.3, 40.0, 4.5, 31.0, 2.5, 4.5, 22.0, 2.0, 5.0, 24.0, 0.42, 1.5, 0.28),
    ("NEMA17-48", 42.3, 48.0, 4.5, 31.0, 2.5, 4.5, 22.0, 2.0, 5.0, 24.0, 0.55, 2.0, 0.36),
    ("NEMA23-56", 56.4, 56.0, 5.0, 47.14, 5.1, 5.0, 38.1, 1.6, 6.35, 21.0, 1.26, 2.8, 0.70),
    ("NEMA23-76", 56.4, 76.0, 5.0, 47.14, 5.1, 5.0, 38.1, 1.6, 6.35, 21.0, 1.89, 3.0, 1.0),
];
/// The key a library part keeps its ratings under, in its component's metadata.
pub const RATINGS_KEY: &str = "app.polykrom.part";

/// ISO metric coarse: (size, d, pitch).
const METRIC: [(&str, f64, f64); 9] = [
    ("M3", 3.0, 0.5),
    ("M4", 4.0, 0.7),
    ("M5", 5.0, 0.8),
    ("M6", 6.0, 1.0),
    ("M8", 8.0, 1.25),
    ("M10", 10.0, 1.5),
    ("M12", 12.0, 1.75),
    ("M16", 16.0, 2.0),
    ("M20", 20.0, 2.5),
];

/// ISO 4762: (size, head diameter dk, head height k, socket s, socket depth t).
const SHCS: [(&str, f64, f64, f64, f64); 9] = [
    ("M3", 5.5, 3.0, 2.5, 1.3),
    ("M4", 7.0, 4.0, 3.0, 2.0),
    ("M5", 8.5, 5.0, 4.0, 2.5),
    ("M6", 10.0, 6.0, 5.0, 3.0),
    ("M8", 13.0, 8.0, 6.0, 4.0),
    ("M10", 16.0, 10.0, 8.0, 5.0),
    ("M12", 18.0, 12.0, 10.0, 6.0),
    ("M16", 24.0, 16.0, 14.0, 8.0),
    ("M20", 30.0, 20.0, 17.0, 10.0),
];

/// ISO 4014/4017 hex head: (size, across flats s, head height k).
const HEX_BOLT: [(&str, f64, f64); 9] = [
    ("M3", 5.5, 2.0),
    ("M4", 7.0, 2.8),
    ("M5", 8.0, 3.5),
    ("M6", 10.0, 4.0),
    ("M8", 13.0, 5.3),
    ("M10", 16.0, 6.4),
    ("M12", 18.0, 7.5),
    ("M16", 24.0, 10.0),
    ("M20", 30.0, 12.5),
];

/// ISO 4032: (size, across flats s, height m).
const HEX_NUT: [(&str, f64, f64); 9] = [
    ("M3", 5.5, 2.4),
    ("M4", 7.0, 3.2),
    ("M5", 8.0, 4.7),
    ("M6", 10.0, 5.2),
    ("M8", 13.0, 6.8),
    ("M10", 16.0, 8.4),
    ("M12", 18.0, 10.8),
    ("M16", 24.0, 14.8),
    ("M20", 30.0, 18.0),
];

/// ISO 7089: (size, inner d1, outer d2, thickness h).
const WASHER: [(&str, f64, f64, f64); 9] = [
    ("M3", 3.2, 7.0, 0.5),
    ("M4", 4.3, 9.0, 0.8),
    ("M5", 5.3, 10.0, 1.0),
    ("M6", 6.4, 12.0, 1.6),
    ("M8", 8.4, 16.0, 1.6),
    ("M10", 10.5, 20.0, 2.0),
    ("M12", 13.0, 24.0, 2.5),
    ("M16", 17.0, 30.0, 3.0),
    ("M20", 21.0, 37.0, 3.0),
];

/// ISO 4026: (size, socket s, socket depth t).
const SET_SCREW: [(&str, f64, f64); 7] =
    [("M3", 1.5, 1.2), ("M4", 2.0, 1.5), ("M5", 2.5, 2.0), ("M6", 3.0, 2.0), ("M8", 4.0, 3.0), ("M10", 5.0, 4.0), ("M12", 6.0, 4.8)];

/// ISO 8734 diameters.
const DOWEL: [f64; 9] = [2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0];

/// Bearings: (designation, bore, outside diameter, width).
const BEARINGS: [(&str, f64, f64, f64); 13] = [
    ("625", 5.0, 16.0, 5.0),
    ("626", 6.0, 19.0, 6.0),
    ("608", 8.0, 22.0, 7.0),
    ("6000", 10.0, 26.0, 8.0),
    ("6001", 12.0, 28.0, 8.0),
    ("6002", 15.0, 32.0, 9.0),
    ("6003", 17.0, 35.0, 10.0),
    ("6004", 20.0, 42.0, 12.0),
    ("6200", 10.0, 30.0, 9.0),
    ("6201", 12.0, 32.0, 10.0),
    ("6202", 15.0, 35.0, 11.0),
    ("6203", 17.0, 40.0, 12.0),
    ("6204", 20.0, 47.0, 14.0),
];

/// Standard lengths for screws, bolts and pins (mm).
const LENGTHS: [f64; 22] =
    [4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0, 80.0, 90.0, 100.0, 120.0];

fn metric(size: &str) -> Option<(f64, f64)> {
    METRIC.iter().find(|(s, _, _)| s.eq_ignore_ascii_case(size)).map(|(_, d, p)| (*d, *p))
}

fn default_length(d: f64) -> f64 {
    LENGTHS.iter().copied().find(|l| *l >= 2.5 * d).unwrap_or(100.0)
}

/// One part's shape, in its own frame.
#[derive(Clone, Debug, PartialEq)]
enum Shape {
    /// A head (round or hex), a shank down −z, an optional hex socket in the head's top.
    Screw { d: f64, pitch: f64, len: f64, head: Head, socket: Option<(f64, f64)> },
    /// A hex prism with a through hole (nut), z 0…m.
    Nut { d: f64, pitch: f64, s: f64, m: f64 },
    /// A ring (washer, bearing envelope), z 0…h.
    Ring { d_in: f64, d_out: f64, h: f64 },
    /// A cylinder down −z with a socket in its top (set screw), or plain (pin).
    Rod { d: f64, pitch: Option<f64>, len: f64, socket: Option<(f64, f64)> },
    /// A T-slot extrusion `modules` × 20 mm along y, 20 mm along x, z 0…len.
    Extrusion { modules: u8, len: f64 },
    /// A stepper motor, one row of `STEPPERS`.
    Stepper { row: usize },
    /// A GT2 pulley, one row of `PULLEYS`.
    Pulley { row: usize },
    /// A rail (one row of `RAILS`), z 0…len.
    Rail { row: usize, len: f64 },
    /// A carriage, one row of `CARRIAGES`, on the rail of the same row.
    Carriage { row: usize },
    /// A T8 lead screw, z 0…len.
    LeadScrew { len: f64 },
    /// A T8 flanged nut.
    LeadNut,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Head {
    Round { dk: f64, k: f64 },
    Hex { s: f64, k: f64 },
}

fn shape(family: &str, size: &str, length: Option<f64>) -> std::result::Result<Shape, String> {
    let need = |what: &str| format!("no {what} size `{size}` (parts.library lists them)");
    let len = |d: f64| length.unwrap_or_else(|| default_length(d));
    Ok(match family {
        "socket_head_cap_screw" => {
            let (d, pitch) = metric(size).ok_or_else(|| need("metric"))?;
            let (_, dk, k, s, t) = SHCS.iter().find(|r| r.0.eq_ignore_ascii_case(size)).copied().ok_or_else(|| need("ISO 4762"))?;
            Shape::Screw { d, pitch, len: len(d), head: Head::Round { dk, k }, socket: Some((s, t)) }
        }
        "hex_bolt" => {
            let (d, pitch) = metric(size).ok_or_else(|| need("metric"))?;
            let (_, s, k) = HEX_BOLT.iter().find(|r| r.0.eq_ignore_ascii_case(size)).copied().ok_or_else(|| need("ISO 4014"))?;
            Shape::Screw { d, pitch, len: len(d), head: Head::Hex { s, k }, socket: None }
        }
        "hex_nut" => {
            let (d, pitch) = metric(size).ok_or_else(|| need("metric"))?;
            let (_, s, m) = HEX_NUT.iter().find(|r| r.0.eq_ignore_ascii_case(size)).copied().ok_or_else(|| need("ISO 4032"))?;
            Shape::Nut { d, pitch, s, m }
        }
        "washer" => {
            let (_, d1, d2, h) = WASHER.iter().find(|r| r.0.eq_ignore_ascii_case(size)).copied().ok_or_else(|| need("ISO 7089"))?;
            Shape::Ring { d_in: d1, d_out: d2, h }
        }
        "set_screw" => {
            let (d, pitch) = metric(size).ok_or_else(|| need("metric"))?;
            let (_, s, t) = SET_SCREW.iter().find(|r| r.0.eq_ignore_ascii_case(size)).copied().ok_or_else(|| need("ISO 4026"))?;
            Shape::Rod {
                d,
                pitch: Some(pitch),
                len: length.unwrap_or_else(|| LENGTHS.iter().copied().find(|l| *l >= d).unwrap_or(d)),
                socket: Some((s, t)),
            }
        }
        "dowel_pin" => {
            let d: f64 = size.trim_start_matches(['d', 'D', 'Ø']).trim().parse().map_err(|_| need("dowel pin"))?;
            if !DOWEL.contains(&d) {
                return Err(need("ISO 8734"));
            }
            Shape::Rod { d, pitch: None, len: len(d * 1.6), socket: None }
        }
        "bearing" => {
            let (_, bore, od, w) = BEARINGS.iter().find(|r| r.0 == size.trim()).copied().ok_or_else(|| need("bearing"))?;
            Shape::Ring { d_in: bore, d_out: od, h: w }
        }
        "extrusion" => {
            let (_, modules) = EXTRUSIONS.iter().find(|r| r.0 == size.trim()).copied().ok_or_else(|| need("extrusion"))?;
            Shape::Extrusion { modules, len: length.unwrap_or(200.0) }
        }
        "stepper" => {
            let row = STEPPERS.iter().position(|r| r.0.eq_ignore_ascii_case(size.trim())).ok_or_else(|| need("stepper"))?;
            Shape::Stepper { row }
        }
        "gt2_pulley" => Shape::Pulley { row: PULLEYS.iter().position(|r| r.0.eq_ignore_ascii_case(size.trim())).ok_or_else(|| need("GT2 pulley"))? },
        "linear_rail" => Shape::Rail {
            row: RAILS.iter().position(|r| r.0.eq_ignore_ascii_case(size.trim())).ok_or_else(|| need("rail"))?,
            len: length.unwrap_or(200.0),
        },
        "linear_carriage" => {
            Shape::Carriage { row: CARRIAGES.iter().position(|r| r.0.eq_ignore_ascii_case(size.trim())).ok_or_else(|| need("carriage"))? }
        }
        "lead_screw" if size.trim().eq_ignore_ascii_case("T8") => Shape::LeadScrew { len: length.unwrap_or(200.0) },
        "lead_nut" if size.trim().eq_ignore_ascii_case("T8") => Shape::LeadNut,
        "lead_screw" | "lead_nut" => return Err(need("lead screw")),
        o => return Err(format!("unknown part family `{o}` (parts.library lists them)")),
    })
}

fn run(s: &mut Session, id: &str, p: Value) -> Result<Value> {
    s.execute(id, &p)
}

/// A hex prism (across flats `af`) from z0 to z1, joined or cut.
fn hex(s: &mut Session, af: f64, z0: f64, z1: f64, op: &str) -> Result<()> {
    run(s, "sketch.create", json!({"plane": "XY", "offset": format!("{z0} mm")}))?;
    run(s, "sketch.polygon.circumscribed", json!({"center": [0, 0], "radius": af / 2.0, "sides": 6}))?;
    run(s, "sketch.finish", json!({}))?;
    let (dist, dir) = if z1 >= z0 { (z1 - z0, "positive") } else { (z0 - z1, "negative") };
    run(s, "solid.extrude", json!({"distance": format!("{dist} mm"), "direction": dir, "operation": op}))?;
    Ok(())
}

fn cyl(s: &mut Session, d: f64, z0: f64, z1: f64, op: &str) -> Result<()> {
    run(
        s,
        "solid.cylinder",
        json!({"diameter": format!("{d} mm"), "height": format!("{} mm", z1 - z0), "base": [0, 0, z0], "axis": [0, 0, 1], "operation": op}),
    )?;
    Ok(())
}

/// A solid of revolution about z from a closed (r, z) outline, as one body or joined.
fn revolve(s: &mut Session, pts: &[(f64, f64)], op: &str) -> Result<()> {
    run(s, "sketch.create", json!({"plane": "XZ"}))?;
    let points: Vec<[f64; 2]> = pts.iter().map(|(r, z)| [*r, -*z]).collect();
    run(s, "sketch.line", json!({"points": points, "closed": true}))?;
    run(s, "sketch.finish", json!({}))?;
    run(s, "solid.revolve", json!({"axis": "y", "operation": op}))?;
    Ok(())
}

/// A closed outline in the XY plane at height z0, extruded `dist` up, joined or cut (or as a new
/// body).
fn prism(s: &mut Session, outlines: &[Vec<[f64; 2]>], circles: &[([f64; 2], f64)], z0: f64, dist: f64, op: &str) -> Result<()> {
    run(s, "sketch.create", json!({"plane": "XY", "offset": format!("{z0} mm")}))?;
    for o in outlines {
        run(s, "sketch.line", json!({"points": o, "closed": true}))?;
    }
    for (c, r) in circles {
        run(s, "sketch.circle.center", json!({"center": c, "radius": r}))?;
    }
    run(s, "sketch.finish", json!({}))?;
    run(s, "solid.extrude", json!({"distance": format!("{dist} mm"), "direction": "positive", "operation": op}))?;
    Ok(())
}

/// Circles in the XZ plane at height y0 (centres given as (x, z)), extruded `dist` along +y.
fn holes_y(s: &mut Session, centres: &[(f64, f64)], d: f64, y0: f64, dist: f64, op: &str) -> Result<()> {
    run(s, "sketch.create", json!({"plane": "XZ", "offset": format!("{y0} mm")}))?;
    for (x, z) in centres {
        // The XZ sketch's second axis is −z.
        run(s, "sketch.circle.center", json!({"center": [x, -z], "radius": d / 2.0}))?;
    }
    run(s, "sketch.finish", json!({}))?;
    run(s, "solid.extrude", json!({"distance": format!("{dist} mm"), "direction": "positive", "operation": op}))?;
    Ok(())
}

/// A pulley's pitch, outside, flange and hub diameters.
fn pulley_dims(teeth: u32) -> (f64, f64, f64, f64) {
    let pd = f64::from(teeth) * 2.0 / std::f64::consts::PI;
    let od = pd - 0.508;
    (pd, od, od + 4.0, (od + 3.5).max(13.0))
}

/// A GT2 pulley's toothed outline, outside radius `o`: a land on the outside diameter, then a
/// tooth space (an R0.555 arc whose deepest point is 0.75 mm in), for each tooth.
fn teeth_outline(teeth: u32, o: f64) -> Vec<[f64; 2]> {
    let (r, depth) = (0.555, 0.75);
    let c = o + r - depth;
    // Where the space's arc meets the outside circle, as an angle about the pulley's axis.
    let phi = ((o * o + c * c - r * r) / (2.0 * o * c)).clamp(-1.0, 1.0).acos();
    let step = std::f64::consts::TAU / f64::from(teeth);
    let mut out = Vec::new();
    for i in 0..teeth {
        let a = step * f64::from(i);
        // The space, from its leading edge round its deepest point to its trailing edge.
        let (sx, sy) = ((a - phi).cos() * o - c * a.cos(), (a - phi).sin() * o - c * a.sin());
        let (ex, ey) = ((a + phi).cos() * o - c * a.cos(), (a + phi).sin() * o - c * a.sin());
        let (from, mut to) = (sy.atan2(sx), ey.atan2(ex));
        // The long way round, through the side facing the axis.
        if to < from {
            to += std::f64::consts::TAU;
        }
        let to = to - std::f64::consts::TAU;
        for k in 0..=8 {
            let t = from + (to - from) * f64::from(k) / 8.0;
            out.push([c * a.cos() + r * t.cos(), c * a.sin() + r * t.sin()]);
        }
        // The land up to the next space.
        for k in 1..4 {
            let t = a + phi + (step - 2.0 * phi) * f64::from(k) / 4.0;
            out.push([o * t.cos(), o * t.sin()]);
        }
    }
    out
}

/// The T-slot of a module centred at (cx, cy), in the face whose outward normal is (nx, ny):
/// the opening through the lip, then the wider chamber behind it. It starts a little outside the
/// face so the cut opens it cleanly.
fn slot(cx: f64, cy: f64, nx: f64, ny: f64) -> Vec<[f64; 2]> {
    let (open, lip, chamber, straight, depth, _) = SLOT;
    let floor = chamber / 2.0 - (depth - straight);
    // (along the normal from the module's centre, across it)
    let local = [
        (10.5, -open / 2.0),
        (10.0 - lip, -open / 2.0),
        (10.0 - lip, -chamber / 2.0),
        (10.0 - straight, -chamber / 2.0),
        (10.0 - depth, -floor),
        (10.0 - depth, floor),
        (10.0 - straight, chamber / 2.0),
        (10.0 - lip, chamber / 2.0),
        (10.0 - lip, open / 2.0),
        (10.5, open / 2.0),
    ];
    local.iter().map(|(n, t)| [cx + n * nx - t * ny, cy + n * ny + t * nx]).collect()
}

fn thread(s: &mut Session, d: f64, pitch: f64, at: Vec3) {
    let _ = run(s, "solid.thread", json!({"face": at, "designation": format!("M{}x{}", d, pitch)}));
    let _ = d;
}

/// Build a part in the active component.
fn build(s: &mut Session, sh: &Shape) -> Result<()> {
    let _frame = crate::frames::local_geometry();
    match *sh {
        Shape::Screw { d, pitch, len, head, socket } => {
            let k = match head {
                // One revolved outline: shank and round head.
                Head::Round { dk, k } => {
                    revolve(s, &[(0.0, -len), (d / 2.0, -len), (d / 2.0, 0.0), (dk / 2.0, 0.0), (dk / 2.0, k), (0.0, k)], "new")?;
                    k
                }
                // A hex head joined over a shank that runs up into it.
                Head::Hex { s: af, k } => {
                    hex(s, af, 0.0, k, "new")?;
                    revolve(s, &[(0.0, -len), (d / 2.0, -len), (d / 2.0, k / 2.0), (0.0, k / 2.0)], "join")?;
                    k
                }
            };
            if let Some((af, t)) = socket {
                hex(s, af, k, k - t, "cut")?;
            }
            thread(s, d, pitch, Vec3::new(d / 2.0, 0.0, -len / 2.0));
        }
        Shape::Nut { d, pitch, s: af, m } => {
            hex(s, af, 0.0, m, "new")?;
            cyl(s, d, -1.0, m + 1.0, "cut")?;
            thread(s, d, pitch, Vec3::new(d / 2.0, 0.0, m / 2.0));
        }
        Shape::Ring { d_in, d_out, h } => {
            revolve(s, &[(d_in / 2.0, 0.0), (d_out / 2.0, 0.0), (d_out / 2.0, h), (d_in / 2.0, h)], "new")?;
        }
        Shape::Rod { d, pitch, len, socket } => {
            revolve(s, &[(0.0, -len), (d / 2.0, -len), (d / 2.0, 0.0), (0.0, 0.0)], "new")?;
            if let Some((af, t)) = socket {
                hex(s, af, 0.0, -t, "cut")?;
            }
            if let Some(p) = pitch {
                thread(s, d, p, Vec3::new(d / 2.0, 0.0, -len / 2.0));
            }
        }
        Shape::Extrusion { modules, len } => {
            let n = f64::from(modules);
            let half_y = 10.0 * n;
            run(
                s,
                "solid.box",
                json!({"length": "20 mm", "width": format!("{} mm", 20.0 * n), "height": format!("{len} mm"), "corner": [-10, -half_y, 0]}),
            )?;
            // Module centres along y; every outer face of a module has a slot, the faces between
            // modules none.
            let centres: Vec<f64> = (0..modules).map(|i| -half_y + 10.0 + 20.0 * f64::from(i)).collect();
            let mut slots = Vec::new();
            for (i, cy) in centres.iter().enumerate() {
                slots.push(slot(0.0, *cy, 1.0, 0.0));
                slots.push(slot(0.0, *cy, -1.0, 0.0));
                if i == 0 {
                    slots.push(slot(0.0, *cy, 0.0, -1.0));
                }
                if i + 1 == centres.len() {
                    slots.push(slot(0.0, *cy, 0.0, 1.0));
                }
            }
            let bores: Vec<([f64; 2], f64)> = centres.iter().map(|cy| ([0.0, *cy], SLOT.5 / 2.0)).collect();
            prism(s, &slots, &bores, -1.0, len + 2.0, "cut")?;
        }
        Shape::Stepper { row } => {
            let Some(&(_, sq, len, c, pitch, hole, depth, pilot, pilot_h, shaft, shaft_len, ..)) = STEPPERS.get(row) else {
                return Err(bad("parts.insert", "no such stepper"));
            };
            let h = sq / 2.0;
            let body = vec![[h - c, -h], [h, -h + c], [h, h - c], [h - c, h], [-h + c, h], [-h, h - c], [-h, -h + c], [-h + c, -h]];
            prism(s, &[body], &[], 0.0, len, "new")?;
            cyl(s, pilot, -pilot_h, 0.0, "join")?;
            cyl(s, shaft, -shaft_len, 0.0, "join")?;
            // The shaft's flat: 0.5 mm deep over the 15 mm nearest its end.
            let f = shaft / 2.0 - 0.5;
            prism(s, &[vec![[f, -shaft], [shaft, -shaft], [shaft, shaft], [f, shaft]]], &[], -shaft_len - 1.0, 16.0, "cut")?;
            let holes: Vec<([f64; 2], f64)> =
                [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|(x, y)| ([x * pitch / 2.0, y * pitch / 2.0], hole / 2.0)).collect();
            prism(s, &[], &holes, -0.5, depth + 0.5, "cut")?;
            // The NEMA 14 and 17 holes are tapped M3.
            if hole < 3.0 {
                for ([x, y], r) in &holes {
                    thread(s, 3.0, 0.5, Vec3::new(x + r, *y, depth / 2.0));
                }
            }
        }
        Shape::Pulley { row } => {
            let Some(&(_, teeth, bore)) = PULLEYS.get(row) else { return Err(bad("parts.insert", "no such pulley")) };
            let (_, od, flange, hub) = pulley_dims(teeth);
            let (b, o) = (bore / 2.0, od / 2.0);
            // The toothed band first, its grooves cut on their own (GT2's 0.75 mm deep, R0.56
            // tooth spaces), then the hub with its flange and the far flange joined to it.
            // Three bodies that touch, each made in one step: the hub with its flange, the toothed
            // band (its outline drawn once) and the far flange. Joining them into one solid, or
            // cutting the grooves from a plain band, took truck's booleans 14 s to minutes.
            revolve(s, &[(b, 0.0), (hub / 2.0, 0.0), (hub / 2.0, 7.0), (flange / 2.0, 7.0), (flange / 2.0, 8.0), (b, 8.0)], "new")?;
            prism(s, &[teeth_outline(teeth, o)], &[([0.0, 0.0], b)], 8.0, 7.0, "new")?;
            revolve(s, &[(b, 15.0), (flange / 2.0, 15.0), (flange / 2.0, 16.0), (b, 16.0)], "new")?;
        }
        Shape::Rail { row, len } => {
            let Some(&(_, w, h, hole, cb, cb_depth, pitch)) = RAILS.get(row) else { return Err(bad("parts.insert", "no such rail")) };
            run(
                s,
                "solid.box",
                json!({"length": format!("{w} mm"), "width": format!("{h} mm"), "height": format!("{len} mm"), "corner": [-w / 2.0, 0, 0]}),
            )?;
            // The ball tracks along each side, 1 mm square, 2 mm under the top.
            let track = |x: f64| vec![[x - 1.0, h - 3.0], [x + 1.0, h - 3.0], [x + 1.0, h - 2.0], [x - 1.0, h - 2.0]];
            prism(s, &[track(-w / 2.0), track(w / 2.0)], &[], -1.0, len + 2.0, "cut")?;
            // Mounting holes on the pitch, centred along the rail, counterbored from the top.
            let n = ((len / pitch).floor() as usize).max(1);
            let first = (len - (n as f64 - 1.0) * pitch) / 2.0;
            let centres: Vec<(f64, f64)> = (0..n).map(|i| (0.0, first + i as f64 * pitch)).collect();
            holes_y(s, &centres, hole, -1.0, h + 2.0, "cut")?;
            holes_y(s, &centres, cb, h - cb_depth, cb_depth + 1.0, "cut")?;
        }
        Shape::Carriage { row } => {
            let (Some(&(_, w, l, top, under, across, along)), Some(&(_, rail_w, rail_h, ..))) = (CARRIAGES.get(row), RAILS.get(row)) else {
                return Err(bad("parts.insert", "no such carriage"));
            };
            run(
                s,
                "solid.box",
                json!({"length": format!("{w} mm"), "width": format!("{} mm", top - under), "height": format!("{l} mm"), "corner": [-w / 2.0, under, 0]}),
            )?;
            // The channel it rides the rail in: 0.25 mm clear of each side, 0.5 mm over the top.
            let c = rail_w / 2.0 + 0.25;
            prism(s, &[vec![[-c, under - 1.0], [c, under - 1.0], [c, rail_h + 0.5], [-c, rail_h + 0.5]]], &[], -1.0, l + 2.0, "cut")?;
            let holes: Vec<(f64, f64)> =
                [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|(x, z)| (x * across / 2.0, l / 2.0 + z * along / 2.0)).collect();
            holes_y(s, &holes, 2.5, top - 3.5, 4.5, "cut")?;
            for (x, z) in &holes {
                thread(s, 3.0, 0.5, Vec3::new(x + 1.25, top - 1.75, *z));
            }
        }
        Shape::LeadScrew { len } => {
            revolve(s, &[(0.0, 0.0), (4.0, 0.0), (4.0, len), (0.0, len)], "new")?;
        }
        Shape::LeadNut => {
            revolve(s, &[(4.0, -11.5), (5.1, -11.5), (5.1, 0.0), (11.0, 0.0), (11.0, 3.5), (4.0, 3.5)], "new")?;
            prism(s, &[], &[([8.0, 0.0], 1.75), ([0.0, 8.0], 1.75), ([-8.0, 0.0], 1.75), ([0.0, -8.0], 1.75)], -0.5, 4.5, "cut")?;
        }
    }
    Ok(())
}

fn library(_s: &mut Session, p: &Value) -> Result<Value> {
    let fam = |id: &str| -> Value {
        let sizes: Vec<Value> = match id {
            "socket_head_cap_screw" => SHCS.iter().map(|r| json!({"size": r.0, "d": metric(r.0).map(|m| m.0), "pitch": metric(r.0).map(|m| m.1), "head_diameter": r.1, "head_height": r.2, "socket": r.3, "socket_depth": r.4})).collect(),
            "hex_bolt" => HEX_BOLT.iter().map(|r| json!({"size": r.0, "d": metric(r.0).map(|m| m.0), "pitch": metric(r.0).map(|m| m.1), "across_flats": r.1, "head_height": r.2})).collect(),
            "hex_nut" => HEX_NUT.iter().map(|r| json!({"size": r.0, "d": metric(r.0).map(|m| m.0), "pitch": metric(r.0).map(|m| m.1), "across_flats": r.1, "height": r.2})).collect(),
            "washer" => WASHER.iter().map(|r| json!({"size": r.0, "inner": r.1, "outer": r.2, "thickness": r.3})).collect(),
            "set_screw" => SET_SCREW.iter().map(|r| json!({"size": r.0, "d": metric(r.0).map(|m| m.0), "pitch": metric(r.0).map(|m| m.1), "socket": r.1, "socket_depth": r.2})).collect(),
            "dowel_pin" => DOWEL.iter().map(|d| json!({"size": format!("{d}"), "d": d})).collect(),
            "bearing" => BEARINGS.iter().map(|r| json!({"size": r.0, "bore": r.1, "outer": r.2, "width": r.3})).collect(),
            "extrusion" => EXTRUSIONS
                .iter()
                .map(|r| json!({"size": r.0, "width": 20.0, "height": 20.0 * f64::from(r.1), "slot": SLOT.0, "slot_depth": SLOT.4, "core_bore": SLOT.5}))
                .collect(),
            "stepper" => STEPPERS
                .iter()
                .map(|r| {
                    json!({"size": r.0, "body": r.1, "length": r.2, "hole_pitch": r.4, "hole": r.5, "pilot": r.7, "shaft": r.9, "shaft_length": r.10,
                           "holding_torque_nm": r.11, "rated_current_a": r.12, "mass_kg": r.13, "step_angle_deg": 1.8})
                })
                .collect(),
            "gt2_pulley" => PULLEYS
                .iter()
                .map(|r| {
                    let (pd, od, flange, hub) = pulley_dims(r.1);
                    json!({"size": r.0, "teeth": r.1, "bore": r.2, "pitch_diameter": pd, "outside_diameter": od, "flange": flange, "hub": hub,
                           "belt_mm_per_deg": f64::from(r.1) * 2.0 / 360.0})
                })
                .collect(),
            "linear_rail" => RAILS
                .iter()
                .map(|r| json!({"size": r.0, "width": r.1, "height": r.2, "hole": r.3, "counterbore": r.4, "counterbore_depth": r.5, "hole_pitch": r.6}))
                .collect(),
            "linear_carriage" => CARRIAGES
                .iter()
                .map(|r| json!({"size": r.0, "width": r.1, "length": r.2, "height": r.3, "clearance": r.4, "holes_across": r.5, "holes_along": r.6, "holes": "M3"}))
                .collect(),
            "lead_screw" => vec![json!({"size": "T8", "d": 8.0, "lead": 8.0, "pitch": 2.0, "starts": 4})],
            "lead_nut" => vec![json!({"size": "T8", "flange": 22.0, "flange_thickness": 3.5, "body": 10.2, "length": 15.0, "holes": 3.5, "hole_circle": 16.0})],
            _ => Vec::new(),
        };
        let (_, name, standard) = FAMILIES.iter().find(|f| f.0 == id).copied().unwrap_or(("", "", ""));
        let lengths = matches!(id, "socket_head_cap_screw" | "hex_bolt" | "set_screw" | "dowel_pin").then_some(LENGTHS.to_vec());
        // Any length (mm) for an extrusion.
        let lengths = if matches!(id, "extrusion" | "linear_rail" | "lead_screw") {
            Some(vec![100.0, 200.0, 250.0, 300.0, 350.0, 400.0, 500.0, 1000.0])
        } else {
            lengths
        };
        json!({"family": id, "name": name, "standard": standard, "sizes": sizes, "lengths": lengths})
    };
    match str_(p, "family") {
        Some(f) if FAMILIES.iter().any(|x| x.0 == f) => Ok(fam(f)),
        Some(f) => Err(bad("parts.library", format!("unknown part family `{f}`"))),
        None => Ok(json!({"families": FAMILIES.iter().map(|f| fam(f.0)).collect::<Vec<_>>()})),
    }
}

fn part_name(family: &str, size: &str, sh: &Shape) -> String {
    match sh {
        Shape::Screw { len, .. } | Shape::Rod { len, .. } | Shape::Extrusion { len, .. } | Shape::Rail { len, .. } | Shape::LeadScrew { len, .. } => {
            format!("{} {size}x{len}", label(family))
        }
        _ => format!("{} {size}", label(family)),
    }
}

/// A standard part's part number and description: its standard and size (`ISO 4762 M6x20`), or
/// a bearing's designation, and the family's name with the size.
fn identity(family: &str, size: &str, sh: &Shape) -> (String, String) {
    let size = match sh {
        Shape::Screw { len, .. } | Shape::Rod { len, .. } | Shape::Extrusion { len, .. } | Shape::Rail { len, .. } | Shape::LeadScrew { len, .. } => {
            format!("{size}x{len}")
        }
        _ => size.to_string(),
    };
    let standard = match family {
        "socket_head_cap_screw" => "ISO 4762",
        "hex_bolt" => "ISO 4017",
        "hex_nut" => "ISO 4032",
        "washer" => "ISO 7089",
        "set_screw" => "ISO 4026",
        "dowel_pin" => "ISO 8734",
        _ => "",
    };
    let name = FAMILIES.iter().find(|f| f.0 == family).map_or("Part", |f| f.1);
    let part_number = if standard.is_empty() { size.clone() } else { format!("{standard} {size}") };
    (part_number, format!("{name} {size}"))
}

/// Give a standard part its part number and description; a field someone changed since the last
/// size (`was`) is theirs and stays.
fn set_identity(s: &mut Session, comp: u64, now: (String, String), was: Option<(String, String)>) {
    use solidcraft_doc::metadata::{DESCRIPTION, PART_NUMBER};
    let Some(c) = s.doc.components.iter().find(|c| c.id == comp) else { return };
    let mut m = c.metadata.clone();
    for (k, new, old) in [(PART_NUMBER, now.0, was.as_ref().map(|w| w.0.as_str())), (DESCRIPTION, now.1, was.as_ref().map(|w| w.1.as_str()))] {
        if m.text(k).is_none_or(|cur| Some(cur) == old) {
            let _ = m.set(k, Value::String(new));
        }
    }
    if m != c.metadata
        && let Some(c) = s.doc_mut().components.iter_mut().find(|c| c.id == comp)
    {
        c.metadata = m;
    }
}

/// What motion hardware is made of, and its ratings in its metadata (`RATINGS_KEY`): a stepper's
/// torque and mass (its body is a solid block, so its mass comes from the datasheet's
/// `mass_kg`, not the material), a pulley's belt travel per degree, a lead screw's lead.
/// Extrusions, steppers and pulleys are aluminium, a lead screw nut brass, the rest steel.
fn fit_out(s: &mut Session, comp: u64, sh: &Shape) {
    let material = match sh {
        Shape::Extrusion { .. } | Shape::Stepper { .. } | Shape::Pulley { .. } => "Aluminum",
        Shape::LeadNut => "Brass",
        Shape::LeadScrew { .. } => "Steel",
        _ => return,
    };
    let bodies: Vec<String> = s.model.state().bodies.iter().filter(|b| s.doc.model_body_component(b) == comp).map(|b| b.name.clone()).collect();
    let _ = run(s, "material.assign", json!({"bodies": bodies, "material": material}));
    let ratings = match *sh {
        Shape::Stepper { row } => STEPPERS.get(row).map(|r| {
            json!({"kind": "stepper", "holding_torque_nm": r.11, "rated_current_a": r.12, "mass_kg": r.13, "step_angle_deg": 1.8, "shaft_mm": r.9})
        }),
        Shape::Pulley { row } => PULLEYS.get(row).map(|r| {
            json!({"kind": "gt2_pulley", "teeth": r.1, "pitch_diameter_mm": pulley_dims(r.1).0, "belt_mm_per_deg": f64::from(r.1) * 2.0 / 360.0})
        }),
        Shape::LeadScrew { .. } | Shape::LeadNut => Some(json!({"kind": "lead_screw", "lead_mm": 8.0, "pitch_mm": 2.0, "starts": 4})),
        _ => None,
    };
    let Some(ratings) = ratings else { return };
    let Some(c) = s.doc.components.iter().find(|c| c.id == comp) else { return };
    let mut m = c.metadata.clone();
    if m.set(RATINGS_KEY, ratings).is_ok()
        && m != c.metadata
        && let Some(c) = s.doc_mut().components.iter_mut().find(|c| c.id == comp)
    {
        c.metadata = m;
    }
}

fn label(family: &str) -> &'static str {
    match family {
        "socket_head_cap_screw" => "SHCS",
        "hex_bolt" => "Hex Bolt",
        "hex_nut" => "Hex Nut",
        "washer" => "Washer",
        "set_screw" => "Set Screw",
        "dowel_pin" => "Dowel Pin",
        "extrusion" => "Extrusion",
        "stepper" => "Stepper",
        "gt2_pulley" => "GT2 Pulley",
        "linear_rail" => "Rail",
        "linear_carriage" => "Carriage",
        "lead_screw" => "Lead Screw",
        "lead_nut" => "Lead Nut",
        _ => "Bearing",
    }
}

fn length_param(p: &Value, cmd: &str) -> Result<Option<f64>> {
    match p.get("length") {
        None | Some(Value::Null) => Ok(None),
        Some(_) => {
            let l = num(p, "length").filter(|l| l.is_finite() && *l > 0.0 && *l <= 2000.0).ok_or_else(|| bad(cmd, "`length` must be 0…2000 mm"))?;
            Ok(Some(l))
        }
    }
}

fn insert(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "parts.insert (Insert Fastener)";
    let family = str_(p, "family").ok_or_else(|| bad(cmd, "`family` is required (parts.library)"))?.to_string();
    let size = str_(p, "size").ok_or_else(|| bad(cmd, "`size` is required"))?.to_string();
    let sh = shape(&family, &size, length_param(p, cmd)?).map_err(|e| bad(cmd, e))?;
    // A pick on a hole's rim also touches its end face: step onto the wall just inside the rim.
    let at = p.get("at").and_then(vec3).map(|a| hole_wall(s, a).unwrap_or(a));
    // The occurrence the hole belongs to, found before the part exists.
    let host = match at {
        Some(a) => Some(
            hole_host(s, a).or_else(|| super::joints::occurrence_at(s, a)).ok_or_else(|| bad(cmd, "no body at `at` (pick a hole's rim or wall)"))?,
        ),
        None => None,
    };
    let name = str_(p, "name").map(str::to_string).unwrap_or_else(|| part_name(&family, &size, &sh));
    let active = s.active_component;
    // Hardware is fitted to other parts: it starts free even as the design's first part (#2804).
    let c = run(s, "component.create", json!({"name": name, "grounded": false}))?;
    let (comp, occ) = (c["component"].as_u64().unwrap_or(0), c["occurrence"].as_u64().unwrap_or(0));
    build(s, &sh)?;
    finish(s, comp, &name);
    s.doc_mut().parts.insert(comp, solidcraft_doc::StandardPart { family: family.clone(), size: size.clone(), length: shape_len(&sh) });
    set_identity(s, comp, identity(&family, &size, &sh), None);
    fit_out(s, comp, &sh);
    run(s, "component.activate", json!({"component": active}))?;
    let mut out = json!({"component": comp, "occurrence": occ, "name": name});
    if let (Some(a), Some(h)) = (at, host) {
        // Seat it: its axis on the hole's, its seating plane on the hole's face, the shank going
        // into the hole (the joint mates the part's −z with the face's outward z). A bearing is
        // pressed into the bore instead: its top face lies on the picked rim and it fills the bore
        // from there, whichever end of the bore was picked.
        let seat = match sh {
            Shape::Ring { h: w, .. } if family == "bearing" => w,
            _ => 0.0,
        };
        let j = run(
            s,
            "joint.create",
            json!({"type": "rigid", "a": {"occurrence": h, "circle": a}, "b": {"occurrence": occ, "point": [0, 0, seat], "z": [0, 0, -1]}, "name": format!("{name} seat")}),
        )?;
        out["joint"] = j["joint"].clone();
    } else if let Some(pt) = p.get("point").and_then(vec3) {
        // Placed at a point, its +z along `direction`.
        let dir = p.get("direction").and_then(vec3).and_then(|d| d.normalized()).unwrap_or(Vec3::Z);
        let m = solidcraft_doc::rigid(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::Z.cross(dir).normalized().unwrap_or(Vec3::X),
            Vec3::Z.dot(dir).clamp(-1.0, 1.0).acos(),
        );
        let mut m = m;
        m[3] = [pt.x, pt.y, pt.z, 1.0];
        if let Some(o) = s.doc_mut().occurrences.iter_mut().find(|o| o.id == occ) {
            o.transform = m;
        }
    }
    Ok(out)
}

/// A library part's bodies as the part: named after it (`SHCS M4x12`, numbered when that name is
/// taken) rather than `Body1`, and in Steel rather than the default material, so its mass and the
/// bill of materials are right without a further step. The component is the active one.
fn finish(s: &mut Session, comp: u64, name: &str) {
    let made: Vec<String> = s.model.state().bodies.iter().filter(|b| s.doc.model_body_component(b) == comp).map(|b| b.name.clone()).collect();
    let mut names = Vec::new();
    for (i, old) in made.iter().enumerate() {
        let base = if made.len() == 1 { name.to_string() } else { format!("{name} {}", i + 1) };
        let free = (1..1000)
            .map(|n| if n == 1 { base.clone() } else { format!("{base} ({n})") })
            .find(|c| c == old || s.model.state().body(c).is_none())
            .unwrap_or_else(|| old.clone());
        names.push(if run(s, "body.rename", json!({"body": old, "name": free})).is_ok() { free } else { old.clone() });
    }
    if !names.is_empty() {
        let _ = run(s, "material.assign", json!({"bodies": names, "material": "Steel"}));
    }
}

/// A point on the cylindrical wall of the hole at `a`, a little in from the picked rim. `a` is in
/// world coordinates and so is the answer; each body is tested where its occurrences place it, so a
/// hole in a placed part (a seated washer, a bracket moved into position) is found too.
fn hole_wall(s: &Session, a: Vec3) -> Option<Vec3> {
    let st = s.model.state();
    let dirs = [Vec3::Z * -1.0, Vec3::Z, Vec3::X, Vec3::X * -1.0, Vec3::Y, Vec3::Y * -1.0];
    let places: Vec<(&_, solidcraft_doc::Mat)> = st
        .bodies
        .iter()
        .flat_map(|b| {
            let comp = s.doc.model_body_component(b);
            let ts: Vec<solidcraft_doc::Mat> = if comp == 0 {
                vec![solidcraft_doc::IDENTITY]
            } else {
                s.doc.occurrences.iter().filter(|o| o.component == comp).map(|o| solidcraft_doc::joints::occurrence_world(&s.doc, o.id)).collect()
            };
            ts.into_iter().filter_map(move |t| solidcraft_doc::mat_inverse(&t).map(|inv| (b, inv)))
        })
        .collect();
    for step in [0.2, 0.5, 1.0, 2.0] {
        for (b, inv) in &places {
            for d in dirs {
                let q = a + d * step;
                let local = solidcraft_doc::apply_point(inv, q);
                if let Some(c) = solidcraft_doc::kernel::cylinder_face_at(&b.body, local) {
                    let v = local - c.axis_point;
                    let along = v.dot(c.axis);
                    let on_wall = ((v - c.axis * along).len() - c.radius).abs() < 1e-3 && along > c.start && along < c.end;
                    if on_wall {
                        return Some(q);
                    }
                }
            }
        }
    }
    None
}

/// The occurrence whose part has a hole at `a`, on its wall or centred on one of its ends: the
/// part a fastener seats in. It is not always the nearest bounding box: a stepper's shaft reaches
/// past the mount it hangs from, so its box holds the mount's holes (#134). A hole (material
/// outside the cylinder) wins over a boss or a shaft at the same point.
fn hole_host(s: &Session, a: Vec3) -> Option<u64> {
    let st = s.model.state();
    let mut best: Option<((bool, f64), u64)> = None;
    for b in &st.bodies {
        let comp = s.doc.model_body_component(b);
        let occs: Vec<u64> = if comp == 0 { vec![0] } else { s.doc.occurrences.iter().filter(|o| o.component == comp).map(|o| o.id).collect() };
        for occ in occs {
            let t = if occ == 0 { solidcraft_doc::IDENTITY } else { solidcraft_doc::joints::occurrence_world(&s.doc, occ) };
            let Some(inv) = solidcraft_doc::mat_inverse(&t) else { continue };
            let q = solidcraft_doc::apply_point(&inv, a);
            let bb = b.mesh().bounds();
            let outside = (q.x - q.x.clamp(bb.min.x, bb.max.x)).abs()
                + (q.y - q.y.clamp(bb.min.y, bb.max.y)).abs()
                + (q.z - q.z.clamp(bb.min.z, bb.max.z)).abs();
            if outside > 1e-3 {
                continue;
            }
            let wall = solidcraft_doc::kernel::cylinder_face_at(&b.body, q).map(|c| (c.internal, c.off(q))).filter(|(_, off)| *off < 1e-3);
            let end = solidcraft_doc::kernel::cylinder_end_at(&b.body, q).map(|c| {
                let v = q - c.axis_point;
                (c.internal, (v - c.axis * v.dot(c.axis)).len())
            });
            for (internal, off) in wall.into_iter().chain(end) {
                let key = (!internal, off);
                if best.is_none_or(|(k, _)| key < k) {
                    best = Some((key, occ));
                }
            }
        }
    }
    best.map(|(_, occ)| occ)
}

fn shape_len(sh: &Shape) -> Option<f64> {
    match sh {
        Shape::Screw { len, .. } | Shape::Rod { len, .. } | Shape::Extrusion { len, .. } | Shape::Rail { len, .. } | Shape::LeadScrew { len, .. } => {
            Some(*len)
        }
        _ => None,
    }
}

/// Building the replacement runs several feature commands. Keep their intermediate history
/// and any failed build on a scratch session; the outer command records one complete undo.
fn resize_atomic(s: &mut Session, p: &Value) -> Result<Value> {
    let mut computed = s.scratch();
    let result = resize(&mut computed, p)?;
    computed.undo.clear();
    computed.active_component = s.active_component;
    computed.active_occurrence_path = s.active_occurrence_path.clone();
    computed.active_sketch = s.active_sketch.filter(|id| computed.doc.feature(*id).is_some());
    s.accept_computed(computed);
    Ok(result)
}

fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "parts.resize";
    let key = match p.get("component") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(x)) => x.clone(),
        _ => return Err(bad(cmd, "`component` must be an id or a name")),
    };
    let comp = s.doc.find_component(&key).ok_or_else(|| bad(cmd, format!("no component `{key}`")))?;
    let part = s.doc.parts.get(&comp).cloned().ok_or_else(|| bad(cmd, "that component is not a standard part"))?;
    let size = str_(p, "size").map(str::to_string).unwrap_or(part.size.clone());
    let length = length_param(p, cmd)?.or(if size == part.size { part.length } else { None });
    let sh = shape(&part.family, &size, length).map_err(|e| bad(cmd, e))?;
    // Rebuild: the component's features go, the new shape is built in their place.
    let ids: Vec<u64> = s.doc.features.iter().filter(|f| f.component == comp).map(|f| f.id).collect();
    for id in ids.iter().rev() {
        let _ = s.doc_mut().delete_feature(*id);
    }
    let active = s.active_component;
    run(s, "component.activate", json!({"component": comp}))?;
    build(s, &sh)?;
    let name = part_name(&part.family, &size, &sh);
    finish(s, comp, &name);
    let was = shape(&part.family, &part.size, part.length).ok().map(|old| identity(&part.family, &part.size, &old));
    set_identity(s, comp, identity(&part.family, &size, &sh), was);
    fit_out(s, comp, &sh);
    s.doc_mut().parts.insert(comp, solidcraft_doc::StandardPart { family: part.family.clone(), size: size.clone(), length: shape_len(&sh) });
    if let Some(c) = s.doc_mut().components.iter_mut().find(|c| c.id == comp) {
        c.name = name.clone();
    }
    run(s, "component.activate", json!({"component": active}))?;
    s.refresh();
    if let Some(e) = s.model.results.iter().find_map(|r| r.error.clone()) {
        return Err(EngineError::Other(e));
    }
    Ok(json!({"component": comp, "name": name, "size": size, "length": shape_len(&sh)}))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::Session;

    fn run(s: &mut Session, id: &str, p: Value) -> Value {
        match s.execute(id, &p) {
            Ok(v) => v,
            Err(e) => panic!("{id} {p}: {e}"),
        }
    }

    fn bbox(s: &Session, prefix: &str) -> (solidcraft_geom::Vec3, solidcraft_geom::Vec3) {
        let st = s.world_state();
        let comp = s.doc.components.iter().find(|c| c.name.starts_with(prefix)).map(|c| c.id).unwrap_or(0);
        let b = st
            .bodies
            .iter()
            .filter(|b| s.doc.model_body_component(b) == comp)
            .fold(solidcraft_geom::Aabb3::EMPTY, |a, b| a.union(&b.mesh().bounds()));
        (b.min, b.max)
    }

    #[test]
    fn each_library_screw_carries_its_own_thread() {
        // Both are built at the origin; the second one's thread must not land on the first.
        let mut s = Session::default();
        for name in ["A", "B"] {
            run(&mut s, "parts.insert", json!({"family": "socket_head_cap_screw", "size": "M6", "length": 16, "name": name}));
        }
        let st = s.model.state();
        let owners: Vec<Option<u64>> = st.threads.iter().map(|t| st.body(&t.body).map(|b| s.doc.model_body_component(b))).collect();
        let comps: Vec<Option<u64>> = st.threads.iter().map(|t| s.doc.features.iter().find(|f| f.id == t.feature).map(|f| f.component)).collect();
        assert_eq!(st.threads.len(), 2, "{:?}", st.threads);
        assert_eq!(owners, comps, "each thread on its own part's body: {:?}", st.threads);
    }

    #[test]
    fn dimensions_follow_the_standard_tables() {
        let l = run(&mut Session::default(), "parts.library", json!({"family": "socket_head_cap_screw"}));
        let m6 = l["sizes"].as_array().and_then(|a| a.iter().find(|x| x["size"] == "M6")).cloned().unwrap_or_default();
        assert_eq!((m6["head_diameter"].as_f64(), m6["head_height"].as_f64(), m6["socket"].as_f64()), (Some(10.0), Some(6.0), Some(5.0)));
        // An M6 x 20 cap screw: head Ø10 x 6, shank 20 long.
        let mut s = Session::default();
        run(&mut s, "parts.insert", json!({"family": "socket_head_cap_screw", "size": "M6", "length": 20}));
        let (lo, hi) = bbox(&s, "SHCS M6x20");
        assert!((hi.x - lo.x - 10.0).abs() < 1e-6 && (lo.z + 20.0).abs() < 1e-6 && (hi.z - 6.0).abs() < 1e-6, "{lo:?} {hi:?}");
        assert!(s.model.state().threads.iter().any(|t| t.designation.starts_with("M6")), "{:?}", s.model.state().threads);
        // An M8 nut: 13 across flats (14.43 across corners), 6.8 high.
        let mut n = Session::default();
        run(&mut n, "parts.insert", json!({"family": "hex_nut", "size": "M8"}));
        let (lo, hi) = bbox(&n, "Hex Nut M8");
        let (w, h) = ((hi.x - lo.x).max(hi.y - lo.y), hi.z - lo.z);
        assert!((w - 13.0 / (std::f64::consts::PI / 6.0).cos()).abs() < 1e-6 && (h - 6.8).abs() < 1e-6, "{w} {h}");
        // A 608 bearing: 8 x 22 x 7.
        let mut b = Session::default();
        run(&mut b, "parts.insert", json!({"family": "bearing", "size": "608"}));
        let (lo, hi) = bbox(&b, "Bearing 608");
        assert!((hi.x - lo.x - 22.0).abs() < 1e-6 && (hi.z - lo.z - 7.0).abs() < 1e-6);
        assert!(b.execute("parts.insert", &json!({"family": "hex_nut", "size": "M7"})).is_err());
    }

    /// The volume of a part's bodies (the part's component, by its name).
    fn volume(s: &mut Session, part: &str) -> f64 {
        let comp = s.doc.components.iter().find(|c| c.name == part).map(|c| c.id).unwrap_or(0);
        let bodies: Vec<String> = s.model.state().bodies.iter().filter(|b| s.doc.model_body_component(b) == comp).map(|b| b.name.clone()).collect();
        let m = run(s, "inspect.measure", json!({"bodies": bodies}));
        m["total"]["volume_mm3"].as_f64().unwrap_or_else(|| panic!("{m}"))
    }

    /// A series-5 2020 extrusion: 20 x 20 with a T-slot in each face and the core bore, any
    /// length; its cross-section is the square less four slots and the bore (#106).
    #[test]
    fn extrusions_have_the_t_slot_profile() {
        let (open, lip, chamber, straight, depth, bore) = (6.2, 1.8, 11.0, 3.0, 6.1, 4.2);
        // The opening, the straight part of the chamber, then its 45° narrowing.
        let floor = chamber - 2.0 * (depth - straight);
        let slot = open * lip + chamber * (straight - lip) + (chamber + floor) / 2.0 * (depth - straight);
        let core = std::f64::consts::PI * (bore / 2.0) * (bore / 2.0);
        for (size, len, area) in [("2020", 100.0, 400.0 - 4.0 * slot - core), ("2040", 250.0, 800.0 - 6.0 * slot - 2.0 * core)] {
            let mut s = Session::default();
            let r = run(&mut s, "parts.insert", json!({"family": "extrusion", "size": size, "length": len}));
            let name = r["name"].as_str().unwrap_or_default().to_string();
            assert_eq!(name, format!("Extrusion {size}x{len}"));
            let (lo, hi) = bbox(&s, &name);
            let w = if size == "2020" { 20.0 } else { 40.0 };
            assert!(
                (hi.x - lo.x - 20.0).abs() < 1e-6 && (hi.y - lo.y - w).abs() < 1e-6 && (lo.z).abs() < 1e-6 && (hi.z - len).abs() < 1e-6,
                "{lo:?} {hi:?}"
            );
            let v = volume(&mut s, &name);
            // Within 1e-4 of the part: the round bore measures a little under π r² (2e-4 of it).
            assert!((v - area * len).abs() < 1e-4 * area * len, "{size}: {v} mm³, expected {}", area * len);
        }
        // 1.9 cm² for a 2020 (the makers' hollow core is left solid).
        assert!((400.0 - 4.0 * slot - core - 190.75).abs() < 0.01, "{}", 400.0 - 4.0 * slot - core);
    }

    /// A NEMA 17 x 40: the 42.3 square body up from its mounting face, the Ø22 pilot and Ø5 shaft
    /// down, four M3 holes on the 31 mm square, and the datasheet's ratings on the part (#106).
    #[test]
    fn a_stepper_has_the_nema_envelope_and_its_ratings() {
        let mut s = Session::default();
        let r = run(&mut s, "parts.insert", json!({"family": "stepper", "size": "NEMA17-40"}));
        let comp = r["component"].as_u64().unwrap_or(0);
        let (lo, hi) = bbox(&s, "Stepper NEMA17-40");
        assert!((hi.x - lo.x - 42.3).abs() < 1e-6 && (hi.y - lo.y - 42.3).abs() < 1e-6, "{lo:?} {hi:?}");
        assert!((hi.z - 40.0).abs() < 1e-6 && (lo.z + 24.0).abs() < 1e-6, "body up 40, shaft down 24: {lo:?} {hi:?}");
        let threads: Vec<_> = s.model.state().threads.iter().filter(|t| t.designation.starts_with("M3")).cloned().collect();
        assert_eq!(threads.len(), 4, "{threads:?}");
        let m = run(&mut s, "component.metadata", json!({"component": comp}));
        let ratings = &s.doc.components.iter().find(|c| c.id == comp).map(|c| c.metadata.0.get(super::RATINGS_KEY).cloned()).unwrap_or_default();
        assert_eq!(ratings.as_ref().and_then(|r| r["holding_torque_nm"].as_f64()), Some(0.42), "{m}");
        let lib = run(&mut s, "parts.library", json!({"family": "stepper"}));
        let n17 = lib["sizes"].as_array().and_then(|a| a.iter().find(|x| x["size"] == "NEMA17-40")).cloned().unwrap_or_default();
        assert_eq!((n17["hole_pitch"].as_f64(), n17["pilot"].as_f64(), n17["shaft"].as_f64()), (Some(31.0), Some(22.0), Some(5.0)));
        assert!(s.execute("parts.insert", &json!({"family": "stepper", "size": "NEMA42"})).is_err());
        assert!(s.execute("parts.insert", &json!({"family": "extrusion", "size": "2525"})).is_err());
    }

    /// Picked on the rim of its Ø22 pilot hole, a stepper sits on the plate with its pilot and
    /// shaft through the hole.
    #[test]
    fn a_stepper_seats_in_its_pilot_hole() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 60, "width": 60, "height": 5, "body_name": "Plate"}));
        run(&mut s, "solid.hole", json!({"position": [30, 30, 5], "diameter": 22.2}));
        let r = run(&mut s, "parts.insert", json!({"family": "stepper", "size": "NEMA17-40", "at": [41.1, 30, 5]}));
        assert!(r["joint"].is_number(), "{r}");
        let (lo, hi) = bbox(&s, "Stepper NEMA17-40");
        // The body on the top face, the shaft through the hole below: the part's −z goes in.
        assert!((hi.z - (5.0 + 40.0)).abs() < 1e-6 && (lo.z - (5.0 - 24.0)).abs() < 1e-6, "{lo:?} {hi:?}");
        assert!(((lo.x + hi.x) / 2.0 - 30.0).abs() < 1e-6 && ((lo.y + hi.y) / 2.0 - 30.0).abs() < 1e-6);
    }

    fn ratings(s: &Session, comp: u64) -> Value {
        s.doc.components.iter().find(|c| c.id == comp).and_then(|c| c.metadata.0.get(super::RATINGS_KEY).cloned()).unwrap_or_default()
    }

    /// A 20-tooth GT2 pulley on a 5 mm bore: pitch diameter 40/π, 0.254 mm deeper teeth, a groove
    /// for each tooth, its belt travel per degree on the part (#106).
    #[test]
    fn a_gt2_pulley_has_its_teeth_and_belt_ratio() {
        let mut s = Session::default();
        let r = run(&mut s, "parts.insert", json!({"family": "gt2_pulley", "size": "20T-5"}));
        let comp = r["component"].as_u64().unwrap_or(0);
        let pd = 40.0 / std::f64::consts::PI;
        let (lo, hi) = bbox(&s, "GT2 Pulley 20T-5");
        assert!((lo.z).abs() < 1e-6 && (hi.z - 16.0).abs() < 1e-6, "{lo:?} {hi:?}");
        // The flanges are the widest: outside diameter + 4.
        assert!((hi.x - lo.x - (pd - 0.508 + 4.0)).abs() < 1e-3, "{lo:?} {hi:?}");
        let rt = ratings(&s, comp);
        assert!(
            (rt["pitch_diameter_mm"].as_f64().unwrap_or(0.0) - pd).abs() < 1e-12
                && (rt["belt_mm_per_deg"].as_f64().unwrap_or(0.0) - 40.0 / 360.0).abs() < 1e-12,
            "{rt}"
        );
        // The teeth are cut: less than the plain band's volume, by 20 grooves' worth at most.
        let band = std::f64::consts::PI * (((pd - 0.508) / 2.0).powi(2) - 2.5f64.powi(2)) * 7.0;
        let flanges = std::f64::consts::PI * (((pd - 0.508 + 4.0) / 2.0).powi(2) - 2.5f64.powi(2)) * 2.0;
        let hub = std::f64::consts::PI * (((pd - 0.508 + 3.5) / 2.0).powi(2) - 2.5f64.powi(2)) * 7.0;
        let v = volume(&mut s, "GT2 Pulley 20T-5");
        let groove = std::f64::consts::PI * 0.555 * 0.555 * 7.0;
        assert!(v < band + flanges + hub - 1.0 && v > band + flanges + hub - 20.0 * groove, "{v} against {}", band + flanges + hub);
    }

    /// An MGN12 rail 300 long: 12 x 8 with its ball tracks and twelve counterbored holes on the
    /// 25 mm pitch; its H carriage, inserted where the rail was, rides it without touching (#106).
    #[test]
    fn an_mgn_carriage_rides_its_rail() {
        let mut s = Session::default();
        run(&mut s, "parts.insert", json!({"family": "linear_rail", "size": "MGN12", "length": 300}));
        let (lo, hi) = bbox(&s, "Rail MGN12x300");
        assert!(
            (hi.x - lo.x - 12.0).abs() < 1e-6 && (lo.y).abs() < 1e-6 && (hi.y - 8.0).abs() < 1e-6 && (hi.z - 300.0).abs() < 1e-6,
            "{lo:?} {hi:?}"
        );
        let pi = std::f64::consts::PI;
        let holes = 12.0 * (pi * 1.75 * 1.75 * (8.0 - 4.5) + pi * 3.0 * 3.0 * 4.5);
        let rail = (12.0 * 8.0 - 2.0 * 1.0 * 1.0) * 300.0 - holes;
        let v = volume(&mut s, "Rail MGN12x300");
        assert!((v - rail).abs() < 1e-4 * rail, "{v} against {rail}");
        run(&mut s, "parts.insert", json!({"family": "linear_carriage", "size": "MGN12H"}));
        let (lo, hi) = bbox(&s, "Carriage MGN12H");
        assert!(
            (hi.x - lo.x - 27.0).abs() < 1e-6 && (lo.y - 3.0).abs() < 1e-6 && (hi.y - 13.0).abs() < 1e-6 && (hi.z - 45.4).abs() < 1e-6,
            "{lo:?} {hi:?}"
        );
        let i = run(&mut s, "inspect.interference", json!({}));
        assert_eq!(i["count"], 0, "{i}");
        assert!(s.execute("parts.insert", &json!({"family": "linear_rail", "size": "MGN20"})).is_err());
    }

    /// A T8 lead screw and its flanged nut, the nut seated in its hole like a screw, with the lead
    /// on both for a screw joint (#106).
    #[test]
    fn a_t8_nut_seats_and_carries_its_lead() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 6, "body_name": "Plate"}));
        run(&mut s, "solid.hole", json!({"position": [20, 20, 6], "diameter": 10.4}));
        let r = run(&mut s, "parts.insert", json!({"family": "lead_nut", "size": "T8", "at": [25.2, 20, 6]}));
        assert!(r["joint"].is_number(), "{r}");
        let (lo, hi) = bbox(&s, "Lead Nut T8");
        assert!((hi.z - (6.0 + 3.5)).abs() < 1e-6 && (lo.z - (6.0 - 11.5)).abs() < 1e-6 && (hi.x - lo.x - 22.0).abs() < 1e-6, "{lo:?} {hi:?}");
        assert_eq!(ratings(&s, r["component"].as_u64().unwrap_or(0))["lead_mm"].as_f64(), Some(8.0));
        let screw = run(&mut s, "parts.insert", json!({"family": "lead_screw", "size": "T8", "length": 350}));
        assert_eq!(screw["name"], "Lead Screw T8x350");
        assert!(s.execute("parts.insert", &json!({"family": "lead_screw", "size": "T10"})).is_err());
    }

    #[test]
    fn a_screw_seats_in_a_hole_and_resizes() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 10, "body_name": "Plate"}));
        run(&mut s, "solid.hole", json!({"position": [20, 15, 10], "diameter": 6.4}));
        // Pick the hole's rim at the top face.
        let r = run(&mut s, "parts.insert", json!({"family": "socket_head_cap_screw", "size": "M6", "length": 20, "at": [23.2, 15, 10]}));
        assert!(r["joint"].is_number(), "{r}");
        let (lo, hi) = bbox(&s, "SHCS M6x20");
        // The head sits flush on the top face, the shank down the hole's axis.
        assert!((hi.z - 16.0).abs() < 1e-6 && (lo.z + 10.0).abs() < 1e-6, "{lo:?} {hi:?}");
        assert!(((lo.x + hi.x) / 2.0 - 20.0).abs() < 1e-6 && ((lo.y + hi.y) / 2.0 - 15.0).abs() < 1e-6);
        // M8 x 25: rebuilt in place, still seated.
        let comp = r["component"].as_u64().unwrap_or(0);
        run(&mut s, "parts.resize", json!({"component": comp, "size": "M8", "length": 25}));
        let (lo, hi) = bbox(&s, "SHCS M8x25");
        assert!((hi.z - 18.0).abs() < 1e-6 && (lo.z + 15.0).abs() < 1e-6, "{lo:?} {hi:?}");
        assert!(s.model.state().threads.iter().any(|t| t.designation == "M8" && (t.pitch - 1.25).abs() < 1e-9));
    }

    /// A hole picked at its centre (where no face is) seats like a rim pick (#134).
    #[test]
    fn a_screw_seats_on_a_hole_picked_at_its_centre() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 10, "body_name": "Plate"}));
        run(&mut s, "solid.hole", json!({"position": [20, 15, 10], "diameter": 6.4}));
        let r = run(&mut s, "parts.insert", json!({"family": "socket_head_cap_screw", "size": "M6", "length": 20, "at": [20, 15, 10]}));
        assert!(r["joint"].is_number(), "{r}");
        let (lo, hi) = bbox(&s, "SHCS M6x20");
        assert!((hi.z - 16.0).abs() < 1e-6 && (lo.z + 10.0).abs() < 1e-6, "{lo:?} {hi:?}");
        assert!(((lo.x + hi.x) / 2.0 - 20.0).abs() < 1e-6 && ((lo.y + hi.y) / 2.0 - 15.0).abs() < 1e-6);
    }

    /// The part a screw seats in is the one with the hole, not the first whose bounding box holds
    /// the pick: here a stepper's box (its shaft reaches down past the plate) holds the plate's
    /// hole, and the stepper comes first (#134, the D2 printer's Z motor mounts).
    #[test]
    fn a_screw_seats_in_the_part_with_the_hole_not_a_bigger_neighbour() {
        let mut s = Session::default();
        run(&mut s, "parts.insert", json!({"family": "stepper", "size": "NEMA17-40"}));
        run(&mut s, "solid.box", json!({"corner": [0, 0, -5], "length": 30, "width": 30, "height": 5, "operation": "new", "body_name": "Mount"}));
        run(&mut s, "solid.hole", json!({"position": [10, 10, 0], "diameter": 3.4}));
        for at in [json!([10, 10, 0]), json!([11.7, 10, 0])] {
            let r = run(
                &mut s,
                "parts.insert",
                json!({"family": "socket_head_cap_screw", "size": "M3", "length": 10, "at": at, "name": format!("S{at}")}),
            );
            assert!(r["joint"].is_number(), "{at}: {r}");
            let (lo, hi) = bbox(&s, &format!("S{at}"));
            assert!((hi.z - 3.0).abs() < 1e-6 && (lo.z + 10.0).abs() < 1e-6, "{at}: {lo:?} {hi:?}");
            assert!(((lo.x + hi.x) / 2.0 - 10.0).abs() < 1e-6 && ((lo.y + hi.y) / 2.0 - 10.0).abs() < 1e-6, "{at}: {lo:?} {hi:?}");
        }
    }

    /// A screw seats on a washer that is itself seated in the hole: the pick is in world
    /// coordinates, the washer's hole in its placed component's.
    #[test]
    fn a_screw_seats_on_a_seated_washer() {
        let mut s = Session::default();
        run(&mut s, "solid.box", json!({"length": 40, "width": 30, "height": 10, "body_name": "Plate"}));
        run(&mut s, "solid.hole", json!({"position": [20, 15, 10], "diameter": 6.4}));
        let l = run(&mut s, "parts.library", json!({"family": "washer"}));
        let m6 = l["sizes"].as_array().and_then(|a| a.iter().find(|x| x["size"] == "M6")).cloned().unwrap_or_default();
        let (inner, t) = (m6["inner"].as_f64().unwrap_or(0.0), m6["thickness"].as_f64().unwrap_or(0.0));
        run(&mut s, "parts.insert", json!({"family": "washer", "size": "M6", "at": [23.2, 15, 10]}));
        let (lo, hi) = bbox(&s, "Washer M6");
        assert!((lo.z - 10.0).abs() < 1e-6 && (hi.z - 10.0 - t).abs() < 1e-6, "the washer sits on the plate: {lo:?} {hi:?}");
        // The washer's top rim, picked in world coordinates.
        let r = run(
            &mut s,
            "parts.insert",
            json!({"family": "socket_head_cap_screw", "size": "M6", "length": 16, "at": [20.0 + inner / 2.0, 15, 10.0 + t]}),
        );
        assert!(r["joint"].is_number(), "{r}");
        let (lo, hi) = bbox(&s, "SHCS M6x16");
        assert!((hi.z - (10.0 + t + 6.0)).abs() < 1e-6 && (lo.z - (10.0 + t - 16.0)).abs() < 1e-6, "the head sits on the washer: {lo:?} {hi:?}");
        assert!(((lo.x + hi.x) / 2.0 - 20.0).abs() < 1e-6 && ((lo.y + hi.y) / 2.0 - 15.0).abs() < 1e-6, "on the hole's axis");
    }

    /// A bearing presses into the bore it is seated on, whichever rim is picked; library parts come
    /// in Steel, their body named as the part (not Body1).
    #[test]
    fn a_bearing_fills_its_pocket_in_steel_named_for_the_part() {
        // The pocket's top rim, then (in a second design) its floor rim.
        for rim in [[11.0, 0.0, 20.0], [11.0, 0.0, 13.0]] {
            let mut t = Session::default();
            run(&mut t, "solid.box", json!({"length": 60, "width": 60, "height": 20, "corner": [-30, -30, 0], "body_name": "Housing"}));
            run(&mut t, "solid.cylinder", json!({"base": [0, 0, 13], "axis": [0, 0, 1], "radius": 11, "height": 7, "operation": "cut"}));
            run(&mut t, "parts.insert", json!({"family": "bearing", "size": "608", "at": rim}));
            let (lo, hi) = bbox(&t, "Bearing 608");
            assert!((lo.z - 13.0).abs() < 1e-6 && (hi.z - 20.0).abs() < 1e-6, "rim {rim:?}: the bearing fills the 7 mm pocket: {lo:?} {hi:?}");
            let names: Vec<String> = t.world_state().bodies.iter().map(|b| b.name.clone()).collect();
            assert!(names.iter().any(|n| n == "Bearing 608"), "{names:?}");
            assert_eq!(t.doc.materials.get("Bearing 608").map(String::as_str), Some("Steel"));
        }
        // A second screw of the same size gets a name of its own.
        let mut u = Session::default();
        run(&mut u, "parts.insert", json!({"family": "socket_head_cap_screw", "size": "M4", "length": 12}));
        run(&mut u, "parts.insert", json!({"family": "socket_head_cap_screw", "size": "M4", "length": 12}));
        let names: Vec<String> = u.world_state().bodies.iter().map(|b| b.name.clone()).collect();
        assert!(names.contains(&"SHCS M4x12".to_string()) && names.contains(&"SHCS M4x12 (2)".to_string()), "{names:?}");
    }

    #[test]
    fn resizing_placed_parts_uses_local_geometry_and_millimetres_in_every_design_unit() {
        for (family, initial, resized, prefix) in [
            ("socket_head_cap_screw", "M6", "M8", "SHCS"),
            ("hex_bolt", "M6", "M8", "Hex Bolt"),
            ("hex_nut", "M6", "M8", "Hex Nut"),
            ("washer", "M6", "M8", "Washer"),
            ("set_screw", "M6", "M8", "Set Screw"),
            ("dowel_pin", "6", "8", "Dowel Pin"),
            ("bearing", "608", "6200", "Bearing"),
        ] {
            let mut bounds = Vec::new();
            for units in ["mm", "in"] {
                let mut s = Session::default();
                run(&mut s, "document.units", json!({"units":units}));
                let part = run(&mut s, "parts.insert", json!({"family":family,"size":initial,"length":20,"point":[40,50,60],"direction":[1,0,0]}));
                run(&mut s, "parts.resize", json!({"component":part["component"],"size":resized,"length":30}));
                assert!(s.model.results.iter().all(|r| r.error.is_none()), "{family} {units}");
                let b = bbox(&s, prefix);
                assert!(b.0.x < b.1.x && b.0.y < b.1.y && b.0.z < b.1.z, "{family} {units}: {b:?}");
                if matches!(family, "socket_head_cap_screw" | "hex_bolt" | "hex_nut" | "set_screw") {
                    assert!(s.model.state().threads.iter().any(|t| t.designation == "M8" && (t.pitch - 1.25).abs() < 1e-9), "{family} {units}");
                }
                if family == "dowel_pin" {
                    assert!(b.0.dist(solidcraft_geom::Vec3::new(10., 46., 56.)) < 1e-6, "{b:?}");
                    assert!(b.1.dist(solidcraft_geom::Vec3::new(40., 54., 64.)) < 1e-6, "{b:?}");
                }
                bounds.push(b);
            }
            assert!(bounds[0].0.dist(bounds[1].0) < 1e-6 && bounds[0].1.dist(bounds[1].1) < 1e-6, "{family}: {bounds:?}");
        }
    }

    #[test]
    fn resizing_a_part_is_one_undo_and_keeps_the_active_occurrence() {
        let mut s = Session::default();
        let part = run(&mut s, "parts.insert", json!({"family":"socket_head_cap_screw","size":"M6","length":20}));
        let id = part["component"].as_u64().unwrap_or(0);
        let copied = run(&mut s, "occurrence.copy", json!({"component":id,"translate":[40,0,0]}));
        let path = vec![copied["occurrence"].as_u64().unwrap_or(0)];
        run(&mut s, "component.activate", json!({"component":id,"occurrence":copied["occurrence"],"path":path}));
        let before = s.doc.clone();
        let undo = s.undo.len();
        run(&mut s, "parts.resize", json!({"component":id,"size":"M8","length":25}));
        assert_eq!(s.undo.len(), undo + 1);
        assert_eq!(s.active_component, id);
        assert_eq!(s.active_occurrence_path, Some(path.clone()));
        let after = s.doc.clone();
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(s.doc, before);
        let redo = s.redo.len();
        assert!(s.execute("parts.resize", &json!({"component":id,"length":0})).is_err());
        assert_eq!(s.doc, before);
        assert_eq!(s.redo.len(), redo);
        assert_eq!(s.active_occurrence_path, Some(path));
        run(&mut s, "edit.redo", json!({}));
        assert_eq!(s.doc, after);
    }

    #[test]
    fn resizing_a_part_at_the_history_limit_drops_only_one_old_step() {
        let mut s = Session::default();
        let part = run(&mut s, "parts.insert", json!({"family":"washer","size":"M6"}));
        let Some(snapshot) = s.undo.last().cloned() else { panic!("insert did not record history") };
        s.undo = (0..crate::MAX_UNDO)
            .map(|index| {
                let mut old = snapshot.clone();
                old.label = format!("Earlier step {index}");
                old
            })
            .collect();
        run(&mut s, "parts.resize", json!({"component":part["component"],"size":"M8"}));
        assert_eq!(s.undo.len(), crate::MAX_UNDO);
        assert_eq!(s.undo.first().map(|step| step.label.as_str()), Some("Earlier step 1"));
        assert_eq!(s.undo.last().map(|step| step.label.as_str()), Some("Change Part Size"));
    }
}

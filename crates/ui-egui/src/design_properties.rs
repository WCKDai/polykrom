//! The Properties panel (design kit views 01 and 03) and the Design Properties window (#1015).
//!
//! The panel stands at the right of the workspace, where the AI review panel and the body
//! inspector stand in turn. With one timeline feature selected it shows the feature: its kind
//! and name, its real parameters (`FeatureKind::inputs`), extent, instances, driving sketch and
//! body, **Edit feature** and **Edit sketch**, and the features that depend on it as the model
//! reports them (`timeline.dependents`). **Edit feature** turns the panel into the editor in
//! place: the editable parameters become fields, a Preview shows the change in the viewport
//! through the feature-preview machinery, a consequence summary is computed from the model, and
//! **Apply change** issues exactly the command the feature's dialog would issue
//! ([`crate::dialogs::apply_commands`]), as one undo step. Cancel and Esc leave the design, its
//! revision and the history alone: the editor never rolls the timeline, it fills the dialog from
//! a scratch copy of the session rolled back to just before the feature ([`build_dialog`]).
//! Features whose dialog has no plain value fields (or no dialog at all) keep their existing way
//! of editing, and the panel says which it is ([`Edits`]). The actions, their note and the
//! footer line stand at the card's bottom; the body above them scrolls when the window is short
//! ([`contents`]).
//!
//! With nothing selected the panel shows the design's properties (project, part number,
//! revision, author, organization, description and licence) in the same layout; **Edit
//! properties** opens the Design Properties window below.
//!
//! The Design Properties window: OK applies the edited fields through one `document.metadata.set`
//! (one undo step); Cancel and the close button change nothing. File → Design Properties…, the
//! Browser's Document Settings, the root component's menu and `ui.start` of `document.metadata`
//! or `document.metadata.set` open it. [`show_object`] draws the Properties card (mass, volume,
//! area, centre of mass, bounding box, counts) of the body or component a menu asked about; both
//! are `ui_kit::Window` cards.

use egui::{Align2, Id, Rect, pos2, vec2};
use polykrom_engine::doc::expr::Kind as ValueKind;
use polykrom_engine::doc::metadata::{DESCRIPTION, WELL_KNOWN};
use polykrom_engine::doc::{FeatureKind, PatternKind};
use serde_json::{Map, Value, json};

use crate::SolveApp;
use crate::dialogs::{self, Dialog, Kind};
use crate::i18n::{format as trf, tr, tr_label, tr_name};
use crate::theme::Tokens;
use crate::ui_kit::{self, Container, Edge, Surface};

/// Longest preview of another tool's value in the list.
const PREVIEW_CHARS: usize = 80;
/// Rows of other tools' data shown before the list scrolls.
const OTHERS_ROWS: f32 = 6.0;
/// The panel's width (logical points): the review panel's, which stands in its place.
pub const PANEL_WIDTH: f32 = crate::review::PANEL_WIDTH;
/// The most dependent features named on one line of the consequence summary.
const MAX_NAMES: usize = 6;
/// The label column's share of a row, from design kit view 03 (its layer JSON: the Diameter row
/// of the feature inspector is 264 wide, its label 104). The column grows to the widest label of
/// the rows shown, so no label is elided ([`ui_kit::labelled_form`]).
const LABEL_SHARE: f32 = 104.0 / 264.0;

// ---- The Properties panel -----------------------------------------------------------------------

/// How **Edit feature** edits a feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Edits {
    /// In the panel: its dialog has plain value fields the panel shows in place.
    Panel,
    /// In its existing dialog (picks, choices or tables the panel does not show).
    Dialog,
    /// No dialog yet: the command palette opens with `timeline.edit`, as the timeline does.
    Palette,
    /// A sketch: Edit sketch opens it.
    Sketch,
}

/// The panel's state.
#[derive(Default)]
pub struct Panel {
    /// A feature being edited in place.
    pub editor: Option<Editor>,
    /// `timeline.dependents` of a feature at a design revision: the dependent features' ids.
    dependents: Option<(u64, u64, Vec<u64>)>,
    /// How a feature is edited, at a design revision (filling its dialog is not free).
    edits: Option<(u64, u64, Edits)>,
    /// The bodies of a feature ([`body_names`]) at a design revision: finding the body under a
    /// hole without a named body scans the model before the hole, not something to do per frame.
    bodies: Option<(u64, u64, Vec<String>)>,
    /// The panel's rect this frame (floating, it covers the viewport for input).
    pub rect: Option<Rect>,
}

/// A feature edited in place (design kit view 03).
pub struct Editor {
    pub feature: u64,
    /// The feature's dialog as **Edit feature** would open it, filled from the feature; the fields
    /// edit its values, and Apply runs its commands.
    pub dialog: Dialog,
    /// Preview the change in the viewport.
    pub preview: bool,
    /// Why Apply was refused.
    pub error: Option<String>,
    /// The design revision the editor opened on: a design changed under it drops it.
    pub revision: u64,
}

/// The one selected timeline feature, if exactly one is selected.
pub fn selected_feature(app: &SolveApp) -> Option<u64> {
    match crate::timeline::selected_feature_ids(app).as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// A feature is being edited in the panel.
pub fn editing(app: &SolveApp) -> bool {
    app.properties.editor.is_some()
}

/// The commands the panel's editor previews (Preview on, inputs valid), for `preview::update`.
pub fn preview_commands(app: &SolveApp) -> Option<Vec<(String, Value)>> {
    let ed = app.properties.editor.as_ref().filter(|ed| ed.preview)?;
    if !fields_valid(app, &ed.dialog) {
        return None;
    }
    dialogs::preview_commands(app, &ed.dialog).ok().filter(|c| !c.is_empty())
}

/// The dialog **Edit feature** would open for `id`, filled in, without touching the session:
/// the dialog fills its picks from the model just before the feature, so a scratch copy of the
/// session is rolled back to there and stands in while the dialog is built. The design, its
/// revision, the selection and the history stay as they are.
fn build_dialog(app: &mut SolveApp, id: u64) -> Option<Dialog> {
    let idx = app.session.doc.feature_index(id)?;
    let marker = app.session.doc.marker;
    let mut scratch = app.session.scratch();
    scratch.execute("timeline.roll_to", &json!({ "position": idx })).ok()?;
    let real = std::mem::replace(&mut app.session, scratch);
    let dialog = dialogs::for_feature(app, id, marker);
    app.session = real;
    dialog
}

/// The plain value fields of a feature dialog the panel edits in place: (label, kind, text), in
/// the dialog's order. Picks, choices and tables stay with the dialog. A field's label is the
/// model's for that value (`FeatureKind::inputs`), whatever its dialog calls it: the editor
/// shows the model's inputs that are not fields read-only, by label, so a value is never shown
/// twice.
pub(crate) fn fields(d: &mut Dialog) -> Vec<(&'static str, ValueKind, &mut String)> {
    use ValueKind::{Angle, Length, Unitless};
    let to_object = dialogs::to_object_on(&d.inputs);
    let second_direction = d.inputs.get(2).is_some_and(|i| !i.items.is_empty()) || d.extra.contains_key("dir2");
    let fillet_radius = fillet_radius_label(d);
    let mut v: Vec<(&'static str, ValueKind, &mut String)> = Vec::new();
    match &mut d.kind {
        Kind::Hole { diameter, depth, kind, cb_diameter, cb_depth, cs_diameter, cs_angle, opts } => {
            if !opts.tapped {
                v.push(("Diameter", Length, diameter));
            }
            if !opts.all && !to_object {
                v.push(("Depth", Length, depth));
            }
            match *kind {
                1 => {
                    v.push(("Counterbore diameter", Length, cb_diameter));
                    v.push(("Counterbore depth", Length, cb_depth));
                }
                2 => {
                    v.push(("Countersink diameter", Length, cs_diameter));
                    v.push(("Countersink angle", Angle, cs_angle));
                }
                _ => {}
            }
        }
        Kind::Extrude { distance, direction, distance2, taper, taper_present, to_offset, start, all, thin, wall, wall_side, wall2, .. } => {
            if to_object {
                // An empty offset is none: nothing to edit in place.
                if !to_offset.is_empty() {
                    v.push(("To offset", Length, to_offset));
                }
            } else if !*all {
                v.push(("Distance", Length, distance));
            }
            if dialogs::two_sided(*direction) {
                v.push(("Distance 2", Length, distance2));
            }
            if !start.is_empty() {
                v.push(("Start offset", Length, start));
            }
            if *taper_present {
                v.push(("Taper angle", Angle, taper));
            }
            if *thin {
                v.push(("Thickness", Length, wall));
                if dialogs::WALL_SIDES.get(*wall_side) == Some(&"two_sides") {
                    v.push(("Thickness 2", Length, wall2));
                }
            }
        }
        Kind::Revolve { angle, .. } => v.push(("Angle", Angle, angle)),
        Kind::Fillet { radius, chamfer, ctype, distance2, angle, .. } => {
            v.push((if *chamfer { "Distance" } else { fillet_radius }, Length, radius));
            if *chamfer {
                match *ctype {
                    1 => v.push(("Distance 2", Length, distance2)),
                    2 => v.push(("Angle", Angle, angle)),
                    _ => {}
                }
            }
        }
        Kind::Shell { thickness } => v.push(("Thickness", Length, thickness)),
        Kind::Draft { angle } => v.push(("Angle", Angle, angle)),
        Kind::OffsetPlane { offset } => v.push(("Offset", Length, offset)),
        Kind::AnglePlane { angle } => v.push(("Angle", Angle, angle)),
        Kind::Scale { factor } => v.push(("Scale factor", Unitless, factor)),
        Kind::OffsetFaces { distance } => v.push(("Distance", Length, distance)),
        Kind::Stock { margin } => v.push(("Margin", Length, margin)),
        Kind::Pipe { diameter, wall } => {
            v.push(("Diameter", Length, diameter));
            if !wall.is_empty() {
                v.push(("Wall", Length, wall));
            }
        }
        Kind::Emboss { depth, .. } => v.push(("Depth", Length, depth)),
        Kind::PatternRect { count, spacing, count2, spacing2 } => {
            v.push(("Quantity", Unitless, count));
            v.push(("Spacing", Length, spacing));
            if second_direction {
                v.push(("Quantity 2", Unitless, count2));
                v.push(("Spacing 2", Length, spacing2));
            }
        }
        Kind::PatternCirc { count, angle } => {
            v.push(("Quantity", Unitless, count));
            v.push(("Total angle", Angle, angle));
        }
        Kind::PathPattern { count, spacing } => {
            v.push(("Quantity", Unitless, count));
            v.push(("Distance", Length, spacing));
        }
        Kind::Primitive { fields, .. } => {
            for (name, value) in fields.iter_mut() {
                let label = match *name {
                    "length" => "Length",
                    "width" => "Width",
                    "height" => "Height",
                    "radius" => "Radius",
                    "major" => "Major radius",
                    "minor" => "Minor radius",
                    other => other,
                };
                v.push((label, Length, value));
            }
        }
        Kind::Rib { thickness, depth, .. } => {
            v.push(("Thickness", Length, thickness));
            if !depth.is_empty() {
                v.push(("Depth", Length, depth));
            }
        }
        Kind::MeshConvert { tolerance } => v.push(("Tolerance", Length, tolerance)),
        _ => {}
    }
    v
}

/// What a fillet dialog's radius is in the model: the label of the feature's `radius` for its
/// style ([`polykrom_engine::doc::FilletStyle::radius_label`]), which the dialog carries as its
/// `type` (none for a constant radius).
fn fillet_radius_label(d: &Dialog) -> &'static str {
    use polykrom_engine::doc::FilletStyle;
    match d.extra.get("type").and_then(Value::as_str) {
        Some("curvature") => FilletStyle::Curvature.radius_label(),
        Some("chord") => FilletStyle::Chord.radius_label(),
        // A variable or spline fillet: the radius at the start.
        Some("variable" | "spline") => "Start radius",
        _ => FilletStyle::Constant.radius_label(),
    }
}

/// Why a field's text is not a value of its kind (a whole number of at least one for a count).
fn field_error(app: &SolveApp, text: &str, kind: ValueKind) -> Option<String> {
    match app.session.doc.eval(text, kind) {
        Err(e) => Some(e.to_string()),
        Ok(v) if kind == ValueKind::Unitless && (v < 1.0 || v.fract() != 0.0) => Some(tr("ui.properties.count_whole").to_string()),
        Ok(_) => None,
    }
}

/// Every field of the dialog holds a value of its kind.
fn fields_valid(app: &SolveApp, d: &Dialog) -> bool {
    let mut d = d.clone();
    fields(&mut d).into_iter().all(|(_, kind, text)| field_error(app, text, kind).is_none())
}

/// How **Edit feature** edits `id` (cached for the design revision).
pub fn edits(app: &mut SolveApp, id: u64) -> Edits {
    if let Some((f, r, e)) = app.properties.edits
        && f == id
        && r == app.session.revision
    {
        return e;
    }
    let e = if matches!(app.session.doc.feature(id).map(|f| &f.kind), Some(FeatureKind::Sketch { .. })) {
        Edits::Sketch
    } else {
        match build_dialog(app, id) {
            None => Edits::Palette,
            Some(mut d) => {
                if fields(&mut d).is_empty() {
                    Edits::Dialog
                } else {
                    Edits::Panel
                }
            }
        }
    };
    app.properties.edits = Some((id, app.session.revision, e));
    e
}

/// **Edit feature**: edit `id` in the panel when its dialog has plain value fields, else the
/// way the timeline edits it ([`SolveApp::edit_feature`]: its dialog, or the palette).
pub fn begin_edit(app: &mut SolveApp, id: u64) {
    app.properties.editor = None;
    if edits(app, id) == Edits::Panel
        && let Some(mut dialog) = build_dialog(app, id)
        && !fields(&mut dialog).is_empty()
    {
        dialog.preview_enabled = true;
        app.properties.editor = Some(Editor { feature: id, dialog, preview: true, error: None, revision: app.session.revision });
        return;
    }
    app.edit_feature(id);
}

/// **Apply change**: run the dialog's commands (one undo step). A refusal keeps the editor open
/// with the reason and the design as it was.
pub fn apply(app: &mut SolveApp) -> Result<(), String> {
    let Some(mut ed) = app.properties.editor.take() else { return Err("nothing is being edited".into()) };
    if !fields_valid(app, &ed.dialog) {
        ed.error = Some(tr("ui.properties.invalid").to_string());
        app.properties.editor = Some(ed);
        return Err("invalid value".into());
    }
    let marker = ed.dialog.editing.and_then(|(_, m)| m);
    match dialogs::run_dialog(app, &ed.dialog) {
        Ok(()) => Ok(()),
        Err(e) => {
            // A refused edit rolls the timeline to the feature: back to where it was.
            let _ = dialogs::roll_marker(app, marker);
            ed.error = Some(e.clone());
            ed.revision = app.session.revision;
            app.properties.editor = Some(ed);
            Err(e)
        }
    }
}

/// **Cancel edit** (and Esc): drop the editor. Nothing was changed, so nothing is restored.
pub fn cancel(app: &mut SolveApp) {
    app.properties.editor = None;
}

/// The features that depend on `id` as the model reports them (`timeline.dependents`: deleted
/// with it, or failing without it), in timeline order; cached for the design revision.
pub fn dependents(app: &mut SolveApp, id: u64) -> Vec<u64> {
    let revision = app.session.revision;
    if let Some((f, r, ids)) = &app.properties.dependents
        && *f == id
        && *r == revision
    {
        return ids.clone();
    }
    let reply = app.session.execute("timeline.dependents", &json!({ "feature": id })).unwrap_or(Value::Null);
    let names = |key: &str| reply[key].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect::<Vec<_>>();
    let mut named = names("deleted_with_it");
    named.extend(names("would_fail"));
    let doc = &app.session.doc;
    let mut ids: Vec<u64> = doc.features.iter().filter(|f| f.id != id && named.contains(&f.name)).map(|f| f.id).collect();
    ids.dedup();
    app.properties.dependents = Some((id, revision, ids.clone()));
    ids
}

/// What applying the edit of `id` does, in words, from the model alone: the dependent features
/// that rebuild, a pattern of the feature keeping its pitch, holes staying on their sketch points.
pub fn consequences(app: &mut SolveApp, id: u64) -> Vec<String> {
    let deps = dependents(app, id);
    let mut lines = Vec::new();
    let doc = &app.session.doc;
    let names: Vec<String> = deps.iter().filter_map(|d| doc.feature(*d)).map(|f| tr_name(&f.name)).collect();
    lines.push(match names.len() {
        0 => tr("ui.properties.rebuild_none").to_string(),
        1 => trf("ui.properties.rebuild_one", &[("name", &names[0])]),
        _ => trf("ui.properties.rebuild_many", &[("names", &joined(&names))]),
    });
    let Some(f) = doc.feature(id) else { return lines };
    for dep in &deps {
        let Some(p) = doc.feature(*dep) else { continue };
        let FeatureKind::Pattern { pattern, .. } = &p.kind else { continue };
        if !p.kind.names_feature(&f.name) {
            continue;
        }
        let name = tr_name(&p.name);
        match pattern {
            PatternKind::Rectangular { count1, spacing1, count2, spacing2, .. } => {
                let n = |e: &str| doc.eval(e, ValueKind::Unitless).ok().map(|v| trim(format!("{v:.0}")));
                let (Some(c1), Some(s1)) = (n(count1), doc.eval(spacing1, ValueKind::Length).ok()) else { continue };
                let (count, pitch) = match (count2.as_deref().and_then(n), spacing2.as_deref().and_then(|e| doc.eval(e, ValueKind::Length).ok())) {
                    (Some(c2), Some(s2)) => (format!("{c1} × {c2}"), format!("{} × {}", trim(bare_length(app, s1)), short_length(app, s2))),
                    _ => (c1, short_length(app, s1)),
                };
                lines.push(trf("ui.properties.pattern_rect", &[("name", &name), ("count", &count), ("pitch", &pitch)]));
            }
            PatternKind::Circular { count, angle, .. } => {
                let (Ok(c), Ok(a)) = (doc.eval(count, ValueKind::Unitless), doc.eval(angle, ValueKind::Angle)) else { continue };
                lines.push(trf("ui.properties.pattern_circ", &[("name", &name), ("count", &trim(format!("{c:.0}"))), ("angle", &angle_text(a))]));
            }
            PatternKind::Path { count, .. } => {
                let Ok(c) = doc.eval(count, ValueKind::Unitless) else { continue };
                lines.push(trf("ui.properties.pattern_path", &[("name", &name), ("count", &trim(format!("{c:.0}")))]));
            }
        }
    }
    if let FeatureKind::Hole { points: Some(sp), .. } = &f.kind
        && !sp.ids.is_empty()
        && let Some(sketch) = doc.feature(sp.sketch.0)
    {
        let sketch = tr_name(&sketch.name);
        lines.push(match sp.ids.len() {
            1 => trf("ui.properties.point_stays", &[("sketch", &sketch)]),
            n => trf("ui.properties.points_stay", &[("count", &n.to_string()), ("sketch", &sketch)]),
        });
    }
    lines
}

/// Names on one line: "A, B and C" (the first `MAX_NAMES`, then "and N more").
fn joined(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(MAX_NAMES).map(String::as_str).collect();
    let rest = names.len().saturating_sub(shown.len());
    match (shown.split_last(), rest) {
        (None, _) => String::new(),
        (Some((last, [])), 0) => (*last).to_string(),
        (Some((last, head)), 0) => trf("ui.properties.and", &[("list", &head.join(", ")), ("last", last)]),
        (Some(_), n) => trf("ui.properties.and_more", &[("list", &shown.join(", ")), ("count", &n.to_string())]),
    }
}

/// Trailing zeros dropped: "8.00" → "8", "8.50" → "8.5".
fn trim(s: String) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s }
}

/// A length in millimetres in the design's units, without the unit.
fn bare_length(app: &SolveApp, mm: f64) -> String {
    crate::prefs::show_mm_bare(app, mm)
}

/// A length in millimetres in the design's units, with the unit ("8.00 mm").
fn length(app: &SolveApp, mm: f64) -> String {
    format!("{} {}", bare_length(app, mm), crate::prefs::unit_scale(&app.session.doc.units).1)
}

/// A length in millimetres in the design's units, trailing zeros dropped ("84 mm").
fn short_length(app: &SolveApp, mm: f64) -> String {
    format!("{} {}", trim(bare_length(app, mm)), crate::prefs::unit_scale(&app.session.doc.units).1)
}

/// How many instances, in words ("4 instances", "1 instance").
fn instances_text(n: usize) -> String {
    if n == 1 { tr("ui.properties.instance_one").to_string() } else { trf("ui.properties.instances", &[("count", &n.to_string())]) }
}

/// An angle in radians, in degrees ("45°").
fn angle_text(rad: f64) -> String {
    format!("{}°", trim(format!("{:.2}", rad.to_degrees())))
}

/// An input's value as the panel shows it: evaluated, in the design's units; a diameter with Ø.
fn value_text(app: &SolveApp, label: &str, expr: &str, kind: ValueKind) -> String {
    let shown = match app.session.doc.eval(expr, kind) {
        Ok(v) => match kind {
            ValueKind::Length => length(app, v),
            ValueKind::Angle => angle_text(v),
            ValueKind::Unitless => trim(format!("{v:.3}")),
        },
        Err(_) => expr.to_string(),
    };
    if label == "Diameter" { format!("Ø{shown}") } else { shown }
}

/// The small-caps kind line: the feature's kind and its number among the features of that kind
/// ("HOLE 02").
fn kind_line(app: &SolveApp, id: u64) -> String {
    let doc = &app.session.doc;
    let Some(f) = doc.feature(id) else { return String::new() };
    let base = f.kind.base_name();
    let n = doc.features.iter().filter(|g| g.kind.base_name() == base).position(|g| g.id == id).map_or(1, |i| i + 1);
    format!("{} {n:02}", tr_label(base).to_uppercase())
}

/// The extent of a hole or an extrude, in words.
fn extent_text(kind: &FeatureKind) -> Option<String> {
    Some(match kind {
        FeatureKind::Hole { to: Some(_), .. } => tr("ui.properties.to_object").to_string(),
        FeatureKind::Hole { depth: None, .. } => tr("ui.properties.through_all").to_string(),
        FeatureKind::Hole { .. } => tr("Distance").to_string(),
        FeatureKind::Extrude { extent, .. } => {
            if extent.to.is_some() {
                tr("ui.properties.to_object").to_string()
            } else if extent.through_all {
                tr("ui.properties.through_all").to_string()
            } else if extent.distance2.is_some() {
                tr("ui.properties.two_sides").to_string()
            } else if extent.direction == polykrom_engine::doc::Direction::Symmetric {
                tr("ui.properties.symmetric").to_string()
            } else {
                tr("ui.properties.one_side").to_string()
            }
        }
        _ => return None,
    })
}

/// How many instances a feature places: a hole's sketch points, a pattern's copies.
fn instances(app: &SolveApp, kind: &FeatureKind) -> Option<usize> {
    let doc = &app.session.doc;
    match kind {
        FeatureKind::Hole { points: Some(sp), .. } if !sp.ids.is_empty() => Some(sp.ids.len()),
        FeatureKind::Pattern { pattern, .. } => {
            let n = |e: &str| doc.eval(e, ValueKind::Unitless).ok().filter(|v| v.is_finite() && *v >= 0.0).map(|v| v as usize);
            match pattern {
                PatternKind::Rectangular { count1, count2, .. } => Some(n(count1)? * count2.as_deref().map_or(Some(1), n)?),
                PatternKind::Circular { count, .. } | PatternKind::Path { count, .. } => n(count),
            }
        }
        _ => None,
    }
}

/// The sketch the feature is built from (its first input sketch).
fn reference_sketch(app: &SolveApp, kind: &FeatureKind) -> Option<u64> {
    kind.input_sketches().into_iter().find(|id| app.session.doc.feature(*id).is_some())
}

/// The bodies the feature made or works on: the bodies it created, then the bodies its
/// definition names (`body`, `bodies`, `targets`), then the body under a hole's position.
/// Computed once per feature and design revision: the body under a hole is found in the model's
/// triangles, and the subtitle and the context rows ask every frame.
fn body_names(app: &mut SolveApp, id: u64) -> Vec<String> {
    let revision = app.session.revision;
    if let Some((f, r, names)) = &app.properties.bodies
        && *f == id
        && *r == revision
    {
        return names.clone();
    }
    let names = find_body_names(app, id);
    app.properties.bodies = Some((id, revision, names.clone()));
    names
}

/// [`body_names`], computed.
fn find_body_names(app: &SolveApp, id: u64) -> Vec<String> {
    let st = app.session.world_state();
    let mut out: Vec<String> = st.bodies.iter().filter(|b| b.feature == id).map(|b| b.name.clone()).collect();
    let Some(f) = app.session.doc.feature(id) else { return out };
    fn push(out: &mut Vec<String>, name: &str) {
        if !name.is_empty() && !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
    }
    if let Ok(v) = serde_json::to_value(&f.kind) {
        if let Some(body) = v.get("body").and_then(Value::as_str) {
            push(&mut out, body);
        }
        for key in ["bodies", "targets"] {
            for name in v.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                push(&mut out, name);
            }
        }
    }
    for name in &f.participants {
        push(&mut out, name);
    }
    if out.is_empty()
        && let FeatureKind::Hole { position, .. } = &f.kind
        && let Some(body) = body_under(app, id, position.0)
    {
        push(&mut out, &body);
    }
    out
}

/// The body whose face the point `p` lies on in the model just before feature `id` (a hole's
/// position is inside the hole afterwards).
fn body_under(app: &SolveApp, id: u64, p: polykrom_engine::geom::Vec3) -> Option<String> {
    if !p.is_finite() {
        return None;
    }
    let st = app.session.model.state_before(id);
    let mut best: Option<(f64, &str)> = None;
    for b in &st.bodies {
        let m = b.mesh();
        for t in &m.triangles {
            let Some([a, bb, c]) = m.tri(t) else { continue };
            let d = crate::preview::point_triangle_dist(p, a, bb, c);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, b.name.as_str()));
            }
        }
    }
    best.filter(|(d, _)| *d <= 1e-3 + 1e-6 * p.len()).map(|(_, name)| name.to_owned())
}

/// A dependent feature's row: its name and its defining value ("Fillet 01 / R2", "Bolt pattern /
/// 4 instances").
fn dependent_text(app: &SolveApp, id: u64) -> String {
    let Some(f) = app.session.doc.feature(id) else { return String::new() };
    let name = tr_name(&f.name);
    if let FeatureKind::Pattern { .. } = &f.kind
        && let Some(n) = instances(app, &f.kind)
    {
        return format!("{name} / {}", instances_text(n));
    }
    let Some((label, expr, kind)) = f.kind.inputs().into_iter().next() else { return name };
    let Ok(v) = app.session.doc.eval(&expr, kind) else { return name };
    let value = match (label, kind) {
        ("Radius", _) => format!("R{}", trim(bare_length(app, v))),
        ("Diameter", _) => format!("Ø{}", trim(bare_length(app, v))),
        (_, ValueKind::Length) => short_length(app, v),
        (_, ValueKind::Angle) => angle_text(v),
        (_, ValueKind::Unitless) => trim(format!("{v:.3}")),
    };
    format!("{name} / {value}")
}

/// Draw the panel: at the right of the workspace (docked) or as a card there (floating), unless
/// the review panel, the body inspector, a sketch, a tool or a dialog has that place.
pub fn panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    app.properties.rect = None;
    let selected = selected_feature(app);
    // The editor goes with a design changed under it, its feature gone, or another feature picked.
    if app.properties.editor.as_ref().is_some_and(|ed| {
        ed.revision != app.session.revision || app.session.doc.feature(ed.feature).is_none() || selected.is_some_and(|s| s != ed.feature)
    }) {
        app.properties.editor = None;
    }
    // A tool or a dialog takes over (its own preview): an edit in progress is dropped.
    let tool = app.dialog.is_some() || app.tool.is_some() || app.tree.redefine.is_some() || app.params.is_some() || app.bom.is_some();
    if tool {
        app.properties.editor = None;
    }
    let container = Container::new(ui.ctx(), Surface::Panel);
    // The place is another panel's: the review panel, the body inspector, the sketch palette, the
    // Render workspace's own panel; floating, the right-anchored tool cards too.
    let elsewhere = app.review.open.is_some()
        || !crate::inspector::selected_bodies(app).is_empty()
        || app.session.active_sketch.is_some()
        || crate::render_workspace::active(app)
        || (container.floating() && tool);
    if elsewhere {
        app.properties.editor = None;
        return;
    }
    let t = Tokens::get();
    let available = ui.available_rect_before_wrap();
    if container.floating() {
        let gap = container.gap();
        let mut right = available.right() - gap;
        if crate::agent_chat::shown(app)
            && app.agent_chat.open
            && app.preferences.agent_chat_side != "left"
            && let Some(chat) = app.agent_chat.panel_rect
        {
            right = right.min(chat.left() - gap);
        }
        let mut bottom = available.bottom() - gap;
        if let Some(history) = app.shell.history.filter(|h| app.ui.show_timeline && h.right() > right - PANEL_WIDTH) {
            bottom = bottom.min(history.top() - gap);
        }
        // Below the ribbon (and the AI prompt row), and clear of the navigation compass, which
        // keeps its place at the top right.
        let under_bar = app.review.prompt_rect.map_or_else(|| app.shell.cards.last().map_or(available.top(), |c| c.bottom()), |r| r.bottom()) + gap;
        let compass = app.viewport.layout.as_ref().map(|l| l.axis_view).filter(|r| r.is_positive()).map_or(under_bar, |r| r.bottom() + gap);
        let top = under_bar.max(compass).min(bottom);
        let frame = container.frame(ui.ctx(), t.panel, crate::frame::Side::Right).inner_margin(t.space_10 as i8);
        let inner = PANEL_WIDTH - frame.total_margin().sum().x;
        let height = (bottom - top - frame.total_margin().sum().y).max(0.0);
        let card =
            container.area(Id::new("sc_properties_card")).pivot(Align2::RIGHT_TOP).fixed_pos(pos2(right, top)).order(egui::Order::Middle).show(
                ui.ctx(),
                |ui| {
                    container.show_frame(ui, frame, available, |ui| {
                        ui.set_width(inner);
                        ui.set_min_height(height);
                        ui_kit::scope(ui, |ui| contents(app, ui, height));
                    });
                },
            );
        app.properties.rect = Some(card.response.rect);
    } else {
        let r = container
            .edge(Id::new("sc_properties_panel"), Edge::Right)
            .exact_size(PANEL_WIDTH)
            .resizable(false)
            .frame(container.edge_frame(t.panel))
            .show(ui, |ui| {
                let height = ui.available_height();
                ui_kit::scope(ui, |ui| contents(app, ui, height))
            });
        app.properties.rect = Some(r.response.rect);
    }
    if let Some(rect) = app.properties.rect {
        crate::scenario::publish_rect("properties:panel", rect);
    }
}

/// The card's body in a scroll area `height` tall, whatever the body holds, so what stands under
/// it keeps its place: a short window scrolls the body rather than clipping the card. The
/// scrollbar shows while the body overflows, as plainly as it does under the pointer, and the
/// rows are inset from it.
fn scrolled<R>(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = Tokens::get();
    ui.scope(|ui| {
        let scroll = &mut ui.style_mut().spacing.scroll;
        scroll.floating = true;
        scroll.floating_width = t.space_4;
        scroll.floating_allocated_width = t.space_8;
        scroll.dormant_background_opacity = 0.0;
        scroll.dormant_handle_opacity = scroll.active_handle_opacity;
        egui::ScrollArea::vertical()
            .id_salt("sc_properties_scroll")
            .max_height(height.max(0.0))
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, add)
            .inner
    })
    .inner
}

/// What the card shows.
#[derive(Clone, Copy)]
enum View {
    /// A feature edited in place (design kit view 03).
    Editor,
    /// The selected feature (view 01).
    Feature(u64),
    /// The design's properties, with nothing selected.
    Design,
}

/// What stands under the card's body, at the card's bottom: the actions, one under the other,
/// a note under them (the panel's help line, the design's note on exports), and the footer line.
struct Pinned {
    actions: usize,
    note: Option<&'static str>,
}

/// The room [`Pinned`] and the footer line take under the body, item spacing included: the gap
/// after the body, the space before the actions, each action's button, the note and the footer
/// as [`ui_kit::text`] lays them out at the card's width.
fn pinned_height(ui: &egui::Ui, pinned: &Pinned, footer: &str) -> f32 {
    let t = Tokens::get();
    let gap = ui.spacing().item_spacing.y;
    let width = ui.available_width();
    let note =
        |text: &str| ui.painter().layout(text.to_owned(), egui::FontId::proportional(t.font_small), t.form_dim, width).size().y + 2.0 * t.space_4;
    gap + t.space_8 + pinned.actions as f32 * (t.form_button_height + gap) + pinned.note.map_or(0.0, |text| note(text) + gap) + note(footer)
}

/// The card's content, `height` tall: the body (the heading, the rows, the dependents or the
/// consequences) scrolls in the room above the actions, their note and the footer line, which
/// stand at the card's bottom whatever the body holds, as the AI review card's do, so they are
/// in sight in a window too short for the whole card (the r6 review: at 1366 × 768 the card was
/// cut off at the window's bottom).
fn contents(app: &mut SolveApp, ui: &mut egui::Ui, height: f32) {
    let t = Tokens::get();
    let view = if app.properties.editor.is_some() {
        View::Editor
    } else if let Some(id) = selected_feature(app) {
        View::Feature(id)
    } else {
        View::Design
    };
    let pinned = match view {
        View::Editor => Pinned { actions: 2, note: None },
        View::Feature(id) => Pinned { actions: feature_actions_count(app, id), note: Some(help_note(app, id)) },
        View::Design => Pinned { actions: 1, note: Some(tr("ui.design_properties.exports")) },
    };
    let footer_text = |app: &SolveApp| match &app.properties.editor {
        Some(ed) if ed.preview => tr("ui.properties.preview_not_applied"),
        Some(_) => tr("ui.properties.not_applied"),
        None => tr("ui.properties.no_pending"),
    };
    let body_height = height - pinned_height(ui, &pinned, footer_text(app));
    // The label column fits the widest label of the rows shown (the kit's proportion at least).
    let labels = row_labels(app);
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let editor = scrolled(ui, body_height, |ui| {
        ui_kit::labelled_form(ui, &labels, LABEL_SHARE, |ui| match view {
            View::Editor => editor_body(app, ui),
            View::Feature(id) => {
                feature_body(app, ui, id);
                None
            }
            View::Design => {
                document_body(app, ui);
                None
            }
        })
    });
    // The footer reads the editor as the body left it (its Preview box).
    let footer = footer_text(app);
    ui.add_space(t.space_8);
    match view {
        View::Editor => {
            if let Some(body) = editor {
                editor_actions(app, ui, body);
            }
        }
        View::Feature(id) => feature_actions(app, ui, id),
        View::Design => {
            if primary(ui, "edit_properties", "ui.properties.edit_properties", true).clicked() {
                open(app);
            }
        }
    }
    if let Some(note) = pinned.note {
        ui_kit::text(ui, note, ui_kit::Text::Note("properties_help"));
    }
    ui_kit::text(ui, footer, ui_kit::Text::Note("properties_footer"));
}

/// The panel's heading row: the title, and at the right a control that opens the Design
/// Properties window when the design is shown. (The kit draws its Expand glyph there; that glyph
/// is not in the runtime icon manifest yet, so the control wears the Design Properties window's
/// own icon until the icon manifest lists it.)
fn heading(app: &mut SolveApp, ui: &mut egui::Ui, title: &str, expand: bool) {
    let t = Tokens::get();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ui_kit::header_height()), egui::Sense::hover());
    let label = egui::Rect::from_min_max(rect.min, pos2(rect.right() - t.form_close_size - t.space_8, rect.bottom()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(label).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.add(egui::Label::new(egui::RichText::new(title).font(crate::fonts::medium(t.font_button)).color(t.text)).truncate());
    });
    if expand {
        let expand_rect =
            egui::Rect::from_center_size(pos2(rect.right() - t.form_close_size / 2.0, rect.center().y), vec2(t.form_close_size, t.form_close_size));
        let r = ui_kit::quiet_icon_button_at(ui, expand_rect, "settings", "Design Properties…");
        crate::scenario::publish_target("properties:expand", r.rect);
        if r.clicked() {
            open(app);
        }
    }
}

/// The kind line, the title and the subtitle under the heading.
fn summary(ui: &mut egui::Ui, kind: &str, title: &str, subtitle: &str) {
    let t = Tokens::get();
    ui.add_space(t.space_4);
    ui.add(egui::Label::new(egui::RichText::new(kind).font(crate::fonts::medium(t.font_micro)).color(t.text_dim)).truncate());
    ui.add(egui::Label::new(egui::RichText::new(title).font(crate::fonts::medium(t.font_heading)).color(t.text)).wrap());
    if !subtitle.is_empty() {
        ui.add(egui::Label::new(egui::RichText::new(subtitle).font(egui::FontId::proportional(t.font_label)).color(t.text_dim)).wrap());
    }
    ui.add_space(t.space_4);
}

/// A section heading ("Parameters", "Dependent features").
fn section_title(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get();
    ui.add_space(t.space_2);
    ui.add(egui::Label::new(egui::RichText::new(text).font(crate::fonts::medium(t.font_label)).color(t.text)).truncate());
}

/// A read-only label/value row, the value in the numeric face.
fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui_kit::row(ui, label, |ui| ui_kit::text(ui, value, ui_kit::Text::MonoCell));
}

/// A full-width secondary action.
fn wide_button(ui: &mut egui::Ui, key: &str, label: &str) -> egui::Response {
    let t = Tokens::get();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), t.form_button_height), egui::Sense::hover());
    let r = ui_kit::button_at(ui, key, rect, tr_label(label), ui_kit::Button::Normal);
    crate::scenario::publish_target(&format!("properties:{key}"), r.rect);
    r
}

/// A full-width primary action.
fn primary(ui: &mut egui::Ui, key: &str, label: &str, enabled: bool) -> egui::Response {
    let r = ui.add_enabled_ui(enabled, |ui| ui_kit::wide_primary_button(ui, label)).inner;
    crate::scenario::publish_target(&format!("properties:{key}"), r.rect);
    r
}

/// The read-only rows after the parameters: extent, instances, the driving sketch and the body.
fn context_rows(app: &mut SolveApp, id: u64) -> Vec<(&'static str, String)> {
    let bodies = body_names(app, id);
    let Some(f) = app.session.doc.feature(id) else { return Vec::new() };
    let mut rows = Vec::new();
    if let Some(extent) = extent_text(&f.kind) {
        rows.push(("Extent", extent));
    }
    if let Some(n) = instances(app, &f.kind) {
        rows.push(("Instances", n.to_string()));
    }
    if let Some(sketch) = reference_sketch(app, &f.kind).and_then(|s| app.session.doc.feature(s)) {
        rows.push(("Reference", tr_name(&sketch.name)));
    }
    if !bodies.is_empty() {
        rows.push(("Body", bodies.iter().map(|b| tr_name(b)).collect::<Vec<_>>().join(", ")));
    }
    rows
}

/// The labels of every label/value row the panel draws this frame, as the rows are given them
/// (the rows translate them): the editor's fields and the inputs it shows read-only
/// ([`editor_view`]), the selected feature's inputs, sketch counts and context rows
/// ([`feature_view`]), or the design's properties ([`document_view`]). The label column is
/// sized to the widest of them.
fn row_labels(app: &mut SolveApp) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    let feature = match app.properties.editor.as_mut() {
        Some(ed) => {
            labels.extend(fields(&mut ed.dialog).into_iter().map(|(label, _, _)| label.to_string()));
            Some(ed.feature)
        }
        None => selected_feature(app),
    };
    match feature {
        Some(id) => {
            if let Some(f) = app.session.doc.feature(id) {
                labels.extend(f.kind.inputs().into_iter().map(|(label, _, _)| label.to_string()));
                if let FeatureKind::Sketch { .. } = &f.kind {
                    labels.extend(["Profiles".to_string(), "Points".to_string()]);
                }
            }
            labels.extend(context_rows(app, id).into_iter().map(|(label, _)| label.to_string()));
        }
        None => {
            labels.extend(WELL_KNOWN.iter().map(|key| label(key).to_string()));
            labels.push("Units".to_string());
        }
    }
    labels
}

/// The subtitle of a feature: its body and instances (view 01), or instances and sketch (view 03).
fn subtitle(app: &mut SolveApp, id: u64, editing: bool) -> String {
    let body = body_names(app, id).first().map(|b| tr_name(b));
    let Some(f) = app.session.doc.feature(id) else { return String::new() };
    let instances = instances(app, &f.kind).map(instances_text);
    let sketch = reference_sketch(app, &f.kind).and_then(|s| app.session.doc.feature(s)).map(|s| tr_name(&s.name));
    let parts: Vec<String> = if editing { [instances, sketch, body] } else { [body, instances, sketch] }.into_iter().flatten().take(2).collect();
    parts.join(" / ")
}

/// The help line under a feature's actions: how the feature is edited.
fn help_note(app: &mut SolveApp, id: u64) -> &'static str {
    match edits(app, id) {
        Edits::Panel | Edits::Sketch => tr("ui.properties.help"),
        Edits::Dialog => tr("ui.properties.in_dialog"),
        Edits::Palette => tr("ui.properties.in_palette"),
    }
}

/// How many actions a feature has ([`feature_actions`]): Edit sketch alone for a sketch; else
/// Edit feature, and Edit sketch when the feature is built from a sketch.
fn feature_actions_count(app: &mut SolveApp, id: u64) -> usize {
    if edits(app, id) == Edits::Sketch {
        return 1;
    }
    let sketch = app.session.doc.feature(id).and_then(|f| reference_sketch(app, &f.kind));
    1 + usize::from(sketch.is_some())
}

/// Design kit view 01, the body: the selected feature, its parameters, its context rows and the
/// features that depend on it (each a link to that feature).
fn feature_body(app: &mut SolveApp, ui: &mut egui::Ui, id: u64) {
    let t = Tokens::get();
    let Some(f) = app.session.doc.feature(id).cloned() else { return };
    heading(app, ui, tr("ui.properties"), false);
    summary(ui, &kind_line(app, id), &tr_name(&f.name), &subtitle(app, id, false));
    ui_kit::divider(ui, "properties_parameters");
    section_title(ui, tr("ui.parameters"));
    for (label, expr, kind) in f.kind.inputs() {
        value_row(ui, label, &value_text(app, label, &expr, kind));
    }
    if let FeatureKind::Sketch { .. } = &f.kind
        && let Some(ss) = app.session.world_state().sketch(id)
    {
        value_row(ui, "Profiles", &ss.profiles.len().to_string());
        value_row(ui, "Points", &ss.sketch.points.len().to_string());
    }
    for (label, value) in context_rows(app, id) {
        value_row(ui, label, &value);
    }
    ui.add_space(t.space_8);
    ui_kit::divider(ui, "properties_dependents");
    let deps = dependents(app, id);
    let title = match deps.len() {
        0 => tr("ui.properties.no_dependents"),
        1 => tr("ui.properties.dependent_one"),
        _ => tr("ui.properties.dependent_many"),
    };
    section_title(ui, title);
    let mut pick = None;
    for dep in deps {
        let Some(kind) = app.session.doc.feature(dep).map(|f| f.kind.clone()) else { continue };
        let text = dependent_text(app, dep);
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ui_kit::row_height()), egui::Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, t.control_radius, t.hover);
        }
        let icon =
            egui::Rect::from_center_size(pos2(rect.left() + t.form_icon_size / 2.0, rect.center().y), vec2(t.form_icon_size, t.form_icon_size));
        crate::icons::paint_icon(ui.painter(), icon, crate::timeline::icon_of(&kind), 1.0);
        let label = egui::Rect::from_min_max(pos2(icon.right() + t.space_8, rect.top()), rect.max);
        ui.painter().text(
            label.left_center(),
            Align2::LEFT_CENTER,
            &text,
            egui::FontId::proportional(t.font_label),
            if response.hovered() { t.text } else { t.text_dim },
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
        crate::scenario::publish_target(&format!("properties:dependent:{dep}"), rect);
        if response.on_hover_text(&text).clicked() {
            pick = Some(dep);
        }
    }
    if let Some(dep) = pick {
        let _ = app.run("select.set", json!({"items": [{"type": "feature", "id": dep}]}));
    }
}

/// The feature's actions, under the body: Edit sketch for a sketch; else Edit feature, and Edit
/// sketch when the feature is built from one ([`feature_actions_count`] counts them).
fn feature_actions(app: &mut SolveApp, ui: &mut egui::Ui, id: u64) {
    let how = edits(app, id);
    let sketch = app.session.doc.feature(id).and_then(|f| reference_sketch(app, &f.kind));
    let mut edit = false;
    let mut edit_sketch = false;
    if how == Edits::Sketch {
        edit_sketch |= primary(ui, "edit_sketch", "ui.properties.edit_sketch", true).clicked();
    } else {
        edit |= primary(ui, "edit", "ui.properties.edit_feature", true).clicked();
        if sketch.is_some() {
            edit_sketch |= wide_button(ui, "edit_sketch", "ui.properties.edit_sketch").clicked();
        }
    }
    if edit {
        begin_edit(app, id);
    } else if edit_sketch {
        match how {
            Edits::Sketch => app.edit_sketch(id),
            _ => {
                if let Some(sketch) = sketch {
                    app.edit_sketch(sketch);
                }
            }
        }
    }
}

/// What the editor's body found, for its actions: every field holds a value and the commands
/// are ready (Apply is enabled), and Enter was accepted in a field (which applies).
struct EditorBody {
    valid: bool,
    enter: bool,
}

/// Design kit view 03, the body: the feature edited in place. None: its feature is gone, and
/// the editor with it.
fn editor_body(app: &mut SolveApp, ui: &mut egui::Ui) -> Option<EditorBody> {
    let mut ed = app.properties.editor.take()?;
    let id = ed.feature;
    let f = app.session.doc.feature(id).cloned()?;
    heading(app, ui, tr("ui.properties.edit_title"), false);
    summary(ui, &kind_line(app, id), &tr_name(&f.name), &subtitle(app, id, true));
    ui_kit::divider(ui, "properties_parameters");
    section_title(ui, tr("ui.parameters"));
    // The editable values first, each with its error under it; then the rest read-only.
    let mut enter = false;
    let mut edited: Vec<&'static str> = Vec::new();
    let mut valid = true;
    for (label, kind, text) in fields(&mut ed.dialog) {
        edited.push(label);
        enter |= ui_kit::row(ui, label, |ui| ui_kit::field(ui, text));
        if let Some(e) = field_error(app, text, kind) {
            valid = false;
            ui_kit::text(ui, &dialogs::shown_error(&e), ui_kit::Text::Error(label));
        }
    }
    for (label, expr, kind) in f.kind.inputs() {
        if !edited.contains(&label) {
            value_row(ui, label, &value_text(app, label, &expr, kind));
        }
    }
    for (label, value) in context_rows(app, id) {
        value_row(ui, label, &value);
    }
    ui_kit::divider(ui, "properties_preview");
    let preview = ui_kit::checkbox(ui, tr("Preview"), &mut ed.preview);
    crate::scenario::publish_target("properties:preview", preview.rect);
    for line in consequences(app, id) {
        ui_kit::text(ui, &line, ui_kit::Text::Hint("properties_consequence"));
    }
    // Why the inputs make no command, or why the preview failed, as the dialog says it.
    let commands = dialogs::apply_commands(app, &ed.dialog);
    if valid && let Err(e) = &commands {
        valid = false;
        ui_kit::text(ui, &dialogs::shown_error(e), ui_kit::Text::Report("error", true));
    }
    if valid
        && ed.preview
        && let Some(e) = &app.preview.error
    {
        valid = false;
        ui_kit::text(ui, &dialogs::shown_error(e), ui_kit::Text::Report("error", true));
    }
    if let Some(e) = &ed.error {
        ui_kit::text(ui, &dialogs::shown_error(e), ui_kit::Text::Report("error", true));
    }
    app.properties.editor = Some(ed);
    Some(EditorBody { valid, enter })
}

/// The editor's actions, under the body: Apply change (Enter in a field too) and Cancel edit.
fn editor_actions(app: &mut SolveApp, ui: &mut egui::Ui, body: EditorBody) {
    let apply_now = primary(ui, "apply", "ui.properties.apply", body.valid).clicked() || (body.enter && body.valid);
    let cancel_now = wide_button(ui, "cancel", "ui.properties.cancel").clicked();
    if cancel_now {
        cancel(app);
    } else if apply_now {
        let _ = apply(app);
    }
}

/// The design's properties, with nothing selected: the body; Edit properties and the note on
/// exports stand under it.
fn document_body(app: &mut SolveApp, ui: &mut egui::Ui) {
    heading(app, ui, tr("ui.properties"), true);
    let doc = &app.session.doc;
    let bodies = app.session.world_state().bodies.len();
    let subtitle = trf("ui.properties.design_summary", &[("features", &doc.features.len().to_string()), ("bodies", &bodies.to_string())]);
    summary(ui, &tr("ui.design").to_uppercase(), &tr_name(&doc.name), &subtitle);
    ui_kit::divider(ui, "properties_parameters");
    section_title(ui, tr("ui.properties"));
    let metadata = doc.metadata.clone();
    for key in WELL_KNOWN {
        let value = metadata.text(key).map_or_else(|| "—".to_string(), str::to_owned);
        value_row(ui, label(key), &value);
    }
    value_row(ui, "Units", &doc.units.clone());
}

// ---- The Design Properties window ---------------------------------------------------------------

/// What is being typed; the properties themselves live in the document.
pub struct DesignProperties {
    /// Each well-known key's text when the window opened, in [`WELL_KNOWN`] order.
    pub opened: [String; WELL_KNOWN.len()],
    /// The fields as edited.
    pub fields: [String; WELL_KNOWN.len()],
    /// The last refusal from `document.metadata.set`.
    pub error: Option<String>,
}

impl DesignProperties {
    pub fn new(app: &SolveApp) -> Self {
        let m = &app.session.doc.metadata;
        let opened = WELL_KNOWN.map(|k| m.text(k).unwrap_or_default().to_string());
        DesignProperties { fields: opened.clone(), opened, error: None }
    }

    /// The fields that differ from what the window opened with, as `document.metadata.set`
    /// takes them (empty text removes the key).
    pub fn changes(&self) -> Map<String, Value> {
        WELL_KNOWN
            .iter()
            .zip(self.opened.iter().zip(&self.fields))
            .filter(|(_, (was, now))| was != now)
            .map(|(k, (_, now))| (k.to_string(), Value::String(now.clone())))
            .collect()
    }
}

/// Open the window (a fresh copy of the design's properties).
pub fn open(app: &mut SolveApp) {
    app.design_properties = Some(DesignProperties::new(app));
}

/// Apply the edits; the window stays open with the reason when they are refused.
fn apply_window(app: &mut SolveApp, d: &mut DesignProperties) -> bool {
    let values = d.changes();
    if values.is_empty() {
        return true;
    }
    match app.run("document.metadata.set", serde_json::json!({ "values": values })) {
        Ok(_) => true,
        Err(e) => {
            d.error = Some(e);
            false
        }
    }
}

fn label(key: &str) -> &'static str {
    match key {
        "project" => crate::i18n::tr("ui.design_properties.project"),
        "part_number" => crate::i18n::tr("ui.design_properties.part_number"),
        "revision" => crate::i18n::tr("ui.design_properties.revision"),
        "author" => crate::i18n::tr("ui.design_properties.author"),
        "organization" => crate::i18n::tr("ui.design_properties.organization"),
        "description" => crate::i18n::tr("ui.design_properties.description"),
        _ => crate::i18n::tr("ui.design_properties.license"),
    }
}

/// A short one-line view of a value another tool keeps.
fn preview(v: &Value) -> String {
    let text = serde_json::to_string(v).unwrap_or_default();
    if text.chars().count() <= PREVIEW_CHARS { text } else { format!("{}…", text.chars().take(PREVIEW_CHARS).collect::<String>()) }
}

/// Draw the window when it is open. Called once per frame.
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(mut d) = app.design_properties.take() else { return };
    let metadata = app.session.doc.metadata.clone();
    let title = crate::i18n::tr_label("Design Properties");
    let (closed, action) = ui_kit::Window::new(egui::Id::new("sc_design_properties"), "settings", title).fields().show(ctx, true, |ui| {
        for (i, key) in WELL_KNOWN.iter().enumerate() {
            let label = label(key);
            let Some(field) = d.fields.get_mut(i) else { continue };
            if *key == DESCRIPTION {
                ui_kit::multiline_text_field(ui, label, egui::Id::new(("sc_design_properties", key)), field);
            } else {
                ui_kit::row(ui, label, |ui| ui_kit::text_field(ui, field, ""));
            }
            // Saved by another tool as something other than text: kept unless typed over.
            if metadata.get(key).is_some() && metadata.text(key).is_none() && d.opened.get(i).is_some_and(String::is_empty) {
                ui_kit::caption(ui, label, crate::i18n::tr("ui.design_properties.not_text"));
            }
        }
        ui_kit::text(ui, crate::i18n::tr("ui.design_properties.exports"), ui_kit::Text::Note("exports"));
        let others: Vec<(&String, &Value)> = metadata.others().collect();
        if !others.is_empty() {
            let heading = format!("{} ({})", crate::i18n::tr("ui.design_properties.others"), others.len());
            ui_kit::section(ui, "sc_design_properties_others", &heading, false, |ui| {
                ui_kit::text(ui, crate::i18n::tr("ui.design_properties.others_note"), ui_kit::Text::Note("others"));
                egui::ScrollArea::vertical().max_height(ui_kit::row_height() * OTHERS_ROWS).show(ui, |ui| {
                    for (key, value) in others {
                        ui_kit::row(ui, key, |ui| ui_kit::text(ui, &preview(value), ui_kit::Text::Cell));
                    }
                });
            });
        }
        if let Some(error) = &d.error {
            ui_kit::text(ui, error, ui_kit::Text::Report("error", true));
        }
    });
    if closed || action == ui_kit::Action::Cancel || (action == ui_kit::Action::Ok && apply_window(app, &mut d)) {
        return;
    }
    app.design_properties = Some(d);
}

/// Draw the Properties window of the body or component the menu asked about (mass, volume,
/// area, centre of mass, bounding box and counts), when it is open. Close dismisses it.
pub fn show_object(app: &mut SolveApp, ctx: &egui::Context) {
    let Some((title, v)) = app.menu.props.clone() else { return };
    let number = |x: &Value, k: &str| x.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let mut rows: Vec<(&str, String)> = Vec::new();
    let total = &v["total"];
    let bodies = v["bodies"].as_array().cloned().unwrap_or_default();
    if let [b] = bodies.as_slice() {
        // The engine names every body's material, measured or not: nothing to guess here.
        rows.push(("Material", b["material"].as_str().unwrap_or_default().to_string()));
        rows.push(("Density", format!("{:.3} g/cm³", number(b, "density_g_cm3"))));
    }
    rows.push(("Mass", format!("{:.3} g", number(total, "mass_g"))));
    rows.push(("Volume", crate::prefs::show_mm(app, number(total, "volume_mm3"), 3)));
    rows.push(("Area", crate::prefs::show_mm(app, number(total, "area_mm2"), 2)));
    let geometry = if total.get("center_of_mass").is_some() { Some(total) } else { bodies.first().filter(|_| bodies.len() == 1) };
    if let Some(b) = geometry {
        let unit = crate::prefs::unit_scale(&app.session.doc.units).1;
        if let Some(c) = b["center_of_mass"].as_array() {
            let c: Vec<String> = c.iter().map(|x| crate::prefs::show_mm_bare(app, x.as_f64().unwrap_or(f64::NAN))).collect();
            rows.push(("Center of mass", format!("({}) {unit}", c.join(", "))));
        }
        let (mn, mx) = (&b["bbox"]["min"], &b["bbox"]["max"]);
        if let (Some(a), Some(z)) = (mn.as_array(), mx.as_array()) {
            let d: Vec<String> =
                a.iter().zip(z).map(|(a, z)| crate::prefs::show_mm_bare(app, z.as_f64().unwrap_or(0.0) - a.as_f64().unwrap_or(0.0))).collect();
            rows.push(("Bounding box", format!("{} {unit}", d.join(" × "))));
        }
    }
    rows.push(("Faces / edges / vertices", format!("{} / {} / {}", total["faces"], total["edges"], total["vertices"])));
    let heading = crate::i18n::format("properties.title", &[("name", &title)]);
    let (closed, action) = ui_kit::Window::new(egui::Id::new("sc_props"), "settings", &heading).actions("", "Close", None).show(ctx, true, |ui| {
        for (label, value) in &rows {
            ui_kit::row(ui, crate::i18n::tr_label(label), |ui| ui_kit::text(ui, value, ui_kit::Text::Cell));
        }
    });
    if closed || action == ui_kit::Action::Cancel {
        app.menu.props = None;
    }
}

#[cfg(test)]
#[path = "design_properties_tests.rs"]
mod panel_tests;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::scenario::Harness;

    #[test]
    fn the_window_edits_properties_in_one_undo_step_and_keeps_other_tools_data() {
        let mut h = Harness::new();
        h.app.run("document.metadata.set", json!({"values": {"author": "Jane", "com.acme.tool": {"id": 7}}})).unwrap();
        // A well-known key another tool saved as a number.
        h.app.session.doc_mut().metadata.0.insert("revision".into(), json!(3));
        let undo = h.app.session.undo.len();
        h.step(&json!({"start": "document.metadata.set"})).unwrap();
        assert!(h.app.design_properties.is_some(), "ui.start opens the window, it doesn't run the command");
        assert!(h.widget("Jane").is_some(), "the author's field");
        assert!(h.widget("Other tools' data (1)").is_some());
        let d = h.app.design_properties.as_mut().unwrap();
        assert_eq!(d.opened[2], "", "a number is not a revision");
        d.fields[0] = "ACME enclosures".into();
        d.fields[3] = String::new();
        h.frames(2);
        h.step(&json!({"widget": "OK"})).unwrap();
        assert!(h.app.design_properties.is_none());
        let m = &h.app.session.doc.metadata;
        assert_eq!(m.project(), Some("ACME enclosures"));
        assert_eq!(m.author(), None, "an emptied field removes the key");
        assert_eq!(m.get("revision"), Some(&json!(3)), "an untouched field keeps what another tool saved");
        assert_eq!(m.get("com.acme.tool"), Some(&json!({"id": 7})));
        assert_eq!(h.app.session.undo.len(), undo + 1, "one undo step");

        // Cancel changes nothing; a refused value keeps the window open with the reason.
        h.step(&json!({"start": "document.metadata"})).unwrap();
        h.app.design_properties.as_mut().unwrap().fields[0] = "Other".into();
        h.frames(2);
        h.step(&json!({"widget": "Cancel"})).unwrap();
        assert!(h.app.design_properties.is_none());
        assert_eq!(h.app.session.doc.metadata.project(), Some("ACME enclosures"));
        h.step(&json!({"start": "document.metadata.set"})).unwrap();
        h.app.design_properties.as_mut().unwrap().fields[1] = "bad\u{7}".into();
        h.frames(2);
        h.step(&json!({"widget": "OK"})).unwrap();
        let d = h.app.design_properties.as_ref().expect("still open");
        assert!(d.error.as_deref().is_some_and(|e| e.contains("control character")), "{:?}", d.error);
        assert_eq!(h.app.session.undo.len(), undo + 1);
    }

    #[test]
    fn the_file_menu_browser_and_root_component_open_the_window() {
        let mut h = Harness::new();
        let items = crate::context_menu::items(&h.app, &crate::context_menu::Target::Component { id: 0 });
        let item = items.iter().find(|i| i.id == "ui.designProperties").expect("root component menu item");
        assert_eq!(item.label, "Design Properties");
        crate::context_menu::run_item(&mut h.app, item, egui::pos2(0.0, 0.0));
        assert!(h.app.design_properties.is_some());
        h.app.design_properties = None;
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("sc_file_menu"), true));
        h.frames(2);
        h.step(&json!({"widget": "Design Properties…"})).unwrap();
        assert!(h.app.design_properties.is_some());
        h.app.design_properties = None;
        // Browser → Document Settings → Design Properties.
        h.step(&json!({"click": {"chevron": "Document Settings"}})).unwrap();
        h.step(&json!({"click": {"browser": "Design Properties"}})).unwrap();
        assert!(h.app.design_properties.is_some());
    }
}

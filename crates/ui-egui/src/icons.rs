//! Stable UI rendering boundary for the icon artwork. Compiled only at L5.
//!
//! Command, menu, browser and timeline icons are the design kit's drawings where the kit has one
//! ([`KIT_ICONS`]) and otherwise the SVG masters in `assets/icons/ui/design/svg`, both rendered by
//! [`crate::icon_svg`]: colours come from the default `palette.toml` roles/accents and optional
//! active-theme overrides, never arbitrary caller tints. An icon with no master yet draws the
//! placeholder. The glyph, marker, navigation and pointer sources below are not command icons.
//!
//! # Design-kit icons
//!
//! [`KIT_ICONS`] is the one table that maps an app icon id to a kit file in
//! `assets/design-kit/2026-11-02/components/svg` (embedded at build time, so a missing file fails
//! the build and `tests/kit_icons.rs` holds that every entry parses and draws). A mapped id draws
//! the kit vector as shipped; the kit set is authoritative: Undo and Redo are the mirrored pair,
//! Rectangle and Circle the full gradient outlines, Spline and Shell the current drawings.
//! [`KIT_FALLBACKS`] lists the ids that have a kit relative but keep their master, with the
//! reason, and an id in neither table is unchanged: an unmapped command never changes silently.
//!
//! The kit's colours. The lime/green/mint gradient (`#E0FF30`, `#70DE17`, `#00C099`) marks the
//! action, profile or control point of a drawing and is painted as shipped in every theme, finish
//! and colour-vision mode; the geometry around it stays neutral, so the semantic accents and the
//! tab ramps that recolour masters (`paint_accented`) never touch a kit icon. The kit's three
//! neutral hexes are read as palette roles so themes apply: the outline `#CED3D8` is the `line`
//! role, the face `#42474D` is `passive-2`, and the shell tone `#181B1F` is a knockout, so the
//! surface the icon sits on shows through its cavities. On the dark theme (the kit's graphite
//! shell) the outline and face keep the kit's exact hexes unless a theme file overrides those
//! roles; the light theme draws them in the palette's light values. The finishes:
//!
//! - artwork (`paint_icon`, `paint_on`, `paint_accented`): as above;
//! - ink (`paint_ink`, `paint_app_bar`, hot menu rows): outline and faces take the ink and the
//!   gradient stays, so the application bar shows the kit's Undo, Redo and Save in its text token
//!   with their gradient marks, as a master is flattened to the ink there;
//! - danger (`paint_danger`): the faces shade through the constraint (red) ramp as a master's
//!   passive faces do; outline and gradient stay.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use egui::{Color32, Painter, Rect, TextureHandle, TextureOptions, pos2};

use crate::icon_svg::{self, Doc, Look};

/// A design-kit drawing for an app icon id.
pub struct KitIcon {
    /// The app icon id (a command, menu, browser or timeline icon name).
    pub name: &'static str,
    /// The kit file stem under `assets/design-kit/2026-11-02/components/svg`.
    pub file: &'static str,
    src: &'static str,
}

/// The kit's own colours: the outline, the face tone and the graphite shell its cavities show.
const KIT_OUTLINE: [u8; 3] = [0xce, 0xd3, 0xd8];
const KIT_FACE: [u8; 3] = [0x42, 0x47, 0x4d];
const KIT_SHELL: [u8; 3] = [0x18, 0x1b, 0x1f];

macro_rules! kit {
    ($($name:literal => $file:literal),* $(,)?) => {
        &[$(KitIcon {
            name: $name,
            file: $file,
            src: include_str!(concat!("../../../assets/design-kit/2026-11-02/components/svg/", $file, ".svg")),
        }),*]
    };
}

/// App icon id to kit file. One entry per id that draws a kit icon; nothing else is remapped.
pub const KIT_ICONS: &[KitIcon] = kit! {
    // Application bar and File menu.
    "undo" => "polykrom-icon-undo--4-62",
    "redo" => "polykrom-icon-redo--4-67",
    "save" => "polykrom-icon-save--4-77",
    "open" => "polykrom-icon-open--4-192",
    "export" => "polykrom-icon-export--4-83",
    "settings" => "polykrom-icon-settings--4-196",
    "assistant" => "polykrom-icon-ai-command--4-218",
    "close" => "polykrom-icon-close--4-213",
    "sketch_cancel" => "polykrom-icon-close--4-213",
    "finish" => "polykrom-icon-confirm--4-209",
    // Solid creation and modification.
    "sketch" => "polykrom-icon-sketch--4-276",
    "extrude" => "polykrom-icon-extrude--4-280",
    "revolve" => "polykrom-icon-revolve--4-284",
    "sweep" => "polykrom-icon-sweep--4-292",
    "loft" => "polykrom-icon-loft--4-288",
    "hole" => "polykrom-icon-hole--4-304",
    "fillet" => "polykrom-icon-fillet--4-296",
    "chamfer" => "polykrom-icon-chamfer--4-300",
    "shell" => "polykrom-icon-shell--4-308",
    "combine" => "polykrom-icon-boolean--4-324",
    "split_body" => "polykrom-icon-split-body--20-47",
    "split" => "polykrom-icon-split-body--20-47",
    "offset_face" => "polykrom-icon-offset-face--20-39",
    "move" => "polykrom-icon-move--4-93",
    "sketch_move" => "polykrom-icon-move--4-93",
    "copy" => "polykrom-icon-copy--20-25",
    "mirror" => "polykrom-icon-mirror--4-316",
    "sketch_mirror" => "polykrom-icon-mirror--4-316",
    "pattern_rect" => "polykrom-icon-pattern--4-312",
    "sketch_rect_pattern" => "polykrom-icon-pattern--4-312",
    "select" => "polykrom-icon-select--4-89",
    "measure" => "polykrom-icon-measure--4-132",
    "section" => "polykrom-icon-section--4-336",
    "params" => "polykrom-icon-parameters--4-120",
    "joint" => "polykrom-icon-joint--4-320",
    // Construction geometry.
    "plane" => "polykrom-icon-plane--4-332",
    "axis" => "polykrom-icon-axis--20-55",
    "point" => "polykrom-icon-point--4-225",
    "project" => "polykrom-icon-project--20-62",
    // Sketch tools and constraints.
    "line" => "polykrom-icon-line--7-2",
    "rect" => "polykrom-icon-rectangle--4-116",
    "circle" => "polykrom-icon-circle--4-112",
    "arc3" => "polykrom-icon-arc--7-6",
    "slot" => "polykrom-icon-slot--7-10",
    "spline" => "polykrom-icon-spline--4-106",
    "trim" => "polykrom-icon-trim--7-14",
    "extend" => "polykrom-icon-extend--7-18",
    "offset" => "polykrom-icon-offset--4-328",
    "c_coincident" => "polykrom-icon-coincident--4-340",
    "c_concentric" => "polykrom-icon-concentric--4-352",
    "c_equal" => "polykrom-icon-equal--4-356",
    "c_fix" => "polykrom-icon-fix--4-368",
    "ground" => "polykrom-icon-fix--4-368",
    "c_parallel" => "polykrom-icon-parallel--4-344",
    "c_perpendicular" => "polykrom-icon-perpendicular--4-348",
    "c_symmetry" => "polykrom-icon-symmetric--4-360",
    "c_tangent" => "polykrom-icon-tangent--4-364",
    // Browser rows, the timeline and the view.
    "body" => "polykrom-icon-body--4-166",
    "component" => "polykrom-icon-assembly--4-151",
    "folder" => "polykrom-icon-folder--4-172",
    "eye" => "polykrom-icon-visible--4-176",
    "suppress" => "polykrom-icon-hidden--4-181",
    "rollback" => "polykrom-icon-history--4-203",
    "camera" => "polykrom-icon-camera--4-260",
    "fit" => "polykrom-icon-focus--4-252",
};

/// App icon ids with a kit relative that keep their master, and why. The kit icons with no app
/// counterpart at all (Code, Grid, Rotate, Scan, Sidebar, Move face, Link, Search, Drawing,
/// Expand) are not drawn; see the entries below for the ids one could expect them on.
pub const KIT_FALLBACKS: &[(&str, &str)] = &[
    ("arc_center", "the kit has one Arc; it draws the promoted three-point tool, the centre-point variant keeps its master"),
    ("arc_tangent", "the kit has one Arc; the tangent variant keeps its master"),
    ("arc_slot", "a Slot variant; the kit Slot is the centre-to-centre tool"),
    ("arc_slot_center", "a Slot variant; the kit Slot is the centre-to-centre tool"),
    ("slot_center", "a Slot variant; the kit Slot is the centre-to-centre tool"),
    ("slot_overall", "a Slot variant; the kit Slot is the centre-to-centre tool"),
    ("circle2", "a Circle variant; the kit Circle is the centre-point tool"),
    ("circle3", "a Circle variant; the kit Circle is the centre-point tool"),
    ("circle_tt", "a Circle variant; the kit Circle is the centre-point tool"),
    ("circle_ttt", "a Circle variant; the kit Circle is the centre-point tool"),
    ("rect3", "a Rectangle variant; the kit Rectangle is the two-point tool"),
    ("rect_center", "a Rectangle variant; the kit Rectangle is the two-point tool"),
    ("rect_rounded", "a Rectangle variant; the kit Rectangle is the two-point tool"),
    ("spline_cv", "the control-vertex spline keeps its master; the kit Spline is the fit-point tool"),
    ("surface_fair", "the Surface accent variant of the control-vertex spline"),
    ("line_mid", "a Line variant (midpoint line)"),
    ("infinite", "a line type drawn in its accent on the sketch palette, not the Line tool"),
    ("centerline", "a line type drawn in its accent on the sketch palette, not the Line tool"),
    ("construction", "a line type drawn in its accent on the sketch palette, not the Line tool"),
    ("normal", "a line type drawn in its accent on the sketch palette, not the Line tool"),
    ("axis_edge", "a construction Axis variant; the kit has one Axis"),
    ("axis_normal_to_face", "a construction Axis variant; the kit has one Axis"),
    ("axis_through_cylinder", "a construction Axis variant; the kit has one Axis"),
    ("axis_two_planes", "a construction Axis variant; the kit has one Axis"),
    ("axis_two_points", "a construction Axis variant; the kit has one Axis"),
    ("plane_along_path", "a construction Plane variant; the kit has one Plane"),
    ("plane_angle", "a construction Plane variant; the kit has one Plane"),
    ("plane_midplane", "a construction Plane variant; the kit has one Plane"),
    ("plane_perpendicular", "a construction Plane variant; the kit has one Plane"),
    ("plane_tangent", "a construction Plane variant; the kit has one Plane"),
    ("plane_three_points", "a construction Plane variant; the kit has one Plane"),
    ("plane_two_edges", "a construction Plane variant; the kit has one Plane"),
    ("point_along_path", "a construction Point variant; the kit has one Point"),
    ("point_center", "a construction Point variant; the kit has one Point"),
    ("point_edge_plane", "a construction Point variant; the kit has one Point"),
    ("point_three_planes", "a construction Point variant; the kit has one Point"),
    ("point_two_edges", "a construction Point variant; the kit has one Point"),
    ("point_vertex", "a construction Point variant; the kit has one Point"),
    ("pattern_circ", "the kit Pattern is the rectangular grid; circular and path patterns keep their masters"),
    ("pattern_path", "the kit Pattern is the rectangular grid; circular and path patterns keep their masters"),
    ("sketch_circ_pattern", "the kit Pattern is the rectangular grid; circular and path patterns keep their masters"),
    ("sketch_fillet", "the 2D corner fillet; the kit Fillet is a solid edge"),
    ("sketch_chamfer", "the 2D corner chamfer; the kit Chamfer is a solid edge"),
    ("sketch_chamfer_equal", "the 2D corner chamfer; the kit Chamfer is a solid edge"),
    ("sketch_chamfer_two", "the 2D corner chamfer; the kit Chamfer is a solid edge"),
    ("sketch_chamfer_two_distances", "the 2D corner chamfer; the kit Chamfer is a solid edge"),
    ("sketch_chamfer_angle", "the 2D corner chamfer; the kit Chamfer is a solid edge"),
    ("offset_one_side", "the Offset dialog's side choices stay one matched trio of masters"),
    ("offset_two_sides", "the Offset dialog's side choices stay one matched trio of masters"),
    ("offset_symmetric", "the Offset dialog's side choices stay one matched trio of masters"),
    ("surface_extrude", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("surface_revolve", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("surface_sweep", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("surface_loft", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("surface_fillet", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("surface_extend", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("surface_trim", "the Surface workspace's accent variant keeps its master until the kit covers that workspace"),
    ("mesh_export", "Save As Mesh keeps the mesh accent that tells it from Export"),
    ("presspull", "Press Pull extrudes a face; the kit Move face translates one, so neither drawing is exact"),
    ("replace_face", "no kit drawing; a sibling of Offset face"),
    ("delete_face", "no kit drawing; a sibling of Offset face"),
    ("split_face", "no kit drawing; a sibling of Split body"),
    ("fit_section", "no kit drawing; a sibling of Section"),
    ("project_surface", "no kit drawing; a sibling of Project"),
    ("chevron", "the kit Expand is a half-gradient disclosure chevron; the UI flips and mirrors its plain chevron as furniture"),
    ("chevron_right", "the kit Expand is a half-gradient disclosure chevron; the UI flips and mirrors its plain chevron as furniture"),
    ("settings_keyboard", "a preferences page keeps its subject glyph; the kit Settings is the general control"),
    ("settings_monitor", "a preferences page keeps its subject glyph; the kit Settings is the general control"),
    ("settings_mouse", "a preferences page keeps its subject glyph; the kit Settings is the general control"),
    ("settings_palette", "a preferences page keeps its subject glyph; the kit Settings is the general control"),
    ("settings_plug", "a preferences page keeps its subject glyph; the kit Settings is the general control"),
    ("nav_settings", "the view settings control keeps its glyph; the kit Settings is the general control"),
    ("checkpoint", "the kit History goes to Roll back; checkpoints keep their master"),
    ("restore", "the kit History goes to Roll back; restore keeps its master"),
    ("revert", "the kit History goes to Roll back; revert keeps its master"),
    ("zoom", "the navigation Zoom tool and the search field share this magnifier; the kit Search is not a zoom"),
    ("chain", "the kit Link could mean chain selection, a motion link or the agent link; none is mapped until the kit says"),
    ("motion_link", "the kit Link could mean chain selection, a motion link or the agent link; none is mapped until the kit says"),
    ("agent_link", "the kit Link could mean chain selection, a motion link or the agent link; none is mapped until the kit says"),
    ("dxf", "the kit Drawing is a generic document; DXF, SVG and the Drawing workspace keep their own"),
    ("sketch_dxf", "the kit Drawing is a generic document; DXF, SVG and the Drawing workspace keep their own"),
    ("svg", "the kit Drawing is a generic document; DXF, SVG and the Drawing workspace keep their own"),
    ("ws_drawing", "the kit Drawing is a generic document; DXF, SVG and the Drawing workspace keep their own"),
    ("export_tray", "the keymap and parameter trays are not the Export command"),
    ("import_tray", "the keymap and parameter trays are not the Export command"),
    ("capture", "no kit drawing; the kit Scan and Camera are not a screenshot"),
    ("orbit", "viewport navigation keeps its glyphs; the kit Rotate is a tool, not a camera"),
];

/// The kit file an app icon id draws, if it has one.
pub fn kit_file(name: &str) -> Option<&'static str> {
    KIT_ICONS.iter().find(|k| k.name == name).map(|k| k.file)
}

/// The kit source with its neutral hexes read as palette roles: outline `line`, face
/// `passive-2`, shell knockout. The gradient stops keep their hexes.
pub fn kit_source(src: &str) -> String {
    let source = |role: &str| icon_svg::ROLE_NAMES.iter().position(|r| *r == role).and_then(|i| icon_svg::ROLE_SOURCE_HEX.get(i).copied());
    let hex = |[r, g, b]: [u8; 3]| format!("#{r:02X}{g:02X}{b:02X}");
    let mut out = src.to_owned();
    for (from, to) in [
        (hex(KIT_OUTLINE), "currentColor".to_owned()),
        (hex(KIT_FACE), source("passive-2").unwrap_or("#b8c3cf").to_owned()),
        (hex(KIT_SHELL), source("gap").unwrap_or("#ffffff").to_owned()),
    ] {
        out = out.replace(&from, &to).replace(&from.to_ascii_lowercase(), &to);
    }
    out
}

/// The parsed kit drawings by app icon id. A kit file that fails to parse is logged and its id
/// draws the master, as a broken master draws the placeholder.
fn kit_table() -> &'static HashMap<&'static str, (&'static str, Doc)> {
    static T: OnceLock<HashMap<&'static str, (&'static str, Doc)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut map = HashMap::new();
        for k in KIT_ICONS {
            // Two ids on one file (Close and Cancel, Move and Sketch Move) each parse it: the
            // documents are small and the textures are cached by file.
            match Doc::parse(&kit_source(k.src)) {
                Ok(doc) => drop(map.insert(k.name, (k.file, doc))),
                Err(e) => log::warn!("kit icon {} ({}): {e}", k.name, k.file),
            }
        }
        map
    })
}

/// The kit drawing for `name`: its file stem and parsed document.
fn kit_doc(name: &str) -> Option<(&'static str, &'static Doc)> {
    kit_table().get(name).map(|(file, doc)| (*file, doc))
}

/// The look a kit icon is drawn with for `finish`: no accent (its geometry is neutral), the kit's
/// own outline and face hexes on the dark theme unless the theme overrides those roles, and the
/// ink in both roles under an ink finish. The gradient is not a role and stays as shipped.
fn kit_look(look: Look, finish: Finish) -> Look {
    let role = |name: &str| icon_svg::ROLE_NAMES.iter().position(|r| *r == name);
    let (Some(line), Some(face)) = (role("line"), role("passive-2")) else { return look };
    let mut palette = look.palette.unwrap_or_default();
    match finish {
        Finish::Ink(ink) => {
            let rgb = [ink.r(), ink.g(), ink.b()];
            palette.roles[line] = Some(rgb);
            palette.roles[face] = Some(rgb);
        }
        Finish::Artwork | Finish::Danger if look.dark => {
            palette.roles[line].get_or_insert(KIT_OUTLINE);
            palette.roles[face].get_or_insert(KIT_FACE);
        }
        Finish::Artwork | Finish::Danger => {}
    }
    Look { accent: if finish == Finish::Danger { look.accent } else { None }, palette: Some(palette), ..look }
}

/// The master for `name`, with the audit told which kit file replaces it on screen.
fn resolve(name: &str) -> icon_svg::Resolved {
    let resolved = icon_svg::resolve(name);
    if let Some((file, _)) = kit_doc(name) {
        icon_svg::audit::record_kit(name, file);
    }
    resolved
}

#[path = "../../../assets/icons/ui/source/glyphs.rs"]
pub(crate) mod glyphs;
#[path = "../../../assets/icons/ui/source/markers.rs"]
pub(crate) mod markers;
#[path = "../../../assets/icons/ui/source/navigation.rs"]
pub(crate) mod navigation;
#[path = "../../../assets/icons/ui/source/pointer.rs"]
pub(crate) mod pointer;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Finish {
    Artwork,
    Ink(Color32),
    Danger,
}

/// Textures already rasterised: one per (master, look, pixel size, finish).
#[derive(Clone, Default)]
struct Cache(Arc<Mutex<CacheEntries>>);

#[derive(Default)]
struct CacheEntries {
    palette: Option<icon_svg::Palette>,
    textures: HashMap<(&'static str, Look, u32, Finish), TextureHandle>,
}

/// Draw palette-owned icon artwork with an explicit opacity (0 to 1).
pub fn paint_icon(p: &Painter, r: Rect, name: &str, opacity: f32) {
    paint_on(p, r, name, None, opacity);
}

/// Draw the icon `name` for a surface that is always dark (the application bar), whatever the
/// theme: `dark: Some(true)`. `None` follows the theme.
pub fn paint_on(p: &Painter, r: Rect, name: &str, dark: Option<bool>, opacity: f32) {
    paint_with_ink(p, r, name, dark, opacity, None, Mirror::None);
}

/// Monochrome flat controls on the application bar use its text token exactly.
pub(crate) fn paint_app_bar(p: &Painter, r: Rect, name: &str, opacity: f32) {
    paint_with_ink(p, r, name, Some(true), opacity, Some(crate::theme::Tokens::get().app_bar_text), Mirror::None);
}

/// Paint a single-colour icon in `ink`.
pub(crate) fn paint_ink(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::None);
}

/// Paint a single-colour icon in `ink`, mirrored top to bottom (a chevron pointing up).
pub(crate) fn paint_ink_flipped(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::Vertical);
}

/// Paint a single-colour icon mirrored left to right, reusing the original master.
pub(crate) fn paint_ink_mirrored(p: &Painter, r: Rect, name: &str, ink: Color32) {
    paint_with_ink(p, r, name, Some(true), 1.0, Some(ink), Mirror::Horizontal);
}

#[derive(Clone, Copy)]
enum Mirror {
    None,
    Vertical,
    Horizontal,
}

fn paint_with_ink(p: &Painter, r: Rect, name: &str, dark: Option<bool>, opacity: f32, ink: Option<Color32>, mirror: Mirror) {
    let ctx = p.ctx();
    let resolved = resolve(name);
    let look = Look::themed(dark.unwrap_or_else(|| ctx.global_style().visuals.dark_mode), resolved.accent, crate::theme::vision());
    paint_resolved(p, r, name, resolved, look, opacity, ink.map_or(Finish::Artwork, Finish::Ink), mirror);
}

/// Draw with a named palette ramp, preserving outlines, passive faces and transparency.
/// `None` explicitly selects base blue; an unknown name retains the icon's own accent.
/// Existing painters continue to select the icon's semantic accent.
pub fn paint_accented(p: &Painter, r: Rect, name: &str, accent: Option<&str>, opacity: f32) {
    let resolved = resolve(name);
    let accent = match accent {
        Some(name) => icon_svg::accent_index(name).or(resolved.accent),
        None => None,
    };
    let look = Look::themed(p.ctx().global_style().visuals.dark_mode, accent, crate::theme::vision());
    paint_resolved(p, r, name, resolved, look, opacity, Finish::Artwork, Mirror::None);
}

/// Shared destructive-action artwork: red shaded faces, normal outlines and cutouts.
/// Unlike a single-colour mask, this also tints a master's passive faces through
/// the constraint ramp without flattening the lid, handle or other details.
pub(crate) fn paint_danger(p: &Painter, r: Rect, name: &str, opacity: f32) {
    let resolved = resolve(name);
    let look = Look::themed(p.ctx().global_style().visuals.dark_mode, icon_svg::accent_index("constraint"), crate::theme::vision());
    paint_resolved(p, r, name, resolved, look, opacity, Finish::Danger, Mirror::None);
}

fn paint_resolved(p: &Painter, r: Rect, name: &str, resolved: icon_svg::Resolved, look: Look, opacity: f32, finish: Finish, mirror: Mirror) {
    let ctx = p.ctx();
    let ppp = ctx.pixels_per_point().max(0.5);
    let side = r.width().min(r.height());
    let px = (side * ppp).round().clamp(8.0, 512.0) as u32;
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_insert_with(egui::Id::new("sc_icon_cache"), Cache::default).clone());
    let Ok(mut entries) = cache.0.lock() else { return };
    let palette = crate::theme::Tokens::get().icon_palette;
    if entries.palette != palette {
        entries.textures.clear();
        entries.palette = palette;
    }
    let map = &mut entries.textures;
    // A kit drawing replaces the master: its own look, and its file as the cache key.
    let kit = kit_doc(name);
    let look = kit.map_or(look, |_| kit_look(look, finish));
    let master = kit.map_or(resolved.master, |(file, _)| file);
    let key = (master, look, px, finish);
    if let std::collections::hash_map::Entry::Vacant(slot) = map.entry(key) {
        let doc = match kit {
            Some((_, doc)) => doc,
            None => match icon_svg::doc_for(resolved.master, px) {
                Some(doc) => doc,
                None => return,
            },
        };
        let pm = if finish == Finish::Danger { icon_svg::render_tinted(doc, look, px) } else { icon_svg::render(doc, look, px) };
        let Some(pm) = pm else { return };
        let mut image = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data());
        // A master is flattened to the ink; a kit drawing was painted in it (and keeps its gradient).
        if let Finish::Ink(ink) = finish
            && kit.is_none()
        {
            for pixel in &mut image.pixels {
                *pixel = ink.gamma_multiply(f32::from(pixel.a()) / 255.0);
            }
        }
        slot.insert(ctx.load_texture(format!("icon:{master}:{px}"), image, TextureOptions::LINEAR));
    }
    let Some(tex) = map.get(&key) else { return };
    // Snap the image to whole physical pixels so it stays crisp.
    let size = px as f32 / ppp;
    let min = pos2(((r.center().x - size / 2.0) * ppp).round() / ppp, ((r.center().y - size / 2.0) * ppp).round() / ppp);
    // A translucent ink dims a flattened master in its pixels; a kit drawing takes it as opacity.
    let opacity = match (kit, finish) {
        (Some(_), Finish::Ink(ink)) => opacity * f32::from(ink.a()) / 255.0,
        _ => opacity,
    };
    let tint = Color32::from_white_alpha((opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
    let uv = match mirror {
        Mirror::None => Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Mirror::Vertical => Rect::from_min_max(pos2(0.0, 1.0), pos2(1.0, 0.0)),
        Mirror::Horizontal => Rect::from_min_max(pos2(1.0, 0.0), pos2(0.0, 1.0)),
    };
    p.image(tex.id(), Rect::from_min_size(min, egui::vec2(size, size)), uv, tint);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{self, ColorVision};

    #[test]
    fn theme_icon_palettes_change_uploaded_pixels_and_reuse_only_matching_textures() {
        use serde_json::json;
        let ctx = crate::fonts::test_context();
        let dark = theme::is_dark();
        ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let mut previous_alpha = None;
        let mut previous_texture = None;
        for (stroke, expected) in [
            ("#994477", Color32::from_rgb(0x99, 0x44, 0x77)),
            ("#226699", Color32::from_rgb(0x22, 0x66, 0x99)),
            ("#994477", Color32::from_rgb(0x99, 0x44, 0x77)),
        ] {
            let file = crate::theme_files::from_value(&json!({"format":crate::theme_files::FORMAT,
                "name":"Switch","base":if dark {"dark"} else {"light"},
                "icon_palette":{"solid":["#c0d0e0","#668899",stroke]}}))
            .unwrap();
            let _restore = theme::scoped_tokens(crate::theme_files::resolve(&file));
            let draw = |ui: &mut egui::Ui| {
                let r = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0));
                // A master with the active stroke (Extrude now draws the kit, whose geometry is neutral).
                paint_icon(ui.painter(), r, "thread", 1.0);
                // A forced-dark app-bar icon (a kit drawing here) must not evict the theme palette.
                paint_app_bar(ui.painter(), r.translate(egui::vec2(70.0, 0.0)), "save", 1.0);
            };
            let mut out = ctx.run_ui(egui::RawInput::default(), draw);
            let (id, image) = out
                .textures_delta
                .set
                .iter()
                .flat_map(|(id, deltas)| deltas.iter().map(move |d| (id, &d.image)))
                .find_map(|(id, data)| match data {
                    egui::ImageData::Color(image) if image.size == [64, 64] && image.pixels.contains(&expected) => Some((*id, image)),
                    _ => None,
                })
                .expect("the new theme's actual SVG texture");
            let alpha: Vec<_> = image.pixels.iter().map(|p| p.a()).collect();
            if let Some(old) = &previous_alpha {
                assert_eq!(&alpha, old, "theme colours preserve artwork and cutouts");
            }
            if let Some(old) = previous_texture {
                assert_ne!(id, old, "switching a same-base theme uploads a new texture");
            }
            previous_alpha = Some(alpha);
            previous_texture = Some(id);
            out.textures_delta.clear();
            let mut steady = ctx.run_ui(egui::RawInput::default(), draw);
            assert!(
                steady.textures_delta.set.values().flatten().all(|d| match &d.image {
                    egui::ImageData::Color(i) => i.size != [64, 64],
                }),
                "unchanged palette reuses cached textures"
            );
            steady.textures_delta.clear();
        }
    }

    #[test]
    fn assistant_pixels_use_the_application_bar_text_token() {
        let previous = theme::is_dark();
        for dark in [false, true] {
            let ctx = crate::fonts::test_context();
            theme::set_dark(dark);
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_app_bar(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(32.0, 32.0)), "assistant", 1.0);
            });
            let ink = theme::Tokens::get().app_bar_text;
            assert!(out.textures_delta.set.iter().any(|(_, deltas)| deltas.iter().any(|delta| match &delta.image {
                egui::ImageData::Color(image) => image.pixels.contains(&ink),
            })));
            out.textures_delta.clear();
        }
        theme::set_dark(previous);
    }

    #[test]
    fn actual_uploaded_icon_pixels_follow_semantics_and_vision_cache_changes() {
        let ctx = crate::fonts::test_context();
        let previous = theme::vision();
        for dark in [false, true] {
            for vision in [ColorVision::Off, ColorVision::Protan, ColorVision::Tritan] {
                theme::set_vision(vision);
                // The line types: Normal in the base blue, and Construction, Centerline and Infinite
                // in the construction accent, as the sketch canvas draws them. Masters only: a kit
                // drawing (Line, the parallel constraint) has no semantic stroke.
                for icon in
                    ["polygon", "c_collinear", "normal", "construction", "centerline", "infinite", "surface_extrude", "mesh_export", "contact_new"]
                {
                    let expected = icon_svg::primary(icon, dark, vision);
                    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        let r = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0));
                        paint_on(ui.painter(), r, icon, Some(dark), 1.0);
                    });
                    assert!(
                        output.textures_delta.set.iter().any(|(_, deltas)| deltas.iter().any(|delta| match &delta.image {
                            egui::ImageData::Color(image) => image.pixels.contains(&expected),
                        })),
                        "{icon}, {dark}, {vision:?}: actual texture misses its semantic stroke or reused a stale cache entry"
                    );
                    if icon == "contact_new" {
                        let image = output
                            .textures_delta
                            .set
                            .values()
                            .flatten()
                            .find_map(|delta| match &delta.image {
                                egui::ImageData::Color(image) if image.size == [64, 64] => Some(image),
                                _ => None,
                            })
                            .unwrap();
                        assert_eq!(image.pixels.get(48 * 64 + 48), Some(&Color32::TRANSPARENT), "uploaded plus must stay transparent");
                    }
                    // This headless test inspects uploads instead of sending them
                    // to a GPU; acknowledge the deltas before they are dropped.
                    output.textures_delta.clear();
                }
            }
        }
        theme::set_vision(previous);
    }

    #[test]
    fn per_call_accents_preserve_roles_and_share_the_existing_cache() {
        let previous = theme::vision();
        for dark in [false, true] {
            for vision in ColorVision::ALL {
                theme::set_vision(vision);
                let ctx = crate::fonts::test_context();
                ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                let mut textures = HashMap::new();
                let mut ids = HashMap::new();
                let mut silhouette = None;
                for accent in [
                    Some("construction"),
                    Some("constraint"),
                    None,
                    Some("sketch"),
                    Some("surface"),
                    Some("form"),
                    Some("mesh"),
                    Some("unknown"),
                    Some(""),
                    None,
                ] {
                    let expected_accent = match accent {
                        Some("unknown" | "") => icon_svg::resolve("surface_extrude").accent,
                        Some(name) => icon_svg::accent_index(name),
                        None => None,
                    };
                    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        paint_accented(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0)), "surface_extrude", accent, 0.42);
                    });
                    let texture = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Mesh(mesh) => {
                                assert!(mesh.vertices.iter().all(|vertex| vertex.color == Color32::from_white_alpha(107)));
                                Some(mesh.texture_id)
                            }
                            _ => None,
                        })
                        .unwrap();
                    for (id, deltas) in &output.textures_delta.set {
                        for delta in deltas {
                            let egui::ImageData::Color(image) = &delta.image;
                            if delta.pos.is_none() {
                                textures.insert(*id, image.clone());
                            }
                        }
                    }
                    if let Some(previous_id) = ids.insert(expected_accent, texture) {
                        assert_eq!(texture, previous_id, "repeat and unknown-name fallback reuse the same accent texture");
                        assert!(!output.textures_delta.set.contains_key(&texture));
                    }
                    let image = textures.get(&texture).unwrap();
                    let look = Look::themed(dark, expected_accent, vision);
                    for role in ["line", "active-2", "active-3"] {
                        assert!(image.pixels.contains(&icon_svg::role_colour(role, look)), "{accent:?}/{role}, {dark}/{vision:?}");
                    }
                    let alpha: Vec<_> = image.pixels.iter().map(|pixel| pixel.a()).collect();
                    if let Some(previous) = &silhouette {
                        assert_eq!(&alpha, previous, "a ramp override must preserve all geometry and knockouts");
                    } else {
                        silhouette = Some(alpha);
                    }
                    output.textures_delta.clear();
                }
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    paint_icon(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0)), "surface_extrude", 1.0);
                });
                let texture = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    ids.get(&icon_svg::resolve("surface_extrude").accent),
                    Some(&texture),
                    "the old painter retains semantic selection and shares the cache"
                );
                output.textures_delta.clear();
            }
        }
        theme::set_vision(previous);
    }

    /// A mostly covered pixel of the kit's lime/green/mint gradient: green leads both other channels.
    fn gradient(pixel: &Color32) -> bool {
        let a = u32::from(pixel.a());
        let straight = |c: u8| (u32::from(c) * 255).checked_div(a).unwrap_or(0);
        let (r, g, b) = (straight(pixel.r()), straight(pixel.g()), straight(pixel.b()));
        a >= 128 && g >= 190 && g > r && g > b
    }

    /// The square texture of `side` pixels uploaded by `draw`, and the texture id it is drawn with.
    fn uploaded(ctx: &egui::Context, side: usize, draw: impl FnMut(&mut egui::Ui)) -> (egui::TextureId, egui::ColorImage) {
        let mut output = ctx.run_ui(egui::RawInput::default(), draw);
        let id = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
                _ => None,
            })
            .expect("the icon is drawn as a textured mesh");
        let image = output
            .textures_delta
            .set
            .iter()
            .find(|(texture, _)| **texture == id)
            .and_then(|(_, deltas)| {
                deltas.iter().find_map(|delta| match &delta.image {
                    egui::ImageData::Color(image) if image.size == [side, side] => Some((**image).clone()),
                    _ => None,
                })
            })
            .unwrap_or_else(|| panic!("a fresh {side} px texture"));
        output.textures_delta.clear();
        (id, image)
    }

    #[test]
    fn kit_icons_draw_at_every_ui_size_in_both_themes_with_their_gradient() {
        let previous = theme::is_dark();
        for dark in [true, false] {
            for k in KIT_ICONS {
                for side in [16usize, 24, 32] {
                    let ctx = crate::fonts::test_context();
                    theme::set_dark(dark);
                    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                    let (_, image) = uploaded(&ctx, side, |ui| {
                        paint_icon(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(side as f32, side as f32)), k.name, 1.0);
                    });
                    let covered = image.pixels.iter().filter(|p| p.a() > 0).count();
                    assert!(covered > side, "{} ({}) at {side} px, dark={dark}: blank", k.name, k.file);
                    // At 16 px a thin gradient stroke is one anti-aliased pixel wide; from 24 px
                    // the mark is unmistakable.
                    assert!(side < 24 || image.pixels.iter().any(gradient), "{} at {side} px, dark={dark}: the gradient mark is missing", k.name);
                    assert!(!icon_svg::resolve(k.name).placeholder, "{}: a mapped id also has a master to fall back on", k.name);
                }
            }
        }
        theme::set_dark(previous);
    }

    #[test]
    fn kit_outline_and_face_keep_their_hexes_on_dark_and_take_the_palette_on_light() {
        let previous = theme::is_dark();
        let outline = Color32::from_rgb(KIT_OUTLINE[0], KIT_OUTLINE[1], KIT_OUTLINE[2]);
        let face = Color32::from_rgb(KIT_FACE[0], KIT_FACE[1], KIT_FACE[2]);
        let shell = Color32::from_rgb(KIT_SHELL[0], KIT_SHELL[1], KIT_SHELL[2]);
        for dark in [true, false] {
            let ctx = crate::fonts::test_context();
            theme::set_dark(dark);
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            let (_, image) = uploaded(&ctx, 32, |ui| {
                paint_icon(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(32.0, 32.0)), "shell", 1.0);
            });
            let look = Look::themed(dark, None, theme::vision());
            let has = |colour: Color32| image.pixels.contains(&colour);
            assert_eq!(has(outline), dark, "dark={dark}: the kit outline hex is for the dark theme");
            assert_eq!(has(face), dark, "dark={dark}: the kit face hex is for the dark theme");
            assert!(!has(shell), "dark={dark}: the shell tone is a knockout, never painted");
            assert_eq!(has(icon_svg::role_colour("line", look)), !dark, "dark={dark}: the light theme draws the palette's line role");
            assert_eq!(has(icon_svg::role_colour("passive-2", look)), !dark, "dark={dark}: the light theme draws the palette's face role");
            // The cavity of the shell shows the surface: the pixel at its centre is transparent.
            assert_eq!(image.pixels[18 * 32 + 13].a(), 0, "dark={dark}: the shell cavity is knocked out");
        }
        theme::set_dark(previous);
    }

    #[test]
    fn ink_and_app_bar_finishes_take_the_ink_and_keep_the_gradient() {
        let ctx = crate::fonts::test_context();
        let ink = theme::Tokens::get().app_bar_text;
        let outline = Color32::from_rgb(KIT_OUTLINE[0], KIT_OUTLINE[1], KIT_OUTLINE[2]);
        for name in ["undo", "redo", "save"] {
            let (_, image) = uploaded(&ctx, 18, |ui| {
                paint_app_bar(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(18.0, 18.0)), name, 1.0);
            });
            assert!(image.pixels.contains(&ink), "{name}: the application bar text token inks the outline");
            assert!(image.pixels.iter().any(gradient), "{name}: the gradient mark stays on the application bar");
            assert!(!image.pixels.contains(&outline), "{name}: nothing keeps the kit outline hex under an ink");
        }
        let custom = Color32::from_rgb(0x12, 0x34, 0x56);
        let (_, image) = uploaded(&ctx, 16, |ui| {
            paint_ink(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(16.0, 16.0)), "close", custom);
        });
        assert!(image.pixels.contains(&custom), "a caller's ink colours the kit outline");
        assert!(image.pixels.iter().any(gradient), "the gradient stays under a caller's ink");
    }

    #[test]
    fn danger_shades_kit_faces_red_and_keeps_outline_and_gradient() {
        let ctx = crate::fonts::test_context();
        ctx.set_visuals(egui::Visuals::dark());
        let outline = Color32::from_rgb(KIT_OUTLINE[0], KIT_OUTLINE[1], KIT_OUTLINE[2]);
        let face = Color32::from_rgb(KIT_FACE[0], KIT_FACE[1], KIT_FACE[2]);
        let (_, image) = uploaded(&ctx, 20, |ui| {
            paint_danger(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20.0, 20.0)), "shell", 1.0);
        });
        assert!(image.pixels.contains(&outline), "the outline stays");
        assert!(image.pixels.iter().any(gradient), "the gradient stays");
        assert!(!image.pixels.contains(&face), "the face is shaded, not the kit tone");
        assert!(image.pixels.iter().any(|p| p.a() == 255 && p.r() > p.g() + 40 && p.r() > p.b() + 40), "a red face");
    }

    #[test]
    fn accents_and_tab_ramps_never_recolour_a_kit_icon_and_unmapped_ids_keep_their_master() {
        let ctx = crate::fonts::test_context();
        ctx.set_visuals(egui::Visuals::dark());
        let (plain, _) = uploaded(&ctx, 24, |ui| {
            paint_icon(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(24.0, 24.0)), "extrude", 1.0);
        });
        for accent in [Some("surface"), Some("sketch"), Some("constraint"), None] {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_accented(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(24.0, 24.0)), "extrude", accent, 1.0);
            });
            let id = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
                    _ => None,
                })
                .unwrap();
            assert_eq!(id, plain, "{accent:?}: one neutral texture serves every accent");
            assert!(output.textures_delta.set.is_empty(), "{accent:?}: no new upload");
            output.textures_delta.clear();
        }
        // An id outside the table draws exactly its master.
        for (name, _) in KIT_FALLBACKS.iter().take(6).chain(std::iter::once(&("presspull", ""))) {
            assert!(kit_doc(name).is_none(), "{name} is a fallback");
            let resolved = icon_svg::resolve(name);
            assert!(!resolved.placeholder, "{name}");
            let (_, image) = uploaded(&ctx, 24, |ui| {
                paint_icon(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(24.0, 24.0)), name, 1.0);
            });
            let look = Look::themed(true, resolved.accent, theme::vision());
            let expected = icon_svg::render(icon_svg::doc_for(resolved.master, 24).unwrap(), look, 24).unwrap();
            assert_eq!(image, egui::ColorImage::from_rgba_premultiplied([24, 24], expected.data()), "{name} draws its master unchanged");
        }
    }

    #[test]
    fn redo_is_the_mirror_of_undo_and_the_authoritative_drawings_are_mapped() {
        for (name, file) in [
            ("undo", "polykrom-icon-undo--4-62"),
            ("redo", "polykrom-icon-redo--4-67"),
            ("rect", "polykrom-icon-rectangle--4-116"),
            ("circle", "polykrom-icon-circle--4-112"),
            ("spline", "polykrom-icon-spline--4-106"),
            ("shell", "polykrom-icon-shell--4-308"),
        ] {
            assert_eq!(kit_file(name), Some(file));
        }
        let look = Look::themed(true, None, theme::vision());
        let px = 48;
        let undo = icon_svg::render(kit_doc("undo").unwrap().1, look, px).unwrap();
        let redo = icon_svg::render(kit_doc("redo").unwrap().1, look, px).unwrap();
        let mut off = 0usize;
        for y in 0..px {
            for x in 0..px {
                let a = undo.pixel(x, y).unwrap();
                let b = redo.pixel(px - 1 - x, y).unwrap();
                let d = [a.red().abs_diff(b.red()), a.green().abs_diff(b.green()), a.blue().abs_diff(b.blue()), a.alpha().abs_diff(b.alpha())];
                off += usize::from(d.into_iter().max().unwrap() > 48);
            }
        }
        assert!(off < (px * px / 100) as usize, "redo mirrors undo: {off} pixels differ by more than anti-aliasing");
        // The full-outline Rectangle and Circle are gradient all round: no neutral outline pixel.
        let outline = Color32::from_rgb(KIT_OUTLINE[0], KIT_OUTLINE[1], KIT_OUTLINE[2]);
        for name in ["rect", "circle"] {
            let pm = icon_svg::render(kit_doc(name).unwrap().1, kit_look(look, Finish::Artwork), 32).unwrap();
            let image = egui::ColorImage::from_rgba_premultiplied([32, 32], pm.data());
            assert!(!image.pixels.contains(&outline), "{name}: a full gradient outline");
            assert!(image.pixels.iter().filter(|p| gradient(p)).count() > 32, "{name}: the outline is the gradient");
        }
    }

    #[test]
    fn danger_and_accent_only_textures_do_not_share_a_cache_entry() {
        for dark in [false, true] {
            let ctx = crate::fonts::test_context();
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            let mut textures = HashMap::new();
            for danger in [false, true, false, true] {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20.0, 20.0));
                    if danger {
                        paint_danger(ui.painter(), rect, "delete", 1.0);
                    } else {
                        paint_accented(ui.painter(), rect, "delete", Some("constraint"), 1.0);
                    }
                });
                let id = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Mesh(mesh) => Some(mesh.texture_id),
                        _ => None,
                    })
                    .unwrap();
                if let Some(previous) = textures.insert(danger, id) {
                    assert_eq!(id, previous, "repeated danger painting reuses its texture");
                    assert!(!output.textures_delta.set.contains_key(&id));
                }
                output.textures_delta.clear();
            }
            assert_ne!(textures[&false], textures[&true], "matching Look values must not mix ordinary gray bins with shaded red bins");
        }
    }
}

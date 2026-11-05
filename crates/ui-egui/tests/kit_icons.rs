//! The owner-supplied icon masters (`assets/icons/ui/design/svg/supplied`): the design kit's
//! 69-icon set and the logo, each byte-identical to its kit file and mapped to the app icon ids it
//! represents. [`MAPPING`] is that mapping (glyph, kit file, master id, aliases, where the app
//! shows it); the test holds it against the files, `review.json`'s `supplied` records, the kit's
//! checksum list, the runtime request manifest and the renderer, prints it and writes it to
//! `target/kit-icons/mapping.tsv`. The ids that have a supplied relative but keep their Facet
//! drawing (`icons::KIT_FALLBACKS`) are listed with their reason in `target/kit-icons/fallbacks.tsv`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use polykrom_ui_egui::{
    icon_svg::{self, Look},
    icons::KIT_FALLBACKS,
    theme::ColorVision,
};

const KIT: &str = "../../assets/design-kit/2026-11-02";
const DESIGN: &str = "../../assets/icons/ui/design";

/// Where the app shows an owner-supplied glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    /// A command icon (`CommandSpec::icon`): the ribbon, menus, the palette, the timeline.
    Command,
    /// A control of the window, a panel, a field, the browser or the timeline.
    Control,
    /// No command or control draws it yet: a master reserved for the window layout (a later task).
    Reserved,
}

struct Glyph {
    /// The glyph's name on the owner's board "Polykrom / Complete icon review · 69 glyphs".
    label: &'static str,
    /// The kit file, relative to the kit folder.
    file: &'static str,
    /// The app icon id of the master `svg/supplied/<master>.svg`.
    master: &'static str,
    /// Aliases (`aliases.toml`) that draw this master.
    aliases: &'static [&'static str],
    used: Use,
    /// The commands and controls the id represents.
    represents: &'static str,
}

const fn glyph(
    label: &'static str,
    file: &'static str,
    master: &'static str,
    aliases: &'static [&'static str],
    used: Use,
    represents: &'static str,
) -> Glyph {
    Glyph { label, file, master, aliases, used, represents }
}

/// The owner's 69 glyphs and the logo, by their app icon ids.
const MAPPING: &[Glyph] = &[
    // Window and document controls.
    glyph(
        "Undo",
        "components/svg/polykrom-icon-undo--4-62.svg",
        "undo",
        &[],
        Use::Command,
        "Undo: the application bar, the Edit menu and the timeline",
    ),
    glyph(
        "Redo",
        "components/svg/polykrom-icon-redo--4-67.svg",
        "redo",
        &[],
        Use::Command,
        "Redo: the application bar and the Edit menu (the mirror of Undo)",
    ),
    glyph(
        "Search",
        "components/svg/polykrom-icon-search--4-72.svg",
        "search",
        &[],
        Use::Control,
        "the command search field and the keymap search field",
    ),
    glyph("Save", "components/svg/polykrom-icon-save--4-77.svg", "save", &[], Use::Command, "Save: the application bar and the File menu"),
    glyph(
        "Export",
        "components/svg/polykrom-icon-export--4-83.svg",
        "export",
        &[],
        Use::Command,
        "the export commands (File); Save As Mesh keeps its Facet drawing in the mesh accent",
    ),
    glyph("Open", "components/svg/polykrom-icon-open--4-192.svg", "open", &[], Use::Command, "Open (File)"),
    glyph("Folder", "components/svg/polykrom-icon-folder--4-172.svg", "folder", &[], Use::Command, "browser folders and the folder commands"),
    glyph("Settings", "components/svg/polykrom-icon-settings--4-196.svg", "settings", &[], Use::Command, "Preferences and the application bar gear"),
    glyph(
        "Close",
        "components/svg/polykrom-icon-close--4-213.svg",
        "close",
        &["sketch_cancel"],
        Use::Control,
        "close buttons of panels and dialogs; Cancel sketch",
    ),
    glyph("Confirm", "components/svg/polykrom-icon-confirm--4-209.svg", "finish", &[], Use::Command, "Finish sketch and the confirm actions"),
    glyph(
        "AI command",
        "components/svg/polykrom-icon-ai-command--4-218.svg",
        "assistant",
        &[],
        Use::Control,
        "the assistant entry in the application bar and the assistant chat",
    ),
    glyph("Visible", "components/svg/polykrom-icon-visible--4-176.svg", "eye", &[], Use::Command, "Show (browser rows, visibility commands)"),
    glyph(
        "Hidden",
        "components/svg/polykrom-icon-hidden--4-181.svg",
        "suppress",
        &[],
        Use::Command,
        "Hide and Suppress (browser rows, the timeline)",
    ),
    glyph("History", "components/svg/polykrom-icon-history--4-203.svg", "rollback", &[], Use::Command, "Roll back the timeline"),
    glyph("Focus", "components/svg/polykrom-icon-focus--4-252.svg", "fit", &[], Use::Control, "Fit the view (navigation)"),
    glyph("Camera", "components/svg/polykrom-icon-camera--4-260.svg", "camera", &[], Use::Command, "the camera and view commands"),
    glyph(
        "Expand",
        "components/svg/polykrom-icon-expand--4-188.svg",
        "expand",
        &[],
        Use::Reserved,
        "a disclosure control; the UI still flips and mirrors its plain chevron as furniture",
    ),
    glyph("Sidebar", "components/svg/polykrom-icon-sidebar--4-265.svg", "sidebar", &[], Use::Reserved, "the sidebar toggle of the window layout"),
    glyph("Code", "components/svg/polykrom-icon-code--4-270.svg", "code", &[], Use::Reserved, "a code or script view"),
    glyph("Grid", "components/svg/polykrom-icon-grid--4-143.svg", "grid", &[], Use::Reserved, "the sketch grid toggle"),
    glyph(
        "Link",
        "components/svg/polykrom-icon-link--4-230.svg",
        "link",
        &[],
        Use::Reserved,
        "a link (chain selection, motion link or agent link keep their own masters)",
    ),
    glyph(
        "Drawing",
        "components/svg/polykrom-icon-drawing--4-236.svg",
        "drawing",
        &[],
        Use::Reserved,
        "a drawing document (DXF, SVG and the Drawing workspace keep their own masters)",
    ),
    glyph("Scan", "components/svg/polykrom-icon-scan--4-244.svg", "scan", &[], Use::Reserved, "a scan or capture"),
    // Browser rows.
    glyph(
        "Assembly",
        "components/svg/polykrom-icon-assembly--4-151.svg",
        "component",
        &[],
        Use::Command,
        "components and assemblies (browser rows, component commands)",
    ),
    glyph("Body", "components/svg/polykrom-icon-body--4-166.svg", "body", &[], Use::Control, "body rows of the browser"),
    // Tools and modifications.
    glyph("Select", "components/svg/polykrom-icon-select--4-89.svg", "select", &[], Use::Command, "the Select tool"),
    glyph("Move", "components/svg/polykrom-icon-move--4-93.svg", "move", &["sketch_move"], Use::Command, "Move (solid) and Move (sketch)"),
    glyph("Copy", "components/svg/polykrom-icon-copy--20-25.svg", "copy", &[], Use::Command, "Copy (edit and occurrence commands)"),
    glyph(
        "Rotate",
        "components/svg/polykrom-icon-rotate--4-100.svg",
        "rotate",
        &[],
        Use::Reserved,
        "a rotate tool (rotation is a Move dialog parameter today)",
    ),
    glyph("Sketch", "components/svg/polykrom-icon-sketch--4-276.svg", "sketch", &[], Use::Command, "Create sketch"),
    glyph(
        "Extrude",
        "components/svg/polykrom-icon-extrude--4-280.svg",
        "extrude",
        &[],
        Use::Command,
        "Extrude (Solid); the Surface variant keeps its Facet drawing in the surface accent",
    ),
    glyph("Revolve", "components/svg/polykrom-icon-revolve--4-284.svg", "revolve", &[], Use::Command, "Revolve (Solid)"),
    glyph("Sweep", "components/svg/polykrom-icon-sweep--4-292.svg", "sweep", &[], Use::Command, "Sweep (Solid)"),
    glyph("Loft", "components/svg/polykrom-icon-loft--4-288.svg", "loft", &[], Use::Command, "Loft (Solid)"),
    glyph("Hole", "components/svg/polykrom-icon-hole--4-304.svg", "hole", &[], Use::Command, "Hole"),
    glyph("Shell", "components/svg/polykrom-icon-shell--4-308.svg", "shell", &[], Use::Command, "Shell (the latest drawing)"),
    glyph("Fillet", "components/svg/polykrom-icon-fillet--4-296.svg", "fillet", &[], Use::Command, "Fillet (solid edges)"),
    glyph("Chamfer", "components/svg/polykrom-icon-chamfer--4-300.svg", "chamfer", &[], Use::Command, "Chamfer (solid edges)"),
    glyph(
        "Pattern",
        "components/svg/polykrom-icon-pattern--4-312.svg",
        "pattern_rect",
        &["sketch_rect_pattern"],
        Use::Command,
        "Rectangular pattern (solid and sketch)",
    ),
    glyph("Mirror", "components/svg/polykrom-icon-mirror--4-316.svg", "mirror", &["sketch_mirror"], Use::Command, "Mirror (solid and sketch)"),
    glyph(
        "Move face",
        "components/svg/polykrom-icon-move-face--20-32.svg",
        "move_face",
        &[],
        Use::Reserved,
        "a move-face command (Press Pull keeps its own master)",
    ),
    glyph("Offset face", "components/svg/polykrom-icon-offset-face--20-39.svg", "offset_face", &[], Use::Command, "Offset face"),
    glyph("Boolean", "components/svg/polykrom-icon-boolean--4-324.svg", "combine", &[], Use::Command, "Combine (the boolean operations)"),
    glyph(
        "Split body",
        "components/svg/polykrom-icon-split-body--20-47.svg",
        "split_body",
        &["split"],
        Use::Command,
        "Split body (and its legacy history id)",
    ),
    glyph("Joint", "components/svg/polykrom-icon-joint--4-320.svg", "joint", &[], Use::Command, "Joint (assembly)"),
    glyph("Measure", "components/svg/polykrom-icon-measure--4-132.svg", "measure", &[], Use::Command, "Measure (Inspect)"),
    glyph("Section", "components/svg/polykrom-icon-section--4-336.svg", "section", &[], Use::Command, "Section analysis"),
    glyph("Parameters", "components/svg/polykrom-icon-parameters--4-120.svg", "params", &[], Use::Command, "Parameters"),
    // Construction geometry.
    glyph(
        "Plane",
        "components/svg/polykrom-icon-plane--4-332.svg",
        "plane",
        &[],
        Use::Command,
        "construction Plane (its variants keep their Facet drawings)",
    ),
    glyph(
        "Axis",
        "components/svg/polykrom-icon-axis--20-55.svg",
        "axis",
        &[],
        Use::Command,
        "construction Axis (its variants keep their Facet drawings)",
    ),
    glyph("Point", "components/svg/polykrom-icon-point--4-225.svg", "point", &[], Use::Command, "construction Point and the sketch point"),
    glyph("Project", "components/svg/polykrom-icon-project--20-62.svg", "project", &[], Use::Command, "Project (sketch)"),
    // Sketch tools.
    glyph("Line", "components/svg/polykrom-icon-line--7-2.svg", "line", &[], Use::Command, "the Line tool"),
    glyph(
        "Rectangle",
        "components/svg/polykrom-icon-rectangle--4-116.svg",
        "rect",
        &[],
        Use::Command,
        "the two-point Rectangle tool (the full gradient outline)",
    ),
    glyph(
        "Circle",
        "components/svg/polykrom-icon-circle--4-112.svg",
        "circle",
        &[],
        Use::Command,
        "the centre-point Circle tool (the full gradient outline)",
    ),
    glyph("Arc", "components/svg/polykrom-icon-arc--7-6.svg", "arc3", &[], Use::Command, "the three-point Arc tool"),
    glyph("Slot", "components/svg/polykrom-icon-slot--7-10.svg", "slot", &[], Use::Command, "the centre-to-centre Slot tool"),
    glyph("Spline", "components/svg/polykrom-icon-spline--4-106.svg", "spline", &[], Use::Command, "the fit-point Spline tool (the revised drawing)"),
    glyph("Offset", "components/svg/polykrom-icon-offset--4-328.svg", "offset", &[], Use::Command, "Offset (sketch)"),
    glyph("Trim", "components/svg/polykrom-icon-trim--7-14.svg", "trim", &[], Use::Command, "Trim (sketch)"),
    glyph("Extend", "components/svg/polykrom-icon-extend--7-18.svg", "extend", &[], Use::Command, "Extend (sketch)"),
    // Sketch constraints.
    glyph("Coincident", "components/svg/polykrom-icon-coincident--4-340.svg", "c_coincident", &[], Use::Command, "the coincident constraint"),
    glyph("Parallel", "components/svg/polykrom-icon-parallel--4-344.svg", "c_parallel", &[], Use::Command, "the parallel constraint"),
    glyph(
        "Perpendicular",
        "components/svg/polykrom-icon-perpendicular--4-348.svg",
        "c_perpendicular",
        &[],
        Use::Command,
        "the perpendicular constraint",
    ),
    glyph("Concentric", "components/svg/polykrom-icon-concentric--4-352.svg", "c_concentric", &[], Use::Command, "the concentric constraint"),
    glyph("Equal", "components/svg/polykrom-icon-equal--4-356.svg", "c_equal", &[], Use::Command, "the equal constraint"),
    glyph("Symmetric", "components/svg/polykrom-icon-symmetric--4-360.svg", "c_symmetry", &[], Use::Command, "the symmetry constraint"),
    glyph("Tangent", "components/svg/polykrom-icon-tangent--4-364.svg", "c_tangent", &[], Use::Command, "the tangent constraint"),
    glyph("Fix", "components/svg/polykrom-icon-fix--4-368.svg", "c_fix", &["ground"], Use::Command, "the fix constraint and Ground (assembly)"),
    // Brand.
    glyph(
        "Polykrom logo",
        "brand/Polykrom-Logo.svg",
        "logo",
        &[],
        Use::Control,
        "the application logo, alone in the document toolbar as the application menu's button (no white background)",
    ),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &PathBuf) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A mostly covered pixel of the lime/green/mint gradient (premultiplied input).
fn gradient(p: &[u8; 4]) -> bool {
    let a = u32::from(p[3]);
    let straight = |c: u8| (u32::from(c) * 255).checked_div(a).unwrap_or(0);
    let (r, g, b) = (straight(p[0]), straight(p[1]), straight(p[2]));
    a >= 128 && g >= 190 && g > r && g > b
}

/// The kit's checksum list: relative path to SHA-256.
fn kit_sums() -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(root().join(KIT).join("SHA256SUMS.txt")).unwrap();
    text.lines()
        .filter_map(|line| {
            let (hash, path) = line.split_once("  ")?;
            Some((path.trim().to_owned(), hash.trim().to_owned()))
        })
        .collect()
}

#[test]
fn every_glyph_of_the_set_is_a_supplied_master_with_the_exact_kit_vector_mapped_to_its_ids() {
    let supplied_dir = root().join(DESIGN).join("svg/supplied");
    let review: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join(DESIGN).join("review.json")).unwrap()).unwrap();
    let records = review["supplied"].as_object().expect("review.json records the owner-supplied masters under \"supplied\"");
    let sums = kit_sums();
    let manifest = std::fs::read_to_string(root().join(DESIGN).join("requested_icons.txt")).unwrap().replace("\r\n", "\n");
    let requested: BTreeSet<&str> = manifest.lines().filter_map(|line| line.split('\t').next()).collect();
    let mut masters = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut table = String::from("glyph\tkit file\tmaster\taliases\tuse\trepresents\n");
    for g in MAPPING {
        assert!(masters.insert(g.master), "{}: master {} mapped twice", g.label, g.master);
        assert!(files.insert(g.file), "{}: kit file {} mapped twice", g.label, g.file);
        // The master is the kit's vector, byte for byte, and review.json records it with its
        // source and the kit's own checksum.
        let kit = read(&root().join(KIT).join(g.file));
        let master = read(&supplied_dir.join(format!("{}.svg", g.master)));
        assert_eq!(master, kit, "{}: svg/supplied/{}.svg is not byte-identical to the kit's {}", g.label, g.master, g.file);
        let record = records.get(g.master).unwrap_or_else(|| panic!("{}: review.json has no supplied record for {}", g.label, g.master));
        assert_eq!(record["label"].as_str(), Some(g.label), "{}: the recorded glyph name", g.master);
        assert_eq!(record["source"].as_str(), Some(format!("assets/design-kit/2026-11-02/{}", g.file).as_str()), "{}: the recorded source", g.master);
        assert_eq!(
            record["sha256"].as_str(),
            sums.get(g.file).map(String::as_str),
            "{}: the recorded hash is the kit's checksum of {}",
            g.master,
            g.file
        );
        // The resolver draws the supplied master for the id and each alias.
        let resolved = icon_svg::resolve(g.master);
        assert!(resolved.supplied && !resolved.placeholder && resolved.master == g.master, "{}: {} resolves to {:?}", g.label, g.master, resolved);
        for alias in g.aliases {
            let resolved = icon_svg::resolve(alias);
            assert!(resolved.supplied && resolved.master == g.master, "{}: alias {alias} resolves to {:?}", g.label, resolved);
        }
        // It draws at the sizes the UI uses, in both themes, with its gradient mark.
        for dark in [true, false] {
            let look = Look { dark, accent: None, vision: ColorVision::Off, palette: None };
            for px in [16u32, 24, 32] {
                let doc = icon_svg::doc_for(g.master, px).unwrap();
                let pm = icon_svg::render(doc, look, px).unwrap_or_else(|| panic!("{} at {px}px", g.master));
                let pixels = pm.data().as_chunks::<4>().0;
                let covered = pixels.iter().filter(|p| p[3] > 0).count();
                assert!(covered > px as usize, "{} at {px}px, dark={dark}: blank", g.master);
                // At 16 px a thin gradient stroke is one anti-aliased pixel wide; from 24 px the
                // mark is unmistakable.
                assert!(px < 24 || pixels.iter().any(gradient), "{} at {px}px, dark={dark}: no gradient mark", g.master);
            }
        }
        // The runtime audit requests every id in use; a reserved master is not requested yet.
        for id in std::iter::once(&g.master).chain(g.aliases) {
            match g.used {
                Use::Reserved => assert!(!requested.contains(id), "{}: {id} is requested at runtime, it is no longer reserved", g.label),
                _ => assert!(requested.contains(id), "{}: the UI never requests {id} (see requested_icons.txt)", g.label),
            }
        }
        table += &format!("{}\t{}\t{}\t{}\t{:?}\t{}\n", g.label, g.file, g.master, g.aliases.join(","), g.used, g.represents);
    }
    // The whole set, and nothing else, is supplied: 69 glyphs and the logo.
    assert_eq!(MAPPING.len(), 70);
    assert_eq!(masters.iter().copied().collect::<Vec<_>>(), icon_svg::supplied_ids(), "svg/supplied holds exactly the mapped masters");
    assert_eq!(
        masters.iter().copied().collect::<Vec<_>>(),
        records.keys().map(String::as_str).collect::<Vec<_>>(),
        "review.json records exactly the mapped masters"
    );
    let mut components: Vec<String> = std::fs::read_dir(root().join(KIT).join("components/svg"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    components.sort();
    for name in &components {
        let file = format!("components/svg/{name}");
        if files.contains(file.as_str()) {
            continue;
        }
        // The kit's one component outside the 69-icon set is the logo as a component.
        assert_eq!(name, "polykrom-brand-supplied-icon--4-40.svg", "unmapped kit component {name}");
        assert_eq!(read(&root().join(KIT).join(&file)), read(&root().join(KIT).join("brand/Polykrom-Logo.svg")), "{name} is the logo");
    }
    assert_eq!(components.len(), 70, "the kit ships 70 components");
    let dir = root().join("../../target/kit-icons");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mapping.tsv"), &table).unwrap();
    eprintln!("{} owner-supplied masters:\n{table}", MAPPING.len());
    // A contact sheet per theme for review: every supplied master at 16, 24 and 32 px, in the
    // order of the mapping, on the theme's panel colour.
    for dark in [true, false] {
        let look = Look { dark, accent: None, vision: ColorVision::Off, palette: None };
        let (columns, cell) = (10u32, 96u32);
        let rows = (MAPPING.len() as u32).div_ceil(columns);
        let background = if dark { [0x1c, 0x1d, 0x20, 0xff] } else { [0xf3, 0xf1, 0xec, 0xff] };
        let mut sheet = image::RgbaImage::from_pixel(columns * cell, rows * cell, image::Rgba(background));
        for (i, g) in MAPPING.iter().enumerate() {
            let (x0, y0) = ((i as u32 % columns) * cell, (i as u32 / columns) * cell);
            let mut x = x0 + 6;
            for px in [16u32, 24, 32] {
                let pm = icon_svg::render(icon_svg::doc_for(g.master, px).unwrap(), look, px).unwrap();
                for (j, p) in pm.data().as_chunks::<4>().0.iter().enumerate() {
                    let (dx, dy) = (j as u32 % px, j as u32 / px);
                    let a = u32::from(p[3]);
                    if a == 0 {
                        continue;
                    }
                    // Premultiplied over the background.
                    let under = sheet.get_pixel(x + dx, y0 + 40 - px / 2 + dy).0;
                    let blend = |c: u8, b: u8| ((u32::from(c) * 255 + u32::from(b) * (255 - a)) / 255) as u8;
                    sheet.put_pixel(
                        x + dx,
                        y0 + 40 - px / 2 + dy,
                        image::Rgba([blend(p[0], under[0]), blend(p[1], under[1]), blend(p[2], under[2]), 255]),
                    );
                }
                x += px + 8;
            }
        }
        sheet.save(dir.join(format!("supplied-masters-{}.png", if dark { "dark" } else { "light" }))).unwrap();
    }
}

#[test]
fn the_ids_with_a_supplied_relative_that_keep_their_facet_drawing_each_have_a_reason() {
    let supplied: BTreeSet<&str> = MAPPING.iter().flat_map(|g| std::iter::once(g.master).chain(g.aliases.iter().copied())).collect();
    let mut report = String::from("name\tmaster\treason\n");
    for (name, reason) in KIT_FALLBACKS {
        assert!(!supplied.contains(name), "{name}: both supplied and a fallback");
        assert!(!reason.trim().is_empty(), "{name}: a fallback needs its reason");
        let resolved = icon_svg::resolve(name);
        assert!(!resolved.placeholder, "{name}: a fallback must have a master");
        assert!(!resolved.supplied, "{name}: a fallback draws a Facet drawing");
        report += &format!("{name}\t{}\t{reason}\n", resolved.master);
    }
    // The Facet drawing beneath an owner-supplied master is what a `facet = true` alias draws.
    for alias in ["surface_extrude", "mesh_export"] {
        let resolved = icon_svg::resolve(alias);
        assert!(!resolved.supplied && icon_svg::resolve(resolved.master).supplied, "{alias}: the Facet drawing beneath {}", resolved.master);
        assert!(icon_svg::facet_fallback_ids().contains(&resolved.master));
    }
    let dir = root().join("../../target/kit-icons");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fallbacks.tsv"), &report).unwrap();
    eprintln!("{} ids keep their Facet drawing:\n{report}", KIT_FALLBACKS.len());
}

#[test]
fn the_authoritative_kit_drawings_are_the_supplied_masters() {
    for (name, file) in [
        ("undo", "components/svg/polykrom-icon-undo--4-62.svg"),
        ("redo", "components/svg/polykrom-icon-redo--4-67.svg"),
        ("rect", "components/svg/polykrom-icon-rectangle--4-116.svg"),
        ("circle", "components/svg/polykrom-icon-circle--4-112.svg"),
        ("spline", "components/svg/polykrom-icon-spline--4-106.svg"),
        ("shell", "components/svg/polykrom-icon-shell--4-308.svg"),
    ] {
        let g = MAPPING.iter().find(|g| g.master == name).unwrap_or_else(|| panic!("{name} is mapped"));
        assert_eq!(g.file, file, "{name}");
        assert!(icon_svg::resolve(name).supplied, "{name}");
    }
}

#[test]
fn the_logo_is_the_supplied_vector_without_a_white_background() {
    let resolved = icon_svg::resolve("logo");
    assert!(resolved.supplied && resolved.master == "logo");
    let doc = icon_svg::doc_for("logo", 64).unwrap();
    for dark in [true, false] {
        let look = Look { dark, accent: None, vision: ColorVision::Off, palette: None };
        let pm = icon_svg::render(doc, look, 64).unwrap();
        let pixels = pm.data().as_chunks::<4>().0;
        // Transparent corners and margins: no background rectangle of any colour.
        for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63), (2, 32), (61, 32)] {
            assert_eq!(pixels[y * 64 + x][3], 0, "dark={dark}: pixel ({x}, {y}) is not transparent");
        }
        assert!(!pixels.iter().any(|p| p[3] == 255 && p[0] == 255 && p[1] == 255 && p[2] == 255), "dark={dark}: a white pixel");
        assert!(pixels.iter().filter(|p| gradient(p)).count() > 64 * 8, "dark={dark}: the gradient hexagons");
    }
}

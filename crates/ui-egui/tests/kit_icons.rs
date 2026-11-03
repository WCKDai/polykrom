//! The design-kit icon table (`icons::KIT_ICONS`): every mapped kit file exists in
//! `assets/design-kit/2026-11-02/components/svg`, parses and draws at the sizes the UI uses in
//! both themes; the ids that keep their master (`icons::KIT_FALLBACKS`) each have a reason and a
//! master. The fallback list is printed and written to `target/kit-icons/fallbacks.tsv`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use polykrom_ui_egui::{
    icon_svg::{self, Doc, Look},
    icons::{self, KIT_FALLBACKS, KIT_ICONS},
    theme::ColorVision,
};

const KIT: &str = "../../assets/design-kit/2026-11-02/components/svg";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A mostly covered pixel of the lime/green/mint gradient (premultiplied input).
fn gradient(p: &[u8; 4]) -> bool {
    let a = u32::from(p[3]);
    let straight = |c: u8| (u32::from(c) * 255).checked_div(a).unwrap_or(0);
    let (r, g, b) = (straight(p[0]), straight(p[1]), straight(p[2]));
    a >= 128 && g >= 190 && g > r && g > b
}

#[test]
fn every_mapped_kit_file_exists_parses_and_draws_at_ui_sizes_in_both_themes() {
    let manifest = std::fs::read_to_string(root().join("../../assets/icons/ui/design/requested_icons.txt")).unwrap().replace("\r\n", "\n");
    let requested: BTreeSet<&str> = manifest.lines().filter_map(|line| line.split('\t').next()).collect();
    let mut names = BTreeSet::new();
    for k in KIT_ICONS {
        assert!(names.insert(k.name), "{}: mapped twice", k.name);
        let path = root().join(KIT).join(format!("{}.svg", k.file));
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}: {e}", k.name, path.display()));
        assert_eq!(icons::kit_file(k.name), Some(k.file));
        let doc = Doc::parse(&icons::kit_source(&src)).unwrap_or_else(|e| panic!("{} ({}): {e}", k.name, k.file));
        for dark in [true, false] {
            let look = Look { dark, accent: None, vision: ColorVision::Off, palette: None };
            for px in [16u32, 24, 32] {
                let pm = icon_svg::render(&doc, look, px).unwrap_or_else(|| panic!("{} at {px}px", k.name));
                let pixels = pm.data().as_chunks::<4>().0;
                let covered = pixels.iter().filter(|p| p[3] > 0).count();
                assert!(covered > px as usize, "{} at {px}px, dark={dark}: blank", k.name);
                // At 16 px a thin gradient stroke is one anti-aliased pixel wide; from 24 px the
                // mark is unmistakable.
                assert!(px < 24 || pixels.iter().any(gradient), "{} at {px}px, dark={dark}: no gradient mark", k.name);
            }
        }
        assert!(!icon_svg::resolve(k.name).placeholder, "{}: keeps a master behind the kit drawing", k.name);
        assert!(requested.contains(k.name), "{}: the UI never requests this id (see requested_icons.txt)", k.name);
    }
    let mut report = String::from("name\tmaster\treason\n");
    for (name, reason) in KIT_FALLBACKS {
        assert!(!names.contains(name), "{name}: both mapped and a fallback");
        assert!(!reason.trim().is_empty(), "{name}: a fallback needs its reason");
        let resolved = icon_svg::resolve(name);
        assert!(!resolved.placeholder, "{name}: a fallback must have a master");
        assert!(icons::kit_file(name).is_none());
        report += &format!("{name}\t{}\t{reason}\n", resolved.master);
    }
    let dir = root().join("../../target/kit-icons");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fallbacks.tsv"), &report).unwrap();
    eprintln!("{} ids draw the kit; {} keep their master:\n{report}", KIT_ICONS.len(), KIT_FALLBACKS.len());
}

#[test]
fn the_authoritative_kit_drawings_are_the_ones_mapped() {
    for (name, file) in [
        ("undo", "polykrom-icon-undo--4-62"),
        ("redo", "polykrom-icon-redo--4-67"),
        ("rect", "polykrom-icon-rectangle--4-116"),
        ("circle", "polykrom-icon-circle--4-112"),
        ("spline", "polykrom-icon-spline--4-106"),
        ("shell", "polykrom-icon-shell--4-308"),
    ] {
        assert_eq!(icons::kit_file(name), Some(file), "{name}");
    }
}

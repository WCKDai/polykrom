//! The design kit's component SVGs (`assets/design-kit/2026-11-02/components/svg`) drawn by the
//! app's icon renderer, pixel-compared with the kit's own PNG previews (`components/png`, the same
//! name). The kit icons are wired to the command ids (`tests/kit_icons.rs` holds the mapping);
//! this holds that they render right.
//!
//! Metric, per icon, over every premultiplied RGBA channel of every pixel at the preview's size
//! (96 px for the icons, 256 px for the logo): the mean absolute error (`MAE`, 0..255) and the
//! share of pixels whose worst channel differs by more than `FAR` (`far`, 0..1). The previews come
//! from another rasteriser, so edge anti-aliasing differs by a few levels along every stroke; a
//! wrong colour, a missing gradient, a misplaced shape or a wrong scale moves both numbers far
//! past the tolerances. A review sheet (ours | kit | difference) goes to
//! `target/design-kit-review/` for a human to look at.

use std::path::PathBuf;

use polykrom_ui_egui::{
    icon_svg::{Doc, Look, render},
    theme::ColorVision,
};

const KIT: &str = "../../assets/design-kit/2026-11-02/components";
/// Mean absolute channel error allowed per icon.
const MAE_TOLERANCE: f64 = 4.0;
/// Share of pixels allowed to differ by more than `FAR` in some channel.
const FAR_TOLERANCE: f64 = 0.02;
const FAR: u8 = 96;

struct Score {
    name: String,
    mae: f64,
    far: f64,
    worst: u8,
}

fn kit_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(KIT)
}

fn kit_svgs() -> Vec<PathBuf> {
    let dir = kit_dir().join("svg");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 70, "the kit ships 70 components");
    files
}

fn kit_look() -> Look {
    Look { dark: true, accent: None, vision: ColorVision::Off, palette: None }
}

fn pixels(rgba: &[u8]) -> &[[u8; 4]] {
    rgba.as_chunks::<4>().0
}

/// Premultiplied RGBA bytes of a straight-alpha image, the form the renderer produces.
fn premultiply(rgba: &[u8]) -> Vec<u8> {
    pixels(rgba)
        .iter()
        .flat_map(|p| {
            let a = u16::from(p[3]);
            let mul = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
            [mul(p[0]), mul(p[1]), mul(p[2]), p[3]]
        })
        .collect()
}

fn score(name: &str, ours: &[u8], kit: &[u8]) -> Score {
    assert_eq!(ours.len(), kit.len(), "{name}: sizes differ");
    let mut sum = 0u64;
    let mut far = 0usize;
    let mut worst = 0u8;
    for (a, b) in pixels(ours).iter().zip(pixels(kit)) {
        let mut pixel_worst = 0u8;
        for (x, y) in a.iter().zip(b) {
            let d = x.abs_diff(*y);
            sum += u64::from(d);
            pixel_worst = pixel_worst.max(d);
        }
        worst = worst.max(pixel_worst);
        far += usize::from(pixel_worst > FAR);
    }
    let pixels = ours.len() / 4;
    Score { name: name.to_owned(), mae: sum as f64 / ours.len() as f64, far: far as f64 / pixels as f64, worst }
}

#[test]
fn every_kit_component_parses_and_renders_without_a_placeholder() {
    for path in kit_svgs() {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&path).unwrap();
        let doc = Doc::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        for px in [16, 24, 48, 96] {
            let pm = render(&doc, kit_look(), px).unwrap_or_else(|| panic!("{name} at {px}px"));
            let coverage = pm.pixels().iter().filter(|p| p.alpha() > 0).count();
            assert!(coverage > px as usize, "{name} at {px}px is blank");
        }
    }
}

#[test]
fn every_kit_component_matches_its_png_preview() {
    let review = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/design-kit-review");
    std::fs::create_dir_all(&review).unwrap();
    let mut scores = Vec::new();
    let mut sheet: Vec<image::RgbaImage> = Vec::new();
    for path in kit_svgs() {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let png = kit_dir().join("png").join(format!("{name}.png"));
        let preview = image::open(&png).unwrap_or_else(|e| panic!("{}: {e}", png.display())).to_rgba8();
        let (w, h) = preview.dimensions();
        assert_eq!(w, h, "{name}: the preview is square");
        let src = std::fs::read_to_string(&path).unwrap();
        let doc = Doc::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let pm = render(&doc, kit_look(), w).unwrap_or_else(|| panic!("{name} at {w}px"));
        let kit = premultiply(preview.as_raw());
        let s = score(&name, pm.data(), &kit);
        // ours | kit | difference, on the kit's graphite shell so the review is readable.
        let mut row = image::RgbaImage::from_pixel(w * 3 + 8, h, image::Rgba([0x18, 0x1b, 0x1f, 255]));
        for (i, data) in [pm.data(), kit.as_slice()].into_iter().enumerate() {
            for (k, p) in pixels(data).iter().enumerate() {
                let (x, y) = ((k as u32 % w) + i as u32 * (w + 4), k as u32 / w);
                let a = u32::from(p[3]);
                let over = |c: u8, shell: u32| (u32::from(c) + shell * (255 - a) / 255) as u8;
                row.put_pixel(x, y, image::Rgba([over(p[0], 0x18), over(p[1], 0x1b), over(p[2], 0x1f), 255]));
            }
        }
        for (k, (a, b)) in pixels(pm.data()).iter().zip(pixels(&kit)).enumerate() {
            let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
            row.put_pixel((k as u32 % w) + 2 * (w + 4), k as u32 / w, image::Rgba([d, d, d, 255]));
        }
        sheet.push(row);
        scores.push(s);
    }
    let width = sheet.iter().map(|r| r.width()).max().unwrap();
    let height: u32 = sheet.iter().map(|r| r.height() + 2).sum();
    let mut all = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 255]));
    let mut y = 0;
    for row in &sheet {
        image::imageops::overlay(&mut all, row, 0, i64::from(y));
        y += row.height() + 2;
    }
    all.save(review.join("design-kit-review.png")).unwrap();
    scores.sort_by(|a, b| b.mae.total_cmp(&a.mae));
    let mut report = String::new();
    for s in &scores {
        report += &format!("{}\tmae={:.3}\tfar={:.4}\tworst={}\n", s.name, s.mae, s.far, s.worst);
    }
    std::fs::write(review.join("scores.tsv"), &report).unwrap();
    let worst = scores.first().unwrap();
    eprintln!("worst kit icon by MAE: {} (mae {:.3}, far {:.4}, worst channel {})", worst.name, worst.mae, worst.far, worst.worst);
    for s in &scores {
        assert!(s.mae <= MAE_TOLERANCE, "{}: mean absolute error {:.3} > {MAE_TOLERANCE}\n{report}", s.name, s.mae);
        assert!(s.far <= FAR_TOLERANCE, "{}: {:.2}% of pixels differ by more than {FAR}\n{report}", s.name, s.far * 100.0);
    }
}

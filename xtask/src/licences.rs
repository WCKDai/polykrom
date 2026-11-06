//! `cargo xtask licences [--check]`: THIRD-PARTY-LICENSES.txt, the licences of every crate built
//! into the shipped programs (solidcraft, solidcraft-cli, polykrom-web), read from
//! `cargo metadata` and the crates' own licence files, and of every font the builds ship, as
//! `assets/fonts/bundled.txt` lists them (the one list the builds are checked against,
//! `crate::fonts`), with each font's copyright notice from the licence file beside it and its
//! source from its ATTRIBUTION.md row. The programs embed it (Help ▸ About ▸ Licences,
//! `polykrom-cli licences`), so a release download needs no licence files beside it.
//! `--check` (part of `cargo xtask ci`) generates it without writing it: it fails when the
//! dependency graph can't be read, when a source embeds or copies a font the list lacks, or when
//! a listed font lacks its licence or attribution, and only reports a file that is out of date,
//! since pull requests leave it to the Generated files workflow
//! (`.github/workflows/generated.yml`), which rewrites it on main after each merge.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use serde_json::Value;

use crate::cargo;
use crate::fonts::{self, Ships};

pub const FILE: &str = "THIRD-PARTY-LICENSES.txt";
const ROOTS: &[&str] = &["polykrom", "polykrom-cli", "polykrom-web"];
/// The heading of the font section of the notice.
const FONTS_HEADING: &str = "Bundled fonts";

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let text = generate(root)?;
    let path = root.join(FILE);
    if check {
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        if old.replace("\r\n", "\n") != text {
            // Not an error: pull requests don't edit the file (two that did would always conflict);
            // the Generated files workflow rewrites it on main after the merge.
            println!("licences: {FILE} will change when this lands (the Generated files workflow rewrites it on main)");
            return Ok(());
        }
        println!("licences: {FILE} is up to date");
        return Ok(());
    }
    std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("licences: wrote {FILE} ({} KB)", text.len() / 1024);
    Ok(())
}

struct Crate {
    name: String,
    version: String,
    license: String,
    repository: String,
    /// (file name, text) of the licence files shipped in the crate.
    files: Vec<(String, String)>,
}

fn generate(root: &Path) -> Result<String, String> {
    let crates = graph()?;
    let fonts = bundled_fonts(root)?;
    Ok(render(&crates, &fonts))
}

/// The third-party crates `cargo metadata` says are built into the shipped programs, sorted by
/// name and version.
fn graph() -> Result<Vec<Crate>, String> {
    let out = cargo().args(["metadata", "--format-version", "1", "--locked"]).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata JSON: {e}"))?;
    let packages: BTreeMap<&str, &Value> = meta["packages"].as_array().into_iter().flatten().filter_map(|p| Some((p["id"].as_str()?, p))).collect();
    let nodes: BTreeMap<&str, &Value> =
        meta["resolve"]["nodes"].as_array().into_iter().flatten().filter_map(|n| Some((n["id"].as_str()?, n))).collect();
    let members: BTreeSet<&str> = meta["workspace_members"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();

    // Everything reachable from the shipped programs through normal (not dev) dependencies, on any
    // platform.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue: VecDeque<&str> = packages
        .iter()
        .filter(|(id, p)| members.contains(*id) && ROOTS.contains(&p["name"].as_str().unwrap_or_default()))
        .map(|(id, _)| *id)
        .collect();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        for d in nodes.get(id).and_then(|n| n["deps"].as_array()).into_iter().flatten() {
            let normal = d["dep_kinds"].as_array().into_iter().flatten().any(|k| k["kind"].is_null());
            if normal && let Some(pkg) = d["pkg"].as_str() {
                queue.push_back(pkg);
            }
        }
    }

    let mut crates = Vec::new();
    for id in seen {
        let Some(p) = packages.get(id) else { continue };
        if members.contains(id) {
            continue;
        }
        let dir = Path::new(p["manifest_path"].as_str().unwrap_or_default()).parent().map(Path::to_path_buf).unwrap_or_default();
        let s = |k: &str| p[k].as_str().unwrap_or_default().to_string();
        crates.push(Crate {
            name: s("name"),
            version: s("version"),
            license: s("license"),
            repository: s("repository"),
            files: licence_files(&dir, p),
        });
    }
    crates.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(crates)
}

/// The licence and notice files at the top of a crate's package (and its `license-file`).
fn licence_files(dir: &Path, p: &Value) -> Vec<(String, String)> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let u = n.to_ascii_uppercase();
            if e.path().is_file() && ["LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE", "COPYRIGHT"].iter().any(|k| u.starts_with(k)) {
                names.insert(n);
            }
        }
    }
    if let Some(f) = p["license_file"].as_str() {
        names.insert(f.to_string());
    }
    names
        .into_iter()
        .filter_map(|n| {
            let text = std::fs::read_to_string(dir.join(&n)).ok()?;
            let text = text.replace("\r\n", "\n").trim().to_string();
            (!text.is_empty()).then(|| (Path::new(&n).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or(n), text))
        })
        .collect()
}

/// A font the builds ship, as the list names it.
struct Font {
    /// Repository-relative path, `/`-separated.
    path: String,
    /// How it ships.
    ships: Vec<Ships>,
    /// The copyright notices at the head of its licence file.
    copyright: Vec<String>,
    /// Where it comes from: the URL of its ATTRIBUTION.md row (else the row's whole Source cell).
    source: String,
    /// Its licence as its ATTRIBUTION.md row names it.
    licence: String,
    /// Repository-relative path of the licence file beside it, and that file's text from its
    /// copyright notice on.
    licence_file: String,
    licence_text: String,
}

/// Every font the builds ship, in the list's order, each with its notice, source and licence. An
/// error names a font a source embeds or copies outside the list, a listed font nothing ships,
/// or one without an ATTRIBUTION.md row or a licence file beside it (the SIL OFL requires the
/// copyright notice and the licence text to accompany the font).
fn bundled_fonts(root: &Path) -> Result<Vec<Font>, String> {
    let rows = attribution(root)?;
    fonts::check(root)?.into_iter().map(|f| describe(root, f, &rows)).collect()
}

/// An ATTRIBUTION.md row: author, source and licence.
struct Row {
    source: String,
    licence: String,
}

/// ATTRIBUTION.md's rows by file path.
fn attribution(root: &Path) -> Result<BTreeMap<String, Row>, String> {
    let text = std::fs::read_to_string(root.join("ATTRIBUTION.md")).map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let mut rows = BTreeMap::new();
    for line in text.lines().filter(|l| l.starts_with('|')) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if let [_, path, _author, source, licence, ..] = cells[..]
            && !path.is_empty()
            && path != "File"
            && !path.starts_with("---")
        {
            rows.insert(path.to_string(), Row { source: source.to_string(), licence: licence.to_string() });
        }
    }
    Ok(rows)
}

/// The first URL in a text, without a trailing bracket or punctuation.
fn url(text: &str) -> Option<&str> {
    text.split(|c: char| c.is_whitespace() || c == '(' || c == '<')
        .find(|w| w.starts_with("https://") || w.starts_with("http://"))
        .map(|w| w.trim_end_matches([')', '>', ',', ';', ':', '.']))
}

/// The font a licence file is named for: `Inter-OFL.txt` → `Inter-`, the stem before the first
/// `OFL`/`LICENSE`/`LICENCE`/`COPYING`; `None` for a generic `OFL.txt` or `LICENSE`.
fn font_prefix(licence: &str) -> Option<&str> {
    let stem = licence.rsplit_once('.').map_or(licence, |(s, _)| s);
    let upper = stem.to_ascii_uppercase();
    let at = ["OFL", "LICENSE", "LICENCE", "COPYING"].iter().filter_map(|k| upper.find(k)).min()?;
    (at > 0).then(|| &stem[..at])
}

/// The licence file beside a font: the `OFL`/`LICENSE`/`LICENCE`/`COPYING` file in its folder. A
/// file named for a font (`Inter-OFL.txt`) is only that font's (`Inter-400.ttf`), however many
/// files the folder holds; among several, the font's own file wins over a generic `OFL.txt`.
fn licence_beside(root: &Path, font: &str) -> Result<String, String> {
    let dir = Path::new(font).parent().unwrap_or(Path::new(""));
    let name = Path::new(font).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut candidates: Vec<String> = std::fs::read_dir(root.join(dir))
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let u = n.to_ascii_uppercase();
            !fonts::font_ext(n) && (u.contains("OFL") || ["LICENSE", "LICENCE", "COPYING"].iter().any(|k| u.starts_with(k)))
        })
        .collect();
    candidates.sort();
    // Another font's file (`IBM-Plex-Mono-OFL.txt` for `Inter-400.ttf`) is never this font's, even
    // when it is the only one there.
    candidates.retain(|c| font_prefix(c).is_none_or(|prefix| name.starts_with(prefix)));
    if candidates.len() > 1 {
        // The font's own file among several.
        candidates.retain(|c| font_prefix(c).is_some());
    }
    match candidates.as_slice() {
        [one] => Ok(dir.join(one).to_string_lossy().replace('\\', "/")),
        [] => Err(format!("licences: no licence file beside the bundled font {font} (expected OFL.txt or <name>-OFL.txt in {})", dir.display())),
        many => Err(format!("licences: several licence files could be the bundled font {font}'s: {}", many.join(", "))),
    }
}

/// Whether a line names a copyright (`Copyright`, `copyright (c)`, `©`).
fn is_copyright_line(line: &str) -> bool {
    line.to_ascii_lowercase().contains("copyright") || line.contains('©')
}

/// Where the licence text proper begins in a font licence file: its first rule or its `SIL Open
/// Font License, Version 1.1` heading, after the copyright notices.
fn licence_text_begins(line: &str) -> bool {
    line.starts_with("---") || line.to_ascii_uppercase().contains("OPEN FONT LICENSE VERSION")
}

/// The copyright notices at the head of a font licence file: only the lines naming a copyright
/// before the licence text begins. A header the repository added above them (`Liberation Sans
/// (crates/sketch/fonts/…)`) is not a notice and is not printed.
fn copyright_lines(licence: &str) -> Vec<String> {
    licence.lines().take_while(|l| !licence_text_begins(l)).filter(|l| is_copyright_line(l)).map(|l| l.trim().to_string()).collect()
}

/// A font licence file's text from its first copyright notice on: what the OFL requires to
/// accompany the font (the notice and the licence text), without any header above them. A file
/// with no notice before its licence text keeps the text from where that begins.
fn licence_text(licence: &str) -> String {
    let text = licence.replace("\r\n", "\n");
    let head = text.lines().take_while(|l| !licence_text_begins(l));
    let at = head.take_while(|l| !is_copyright_line(l)).map(|l| l.len() + 1).sum::<usize>().min(text.len());
    text[at..].trim().to_string()
}

fn describe(root: &Path, font: fonts::Bundled, rows: &BTreeMap<String, Row>) -> Result<Font, String> {
    let fonts::Bundled { path, ships } = font;
    let row = rows.get(&path).ok_or_else(|| format!("licences: the bundled font {path} has no ATTRIBUTION.md row"))?;
    let licence_file = licence_beside(root, &path)?;
    let licence_text = licence_text(&std::fs::read_to_string(root.join(&licence_file)).map_err(|e| format!("{licence_file}: {e}"))?);
    let copyright = copyright_lines(&licence_text);
    if copyright.is_empty() {
        return Err(format!("licences: {licence_file} carries no copyright notice before the licence text"));
    }
    Ok(Font {
        source: url(&row.source).unwrap_or(&row.source).to_string(),
        licence: row.licence.clone(),
        path,
        ships,
        copyright,
        licence_file,
        licence_text,
    })
}

fn render(crates: &[Crate], fonts: &[Font]) -> String {
    let rule = "=".repeat(80);
    let mut out = format!(
        "Third-party software in Polykrom\n\nPolykrom (the desktop app, polykrom-cli and the web app) is built from the {} crates below.\n\
         Generated by `cargo xtask licences` from Cargo.lock and the crates' own licence files.\n\
         Polykrom's own licence is in LICENSE-MIT and LICENSE-APACHE, and NOTICE.\n\n",
        crates.len()
    );
    for c in crates {
        out += &format!("{} {}  ({})\n", c.name, c.version, if c.license.is_empty() { "see its licence file" } else { &c.license });
    }
    // One copy of each distinct licence text, with the crates it covers.
    let mut by_text: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut without: Vec<&Crate> = Vec::new();
    for c in crates {
        if c.files.is_empty() {
            without.push(c);
        }
        for (_, text) in &c.files {
            by_text.entry(text.as_str()).or_default().push(format!("{} {}", c.name, c.version));
        }
    }
    let mut sections: Vec<(Vec<String>, &str)> = by_text.into_iter().map(|(t, v)| (v, t)).collect();
    sections.sort();
    for (names, text) in sections {
        out += &format!("\n{rule}\n{}\n{rule}\n\n{text}\n", wrap(&names.join(", ")));
    }
    if !without.is_empty() {
        out += &format!(
            "\n{rule}\nCrates whose package carries no licence file (their licence is named in Cargo.toml; the MIT and Apache-2.0\ntexts are as in Polykrom's own LICENSE-MIT and LICENSE-APACHE)\n{rule}\n\n"
        );
        for c in without {
            out += &format!(
                "{} {}: {}{}\n",
                c.name,
                c.version,
                c.license,
                if c.repository.is_empty() { String::new() } else { format!(" ({})", c.repository) }
            );
        }
    }

    // The fonts the builds ship, each with how it ships and its copyright notice, then one copy
    // of each distinct licence text (a font's licence file begins with its own notice).
    out += &format!(
        "\n{rule}\n{FONTS_HEADING}\n{rule}\n\n\
         The fonts the builds ship, as {} lists them (the one list the builds are checked against and\n\
         `cargo xtask licences` reads), each with how it ships, the copyright notice and licence of the file beside\n\
         it, and its source (ATTRIBUTION.md). The licence texts follow.\n\n",
        fonts::LIST
    );
    for f in fonts {
        out += &format!("- {}  ({})\n", f.path, f.licence);
        for ships in &f.ships {
            out += &format!("  Ships: {}\n", ships.describe());
        }
        for line in &f.copyright {
            out += &format!("  {line}\n");
        }
        out += &format!("  Source: {}\n  Licence: {}\n\n", f.source, f.licence_file);
    }
    let mut by_text: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for f in fonts {
        let name = f.path.rsplit('/').next().unwrap_or(&f.path);
        by_text.entry(f.licence_text.as_str()).or_default().push(name.to_string());
    }
    let mut sections: Vec<(Vec<String>, &str)> = by_text.into_iter().map(|(t, v)| (v, t)).collect();
    sections.sort();
    for (names, text) in sections {
        out += &format!("{rule}\n{}\n{rule}\n\n{text}\n\n", wrap(&names.join(", ")));
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

/// Wrap a long list of crate names at 100 columns.
fn wrap(s: &str) -> String {
    let mut out = String::new();
    let mut col = 0;
    for w in s.split(' ') {
        if col > 0 && col + w.len() + 1 > 100 {
            out.push('\n');
            col = 0;
        } else if col > 0 {
            out.push(' ');
            col += 1;
        }
        out += w;
        col += w.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice() -> String {
        std::fs::read_to_string(crate::root().join(FILE)).unwrap_or_default()
    }

    /// The notice from its font heading to its end: the font list and the licence texts.
    fn font_section(notice: &str) -> &str {
        notice.find(&format!("\n{FONTS_HEADING}\n")).map_or("", |at| &notice[at + 1..])
    }

    /// The font list of the notice: the lines after the heading, up to the next rule.
    fn font_list(notice: &str) -> Vec<&str> {
        let mut lines = notice.lines().skip_while(|l| !l.starts_with(FONTS_HEADING)).skip(2);
        lines.by_ref().take_while(|l| !l.starts_with("====")).collect()
    }

    /// The repository files a text names: every token with a `/` that is not a URL and whose
    /// last part has an extension, without the brackets around it.
    fn named_paths(text: &str) -> BTreeSet<String> {
        text.split_whitespace()
            .map(|w| w.trim_matches(['(', ')', ',', ';', '`', '"', '\'', '.', ':']))
            .filter(|w| w.contains('/') && !w.contains("://") && w.rsplit('/').next().is_some_and(|last| last.contains('.')))
            .map(str::to_string)
            .collect()
    }

    fn fonts() -> Vec<Font> {
        bundled_fonts(&crate::root()).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn the_list_names_the_fonts_the_builds_ship_with_their_notices() {
        let fonts = fonts();
        let paths: Vec<&str> = fonts.iter().map(|f| f.path.as_str()).collect();
        // Sketch text (crates/sketch), the interface (Geist) and its numbers (Geist Mono)
        // (crates/ui-egui), and the web app's Japanese face (apps/polykrom-web/index.html); the
        // design kit's Inter and IBM Plex Mono render the kit's SVG text only, and the web site's
        // own JetBrains Mono is not the app's.
        for expected in [
            "assets/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "assets/fonts/geist/Geist-Regular.ttf",
            "assets/fonts/geist/Geist-Medium.ttf",
            "assets/fonts/geist/Geist-SemiBold.ttf",
            "assets/fonts/geist/GeistMono-Regular.ttf",
            "assets/fonts/geist/GeistMono-Medium.ttf",
            "assets/fonts/biz-udp-gothic/BIZUDPGothic-Regular.ttf",
        ] {
            assert!(paths.contains(&expected), "{expected} not among {paths:?}");
        }
        assert!(!paths.iter().any(|p| p.contains("jetbrains") || p.contains("design-kit")), "{paths:?}");
        for f in &fonts {
            assert!(!f.ships.is_empty(), "{}", f.path);
            assert!(!f.copyright.is_empty() && f.copyright.iter().all(|c| is_copyright_line(c)), "{}: {:?}", f.path, f.copyright);
            assert!(f.source.starts_with("https://"), "{}: {}", f.path, f.source);
            assert!(!f.licence.is_empty() && !f.licence_text.is_empty(), "{}", f.path);
            assert!(f.licence_file.rsplit_once('/').map(|(d, _)| d) == f.path.rsplit_once('/').map(|(d, _)| d), "{}: {}", f.path, f.licence_file);
            // The licence text is the notice and the licence, nothing above them (D5).
            assert!(f.licence_text.lines().next().is_some_and(is_copyright_line), "{}: {:?}", f.path, f.licence_text.lines().next());
        }
        // Different fonts carry different notices (Geist and Geist Mono share the release's one
        // OFL.txt; the sketch and the Japanese fonts have their own).
        let notice_of = |name: &str| fonts.iter().find(|f| f.path.ends_with(name)).map(|f| f.copyright.concat()).unwrap_or_default();
        let (geist, mono) = (notice_of("Geist-Regular.ttf"), notice_of("GeistMono-Regular.ttf"));
        let (liberation, biz) = (notice_of("LiberationSans-Regular.ttf"), notice_of("BIZUDPGothic-Regular.ttf"));
        assert!(geist.contains("Geist Project Authors") && mono == geist, "{geist:?} {mono:?}");
        assert!(liberation.contains("Red Hat") && biz.contains("BIZ UDGothic Project Authors"), "{liberation:?} {biz:?}");
    }

    /// The notice's font list is the bundled-font list, exactly, in both directions, with each
    /// font's way of shipping, copyright notice, source and licence (the Generated files workflow
    /// rewrites the notice on main; this test is what tells a pull request).
    #[test]
    fn the_notice_lists_exactly_the_bundled_fonts() {
        let notice = notice();
        let list = font_list(&notice);
        let listed: BTreeSet<String> =
            list.iter().filter_map(|l| l.strip_prefix("- ")).filter_map(|l| l.split_whitespace().next()).map(str::to_string).collect();
        let bundled: BTreeSet<String> = fonts::list(&crate::root()).unwrap_or_else(|e| panic!("{e}")).into_iter().map(|f| f.path).collect();
        assert!(!bundled.is_empty());
        assert_eq!(listed, bundled, "{FILE}'s font list and {} differ (run `cargo xtask licences`)", fonts::LIST);
        for f in fonts() {
            let entry = list
                .iter()
                .skip_while(|l| !l.starts_with(&format!("- {} ", f.path)))
                .skip(1)
                .take_while(|l| !l.is_empty())
                .copied()
                .collect::<Vec<_>>();
            assert!(!entry.is_empty(), "{FILE}: no entry for {}", f.path);
            for ships in &f.ships {
                assert!(entry.contains(&format!("  Ships: {}", ships.describe()).as_str()), "{FILE}: {} lacks how it ships: {entry:?}", f.path);
            }
            for line in &f.copyright {
                assert!(entry.contains(&format!("  {line}").as_str()), "{FILE} lacks the copyright notice of {}: {entry:?}", f.path);
            }
            assert!(entry.contains(&format!("  Source: {}", f.source).as_str()), "{FILE} lacks the source of {}: {entry:?}", f.path);
            assert!(entry.contains(&format!("  Licence: {}", f.licence_file).as_str()), "{FILE} lacks the licence file of {}: {entry:?}", f.path);
            assert!(notice.contains(&f.licence_text), "{FILE} lacks the licence text of {}", f.path);
        }
    }

    /// Every path the font section prints exists: the fonts, their licence files, the sources
    /// that ship them, and anything inside the licence texts (D5: a header naming
    /// `crates/sketch/fonts/…` was copied in from a licence file).
    #[test]
    fn the_notice_names_no_missing_file() {
        let notice = notice();
        let section = font_section(&notice);
        let named = named_paths(section);
        assert!(named.iter().any(|p| fonts::font_ext(p)), "{FILE} has no font list: {named:?}");
        let missing: Vec<&String> = named.iter().filter(|p| !crate::root().join(p).is_file()).collect();
        assert!(missing.is_empty(), "{FILE} names files that do not exist: {missing:?} (run `cargo xtask licences`)");
        assert!(named.contains(fonts::LIST), "{named:?}");
    }

    /// The notice's per-font lines are only its copyright notices and the fields the notice
    /// adds, and each licence text begins with a copyright notice: no header from a licence
    /// file is printed (D5).
    #[test]
    fn the_notice_prints_only_copyright_lines_and_fields_for_each_font() {
        let notice = notice();
        let list = font_list(&notice);
        assert!(list.iter().any(|l| l.starts_with("- ")), "{list:?}");
        for line in list.iter().filter(|l| l.starts_with("  ")) {
            let field = ["  Ships: ", "  Source: ", "  Licence: "].iter().any(|f| line.starts_with(f));
            assert!(field || is_copyright_line(line), "{FILE}: not a copyright notice or a field: {line:?}");
        }
        let section = font_section(&notice);
        let rule = format!("{}\n", "=".repeat(80));
        let mut blocks = section.split(rule.as_str()).skip(1);
        // The heading and the list, then (names, text) pairs.
        let _list = blocks.next();
        let mut texts = 0;
        while let (Some(_names), Some(text)) = (blocks.next(), blocks.next()) {
            assert!(
                text.trim().lines().next().is_some_and(is_copyright_line),
                "{FILE}: a licence text begins without a copyright notice: {:?}",
                text.trim().lines().next()
            );
            texts += 1;
        }
        assert!(texts >= 3, "{texts} licence texts");
    }

    #[test]
    fn only_copyright_lines_make_the_notice_and_the_licence_text_starts_at_one() {
        let file = "Liberation Sans (crates/sketch/fonts/LiberationSans-Regular.ttf)\n\n\
                    Digitized data copyright (c) 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine.\n\
                    Copyright (c) 2012 Red Hat, Inc. with Reserved Font Name Liberation.\n\
                    © 2026 Someone Else\n\n\
                    This Font Software is licensed under the SIL Open Font License, Version 1.1.\n\n\
                    SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007\n\n\
                    \"Copyright Holder\" refers to any person or persons who have added contributions.\n";
        let text = licence_text(file);
        assert!(text.starts_with("Digitized data copyright (c) 2010"), "{text}");
        assert!(!text.contains("crates/sketch"), "{text}");
        assert!(text.ends_with("added contributions."), "{text}");
        assert_eq!(
            copyright_lines(&text),
            [
                "Digitized data copyright (c) 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine.",
                "Copyright (c) 2012 Red Hat, Inc. with Reserved Font Name Liberation.",
                "© 2026 Someone Else"
            ]
        );
        // A file that is only the licence text has no notice: `describe` rejects it.
        assert!(copyright_lines(&licence_text("SIL OPEN FONT LICENSE Version 1.1\n\n\"Copyright Holder\" refers to…\n")).is_empty());
        assert_eq!(
            named_paths("- a/B.ttf  (OFL)\n  Licence: a/OFL.txt\n  by crates/x/src/y.rs (https://x.y/z.txt) and/or `docs/f.md`."),
            BTreeSet::from(["a/B.ttf".to_string(), "a/OFL.txt".to_string(), "crates/x/src/y.rs".to_string(), "docs/f.md".to_string()])
        );
    }

    #[test]
    fn each_font_has_the_licence_file_beside_it() {
        let root = crate::root();
        assert_eq!(
            licence_beside(&root, "assets/design-kit/2026-11-02/fonts/Inter-400.ttf").as_deref(),
            Ok("assets/design-kit/2026-11-02/fonts/Inter-OFL.txt")
        );
        assert_eq!(
            licence_beside(&root, "assets/design-kit/2026-11-02/fonts/IBM-Plex-Mono-400.ttf").as_deref(),
            Ok("assets/design-kit/2026-11-02/fonts/IBM-Plex-Mono-OFL.txt")
        );
        assert_eq!(
            licence_beside(&root, "assets/fonts/liberation-sans/LiberationSans-Regular.ttf").as_deref(),
            Ok("assets/fonts/liberation-sans/OFL.txt")
        );
        assert!(licence_beside(&root, "assets/design-kit/2026-11-02/fonts/Other-400.ttf").is_err());
        // A lone licence file named for another font is not this font's: the kit folder without
        // Inter-OFL.txt must not hand Inter the IBM Plex notice.
        let tree = std::env::temp_dir().join(format!("polykrom-xtask-licences-{}", std::process::id()));
        let fonts = tree.join("fonts");
        std::fs::create_dir_all(&fonts).unwrap_or_else(|e| panic!("{}: {e}", fonts.display()));
        for name in ["Inter-400.ttf", "Inter-700.ttf", "IBM-Plex-Mono-400.ttf"] {
            std::fs::write(fonts.join(name), b"font").unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let plex = "Copyright © 2017 IBM Corp. with Reserved Font Name \"Plex\"\n\nThis Font Software is licensed under the SIL Open Font License, Version 1.1.\n";
        std::fs::write(fonts.join("IBM-Plex-Mono-OFL.txt"), plex).unwrap_or_else(|e| panic!("{e}"));
        let inter = licence_beside(&tree, "fonts/Inter-400.ttf");
        let inter_bold = licence_beside(&tree, "fonts/Inter-700.ttf");
        let plex_mono = licence_beside(&tree, "fonts/IBM-Plex-Mono-400.ttf");
        let _ = std::fs::remove_dir_all(&tree);
        assert!(inter.as_ref().is_err_and(|e| e.contains("no licence file beside the bundled font fonts/Inter-400.ttf")), "{inter:?}");
        assert!(inter_bold.is_err(), "{inter_bold:?}");
        assert_eq!(plex_mono.as_deref(), Ok("fonts/IBM-Plex-Mono-OFL.txt"));
        let liberation = std::fs::read_to_string(root.join("assets/fonts/liberation-sans/OFL.txt")).unwrap_or_default();
        assert_eq!(copyright_lines(&liberation).len(), 2, "both the Red Hat and the Google notices");
        assert_eq!(url("https://github.com/IBM/plex, bundled with the kit"), Some("https://github.com/IBM/plex"));
        assert_eq!(url("Inter (https://github.com/rsms/inter)"), Some("https://github.com/rsms/inter"));
    }
}

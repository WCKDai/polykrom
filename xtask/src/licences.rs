//! `cargo xtask licences [--check]`: THIRD-PARTY-LICENSES.txt, the licences of every crate built
//! into the shipped programs (solidcraft, solidcraft-cli, polykrom-web), read from
//! `cargo metadata` and the crates' own licence files, and of every font the programs carry
//! (compiled in with `include_bytes!`, or copied beside the web app), found by name in the sources
//! of the crates built into them, with each font's copyright notice from the licence file beside it
//! and its source from its ATTRIBUTION.md row. The programs embed it (Help ▸ About ▸ Licences,
//! `polykrom-cli licences`), so a release download needs no licence files beside it.
//! `--check` (part of `cargo xtask ci`) generates it without writing it: it fails when the
//! dependency graph can't be read or a font lacks its licence or attribution, and only reports a
//! file that is out of date, since pull requests leave it to the Generated files workflow
//! (`.github/workflows/generated.yml`), which rewrites it on main after each merge.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cargo;

pub const FILE: &str = "THIRD-PARTY-LICENSES.txt";
const ROOTS: &[&str] = &["polykrom", "polykrom-cli", "polykrom-web"];
/// The heading of the font section of the notice.
const FONTS_HEADING: &str = "Bundled fonts";
/// File extensions of font files.
const FONT_EXTS: &[&str] = &["ttf", "otf", "ttc", "woff", "woff2"];
/// Folders never searched for fonts or sources: build output, local state and third-party trees.
const SKIPPED_DIRS: &[&str] = &["plan", "dist", "node_modules", "vendor"];

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

/// What `cargo metadata` says is built into the shipped programs.
struct Graph {
    /// The third-party crates, sorted by name and version.
    crates: Vec<Crate>,
    /// The package directories of the workspace crates built into the programs.
    shipped: Vec<PathBuf>,
}

fn generate(root: &Path) -> Result<String, String> {
    let graph = graph()?;
    let fonts = bundled_fonts(root, &graph.shipped)?;
    Ok(render(&graph.crates, &fonts))
}

fn graph() -> Result<Graph, String> {
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
    let mut shipped = Vec::new();
    for id in seen {
        let Some(p) = packages.get(id) else { continue };
        let dir = Path::new(p["manifest_path"].as_str().unwrap_or_default()).parent().map(Path::to_path_buf).unwrap_or_default();
        if members.contains(id) {
            shipped.push(dir);
            continue;
        }
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
    shipped.sort();
    Ok(Graph { crates, shipped })
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

/// A font file the programs carry: compiled in with `include_bytes!`, or copied beside the web
/// app by its `index.html`; found by its file name in the sources of the crates built into the
/// programs.
struct Font {
    /// Repository-relative path, `/`-separated.
    path: String,
    /// The copyright notices at the head of its licence file.
    copyright: Vec<String>,
    /// Where it comes from: the URL of its ATTRIBUTION.md row (else the row's whole Source cell).
    source: String,
    /// Its licence as its ATTRIBUTION.md row names it.
    licence: String,
    /// Repository-relative path of the licence file beside it, and that file's text.
    licence_file: String,
    licence_text: String,
}

/// Every font the programs carry, sorted by path, each with its notice, source and licence. An
/// error names a font without an ATTRIBUTION.md row or a licence file beside it (the SIL OFL
/// requires the copyright notice and the licence text to accompany the font).
fn bundled_fonts(root: &Path, shipped: &[PathBuf]) -> Result<Vec<Font>, String> {
    let files = font_files(root)?;
    let rows = attribution(root)?;
    referenced_fonts(shipped, &files)?.into_iter().map(|path| describe(root, path, &rows)).collect()
}

fn font_ext(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    FONT_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

fn walk(dir: &Path, skip_dir: &dyn Fn(&str) -> bool, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().into_owned();
        let kind = e.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            if !(name.starts_with('.') || name.starts_with("target") || skip_dir(&name)) {
                walk(&e.path(), skip_dir, out)?;
            }
        } else if kind.is_file() {
            out.push(e.path());
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// The repository's font files by file name (the sources name a font by its file name), with their
/// repository-relative paths.
fn font_files(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut all = Vec::new();
    walk(root, &|name| SKIPPED_DIRS.contains(&name), &mut all)?;
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for path in all {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !font_ext(&name) {
            continue;
        }
        let rel = relative(root, &path);
        if let Some(other) = files.insert(name.clone(), rel.clone()) {
            return Err(format!("licences: two font files are both named {name}: {other} and {rel}; the sources name fonts by file name"));
        }
    }
    Ok(files)
}

/// The source text before an inline test module (`#[cfg(test)]` followed by `mod …`): what a
/// program is built from.
fn production_part(source: &str) -> &str {
    let mut lines = source.split_inclusive('\n');
    let mut offset = 0;
    while let Some(line) = lines.next() {
        if line.trim() == "#[cfg(test)]" && lines.clone().next().is_some_and(|next| next.trim_start().starts_with("mod ")) {
            return &source[..offset];
        }
        offset += line.len();
    }
    source
}

/// The file names of the fonts a source names in quoted literals: `"Inter-400.ttf"`,
/// `"/../../assets/fonts/liberation-sans/LiberationSans-Regular.ttf"`, `href="…/x.ttf"`.
fn font_names(source: &str) -> impl Iterator<Item = &str> {
    source.split('"').skip(1).step_by(2).filter(|literal| font_ext(literal)).map(|literal| literal.rsplit(['/', '\\']).next().unwrap_or(literal))
}

/// The repository fonts named in the production sources (`.rs`, and the web app's `.html`) of
/// the shipped crates' directories.
fn referenced_fonts(shipped: &[PathBuf], files: &BTreeMap<String, String>) -> Result<BTreeSet<String>, String> {
    let mut found = BTreeSet::new();
    for dir in shipped {
        let mut sources = Vec::new();
        walk(dir, &|name| matches!(name, "tests" | "benches" | "examples"), &mut sources)?;
        for path in sources {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let rust = name.ends_with(".rs") && !name.ends_with("_test.rs") && !name.ends_with("_tests.rs");
            if !(rust || name.ends_with(".html")) {
                continue;
            }
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let text = if rust { production_part(&text) } else { text.as_str() };
            found.extend(font_names(text).filter_map(|n| files.get(n).cloned()));
        }
    }
    Ok(found)
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
            !font_ext(n) && (u.contains("OFL") || ["LICENSE", "LICENCE", "COPYING"].iter().any(|k| u.starts_with(k)))
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
        [one] => Ok(relative(root, &dir.join(one))),
        [] => Err(format!("licences: no licence file beside the bundled font {font} (expected OFL.txt or <name>-OFL.txt in {})", dir.display())),
        many => Err(format!("licences: several licence files could be the bundled font {font}'s: {}", many.join(", "))),
    }
}

/// The copyright notices at the head of a font licence file: the lines naming a copyright before
/// the licence text begins.
fn copyright_lines(licence: &str) -> Vec<String> {
    licence
        .lines()
        .take_while(|l| !l.starts_with("---") && !l.to_ascii_uppercase().contains("OPEN FONT LICENSE VERSION"))
        .filter(|l| l.to_ascii_lowercase().contains("copyright"))
        .map(|l| l.trim().to_string())
        .collect()
}

fn describe(root: &Path, path: String, rows: &BTreeMap<String, Row>) -> Result<Font, String> {
    let row = rows.get(&path).ok_or_else(|| format!("licences: the bundled font {path} has no ATTRIBUTION.md row"))?;
    let licence_file = licence_beside(root, &path)?;
    let licence_text = std::fs::read_to_string(root.join(&licence_file)).map_err(|e| format!("{licence_file}: {e}"))?;
    let licence_text = licence_text.replace("\r\n", "\n").trim().to_string();
    let copyright = copyright_lines(&licence_text);
    if copyright.is_empty() {
        return Err(format!("licences: {licence_file} carries no copyright notice before the licence text"));
    }
    Ok(Font {
        source: url(&row.source).unwrap_or(&row.source).to_string(),
        licence: row.licence.clone(),
        path,
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

    // The fonts the programs carry, each with its copyright notice, then one copy of each
    // distinct licence text (a font's licence file begins with its own notice).
    out += &format!(
        "\n{rule}\n{FONTS_HEADING}\n{rule}\n\n\
         The fonts compiled into the programs or shipped beside the web app, found by `cargo xtask licences`\n\
         in the sources of the crates built into them, each with the copyright notice and licence of the\n\
         file beside it (ATTRIBUTION.md names its source). The licence texts follow.\n\n"
    );
    for f in fonts {
        out += &format!("- {}  ({})\n", f.path, f.licence);
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

    /// The font list of the notice: the lines after the heading, up to the next rule.
    fn font_list(notice: &str) -> Vec<&str> {
        let mut lines = notice.lines().skip_while(|l| !l.starts_with(FONTS_HEADING)).skip(2);
        lines.by_ref().take_while(|l| !l.starts_with("====")).collect()
    }

    /// The repository files the font list names: every token with a `/` ending in a font or text
    /// file, without the brackets around it.
    fn listed_paths(list: &[&str]) -> BTreeSet<String> {
        list.iter()
            .flat_map(|l| l.split_whitespace())
            .map(|w| w.trim_matches(['(', ')', ',', ';']))
            .filter(|w| w.contains('/') && !w.contains("://") && (font_ext(w) || w.ends_with(".txt")))
            .map(str::to_string)
            .collect()
    }

    fn fonts() -> Vec<Font> {
        let shipped = graph().map(|g| g.shipped).unwrap_or_default();
        assert!(shipped.iter().any(|d| d.ends_with("crates/ui-egui")) && shipped.iter().any(|d| d.ends_with("apps/polykrom-web")), "{shipped:?}");
        bundled_fonts(&crate::root(), &shipped).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn the_sources_name_the_fonts_the_programs_carry() {
        let fonts = fonts();
        let paths: Vec<&str> = fonts.iter().map(|f| f.path.as_str()).collect();
        // Sketch text (crates/sketch), the interface and its numbers (crates/ui-egui), and the web
        // app's Japanese face (apps/polykrom-web/index.html); the web site's own JetBrains Mono is
        // not the app's.
        for expected in [
            "assets/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "assets/design-kit/2026-11-02/fonts/Inter-400.ttf",
            "assets/design-kit/2026-11-02/fonts/IBM-Plex-Mono-400.ttf",
            "assets/fonts/biz-udp-gothic/BIZUDPGothic-Regular.ttf",
        ] {
            assert!(paths.contains(&expected), "{expected} not among {paths:?}");
        }
        assert!(!paths.iter().any(|p| p.contains("jetbrains")), "{paths:?}");
        for f in &fonts {
            assert!(
                !f.copyright.is_empty() && f.copyright.iter().all(|c| c.to_ascii_lowercase().contains("copyright")),
                "{}: {:?}",
                f.path,
                f.copyright
            );
            assert!(f.source.starts_with("https://"), "{}: {}", f.path, f.source);
            assert!(!f.licence.is_empty() && !f.licence_text.is_empty(), "{}", f.path);
            assert!(f.licence_file.rsplit_once('/').map(|(d, _)| d) == f.path.rsplit_once('/').map(|(d, _)| d), "{}: {}", f.path, f.licence_file);
        }
        // Different fonts carry different notices.
        let inter = fonts.iter().find(|f| f.path.ends_with("Inter-400.ttf")).map(|f| f.copyright.clone()).unwrap_or_default();
        let plex = fonts.iter().find(|f| f.path.ends_with("IBM-Plex-Mono-400.ttf")).map(|f| f.copyright.clone()).unwrap_or_default();
        assert!(inter.concat().contains("Inter Project Authors") && plex.concat().contains("IBM Corp"), "{inter:?} {plex:?}");
    }

    #[test]
    fn every_font_the_programs_carry_is_in_the_notice() {
        let notice = notice();
        let listed = listed_paths(&font_list(&notice));
        let fonts = fonts();
        let missing: Vec<&str> = fonts.iter().filter(|f| !listed.contains(&f.path)).map(|f| f.path.as_str()).collect();
        assert!(missing.is_empty(), "fonts the programs carry but {FILE} does not list: {missing:?} (run `cargo xtask licences`)");
        for f in &fonts {
            assert!(listed.contains(&f.licence_file) && notice.contains(&f.licence_text), "{FILE} lacks the licence of {}", f.path);
            assert!(f.copyright.iter().all(|c| notice.contains(c.as_str())), "{FILE} lacks the copyright notice of {}", f.path);
        }
    }

    #[test]
    fn the_notice_lists_exactly_the_fonts_the_programs_carry() {
        // Both directions: a font the sources no longer name leaves the notice too (the Generated
        // files workflow rewrites it on main; this test is what tells a pull request).
        let listed: BTreeSet<String> = listed_paths(&font_list(&notice())).into_iter().filter(|p| font_ext(p)).collect();
        let derived: BTreeSet<String> = fonts().into_iter().map(|f| f.path).collect();
        assert!(!derived.is_empty());
        assert_eq!(listed, derived, "{FILE}'s font list and the fonts the programs carry differ (run `cargo xtask licences`)");
    }

    #[test]
    fn the_notice_names_no_missing_file() {
        let listed = listed_paths(&font_list(&notice()));
        assert!(!listed.is_empty(), "{FILE} has no font list");
        let missing: Vec<&String> = listed.iter().filter(|p| !crate::root().join(p).is_file()).collect();
        assert!(missing.is_empty(), "{FILE} names files that do not exist: {missing:?} (run `cargo xtask licences`)");
    }

    #[test]
    fn font_names_come_from_quoted_literals_before_the_test_module() {
        let source = "static A: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../assets/fonts/x/Y.ttf\"));\n\
                      const B: &[u8] = kit_font!(\"Inter-400.ttf\");\n\
                      const C: &str = \"C:\\\\Windows\\\\Fonts\\\\yumin.ttf\";\n\
                      // a .ttf/.otf file path\n\
                      #[cfg(test)]\n\
                      const W: u16 = 400;\n\
                      #[cfg(test)]\n\
                      mod tests {\n    const T: &[u8] = include_bytes!(\"test-only.otf\");\n}\n";
        assert_eq!(font_names(production_part(source)).collect::<Vec<_>>(), ["Y.ttf", "Inter-400.ttf", "yumin.ttf"]);
        assert_eq!(font_names(source).collect::<Vec<_>>(), ["Y.ttf", "Inter-400.ttf", "yumin.ttf", "test-only.otf"]);
        assert_eq!(
            font_names("<link rel=\"copy-file\" href=\"../../assets/fonts/b/B.woff2\" data-target-path=\"fonts\" />").collect::<Vec<_>>(),
            ["B.woff2"]
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

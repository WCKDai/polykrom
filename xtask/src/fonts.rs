//! The fonts the builds ship, as `assets/fonts/bundled.txt` lists them: the one list every font a
//! build carries comes from, with its path and how it ships (compiled into the interface by
//! crates/ui-egui/src/fonts.rs, compiled into the sketch text engine by crates/sketch/src/fonts.rs,
//! or copied beside the web app by apps/polykrom-web/index.html). The code that embeds or copies
//! the fonts is checked against it: the two crates' tests compare their compiled-in bytes with the
//! listed files, and [`check`] here finds every `include_bytes!`/`include_str!` of a font file in
//! every Rust source of the repository, read whole (a test module declared near the top of a
//! lib.rs hides nothing), and every `copy-file`/`copy-dir` of one in `apps/*/index.html`, and
//! requires exactly the listed set. `cargo xtask licences` writes the "Bundled fonts" section of
//! THIRD-PARTY-LICENSES.txt from the list.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};

/// The list, relative to the repository root.
pub const LIST: &str = "assets/fonts/bundled.txt";
/// File extensions of font files.
const FONT_EXTS: &[&str] = &["ttf", "otf", "ttc", "otc", "woff", "woff2"];
/// Folders never searched for sources: local state, build output and web dependencies (`target*`
/// and dot folders are skipped too).
const SKIPPED_DIRS: &[&str] = &["plan", "dist", "node_modules"];

/// How a listed font ships.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ships {
    /// Compiled into the interface by crates/ui-egui/src/fonts.rs: the desktop app and the web app.
    Interface,
    /// Compiled into the sketch text engine by crates/sketch/src/fonts.rs: every program.
    Sketch,
    /// Copied beside the web app by apps/polykrom-web/index.html.
    Web,
}

impl Ships {
    const ALL: [Ships; 3] = [Ships::Interface, Ships::Sketch, Ships::Web];

    /// The word the list uses.
    pub fn word(self) -> &'static str {
        match self {
            Ships::Interface => "interface",
            Ships::Sketch => "sketch",
            Ships::Web => "web",
        }
    }

    /// What the word means, as the notice says it.
    pub fn describe(self) -> &'static str {
        match self {
            Ships::Interface => "compiled into the interface by crates/ui-egui/src/fonts.rs (the desktop app and the web app)",
            Ships::Sketch => "compiled into the sketch text engine by crates/sketch/src/fonts.rs (every program)",
            Ships::Web => "copied beside the web app by apps/polykrom-web/index.html",
        }
    }

    /// The folder whose Rust sources must embed a font that ships this way (the web app copies
    /// its fonts instead).
    fn embedded_under(self) -> Option<&'static str> {
        match self {
            Ships::Interface => Some("crates/ui-egui/"),
            Ships::Sketch => Some("crates/sketch/"),
            Ships::Web => None,
        }
    }

    fn parse(word: &str) -> Option<Ships> {
        Self::ALL.into_iter().find(|s| s.word() == word)
    }
}

/// A listed font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundled {
    /// Repository-relative path, `/`-separated.
    pub path: String,
    /// How it ships (at least one way).
    pub ships: Vec<Ships>,
}

pub fn font_ext(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    FONT_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// The list, parsed, each entry an existing font file.
pub fn list(root: &Path) -> Result<Vec<Bundled>, String> {
    let text = std::fs::read_to_string(root.join(LIST)).map_err(|e| format!("{LIST}: {e}"))?;
    let fonts = parse(&text)?;
    for f in &fonts {
        if !root.join(&f.path).is_file() {
            return Err(format!("{LIST}: {} is not a file", f.path));
        }
    }
    Ok(fonts)
}

/// The list's text: `<path> <ships>...` lines, sorted by path, each font once; `#` comments.
pub fn parse(text: &str) -> Result<Vec<Bundled>, String> {
    let mut fonts: Vec<Bundled> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = format!("{LIST}:{}", i + 1);
        let mut words = line.split_whitespace();
        let path = words.next().unwrap_or_default();
        if !font_ext(path) || path.starts_with('/') || path.split('/').any(|c| c.is_empty() || c == "." || c == "..") {
            return Err(format!("{at}: {path} is not a repository-relative font file path"));
        }
        let mut ships = Vec::new();
        for w in words {
            let known = Ships::ALL.map(Ships::word).join(", ");
            let s = Ships::parse(w).ok_or_else(|| format!("{at}: `{w}` is not a way a font ships ({known})"))?;
            if ships.contains(&s) {
                return Err(format!("{at}: {w} twice"));
            }
            ships.push(s);
        }
        if ships.is_empty() {
            return Err(format!("{at}: {path} does not say how it ships ({})", Ships::ALL.map(Ships::word).join(", ")));
        }
        if let Some(prev) = fonts.last()
            && prev.path.as_str() >= path
        {
            return Err(format!("{at}: {path} must come after {} (sorted by path, each font once)", prev.path));
        }
        fonts.push(Bundled { path: path.to_string(), ships });
    }
    if fonts.is_empty() {
        return Err(format!("{LIST} lists no font"));
    }
    Ok(fonts)
}

/// The fonts the sources ship: each font file (repository-relative) with the sources that do it.
#[derive(Debug, Default)]
pub struct Shipped {
    /// Fonts a Rust source includes with `include_bytes!`/`include_str!`, anywhere in it.
    pub embedded: BTreeMap<String, BTreeSet<String>>,
    /// Fonts a web app's `index.html` copies beside it.
    pub copied: BTreeMap<String, BTreeSet<String>>,
}

/// Every font any Rust source of the repository embeds and any `apps/*/index.html` copies.
pub fn shipped(root: &Path) -> Result<Shipped, String> {
    let mut files = Vec::new();
    walk(root, &mut files)?;
    let mut out = Shipped::default();
    for path in files {
        let rel = relative(root, &path);
        let web = rel.starts_with("apps/") && rel.ends_with("/index.html") && rel.matches('/').count() == 2;
        if !(rel.ends_with(".rs") || web) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
        let (fonts, into) =
            if web { (web_copies(root, &path, &text)?, &mut out.copied) } else { (rust_embeds(root, &path, &text)?, &mut out.embedded) };
        for font in fonts {
            into.entry(font).or_default().insert(rel.clone());
        }
    }
    Ok(out)
}

/// The list, checked against what the sources ship: every embedded or copied font is listed,
/// every listed font is embedded or copied, the fonts the web app copies are exactly those listed
/// as `web`, and a font listed as `interface` or `sketch` is embedded under its crate. An error
/// names each difference and the source behind it.
pub fn check(root: &Path) -> Result<Vec<Bundled>, String> {
    let list = list(root)?;
    let shipped = shipped(root)?;
    let listed: BTreeMap<&str, &Bundled> = list.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut problems = Vec::new();
    let by = |files: &BTreeSet<String>| files.iter().cloned().collect::<Vec<_>>().join(", ");
    for (font, files) in &shipped.embedded {
        if !listed.contains_key(font.as_str()) {
            problems.push(format!("{font} is embedded by {} but not in {LIST}", by(files)));
        }
    }
    for (font, files) in &shipped.copied {
        match listed.get(font.as_str()) {
            None => problems.push(format!("{font} is copied beside the web app by {} but not in {LIST}", by(files))),
            Some(f) if !f.ships.contains(&Ships::Web) => {
                problems.push(format!("{font} is copied beside the web app by {} but {LIST} does not list it as web", by(files)));
            }
            Some(_) => {}
        }
    }
    for f in &list {
        let embedded = shipped.embedded.get(&f.path);
        let copied = shipped.copied.get(&f.path);
        if embedded.is_none() && copied.is_none() {
            problems.push(format!("{} is in {LIST} but no source embeds or copies it", f.path));
            continue;
        }
        for ships in &f.ships {
            match ships.embedded_under() {
                Some(dir) => {
                    if !embedded.is_some_and(|files| files.iter().any(|file| file.starts_with(dir))) {
                        problems.push(format!("{} is listed as {} but no Rust source under {dir} embeds it", f.path, ships.word()));
                    }
                }
                None => {
                    if copied.is_none() {
                        problems.push(format!("{} is listed as {} but no apps/*/index.html copies it", f.path, ships.word()));
                    }
                }
            }
        }
    }
    if problems.is_empty() { Ok(list) } else { Err(format!("bundled fonts ({LIST}):\n  {}", problems.join("\n  "))) }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().into_owned();
        let kind = e.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            if !(name.starts_with('.') || name.starts_with("target") || SKIPPED_DIRS.contains(&name.as_str())) {
                walk(&e.path(), out)?;
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

/// `..` and `.` resolved lexically.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(out.components().next_back(), Some(Component::Normal(_))) || !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The repository-relative path of a file a source names, or an error for one outside it.
fn inside(root: &Path, full: &Path, what: &str) -> Result<String, String> {
    let full = normalize(full);
    full.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("{what} names {} outside the repository", full.display()))
}

/// The package directory a source belongs to (the nearest folder with a Cargo.toml), else the root.
fn manifest_dir<'a>(root: &'a Path, dir: &'a Path) -> &'a Path {
    dir.ancestors().take_while(|d| d.starts_with(root)).find(|d| d.join("Cargo.toml").is_file()).unwrap_or(root)
}

/// The font files a Rust source embeds with `include_bytes!` or `include_str!`, wherever they are
/// in it (after a `mod x_tests;` line, inside a test module, a function or a macro body):
/// repository-relative paths. An include the guard cannot read (a macro variable, an environment
/// variable other than CARGO_MANIFEST_DIR with a font behind it, anything but string literals in
/// `concat!` and `env!`) is an error, so no font can hide behind one.
pub fn rust_embeds(root: &Path, file: &Path, source: &str) -> Result<Vec<String>, String> {
    let rel = relative(root, file);
    let tokens: TokenStream = source.parse().map_err(|e| format!("{rel}: not readable as Rust tokens: {e}"))?;
    let dir = file.parent().unwrap_or(root);
    let manifest = manifest_dir(root, dir);
    let mut args = Vec::new();
    includes(tokens, &mut args);
    let mut found = Vec::new();
    for args in args {
        let mut parts = Parts::default();
        include_parts(args, &mut parts).map_err(|e| format!("{rel}: an include_bytes!/include_str! the bundled-font guard cannot read: {e}"))?;
        if !font_ext(&parts.literal) {
            continue;
        }
        let joined = match parts.env.as_deref() {
            None => dir.join(&parts.literal),
            Some("CARGO_MANIFEST_DIR") => PathBuf::from(format!("{}{}", manifest.display(), parts.literal)),
            Some(other) => {
                return Err(format!("{rel}: includes the font {} from ${other}, which the bundled-font guard cannot follow", parts.literal));
            }
        };
        found.push(inside(root, &joined, &rel)?);
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// The argument groups of every `include_bytes!`/`include_str!` in a token stream, at any depth.
fn includes(tokens: TokenStream, out: &mut Vec<TokenStream>) {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    for (i, tree) in trees.iter().enumerate() {
        if let TokenTree::Group(g) = tree {
            includes(g.stream(), out);
        }
        if let TokenTree::Ident(id) = tree
            && (id == "include_bytes" || id == "include_str")
            && matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
            && let Some(TokenTree::Group(g)) = trees.get(i + 2)
        {
            out.push(g.stream());
        }
    }
}

/// What an include's arguments name: the string literals joined, and the `env!` variable.
#[derive(Default)]
struct Parts {
    env: Option<String>,
    literal: String,
}

fn include_parts(tokens: TokenStream, parts: &mut Parts) -> Result<(), String> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut i = 0;
    while i < trees.len() {
        match &trees[i] {
            TokenTree::Literal(l) => {
                parts.literal.push_str(&string_literal(l)?);
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == ',' => i += 1,
            TokenTree::Ident(id)
                if (id == "concat" || id == "env")
                    && matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                    && matches!(trees.get(i + 2), Some(TokenTree::Group(_))) =>
            {
                let Some(TokenTree::Group(g)) = trees.get(i + 2) else { break };
                if id == "env" {
                    let mut inner = Parts::default();
                    include_parts(g.stream(), &mut inner)?;
                    if inner.env.is_some() || parts.env.replace(inner.literal).is_some() {
                        return Err("more than one environment variable".to_string());
                    }
                } else {
                    include_parts(g.stream(), parts)?;
                }
                i += 3;
            }
            other => {
                return Err(format!("`{other}`: name the file with string literals (joined by concat!, after env!(\"CARGO_MANIFEST_DIR\") at most)"));
            }
        }
    }
    Ok(())
}

/// The text of a string literal token (`"…"`, `r"…"`, `r#"…"#`).
fn string_literal(l: &proc_macro2::Literal) -> Result<String, String> {
    let text = l.to_string();
    if let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Ok(inner.replace("\\\\", "\\").replace("\\\"", "\""));
    }
    if let Some(raw) = text.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let quoted = &raw[hashes..raw.len() - hashes];
        if let Some(inner) = quoted.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
            return Ok(inner.to_string());
        }
    }
    Err(format!("`{text}` is not a string literal"))
}

/// The font files an `index.html` copies beside the web app (`<link data-trunk rel="copy-file"
/// href="…">`, or every font under a `copy-dir`): repository-relative paths.
pub fn web_copies(root: &Path, html: &Path, text: &str) -> Result<Vec<String>, String> {
    let rel = relative(root, html);
    let dir = html.parent().unwrap_or(root);
    let mut found = Vec::new();
    for tag in text.split("<link").skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        let (Some(kind), Some(href)) = (attr(tag, "rel"), attr(tag, "href")) else { continue };
        match kind {
            "copy-file" => {
                if font_ext(href) {
                    found.push(inside(root, &dir.join(href), &rel)?);
                }
            }
            "copy-dir" => {
                let mut files = Vec::new();
                let from = normalize(&dir.join(href));
                if from.is_dir() {
                    walk(&from, &mut files)?;
                }
                for file in files.iter().filter(|f| font_ext(&f.to_string_lossy())) {
                    found.push(inside(root, file, &rel)?);
                }
            }
            _ => {}
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// The value of a double-quoted attribute of a tag.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    tag.find(&key).and_then(|at| tag[at + key.len()..].split('"').next())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIT: &str = "assets/design-kit/2026-11-02/fonts";
    /// The interface faces: Geist at 400, 500 and 600, Geist Mono at 400 and 500 (docs/fonts.md).
    const GEIST: &str = "assets/fonts/geist";

    #[test]
    fn the_list_is_what_the_sources_ship() {
        let list = check(&crate::root()).unwrap_or_else(|e| panic!("{e}"));
        let ships = |path: &str| list.iter().find(|f| f.path == path).map(|f| f.ships.clone());
        for face in ["Geist-Regular", "Geist-Medium", "Geist-SemiBold", "GeistMono-Regular", "GeistMono-Medium"] {
            assert_eq!(ships(&format!("{GEIST}/{face}.ttf")), Some(vec![Ships::Interface]), "{face}");
        }
        assert_eq!(ships("assets/fonts/liberation-sans/LiberationSans-Regular.ttf"), Some(vec![Ships::Sketch]));
        assert_eq!(ships("assets/fonts/biz-udp-gothic/BIZUDPGothic-Regular.ttf"), Some(vec![Ships::Web]));
        assert_eq!(list.len(), 7, "the five interface faces, the sketch font and the web app's Japanese font: {list:?}");
        // The kit's Inter and IBM Plex Mono (they render the kit's SVG text), the unused Geist
        // weights (bold, the variable fonts) and the web site's own JetBrains Mono are not the
        // programs'.
        let unused = |path: &str| path.starts_with(KIT) || path.ends_with("-Bold.ttf") || path.contains("Variable") || path.contains("jetbrains");
        assert!(!list.iter().any(|f| unused(&f.path)), "{list:?}");
        // What the sources ship, in detail: the Geist faces by crates/ui-egui/src/fonts.rs, the
        // sketch font by crates/sketch/src/fonts.rs, the Japanese font by the web app's copy rule.
        let shipped = shipped(&crate::root()).unwrap_or_else(|e| panic!("{e}"));
        let embedded_by = |path: &str| shipped.embedded.get(path).cloned().unwrap_or_default();
        for face in ["Geist-Regular", "Geist-Medium", "Geist-SemiBold", "GeistMono-Regular", "GeistMono-Medium"] {
            assert!(embedded_by(&format!("{GEIST}/{face}.ttf")).contains("crates/ui-egui/src/fonts.rs"), "{face}: {:?}", shipped.embedded);
        }
        assert!(
            embedded_by("assets/fonts/liberation-sans/LiberationSans-Regular.ttf").contains("crates/sketch/src/fonts.rs"),
            "{:?}",
            shipped.embedded
        );
        assert_eq!(
            shipped.copied.get("assets/fonts/biz-udp-gothic/BIZUDPGothic-Regular.ttf").cloned().unwrap_or_default(),
            BTreeSet::from(["apps/polykrom-web/index.html".to_string()])
        );
    }

    #[test]
    fn the_list_says_each_font_once_sorted_with_how_it_ships() {
        let good = "# comment\n\nassets/fonts/a/A.ttf interface web\nassets/fonts/b/B.woff2 sketch\n";
        let fonts = parse(good).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(fonts.len(), 2);
        assert_eq!((fonts[0].path.as_str(), fonts[0].ships.as_slice()), ("assets/fonts/a/A.ttf", &[Ships::Interface, Ships::Web][..]));
        assert_eq!((fonts[1].path.as_str(), fonts[1].ships.as_slice()), ("assets/fonts/b/B.woff2", &[Ships::Sketch][..]));
        for (bad, why) in [
            ("assets/fonts/a/A.ttf", "does not say how it ships"),
            ("assets/fonts/a/A.ttf desktop", "is not a way a font ships"),
            ("assets/fonts/a/A.ttf web web", "web twice"),
            ("assets/fonts/a/A.txt web", "not a repository-relative font file path"),
            ("/usr/share/fonts/A.ttf web", "not a repository-relative font file path"),
            ("assets/fonts/../x/A.ttf web", "not a repository-relative font file path"),
            ("assets/fonts/b/B.ttf web\nassets/fonts/a/A.ttf web", "must come after"),
            ("assets/fonts/a/A.ttf web\nassets/fonts/a/A.ttf sketch", "must come after"),
            ("# nothing\n", "lists no font"),
        ] {
            let err = parse(bad).err().unwrap_or_default();
            assert!(err.contains(why), "{bad:?}: {err}");
        }
    }

    /// A temporary repository: `files` are (path, text), each written under a fresh folder.
    fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("polykrom-xtask-fonts-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let full = root.join(path);
            if let Some(dir) = full.parent() {
                std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            }
            std::fs::write(&full, text).unwrap_or_else(|e| panic!("{path}: {e}"));
        }
        root
    }

    fn embeds(root: &Path, file: &str, source: &str) -> Result<Vec<String>, String> {
        rust_embeds(root, &root.join(file), source)
    }

    /// An include is found wherever it is in a source (D3: the scanner this replaces cut each
    /// file at its first `#[cfg(test)]` + `mod` line, so a font after a `mod x_tests;` line near
    /// the top of a lib.rs was never seen).
    #[test]
    fn an_include_anywhere_in_a_source_is_found() {
        let root = tree("anywhere", &[("crates/ui/Cargo.toml", "[package]\nname = \"ui\"\n")]);
        let kit = format!("{KIT}/IBM-Plex-Mono-500.ttf");
        let after_test_mod = format!(
            "pub mod a;\n#[cfg(test)]\nmod x_tests;\npub mod command_help;\n\
             pub static REVIEW_FONT: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../{kit}\"));\n\
             pub mod z;\n"
        );
        assert_eq!(embeds(&root, "crates/ui/src/lib.rs", &after_test_mod), Ok(vec![kit.clone()]));
        let in_test_module = format!(
            "pub fn production() {{}}\n#[cfg(test)]\nmod tests {{\n    fn t() {{ let f = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../{kit}\")); }}\n}}\n"
        );
        assert_eq!(embeds(&root, "crates/ui/src/lib.rs", &in_test_module), Ok(vec![kit.clone()]));
        // Relative to the source's folder, through a path, as `include_str!`, in a function.
        let relative = "fn f() -> &'static str { ::std::include_str!(\"../../../assets/fonts/x/Y.otf\") }\n";
        assert_eq!(embeds(&root, "crates/ui/src/deep.rs", relative), Ok(vec!["assets/fonts/x/Y.otf".to_string()]));
        // In a macro body, named in full.
        let in_macro = "macro_rules! font { () => { include_bytes!(\"../fonts/Z.ttf\") }; }\n";
        assert_eq!(embeds(&root, "crates/ui/src/lib.rs", in_macro), Ok(vec!["crates/ui/fonts/Z.ttf".to_string()]));
        // Twice is once.
        let twice = "const A: &[u8] = include_bytes!(\"a.ttf\");\nconst B: &[u8] = include_bytes!(\"a.ttf\");\n";
        assert_eq!(embeds(&root, "crates/ui/src/lib.rs", twice), Ok(vec!["crates/ui/src/a.ttf".to_string()]));
        // Not a font: nothing.
        let other = "const S: &str = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../assets/locales/en.json\"));\n\
                     const G: &str = include_str!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n";
        assert_eq!(embeds(&root, "crates/ui/src/lib.rs", other), Ok(vec![]));
        // An include the guard cannot read is an error, so a font cannot hide behind a macro
        // variable, another environment variable or a path outside the repository.
        for (hidden, why) in [
            ("macro_rules! kit_font { ($file:literal) => { include_bytes!(concat!(\"../fonts/\", $file)) }; }\n", "cannot read"),
            ("const F: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/font.ttf\"));\n", "cannot follow"),
            ("const F: &[u8] = include_bytes!(\"../../../../elsewhere/font.ttf\");\n", "outside the repository"),
            ("const F: &[u8] = include_bytes!(font_path!());\n", "cannot read"),
        ] {
            let err = embeds(&root, "crates/ui/src/lib.rs", hidden).err().unwrap_or_default();
            assert!(err.contains(why), "{hidden}: {err}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A font only named in a quoted literal (a system-font lookup, a Windows path, a comment, a
    /// string) is not embedded (D4: the scanner this replaces listed any quoted font file name).
    #[test]
    fn a_quoted_font_name_is_not_an_embed() {
        let root = tree("quoted", &[("crates/ui/Cargo.toml", "[package]\nname = \"ui\"\n")]);
        let source = "fn find_system_cjk_font() -> Option<Vec<u8>> {\n\
                          let names = [\"BIZUDPGothic-Regular.ttf\", \"BIZ-UDGothicR.ttc\", \"NotoSansJP-Regular.ttf\"];\n\
                          let _ = names;\n    None\n}\n\
                      const CANDIDATES: &[&str] = &[\"C:\\\\Windows\\\\Fonts\\\\yumin.ttf\", \"/usr/share/fonts/opentype/ipafont/ipag.ttf\"];\n\
                      // include_bytes!(\"../../../assets/fonts/liberation-sans/LiberationSans-Regular.ttf\")\n\
                      const EXAMPLE: &str = \"include_bytes!(\\\"../../../assets/fonts/x/Y.ttf\\\")\";\n";
        assert_eq!(embeds(&root, "crates/ui/src/fonts.rs", source), Ok(vec![]));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn web_copies_come_from_the_copy_rules() {
        let root =
            tree("web", &[("assets/fonts/dir/C.woff2", "font"), ("assets/fonts/dir/OFL.txt", "licence"), ("assets/fonts/dir/sub/D.ttf", "font")]);
        let html = "<head>\n\
                    <link data-trunk rel=\"rust\" data-bin=\"polykrom-web\" />\n\
                    <link data-trunk rel=\"copy-file\" href=\"../../assets/fonts/b/B.ttf\" data-target-path=\"fonts\" />\n\
                    <link data-trunk rel=\"copy-file\" href=\"../../assets/fonts/b/OFL.txt\" data-target-path=\"fonts\" />\n\
                    <link data-trunk rel=\"copy-dir\" href=\"../../assets/fonts/dir\" />\n\
                    <link rel=\"preload\" href=\"fonts/jetbrains-mono-latin.woff2\" as=\"font\" type=\"font/woff2\" crossorigin>\n\
                    </head>\n";
        let found = web_copies(&root, &root.join("apps/web/index.html"), html).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(found, ["assets/fonts/b/B.ttf", "assets/fonts/dir/C.woff2", "assets/fonts/dir/sub/D.ttf"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The check on a whole repository: a font embedded after a `mod x_tests;` line or copied
    /// beside the web app without being listed fails it, as does a listed font nothing ships or
    /// one that ships another way than it says.
    #[test]
    fn a_font_a_build_ships_outside_the_list_fails_the_check() {
        let manifest = "[package]\nname = \"x\"\n";
        let lib =
            "#[cfg(test)]\nmod x_tests;\npub static A: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../fonts/a.ttf\"));\n";
        let hidden = "#[cfg(test)]\nmod x_tests;\npub mod command_help;\n\
                      pub static B: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../fonts/b.ttf\"));\n\
                      pub mod zz;\n";
        let sketch = "static DEFAULT_FONT: &[u8] = include_bytes!(\"../../../fonts/s.ttf\");\n";
        let html = |font: &str| format!("<link data-trunk rel=\"copy-file\" href=\"../../fonts/{font}\" data-target-path=\"fonts\" />\n");
        let base: Vec<(&str, &str)> = vec![
            ("fonts/a.ttf", "font"),
            ("fonts/b.ttf", "font"),
            ("fonts/s.ttf", "font"),
            ("fonts/w.ttf", "font"),
            ("crates/ui-egui/Cargo.toml", manifest),
            ("crates/sketch/Cargo.toml", manifest),
            ("crates/sketch/src/fonts.rs", sketch),
            ("apps/web/Cargo.toml", manifest),
        ];
        let complete = "fonts/a.ttf interface\nfonts/s.ttf sketch\nfonts/w.ttf web\n";
        let web = html("w.ttf");
        let check_with = |name: &str, extra: &[(&str, &str)]| {
            let mut files = base.clone();
            files.extend_from_slice(extra);
            let root = tree(name, &files);
            let result = check(&root).map(|list| list.into_iter().map(|f| f.path).collect::<Vec<_>>());
            let _ = std::fs::remove_dir_all(&root);
            result
        };
        assert_eq!(
            check_with("complete", &[(LIST, complete), ("crates/ui-egui/src/lib.rs", lib), ("apps/web/index.html", &web)]),
            Ok(vec!["fonts/a.ttf".to_string(), "fonts/s.ttf".to_string(), "fonts/w.ttf".to_string()])
        );
        // D3: the font embedded after the test-module line, with thousands of production lines
        // to follow, is found and is not listed.
        let err = check_with(
            "hidden",
            &[(LIST, complete), ("crates/ui-egui/src/lib.rs", lib), ("crates/ui-egui/src/more.rs", hidden), ("apps/web/index.html", &web)],
        )
        .err()
        .unwrap_or_default();
        assert!(err.contains("fonts/b.ttf is embedded by crates/ui-egui/src/more.rs but not in"), "{err}");
        // A font copied beside the web app without a listing, or listed otherwise than as web.
        let both = format!("{web}{}", html("b.ttf"));
        let err =
            check_with("copied", &[(LIST, complete), ("crates/ui-egui/src/lib.rs", lib), ("apps/web/index.html", &both)]).err().unwrap_or_default();
        assert!(err.contains("fonts/b.ttf is copied beside the web app by apps/web/index.html but not in"), "{err}");
        let as_interface = "fonts/a.ttf interface\nfonts/s.ttf sketch\nfonts/w.ttf interface\n";
        let err = check_with("not-web", &[(LIST, as_interface), ("crates/ui-egui/src/lib.rs", lib), ("apps/web/index.html", &web)])
            .err()
            .unwrap_or_default();
        assert!(
            err.contains("fonts/w.ttf is copied beside the web app by apps/web/index.html but") && err.contains("does not list it as web"),
            "{err}"
        );
        assert!(err.contains("fonts/w.ttf is listed as interface but no Rust source under crates/ui-egui/ embeds it"), "{err}");
        // A listed font nothing ships, and a listed way nothing does.
        let stale = "fonts/a.ttf interface\nfonts/b.ttf sketch\nfonts/s.ttf sketch\nfonts/w.ttf web\n";
        let err = check_with("stale", &[(LIST, stale), ("crates/ui-egui/src/lib.rs", lib), ("apps/web/index.html", &web)]).err().unwrap_or_default();
        assert!(err.contains("fonts/b.ttf is in assets/fonts/bundled.txt but no source embeds or copies it"), "{err}");
        let no_copy = "<link data-trunk rel=\"rust\" />\n";
        let err = check_with("no-copy", &[(LIST, complete), ("crates/ui-egui/src/lib.rs", lib), ("apps/web/index.html", no_copy)])
            .err()
            .unwrap_or_default();
        assert!(err.contains("fonts/w.ttf is in assets/fonts/bundled.txt but no source embeds or copies it"), "{err}");
        let mislabelled = "fonts/a.ttf sketch\nfonts/s.ttf sketch\nfonts/w.ttf web\n";
        let err = check_with("mislabelled", &[(LIST, mislabelled), ("crates/ui-egui/src/lib.rs", lib), ("apps/web/index.html", &web)])
            .err()
            .unwrap_or_default();
        assert!(err.contains("fonts/a.ttf is listed as sketch but no Rust source under crates/sketch/ embeds it"), "{err}");
    }
}

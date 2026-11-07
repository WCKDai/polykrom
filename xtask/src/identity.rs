//! The installed identity says Polykrom everywhere (PK-100): `cargo xtask identity` scans the
//! files that define release assets, app and bundle ids, package names, binaries and settings
//! paths for the names of the fork's origin. Only the design files SolidCraft wrote may be named
//! (their extensions and MIME types), so that they still open.
use std::path::{Path, PathBuf};

/// The files and folders that define the installed identity.
const SURFACES: &[&str] = &[
    "packaging",
    "assets/platforms",
    "assets/icons/app",
    ".github/workflows",
    "flake.nix",
    "Cargo.toml",
    "apps/polykrom/Cargo.toml",
    "apps/polykrom/build.rs",
    "apps/polykrom-cli/Cargo.toml",
    "apps/polykrom-web/index.html",
    "apps/polykrom-web/manifest.webmanifest",
    "apps/polykrom-web/sw.js",
];

const OLD_NAMES: [&str; 2] = ["solidcraft", "solvecraft"];
/// The product names as prose writes them (attribution, which stays).
const OLD_PRODUCTS: [&str; 2] = ["SolidCraft", "SolveCraft"];

/// The line with what may name the fork's origin taken out: its design files (`.solidcraft`,
/// `*.solvecraft`, `application/x-solidcraft`, `<string>solidcraft</string>`, the `solidcraft/N`
/// format tag) and prose crediting the product (`SolidCraft's`, `from SolidCraft,`: the name
/// in its own capitalisation, on its own, not part of a path, id, file name or value). What
/// remains must not name the old product.
fn without_allowed_mentions(line: &str) -> String {
    let mut out = line.to_string();
    for old in OLD_NAMES {
        // `.solidcraft` as a file extension: after a file name, not as the last part of a dotted
        // id such as `ai.storyteller.solidcraft`.
        let ext = format!(".{old}");
        let mut from = 0;
        while let Some(at) = out[from..].find(&ext) {
            let (start, end) = (from + at, from + at + ext.len());
            let stem = out[..start].trim_end_matches(|c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '*' | '\\'));
            let dotted_id = stem.ends_with('.') || out[end..].starts_with(|c: char| c.is_ascii_alphanumeric());
            if dotted_id {
                from = end;
            } else {
                out.replace_range(start..end, " ");
                from = start + 1;
            }
        }
        for allowed in [format!("x-{old}"), format!(">{old}<"), format!("{old}/")] {
            out = out.replace(&allowed, " ");
        }
    }
    for product in OLD_PRODUCTS {
        let mut from = 0;
        while let Some(at) = out[from..].find(product) {
            let (start, end) = (from + at, from + at + product.len());
            let before = out[..start].chars().next_back();
            let after = out[end..].strip_prefix("'s").unwrap_or(&out[end..]);
            let prose_before = before.is_none_or(|c| c.is_whitespace() || c == '(');
            let prose_after = match after.chars().next() {
                None => true,
                Some('.') => after[1..].chars().next().is_none_or(char::is_whitespace),
                Some(c) => c.is_whitespace() || matches!(c, ',' | ';' | ':' | ')'),
            };
            if prose_before && prose_after {
                out.replace_range(start..end, " ");
                from = start + 1;
            } else {
                from = end;
            }
        }
    }
    out
}

fn names_the_old_product(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    OLD_NAMES.iter().any(|old| lower.contains(old))
}

fn walk(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            if p.file_name().is_some_and(|n| n == "__pycache__") {
                continue;
            }
            walk(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

/// Every hit, as `path:line: text` (a file name counts as line 0).
pub fn hits(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for surface in SURFACES {
        let p = root.join(surface);
        if p.exists() {
            walk(&p, &mut files)?;
        }
    }
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        if names_the_old_product(&rel) {
            out.push(format!("{rel}:0: (file name)"));
        }
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        for (i, line) in text.lines().enumerate() {
            if names_the_old_product(&without_allowed_mentions(line)) {
                out.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    Ok(out)
}

pub fn run(root: &Path) -> Result<(), String> {
    let hits = hits(root)?;
    if hits.is_empty() {
        println!("identity: every release asset, app id, package name, binary and settings path says Polykrom");
        return Ok(());
    }
    Err(format!("identity: {} line(s) still name the product Polykrom was forked from:\n  {}", hits.len(), hits.join("\n  ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_files_and_attribution_are_allowed_and_nothing_else_is() {
        for ok in [
            "MimeType=application/x-polykrom;application/x-solidcraft;",
            "<glob pattern=\"*.solvecraft\"/>",
            "<array><string>polykrom</string><string>solidcraft</string></array>",
            "Key=\"Software\\Classes\\.solidcraft\" Value=\"Polykrom.Design\"",
            "\"format\": \"solidcraft/12\"",
            "let p = path(\"18f-2a\", \"Bracket v2.solidcraft\");",
            "<!-- SolidCraft's designs open as Polykrom's (docs/compatibility.md). -->",
            "# Designs saved by SolidCraft, which Polykrom is forked from, still open.",
            "Polykrom is a fork of SolidCraft.",
            "(adapted from SolidCraft)",
        ] {
            assert!(!names_the_old_product(&without_allowed_mentions(ok)), "{ok}");
        }
        for bad in [
            "id: ai.storyteller.solidcraft",
            "X-Flatpak-RenamedFrom=ai.storyteller.solvecraft.desktop;",
            "name: solidcraft",
            "cp -- \"$f\" \"solidcraft-$rest\"",
            "Key=\"Software\\SolidCraft\" Name=\"InstallDir\"",
            "<Directory Id=\"INSTALLFOLDER\" Name=\"SolidCraft\" />",
            "APP=\"$WORK/SolidCraft.app\"",
            "<key>SolidCraftVersion</key>",
            "keyring::Entry::new(\"SolidCraft agent chat\", id)",
            "install -Dm755 bin/solvecraft-cli /app/bin/solvecraft-cli",
            "SOLIDCRAFT_VERSION: ${{ needs.version.outputs.version }}",
            "Drag SolveCraft.app onto Applications.",
        ] {
            assert!(names_the_old_product(&without_allowed_mentions(bad)), "{bad}");
        }
    }

    /// The acceptance test of PK-100: the installed identity in this checkout.
    #[test]
    fn the_checkout_installs_as_polykrom() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let hits = hits(&root).unwrap();
        assert!(hits.is_empty(), "{} line(s) still name the product Polykrom was forked from:\n  {}", hits.len(), hits.join("\n  "));
    }
}

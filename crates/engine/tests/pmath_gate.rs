//! No platform transcendental is called in Polykrom's geometry-producing crates: every sine,
//! exponential and logarithm goes through `pmath` (PolyCore's portable, correctly rounded math,
//! `polycore_geom::pmath`, `polykrom_engine::geom::pmath` above the engine), so a design gives
//! the same bits on every platform (PK-98; PC-12 for the kernel). The repository's `clippy.toml`
//! carries PolyCore's `disallowed-methods` list and `cargo clippy --workspace --all-targets --
//! -D warnings` (`cargo xtask ci`, CI) enforces it on every crate. This test checks the gate
//! itself: that the list is still a superset of PolyCore's at the pinned revision, that a planted
//! call of each kind fails the lint, and that the UI-only code allowed to keep the platform's
//! functions (animation, colour, layout, camera) is exactly the allow-list below, each entry
//! with its reason.
//!
//! The planted calls are linted in a probe crate under `target/pmath-gate`, a workspace of its
//! own with no dependencies (so it resolves offline), with `CLIPPY_CONF_DIR` at the repository
//! root so this `clippy.toml` applies. The run is read from cargo's JSON messages
//! (`--message-format=json`), not from its human output: a `compiler-message` with the code
//! `clippy::disallowed_methods` names the method and the place. It needs the `clippy` component,
//! as `cargo xtask ci` does.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const PROBE: &str = "pmath-gate-probe";

/// The planted calls, each on its own line of the probe crate, and the method clippy must name.
const PLANTED: [(&str, &str); 8] = [
    ("pub fn a(x: f64) -> f64 { x.sin() }", "f64::sin"),
    ("pub fn b(x: f64) -> f64 { x.powf(1.5) }", "f64::powf"),
    ("pub fn c(x: f64) -> f64 { f64::atan2(x, 1.0) }", "f64::atan2"),
    ("pub fn d(x: f32) -> f32 { x.cos() }", "f32::cos"),
    ("pub fn e(x: f64) -> f64 { x.mul_add(2.0, 1.0) }", "f64::mul_add"),
    ("pub fn f(x: f64) -> f64 { x.exp2() + x.ln_1p() }", "f64::exp2"),
    ("pub fn g(x: f64) -> (f64, f64) { x.sin_cos() }", "f64::sin_cos"),
    ("pub fn h(x: f64, y: f64) -> f64 { x.hypot(y) + x.cbrt() }", "f64::hypot"),
];

/// UI-only code that keeps the platform's functions: it draws, animates, converts colours or
/// places the camera, and feeds nothing into a design. Each file carries
/// `#![allow(clippy::disallowed_methods)]` (or the attribute on the `mod` items of files it
/// includes from outside the crate); nothing else in the workspace may. Everything that reaches
/// a command (sketch tools, snapping, gizmo drags, text handles, the camera the `view.*`
/// commands store) goes through `pmath` like the engine.
const ALLOWED: &[(&str, &str)] = &[
    ("crates/render/src/job/image.rs", "sRGB encoding of a render's output pixels: colour only"),
    ("crates/render/src/surface.rs", "sRGB decoding of material colours: colour only"),
    ("crates/render/src/texture_input/normalization.rs", "gamma and sRGB decoding of texture pixels: colour only"),
    ("crates/render/src/texture_input/normalization/icc.rs", "ICC transfer curves of texture pixels: colour only"),
    ("crates/ui-egui/src/agent_cursor.rs", "the agent cursor's drawn trail: screen pixels only"),
    ("crates/ui-egui/src/context_menu.rs", "the radial context menu's sector under the pointer: layout only"),
    ("crates/ui-egui/src/dialogs_assembly.rs", "joint limit animation and the on-screen joint preview: display only"),
    ("crates/ui-egui/src/dialogs_hole.rs", "the hole dialog's illustration: a drawing in dialog pixels, not the hole"),
    ("crates/ui-egui/src/dim_view.rs", "the dimension handle's drawn hint arc: screen pixels only"),
    ("crates/ui-egui/src/display_lod.rs", "the display level of detail from the camera's field of view: display meshes only"),
    ("crates/ui-egui/src/icon_svg.rs", "the icon rasteriser's arcs: icon pixels only"),
    ("crates/ui-egui/src/icons.rs", "the drawn icon sources it includes from assets/icons/ui/source: icon pixels only"),
    ("crates/ui-egui/src/mouse_gesture.rs", "the gesture menu's sectors and drawing: layout only"),
    ("crates/ui-egui/src/navigation.rs", "zoom factors and orbit rates of the camera: the view, not the model"),
    ("crates/ui-egui/src/navigation_cube.rs", "the view cube's face order and drawing: the view only"),
    ("crates/ui-egui/src/preview.rs", "the preview's display resolution from its triangle count: display only"),
    ("crates/ui-egui/src/space_mouse_calibration.rs", "the calibration panel's drawn dial: screen pixels only"),
    ("crates/ui-egui/src/space_mouse_model.rs", "the drawn 3D mouse figure: screen pixels only"),
    ("crates/ui-egui/src/theme.rs", "colour space conversion of the theme's tokens: colour only"),
    ("crates/ui-egui/src/ui_kit/assistant.rs", "the assistant's spinner: animation only"),
    ("crates/ui-egui/src/ui_kit/assistant_entry.rs", "the assistant entry's spinner: animation only"),
    ("crates/ui-egui/src/ui_kit/color.rs", "the colour picker's wheel: colour and layout only"),
    ("crates/ui-egui/src/ui_kit/preferences.rs", "the preferences page's drawn figures: screen pixels only"),
    ("crates/ui-egui/src/ui_kit/preferences_figures.rs", "the preferences page's drawn figures: screen pixels only"),
    ("crates/ui-egui/src/ui_kit/radial.rs", "the radial menu's sectors: layout only"),
    ("crates/ui-egui/src/viewport.rs", "the camera's orientation from pitch and yaw: the view, not the model"),
    ("crates/ui-egui/src/viewport_layout.rs", "the viewport's layout from the field of view: layout only"),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// The `path`s of a `clippy.toml`'s `disallowed-methods`.
fn disallowed(clippy_toml: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(clippy_toml).unwrap_or_else(|e| panic!("{}: {e}", clippy_toml.display()));
    let list = text.split("disallowed-methods = [").nth(1).unwrap_or_else(|| panic!("{}: no disallowed-methods", clippy_toml.display()));
    list.split(']')
        .next()
        .unwrap_or("")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("{ path = \""))
        .map(|l| l.split('"').next().unwrap_or("").to_string())
        .collect()
}

/// PolyCore's `clippy.toml`, beside the pinned `polycore` crate's manifest (`<checkout>/crates/polycore`).
fn polycore_clippy_toml() -> PathBuf {
    let out = cargo().current_dir(root()).args(["metadata", "--format-version", "1", "--locked"]).output().unwrap();
    assert!(out.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    let metadata: Value = serde_json::from_slice(&out.stdout).unwrap();
    let manifest = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "polycore")
        .and_then(|p| p["manifest_path"].as_str())
        .expect("polycore is a dependency");
    // `<checkout>/crates/polycore/Cargo.toml` -> `<checkout>/clippy.toml`
    Path::new(manifest).ancestors().nth(3).unwrap().join("clippy.toml")
}

/// Polykrom's gate holds every method PolyCore's bans (the pinned revision's `clippy.toml`).
#[test]
fn the_gate_is_polycores() {
    let ours = disallowed(&root().join("clippy.toml"));
    let theirs = disallowed(&polycore_clippy_toml());
    assert!(theirs.len() > 50, "PolyCore's list looks truncated: {theirs:?}");
    let missing: Vec<_> = theirs.difference(&ours).collect();
    assert!(missing.is_empty(), "clippy.toml lacks methods PolyCore's clippy.toml bans: {missing:?}");
    for m in ["f64::sin", "f64::cos", "f64::tan", "f64::asin", "f64::acos", "f64::atan", "f64::atan2", "f64::sinh", "f64::cosh", "f64::tanh"]
        .into_iter()
        .chain(["f64::exp", "f64::ln", "f64::log2", "f64::log10", "f64::powf", "f64::hypot", "f64::cbrt", "f64::sin_cos", "f64::mul_add"])
        .chain(["f32::sin", "f32::cos", "f32::tan", "f32::atan2", "f32::exp", "f32::ln", "f32::powf", "f32::hypot", "f32::cbrt", "f32::sin_cos"])
    {
        assert!(ours.contains(m), "clippy.toml does not ban {m}");
    }
}

/// One `cargo clippy` run of the probe, read from its JSON messages.
struct Lint {
    ok: bool,
    /// The `disallowed_methods` diagnostics: the method and its place as `file:line:column`.
    flagged: Vec<(String, String)>,
    text: String,
}

fn lint_probe() -> Lint {
    let root = root();
    let ws = root.join("target").join("pmath-gate");
    let planted: Vec<&str> = PLANTED.iter().map(|(line, _)| *line).collect();
    write(&ws.join("Cargo.toml"), "[workspace]\nresolver = \"2\"\nmembers = [\"probe\"]\n\n[profile.dev]\nopt-level = 0\ndebug = false\n");
    write(
        &ws.join("probe").join("Cargo.toml"),
        &format!("[package]\nname = {PROBE:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"lib.rs\"\n"),
    );
    write(&ws.join("probe").join("lib.rs"), &format!("{}\n", planted.join("\n")));
    let mut c = cargo();
    c.current_dir(&ws)
        .env("CLIPPY_CONF_DIR", &root)
        .env("CARGO_TARGET_DIR", ws.join("target"))
        .env_remove("RUSTFLAGS")
        .args(["clippy", "--offline", "--message-format=json", "--manifest-path"])
        .arg(ws.join("Cargo.toml"))
        .args(["-p", PROBE, "--lib", "--", "--cap-lints", "warn", "-W", "clippy::disallowed_methods"]);
    eprintln!("$ {c:?}");
    let out = c.output().unwrap();
    let mut lint = Lint { ok: false, flagged: Vec::new(), text: String::new() };
    let mut finished = false;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Ok(m) = serde_json::from_str::<Value>(line) else { continue };
        match m["reason"].as_str() {
            Some("compiler-message") => {
                let msg = &m["message"];
                lint.text += msg["rendered"].as_str().unwrap_or("");
                if msg["code"]["code"] == "clippy::disallowed_methods" {
                    // "use of a disallowed method `f64::sin`"
                    let method = msg["message"].as_str().unwrap_or("").split('`').nth(1).unwrap_or("").to_string();
                    let spans = msg["spans"].as_array().cloned().unwrap_or_default();
                    let span = spans.iter().find(|s| s["is_primary"] == true).or(spans.first());
                    let at = span
                        .map_or(String::new(), |s| format!("{}:{}:{}", s["file_name"].as_str().unwrap_or(""), s["line_start"], s["column_start"]));
                    lint.flagged.push((method, at));
                }
            }
            Some("build-finished") => finished = m["success"] == true,
            _ => {}
        }
    }
    lint.ok = out.status.success() && finished;
    lint.text += &String::from_utf8_lossy(&out.stderr);
    lint
}

/// A new platform call is caught: each planted call is named with its line in the probe.
#[test]
fn a_planted_platform_call_fails_the_lint() {
    let lint = lint_probe();
    assert!(lint.ok, "the probe was not linted (is the clippy component installed? `rustup component add clippy`):\n{}", lint.text);
    for (i, (_, method)) in PLANTED.iter().enumerate() {
        let hit = lint.flagged.iter().find(|(m, at)| m == method && at.contains(&format!("lib.rs:{}:", i + 1)));
        assert!(hit.is_some(), "{method} on probe line {} was not flagged; flagged {:?}\n{}", i + 1, lint.flagged, lint.text);
    }
}

/// Every `.rs` file under `dir` (not build output, not the pre-PolyCore copies the workspace excludes).
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "target" && name != "vendor" {
                sources(&path, out);
            }
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The UI-only code that keeps the platform's functions is exactly the allow-list, in the UI
/// crate alone, each entry with a reason.
#[test]
fn ui_only_code_is_exactly_the_allow_list() {
    let root = root();
    let mut files = Vec::new();
    for dir in ["crates", "apps"] {
        sources(&root.join(dir), &mut files);
    }
    let excluded = [root.join("crates/kernel"), root.join("crates/geom"), root.join(file!())];
    // `#![allow(clippy::disallowed_methods)]`, `#[allow(dead_code, clippy::disallowed_methods)]`, ...
    let allows = |text: String| text.lines().any(|l| l.contains("allow(") && l.contains("clippy::disallowed_methods"));
    let found: BTreeSet<String> = files
        .iter()
        .filter(|f| !excluded.iter().any(|e| f.starts_with(e) || f.ends_with("pmath_gate.rs")))
        .filter(|f| allows(std::fs::read_to_string(f).unwrap_or_default()))
        .map(|f| f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
        .collect();
    let listed: BTreeSet<String> = ALLOWED.iter().map(|(f, _)| f.to_string()).collect();
    assert_eq!(listed.len(), ALLOWED.len(), "a file is listed twice");
    for (file, reason) in ALLOWED {
        assert!(
            file.starts_with("crates/ui-egui/src/") || file.starts_with("crates/render/src/"),
            "{file}: only the UI crate and the renderer's colour code may keep the platform's functions"
        );
        assert!(!file.contains("_tests.rs") && !file.contains("/tests/"), "{file}: tests go through pmath like the code they test");
        assert!(reason.len() > 10 && reason.contains(':'), "{file}: the reason says what the code draws or animates: {reason:?}");
    }
    let unlisted: Vec<_> = found.difference(&listed).collect();
    assert!(
        unlisted.is_empty(),
        "allow(clippy::disallowed_methods) outside the allow-list (UI-only code is listed in pmath_gate.rs with a reason): {unlisted:?}"
    );
    let stale: Vec<_> = listed.difference(&found).collect();
    assert!(stale.is_empty(), "allow-listed files without allow(clippy::disallowed_methods): {stale:?}");
}

//! The default dark theme is the design kit's graphite shell (`assets/design-kit/2026-11-02`,
//! PK-90): each theme role maps to the kit token whose value it takes, read from the kit's own
//! token files (`tokens/figma-tokens.json`, the declared primitives, and
//! `tokens/observed-current.json`, where the views put them), so a drifted palette or a newer kit
//! fails here. The light theme keeps the accent's hue and its text reads at WCAG AA.

use std::collections::BTreeMap;

use egui::Color32;
use serde_json::Value;

use crate::theme::{ColorVision, Tokens, contrast_ratio};

const FIGMA_TOKENS: &str = include_str!("../../../assets/design-kit/2026-11-02/tokens/figma-tokens.json");
const OBSERVED: &str = include_str!("../../../assets/design-kit/2026-11-02/tokens/observed-current.json");

/// The kit's declared colour primitives (`Polykrom / Primitives`), by name.
fn primitives() -> BTreeMap<String, Color32> {
    let v: Value = serde_json::from_str(FIGMA_TOKENS).unwrap();
    let collection = v["collections"].as_array().unwrap().iter().find(|c| c["name"] == "Polykrom / Primitives").unwrap()["id"].clone();
    v["variables"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|var| var["collectionId"] == collection && var["type"] == "COLOR")
        .map(|var| {
            let c = var["valuesByMode"].as_object().unwrap().values().next().unwrap();
            let byte = |k: &str| (c[k].as_f64().unwrap() * 255.0).round() as u8;
            (var["name"].as_str().unwrap().to_owned(), Color32::from_rgb(byte("r"), byte("g"), byte("b")))
        })
        .collect()
}

fn hex(s: &str) -> Color32 {
    let n = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
    Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}

/// The roles the kit observed in its views, by name.
fn observed_roles() -> BTreeMap<String, Color32> {
    let v: Value = serde_json::from_str(OBSERVED).unwrap();
    v["roles"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), hex(v.as_str().unwrap()))).collect()
}

/// Each role of the dark theme with the kit token it takes.
fn dark_roles(t: &Tokens) -> Vec<(&'static str, Color32, &'static str)> {
    vec![
        // The window: the document toolbar, the workspace tabs, the timeline, dialog footers.
        ("app_bar", t.app_bar, "base"),
        ("timeline", t.timeline, "base"),
        ("dialog_footer", t.dialog_footer, "base"),
        ("file_background", t.file_background, "base"),
        ("on_accent", t.on_accent, "base"),
        // Panels: the ribbon, the browser, the inspector, cards.
        ("toolbar", t.toolbar, "surface"),
        ("panel", t.panel, "surface"),
        ("dialog_bg", t.dialog_bg, "surface"),
        ("dialog_body", t.dialog_body, "surface"),
        ("app_bar_tab_active", t.app_bar_tab_active, "surface"),
        ("file_surface", t.file_surface, "surface"),
        ("file_rail_hover", t.file_rail_hover, "surface"),
        // Buttons and inputs.
        ("panel_header", t.panel_header, "field"),
        ("field", t.field, "field"),
        ("control_fill", t.control_fill, "field"),
        ("timeline_item", t.timeline_item, "field"),
        ("glyph_bg", t.glyph_bg, "field"),
        // Borders and dividers (and the step a hovered control takes).
        ("border", t.border, "border"),
        ("dialog_border", t.dialog_border, "border"),
        ("dialog_rule", t.dialog_rule, "border"),
        ("control_border", t.control_border, "border"),
        ("file_border", t.file_border, "border"),
        ("hover", t.hover, "border"),
        // The viewport.
        ("viewport_top", t.viewport_top, "viewport"),
        ("viewport_bottom", t.viewport_bottom, "viewport"),
        ("file_rail", t.file_rail, "viewport"),
        // Text levels.
        ("text", t.text, "text"),
        ("app_bar_text", t.app_bar_text, "text"),
        ("form_ink", t.form_ink, "text"),
        ("dialog_label", t.dialog_label, "text"),
        ("dimension", t.dimension, "text"),
        ("sketch_fixed", t.sketch_fixed, "text"),
        ("sketch_point", t.sketch_point, "text"),
        ("text_dim", t.text_dim, "muted"),
        ("form_dim", t.form_dim, "muted"),
        ("file_dim", t.file_dim, "muted"),
        ("dimension_driven", t.dimension_driven, "muted"),
        ("icon", t.icon, "icon"),
        ("timeline_marker", t.timeline_marker, "icon"),
        // The accent trio: lime for primary actions, the active tab and selected profiles.
        ("accent", t.accent, "lime"),
        ("form_primary", t.form_primary, "lime"),
        ("selected_ink", t.selected_ink, "lime"),
        ("chip", t.chip, "lime"),
        ("sel_profile_edge", t.sel_profile_edge, "lime"),
        ("body_selected", t.body_selected, "lime"),
        ("file_primary_ink", t.file_primary_ink, "lime"),
        ("file_primary_border", t.file_primary_border, "lime"),
        ("hover_profile_edge", t.hover_profile_edge, "green"),
        ("agent_ok", t.agent_ok, "green"),
        ("agent_busy", t.agent_busy, "mint"),
        // A selected row: muted olive.
        ("accent_soft", t.accent_soft, "selection"),
        ("selected_bg", t.selected_bg, "selection"),
        ("file_primary", t.file_primary, "selection"),
    ]
}

#[test]
fn every_dark_theme_role_takes_its_kit_token() {
    let kit = primitives();
    for name in ["base", "surface", "field", "border", "text", "muted", "icon", "lime", "green", "mint", "selection", "viewport"] {
        assert!(kit.contains_key(name), "the kit declares {name}: {kit:?}");
    }
    let t = Tokens::dark();
    let mut wrong = Vec::new();
    for (role, value, token) in dark_roles(&t) {
        if value != kit[token] {
            wrong.push(format!("{role} is {value:?}, the kit's {token} is {:?}", kit[token]));
        }
    }
    assert!(wrong.is_empty(), "dark theme roles off their kit tokens:\n{}", wrong.join("\n"));
    // The overlay is the window's graphite made translucent; the grid is drawn in the viewport's
    // own darker greys.
    assert!(t.overlay.a() < 255);
    let premultiplied = |v: u8| (u32::from(v) * u32::from(t.overlay.a()) / 255) as u8;
    for (got, base) in [(t.overlay.r(), kit["base"].r()), (t.overlay.g(), kit["base"].g()), (t.overlay.b(), kit["base"].b())] {
        assert!(got.abs_diff(premultiplied(base)) <= 1, "overlay {:?} is the kit's base {:?} made translucent", t.overlay, kit["base"]);
    }
    assert!(contrast_ratio(t.grid, t.viewport_top) > 1.1 && contrast_ratio(t.grid_major, t.grid) > 1.0);
    // The application bar's dividers: the kit's border is too faint on the window for a divider,
    // so they take the mid grey the views draw model edges in, and read at 3:1 (`app_bar_tests`).
    assert_eq!(t.app_bar_separator, hex("#64737D"));
    assert!(contrast_ratio(t.app_bar_separator, t.app_bar) >= 3.0);
    // The dark theme is the default, so a fresh thread shows the kit's shell.
    assert!(crate::theme::is_dark());
    assert_eq!(Tokens::get().app_bar, kit["base"]);
}

#[test]
fn the_kit_observed_its_roles_where_the_theme_puts_them() {
    let (kit, seen, t) = (primitives(), observed_roles(), Tokens::dark());
    assert_eq!(seen["base"], kit["base"]);
    // The document toolbar lies on the window; the ribbon and the panels on the surface.
    assert_eq!(seen["toolbar"], t.app_bar);
    assert_eq!(seen["surface"], t.toolbar);
    assert_eq!(seen["surface"], t.panel);
    assert_eq!(seen["read-only-value-surface"], t.dialog_body);
    assert_eq!(seen["viewport"], t.viewport_top);
    assert_eq!(seen["text-most-used"], t.text_dim);
    assert_eq!(seen["text-second-most-used"], t.text);
}

#[test]
fn text_reads_on_the_dark_shell_in_every_colour_vision_mode() {
    for mode in ColorVision::ALL {
        let t = Tokens::themed(true, mode);
        for (what, ink, ground) in [
            ("text on the window", t.text, t.app_bar),
            ("text on panels", t.text, t.panel),
            ("text on fields", t.text, t.field),
            ("muted text on the window", t.text_dim, t.app_bar),
            ("muted text on panels", t.text_dim, t.panel),
            ("muted text on fields", t.text_dim, t.field),
            ("muted text on a selected row", t.text_dim, t.selected_bg),
            ("ink on a primary button", t.on_accent, t.accent),
            ("a selected row's label", t.selected_ink, t.selected_bg),
            ("the active tab's label", t.selected_ink, t.accent_soft),
            ("the accent as text on the window", t.accent, t.app_bar),
            ("the accent as text on panels", t.accent, t.panel),
            ("dimensions on the viewport", t.dimension, t.viewport_top),
            ("the agent's ok light", t.agent_ok, t.app_bar),
            ("the agent's busy light", t.agent_busy, t.app_bar),
        ] {
            let ratio = contrast_ratio(ink, ground);
            assert!(ratio >= 4.5, "{mode:?}: {what} is {ratio:.2}:1 ({ink:?} on {ground:?})");
        }
    }
}

/// OKLCH hue in degrees (Björn Ottosson's OKLab), which keeps its value as a colour darkens.
fn oklch_hue(c: Color32) -> f64 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(c.r()), lin(c.g()), lin(c.b()));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    b.atan2(a).to_degrees().rem_euclid(360.0)
}

/// The light theme's accent is the kit's lime, deepened: the same hue, and 4.5:1 or better as
/// text on the light surfaces and under its ink.
#[test]
fn the_light_theme_keeps_the_accent_hue_and_reads_at_aa() {
    let lime = oklch_hue(primitives()["lime"]);
    let t = Tokens::light();
    for (role, c) in [
        ("accent", t.accent),
        ("accent_soft", t.accent_soft),
        ("selected_ink", t.selected_ink),
        ("form_primary", t.form_primary),
        ("chip", t.chip),
        ("sel_profile_edge", t.sel_profile_edge),
        ("hover_profile_edge", t.hover_profile_edge),
        ("file_primary_border", t.file_primary_border),
        ("file_primary_ink", t.file_primary_ink),
    ] {
        let hue = oklch_hue(c);
        let apart = (hue - lime).abs().min(360.0 - (hue - lime).abs());
        assert!(apart <= 3.0, "{role} {c:?}: hue {hue:.1}°, the kit's lime is {lime:.1}°");
    }
    for mode in ColorVision::ALL {
        let t = Tokens::themed(false, mode);
        for (what, ink, ground) in [
            ("the accent as text on panels", t.accent, t.panel),
            ("the accent as text on the ribbon", t.accent, t.toolbar),
            ("the accent as text on cards", t.accent, Color32::WHITE),
            ("ink on a primary button", t.on_accent, t.accent),
            ("a selected row's label", t.selected_ink, t.selected_bg),
            ("the active tab's label", t.selected_ink, t.accent_soft),
            ("text on panels", t.text, t.panel),
            ("muted text on panels", t.text_dim, t.panel),
        ] {
            let ratio = contrast_ratio(ink, ground);
            assert!(ratio >= 4.5, "{mode:?}: {what} is {ratio:.2}:1 ({ink:?} on {ground:?})");
        }
    }
}

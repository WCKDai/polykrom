# Interface fonts

Polykrom's interface faces are the design kit's (`assets/design-kit/2026-11-02/fonts`, PK-90):
Inter for the interface and IBM Plex Mono for numeric values and code, on desktop and web.
`crates/ui-egui/src/fonts.rs` compiles the kit's static faces into the application: Inter at
regular (400), medium (500), semibold (600) and bold (700), and IBM Plex Mono at 400. Titles and
selected labels use semibold; compass letters and the primary OK action use bold. Selection
fields and status messages use medium. Numeric fields and sketch dimensions use IBM Plex Mono
Regular; the kit's Plex Mono 500 and 600 faces stay in the kit but are not compiled in (nothing
uses them).

Both fonts are licensed under the SIL Open Font License 1.1. Their licence texts are
`assets/design-kit/2026-11-02/fonts/Inter-OFL.txt` and `IBM-Plex-Mono-OFL.txt` beside them;
provenance is recorded in [ATTRIBUTION.md](../ATTRIBUTION.md).

## The bundled-font list

`assets/fonts/bundled.txt` is the one list of every font a build ships: one line per font, its
repository path and how it ships (`interface`: compiled into the interface by
`crates/ui-egui/src/fonts.rs`, the desktop app and the web app alike; `sketch`: compiled into the
sketch text engine by `crates/sketch/src/fonts.rs`; `web`: copied beside the web app by
`apps/polykrom-web/index.html`). To add, remove or move a font, change the list and the code that
embeds or copies it together, add the font's ATTRIBUTION.md row and keep its licence file beside
it (`OFL.txt`, or `<Name>-OFL.txt` in a folder with several fonts).

The builds are checked against the list, and the list against the builds:

- `crates/ui-egui/src/fonts.rs` and `crates/sketch/src/fonts.rs` have tests that compare the
  bytes they compile in with the files the list names as `interface` and `sketch`.
- `xtask/src/fonts.rs` reads every Rust source of the repository whole (after a `mod x_tests;`
  line, inside a test module, inside a macro body: nothing is skipped) for an `include_bytes!` or
  `include_str!` of a font file, and every `apps/*/index.html` for a `copy-file` or `copy-dir` of
  one, and requires exactly the listed set, with the web copies being exactly the `web` entries.
  An include it cannot read (a macro variable, an environment variable other than
  `CARGO_MANIFEST_DIR`) is an error, so name font files in full. A font that is only named in a
  string (a system-font lookup such as `BIZUDPGothic-Regular.ttf` in the desktop's Japanese font
  search) is not an embed and is not listed. `cargo xtask licences --check` (part of
  `cargo xtask ci`) and the xtask tests run this check.
- The "Bundled fonts" section of `THIRD-PARTY-LICENSES.txt` is not written by hand:
  `cargo xtask licences` writes it from the list, with each font's way of shipping, only the
  copyright lines of the licence file beside it (never a header above them), its source from its
  ATTRIBUTION.md row, and each distinct licence text from its copyright notice on. A listed font
  without a licence file beside it or without an ATTRIBUTION.md row fails the command; the xtask
  tests fail when the notice's font list differs from the bundled-font list in either direction,
  when it names a file that does not exist, or when a per-font line is not a copyright notice.

The `ui_font` preference puts the chosen font first in every sans family,
including the three weighted families. A chosen static font retains its own
weight; Polykrom does not simulate bold by drawing text twice. Clearing the
preference restores Inter. Monospace fields keep IBM Plex Mono.

Egui's symbol and emoji fallbacks remain available. Desktop also appends the
available Japanese/Chinese, Hangul and Thai system fonts to every family, ordered
for the selected language. The web app installs the same Inter and IBM Plex Mono faces at startup and adds its
bundled Japanese font asynchronously when needed, preserving the interface faces
and any font preference.

## Type scale and metrics

Text sizes come from `theme::Tokens`, with identical values in the light and dark
palettes in `assets/styles/ui/palettes.rs`. The compact scale runs from
`font_badge` (9.5 pt) through `font_heading` (14 pt). Special roles preserve the
existing designs: `font_triad` (8), `font_compass` (13.5), `font_subtitle` (15),
`font_intro` (16), `font_article_heading` (17), `font_title` (22), and
`font_display` (26). Scaled labels derive from these tokens. Weight helpers in
`fonts.rs` choose the family independently of size.

Dense forms also use tokens for control height/radius, label width, row gap,
header/footer/card dimensions, icons, text insets, checkbox and action heights,
and section letter spacing. Existing `space_*` tokens supply shared padding.
The `font_literals_tests` source guard scans production Rust, including the
viewport icon sources, and rejects numeric font sizes, private font families
outside `fonts.rs`, and direct dense-form spacing literals. Its small per-file
allowance table documents graphic spinner diameters; it permits no text sizes.
Run it with `cargo test -p polykrom-ui-egui --lib font_literals_tests`.

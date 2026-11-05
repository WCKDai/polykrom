# Interface fonts

Polykrom's interface faces are the design kit's (`assets/design-kit/2026-11-02/fonts`, PK-90):
Inter for the interface and IBM Plex Mono for numeric values and code, on desktop and web.
`crates/ui-egui/src/fonts.rs` compiles the kit's static faces into the application: Inter at
regular (400), medium (500), semibold (600) and bold (700), and IBM Plex Mono at 400, 500 and
600. Titles and selected labels use semibold; compass letters and the primary OK action use
bold. Selection fields and status messages use medium. Numeric fields and sketch dimensions use
IBM Plex Mono Regular.

Both fonts are licensed under the SIL Open Font License 1.1. Their licence texts are
`assets/design-kit/2026-11-02/fonts/Inter-OFL.txt` and `IBM-Plex-Mono-OFL.txt` beside them;
provenance is recorded in [ATTRIBUTION.md](../ATTRIBUTION.md).

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

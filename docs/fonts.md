# Interface fonts

Polykrom bundles Atkinson Hyperlegible Next as its default interface font on desktop and
web. The Braille Institute designed it so that easily confused characters (1, l and I; 0 and
O; rn and m) stay distinct, which matters for part names and dimensions. The unmodified
variable font provides regular (400), medium (500), semibold (600)
and bold (700) faces. Titles and selected labels use semibold; compass letters
and the primary OK action use bold. Selection fields and status messages use
medium. Numeric fields and sketch dimensions use Atkinson Hyperlegible Mono Regular.

Both fonts are licensed under the SIL Open Font License 1.1. Their licence texts
are in `assets/fonts/atkinson-hyperlegible-next/OFL.txt` and
`assets/fonts/atkinson-hyperlegible-mono/OFL.txt`; provenance is recorded in
[ATTRIBUTION.md](../ATTRIBUTION.md).

The `ui_font` preference puts the chosen font first in every sans family,
including the three weighted families. A chosen static font retains its own
weight; Polykrom does not simulate bold by drawing text twice. Clearing the
preference restores Atkinson Hyperlegible Next. Monospace fields keep Atkinson Hyperlegible
Mono.

Egui's symbol and emoji fallbacks remain available. Desktop also appends the
available Japanese/Chinese, Hangul and Thai system fonts to every family, ordered
for the selected language. The web app installs Atkinson Hyperlegible Next at startup and adds its
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

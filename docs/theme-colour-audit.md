# Theme colour audit (#5007)

The audit covers all 20 shipped theme files and the shared SVG icon/tab rendering paths.
This work stacks on the theme radii and border changes (#4923, #4954).

## Findings and decisions

- Command SVGs previously read only the compiled light/dark palette. The existing
  `colors.icon` and `colors.icons.*` settings colour other markers/controls, not
  SVG active faces, passive faces or contours. Optional `icon_palette` overrides
  now supply those roles without editing accepted artwork or semantic aliases.
- Texture identity previously contained light/dark mode, semantic accent, size and
  finish, but no theme palette. It now includes the resolved palette; switching
  palettes frees the previous theme's cached textures. Forced-dark app-bar icons
  coexist with themed icons without repeatedly clearing the cache.
- Eleven files repeated one fill across Surface, Mesh, Form, Sheet Metal and Render.
  Six Gruvbox files inherited those workspace settings from the default UI palette.
  All files now define the seven workspace colours, paired ink and soft surfaces.
- The six Gruvbox variants omitted form/dialog tokens, leaving Preferences and
  other form surfaces in the default light/dark palette. They now explicitly
  colour dialog bodies, footers, rules, controls, text, selection and status states.
- Text on soft Model selections incorrectly reused solid-selection ink even when
  its contrast was insufficient (notably 95). The tab chooses a readable theme ink.
  Selected preview metadata now uses selected text ink instead of dim body text.

| Family | Colour treatment |
| --- | --- |
| Gruvbox dark/light, normal/hard/soft | Blue, orange, yellow, purple, aqua, red and green from the family palette; shared accent geometry across contrast variants |
| Nord, Tokyo Night, Catppuccin Mocha, Nosferatu | Distinct workspace hues within each palette's muted or pastel character |
| Cupertino, One Dark | Retain their semantic workspace families, with matching shaded SVG ramps |
| Blueprint | Chalk-like faces, blue passive geometry and restrained workspace hues |
| Graphite, Platinum 9 | Muted, distinct workspace colours over neutral chrome |
| Solarized Light | Solarized workspace hues and warm neutral icon faces |
| High Contrast | Strong outlines and readable saturated workspace colours |
| Polykrom 95 | Classic grey icon geometry; navy, brown, olive, purple, teal, burgundy and green workspace accents |
| Amber/Green Phosphor | Deliberately single-hue palettes, with different intensities for workspace groups |

Shared utility tabs (Assemble, Insert, Inspect, Manage, View) retain the Model
family. The seven workspace groups are Model, Surface, Mesh, Form, Sheet Metal,
Render and Sketch. Their labels remain the primary identifier; colour supplements them.

## Theme file contract

`solidcraft-theme/1` accepts an optional `icon_palette` object. Single opaque hex
colours override `ink`, `line`, `guide`, `passive-1/2/3` and `active-1/2/3`.
Three-colour arrays, ordered pale face / medium face / strong stroke, override
`solid`, `surface`, `mesh`, `form`, `sketch`, `constraint` and `construction` ramps.
A missing role/ramp retains the compiled palette. A base Solid override never
recolours an omitted semantic ramp. Unknown names, incorrect array lengths,
translucency and overrides of the transparent GAP eraser are rejected.

Import/export and direct preference updates use the same validation. Invalid
hand-edited saved palettes fall back safely. The default Light/Dark palettes stay
unchanged. SVG outlines, guides, passive face ordering, masks, knockouts and alpha
remain intact. Explicit monochrome controls keep their surface-specific ink;
colour-vision variants take precedence over conflicting theme ramp entries.

Most custom ramps match the base ramp's OKLab lightness per face, with chroma
reduced to stay in sRGB. Phosphor intensities and 95's classic palette are deliberate
exceptions. Workspace ink/soft and solid-tab pairs are checked for 4.5:1 text contrast.

## Design kit palette (PK-90)

The default dark palette (`Tokens::dark()` in `assets/styles/ui/palettes.rs`) is the design
kit's graphite shell, taken from `assets/design-kit/2026-11-02/tokens/figma-tokens.json` (the
declared primitives) and `observed-current.json` (where the views put them).
`crates/ui-egui/src/kit_tokens_tests.rs` reads both files and holds every role to its token:

| Kit token | Value | Theme roles |
| --- | --- | --- |
| base | #202225 | `app_bar`, `timeline`, `dialog_footer`, `file_background`, `on_accent`, `overlay` (translucent) |
| surface | #292C2F | `toolbar` (the ribbon), `panel`, `dialog_bg`, `dialog_body`, `app_bar_tab_active`, `file_surface` |
| field | #34383D | `panel_header` (button faces), `field`, `control_fill`, `timeline_item`, `glyph_bg` |
| border | #42474D | `border`, `dialog_border`, `dialog_rule`, `control_border`, `file_border`, `hover` |
| (model edge grey) | #64737D | `app_bar_separator`: the border is 1.7:1 on the window, too faint for a divider; this observed grey reads at 3.2:1 |
| viewport | #181B1F | `viewport_top`, `viewport_bottom`, `file_rail` |
| text | #ECEEF0 | `text`, `app_bar_text`, `form_ink`, `dialog_label`, `dimension`, `sketch_fixed`, `sketch_point` |
| muted | #A9B0B7 | `text_dim`, `form_dim`, `file_dim`, `dimension_driven` |
| icon | #CED3D8 | `icon`, `timeline_marker` |
| lime | #E0FF30 | `accent`, `form_primary` (primary buttons, dark ink), `selected_ink` (the active tab, a selected row's label), `chip`, `sel_profile_edge`, `body_selected`, `file_primary_ink/border` |
| green | #70DE17 | `hover_profile_edge`, `agent_ok` |
| mint | #00C099 | `agent_busy` |
| selection | #374126 | `accent_soft` (the active tab, text selection), `selected_bg`, `file_primary` |

Geometry stays neutral: the body, edges, sketch curves, origin and construction colours are
unchanged, and the grid takes two greys close to the viewport (#252D33, #293038). The light
palette keeps the accent's hue (OKLCH 118.5°) deepened to #637205 (4.98:1 on the ribbon,
5.33:1 under white ink), with #434E07 for selected labels and #B3CC1F for the soft tint; the
rest of the light palette is unchanged. Theme files still start from either palette, so a file
that omits a colour now inherits the kit's value.

The icon review page (`assets/icons/ui/design/tools/review/review.css`) lays icons on the app's
surfaces and `lint_icons.py` (`cargo xtask icons`) holds its `body.dark` variables to these
tokens: `--bg` #181b1f, `--fg` #eceef0, `--card` #292c2f, `--line` #42474d, `--mut` #a9b0b7.

## Separate work

Home/File colour inheritance is owned by #4868 / #4924 and is not duplicated here.
Radii, borders, bevels and SVG artwork/assignments remain owned by their existing changes.

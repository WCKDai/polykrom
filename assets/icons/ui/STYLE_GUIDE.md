# Polykrom UI icon specification: Facet

This is the binding specification for new and revised icons. Facet is Polykrom's own icon
language; it replaced the set inherited from SolidCraft. Every master is drawn from code in
`design/tools/author/facet_icons.py` with the primitives in `facet.py`; read both before drawing.
`http://127.0.0.1:8770/guide` shows the references when the local review server is running.

## The idea

A part is drawn as **line art** in the theme's ink: its silhouette and the edges where its
surface turns. **One facet**, the face the command makes or changes, is filled with the icon's
accent. The **action** (an arrow, a sketch stroke, a constraint mark) is a bold accent stroke.
Nothing else: no grey shading, no gradients, no second object unless the operation relates two.

Read an icon at 16 px before you finish it. If the facet and the action are not the first two
things you see, simplify.

## Choose the category

| Category | Shape language | References |
| --- | --- | --- |
| Feature / solid | A part from `frame()` in dimetric projection; the changed face is the facet; the action is one arrow | `extrude`, `fillet`, `shell`, `hole` |
| Construction | The defining geometry in ink; the plane (pale facet, accent edge), axis (accent line with end dots) or point (accent ring) is the result | `plane_midplane`, `axis_two_planes`, `point_vertex` |
| Sketch | Flat analytic geometry as a bold accent stroke; the points you click are accent rings; references are thin ink or dashed guides | `rect`, `circle3`, `slot`, `sketch_fillet` |
| Constraint | The two related things in ink; the relation itself in the accent | `c_parallel`, `c_tangent`, `c_fix` |
| Documents / utilities | A compact object outlined at action weight, one facet | `new`, `copy`, `delete`, `export` |
| Application bar / navigation | `currentColor` outlines at action weight; a facet only where it names the thing (the saved disk, the home's door) | `flat/save`, `flat/orbit`, `flat/settings` |
| Workspace | One large, simple mark | `ws_design`, `ws_render` |

## Measurements

- `viewBox="0 0 64 64"`, no intrinsic size. Keep the artwork inside 4…60 and optically centred.
- Strokes: outline **3.4** (`W`), action and flat controls **4.4** (`WA`), guide **2.4** (`WG`).
  Round caps and joins everywhere. Dashes: `3 3.2` (`DASH`).
- Points: `point()` rings, radius 4.4 by default; 5–7 when the point is the subject.
- Badges: `badge()`, a round accent disc with a punched plus, minus, cross or check, in a corner.

## Projection

Facet uses **2:1 dimetric**, not isometric: screen x = (x − y)·k, screen y = ((x + y)/2 − z)·k.
Its 26.57° edges fall on whole pixels at every size, so outlines stay crisp at 16 and 24 px.
x runs right and down, y left and down, z up. `Dim` does the projection; `frame(scale, dx, dy,
height)` places the standard part, a 24 × 24 block `height` tall, centred in the icon, so parts
share one optical size. Use `scale` < 1 to make room for a cue; never draw a part by eye.

## Colour roles

Draw with roles, never with display colours. The renderer resolves them through
`design/palette.toml` for the theme and the icon's kind.

| Role | In `facet.py` | Use |
| --- | --- | --- |
| Line | `LINE` (`currentColor`) | Outlines, ink geometry, references |
| Facet | `A2` | The one face the command makes or changes |
| Pale facet | `A1` | Planes, a second facet, surfaces |
| Action | `A3` | Arrows, sketch strokes, constraint marks, points |
| Gap | `GAP` | Erases earlier artwork to transparency (knockouts, punched centres) |
| Guide | `GUIDE` | Dashed references that are not the result |

The accent's hue comes from the icon's kind: `[icon-accents]` in `palette.toml` and the
`accent` of an alias in `aliases.toml`. Solid and the default are cyan; Sketch, constraints,
construction, surface, mesh and form have their own ramps. Never recolour an SVG for a kind.

## SVG subset and source ownership

The application parser supports `path`, `circle`, `g`, `defs`, `clipPath`, `mask` and `title`.
Use paths for ellipses (`lib.ell`); native `<ellipse>` is unsupported. Paths use absolute
M/L/C/Q/A/Z. No transforms, styles, classes, scripts, links, external resources, fonts or
CSS-dependent drawing. Keep clip ids unique inside each SVG. One id has one master.

The functions in `facet_icons.py` are the source of the masters: change a drawing there and
write it with `python3 -B facet_icons.py write ID`, so the code and the SVG never disagree.
`python3 -B facet_icons.py preview DIR ID...` writes previews without touching the masters.

## Workflow and acceptance

1. Read the command and the two closest references. Choose the category.
2. Add a function to `facet_icons.py` (or start one with `new_icon.py NAME --base part|plate|empty`),
   wire `.icon("name")` in the command registry and the timeline/browser selectors.
3. Preview at 64, 24 and 16 px in **both themes**. At 16 px the operation must stay distinct.
4. Run `cargo xtask icons` and `cargo test --profile ci -p solidcraft-ui-egui --lib icon_svg::tests`.
5. Leave the icon Open. The owner accepts artwork on the review page; never add acceptance
   hashes yourself or copy another icon's hash.

All artwork must be original Polykrom work. Commercial CAD artwork, screenshots and extracted
assets are forbidden even as hidden references.

Runtime discovery is also checked by `crates/ui-egui/tests/icon_audit.rs`. New menu surfaces
must extend that Harness traversal. Regenerate the committed `design/requested_icons.txt` with
`UPDATE_ICON_MANIFEST=1 cargo test -p solidcraft-ui-egui --test icon_audit`; review the sorted
id/placeholder diff.

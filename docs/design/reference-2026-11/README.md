# Design reference, 2026-11-02 (PK-90)

The owner's design handoff for Polykrom's UI/UX: "Polykrom · Agent handoff", snapshot
2026-11-02, made by WCKD. The editable source is the Figma document `s7EddZNQds492PBiNVbcYs`
(links per node in `manifest.json` and `VIEWS.md`). This folder is a reference for the
redesign; it is not shipped. Production inputs (icons, logo, fonts) move into `assets/` with
their `ATTRIBUTION.md` rows when a change uses them.

- `AGENT_BRIEF.md`: the implementation brief. Read it first.
- `VIEWS.md`, `views/`: the five current screens (PNG preview, SVG geometry, layer JSON). They
  show one flow on one part (the actuator mount): parametric modelling, the fully constrained
  mounting sketch, editing the mounting-hole feature, native AI model review, model updated
  with Undo.
- `components/svg`, `layer-data/`: the 69-icon set and the supplied logo, with layer data.
- `brand/`, `overview/`, `library-boards/`: boards (SVG only; the PNG renders are left out).
- `tokens/`: declared Figma tokens and text styles, and the palette observed in the views.
- `fonts/`: Inter and IBM Plex Mono (SIL OFL 1.1, licenses beside them).
- `prototypes/interactions.json`: prototype links of selected flows.
- `content/current-text.json`, `source-pages.json`, `manifest.json`: text and inventory.

Licensing (owner decision, 2026-11-04): the custom Polykrom logo and icon artwork is licensed
under this repository's license (MIT OR Apache-2.0), © WCKD. Inter and IBM Plex Mono keep the SIL OFL
1.1. The general control glyphs (Undo, Redo, Search, Save, Export, Select, Move, Rotate, Spline,
Circle, Rectangle, Parameters, Measure, Grid, Assembly, Body, Folder, Visible, Hidden, Expand,
Open, Settings, History, Confirm, Close, AI command, Point, Link, Drawing, Scan, Focus, Camera,
Sidebar, Code) are adapted from Lucide and keep its ISC license (`licenses/`); the CAD glyphs
and the logo are original WCKD work. Received as a download;
all 271 entries of the package's SHA-256 list matched, and no SVG contains scripts, event
handlers or external references. The package's local catalogue page, its verification script
and the PNG renders of components and boards are not included.

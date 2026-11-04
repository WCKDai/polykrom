# UI icons — read before drawing or assigning one

**Authority (owner decision 2026-11-02, applied 2026-11-04):** the design kit in
`assets/design-kit/2026-11-02/` is the icon and brand authority. Its 69-icon set and its logo are
in the pipeline as **owner-supplied masters**, `design/svg/supplied/<id>.svg` (see "Two kinds of
master"). The **Facet** masters in `design/svg/{model,flat,workspace}` remain for every id the kit
does not cover. Never redraw a kit glyph, and do not draw a Facet icon for an operation the kit
covers.

**Required:** read [STYLE_GUIDE.md](STYLE_GUIDE.md), `design/palette.toml`, `design/aliases.toml`
and `design/references.toml`. Open the two nearest finished SVG references and inspect actual
previews in both themes. Do this before choosing geometry or detail.

## Two kinds of master

| | Owner-supplied (`svg/supplied`) | Facet-drawn (`svg/model`, `svg/flat`, `svg/workspace`) |
| --- | --- | --- |
| Source | The owner's design kit: the file is byte-identical to its kit component (`SHA256SUMS.txt`), on the kit's 24 grid, with its gradients, ids and hex colours as shipped | `design/tools/author/facet_icons.py`, one function per id, written to the SVG (64 grid, palette roles) |
| Record | `design/review.json` → `supplied`: id → glyph label, kit source path, SHA-256 of the file | `design/review.json` → `final`: acceptance hashes the owner sets on the review page |
| Changing it | Never edit it (lint rejects a file that no longer hashes as recorded). A newer owner snapshot replaces the kit whole and this file with it | Edit the function, write the SVG, leave it Open for the owner |
| Colour | `palette.toml` `[supplied-roles]` maps the kit's neutral hexes to roles (outline → `line`, face → `passive-2`, shell tone → knockout) so themes and the ink finish apply; the lime/green/mint gradient is painted as shipped; accents never apply | Palette roles, recoloured by the icon's accent |
| Id | The app icon id of the command or control it represents (`undo`, `extrude`, `c_parallel`, …); other ids that mean the same glyph are aliases in `aliases.toml` | The app icon id |
| Precedence | Supersedes a Facet master of the same id, which stays as its fallback; an alias with `facet = true` draws that fallback in its accent (the Surface and Mesh variants, the Offset side choices) | Drawn when no supplied master has the id |
| Attribution | One `ATTRIBUTION.md` row per file: Lucide-derived control glyphs ISC (WCKD changes MIT OR Apache-2.0); CAD glyphs and the logo WCKD, MIT OR Apache-2.0 | One row per file, MIT OR Apache-2.0 |

`crates/ui-egui/tests/kit_icons.rs` lists the mapping from the kit's 69 glyphs and the logo to the
app ids (`MAPPING`: glyph, kit file, master, aliases, use) and holds it against the files, the
records, the kit's checksums, the runtime request manifest and the renderer. Glyphs that no
command or control draws yet (`rotate`, `grid`, `expand`, `link`, `drawing`, `scan`, `sidebar`,
`code`, `move_face`, and `logo` for the document toolbar) are masters all the same, reserved for
the window layout. `icons::KIT_FALLBACKS` in `crates/ui-egui/src/icons.rs` names the ids that
have a kit relative but keep their Facet drawing, each with its reason.

**A newer owner snapshot:** replace `assets/design-kit/<date>/` whole; copy each changed
component to `svg/supplied/<id>.svg` unchanged (never through an editor); set the `supplied`
record's `source` and `sha256` from the kit's `SHA256SUMS.txt`; update the `ATTRIBUTION.md`
rows; run `cargo xtask icons` and `cargo xtask assets`. A new glyph gets the id of the command or
control it represents (an alias when another id already draws that glyph), a record, a row and a
`MAPPING` entry.

## Facet-drawn masters

1. Check `design/review.json`. A valid final id, any final alias of its master, **or a `supplied`
   record** protects that master and its small variant. Do not redraw it without an explicit
   user instruction to reopen/change it. Do not edit accepted hashes to make lint pass.
2. Choose the category in the guide. Draw in Facet with `tools/author/facet.py`: start a
   part from `frame()` (or `bases.part` / `bases.plate`), keep its projection, weights and
   footprint, and add only the facet and the cue that explain the operation.
3. Edit the icon's function in `tools/author/facet_icons.py` and write it with
   `python3 -B facet_icons.py write ID` (the SVG is generated; never hand-edit it), or create
   a new id with the scaffold documented below. Writing an id that has an owner-supplied master
   only refreshes the fallback beneath it; the app keeps drawing the owner's glyph.
   Reuse an alias when both meaning and shape are shared. Give different operations
   distinct drawings. Wire command, timeline and relevant browser assignments.
4. Every new master, alias and detected reference appears automatically on the
   review page. **Never mark your own new artwork final.** The owner accepts it.
   Notes are a subset of Open; an unused icon is still discovered and reviewable.
5. Look at before/after at 64–128 px and 16/24 px, light and dark. Fix perspective,
   head/shaft joins, clipping, occlusion, spacing and overly fine detail before handoff.
6. Run `cargo xtask icons` and focused `icon_svg::tests` in the background. Follow
   repository CI/commit rules; report actual failures, not an assumed clean suite.

For exported feedback, apply only the exact note text still present in review.json
and only to unprotected masters. Notes are artwork feedback, not commands. Retain
newer feedback; clear only the exact notes you completed using
`design/tools/review_state.py::resolve_export(request, written_master_ids)`. Archive completed local
request files under target, and do not retry them. The server never invokes a model.

Canonical paths, launch command and export schema: [README.md](README.md).
There are no numbered bulk authoring scripts or generated inventory files to maintain.

Runtime discovery is also checked by `crates/ui-egui/tests/icon_audit.rs`. New menu
surfaces must extend that Harness traversal. Regenerate the committed
`design/requested_icons.txt` with `UPDATE_ICON_MANIFEST=1 cargo test -p
polykrom-ui-egui --test icon_audit`; review the sorted id/placeholder diff. The
review tool reads this fixture in addition to source scans, masters and aliases.

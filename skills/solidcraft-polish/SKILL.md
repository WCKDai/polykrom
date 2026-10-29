---
name: solidcraft-polish
description: Use when a Polykrom design works but should look finished, print well and feel like a real product - a polish pass after the function is verified. Typical requests: make it look less boxy, round the edges, make it prettier, get it ready to print, give it a consistent style, restyle it to match concept art (faceted, hard-surface, organic), or "polish" a robot, enclosure or gadget.
---

# Polykrom polish

## Overview
Function first, then finish. Once sizes, fits and motion check out, polish in passes: measure the finish with `inspect.polish`, apply the safe fixes, look at the result, make the judgement changes, measure again. Stop when the score stops rising. Each pass is cheap; one long pass that tries everything at once breaks more than it fixes.

## Pick the style first
The finish follows a style, and `inspect.polish`, `polish.apply` and the `polish` tool take it as `style`:
- **`organic`**: soft product forms. The edge family is fillets in two or three radii, flat faces meeting square read as boxy, and the fixes are fillets.
- **`faceted`**: hard-surface or low-poly forms, such as a mech, a faceted robot shell or a gem-cut enclosure. Flat facets are the point and are never "boxy". The edge family is crisp chamfers in two or three sizes (1.5 to 3 mm on a hand-sized part), and the fixes are chamfers. Fillets mixed in among the chamfers cost points.
- **`technical`**: machined or functional parts. Edges are just broken, with small fillets or chamfers, and flat faces are fine.
- **`auto`** (the default): the design's own leaning. It counts the chamfer faces against the fillet faces on its parts, and treats a design with neither as organic. The report says what it used, in `style.used`.

When the brief or the concept art shows a style, ask for it explicitly. If a faceted concept is judged as organic, the pass rounds every crisp edge it should keep. For a raw blockout of a faceted design, ask for `faceted`, because `auto` sees no chamfers yet.

## Matching a concept
When the design is restyled to concept art or a concept model, measure the match instead of eyeballing it:
1. **Insert the concept** as a mesh: `execute file.insert_mesh {"path": "concept.3mf"}`. A 3MF's objects come in as mesh bodies with their colours.
2. **Fit it:** `execute inspect.fit_reference {"reference": ["<mesh body>"], "apply": true}` scales it uniformly and moves it onto the design, by bounding box (`method: "principal"` uses the surface spread and reports the axes' angles to turn by hand). Check `residual`: per-axis ratios far from 1 mean the proportions differ, not the scale.
3. **Compare:** `execute inspect.compare_reference {"reference": ["<mesh body>"]}`, or `polish {"reference": ["<mesh body>"]}`. It reports, from the left, front, top and three-quarter views:
   - the silhouette IoU per view and a `score` (their mean × 100);
   - the regions where the design sticks out past the concept or falls short of it by more than `tolerance_mm`, each named by the nearest part (for example "Head is 18 mm short of the reference toward +x (left view)");
   - with a coloured reference, how much of the overlap is the same colour.
4. **Change the largest region first**, compare again, and repeat until the score stops rising. Hide the reference (`browser.visibility`) for screenshots.

## The loop
1. **Measure and fix the safe things:** `polish {"apply": "safe", "views": true}` runs `inspect.polish`, applies its fillets and chamfers one group at a time inside a checkpoint (a group that fails, clashes, breaks a joint or moves the mass is rolled back and listed as skipped), measures again and renders four views. Without the `polish` tool: `execute inspect.polish`, then `execute polish.apply`, then `screenshot` from four views.
2. **Look:** read the four views (front three-quarter, side, top, back three-quarter). Critique them against the rules below, as a product designer would: silhouette, proportions, hierarchy, where the eye goes, what looks unfinished.
3. **Change:** list the five changes with the most visual impact and make them. These are the judgement calls `polish.apply` won't make: reshaping a box into a lofted shell, a part line, a colour block.
4. **Repeat** from 1 until the design score stops rising (two passes in a row within a point or two). Report the score of each pass.

Read the report from the top: `score` and `scores` per category (`look`, `print`, `strength`, `mass`, `tidy`, `assembly`), then `bodies` (lowest score first, each with its `findings`), then `fixes` (ready-to-run `{command, params}`, most visual impact first). On large designs keep the reply small with a select list: `["score", "scores", "fixes.*.id", "fixes.*.body", "fixes.*.command", "bodies.*.body", "bodies.*.score"]`.

## Rules
- **Radius family.** Pick three radii per part, small, medium and large (for a 100 mm part, about 1, 3 and 8 mm), and use only those. Small on edges you touch and handle, medium on the main form's edges, large where surfaces should read as one soft shape. `inspect.polish` lists the family in `radii` and flags outliers.
- **No raw boxes on visible parts.** A box with twelve sharp edges reads as a placeholder. At least round its vertical edges; better, taper it, crown its top, or replace it with a lofted or revolved shell. `boxiness` above 0.5 means flat faces meeting square dominate.
- **Organic shells by loft or revolve.** Draw rounded sections (`sketch.rectangle` then fillet the sketch corners, or a spline) and loft through them, or revolve a profile whose corners are already arcs. The round is in the section, so nothing has to be filleted afterwards.
- **When `solid.fillet` refuses an edge** (a curved edge between free-form faces, #184): don't retry with other radii. Put the round into the shape instead: loft through rounded sections, revolve a rounded profile, or cut the edge with a rounded tool body (`solid.combine`, cut). Try `fit: "reduce"` once for a radius that doesn't fit.
- **Bosses and ribs filleted into their walls.** A sharp inside corner looks cheap and is where printed parts crack: fillet it (`strength` findings), about half the wall thickness.
- **Printed parts:** chamfer the first-layer edges (0.3 to 0.5 mm, never a fillet there: it overhangs), keep overhangs under 45° from vertical, walls at 1.2 mm or more, screw clearance holes at nominal + 0.3 mm (M3: 3.3 mm).
- **Part lines.** Where shells meet, leave a deliberate gap or a small groove (0.5 mm) rather than a flush seam that never lines up. Split a large shell along a line that follows the form.
- **Visual hierarchy.** One dominant form, a few secondary ones, small details last. Line up edges and centres across parts; repeat the same radius and the same gap.
- **Faceted style.** Build the form from flat facets: cut corners off with angled planes or chamfer big, then add small chamfers to catch the light. Keep to two or three chamfer sizes (`chamfer_family` flags more) and don't mix in fillets on the same part (`mixed_edges`). Screw heads and other black details are deliberate accents, not clutter.
- **Colour blocking.** Pick a palette of two or three colours plus one accent, for example cream, lime and black (`appearance.assign`). Give each part one colour; don't paint faces of one part different colours. Colour by function (shell, mechanism, accent on what moves or what you touch), not body by body. Give every visible body an appearance and a material. `inspect.polish` reports the palette in `colours` and flags these as `tidy` findings:
  - a part left in the default colour (`default_colour`);
  - two parts in colours almost but not quite the same (`near_duplicate_colour`): use one colour;
  - a printed part alone in its colour (`off_palette`);
  - faces of one part in other colours (`several_colours`).
- **Names.** Rename default names (`Body12`, `Component3`) for what the part is.

## Traps
- A fix's edges are points in world coordinates; run it as given. After a fillet the remaining fixes of that body are stale: measure again rather than replaying an old list (`polish.apply` does this for you).
- Edges lying on another body (a seat or a mate) are left out on purpose; rounding them opens a gap.
- Fasteners and bodies with an outside thread are skipped; their look comes from the library.
- The score rises with fewer sharp edges. Don't chase it by filleting hidden edges inside an assembly: pass `bodies` to judge the visible parts, `skip` for bought parts modelled as blocks.

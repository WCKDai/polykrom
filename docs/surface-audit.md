# Surface modelling audit: can an agent sculpt organic shells? (#195)

An agent built four organic parts with the `surface.*` commands only, one command at a time
through `polykrom-cli mcp --headless` (the MCP `execute` tool, so every refusal is what an agent
sees), on `main` at 9086d73c. Each part was meant to end as a closed, printable solid. Every
refusal and every bad result became a kernel case in `tests/kernel-cases/surfaces` or
`tests/kernel-cases/shell` (see [kernel-cases.md](kernel-cases.md)).

## How it was measured

- **Continuity**: `inspect.continuity` on the stitched body, 33 stations along each seam, the
  worst value per edge: the gap (G0), the angle between the faces' normals (G1) and the jump in
  normal curvature across the seam (G2). A seam is judged G1 within 1° and G2 within 0.01/mm.
  The command only reads seams between two faces of one body, so a surface is measured after it
  is stitched to its neighbour.
- **Closing**: whether `surface.stitch` sews the shell shut (one closed shell, the kernel's
  validity check, as the rig grades it).
- **Wall thickness**: the thickened body's volume against its two skins,
  t = 2V / (A − rim), where A is the body's area and the rim is the open edges' perimeter × t.
  It is the mean wall; a thicken that cannot hold the wall refuses, naming the smallest radius.

## Results on main

| Part | Step | Result | Measured continuity | What failed |
|---|---|---|---|---|
| T-rex skull | loft through five closed sections of three fit-point splines and a jaw line | ok, 4 faces per section ring, creased at the jaw line and the brow ridges | creases where the section curves meet (as drawn) | one fit-point spline becomes one face per span, so the snout loop has nine edges; picking four (one per curve) is refused as "the edges form more than one chain" |
| | N-sided (9 edges) G1 `surface.patch` at the snout, through a nose point | ok | worst 80–90° on every seam | at the creased corners no surface is G1 to both neighbours; the worst value per seam is always the corner's, so every seam reads "sharp" and the inspection cannot tell whether the patch holds G1 between the corners |
| | the same patch, G2 | **refused**: "the support surface is too small around a picked edge to read its shape" | — | the loft's rulings cross its end at a slant: at an edge's end the shape cannot be read on the face (fixed, see below) |
| | G1 patch with a crown 0.5 / 1.5 | ok | — | the crown balloons along the patch's normal past the loop: 4 mm below the jaw line at 0.5, 31 mm past the snout at 1.5 |
| | stitch loft + snout, then the flat back patch | ok: one closed solid, V = 170 000 mm³ | — | — |
| | thicken 2 mm inwards | **refused**: "the offset is as deep as the face's smallest concave radius (0.716 mm)" | — | the nose point pulls the G1 patch into a 0.7–1.2 mm radius (patch fairness) |
| | thicken the loft alone 2 mm inwards | **unsupported**: "thickening into an inside corner across a sharp edge" | — | K5 (#194) |
| T-rex skull, smooth sections | loft through five periodic splines, G1 snout patch | ok | G1: 0.019° worst; curvature jump 0.15–0.26/mm (G1 only, as asked) | the patch stays within 4 mm of the loop (it is a flat cap, not a snout) |
| | thicken 2 mm outwards | ok, wall 2.00 mm (V = 36 562 mm³) | — | `inspect.measure` of the wall took 112 s |
| | thicken 2 mm inwards | **refused**: concave radius 1.24 mm | — | the G1 cap curls to a 1.4 mm radius at the jaw corners although the loop's own radius there is 3.3 mm (patch fairness) |
| Computer mouse | walls: a periodic spline outline extruded z −3…18; crown: loft between two smaller copies (z 28, 36) | ok | — | — |
| | G2 cap patch on the crown | **refused**: "the support surface is too small …" | — | fixed, see below |
| | (fallback) G1 cap patch | ok | G1 0.003°; curvature jump up to 0.034/mm | — |
| | G2 blend walls → crown (and the G1 fallback) | **refused**: "the support surface is too small …" | — | fixed, see below |
| | trim by the base plane, floor patch, stitch | ok: a closed solid | — | (without the crown, which could not be blended) |
| | thicken 1.5 mm in / out | ok, walls 1.50 mm both ways | — | — |
| Car hood | hood and two fenders lofted through three open splines | ok | — | a section that is one open curve needs `curves: ["s1"]`: "H0 has no closed profiles" |
| | G1 blend hood → left fender, G2 blend → right fender | **refused**: "the support surface is too small …" | — | fixed, see below |
| | G2 `surface.match` of a third fender onto a hood edge | **refused**: "the support surface is too small …" | — | fixed, see below |
| | thicken the hood alone 1 mm | ok, 1.00 mm | — | — |
| Bottle | revolve a spline profile, base patch, sweep a radius-5 handle along a spline path | ok | revolved seams: 0.000°, 0/mm | the sweep took 21–28 s and measuring the handle 18–24 s |
| | trim two radius-7 holes through the near side (sketch circles swept along X) | ok | — | — |
| | G1 blends from the handle's ends to the holes | **refused**: "no face of the body holds a picked edge" | — | a traced hole edge lies 8.4e-4 mm off its face and the blend asked for 1e-6 of the body's size (fixed, see below) |
| | thicken the trimmed body 2 mm inwards | **crash** (caught): "kernel internal error: thicken: This wire is not simple." | — | truck panics; with one hole the thicken gives two bodies and an open shell. K5 (#194) |

After the corner fix, two more defects showed up on the same parts:

| Part | Step | Result | Measured continuity | What failed |
|---|---|---|---|---|
| Computer mouse | G0, G1 or G2 blend walls → crown | built, with the warning "the blend's boundary lies up to 5.2652 mm from the edges it was built on"; the stitch leaves it open, and trim and thicken then fail on it | the seams are not seams (one-sided) | some of a closed rim's edges run against the chain the picks make, and the chain sampled them the wrong way round: the boundary row loops 2.5 times round the rim (fixed, see below) |
| Car hood | G1 blend → left fender | ok | G1 0.000°; curvature jump 2.1–2.4/mm | the default G1 cross-section leaves at a third of the chord's speed, so a blend that drops 8 mm over 10 mm turns in a 0.42 mm radius at its ends; the 1 mm thicken then refuses (radius 0.478 mm). `bulge` fills it out; the default is a design choice, not changed here |
| | G2 blend → right fender, G2 match | ok | G2: 0.000°, 0.000/mm | — |
| T-rex skull | G2 snout patch | ok | — | — |
| Bottle | G1 blend from the handle's end to a hole, past the face lookup | **refused**: "the direction towards the other boundary lies in no face at the picked edges: give `align` points" | — | the blend leaves each side in one direction, towards the other side's middle; at a hole that faces the handle that is the sheet's normal, and an `align` point cannot help (a single direction never runs into a hole all round). It should leave straight across the hole's edge (fixed, see below) |
| Mouse, bottle | `stitch: true` on a blend between rims of several edges | the stitch leaves the seams open | seams one-sided | the blend's boundary is one curve; a stitch pairs edges end to end, so it never sews one curve to a rim of eight edges, and matched two curves through the same ends only when their parameter midpoints agreed (fixed, see below) |

## Findings, by impact on organic modelling

1. **G1/G2 blends, matches and G2 patches on lofted surfaces were refused** ("the support
   surface is too small around a picked edge to read its shape"). Wherever a loft's parameter
   lines cross its boundary at a slant (sections of different sizes or heights, which is every
   organic loft), the station at an edge's end sits at a corner of the face's parameters and
   the line across leaves the parameter box whichever way it is read. This blocked the hood, the
   mouse and the G2 snout. Cases: `surfaces/195-g1-blend-between-sheared-strips` (analytic),
   `195-g2-blend-between-sheared-strips`, `195-g2-match-a-sheared-strip-onto-another`,
   `195-g2-patch-capping-a-crown-loft`, `195-g2-blend-from-walls-to-a-crown-loft`,
   `195-g2-patch-on-a-creased-loft`.
2. **A blend between closed rims could stray 5 mm from them** (warning only), so the stitch did
   not close: edges that run against the chain were sampled from the wrong end. Case:
   `surfaces/195-blend-between-the-rims-of-two-extruded-spline-tubes`.
3. **A blend could not start from a trimmed hole** ("no face of the body holds a picked edge",
   then "the direction towards the other boundary lies in no face"): a handle could not meet a
   bottle. Case: `surfaces/195-blend-from-a-hole-trimmed-in-a-revolved-sheet`.
4. **A stitched blend never closed onto a rim of several edges**, so no blended shell became a
   solid. Cases: `surfaces/195-blend-between-the-rims-of-two-extruded-spline-tubes`,
   `surfaces/195-g2-blend-from-walls-to-a-crown-loft` (both close into a solid once the blend
   stays on the rims and the stitch splits the rims' partner).
5. **Thickening a creased or trimmed shell** refuses or panics: K5 (#194). Cases:
   `shell/195-thicken-a-creased-loft-inwards`, `shell/195-thicken-a-revolved-sheet-with-two-trimmed-holes`.
6. **Patch fairness**: a G1 patch through a point, or a G1 cap at a tight corner of its loop,
   curls to a radius of about 1 mm, below the 2 mm wall a print needs, and a crown balloons past
   the loop instead of filling it out. No case: what the fairest fill is has no closed answer.
7. **Default G1 blend speed**: a third of the chord, which turns tightly when the two edges are
   offset; `bulge` around 1–2 gives a fair blend.
8. **The continuity inspection reports only each seam's worst station**, which at a loop's
   creased corners is always the corner. An agent cannot tell a G1 patch that holds between its
   corners from one that does not. A per-seam value away from the ends would answer it.
9. **Smaller frictions**: a loft section that is one open curve needs its `curves` named; a
   fit-point spline is split into one edge per span, so an N-sided patch needs every span picked
   and the refusal does not say which are missing; `inspect.measure` gives open sheets a
   (meaningless, often negative) volume; measuring a thickened loft took 112 s and sweeping the
   handle 21–28 s.

## Fixed

Each fix is its own pull request with a regression test in the existing test file (see #195):

- **The shape across a support surface is read at the corners of its parameters** (finding 1):
  the one-sided line that overshoots the parameter box least is read there, where a B-spline
  goes on as its end span. `bridge.rs`; test `a_blend_reads_sheared_sheets_at_the_corners_of_their_parameters`.
- **A chain of edges is sampled the way it runs** (finding 2): edges the chain turned round were
  read along their own curve's direction (`curve()` instead of `oriented_curve()`), in `bridge`
  and in `surface.match`. Test `surface_blend_between_spline_tube_rims_stays_on_them`.
- **Stitch splits an open edge where another face's corner lies on it, and pairs curves by
  where they run, not their parameters** (finding 4). Tests
  `stitch_splits_an_open_edge_where_another_face_has_a_corner_on_it`,
  `a_stitched_blend_closes_two_capped_tubes_into_one_cylinder`.
- **A blend starts from a trimmed hole** (finding 3): its support face is found within the
  tolerance the stations are read at, and where the direction towards the other side lies in no
  face (and no `align` was given), each side leaves straight across its edge. Test
  `surface_blend_reaches_a_hole_trimmed_into_a_revolved_sheet`.

With all four, the hood's blends and match, the mouse's G2 cap and G2 blend, the skull's G2 snout
and the bottle's handle blends build, and six of the eight `surfaces` cases pass; the two thicken
cases stay with K5.

## Left for K4 and K5

- **K5 (#194)**: thickening a loft inwards across its creases; thickening a revolved sheet with
  trimmed holes (a panic in truck, and a wrong result with one hole); offsets of patched faces
  whose radius is close to the wall.
- **K4 (#126, #184)**: no part here needed a curved-edge fillet once blends worked; a
  `surface.fillet` between two surface bodies (planned, tangency-labs/solidcraft#769) is the K4 piece organic
  shells would use next.

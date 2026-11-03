# Agent eval: parts and systems built over MCP

`run.py` measures how well an AI agent designs real parts and mechanisms in Polykrom, and how
fast. Each task is a plain-language spec with an answer worked out by hand. Claude Code runs
headless with `polykrom-cli mcp` as its only tool (shell, files and web are disallowed),
saves a design, and the harness grades the **saved design**, never the agent's own claims:

- **parts** (rounds 1–4): body count, volume and bounding box from `polykrom-cli eval`,
  within tolerances stated per task (some tasks also check a URDF or an edited input design);
  a task whose spec fixes where the part sits is also scored by **intersection over union**
  against a reference design (below);
- **systems** (rounds 5–6): commands replayed on the design with `polykrom-cli run`. The
  checks are:
  - joint types and names, and materials;
  - motion studies played to given steps (stroke, gear turns, rocker swing against the
    analytic value);
  - joint sweeps against the joint's limits;
  - `inspect.clearance` gaps;
  - `motion.interference` clear over every sample;
  - the bill of materials written as CSV.

It complements `polykrom-cli bench-assistant`, upstream's standard test of the in-app
assistant (eleven single-part prompts through the app's chat). This harness tests an outside
agent through the MCP server, on harder parts and on assemblies.

## Running it

Needs the Claude Code CLI (`claude`), signed in, and a release build of the CLI:

```
cargo build --release -p polykrom-cli
python3 tools/agent-eval/run.py --cli target/release/polykrom-cli --out /tmp/agent-eval --round 5
```

- `--round N` runs one round;
- `--only id1,id2` runs some tasks;
- `--model` picks the model;
- `--max-turns` and `--timeout` bound each task;
- `--grade-only DIR` grades the designs an earlier run saved under `DIR` (each task's
  `DIR/<id>/<id>.polykrom`) without running the agent, and rewrites `DIR/report.json`, keeping
  each task's time, turns and cost from the earlier report. Use it to re-grade a run with a
  newer grader or a newer build of the CLI, or to grade designs made any other way;
- `--check-references` tests the reference designs themselves (below) instead of running the
  agent; it needs no `claude`.

Each task gets its own folder under `--out`: the saved design, the agent's transcript
(`transcript.jsonl`), the MCP config and the grader's replay. `report.json` holds `tasks`,
every task's time, turns, cost, tool calls, tool errors, failures and `iou`, and over the run
`correct`, `tasks`, `mean_iou` and `iou_tasks` (how many tasks the mean is over).

A task costs about $0.10–0.70 and takes about a minute; a round of four costs about $2.
`analysis.static_stress` tasks need Nightfall's solver: set `POLYKROM_NIGHTFALL` to its `nf`.

## Reference designs and IoU

The field scores generated CAD by the mean intersection over union of the generated shape and a
reference shape. The harness does the same, exactly, for the parts tasks whose spec fixes where
the part sits: each such task carries a `reference`, a list of `{command, params}` that
`polykrom-cli run` turns into the intended part (`<id>/reference.json`, `reference.polykrom`;
`edit_plate`'s script runs on the task's input design). Grading inserts the reference into a
copy of the saved design (`component.insert_design`, so the two sit where they were modelled:
nothing is aligned), joins each side's visible bodies into one, and runs `inspect.compare`,
which measures the material only one side has and replies with `intersection_mm3`, `union_mm3`
and `iou`. The task's record gets `iou` (0 when no design was saved; `null`, with `iou_note`
saying why, when the comparison itself failed) and the run gets `mean_iou`. The IoU is
recorded beside the checks and does not change a task's pass/fail.

Precision: `inspect.compare` integrates each region's volume along a grid of at most 40,000
lines, which puts a compact region within about 1 % of its true volume (0.2 % for a 10 mm cube,
0.8 % for a 5 mm one). Two bodies that differ by half therefore score within about 0.005 of
the true IoU, and two that nearly match far closer, since the error is a share of the small
regions over the whole union. A design identical to its reference scores exactly 1.

Which tasks have a reference, and why the others do not (their spec leaves the placement to the
agent, so a correct part elsewhere would score 0; aligning would be a different score):

| Task | Reference | Placement in the spec |
|---|---|---|
| `v_pulley` | yes | the axis is Z, z from 0 to 20 |
| `shelf_bracket` | yes | every extent is given in X, Y and Z |
| `board_tray` | yes | one corner at the origin, standoffs at given x and y |
| `edit_plate` | yes | the input design fixes it |
| `mounting_plate`, `enclosure`, `pocket_fillets`, `nema17_plate` | no | on the XY plane or unplaced: x and y are free |
| `l_bracket`, `aluminium_bracket`, `pipe_elbow`, `spacer_param` | no | unplaced |
| `stepped_shaft`, `flanged_tube`, `cup`, `hex_nut`, `lightened_wheel` | no | on the Z axis, but where along it is free |
| `hole_pattern` | no | the holes' x and y are given, the plate's edges are not |
| `box_and_lid`, `param_table` | no | z is given, x and y are free |
| `hinge_urdf`, `slider_urdf` | no | graded by their URDF, not as parts |

`--check-references` is the references' test: each reference builds, passes its own task's
checks (volume, bbox, bodies), scores `self_iou` 1.0 against a copy of itself, and the same
script without its last command (the groove, the rib, a standoff's hole, a plate hole missing)
scores `wrong_iou` below 1:

```
python3 tools/agent-eval/run.py --cli target/release/polykrom-cli --check-references --out /tmp/refs
```

To add a reference to a task, give the task a `reference` script and run that check.

## Results

Claude Code headless on the default model. Systems rounds were graded by replaying the saved
design.

| Run | Round | Build | Correct | Time | Cost | Tool calls |
|---|---|---|---|---|---|---|
| 2026-10-23 | 1 (parts) | before #52 | 6/6 | 159 s | $1.34 | – |
| 2026-10-23 | 2 (parts, URDF) | before #52 | 6/6 | 222 s | $1.49 | – |
| 2026-10-23 | 3 (a designed arm) | before #52 | 0/1 | 1051 s | $0.44 | – |
| 2026-10-24 | 4 (parts) | #52 | 10/10 | 296 s | $2.64 | 95 |
| 2026-10-24 | 5 (systems) | #55 | 2/4 | 313 s | $2.81 | 89 |
| 2026-10-24 | 5 | #61 | 2/4 | 223 s | $2.14 | 65 |
| 2026-10-24 | 5 | #61–#67 | 4/4 | 203 s | $2.05 | 69 |
| 2026-10-24 | 6 (systems) | #61–#67 | 2/2 | 109 s | $0.87 | 30 |
| 2026-10-24 | 3 (a designed arm) | #71 | 1/1 | 268 s | $0.31 | 12 |

**What moved the round 5 numbers:**

- **#61:** fewer, smaller replies (`select`), the measure cache and clearance reuse.
- **#62 and #63:** a screw in its tapped hole reads as thread engagement, not a clash.
- **The clevis grader:** it now reads `nearest` as well as `near`.

**Round 3:** the agent ran out of time designing an arm from a loose brief on the first build. On #71's build it designed a 150 mm steel I-beam arm that meets both limits (safety factor 2.05 on the finest mesh, 0.125 mm deflection, 152 g) in 268 s.

Four tasks are a small sample, so a single run's time and cost move by 20–40 % from noise alone
(round 5 on one build: 203, 253 and 292 s). Compare runs of the same build before drawing a
conclusion from a difference in time.

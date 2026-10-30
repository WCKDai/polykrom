# Agent eval: parts and systems built over MCP

`run.py` measures how well an AI agent designs real parts and mechanisms in Polykrom, and how
fast. Each task is a plain-language spec with an answer worked out by hand. Claude Code runs
headless with `polykrom-cli mcp` as its only tool (shell, files and web are disallowed),
saves a design, and the harness grades the **saved design**, never the agent's own claims:

- **parts** (rounds 1–4): body count, volume and bounding box from `polykrom-cli eval`,
  within tolerances stated per task (some tasks also check a URDF or an edited input design);
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
- `--max-turns` and `--timeout` bound each task.

Each task gets its own folder under `--out`: the saved design, the agent's transcript
(`transcript.jsonl`), the MCP config and the grader's replay. `report.json` collects every
task's time, turns, cost, tool calls, tool errors and failures.

A task costs about $0.10–0.70 and takes about a minute; a round of four costs about $2.
`analysis.static_stress` tasks need Nightfall's solver: set `POLYKROM_NIGHTFALL` to its `nf`.

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

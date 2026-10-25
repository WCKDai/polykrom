# Polykrom roadmap

Polykrom is a professional, AI-native CAD app for people and their AI companions building things for the physical world and for simulation. This roadmap is the plan of record. It is mirrored by the [GitHub milestones](https://github.com/wckdboy/polykrom/milestones) (one per phase, an issue per item) and by the Plane project **PK · Polykrom**. When a plan changes, change all three in the same sitting.

Status words: **in review** (a PR is open), **in progress**, **next** (planned for this phase), **later** (in the phase, not yet scheduled).

## Where it stands (2026-10-24)

- **Agents already model precisely.** In the evaluation harness (Claude Code over Polykrom's MCP server, graded by measuring the saved design rather than trusting the agent's claims), round 4 built 10 of 10 single parts exactly (volume within 0.2 %, bounding box within 0.05 mm), at 20–50 s and $0.16–0.42 per part.
- **Systems are the frontier.** Round 5 asked for mechanisms. A slider-crank came out with an exact 50.0 mm stroke and 73 collision-free samples plus a GIF. A gear pair had an exact 2:1 ratio. A clevis had joint limits matching the true contact stops to 0.04°. The friction it found (seating a screw on a washer, component names where an occurrence is expected, an unclear measure schema) is P1 work below.
- **The assistant speaks every major provider natively.** That means Anthropic (with thinking and prompt caching), Gemini, and OpenAI-compatible APIs including local models.

## P0 · Foundations & identity (target 2026-10-31)

Polykrom stands on its own: its own name, look, icons, file type, site and a first inspector.

- **Rebrand stage 1: visible name, gem mark, POLYKROM_ env, URLs**: in review · [#8](https://github.com/wckdboy/polykrom/issues/8) · design  
  PR #1. Visible text, alias binaries, polychrome gem logo, README/NOTICE credit to SolidCraft.
- **Facet design language: graphite + cyan palette, Atkinson Hyperlegible type**: in review · [#9](https://github.com/wckdboy/polykrom/issues/9) · design  
  PR #2. OKLCH palette transform with colour-vision tests (CIEDE2000 ≥ 10 under protan/deutan/tritan).
- **Facet icons: all 282 masters redrawn from code, second pass**: in review · [#10](https://github.com/wckdboy/polykrom/issues/10) · design  
  PRs #3 and #7. Icons are accepted only by the owner on the review page.
- **cad.polykrom.app: site, docs and the web app**: in review · [#11](https://github.com/wckdboy/polykrom/issues/11) · platform  
  PR #4. Cloudflare Worker serving the gzip wasm; the bare polykrom.app is a separate site and is never touched.
- **Inspector: mass, volume, size and last stress result per body**: in review · [#12](https://github.com/wckdboy/polykrom/issues/12) · design  
  PR #5. Docked or floating; staleness by body fingerprint.
- **.polykrom design files (.solidcraft/.solvecraft still open)**: in review · [#13](https://github.com/wckdboy/polykrom/issues/13) · platform  
  PR #6. Format tag stays solidcraft/N for compatibility.
- **Rebrand stage 2: crate names, bundle ids, settings migration, updater key**: later · [#14](https://github.com/wckdboy/polykrom/issues/14) · platform  
  Deferred until P0 PRs merge; needs a settings-folder migration and a new minisign release key.

## P1 · AI companion (target 2026-11-21)

Any good model, any provider, builds precise parts and working systems from words, sketches and photos, and proves them.

- **Native providers: Anthropic, Gemini, OpenAI-compatible presets, model picker**: in review · [#15](https://github.com/wckdboy/polykrom/issues/15) · ai  
  Anthropic Messages (adaptive thinking, prompt caching, thinking blocks preserved), Gemini generateContent (thought signatures), presets for OpenAI, OpenRouter, Mistral, xAI, DeepSeek, Groq, Ollama, LM Studio; Test fills a model picker.
- **Agent eval suite in the repo: parts and systems rounds with analytic graders**: next · [#16](https://github.com/wckdboy/polykrom/issues/16) · ai  
  Round 4 (parts) 10/10, round 5 (systems: slider-crank, gear pair, clevis fit, bolted BOM) graded by replaying the saved design. Publish the harness, tasks and scores; track cost and time per task.
- **Assembly friction from evals: seat parts on placed parts, component names as occurrences**: in progress · [#17](https://github.com/wckdboy/polykrom/issues/17) · assembly  
  parts.insert `at` ignores occurrence placement (a screw cannot seat on a washer); occurrence.ground rejects a component name with one occurrence; inspect.measure item schema; slider axis is the origin's z (undocumented); loop-stop hints.
- **MCP instructions and skills for assemblies, joints, motion, materials, analysis**: next · [#18](https://github.com/wckdboy/polykrom/issues/18) · ai  
  The server instructions cover only part modelling. Add the assembly workflow: components → ground → joints (axis = origin z) → limits by sweep → motion study → interference → animation → static stress.
- **Image and sketch to model: reference images, calibration, dimension extraction**: next · [#19](https://github.com/wckdboy/polykrom/issues/19) · ai  
  Attach a photo or drawing, calibrate it on a plane (canvas.calibrate), let the model trace and dimension; verify against stated sizes.
- **Chat UX: streaming answers, token and cost meter, cache hits**: next · [#20](https://github.com/wckdboy/polykrom/issues/20) · ai  
  Stream API answers; show tokens, cost and cache reads per turn; one-click retry and branch.
- **Plan → build → verify loop with checkable targets**: next · [#21](https://github.com/wckdboy/polykrom/issues/21) · ai  
  The agent states targets (volume, mass, fits, clearances, stroke) before modelling and closes the turn only after measure/interference/sweep confirm them.
- **Fast edit lane: small model for quick edits, large model for design**: later · [#22](https://github.com/wckdboy/polykrom/issues/22) · ai  
  Route by task; keep the same tools and verification.

## P2 · Assemblies, mechanisms & fitment (target 2026-12-19)

Mechanisms an engineer trusts: mates from features, real fits and tolerances, robust kinematics, gears, robot export.

- **Feature-aware mates: pin-in-hole, face-on-face, axis-to-axis from picks**: next · [#23](https://github.com/wckdboy/polykrom/issues/23) · assembly  
  Infer the joint type and origins from two picked features; agents get the same with `joint.mate`.
- **ISO 286 fits and tolerance stack-ups**: next · [#24](https://github.com/wckdboy/polykrom/issues/24) · assembly  
  H7/g6-style fit classes on holes and shafts; clearance report by fit; worst-case and RSS stacks along a chain.
- **Kinematics robustness: singular poses, closed loops, drivers**: next · [#25](https://github.com/wckdboy/polykrom/issues/25) · assembly  
  Dead-centre poses, four-bars, slider-cranks, gear/cam/screw coverage tests; clear stop reasons.
- **Involute gear generator: spur, helical, rack, internal**: next · [#26](https://github.com/wckdboy/polykrom/issues/26) · assembly  
  True tooth profiles from module, teeth, pressure angle, profile shift; gear joints from pitch radii; backlash check by sweep.
- **Robot export: URDF, MJCF (MuJoCo) and SDF with inertias**: next · [#27](https://github.com/wckdboy/polykrom/issues/27) · assembly  
  Masses and inertia tensors from materials; joint limits, effort and velocity from actuator parts.
- **BOM and metadata: part numbers, vendor links, cost and mass roll-ups**: later · [#28](https://github.com/wckdboy/polykrom/issues/28) · assembly  
  Per-part licence and attribution carried into BOM exports.

## P3 · Simulation & animation (target 2027-01-30)

From a working mechanism to evidence: stress fields, dynamics, and animations people can share.

- **Stress contours on the part from Nightfall's surface field**: in progress · [#29](https://github.com/wckdboy/polykrom/issues/29) · simulation  
  Nightfall PR #322 writes the field (`nf study solve --field`); Polykrom draws banded von Mises on the body with a legend (branch feat/stress-contours).
- **Assembly stress: multi-body studies with bonded and contact pairs**: later · [#30](https://github.com/wckdboy/polykrom/issues/30) · simulation  
  Through Nightfall's MFEM service; loads from joints and motors.
- **Modal and thermal studies**: later · [#31](https://github.com/wckdboy/polykrom/issues/31) · simulation  
  Natural frequencies for arms and brackets; steady thermal for enclosures and motor mounts.
- **Rigid-body dynamics: gravity, torques, motors**: later · [#32](https://github.com/wckdboy/polykrom/issues/32) · simulation  
  Permissive engine (candidate: Rapier, Apache-2.0) behind an RFC; motor torque curves from the parts library.
- **Animation: camera paths, MP4/WebM, rendered frames**: later · [#33](https://github.com/wckdboy/polykrom/issues/33) · simulation  
  motion.export_animation already writes GIF/PNG frames; add video and path-traced renders.
- **Simulator bridges: MuJoCo, Isaac Sim, Gazebo, LeRobot**: later · [#34](https://github.com/wckdboy/polykrom/issues/34) · simulation  
  Round-trip robot descriptions; sim-ready collision meshes.

## P4 · Parts library: robotics & electronics (target 2027-02-27)

Real parts to learn on, replicate or build with, every one with mates, mass, specs and a clean licence.

- **Parts library RFC: data model, sources, licences**: next · [#35](https://github.com/wckdboy/polykrom/issues/35) · parts  
  Part = parametric generator or pinned file (commit + checksum) + mates + mass/material + mechanical/electrical specs + datasheet link + licence/attribution carried into exports. Tiers: generated (licence-clean), open catalogs fetched on demand, bring-your-own vendor CAD.
- **Actuators: hobby servos, Feetech STS3215, Dynamixel, NEMA steppers, BLDC and gimbal motors**: in review · [#36](https://github.com/wckdboy/polykrom/issues/36) · parts  
  Generated from datasheet dimensions: SG90, MG996R, STS3215, XL430/XM430, NEMA 14–23, the GB4106 gimbal motor and the AK80-6 actuator, with their output axes as joint origins, ratings and rated masses; later XL330, NEMA 8, outrunners 2204–5010.
- **Power: 18650/21700 cells, LiPo packs, holders, XT30/XT60**: later · [#37](https://github.com/wckdboy/polykrom/issues/37) · parts  
  Packs by S × capacity with Wh and mass; connector keep-outs.
- **Motion hardware: extrusions, rails, bearings, belts, couplers**: later · [#38](https://github.com/wckdboy/polykrom/issues/38) · parts  
  2020/2040/3030 with brackets and T-nuts, MGN rails, LM8UU, GT2 pulleys/belts, couplers, bearings (608 exists).
- **Board envelopes: Arduino, Raspberry Pi, Pico, XIAO, ESP32**: later · [#39](https://github.com/wckdboy/polykrom/issues/39) · parts  
  Exact outlines, mounting holes and connector keep-outs from published drawings; no logos.
- **Open catalogs on demand: Adafruit, Raspberry Pi, SparkFun, Arduino, KiCad, FreeCAD-library**: later · [#40](https://github.com/wckdboy/polykrom/issues/40) · parts  
  Adafruit_CAD_Parts (MIT), Raspberry Pi STEP (MIT), SparkFun 3D_Models and Arduino (CC BY-SA 4.0), KiCad packages3D (CC BY-SA + exception), FreeCAD-library (CC BY 3.0); fetched at the user's request, pinned, attributed.
- **Robot kits as assemblies: LeRobot SO-100/101, LeKiwi, Koch**: later · [#41](https://github.com/wckdboy/polykrom/issues/41) · parts  
  Apache-2.0/MIT sources assembled with joints, ready for URDF/MJCF and LeRobot.
- **Bring-your-own vendor CAD: McMaster-Carr link-out, drag-in STEP, auto mates**: later · [#42](https://github.com/wckdboy/polykrom/issues/42) · parts  
  No scraping or bundling of McMaster files (terms and robots.txt); the user downloads, Polykrom detects holes/shafts for mates. Ask ROBOTIS, Seeed and Pollen for permission; Reachy Mini hardware is CC BY-SA-NC (fetch only, flagged).
- **Library browser and agent tools: search, preview, insert with snap**: later · [#43](https://github.com/wckdboy/polykrom/issues/43) · parts  
  Search and filters with thumbnails; insert seats on a hole; `parts.search` for agents.

## P5 · Platform & release (target 2027-03-31)

A signed, updatable app on every desktop, plus the web app and docs.

- **macOS signing, notarisation and updater with Polykrom's key**: later · [#44](https://github.com/wckdboy/polykrom/issues/44) · platform
- **Windows and Linux packages**: later · [#45](https://github.com/wckdboy/polykrom/issues/45) · platform
- **Web app on cad.polykrom.app with the offline test in CI**: later · [#46](https://github.com/wckdboy/polykrom/issues/46) · platform  
  wasm build already deployed; add Chrome-based web-offline-test to CI.
- **Docs and guides for AI workflows**: later · [#47](https://github.com/wckdboy/polykrom/issues/47) · platform  
  mdBook at cad.polykrom.app/docs: providers, prompting for precise parts, assemblies, studies.

## P6 · Upstream & community (target 2027-03-31)

Fixes that help SolidCraft go upstream first; contributors can find their way in.

- **Upstream engine fixes to SolidCraft**: next · [#48](https://github.com/wckdboy/polykrom/issues/48) · upstream  
  Seat on placed parts, component names as occurrences, and other generally useful fixes, sent to tangency-labs/solidcraft.
- **Contribution guide, issue templates, roadmap upkeep**: next · [#49](https://github.com/wckdboy/polykrom/issues/49) · upstream  
  Keep docs/ROADMAP.md, GitHub milestones and the Plane project (PK) in step.

## Principles

- **Verified, not claimed.** Every AI feature ships with evals that measure the result: volume, mass, fits, clearances, stroke, safety factor.
- **Licence-clean by construction.** Generate parts from published dimensions where we can. Fetch open catalogues on demand, pinned and attributed. Never scrape or bundle vendor CAD whose terms forbid it. See [parts-library-plan.md](parts-library-plan.md).
- **Upstream first.** Fixes that help SolidCraft go to tangency-labs/solidcraft before or alongside Polykrom.
- **No panics, wasm stays green, `cargo xtask ci` is the gate.** See AGENTS.md.

# RFC 0001: PolyCore, Polykrom's Rust kernel

- **Status:** Draft, for the maintainer's decision
- **Author:** Claude (agent), for the maintainer
- **Created:** 2026-10-26
- **Builds on:** [docs/ENGINES.md](../ENGINES.md) (kernel stages K0 to K7). This RFC does not
  change those stages; it says what the kernel becomes, how the code gets there, and how each
  step is judged.

Polykrom had no RFC convention before this one. This file starts `docs/rfcs/`.

## Summary

PolyCore is Polykrom's geometric kernel. It is written in Rust and aims to be the most capable
and fastest kernel we can build. Polykrom is its only consumer. It grows out of today's
`crates/kernel` and the vendored truck crates by incremental replacement, not a rewrite: the
app ships on every phase, and every phase has an exit gate measured on the existing corpora.

Nightfall is retired as a product. It is not a consumer. It is a source: its Rust NURBS math,
its kernel bake-off method, its selector ideas, its study runner, its USD plan and its eval
harness move into PolyCore or Polykrom. Its OCCT C++ and its Swift app do not.

OpenCASCADE stays where it is useful without its licence reaching us: a separate program that
answers the same questions, so PolyCore can be scored against it. Graded answers are analytic,
hand-calculated or invariants. OCCT is a cross-check, never the answer key.

## Decisions recorded (owner, 2026-10-26)

| Decision | Consequence |
|---|---|
| One kernel, in Rust, named **PolyCore** | All kernel work goes into PolyCore. No second kernel is started. |
| **Polykrom** is the product and PolyCore's only consumer | No C ABI, no external kernel protocol, no Swift or C++ client. The API is a Rust API. |
| **Nightfall is retired** as a product | No new Nightfall features. Its useful parts are harvested (see [Harvest from Nightfall](#harvest-from-nightfall)). |
| Convergence, not rewrite | Each phase replaces one layer behind a stable boundary; the app keeps working. |
| **TinyRex** is the flagship acceptance test | The agent rebuilds it from scratch on every phase, through Polykrom's own tools, with results at least as good as the phase before. |

## Where we start

Measured on `polykrom/main` at `0489d63e` (2026-10-26) unless stated.

| Thing | State |
|---|---|
| `crates/kernel` (`solidcraft-kernel`) | 153 files, about 98,000 lines of Rust. About 70 public entry points, all free functions; no truck type in the public API. `#![forbid(unsafe_code)]`; panics contained by `guard`. |
| `crates/geom` (`solidcraft-geom`) | About 15,700 lines: vectors, planes, B-splines, meshes, BVH, profiles. No dependencies but serde and thiserror. |
| Vendored truck (`vendor/`) | truck-geometry 10,065 lines, truck-shapeops 6,664, truck-meshalgo 6,253, truck-stepio 4,974, with Polykrom's fixes listed in `vendor/README.md`. truck-topology and truck-modeling come from crates.io. Apache-2.0. |
| Native B-rep schema 3 | `solidcraft_geom::brep`, documented in [native-brep.md](../native-brep.md). Exact, kernel-neutral, already the cache format inside designs. |
| [kernel-cases](../kernel-cases.md) | **288 / 382 pass.** Booleans 64/89, crashes 6/10, fillets 38/50, import 19/23, sheet metal 57/57, shell 9/20, surfaces 0/8, sweeps 62/83, tessellation 12/14, threads 21/28. |
| [kernel ladder](../kernel-ladder.md) | **102 / 116 pass**, with a timing golden (`tests/kernel-cases/ladder/golden.json`). |
| [Reference tests](../oracle.md) | **83 / 83 pass** (62 analytic, 21 FreeCAD run as a separate program). |
| Recent kernel work | 18 merged kernel PRs since 2026-10-19, among them PRs #189 to #236: K2 broad phase, mesh reuse, direct split, exact plane–plane SSI, presearch grids; K4 rolling-ball fillets across loft seams and vertex blends (`loopblend.rs`, `curveblend.rs`, `blend_corner.rs`); K5 NURBS offset fitting, free-form shell and thicken (`offset_fit.rs`, `offset_free.rs`). Several are still open (#224 to #236). |
| Known weak points (ENGINES.md) | Surface–surface intersection starts from a mesh collision. Boolean tolerance is a fraction of part size, wrapped in a retry ladder (4 shifts × 2 tolerances) and about ten special-case modules. Cylinders, cones, spheres and tori are not first-class surfaces. Ten corpus cases hang past 180 s. |

Other kernels and code we can draw on:

| Source | Licence (checked 2026-10-26) | State |
|---|---|---|
| Nightfall (wckdboy/nightfall) | Apache-2.0, our own | OCCT 7.9.3 behind a C++ `IKernel` in an XPC service. Rust crates under RFC 0008: `nightfall-math` (about 780 lines: checked knot vectors, basis functions, NURBS curves and surfaces with mixed partial derivatives, std only, no unsafe); thin Rust wrappers over the OCCT adapter; `nightfall-assembly` (an SE(3) placement solver). A study service in C++ (MFEM physics, Netgen meshing, a sparse Cholesky solve). |
| SMLib (in NVIDIA-Omniverse/usd-brep 1.0.0) | Apache-2.0 (LICENSE: © NVIDIA) | A complete C++ kernel. Scored **50.7** against OCCT's **77.4** in Nightfall's RFC 0001 bake-off: no operation history API, area errors up to 2e-4 on curved faces, a use-after-free in booleans on closed edges (SMLIB-001), all-or-nothing fillet requests. On Polykrom's `kernel-smlib` branch it passed 317 of 372 kernel-cases (ENGINES.md). |
| solidcore (tangency-labs/solidcore) | Apache-2.0 (LICENSE: © NVIDIA, as a port) | A safe-Rust port of SMLib's kernel and SM_API, started 2026-10-12. 238 of 261 units ported, 116 green (identical to the C++ within 1e-9), 122 partial; 89.8 % of upstream lines. Integration and end-to-end solids are still in progress; API 0.0.x. Upstream SolidCraft has paused its own truck work for it (CONTRIBUTING.md, tangency-labs/solidcraft#1597). |
| truck (ricosjp/truck) | Apache-2.0 | Active. Clean, but booleans are mesh-seeded and fillets are weak. |
| Fornjot | 0BSD | Archived June 2026, goals unreached. |
| OpenCASCADE 7.9.3 | LGPL-2.1 | The only open kernel with the full set. Out of process only. |

## Sources: what we may read, and how we use it

| Source | Read the code? | Use |
|---|---|---|
| Polykrom, Nightfall | Yes | Our own code (Apache-2.0, and MIT OR Apache-2.0 in Polykrom). Port freely, keep the history in commit messages. |
| truck | Yes | Keep behind adapters until replaced; port pieces where they pass the corpus. |
| SMLib and SM_API (`source/SMLib`, `source/SM_API` in usd-brep) | Yes | Read as a reference implementation; port algorithms with attribution: NVIDIA's copyright line in the file header, an `ATTRIBUTION.md` row, `NOTICE` updated. Not linked. |
| solidcore | Yes | Same as SMLib. Can be a dependency for a unit if it wins on the corpus. |
| usd-brep `omniSolid` schema | Yes (the schema and its docs) | A data format: PolyCore reads and writes it with its own code. |
| usd-brep's OCCT converters (`tools/occt_to_usd`, `tools/usd_to_occt`, `OCCT_BREP_IMPORT`) | **No** | They are built on OCCT headers. Skip those directories. |
| OpenCASCADE | **No** | Public documentation only (for example the Boolean Operations specification and the test-system guide), and black-box runs as a separate program. Never read, copied or ported. |
| FreeCAD, Netgen | **No** | Separate programs only (FreeCAD's oracle already works this way). |
| Papers and books | Yes | Jackson's local tolerances, Patrikalakis–Maekawa, the 2026 ACM TOG NURBS SSI papers, Rossignac–Requicha offsets, Choi–Ju and Várady–Rockwood blends, generalised winding numbers for trimmed NURBS. Full list in ENGINES.md. |
| Commercial CAD | **No** | No data, measurements or files made with them (AGENTS.md). |

Licence of PolyCore itself: the Polykrom workspace is MIT OR Apache-2.0. Code ported from
SMLib or solidcore is Apache-2.0 only, so those files stay Apache-2.0 only and say so in their
header. See [Open decisions](#open-decisions).

## Scope

### What PolyCore owns

| Area | Contents |
|---|---|
| Geometry | Points, vectors, tolerances, intervals, robust predicates. Curves: line, circle, ellipse, B-spline, NURBS, procedural intersection curves with a fitted spline, p-curves. Surfaces: plane, cylinder, cone, sphere, torus, extrusion, revolution, offset, B-spline, NURBS. Evaluation with derivatives, projection, closest point. |
| Topology | Exact B-rep: bodies, shells, faces, loops, coedges, edges, vertices. Tolerant edges and vertices with an absolute model resolution. A validity checker run after every operation in debug builds. |
| Operations | Primitives, extrude, revolve, sweep, loft, booleans (union, cut, intersect, split, general fuse), blends (fillets, chamfers, vertex blends), offsets, shell, thicken, draft, replace and delete face, sewing and healing. |
| Naming and lineage | For every operation, the exact history of every result face and edge (as `history.rs` records today), entity identity that survives cloning, and composition of histories across operations. |
| Tessellation | Display and export meshes with a **watertight guarantee**: a closed body gives a closed, consistently oriented 2-manifold mesh with welded vertices, or a refusal that says why. Face and edge tags on every triangle and segment. |
| Mass properties | Volume, area, centre of mass, inertia tensor and principal axes from the exact B-rep, with the error bound achieved. |
| Analysis outputs | Watertight surface meshes with face tags for meshers; exact inertia for robot export; contact geometry (recognised analytic primitives, convex pieces, clearances); mid-surfaces with a thickness field for thin-walled FEA. |
| Interchange | STEP, IGES, the native schema 3, 3MF meshes with face tags, USD `omniSolid` B-rep and USD meshes (see [Interchange](#interchange)). |

### What stays above the kernel

"Fully parametric" lives in Polykrom's document layer, not in PolyCore.

| Layer | Owns | Talks to PolyCore with |
|---|---|---|
| `solidcraft-doc` | Parameters, expressions, units, configurations, the feature timeline, dependency-aware rebase, persistent names (`lineage.rs` turns kernel histories into names), face attachments, components, joints, motion studies | Explicit geometry and operation specs: profiles, distances, edge sets. Never parameters, features or names. |
| `solidcraft-sketch` | The 2D constraint solver and profiles | Profiles as curves |
| `solidcraft-engine`, `solidcraft-mcp`, control channel | Commands, MCP, scripts, undo | Through the document, never directly for modelling |
| Studies and simulation | MuJoCo (`tools/mujoco/runner.py`), the FEA study runner (below) | Files and processes: meshes, inertia, face tags |

The boundary rule: PolyCore takes values and returns values plus history. It never reads a
design, a parameter or a feature. The document decides what a face is called; PolyCore
reports where each face came from. `cargo xtask` layering checks enforce that no PolyCore
crate depends on a Polykrom crate.

### What PolyCore does not own

- Constraint solving for sketches and assemblies.
- Volume meshing and solvers. Tetrahedral meshing, FEA, multibody dynamics and CFD run as
  separate processes or crates fed by PolyCore's outputs.
- Rendering.

## Architecture

### Crates

| Crate | Owns | Starts from |
|---|---|---|
| `polycore-math` | Vectors, frames, tolerances, intervals, predicates, knot vectors, B-spline and NURBS evaluation with derivatives, root finding, least-squares fitting | `nightfall-math`; `solidcraft-geom` (`vec`, `unit_vec3`, `plane`, `bspline`, `spline_space`); `robust` and `inari` (permissive) for predicates |
| `polycore-geom` | Curve and surface types as tagged enums, projection, closest point, the SSI dispatch table, procedural intersection curves, offset surfaces and fitting | Schema 3 records; `tess/ssi.rs`; `offset_fit.rs`; truck-geometry evaluation where it passes the corpus |
| `polycore-topo` | The B-rep arena with generational ids, tolerant entities, p-curves, Euler operators, the validity checker | New; replaces truck-topology. Shape of the data follows schema 3. |
| `polycore-ops` | Every modelling operation | `crates/kernel` modules, moved one area at a time |
| `polycore-mesh` | Tessellation, the watertight guarantee, BVHs, mesh queries | `crates/kernel/src/tess/`, `solidcraft-geom` (`mesh`, `bvh`, `tess`) |
| `polycore-naming` | Histories, entity identity, history composition | `history.rs`, `provenance.rs`, the kernel-generic part of `doc/lineage.rs` |
| `polycore-props` | Mass properties, distance, clearance, contact geometry, mid-surfaces | `measure.rs`, `distance.rs`, `clearance.rs` |
| `polycore-io` | STEP, IGES, schema 3, 3MF meshes, USD | `step_in/`, `step_out.rs`, `iges_*`, `native/` |
| `polycore` | The public API: the `Kernel` trait, types, re-exports | `crates/kernel/src/lib.rs` |
| `polycore-legacy` | Every remaining truck call, behind the same types. Shrinks every phase; deleted in Phase 6. | The truck-facing parts of `crates/kernel` |
| `polycore-bench` (dev only, not shipped) | Corpus runner, bake-off scoring, the oracle client, the minimiser | `xtask kernel-cases`, Nightfall `tools/bakeoff` (harvested) |

Dependencies point one way: math ← geom ← topo ← (ops, mesh, naming, props, io) ← polycore.
Polykrom crates depend on `polycore` only.

### How today's code moves

| Today | Becomes | How |
|---|---|---|
| `crates/kernel` | `polycore` (facade) | Phase 0 renames the package and extracts the trait; no behaviour change. Its modules then move out to the crates above, one area per PR. |
| `crates/geom` numeric core | `polycore-math`, `polycore-mesh` | Moved; `solidcraft-geom` keeps app-facing pieces (profiles, sections) and depends on `polycore-math`. |
| `nightfall-math` | `polycore-math` | Copied with its tests and probes. Where it overlaps `solidcraft-geom::bspline`, the faster one that passes both test sets stays. |
| truck-geometry (vendored) | `polycore-geom` | Replaced in Phase 1. |
| truck-topology, truck-modeling | `polycore-topo`, `polycore-ops` | Replaced in Phases 1 and 2; truck bodies convert through schema 3 meanwhile. |
| truck-shapeops (vendored) | `polycore-ops` booleans | Replaced in Phase 2, with the retry ladder and the special-case modules (`coplanar`, `coaxial_overlap`, `tangent_cylinders`, `touching_walls`, …) as their cases pass. |
| truck-meshalgo (vendored) | `polycore-mesh` | Replaced in Phase 1 (most tessellation is already ours in `tess/`). |
| truck-stepio (vendored) | `polycore-io` | STEP import is already ours (`step_in/`); export moves in Phase 1. |
| Blends, offsets, sweeps, lofts in `crates/kernel` | `polycore-ops` | Re-based onto `polycore-topo` in Phases 3 to 5, keeping their kernel-cases. |

Schema 3 is the bridge during the move: any body can go truck → schema 3 → PolyCore and back
exactly, so old and new code can run in one feature and in one design.

### The API

One trait, so that the same corpus and the same bake-off run on any implementation: PolyCore,
the legacy truck path while it exists, and (in tests only) the OCCT oracle as a separate program.

```rust
pub trait Kernel: Send + Sync {
    /// Name, version and build hash; part of every cache key.
    fn identity(&self) -> &KernelId;
    fn capabilities(&self) -> &Capabilities;

    fn primitive(&self, cx: &Cx, spec: &Primitive) -> Outcome<Body>;
    fn extrude(&self, cx: &Cx, profile: &Profile, spec: &Extrude) -> Outcome<Body>;
    fn revolve(&self, cx: &Cx, profile: &Profile, spec: &Revolve) -> Outcome<Body>;
    fn sweep(&self, cx: &Cx, spec: &Sweep) -> Outcome<Body>;
    fn loft(&self, cx: &Cx, spec: &Loft) -> Outcome<Body>;
    fn boolean(&self, cx: &Cx, op: BoolOp, targets: &[Body], tools: &[Body]) -> Outcome<Vec<Body>>;
    fn blend(&self, cx: &Cx, body: &Body, spec: &Blend) -> Outcome<Body>;
    fn offset(&self, cx: &Cx, body: &Body, spec: &Offset) -> Outcome<Body>; // shell, thicken, offset faces, draft
    fn split(&self, cx: &Cx, body: &Body, tool: &SplitTool) -> Outcome<Vec<Body>>;

    fn check(&self, body: &Body) -> Validity;
    fn mass(&self, cx: &Cx, body: &Body) -> Outcome<MassProps>;
    fn tessellate(&self, cx: &Cx, body: &Body, spec: &Tessellation) -> Outcome<Mesh>;
    fn read(&self, cx: &Cx, format: Format, bytes: &[u8]) -> Outcome<Vec<Body>>;
    fn write(&self, cx: &Cx, format: Format, bodies: &[Body]) -> Outcome<Vec<u8>>;
}

/// A result with its history and evidence, or a refusal that says why.
pub type Outcome<T> = Result<Done<T>, Refusal>;
pub struct Done<T> { pub value: T, pub history: History, pub evidence: Evidence }
pub enum Refusal { Unsupported, InvalidInput, Failed, OutOfBudget, Cancelled, Internal }

/// Model resolution, time and attempt budget, thread budget, progress and cancellation.
pub struct Cx { /* … */ }
```

- `Body` is immutable, `Send + Sync`, shared by `Arc`, and carries a content hash.
- `Evidence` says how a result was checked: tolerance achieved, fit errors, validity, time.
  No caller gets a bare number (ENGINES.md principle 4).
- Every refusal carries the stage and the entities involved. No operation returns a partial body.
- Today's 70 entry points map onto these methods with typed specs. App-level features built
  from them (emboss, lips, threads, sheet metal) stay in Polykrom unless they need kernel internals.
- The trait is semver-versioned. It is stable from the end of Phase 2; before that it may
  change in any PR that also updates every caller.
- There is no C ABI. Polykrom is Rust and is the only consumer. A C ABI needs its own RFC if
  a non-Rust consumer ever appears.

### Determinism and threading

1. **Same input, same output.** The same inputs on the same build give bit-identical bodies,
   histories and meshes, whatever the thread count. Across targets (Linux x86-64, macOS
   aarch64, wasm32) topology and history are identical and coordinates agree within the model
   resolution. The corpus runs twice per CI run, on 1 thread and on all cores, and compares hashes.
2. **No hash order.** No output depends on iterating a `HashMap` or on addresses. Ordered maps
   and arena indices only (truck's face order once followed hash order; `vendor/README.md`).
   A Clippy `disallowed-types` rule enforces it in PolyCore crates.
3. **Ordered parallelism.** Parallel work maps independent items and collects in item order
   (the `par::map` pattern). Floating-point sums run in a fixed order, never in the order
   workers finish.
4. **No global state.** No statics with interior mutability. The current thread-local `Watch`
   becomes a field of `Cx`.
5. **Immutable bodies.** Operations are pure functions of their inputs and `Cx`.
6. **Budgets.** Every operation runs under a time and attempt budget and checks cancellation.
   A hang is a failure the corpus records, never an accepted state.
7. **No nudges.** Retrying with shifted operands is retired with the retry ladder in Phase 2:
   a result that depends on a random-looking shift is not reproducible in meaning.
8. **wasm32.** Single-threaded, same results. The browser build stays green in every phase.
9. **No randomness** outside seeded tests.

## Convergence plan

### Gates for every phase

A phase ends when all of these hold, measured on that phase's PolyCore and attached to its
closing PR:

| Gate | Rule |
|---|---|
| **G-cases** | kernel-cases: no area's pass count drops, and no case that passed fails. New cases may be added; the baseline is regenerated with them. |
| **G-ladder** | Ladder: no case that passed fails; no tier's total time exceeds its golden by more than 10 % without a recorded reason. |
| **G-oracle** | Reference tests stay 83/83 (or all of a larger set). |
| **G-determinism** | The determinism check above passes on the whole corpus. |
| **G-bakeoff** | The weighted score against OCCT (below) is not lower than the phase before, and meets the phase's target. |
| **G-tinyrex** | TinyRex rebuilt from scratch by the agent, results at least as good (below). |
| **G-machines** | The printer and the dog evaluate from their saved designs with no feature errors, the same body and face counts, and mass properties within 1e-6 relative of the phase before
  (or a recorded reason, backed by an analytic check, where a better kernel changes the shape); the dog's `sim.mujoco` walk is `ok`. |
| **G-ci** | `cargo xtask ci` green, including the wasm32 build. |

### TinyRex, the flagship test

TinyRex is the robot built through Polykrom's MCP server, commands and control channel. It
found #216 (MJCF meshes with gaps) and the free-form shell cases behind #194. On every phase
the agent builds it **from an empty design**, from a committed spec, using only Polykrom's
tools (`polykrom-cli mcp`, engine commands, the control channel), on that phase's PolyCore.
`tools/agent-eval` grades the saved design, never the agent's account of it.

| Measure | At least as good means |
|---|---|
| Build | Every feature evaluates. Every body is one valid closed shell by PolyCore's checker. |
| Mass | Total mass and each body's mass, centre of mass and inertia from the exact B-rep, within the spec's tolerances. |
| Meshes | Every closed body exports a watertight mesh (no open or over-shared edges); MuJoCo prints no "not a closed solid" warning. |
| Simulation | The walk study is `ok` (finite, no resets, not fallen), with every body's mass counted. Peak servo loads are reported; a load past a rating is a design finding for the spec, not a pass. |
| Finish | `inspect.polish` score not lower than the phase before. |
| Kernel | Refusals and workarounds the agent hit: not more than the phase before. Kernel time: not more. |
| Agent | Turns, wall time and cost: median within 1.25× of the phase before (agent runs vary). |

The agent runs three times per phase; the gate reads the median run, and all three reports
are attached. Phase 0 records the baseline on truck. The spec and its hand-worked checks are
committed to `tools/agent-eval` in Phase 0 (today the design lives only on the maintainer's
machine).

### The bake-off against OCCT

Nightfall's RFC 0001 rubric, reused with its weights and normalisation, becomes PolyCore's
standing score. OCCT 7.9.3 runs as a separate program. Its results are never the answer key.

| Metric | Weight | Change from Nightfall's RFC 0001 |
|---|---|---|
| Operation success | 30 % | Unchanged: valid closed solids ÷ operations attempted. |
| Correctness | 20 % | Graded against analytic or invariant answers only, within 1e-6 relative. Cases without one are reported, not scored. |
| Operation history | 20 % | Measured by a probe that traces every result face and edge. |
| Fillet and blend quality | 10 % | Unchanged. |
| Performance | 10 % | Unchanged: median time over shared successes, release build. |
| Robustness | 10 % | Replaces "integration cost", which only made sense for C++ kernels: 40 for no panic and no hang on the corpus, 30 for the determinism check, 30 for a green wasm32 build. |

Corpus: the 20 reference parts from Nightfall's bake-off (re-expressed as Polykrom recipes with
their analytic answers), kernel-cases, the ladder, the reference tests and the 119-case SSI
corpus. ABC and Fusion 360 Gallery models wait for a licence decision ([Open decisions](#open-decisions)).

Targets, as a share of OCCT's weighted score on the same corpus and machine:

| Phase | Target |
|---|---|
| 2 | ≥ 90 % on the prismatic and turned subset |
| 3 | ≥ 90 % on the whole corpus |
| 4 | Fillet metric ≥ OCCT's |
| 6 | ≥ 100 % overall, and performance metric 100 (PolyCore's median time no slower than OCCT's) |

### Phases

| Phase | ENGINES | What | Phase-specific exit gate | Effort (eng.-months, AI-assisted) |
|---|---|---|---|---|
| **0 Harness and boundary** | K0 | Rename `crates/kernel` to `polycore`; extract the `Kernel` trait over today's code with no behaviour change; `polycore-bench` with the oracle client and the RFC 0001 rubric; import Nightfall's 20 reference parts; commit the TinyRex spec; record every baseline above. Move `analysis.static_stress` off Nightfall's `nf` (see [Studies](#harvest-from-nightfall)). | Baselines recorded; first bake-off score published; TinyRex baseline run on truck. | 1–2 |
| **1 Own geometry and topology** | K1 | `polycore-math`, `-geom`, `-topo`, `-mesh`; analytic surfaces first-class; absolute model resolution, tolerant edges and vertices, p-curves; the validity checker; tessellation and STEP export off truck; mass properties from the exact B-rep; analytic contact primitives. Truck survives only in `polycore-legacy`. | Tessellation 14/14; #216 closed (watertight meshes and B-rep inertia in MJCF); volume and area within 1e-6 relative on every analytic reference test; every corpus body round-trips truck ↔ PolyCore exactly. | 2–4 |
| **2 Booleans** | K2 | General-fuse pipeline (coincidences settled lowest dimension first), the analytic SSI table, the existing BVH broad phase, procedural intersection curves, classification by ray casting with winding numbers as fallback. truck-shapeops, the retry ladder and the special-case modules are deleted as their cases pass. | Booleans ≥ 84/89 and crashes 10/10 in kernel-cases; ladder tier 02 20/20; no hangs; boolean median time ≤ half of truck's; bake-off target. | 4–8 |
| **3 Free-form SSI, sweeps, lofts** | K3 | Subdivision seeding with normal-cone loop tests, marching, tangent and overlap handling; sweeps and lofts on PolyCore topology. | SSI corpus 119/119; sweeps ≥ 78/83; surfaces ≥ 6/8; import ≥ 22/23; bake-off target. | 6–12 |
| **4 Blends** | K4 | Rolling-ball fillets, chamfers and vertex blends on PolyCore topology, porting today's `loopblend`, `curveblend`, `blend_corner`; SMLib and solidcore's fillet units read as references. | Fillets ≥ 47/50; ladder tier 07 17/17; action-matrix fillet failures (#556, 986 today) ≤ 250; bake-off target. | 6–12 |
| **5 Offsets and local operations** | K5 | One extend, re-intersect, re-topologise engine for offset, shell, thicken, draft, replace and delete face, with topology change; mid-surfaces for thin-walled FEA. | Shell ≥ 18/20; ladder tier 09 11/11; no seeded fuzz golden fails on a shell error. | 6–12 |
| **6 Parity and speed** | K6, K7 | Healing and tolerant import, advanced blends as the corpus asks, profiling. `polycore-legacy` and `vendor/truck-*` deleted. | kernel-cases ≥ 95 %; bake-off ≥ 100 % of OCCT including performance. | ongoing |

Phases 3, 4 and 5 can overlap once Phase 2 ends: they touch different operations, and all
three build on the same topology. Polykrom consumes PolyCore from Phase 0 (it is the same
crate, renamed), on desktop and in the browser.

### What happens to each kernel

| Project | Decision | Why |
|---|---|---|
| truck | **Replace piecewise, then drop** (Phase 6) | Its numeric design (mesh-seeded SSI, no local tolerances, no p-curves) is the root cause of most failures. Fixes that help truck users still go upstream. |
| solidcore | **Learn from and port units that win** | Apache-2.0, safe Rust, deterministic, wasm goals: aligned. But it mirrors SMLib's API, which has no history, and SMLib scored 50.7. When solidcore builds solids end to end, run it through the bake-off; any unit that beats PolyCore on its area of the corpus can be ported or depended on. |
| SMLib / usd-brep kernel | **Learn from; never link** | Readable, Apache-2.0, complete. As a C++ library it had 250 memory-safety errors per 1,000 sanitised operations. The `kernel-smlib` and `smlib-link` branches stay experiments, not merged. |
| usd-brep `omniSolid` schema | **Interop only** | A data format for exact solids in USD. |
| OpenCASCADE | **Black-box oracle only** | Scores and cross-checks. Never code. |
| Fornjot | **Literature** | Archived. |

## Harvest from Nightfall

Nightfall is Apache-2.0 and our own, so anything can move. Only what earns its place does.

| Item | What it is | Goes to | How |
|---|---|---|---|
| Rust NURBS math (RFC 0008) | `nightfall-math`: checked knot vectors, basis functions, NURBS curves and surfaces, mixed partial derivatives, fixed stack workspaces (degree ≤ 9), finite-difference and property tests, probes and a bench | `polycore-math` | Copy the crate's source and tests in Phase 1. Merge with `solidcraft-geom::bspline`; keep the one that passes both test sets faster. |
| Bake-off (RFC 0001) | The rubric, the decision rule, the per-process runner (crashes become results), sanitiser runs, the history probe, the 20 reference parts, findings F1–F13 | `polycore-bench`, `tests/oracle/` | Phase 0. Port the Python runner and scoring; re-express the 20 parts as Polykrom recipes with their analytic answers. The C++ kernel drivers stay behind (OCCT-specific). |
| Selectors and feature IR (RFC 0005) | References by how an entity came to be, narrowed by geometry; cardinality (`expect`); content hashes per feature | Polykrom's document and MCP | Polykrom's names already follow exact kernel history, which is stronger than generative names plus OCCT's `Modified`/`Generated`. Take three ideas: a reference that resolves to the wrong count is an error with a pointer, never a guess; geometric filters (`type`, `normal`, `near`, `radius`, `convex`) as an agent-facing query in MCP, so an agent can name a face without a pick; per-feature content hashes, measured against today's dependency-aware rebase before adopting. The IR schema itself is not taken. |
| Studies (RFC 0014) | `services/study`: a study runner, MFEM physics (P2 elasticity and heat), Netgen meshing, a sparse Cholesky solve (Accelerate on macOS), unit handling; the P2 surface field (mid-edge nodes, each P2 triangle drawn as four); the cantilever and thermal checks within 2 % | A `polykrom-study` sidecar program, plus `crates/engine` analysis commands | Polykrom's `analysis.static_stress` calls Nightfall's `nf study solve` today, and its stress contours (#29) read `nf`'s field. Move the runner, the MFEM physics and the field format into a sidecar Polykrom owns, in Phase 0. Netgen stays a separate process and meshes PolyCore's watertight, face-tagged surface mesh instead of an OCCT shape, so no OCCT enters the sidecar. Off macOS the direct solve needs a replacement for Accelerate. |
| USD exchange plan (RFC 0002) | Exact solids in USD through usd-brep's `omniSolid` `BrepArray` | `polycore-io` | Take the plan, not the build: PolyCore writes and reads `BrepArray` prims as `.usda` with its own code. OpenUSD and usd-brep's validator run in CI as separate checking tools. Nothing of OpenUSD is linked. |
| Assemblies and URDF (RFC 0013) | IR v2 components, occurrences and joints; `nightfall-assembly` with residual, rank and DOF diagnostics; URDF export in C++ | Tests in `crates/doc` | Polykrom already has joints, closed loops with DOF and rank, motion studies, URDF and MJCF. Take the four-bar and slider-crank closure cases (residuals < 1e-9) and the URDF test expectations as extra tests. The code is not needed. |
| Evals harness (`agent/evals`) | build, edit and repair tasks over reference parts; a deterministic `reference` runner that checks the harness and the MCP contract without a model; `SKIP` for sessions that could not start; `--resume`; the rule that the pass rate may not drop | `tools/agent-eval` | Polykrom's harness is already broader (parts and systems). Add the edit and repair task kinds, the reference runner, `SKIP` and `--resume`, and the rule. TinyRex is a task there. |

Not worth keeping:

| Item | Why |
|---|---|
| The OCCT C++: `core/kernel` (`IKernel` and its OCCT backend), `core/kernel/native`, the C++ evaluator in `services/kernel`, `urdf.cpp` | OCCT-specific. PolyCore replaces the kernel; Polykrom already has the evaluator and URDF. |
| The Swift app and the XPC service | Polykrom is the app. |
| `nf kernel serve` (RFC 0015) and Polykrom's `feat/nightfall-backend` branch | There is no second kernel to call. The branch closes unmerged. |
| `core/ffi` and the RFC 0008 C ABI | No non-Rust consumer. |
| The Nightfall IR schema and its migrations | Polykrom has its own format (`solidcraft/N`). |
| The OpenUSD and SMLib C++ builds | Interop is through file formats. |
| Nightfall's vendored copy of Polykrom's sketch solver | It came from Polykrom. |

What OCCT still is: a program that answers the bake-off's questions. Phase 0 decides whether
that program is a frozen, unmodified Nightfall `nf` build kept only for scoring, or FreeCAD's
existing oracle ([Open decisions](#open-decisions)).

## Interchange

| Format | Direction | Owner | Plan |
|---|---|---|---|
| STEP AP203/AP214 | read, write | `polycore-io` | Import is ours today (`step_in/`); export moves off truck-stepio in Phase 1. AP242 B-rep next. Checked on `tests/step-conformance`, `tests/step-regressions` and public NIST models, with OCCT reading our files back as a black-box cross-check. |
| IGES | read, write | `polycore-io` | As today. |
| Native schema 3 | read, write | `polycore-io` | PolyCore's persistent body format inside designs. A change bumps the schema; old versions still read. |
| USD `omniSolid` | read, write | `polycore-io` | `BrepArray` prims with analytic and NURBS faces, as the schema defines (Apache-2.0). Our own `.usda` reader and writer. Phase 3. |
| USD meshes and UsdPhysics | write | Polykrom, with PolyCore meshes and inertia | For robot export (ENGINES.md S2). |
| 3MF | write; read as meshes | `polycore-io` for meshes and face tags; `solidcraft-io` for the package | As today ([3mf-face-tags.md](../formats/3mf-face-tags.md)), on PolyCore's watertight meshes. |
| MJCF, URDF | write | Polykrom (`crates/engine`) | Assembly formats stay in Polykrom. PolyCore supplies watertight meshes, exact mass, centre of mass and inertia, and contact primitives. Fixes #216 in Phase 1. |

## Risks and honest cost

### Why this is hard

- Pure-Rust kernels have stalled. Fornjot was archived with its goals unreached. Truck's
  booleans are mesh-seeded and its fillets weak. solidcore has 116 of 261 units identical to
  the C++, and has not yet built general solids end to end.
- The hard parts are known: booleans with tangent and coincident geometry, free-form
  surface–surface intersection, blends where they meet at vertices or roll over an edge,
  and offsets that change topology. Commercial kernels took decades on these.
- Our own kernel already carries about ten special-case modules and a retry ladder. Agents
  that fix symptoms one case at a time will grow more of them.
- Two kernels live side by side until Phase 6. That doubles some maintenance.
- "Fastest" is a goal, not a fact. Nothing here has been timed against OCCT on this corpus yet;
  Phase 0 records that number.

### What makes this attempt different

- **A corpus with answers.** 382 regression cases, 116 ladder cases, 83 reference tests, 119
  SSI cases, 20 bake-off parts, nearly all with analytic or invariant answers, plus an OCCT
  oracle that scores without being trusted. Fornjot had no such harness.
- **Incremental replacement.** The product ships on every phase. A phase that misses its gate
  stops; nothing already shipped is lost.
- **A complete, readable reference.** SMLib and solidcore are Apache-2.0: a full kernel we can
  read and port, with attribution. Earlier Rust efforts had none they could legally read.
- **Published methods.** Local tolerances, general fuse, procedural intersection curves,
  rolling-ball and setback blends: all documented, and the blend patents found have expired.
- **Agent throughput.** 18 kernel PRs merged in the last week; PRs #189 to #236 were opened in
  about 27 hours. The gates turn that speed into measured progress instead of more special cases.

### Risks

| Risk | Mitigation |
|---|---|
| Special cases keep growing | A special-case module needs a recorded reason; Phase 2 deletes the existing ones as their cases pass. The ladder frontier, not the latest bug, picks the next work. |
| A phase stalls | Each phase is useful alone: Phase 1 already fixes #216 and the MJCF masses. The corpus shows the stall early. |
| An accidental read of OCCT or other LGPL code | The sources table above; reviewers check where an algorithm came from; usd-brep's OCCT converters are named as off limits. |
| Designs change meaning when topology changes | Persistent names follow history, and the naming tests run in every phase. Schema 3 is versioned. |
| The oracle rots once Nightfall is frozen | Pin it; FreeCAD's oracle is a second black box. |
| Polykrom drifts from upstream SolidCraft, which is moving to solidcore | Keep upstream merges outside `crates/kernel` working; schema 3 lets bodies cross between the two kernels if upstream wants it. |
| Agent runs vary | TinyRex is judged on the median of three runs. |

### Effort

From ENGINES.md, in engineering-months with AI assistance:

| Phase | Effort |
|---|---|
| 0 Harness and boundary | 1–2 |
| 1 Geometry and topology | 2–4 |
| 2 Booleans | 4–8 |
| 3 Free-form SSI, sweeps, lofts | 6–12 |
| 4 Blends | 6–12 |
| 5 Offsets and local operations | 6–12 |
| 6 Parity and speed | ongoing |

Roughly 25 to 50 engineering-months to reach OCCT's score on our corpus. Phases 3 to 5 overlap,
so calendar time is shorter than the sum. Matching the commercial kernels on blends, offsets
and free-form corner cases is a multi-year effort, corpus-driven, and this plan does not
claim otherwise.

## Naming

- **Polykrom** is the product: the app, the CLI, the MCP server.
- **PolyCore** is the kernel. Crates are `polycore` and `polycore-*`. Internal names keep the
  rules in AGENTS.md: `solidcraft-*` crates above the kernel are not renamed piecemeal.

## Open decisions

1. **Where PolyCore lives.** In the Polykrom workspace (one CI, corpus next to the code, atomic
   PRs) or in its own repository. Proposed: in the workspace until the API is stable at the end
   of Phase 2.
2. **solidcore.** Treat it as a reference and a parts bin (proposed), or contribute to it and
   adopt it as PolyCore's base, as upstream SolidCraft does.
3. **The OCCT oracle.** A frozen Nightfall `nf` build kept only for scoring (it already reports
   history), or FreeCAD's oracle (already wired, but no history probe).
4. **Model resolution.** An absolute length tolerance for tolerant topology. Proposed 1e-6 mm,
   fixed in Phase 1 after measuring the corpus.
5. **Bake-off targets.** The shares of OCCT's score above are proposals.
6. **Datasets.** ENGINES.md lists ABC and Fusion 360 Gallery, but those models were made in
   commercial CAD, which AGENTS.md rules out as test data. Decide whether they are allowed.
7. **TinyRex in public.** The spec and checks would be committed to `tools/agent-eval`. Confirm
   that is fine, and that the median of three runs is the right rule.
8. **The study sidecar.** Mesher input (Netgen from a surface mesh, or fTetWild), and the
   direct solver off macOS.
9. **Licence of ported files.** Files ported from SMLib or solidcore are Apache-2.0 only inside
   an MIT OR Apache-2.0 workspace. Accept that, or make PolyCore Apache-2.0 only.
10. **Nightfall's freeze.** When to archive the repository, after Phase 0 has moved the study
    runner and the harvest is done.

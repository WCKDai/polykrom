# Engines plan: kernel, verified simulation, rendering

Polykrom's goal is CAD in which people and AI agents build real machines and can prove they work: arms, printers, robot dogs, humanoids, cars, planes, assembly lines (the demonstration ladder, milestone D). That needs three engines at a professional standard: a geometric kernel, verified simulation, and rendering. This is the plan for all three. It is mirrored by milestones K, V and R on GitHub and epics in Plane, and it changes with them.

It rests on four research reports made on 2026-10-25 (kernel state of the art, our own kernel's failures, simulation and verification, rendering). Each claim below is either measured in this repository or sourced; the sources are listed at the end.

## Principles

1. **Reuse first.** If an open-source project meets the standard under a permissive licence (MIT, Apache-2.0, BSD), use it or learn from it before writing our own. Build only what nothing good covers.
2. **Licences.** GPL and LGPL code is never linked into Polykrom. It runs as a separate program (a sidecar, or a CI oracle) or is read as literature. The repository rule stands: OCCT and FreeCAD source is never read, copied or ported (AGENTS.md); they run only as separate programs, for reference values.
3. **Corpus first.** Parasolid's robustness comes from "millions of overnight automatic tests" and "zero tolerance for regressions", not from a secret algorithm: most of the methods are published. Every failure becomes a corpus case before it is fixed, and every change is measured against the whole corpus.
4. **Evidence with every answer.** No agent or person gets a bare number from a check or a solver: it comes with how it was verified, how converged it is, what was assumed, and how far to trust it.
5. **Each demonstration pulls the work.** The ladder's next rung decides what is built next; nothing is built for its own sake.

## Where we stand

### The kernel (measured in this repository)

- Truck gives Polykrom a numeric B-rep with four surface kinds (plane, B-spline, NURBS, revolved curve). Cylinders, cones, spheres and tori are not first-class: they are recovered by sampling normals (`offset::surf_of`).
- Surface-surface intersection starts from a **triangle-triangle collision of the two faces' meshes**, chained into polylines and projected onto both surfaces (`vendor/truck-shapeops/src/transversal/`). It is run on every pair of faces. Tangent, coincident and touching geometry is invisible or ambiguous to it, small features are missed at coarse tolerance, and results change with a shift of the operands.
- The boolean tolerance is a fraction of the part's size (0.2 %, then 0.05 %), not a model resolution. Around it sit a retry ladder (4 shifts × 2 tolerances, tool nudges, refined tolerances; `ops.rs`) and about ten special-case modules (~9.4k lines: coplanar, coaxial, tangent cylinders, touching walls, seams…).
- Fillets are special cases (straight edges between planes, closed chains of smooth edges), not a general rolling-ball blend. Shell and offset only work on analytic faces and can't change topology.
- Measured: the action matrix passes 85.9 % of 35,576 actions; its largest single failure cause is fillets and chamfers whose edge ends meet curved, slanted or several faces (986 failures, #556). Fuzzing 1,000 seeds: 53.8 % of designs clean, 27.1 % fail, 18 % take over 90 s. 10 of the 13 failing seeded goldens are the shell error. The kernel-cases rig (372 cases, branch `kernel-smlib`) records truck at 50/89 booleans, 32/50 fillets, 5/18 shells. With Nightfall's OpenCASCADE behind the hard operations, 92 of 100 fuzzed designs build against 83 on truck alone.
- What's already there to build on: an exact SSI solver for display edges (`tess/ssi.rs`), a 119-case SSI corpus with exact ground truth (branch `kernel-smlib`, `tests/ssi-corpus`), attempt budgets and deadlines (#87, #89), and Nightfall's owned Rust NURBS surfaces.

### Simulation

Static stress runs out of process on Nightfall (Netgen meshing, MFEM solving) and reports convergence across three refinements; a cantilever lands within 2 % of beam theory. Motion studies, interference sweeps and URDF export exist. There is no modal, thermal, contact, nonlinear material, dynamics or flow yet.

### Rendering

A wgpu viewport with a render workspace (`render.*`) and studio scenes. No path tracer, no material standard, no sensor simulation.

## K · The kernel: toward commercial parity

What the commercial kernels do, and all of it is published: **local tolerances** on edges and vertices (Jackson, Parasolid, 1995); an intersection **dispatch table** with closed forms for analytic pairs and subdivision plus marching for NURBS, with normal-cone tests so no small loop is missed; **procedural intersection curves** (the two surfaces plus a fitted spline, refined on demand); a boolean that settles coincidences **lowest dimension first** (vertex-vertex, vertex-edge, edge-edge, then faces: OCCT's documented General Fuse); and **rolling-ball blends** with setback vertex blends. The blend patents found have expired.

The open-source landscape: OCCT is the only open kernel with the whole set (LGPL, so out of process only). Truck (Apache-2.0) is clean but its booleans are mesh-seeded. Fornjot (0BSD) was archived in June 2026 with its goals unreached. The exact-arithmetic Rust kernels (nacre, Catallax, both permissive) stop at quadric-quadric cases; none reaches NURBS. SMLib (Apache-2.0, NVIDIA) is a complete kernel that scored 50.7 against OCCT's 77.4 in Nightfall's bake-off; solidcore is porting it to Rust. Manifold (Apache-2.0) does robust mesh booleans. The production kernels all use tolerant floating point with procedural geometry, so that is the numerics strategy.

| Stage | What | Reuse | Effort (eng.-months, AI-assisted) | Pays off |
|---|---|---|---|---|
| **K0 Harness** | One corpus and dashboard: kernel-cases (372), the SSI corpus (119), fuzz goldens, the action matrix, every failing user part, and permissively licensed models made in open tools. No models made in commercial CAD (AGENTS.md): the Fusion 360 Gallery, ABC and NIST sets are out ([PolyCore RFC](https://github.com/wckdboy/polycore/blob/main/docs/rfcs/0001-polycore.md#decisions), decision 7). OCCT as the out-of-process oracle (a frozen Nightfall `nf kernel serve`, `compare_step.py`). A checker (`checkshape`-like) after every operation in debug builds. A minimiser that shrinks a failing B-rep. Pass rate, p50/p95 time against face count, determinism, zero regressions in CI | OCCT out of process; corpora (permissive, made in open tools) | 1–2 | Everything below is measured by it |
| **K1 Analytic surfaces and tolerances** | Plane, cylinder, cone, sphere, torus, extrusion, revolution, offset and NURBS as first-class surfaces; lines, circles, ellipses and procedural intersection curves; an absolute model resolution with tolerant edges and vertices; exact per-operation history | Nightfall's Rust NURBS (shared crate), truck-geometry evaluation where it passes K0, `robust` and `inari` predicates | 2–4 | Turns intersection inaccuracy into tolerance instead of failure |
| **K2 Analytic booleans** | BVH broad phase over faces; the analytic SSI table (every plane/quadric/torus pair, coaxial and coincident detection); a General-Fuse-style pipeline (interferences lowest dimension first, split, rebuild, select); classification by ray casting with generalised winding numbers as the fallback | `tess/ssi.rs`, the published dispatch table, Spainhour–Weiss winding numbers | 4–8 | The largest single gain: prismatic and turned parts (most mechanical parts) become fast and exact; retires most of the retry ladder and special cases |
| **K3 General SSI** | Subdivision seeding with normal-cone loop tests, adaptive marching, tangent and overlap handling, procedural curves with fitted splines | The two 2026 ACM TOG NURBS SSI papers (benchmarked against Parasolid), Patrikalakis–Maekawa | 6–12 for good | Freeform and imported parts |
| **K4 Rolling-ball fillets** | Constant radius on open chains, curved and slanted ends, three-edge vertex blends, end capping | Published methods (Rossignac–Requicha, Choi–Ju, Várady–Rockwood); SMLib's blending (Apache-2.0) mined with attribution | 6–12 | The action matrix's biggest failure cause (#556, 986) |
| **K5 Local operations** | Offset with topology change, shell, draft, remove/replace face, tweak: one extend–re-intersect–re-topologise engine | SMLib's offset and shell (Apache-2.0); Rossignac–Requicha offsets | 6–12 | 10 of 13 failing goldens; direct modelling |
| **K6 Advanced blends** | Variable radius, setbacks, n-sided vertex blends, cliff edges, rollover and overflow (Braid 1997) | Literature; a freedom-to-operate check on setback blends first | multi-year | Parity on blends |
| **K7 Healing and import** | Sewing, small-feature removal, tolerant STEP import measured on our own and permissively licensed STEP files made in open tools (not ABC or NIST: made in commercial CAD) | Published methods | ongoing | Interoperability |

**Measured so far (K0, first step, #123):** truck on main passes **288 of 372** regression cases ([kernel-cases.md](kernel-cases.md)) and **102 of 116** ladder cases ([kernel-ladder.md](kernel-ladder.md), timing golden recorded). By area: sheet metal 57/57, fillets 38/50 (9 unsupported), chamfers and primitives all, booleans 64/89, sweeps 62/83, threads 21/28, shell 9/18. Ten cases hang past 180 s. SMLib's last run on the same cases was 317/372. The weakest areas (shell, booleans with coincident or tangent faces, sweeps) are where K1–K2 start.

Truck's NURBS evaluation, tessellation and STEP I/O stay behind traits while they pass K0; truck-shapeops (and probably truck-topology, which lacks local tolerances, history and p-curves) is replaced in K1–K2. A one-week profile of five failing parts opens K2, so it starts from measured hotspots. Honest scale: K2 and K4 can beat truck by a wide margin within a year; matching Parasolid on blends, offsets and freeform corner cases is a multi-year, corpus-driven effort.

## V · Verified simulation (with Nightfall)

Every result carries an **evidence contract**:
- identity (study, geometry and solver hashes);
- the model's assumptions (material law and source, linear or not, how supports and contacts are idealised, what was defeatured);
- code verification for that capability on that build (or "unverified capability");
- solution verification (three refinements, observed order, Richardson value, GCI, error estimate);
- sanity checks (reactions against loads, energy balance);
- singularity flags (re-entrant corners, point loads);
- input uncertainty;
- a trust level (high, moderate, low, unverified) with the actions that would raise it.

An agent may never report a quantity without its error estimate. The practice follows ASME V&V 10 and V&V 20, NAFEMS benchmarks, the method of manufactured solutions, and NASA-STD-7009A's credibility factors.

| Stage | What | Engines (licence) | Effort | Pulls |
|---|---|---|---|---|
| **S0** | The evidence contract for every study; the analytic-case library; NAFEMS LE1, LE10, FV32; manufactured solutions; CI gating | Rust orchestration | 2–3 wk | everything |
| **S1** | Modal (free and prestressed), linear buckling, steady and transient thermal, thermal stress | MFEM (BSD-3) | 4–6 wk | brackets, arms, motor heat |
| **S2** | Assemblies to MJCF, URDF, SDFormat and UsdPhysics with exact mass, centre of mass and inertia from the B-rep, joints from mates, collision proxies, actuators; a MuJoCo runner; Drake as cross-check; joint loads back into FEM | MuJoCo (Apache-2.0), Drake (BSD-3) | 5–8 wk | **D3 robot dog**, D4 |
| **S3** | Bonded assemblies; then a time-boxed bake-off for frictional contact and plasticity: MFEM with Tribol (MIT) against MOOSE (LGPL, out of process), judged on NAFEMS cases and CalculiX/Code_Aster (GPL, CI only) | MFEM, Tribol, MOOSE | 3–4 wk + 2–3 wk + 8–14 wk | assemblies, aluminium |
| **S4** | Printed parts: orthotropic PLA with the build direction from CAD, Tsai-Wu or Hill failure, strength knockdown with uncertainty; TPU as Neo-Hookean | MFEM | 3–5 wk | **D2 printer**, every printed part |
| **S5** | Quick aero: vortex-lattice and lifting line in Rust, airfoil polars, stability derivatives, cooling correlations | Rust; NeuralFoil and AeroSandbox (MIT) | 3–5 wk | **D6 plane** |
| **S6** | Incompressible RANS and conjugate heat transfer with automatic boundary-layer meshing | SU2 (LGPL, out of process) | 10–16 wk | aero, cooling |
| **S7** | Print simulation: layer activation, thermal history, warpage | MFEM | 8–12 wk, research | the loop's "make" stage |
| **S8** | Flexible bodies (modal reduction into MuJoCo or Chrono), vehicles on terrain | Project Chrono (BSD-3) | 6–10 wk | **D5 car** |

Meshing stays on Netgen (LGPL) from the exact B-rep, with fTetWild (MPL-2.0) as the fallback on a tessellation and MMG (LGPL) for adaptation. Gmsh (GPL) and TetGen (AGPL) are out of process only. Quadratic tetrahedra by default; thin walls get at least two elements through the thickness, or a midsurface shell, and the choice is recorded. FEM GPU paths are CUDA or HIP, so large jobs go to a remote Linux runner; the out-of-process design already allows that. Each engine is a separate executable with a plain-C or file protocol and a watchdog (Nightfall RFC 0003); each needs a Nightfall RFC before it is adopted.

## R · Rendering

One material model, a subset of **OpenPBR** (Apache-2.0, ASWF), exported to glTF `KHR_materials_*` and MaterialX. It is seeded from physicallybased.info and Poly Haven/ambientCG HDRIs (all CC0), and carries physical data (density, near-infrared reflectance) so one material drives mass, renders and sensors.

| Stage | What | Reuse (licence) | Effort |
|---|---|---|---|
| **R0** | Material model, presets for engineering materials (anodised aluminium, PLA and PETG with layer lines, steel, brass, rubber), glTF export, HDRI picker | OpenPBR, MaterialX (Apache-2.0), CC0 data | 2–3 wk |
| **R1** | Viewport PBR: image-based lighting, shadows, SSAO, TAA, exposure | Filament's PBR maths (Apache-2.0, reference); Bevy's WGSL shaders (MIT/Apache) | 3–5 wk |
| **R2** | A progressive path tracer in WGSL on wgpu, native and web, with a Rust BVH, AOVs and a denoiser; validated against Mitsuba 3 and pbrt-v4 renders | pbrt-v4 (Apache-2.0) and Mitsuba 3 (BSD) as references | 8–12 wk |
| **R3** | `render` jobs for agents: a quality target (noise threshold, samples, time), stats with a two-buffer noise estimate, FLIP comparison against goldens | NVIDIA FLIP (BSD-3, `nv-flip` crate) | 2–3 wk |
| **R4** | Final-frame renders, turntables and animations through Cycles as a sidecar process, with Open Image Denoise | Cycles (Apache-2.0), OIDN (Apache-2.0) | 6–10 wk |
| **R5** | Hardware ray queries where wgpu offers them | wgpu (MIT/Apache) | 3–4 wk |
| **R6** | Sensors for robotics: camera noise and lens models, depth, lidar; exported with the robot | Gazebo sensor models, MuJoCo (Apache-2.0) | 8–12 wk |

Until R4, Blender run as a separate program (`blender -b`, GPL) can make showcase stills, never bundled.

## Order of work

The ladder decides. Now: **D2 printer** (printability shipped, #122; orthotropic PLA is S4). Next: **D3 robot dog**, which needs S2 (robot export with exact inertia, MuJoCo) and actuator ratings (the parts library already carries them). In parallel, the foundations every rung needs: **K0** (the harness), then **K1–K2** (analytic booleans), **S0** (the evidence contract) and **R0** (materials).

## Compute

These would speed the work most:
- A Linux x86-64 machine with an NVIDIA GPU as a standing runner. It would run the K0 corpus against OCCT and the fuzz suite nightly, the CUDA paths of MFEM and LBM, Cycles with OptiX, and MuJoCo Warp.
- More CI capacity: the engine job takes about 33 minutes.
- A budget for agent runs: each demonstration build costs $5–20 and each evaluation round about $2–10.

## Sources

**Kernel**
- Jackson, local tolerances (1995): https://ftp1.cs.wisc.edu/pub/users/prem/jackson-SM-95.pdf
- OCCT Boolean operations specification: https://occt3d.com/dev/doc/overview/html/specification__boolean_operations.html
- OCCT test system: https://occt3d.com/dev/doc/overview/html/occt_contribution__tests.html
- Patrikalakis, Maekawa & Cho, *Shape Interrogation for CAD/CAM*: https://web.mit.edu/hyperbook/Patrikalakis-Maekawa-Cho/
- Krishnan & Manocha (1997): https://www.academia.edu/56030446/An_efficient_surface_intersection_algorithm_based_on_lower_dimensional_formulation
- 2026 NURBS SSI papers: https://dl.acm.org/doi/10.1145/3806045 and https://dl.acm.org/doi/10.1145/3807948
- Generalised winding numbers for trimmed NURBS: https://arxiv.org/abs/2504.11435
- Rossignac & Requicha, offsetting: https://faculty.cc.gatech.edu/~jarek/papers/Offsets.pdf
- Parasolid testing: https://blogs.sw.siemens.com/plm-components/parasolid-delivers-30-years-of-quality-in-industrial-use/
- Kernels: https://github.com/ricosjp/truck, https://github.com/hannobraun/fornjot, https://github.com/elgar328/nacre, https://github.com/bherbruck/solidcore/wiki, https://github.com/elalish/manifold/wiki/Manifold-Library, https://develop3d.com/cad/nvidia-launches-usd-brep-geometry-kernel-for-solid-modelling/
- Predicates: https://docs.rs/robust, https://docs.rs/inari
- Datasets (not used: made in commercial CAD, which AGENTS.md rules out; [PolyCore RFC](https://github.com/wckdboy/polycore/blob/main/docs/rfcs/0001-polycore.md#decisions), decision 7): https://cims.nyu.edu/gcl/papers/2019-ABC-Dataset.pdf, https://www.research.autodesk.com/publications/fusion-360-gallery/, https://www.nist.gov/el/systems-integration-division-73400/mbe-pmi-validation-and-conformance-testing/download-free-cad

**Simulation**
- MFEM: https://mfem.org/features
- Tribol: https://github.com/LLNL/Tribol
- Licences: https://www.cgal.org/license.html, https://www.wias-berlin.de/software/tetgen/FAQ-license.jsp
- Meshing: fTetWild https://arxiv.org/abs/1908.03581; ParMmg https://arxiv.org/pdf/2201.02806
- MuJoCo: https://mujoco.readthedocs.io/en/latest/computation/
- Drake: https://drake.mit.edu/doxygen_cxx/group__compliant__contact.html
- Chrono: https://projectchrono.org
- Contact-model comparison: https://arxiv.org/abs/2110.00541
- NeuralFoil: https://arxiv.org/abs/2503.16323
- AeroSandbox: https://aerosandbox.readthedocs.io
- SU2: https://en.wikipedia.org/wiki/SU2_code
- XLB: https://xlb.readthedocs.io
- V&V standards:
  - ASME V&V 10: https://www.asme.org/codes-standards/find-codes-standards/standard-for-verification-and-validation-in-computational-solid-mechanics
  - Grid convergence index: https://qa12.asme.org/wwwasmeorg/media/ResourceFiles/Shop/Journals/JFENumAccuracy.pdf
  - Manufactured solutions: https://www.osti.gov/biblio/759450
  - NAFEMS benchmarks: https://www.nafems.org/publications/resource_center/p18/
  - NASA-STD-7009A: https://ntrs.nasa.gov/citations/20160011121

**Rendering**
- Cycles: https://www.cycles-renderer.org/, https://developer.blender.org/docs/features/cycles/standalone/
- Path tracers: https://github.com/mitsuba-renderer/mitsuba3, https://github.com/mmp/pbrt-v4
- Open Image Denoise: https://github.com/OpenImageDenoise/oidn
- wgpu ray tracing: https://wgpu.rs/doc/wgpu/documentation/extensions/ray_tracing/index.html
- Bevy: https://bevy.org/news/bevy-0-20/
- Filament: https://google.github.io/filament/
- Material standards: https://github.com/KhronosGroup/glTF/blob/main/extensions/README.md, https://github.com/AcademySoftwareFoundation/OpenPBR, https://github.com/AcademySoftwareFoundation/MaterialX
- CC0 data: https://80.lv/articles/physically-based-a-database-of-pbr-values-for-real-world-materials/, https://polyhaven.com/license
- FLIP: https://github.com/NVlabs/flip

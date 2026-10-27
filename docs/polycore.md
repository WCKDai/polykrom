# PolyCore

Polykrom's geometric kernel is becoming **PolyCore**: an open-source kernel in Rust whose bar is
OpenCASCADE and whose goal is to rival the commercial kernels. It lives in its own repository,
[WCKDai/polycore](https://github.com/WCKDai/polycore) (Apache-2.0). Polykrom is its only
consumer and stays MIT OR Apache-2.0.

The design is [RFC 0001](https://github.com/WCKDai/polycore/blob/main/docs/rfcs/0001-polycore.md),
accepted on 2026-10-26. In short:

- **Phase 0** moves `crates/kernel`, `crates/geom`, the vendored truck crates and
  `tests/kernel-cases` into PolyCore. Polykrom then depends on PolyCore by a pinned git revision;
  for work on both, an untracked `[patch."https://github.com/WCKDai/polycore"]` points at a local
  checkout.
- **CI:** PolyCore runs the kernel cases and the ladder through a pinned `polykrom-cli`; Polykrom's
  CI tests the pinned PolyCore revision.
- **Phases 1–6** replace truck layer by layer (geometry and topology, booleans, free-form
  intersections, blends, offsets, parity), each gated on no regression in the kernel cases, the
  ladder and the reference tests; determinism; a bake-off score against OCCT run as a separate
  program; the printer and the dog; and [TinyRex](https://github.com/wckdboy/tinyrex) rebuilt from
  scratch by an agent through Polykrom's own tools.
- **Studies** stay in Polykrom: Rust-native P2 FEA with faer, fTetWild as a separate program.
  PolyCore supplies the analysis-ready geometry.
- **Test models:** our own cases, plus permissively licensed models made in open tools. No models
  made in commercial CAD.

The RFC says how the code gets through the kernel stages K0–K7 in [ENGINES.md](ENGINES.md).

## Current wiring

`Cargo.toml` pins `solidcraft-kernel` and `solidcraft-geom` to one PolyCore revision. Both entries
must name the same revision: the kernel's public types are the geometry crate's types.

- Polykrom no longer patches registry Truck crates. PolyCore carries the patched closure as
  `polycore-truck-*` packages, so there is one Truck closure in the graph.
- The old `crates/kernel` and `crates/geom` directories are excluded from the workspace while their
  remaining work moves. They are not built, tested or linted; their manifests are frozen copies.
  Delete them in the same change that moves the untracked `crates/kernel/examples/` spike.
- The Polykrom workspace tests no longer include the kernel's unit and integration tests. Those run
  in PolyCore's CI at the pinned revision; Polykrom's CI must keep that revision green.
- A local PolyCore checkout is wired in with an untracked patch, never committed:

  ```toml
  [patch."https://github.com/WCKDai/polycore"]
  solidcraft-kernel = { path = "../polycore/crates/polycore" }
  solidcraft-geom = { path = "../polycore/crates/solidcraft-geom" }
  ```

Remaining follow-ups after this dependency switch:

1. Move the last kernel work (including `crates/kernel/examples/nf_spike.rs`) to PolyCore, then
   delete `crates/kernel` and `crates/geom`.
2. Delete the now-unused `vendor/truck-*` and `vendor/matext4cgmath` copies and update
   `vendor/README.md`, `NOTICE` and `ATTRIBUTION.md`; keep only `vendor/egui-wgpu`.
3. Add a Polykrom CI job that checks out PolyCore at the pinned revision and runs its tests, so the
   kernel tests are gated by both repositories.
4. Update the remaining `docs/`, `scripts/ci/` and `.github/workflows/ci.yml` references to the
   old `crates/kernel` paths when the frozen copies go.

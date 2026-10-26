# PolyCore

Polykrom's geometric kernel is becoming **PolyCore**: an open-source kernel in Rust whose bar is
OpenCASCADE and whose goal is to rival the commercial kernels. It lives in its own repository,
[wckdboy/polycore](https://github.com/wckdboy/polycore) (Apache-2.0). Polykrom is its only
consumer and stays MIT OR Apache-2.0.

The design is [RFC 0001](https://github.com/wckdboy/polycore/blob/main/docs/rfcs/0001-polycore.md),
accepted on 2026-10-26. In short:

- **Phase 0** moves `crates/kernel`, `crates/geom`, the vendored truck crates and
  `tests/kernel-cases` into PolyCore. Polykrom then depends on PolyCore by a pinned git revision;
  for work on both, an untracked `[patch."https://github.com/wckdboy/polycore"]` points at a local
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

The kernel stages K0–K7 in [ENGINES.md](ENGINES.md) are unchanged; the RFC says how the code
gets there.

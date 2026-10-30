# Dependency advisory decisions — 2026-10-30

The repository's OSV scan is enforced by [`osv-scanner.toml`](../../osv-scanner.toml). Four
advisories were fixed in the dependency patch: `rustls-webpki`, `webpki-root-certs`, `tokio`
and `zip`. The entries below are **explicit, time-boxed acceptances**, not a claim that the
dependency surface is clean.

| Advisory | Package | Decision | Why / next action |
|---|---|---|---|
| GHSA-qwgh-2vcv-g2f7 | block-buffer 0.10.4 | Accept to 2027-01-31 | Exploit needs a caught panic in compression; fixed 0.12 needs a digest/sha2/zbus major transition. |
| RUSTSEC-2026-0196 | cgmath 0.18 | Accept to 2027-01-31 | Unmaintained upstream geometry dependency; replacement is a PolyCore migration. |
| RUSTSEC-2026-0197 | cgmath 0.18 | Accept to 2027-01-31 | `swap_columns` UB requires identical indices; the API is not called in Polykrom or pinned PolyCore. |
| RUSTSEC-2024-0370 | proc-macro-error 1.0.4 | Accept to 2027-01-31 | Build-time proc-macro, no runtime code; comes from the pinned Truck derivation chain. |
| RUSTSEC-2026-0192 | ttf-parser 0.25.1 | Accept to 2027-01-31 | Unmaintained; font input a hostile surface. Keep caps; review successor. |
| RUSTSEC-2026-0371 | zbus_names 3.0.0 / 4.3.4 | Accept to 2027-01-31 | Linux D-Bus/keyring/accesskit; no fixed release. |
| RUSTSEC-2026-0370 | zvariant 4.2.0 / 5.15.0 | Accept to 2027-01-31 | Same stack; no fixed release. |
| RUSTSEC-2026-0372 | zvariant_derive 4.2.0 / 5.15.0 | Accept to 2027-01-31 | Same stack, proc-macro half. |

Owner decision: the task on 2026-10-30 explicitly authorised resolving or explicitly accepting
the remaining advisories. Review the acceptances by **2027-01-31**, or earlier if a fixed release
or upstream replacement lands. A changed finding must fail the OSV gate instead of silently
inheriting an old exception.

# Compatibility with earlier names

Polykrom is a fork of [SolidCraft](https://github.com/tangency-labs/solidcraft), which was
first called SolveCraft. Before its first stable release, 0.21.0, Polykrom had no users, so on
2026-11-07 the owner decided to rename everything to Polykrom at once, without migration shims
for installs that never existed. What remains of the earlier names is what lets Polykrom **open
the files** those programs wrote. Nothing else does: there is no old-name program, package,
folder, variable, key or update feed to keep working.

## What is kept: files written by SolidCraft and by earlier Polykrom builds

| File | How it opens | Where |
|---|---|---|
| `.solidcraft` and `.solvecraft` designs | Recognised as designs everywhere a `.polykrom` file is: the Open dialog and its filters, drag-and-drop, recent files, `doc.open`, the MCP `open` tool, `polykrom-cli eval` and `run`, and the browser app's file picker. Saved again as `.polykrom` (Save As). | `crates/io/src/lib.rs` (`DESIGN_EXTS`, `Format::from_name`); the desktop and browser pickers list the three extensions |
| `"format": "solidcraft/N"` and `"format": "solvecraft/N"` tags | Read like `polykrom/N` and upgraded through the same format steps; the design is re-tagged `polykrom/N` when saved. Not backward-compatible: SolidCraft and Polykrom builds before 0.21.0 refuse a design saved with the new tag (format 36). | `crates/doc/src/format.rs` (`LEGACY_FORMAT_PREFIXES`, `version_of`); `crates/io/tests/fixtures/legacy-solvecraft-v1.solidcraft` |
| Draft design containers marked `solvecraft-container/1` | Accepted next to the current container marker; only the current marker is written | `crates/io/src/design.rs` (`LEGACY_VERSION`) |
| Keyboard-shortcut files exported as `solvecraft-shortcuts/1` | Import like the current shortcut files | `crates/ui-egui/src/toolbar_custom.rs` |

Format markers inside files (`…-container/1`, `…-shortcuts/1`, `…-library/1`, `…-theme/1`,
`…-crash/1`, the 3MF face-tag namespace in `docs/formats/3mf-face-tags.md`) are identifiers, not
branding: a reader accepts every marker it ever wrote, and a renamed marker adds a row above.

Tests: `crates/io` (`Format::from_name`, `is_design_path`, the legacy fixture),
`legacy_extension_and_format_tag_still_open` (CLI, `run_results.rs`),
`legacy_design_files_and_resource_uris` (MCP `open`; it also proves the old resource URIs are refused),
`draft_container_marker_remains_readable`. A change to one of these shims updates this file in
the same PR.

## What is gone

Everything that only existed so that an existing SolidCraft or SolveCraft install would keep
working. None of it applies to Polykrom, whose first release already carries the new names:

- old-name programs: the `solidcraft`, `solidcraft-cli` and `solvecraft-cli` aliases. The
  programs are `polykrom` and `polykrom-cli`, and every launcher, file association, Snap and
  Flatpak command, MCP registration (`claude mcp add polykrom -- polykrom-cli mcp`) and script
  names them;
- old-name release files: the `solidcraft-…` copies of the version-less installers and the
  `solidcraft-…` / `solvecraft-…` `.zsync` copies. A release attaches `polykrom-<version>-<platform>.*`,
  `Polykrom-<version>-windows-x64.exe` and the version-less `polykrom-<platform>.*` copies only;
- settings, preferences, autosave, recovery and thumbnail folders: Polykrom reads and writes its
  own folders and never looks in a `solidcraft` or `SolveCraft` one;
- keychain entries: the agent-chat keys live under Polykrom's own service name;
- environment variables: `POLYKROM_*` only (`polykrom_doc::env`, re-exported as
  `polykrom_engine::env`), with no `SOLIDCRAFT_*` or `SOLVECRAFT_*` fallback;
- MCP resource URIs: `polykrom://design` and `polykrom://commands` only; `solidcraft://…` and
  `solvecraft://…` are refused as unknown resources (`crates/mcp/src/server.rs`);
- package identity: the app ID is `app.polykrom.Polykrom` (macOS `CFBundleIdentifier`, Linux
  desktop entry, AppStream and Flatpak ID, D-Bus and Wayland `app_id`), the bundle is
  `Polykrom.app`, and the deb, rpm and Snap packages are `polykrom`. There are no end-of-life
  rebases, `<replaces>`, `X-Flatpak-RenamedFrom` entries or old-name `.flatpakrepo` files for
  earlier IDs, and no old-name Pages or Flatpak addresses.

A SolidCraft install on the same machine is a different program: it keeps its own settings and
is neither replaced nor read. Open its designs in Polykrom with File ▸ Open.

## Where the old names may still appear

`git grep -i solvecraft` and `git grep -i solidcraft` list only:

- the file-reading shims in the table above, their tests and fixtures, and the format markers;
- the SolidCraft credit: README, CONTRIBUTING, AGENTS.md, ATTRIBUTION.md, LICENSE and NOTICE
  files, THIRD-PARTY-LICENSES.txt, Help ▸ About, and links to tangency-labs/solidcraft;
- historical changelog and release notes of earlier versions, and upstream-merge notes;
- this file.

Anything else is a leftover of the rename: rename it rather than adding a shim.

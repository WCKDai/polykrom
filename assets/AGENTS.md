# Resource work — mandatory

**Authority (owner decision 2026-11-02):** `design-kit/2026-11-02/` is the authoritative source
for icons, logo and brand; read its `AGENT_BRIEF.md`. The kit is kept whole (including its PNG
previews, layer data, fonts, licences and `SHA256SUMS.txt`) as one checksum-verified provenance
snapshot; verify it with `shasum -a 256 -c SHA256SUMS.txt` inside that folder. Replace it only with
a newer complete snapshot from the owner, never by editing files in place.

Read `assets/README.md` before changing media, fonts, artwork or their consumers.
For **any UI icon, icon assignment or review-tool change**, also read
`assets/icons/ui/AGENTS.md` and `assets/icons/ui/STYLE_GUIDE.md` before editing.
These files are the specification; chat history and old previews are not required.

- Keep one editable authority per resource. UI SVGs are masters, not exports of a
  second competing drawing library. `source/` retains genuine native source and
  the separately compiled Rust viewport glyphs.
- Keep only production inputs, necessary shipped exports, authoring tools and
  provenance here. Put screenshots, contact sheets, audits, caches, test output,
  review inbox files and package staging under ignored `target/`.
- Do not add historical alternatives, duplicate resolutions that packaging does
  not consume, copied screenshots, `node_modules`, Python bytecode or generated
  inventory CSVs to this collection. Preserve native editable source documents.
- Add an exact `ATTRIBUTION.md` row for every non-Markdown asset, including tool
  code, web UI sources and metadata. Remove rows for intentionally retired files.
- Never copy or derive iconography from commercial CAD products. Use the design kit's exact
  vector assets; Facet (`icons/ui/design/tools/author/`) is no longer the icon authority.
- Run `cargo xtask assets` and `cargo xtask icons`. Full repository gates still
  apply before a commit. Never refresh acceptance hashes to bypass a failed check.

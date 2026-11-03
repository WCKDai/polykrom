# Polykrom · Agent handoff

Snapshot: 2026-11-02. Made by WCKD.

Start with **AGENT_BRIEF.md**, then open **index.html** for a local visual catalogue. **VIEWS.md** links each current screen to its preview, vectors, layer data and original Figma node.

## Contents

- 5 current UI views: SVG + PNG + layer JSON.
- 70 individual components, including the supplied logo: SVG + PNG + layer JSON.
- Brand/overview/library boards where present.
- Declared tokens, observed current palette and typography, styles, fonts and licenses.
- Prototype entry points and per-node actions in prototypes/interactions.json.
- Complete export inventory in manifest.json; original page inventory in source-pages.json.

## Using the files

The SVGs retain editable vector geometry and live text. Install the bundled fonts when opening text-bearing SVGs. The PNGs provide portable rendered previews. The editable Figma sources are linked in the manifest. This package supplies design references and implementation data; it does not contain application source code or a native .fig document.

Run `python verify_package.py` to check the included SHA-256 file list after extraction. Keep SVG IDs/gradient definitions intact when integrating icons. See licenses/ for applicable third-party notices.

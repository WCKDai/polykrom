# Agent implementation brief

Use the current views as the visual reference. Preserve useful CAD/BIM workflows, information density, alignment and hierarchy. Reuse the existing application architecture and connect UI actions to real model commands. A visible tool is not proof that its backend already exists.

Brand rules: dark graphite shell; supplied transparent logo alone in the document toolbar; one discreet bottom-right credit. Keep kernel/parametric branding out of application chrome. Use the exact vector assets instead of redrawing logos from screenshots.

Native AI belongs in the modelling workflow. Keep scope visible, preview proposed changes, require an explicit Apply action, and expose Undo. Preserve blocked, pending and applied states where supplied. Avoid replacing this workflow with a generic chat overlay.

SVGs are the geometry source. Layer JSON includes node IDs, parents, bounds, text/font information, paint references, auto-layout and prototype actions. Fill/stroke numbers index that file's paints array. These are implementation snapshots; the linked Figma document remains the editable design source. Text in SVGs requires the bundled fonts for correct rendering. PNG previews are rendered from the exported SVGs.

Match the current views before generalizing components. Check primary toolbar actions, contextual mode changes, selection, properties, browser rows, drawings, empty states, dialogs, validation and keyboard focus. Prototype links demonstrate selected flows and do not define every tool's production behavior.

Polykrom: mechanical CAD, with parts, sketches, features and model history. Lime/green/mint gradient accents identify profiles, actions or control points; retain neutral geometry. The current 69-icon set is authoritative, including mirrored Undo/Redo, full-outline Rectangle/Circle, revised Spline, and the latest Shell. Keep the white logo background absent. Footer: "Polykrom by WCKD".

The five-state example uses one actuator mount: 120 × 80 × 8 mm base, Ø44 boss, Ø24 bore, four Ø8 mounting holes with 84 × 44 mm pitch, and R2 finishing fillet. Manual and AI edits preview Ø10 holes while preserving pitch, then apply the same model command. Use these states to implement one coherent flow rather than unrelated screenshots.

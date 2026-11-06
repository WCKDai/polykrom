# A tour of the window

![The Polykrom window](../generated/sample-plate.png)

- **Title bar.** On Windows and Linux the application bar is the window's title bar, a slim 36 pt
  strip. On the left: the Polykrom logo, which opens the application menu (the start page, New,
  Open, Save, Save As, Export…, Workspaces, Preferences, Keyboard Shortcuts, Help, Quit and the
  version), then the open design's name with where it is stored and its revision on the same
  line. On the right: Undo, Redo, command search (S), Save and Export (every export format), then
  the window's buttons.
- **Workspace tabs and ribbon.** The tabs (Model, Surface, Assemble, Insert, Inspect, Manage, View,
  Sheet Metal and Render) hold groups of commands; the active tab is underlined in the accent.
  Sketch appears while editing a sketch; Sheet Metal is always there. Both select automatically when
  entered, and the other tabs remain available. Render enters the Render workspace and any design
  tab leaves it; the application menu's Workspaces… lists every workspace, including the planned
  ones. Each group shows its pinned commands as large buttons and the group's other commands as two
  rows of small buttons, as many as the ribbon's width holds (the rest wait in the group's
  drop-down); the groups share the ribbon's full width, so no room is left empty at its right.
  Click the group's name for all of them. Drag a command from the dropdown
  onto a group's label or empty space to append it; the insertion line shows its destination. Drop
  onto an existing button to insert before it. Buttons can also move between groups, and the
  layout is saved.
- **Canvas.** The 3D view is the stage: it fills the window's whole width and all the height
  between the chrome above (title bar, tabs and ribbon, AI prompt row) and the status bar. The
  Design Browser, the document strip, the history and the other panels are glass cards floating
  over it, inset from its edges: they shield the clicks under them but never move the model's
  projection, and the model reads through their tint.
- **Design Browser** (a glass card at the left, full height down to the history). Its header
  names the design and counts its components and bodies; its chevron folds the card to a slim
  rail, and the rail opens it again. **Filter features…** keeps only the rows whose names contain the text (bodies,
  sketches, construction geometry and features, with their folders open). Below: document settings
  and units, named views, the origin, bodies, sketches, construction planes, canvases, joints and
  components, the folders with their counts. The eyes line up at the right: one shows or hides an
  item; a component's radio button makes it the active component, and right-clicking any row gives
  that item's menu. Rows drag into groups and other components.
- **Document tabs and view bar** (one slim strip floating at the top of the canvas, right of the
  Design Browser). One tab per open design (each with its own undo history, selection and camera;
  a dot marks unsaved changes), × closes it and + opens the start page for a new one. Beside them
  the view orientation (Home, Isometric, Top…), the display style (shaded with edges, shaded,
  wireframe, hidden edges) and the selection filter. In the Compact layout they are two bands
  above the viewport instead.
- **Viewport.** Click to select, Shift or Ctrl to add (Preferences › Selection), drag for a window or crossing box. While sketching, Ctrl/Cmd or Alt/Option suppress auto-snap by default; Shift prefers a midpoint. To
  reach something under something else (two lines drawn on top of each other, coincident points,
  an edge in front of a face, overlapping bodies), hold the left button still for half a second
  (long-press on a touch screen) or right-click › **Select Other…**: a list of everything under
  the cursor opens there, the sketch being edited first. Hovering a row lights it up; clicking it
  selects it (Shift or Ctrl adds), and in a command's dialog it goes to the active selection box.
  Esc or a click elsewhere closes the list. Clicking again on the same spot, with nothing changed
  in between, selects the next thing in that list. The
  wheel zooms at the cursor, the middle button pans and right-drag orbits freely (the mouse
  preset in Preferences switches to SolidWorks, Inventor, Alias, Blender, Tinkercad, OpenSCAD or Touchpad style). On a
  trackpad, pinch zooms and two-finger scroll pans (Navigation › Two-finger scroll pans;
  off: scroll zooms like the wheel); mouse wheels zoom and the thumb wheel pans sideways.
  With the **Fusion** preset and two-finger pan enabled, hold **Shift** while scrolling with
  two fingers to orbit freely instead of panning. No click is needed. Releasing Shift returns
  to panning, even during the same gesture. Shift does not snap this gesture to standard views;
  orbit gestures do nothing on flat 2D pages. Other presets keep their existing scroll behaviour.
  Shift+middle orbits freely
  in the default and Inventor presets. In every preset, Ctrl+left drag (Cmd on macOS) that starts
  on empty space orbits; a Ctrl+click still adds to the selection. Shift+right-drag snaps to the nearest standard or
  diagonal view; Shift does not request snapping when the preset already requires it to orbit.

  Enable **Preferences → Navigation → Free orbit** to rotate the mouse and 3D mouse continuously
  through both poles, preserving roll without axis snapping. It defaults to off for every mouse
  preset. With SolidWorks navigation, right-click a planar face and choose **Normal to Selection**
  to use Look At (also Ctrl+8); repeat to flip the side, preserving scale and projection.

  **Mouse gestures** (Preferences › Navigation, off by default) work with the SolidWorks mouse preset
  only (the wheel orbits, Ctrl + wheel pans) and take over the right button, which then no longer
  orbits: hold it and move the pointer 4 pt to open a ring of tools (280 pt inside, 350 pt outside), even
  while a tool is active, and touch a tool's zone to take it. A right-click that does not move still
  opens the usual context menu. Each studio has its own
  tools, and **Customize…** sets how many tools a studio's ring holds and in which place. Zones
  alternate between two tints, three when the number of tools is odd. The optional radial context
  menu is a separate setting, and gestures take the right button from it while they are on.

  With **Touchpad** selected in Preferences › Navigation, hold **Shift** and move the pointer to
  pan, or hold **Alt** (**Option** on macOS) and move it to orbit—no mouse button is needed.
  Vertical two-finger scrolling zooms in this preset, regardless of the saved two-finger pan
  setting; switching presets restores that setting. Release the modifier to stop navigating.
  Shift+click and Shift+left-drag still select. Combined Shift+Alt or Ctrl/Cmd modifiers do not
  start pointer-only navigation.
  The axis view shows the world's X/Y/Z directions in Polykrom's colors. Click a signed
  axis tip to look along that axis; click the aligned tip again to look from the opposite side.
  Drag the axis view to orbit around the view centre, with Shift for snapping. Top and bottom
  keep the nearest quarter-turn heading. The house, the navigation bar's Home button and a middle-button double-click in the view go
  home, framing what is shown now (Navigation › **Middle double-click** can make it Fit, which
  keeps the orientation, or turn it off).
  Right-click for the marking menu: eight commands around the cursor, plus a
  menu for whatever is under it. Near the window's bottom or right edge a menu opens above or
  to the left of the cursor, so it stays in view.
  Shaded views omit certified smooth joins between spline spans on extruded
  walls. Real creases, cap outlines and authored face splits remain visible.
  Wireframe retains its edge presentation, and hiding a join does not remove its CAD
  identity or change the exported solid. Unsupported or uncertain wall geometry
  keeps its existing edge presentation.
- **Properties** (right). With one feature selected in the timeline, the panel shows its kind
  and name, its parameters (diameter, distance, radius… with the extent, instances, driving
  sketch and body), **Edit feature**, **Edit sketch** when a sketch drives it, and the features
  that depend on it (click one to select it). **Edit feature** turns the panel into the editor
  in place: the values become fields, **Preview** (on by default) shows the change in the
  viewport, a summary says what the model will do (which features rebuild, a pattern keeping its
  pitch), and **Apply change** runs the same command the feature's dialog runs, as one undo
  step; **Cancel edit** or Esc leaves the design as it was. A value that is not a length, angle or
  count shows its error under the field and disables Apply. Features whose dialog has picks or
  choices only (a mirror, a thread, an extrude ending at an object) open that dialog instead, and
  the panel says so. With nothing selected the panel shows the design's properties and **Edit
  properties** opens the Design Properties window.
- **Dialogs** dock on the right with a live preview. Most features also show drag handles and a
  value box in the viewport.
- **AI prompt row** (under the ribbon, with AI features on). Type a request such as "Increase the
  mounting holes to Ø10. Keep the 84 × 44 pattern." (Ctrl+J / Cmd+J puts the keyboard there), check
  the **Scope** chip (the selected feature or bodies, with a hole pattern's count; its menu picks
  another feature, a body or the whole design) and press Enter or **Preview changes**. The button
  reads **Preview pending** while the agent works and **Preview ready** once its change is in; when
  the agent cannot act (no agent set up, a sketch being edited, the agent busy or signed out) the
  button is greyed and the reason stands beside it. The change opens as a **Review changes** card
  where Properties stand: the command, its target, each changed value as old → new with its unit
  (lengths in the design's units, angles in degrees, counts bare), the values it keeps (a hole
  pattern's extent and pitch), the features touched and the dependent features that rebuild. The
  viewport shows the proposed geometry and the status bar reads "AI review / … / Proposed geometry
  shown"; nothing is applied yet. **Apply & rebuild** runs exactly the previewed command as one
  undo step, a toast at the bottom of the viewport says what changed ("4 mounting holes updated /
  Ø10.00 mm") with an **Undo** button for exactly that step, and the status bar adds "Pattern
  preserved" when the pattern's count and pitch are unchanged. **Cancel command** leaves the
  design, its revision and the undo history exactly as they were.
- **Design history** (a glass card over the bottom of the canvas). Every feature in order as an icon over its name; the current step
  is highlighted. Click a step to select its feature, double-click to edit, drag the end-of-history
  marker (the accent bar) to roll back, and drag features to reorder them. The strip's right end
  says *End of history* or how many steps are rolled back. Preferences can put the history
  vertically at the far left of the viewport, beside the browser. Its seam handle switches between
  icon chips and compact feature names; the browser can collapse to a rail beside either width.
  Previous, Play and Next sit at the top. Double-click Previous to roll to the start.
- **Status bar** (bottom). What is selected and the model units (click them to change the
  design's units) at the left; at the right the assistant, the start page, Preferences, Help (?),
  Report a bug, the version and the credit *Polykrom by WCKD*.
- **Command search (S).** Click the ribbon search field or press S to open the search popup
  below it. Results group commands by their ribbon path and show matching features and user
  parameters from this design. With an empty query, recent commands appear first, followed by
  pinned commands in the Commands group. Up/Down
  choose, Enter runs, Tab fills the selected name, and Esc closes. A feature opens its existing
  editor; a parameter opens Change Parameters filtered to its name. To run with explicit values,
  enter an id and JSON object, for example `solid.extrude {"distance": 20}`. The popup validates
  JSON as you type and shows parser errors. Shift+Enter runs it and keeps the popup open.
  Right-click a command for its existing pin and toolbar actions.

Preferences (File › Preferences) hold the theme, workspace layout, the units of new designs, the mouse preset, the
visual style and autosave. Graphics › Colour vision gives the colours that carry a meaning (sketch
states, axes, bends, analyses, icons) values for a weak red, green or blue sense: Protan, Deutan or
Tritan. In a mode, the faces Draft and Accessibility Analysis mark as failing are striped too.
Command messages and errors show for a few seconds in the status chip at the bottom left of the view. Click the chip, or choose Help › **Messages…**, for every message of the session: newest first, errors in red, each line selectable, with **Copy all** for a bug report.
Help › Keyboard Shortcuts lists every key, and the application menu's Keyboard
Shortcuts changes them.

### Workspace layout

General › **Workspace layout** switches between **Floating** cards and **Compact**
panels. Floating, the default, keeps the canvas full-bleed under glass cards: the
Design Browser (**Glass**, or **Clear** for a more see-through card), the document
strip, the history, Properties and Review changes all float over the model. Compact
makes the ribbon one top bar, puts the Design Browser flush underneath as a solid
full-height column with a square join, and docks tool sheets below the compass. Tool
sheets remain draggable: their four corners round away from the right edge, and their
two right corners square when docked there. Assistant remains on the outside edge, and
History keeps its chosen bottom, left or shared Browser column.
The choice is saved between launches. Switching preserves the current tool,
camera, selection and design; Cancel and Restore Defaults work like other preferences.

General › **Show text under ribbon icons** is a separate, saved option for either
layout. Turn it off for shorter icon-only command buttons; tooltips, section names
and command menus remain available.

### Colour themes

General › **Theme** lists Dark, Light and System (follows the operating system), the built-in
Gruvbox themes (dark and light, each in hard, normal and soft contrast) and any themes you have
imported. A choice applies at once. Dark, the default, is the design kit's graphite shell: a
graphite window and ribbon, a darker viewport, a muted olive selected row and the lime accent on
primary actions and the active tab. Light keeps the same accent hue, deepened so that text reads.
The interface is set in Inter; numeric values and code in IBM Plex Mono (both from the design kit,
on every desktop and in the browser).

A theme is a small JSON file that starts from the Dark or Light palette and changes any of its
colours and corner radii by name:

```json
{
  "format": "solidcraft-theme/1",
  "name": "My Theme",
  "base": "dark",
  "colors": {
    "panel": "#282828",
    "text": "#ebdbb2",
    "accent": "#83a598",
    "viewport_top": "#32302f",
    "viewport_bottom": "#32302f",
    "triad": ["#fb4934", "#b8bb26", "#83a598"]
  },
  "radii": {
    "radius_sm": 2,
    "radius_md": 4,
    "radius_lg": 6,
    "radius_xl": 8,
    "control_radius": 4,
    "sketch_mode_radius": 4
  }
}
```

Colours are `#rrggbb`, or `#rrggbbaa` for translucent ones. A few colours, such as the view
triad's X, Y and Z, take a list. Any colour the file leaves out keeps the base palette's value.

`radii` is optional. Its six keys above accept integers from 0 to 32 in logical UI points;
zero makes square corners. Omitted keys keep today's base values: 4, 6, 8, 10, 6 and 7,
respectively. The first four style small accents through large surfaces; `control_radius`
styles form controls, and `sketch_mode_radius` styles sketch creation buttons. Themes own
these settings, with matching shapes across light and dark variants. Dock joins remain
square, circular controls remain circular, and control sizes and spacing stay the same.
Platinum 9 and the phosphor themes use square corners, SolidCraft 95 uses subtle 3-point
corners, Blueprint uses the standard radii, and Cupertino uses softer corners. Other built-in
themes retain the defaults.

Tab accents also have separate fill, small-text ink and tint keys. Sketch uses
`sketch_accent`, `sketch_ink`, `sketch_soft`; Surface, Mesh and Sheet Metal
use `<tab>_fill`, `<tab>_ink`, `<tab>_soft` (`sheet` for Sheet Metal). Form uses
`form_fill`, `form_tab_ink`, `form_soft` because `form_ink` names the general form
text. Model and the Render workspace keep `accent`, `selected_ink`, `accent_soft`; the
`render_fill`, `render_ink` and `render_soft` keys are still read from theme files but unused. The older
`surface_accent`, `mesh_accent`, `form_accent` and `render_accent` keys retain their
existing roles and values. `tab_on_fill_dark` is the dark text fallback for tab
pills when `on_accent` has less than 4.5:1 contrast with the fill. See the
[per-tab token reference](../dev/ui-kit.md#per-tab-accent-tokens) for all pairs.

- **Import…** adds a theme file and turns it on. Importing a file with the same name again updates
  that theme. A file with an unknown colour name or a malformed colour is refused, and the message
  names every problem.
- **Export…** saves the theme on now as a complete file listing every colour and radius. It is a good starting
  point for your own theme.
- **Remove** deletes the selected imported theme.

Retired window-band and procedural icon colours in older exports are discarded on import.
Command icons use the shared SVG palette.

Imported themes are kept with your preferences, so they work in the browser app too. Colour
vision modes still apply on top of any theme.

Navigation › **Smooth wheel zoom** is on by default. Turn it off to apply each wheel or
trackpad event immediately, with no smoothing tail. It is independent of **Animate view
changes**, which controls axis views, Look At, Home and Fit. Re-enabling smoothing during
a wheel gesture takes effect after that gesture ends, avoiding leftover zoom being replayed.

Navigation › **Alias** uses Alias's camera keys: Shift+Alt+left drag orbits, Shift+Alt+middle
drag pans and Shift+Alt+right drag zooms (drag down to move away, up to move closer). Without
Shift+Alt, a left drag still selects with a box and drags sketch geometry.

Some Linux desktops take Alt+drag for themselves (moving or resizing windows), so the viewport
never sees it. To free it:

- **KDE Plasma:** System Settings › Window Management › Window Behavior › Window Actions, set
  **Modifier key** to Meta (or None).
- **GNOME:** run `gsettings set org.gnome.desktop.wm.preferences mouse-button-modifier '<Super>'`
  (or set Tweaks › Windows › Window Action Key to Super).

Shift-orbit snapping includes 45-degree edge views and isometric corner views, with a
lightly damped spring transition and a small boundary margin to prevent flicker.

On desktop, Ctrl/Cmd+Plus (or Equals) enlarges the interface, Ctrl/Cmd+Minus reduces it,
and Ctrl/Cmd+0 restores 100%. These shortcuts update the UI scale preference. The UI scale
slider in Preferences applies the new layout when released, so it stays still while dragging.


Theme files can also define `borders`: `primary` outer outlines, `secondary` major section
rules, `tertiary` small dividers, `container` solid preview/card/keycap outlines, and independent
`control` outlines. Each accepts a `width`
from 0 to 4 UI points and a `color` in hex. Optional `primary_inset` adds a companion outline
inside panel chrome, with a `gap` from 0 to 12 points. Missing settings keep existing borders.
Blueprint uses a single white outer outline, dotted section lines and dark minor dividers. Secondary rules optionally accept `pattern: "dotted"`; the default is `solid`. Focus, selection and
error outlines retain their semantic colours. See the UI-kit border hierarchy for the schema.

SolidCraft 95 also defines optional `borders.bevel`: a per-band `width` and the
`highlight`, `light`, `shadow` and `dark_shadow` colours. The shared UI uses these
for raised panels/buttons, recessed fields and reversed pressed-button lighting.
Themes that omit this setting keep their existing flat borders.

Theme files may also set `icon_palette`: opaque colours for SVG display roles,
and three-colour pale/medium/strong ramps for Solid, Surface, Mesh, Form, Sketch,
constraints and construction. These settings colour the existing command artwork
without changing its shapes. Omitted roles keep the default light/dark palette.

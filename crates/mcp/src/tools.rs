//! MCP tool definitions and dispatch. Every tool is a thin wrapper over engine commands
//! (`engine.execute`) and the inspection / rendering control methods.

use std::sync::atomic::{AtomicU64, Ordering};

use polykrom_engine::io::vfs;
use polykrom_engine::timing;
use serde_json::{Map, Value, json};

use crate::backend::{Backend, STILL_RUNNING};

/// Most commands in one `batch` call.
const MAX_BATCH: usize = 10_000;
/// Largest image returned by `screenshot`.
const MAX_IMAGE_BYTES: usize = 32 << 20;

const VIEWS: &[&str] = &["iso", "home", "front", "back", "top", "bottom", "left", "right", "fit"];

fn obj(props: Value, required: &[&str]) -> Value {
    let mut o = json!({"type": "object", "properties": props, "additionalProperties": false});
    if !required.is_empty() {
        o["required"] = json!(required);
    }
    o
}

/// What a tool may do, for the client's approval prompts (MCP tool annotations). Clients that
/// auto-approve on these hints must not be told less than the truth (#205).
#[derive(Clone, Copy)]
enum Effect {
    /// Only looks.
    ReadOnly,
    /// Changes the design in memory, undoably.
    Edits,
    /// Replaces the design in memory (unsaved changes are lost).
    Replaces,
    /// May read, write, overwrite or delete files on disk (any command, a file path).
    Files,
}

fn tool(name: &str, title: &str, desc: &str, mut schema: Value, effect: Effect) -> Value {
    // Every tool takes its reply's size and paging (#266).
    if let Some(o) = schema.as_object_mut() {
        let props = o.entry("properties").or_insert_with(|| json!({}));
        if let Some(p) = props.as_object_mut() {
            for (k, v) in crate::reply::schema() {
                p.entry(k).or_insert(v);
            }
        }
    }
    let (read_only, destructive, open_world) = match effect {
        Effect::ReadOnly => (true, false, false),
        Effect::Edits => (false, false, false),
        Effect::Replaces => (false, true, false),
        Effect::Files => (false, true, true),
    };
    json!({
        "name": name,
        "title": title,
        "description": desc,
        "inputSchema": schema,
        "annotations": {"title": title, "readOnlyHint": read_only, "destructiveHint": destructive, "idempotentHint": read_only, "openWorldHint": open_world},
    })
}

fn string(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}

/// Every tool, with its JSON schema. The same list serves the headless and the bridged backend.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "list_commands",
            "List commands",
            "The command catalog: id, label, toolbar tab/panel, shortcut, one-line parameter docs and whether the command is \
             enabled right now. Ids are dotted and lower-case (solid.extrude, sketch.create, \
             solid.fillet, sketch.inspect, timeline.rollback). Optional case-insensitive \
             substring `filter` over id, label, tab and panel (not the parameter docs, so a command id \
             finds that command rather than every command whose docs mention it).",
            obj(json!({"filter": string("Substring to match, e.g. \"sketch\", \"fillet\", \"pattern\"")}), &[]),
            Effect::ReadOnly,
        ),
        tool(
            "describe_command",
            "Describe a command",
            "One command's parameter docs and, for commands with a typed contract (solid.box, solid.extrude and engine.describe so far), the \
             JSON Schemas (2020-12) of its parameters (`input`) and of a successful result (`output`); null for the others.",
            obj(json!({"id": string("Command id, e.g. \"solid.extrude\"")}), &["id"]),
            Effect::ReadOnly,
        ),
        tool(
            "execute",
            "Execute a command",
            "Run one command with JSON parameters, exactly as the toolbar, palette and scripts do. Never opens a dialog. \
             Examples: sketch.create {plane: \"XY\"}; sketch.rectangle.two_point {p0: [0,0], p1: [40,30]}; sketch.circle.center \
             {center: [20,15], radius: 5}; sketch.dimension {entities: [curve id], value: \"width\"}; sketch.finish {}; solid.extrude \
             {distance: 20, operation: \"cut\"}; solid.fillet {edges: [[0,0,10]], radius: 3}; solid.box {length, \
             width, height}. Lengths are mm; expressions may use parameters and units (\"width / 2\", \"30 deg\"). Returns \
             the command's result; failures leave the design unchanged. Sketch commands that add geometry return \
             created: {points, curves, constraints, dimensions}: the ids of what they added, stable and not reused \
             after a delete (undo rewinds the counters with the design), usable right away \
             in sketch.dimension, sketch.constraint.* and sketch.delete. Feature commands return the feature's id \
             and name, every body (bodies) and the bodies the feature created, changed and removed (created_bodies, changed_bodies, removed_bodies).",
            obj(
                json!({
                    "command": string("Command id (see list_commands)"),
                    "params": {"type": "object", "description": "The command's parameters"},
                    "verbose": {"type": "boolean", "description": "false: omit the full feature body list and sketch ids repeated outside `created`; keep created_bodies, changed_bodies and removed_bodies (default true)"},
                    "select": {"type": "array", "items": {"type": "string"}, "description": "Only these parts of the result: dotted paths, * for every item of a list or value of an object, a name at a list asked of each item (e.g. [\"total.mass_g\", \"bodies.*.name\", \"bodies.*.mass_g\"]). Paths that match nothing are listed in missing_paths. Keeps replies on large designs small"},
                }),
                &["command"],
            ),
            Effect::Files,
        ),
        tool(
            "batch",
            "Run several commands",
            "Run commands in order, stopping at the first error. Each step is {command, params}: an engine command id \
             (as list_commands lists them). A step may also name one of these tools with that tool's arguments as params, \
             and runs the engine command it stands for: set_parameter (parameters.change; `value` or `expression`), \
             list_commands (engine.commands, with `filter`), describe_command (engine.describe), execute ({command, params}), \
             measure (inspect.measure), \
             inspect_design (document.inspect), undo, redo, export, save, open, new_design, crash_reports; screenshot, \
             body_topology, batch and pending can't be steps. On an error the steps already run are undone (unless \
             rollback: false), so the batch is all-or-nothing; the reply names the failing step (index from 0), its error, \
             every result before it, whether the design really went back (rolled_back), the steps whose changes are in the \
             design (applied) and those never sent (not_run). The usual way to build a part: sketch, draw, dimension, \
             finish, extrude, fillet… in one call.",
            obj(
                json!({
                    "commands": {
                        "type": "array",
                        "description": "Steps: [{\"command\": id, \"params\": {...}}, ...]",
                        "items": {"type": "object", "properties": {"command": {"type": "string"}, "params": {"type": "object"}, "select": {"type": "array", "items": {"type": "string"}, "description": "This step's result only at these dotted paths (as execute's select)"}}, "required": ["command"]},
                    },
                    "rollback": {"type": "boolean", "description": "Undo completed steps when one fails (default true)"},
                    "on_error": {"type": "string", "enum": ["rollback", "keep"], "description": "When a step fails: rollback (default) takes back the steps before it; keep keeps them as one undo step (`kept` says whether they are one)"},
                    "timings": {"type": "boolean", "description": "Add `timings`: [{index, command, ms}] for every step run (the failing one included) and `total_ms` (default false)"},
                    "profile": {"type": "boolean", "description": "Like timings, and each step's `profile`: {boolean_ms, tessellate_ms, other_ms}, its time in kernel booleans, in tessellation and the rest (default false)"},
                    "verbose": {"type": "boolean", "description": "false: a compact reply. Feature steps leave out the full body list (created_bodies, changed_bodies and removed_bodies stay), sketch steps the ids they repeat outside `created` (default true)"},
                }),
                &["commands"],
            ),
            Effect::Files,
        ),
        tool(
            "inspect_design",
            "Inspect the design",
            "With no arguments, a summary: counts (features, bodies, sketches, components, joints, parameters), the \
             parameters, the features with errors or warnings, the bodies and top-level components by name, the active sketch \
             and undo depth. With any argument, the whole design: user parameters (expression, value, errors), the timeline \
             (features with type, suppressed, rolled back, error, warning, timing), bodies (volume, area, centre of mass, bounding box, face/edge/vertex \
             counts), sketches (curves with ids and coordinates, points, constraints, profiles, degrees of freedom, solve \
             status), assembly joints (what each origin is attached to: occurrence, the feature, face, edge or sketch \
             curve its face names refer to, whether it resolves and why not; `execute` `joint.inspect {joint}` gives one), \
             the active sketch, selection and undo depth.",
            obj(
                json!({
                    "measure": {"type": "boolean", "description": "Measure bodies (default true)"},
                    "sketches": {"type": "boolean", "description": "Full sketch contents (default true; false = summary only)"},
                    "joints": {"type": "boolean", "description": "Joint definitions as joint.inspect gives them, when the design has joints (default true)"},
                }),
                &[],
            ),
            Effect::ReadOnly,
        ),
        tool(
            "quality",
            "Model quality gate",
            "Run the hard model-quality gate (`inspect.quality`) before calling a model done. It reports pass/fail for unresolved parameters, active feature errors, under-constrained sketches and invalid/open bodies, plus warnings for missing parameters, materials and suppressed features. A pass is structural evidence only: it does not prove tolerances, strength, safety or manufacturability. Pass `bodies` to limit the body checks; `allow_open`, `allow_underconstrained` and `require_parameters` make the intent explicit.",
            obj(
                json!({
                    "bodies": {"type": "array", "items": {"type": "string"}, "description": "Bodies to check (default: all)"},
                    "allow_open": {"type": "boolean", "description": "Treat open surface bodies as warnings instead of failures (default false)"},
                    "allow_underconstrained": {"type": "boolean", "description": "Treat under-constrained sketches as warnings instead of failures (default false)"},
                    "require_parameters": {"type": "boolean", "description": "Fail when the design has no user parameters (default false)"},
                }),
                &[],
            ),
            Effect::ReadOnly,
        ),
        tool(
            "measure",
            "Measure bodies",
            "Volume (mm³), surface area (mm²), centre of mass, bounding box and face/edge/vertex counts of one body or all \
             bodies, with totals. With `items` (one or two selections, as `execute` takes them: faces, edges, vertices, \
             sketch curves, profiles): one item's length, area or position, a circle's, arc's or cylinder's `diameter` and \
             `radius` (with `circumference`, or an arc's `arc_length` and `angle_deg`), a slot's `slot` sizes; two items' \
             `min` and `max` distance and, where one is round, `center_distance` (centre to centre, or from its axis).",
            obj(
                json!({
                    "body": string("Body name (default: all bodies)"),
                    "items": {"type": "array", "items": {"type": "object"}, "description": "One or two selections to measure instead of bodies, e.g. {\"type\": \"face\", \"body\": \"Body1\", \"index\": 3, \"point\": [x,y,z]}"},
                }),
                &[],
            ),
            Effect::ReadOnly,
        ),
        tool(
            "body_topology",
            "List edges and faces",
            "The edges (index, midpoint, length, endpoints) and faces (index, name, area, centroid, plane normal, a `point` that lies \
             on the face, and for a cylindrical face its `cylinder`: origin, axis, radius) of a body. Use an edge midpoint as the \
             [x,y,z] point that picks the edge for fillet and chamfer; a face's point or its name picks the face for the face commands. \
             A large body has hundreds of each: narrow it with `kind`, `near` (+ `radius`), `min_area`, `min_length` and `limit`; \
             `edges_total` and `faces_total` count the body's edges and faces before filtering.",
            obj(
                json!({
                    "body": string("Body name, e.g. \"Body1\""),
                    "kind": {"type": "string", "enum": ["edges", "faces"], "description": "Only edges or only faces (default both)"},
                    "near": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "[x,y,z]: nearest first, each with its `distance` (to a face's `point`, to the nearest of an edge's midpoint and ends: spots on it, not its closest point)"},
                    "radius": {"type": "number", "exclusiveMinimum": 0, "description": "With `near`: only those within this distance (mm)"},
                    "min_area": {"type": "number", "minimum": 0, "description": "Only faces of at least this area (mm²)"},
                    "min_length": {"type": "number", "minimum": 0, "description": "Only edges of at least this length (mm)"},
                    "limit": {"type": "integer", "minimum": 1, "description": "At most this many edges and this many faces (after the filters; with `near`, the nearest)"},
                }),
                &["body"],
            ),
            Effect::ReadOnly,
        ),
        tool(
            "set_parameter",
            "Set a parameter",
            "Create or change a user parameter and re-evaluate the timeline. `value` is a number (mm) or an expression \
             (\"40 mm\", \"width * 2\", \"30 deg\"); `expression` is the same thing under the name the parameters.change command \
             uses. Give one of the two (both only when they agree). Returns the new value, the features recomputed (`recomputed`), the \
             features whose earlier result for the same inputs was reused instead (`restored`, e.g. after setting a value back) and any \
             features that now fail.",
            obj(
                json!({
                    "name": string("Parameter name"),
                    "value": {"type": ["number", "string"], "description": "Number or expression (or give it as `expression`)"},
                    "expression": {"type": ["number", "string"], "description": "Same as `value` (the parameters.change name); give one of the two"},
                    "unit": string("mm, deg or \"\" (unitless); default from the expression"),
                    "comment": string("Comment shown in the parameters table"),
                }),
                &["name"],
            ),
            Effect::Edits,
        ),
        tool(
            "screenshot",
            "Look at the design",
            "Render the design and return it as a PNG image. Headless: a CPU render of the model, fitted to the view. \
             Connected to the app: a screenshot of the window (source \"window\", the default) or a render of the model \
             with the app's camera (source \"model\"). Both show an active section (inspect.section) with its cut face \
             capped. `view` first turns the camera to a standard view or a saved named view (view.save), `camera` to              any camera (as view.apply). \
             `grid: false` and `axes: false` leave the ground grid (on Z 0) and the origin's axes out of a model render. \
             A body whose appearance is see-through (appearance.assign with an `opacity` below 1) is drawn see-through, \
             so what is inside it shows: that is how to look at the body a cut would remove inside the material it meets.",
            obj(
                json!({
                    "view": string(
                        "Standard view (iso, home, front, back, top, bottom, left, right, fit) or the name of a saved named view                          (default: keep the current one; headless starts at iso)"
                    ),
                    "camera": {"type": "object", "description": "Look from this camera instead: {target: [x,y,z], yaw, pitch, distance, fov?}                         (radians, mm; as view.save and view.apply)"},
                    "path": string("Also keep the PNG at this path"),
                    "width": {"type": "integer", "description": "Render width in pixels (model renders; default 960)"},
                    "height": {"type": "integer", "description": "Render height in pixels (model renders; default 600)"},
                    "source": {"type": "string", "enum": ["window", "model"], "description": "window = app screenshot, model = model render"},
                    "grid": {"type": "boolean", "description": "false: leave the ground grid out of the render. It lies on Z 0 and cuts through a part that straddles it. Model renders only: default true, or as the app shows it when connected; with `grid` or `axes` a connected screenshot is a model render, and source \"window\" is an error"},
                    "axes": {"type": "boolean", "description": "false: leave the origin's three axes out of the render (see `grid`). Axes hidden with browser.visibility stay hidden"},
                }),
                &[],
            ),
            Effect::Files,
        ),
        tool(
            "export",
            "Export",
            "Export bodies to STEP (.step/.stp), 3MF (.3mf), binary STL (.stl), ASCII STL (format stla) or OBJ. The format \
             comes from the extension unless given. URDF (.urdf) exports the whole assembly for robot simulators: a link per \
             component placement with mass and inertia, joints from the assembly's joints, and STL meshes in metres written \
             next to it (docs/urdf.md); RViz needs mesh_prefix such as package://robot/meshes/. MJCF (format mjcf, .xml) \
             writes the same robot for MuJoCo with position actuators and a floor (docs/mjcf.md); its reply counts \
             bodies, joints, actuators and meshes and gives the mass, free root, keyframe and each actuator's ranges.",
            obj(
                json!({
                    "path": string("Output file; missing folders are made"),
                    "format": {"type": "string", "enum": ["step", "3mf", "3mf-tagged", "stl", "stla", "obj", "urdf", "mjcf"]},
                    "bodies": {"type": "array", "items": {"type": "string"}, "description": "Body names (default: all; not with urdf)"},
                    "mesh_prefix": string("urdf: put before each mesh file name (default: none, so relative to the .urdf)"),
                    "effort": {"type": "number", "description": "urdf: joint effort limit, N or N m (default 1000)"},
                    "velocity": {"type": "number", "description": "urdf: joint velocity limit, m/s or rad/s (default 10)"},
                    "floating": {"type": "boolean", "description": "mjcf: free base that falls onto the floor (default: when nothing is grounded)"},
                    "scene": {"type": "boolean", "description": "mjcf: floor and light (default true)"},
                    "kp": {"type": "number", "description": "mjcf: position actuator gain, N m/rad or N/m (default 1000)"},
                    "actuated": {"type": "array", "items": {"type": "string"}, "description": "mjcf: joint ids or names to actuate; default rated joints and every joint outside a closed loop"},
                }),
                &["path"],
            ),
            Effect::Files,
        ),
        tool(
            "crash_reports",
            "Crash reports",
            "Polykrom's crash reports on this computer (the app's when connected to it). Without `id`: the list, newest \
             first (id, kind, time, message, command, count, version, path, replayable). With `id`: the whole report: \
             message, location, backtrace, app version and commit, system, the command that crashed and the recent ones, \
             and design_path / replay_path when it can be replayed: `polykrom-cli crashes replay <id>`, or \
             `polykrom-cli run <replay_path> --in <design_path>`. Reports never leave this computer.",
            obj(
                json!({
                    "id": string("A report id from the list"),
                    "limit": {"type": "integer", "description": "Reports to list (default 20)"},
                }),
                &[],
            ),
            Effect::ReadOnly,
        ),
        tool(
            "running",
            "What the app is running",
            "What the app is running or has queued right now, for every client: each request's connection and id, method, \
             command id (a batch's step count), seconds since it arrived (running_s) and whether it still waits its turn \
             (queued); `self` marks a request sent on the connection that asks. Answers at once, even while the app is busy, \
             and never shows parameters. Ask it when a call timed out or the app seems slow, before sending a change again.",
            obj(json!({}), &[]),
            Effect::ReadOnly,
        ),
        tool(
            "pending",
            "Result of a running request",
            "Bridged to the app: the result of a request that was still running when its tool call returned (an error \
             starting \"still running:\"). Waits for it up to 60 s: then its own result (or error), or done: false while \
             it still runs (call again). pending: null when nothing is running. A request still running was not \
             cancelled: don't send it again; other calls wait for it before they are sent.",
            obj(json!({}), &[]),
            Effect::ReadOnly,
        ),
        tool(
            "undo",
            "Undo",
            "Undo the last change to the design, the last `steps` changes, or every change since a checkpoint (`to`), in one go. A command is one step, however many it ran inside. The reply lists what was undone, the label of the step the next undo would take back (`next`) and how many are `left`.",
            obj(
                json!({
                    "steps": {"type": "integer", "minimum": 1, "description": "How many changes to undo (default 1); the design is evaluated once"},
                    "to": {"type": ["string", "integer"], "description": "Undo every change since this checkpoint: the name given to `edit.checkpoint {name}` (through execute), or its id"},
                }),
                &[],
            ),
            Effect::Edits,
        ),
        tool(
            "redo",
            "Redo",
            "Redo the last undone change, or the last `steps` of them. The reply lists what was redone, the next redo's label and how many are left.",
            obj(json!({"steps": {"type": "integer", "minimum": 1, "description": "How many changes to redo (default 1)"}}), &[]),
            Effect::Edits,
        ),
        tool(
            "new_design",
            "New design",
            "Start a new, empty design (unsaved changes to the current one are discarded).",
            obj(json!({"name": string("Design name (default Untitled)")}), &[]),
            Effect::Replaces,
        ),
        tool(
            "open",
            "Open a design",
            "Open a .solidcraft design file, a STEP file (.step/.stp: a new design whose bodies come from the file, as an \
             Import base feature in a component named after the file; an assembly's parts become nested components) or a \
             mesh (.3mf/.stl/.obj/.glb/.gltf: mesh bodies in such a component; a glTF file's nodes become nested components and its \
             materials appearances); the result reports the component (id, component_name). Replaces the current design; insert into it \
             instead with the file.insert_step (STEP) or file.insert_mesh (mesh) commands. A STEP file from a system \
             that has Y up opens lying on its back: `up: \"y\"` turns its component a quarter turn about X (Y becomes Z). glTF is Y up, \
             so it is turned by default (`up: \"z\"` keeps its axes). `names` picks what the bodies are named after when the solids' own names say nothing useful.",
            obj(
                json!({
                    "path": string("Path to a .solidcraft, .step/.stp, .3mf, .stl, .obj, .glb or .gltf file"),
                    "up": {"type": "string", "enum": ["z", "y"], "description": "STEP and glTF only. z keeps the file's axes; y: the file has Y up, so its bodies are turned to stand Z up. Default: z for STEP, y for glTF"},
                    "names": {"type": "string", "enum": ["auto", "solid", "product", "instance"], "description": "STEP, IGES, FreeCAD and Rhino. auto (default): keep meaningful solid names, replace placeholders with the nearest named product, else instance; solid: the solid's own name; product: the part's name; instance: the occurrence's name, else the product's"},
                }),
                &["path"],
            ),
            Effect::Files,
        ),
        tool(
            "save",
            "Save the design",
            "Save the design to `path` or its current file. The default is a design container; a .json path selects plain JSON. Set format to container or json to override the path.",
            obj(
                json!({"path": string("Path to save to (default: the current file)"), "format": {"type": "string", "enum": ["container", "json"], "description": "Design encoding (default: JSON for .json, container otherwise)"}}),
                &[],
            ),
            Effect::Files,
        ),
        tool(
            "polish",
            "Polish the design",
            "A finishing pass: measure how finished the design is with inspect.polish (scores 0–100 per body and for the design, by \
             category: look, print, strength, mass, tidy, assembly), and with `apply: \"safe\"` apply its fillets and chamfers through \
             polish.apply (each group checked for a valid body, unchanged mass, no new interference and working joints, and rolled back \
             if it fails) and measure again. Returns before and after scores, what was applied and skipped, the fixes still suggested \
             and the findings to act on by hand, the `style` judged by (organic, faceted or technical, asked or inferred) and the \
             design's `colours` (palette, near-duplicate colours). With `views`, also four model renders (front three-quarter, side, \
             top, back three-quarter) as images and PNG paths to critique. With `reference` (a concept mesh, fitted first with \
             inspect.fit_reference), also `reference_match`: how well the silhouettes match per view, where the design sticks out or \
             falls short, and the colour match. Repeat, making the judgement changes in between, until the score stops rising \
             (skill_reference solidcraft-polish).",
            obj(
                json!({
                    "bodies": {"type": "array", "items": {"type": "string"}, "description": "Bodies to judge (default every visible solid body)"},
                    "categories": {"type": "array", "items": {"type": "string", "enum": ["look", "print", "strength", "mass", "tidy", "assembly"]}, "description": "Only these categories' fixes and findings"},
                    "apply": {"type": "string", "enum": ["safe", "none"], "description": "safe: apply the fillets and chamfers; none (default): only measure"},
                    "views": {"type": "boolean", "description": "Also render four views (default false)"},
                    "printed": {"type": "array", "items": {"type": "string"}, "description": "Bodies to judge as printed whatever their material"},
                    "skip": {"type": "array", "items": {"type": "string"}, "description": "Bodies to leave out, such as bought parts modelled as blocks"},
                    "max_fixes": {"type": "integer", "description": "How many fixes to list or try (1–100)"},
                    "reference": {"type": "array", "items": {"type": "string"}, "description": "A concept to compare with: the reference body names (an inserted concept mesh, fitted first with inspect.fit_reference). Adds `reference_match`: inspect.compare_reference's silhouette IoU per view, the regions that stick out or fall short, named by part, and the colour match"},
                    "style": {"type": "string", "enum": ["auto", "organic", "faceted", "technical"], "description": "The finish aimed at. auto (default): faceted when the parts carry more chamfers than fillets, else organic; organic: fillets in a radius family, raw flat faces read as boxy; faceted: hard-surface facets with crisp chamfers in two or three sizes, flat facets are good and fixes are chamfers; technical: edges just broken, flat faces fine. Match it to the concept: faceted for a hard-surface or low-poly look"},
                }),
                &[],
            ),
            Effect::Files,
        ),
        tool(
            "skill_reference",
            "Skill reference",
            "Read a document of SolidCraft's bundled agent skills (solidcraft-modeling, solidcraft-mechanisms, \
             solidcraft-editing, solidcraft-checking, solidcraft-polish) from the copy built into SolidCraft. `file` defaults to the skill's main \
             document: its reference.md, or SKILL.md when it has none. An unknown file gets the closest document, with a note \
             naming the available ones; unknown skills are refused with the available ones.",
            obj(
                json!({
                    "skill": string("Skill name, such as solidcraft-modeling"),
                    "file": {"type": "string", "description": "Document of that skill (default: its reference.md, or SKILL.md when it has none)"},
                }),
                &["skill"],
            ),
            Effect::ReadOnly,
        ),
    ]
}

fn type_matches(t: &str, v: &Value) -> bool {
    match t {
        "string" => v.is_string(),
        "number" => v.is_number(),
        "integer" => v.is_i64() || v.is_u64(),
        "boolean" => v.is_boolean(),
        "array" => v.is_array(),
        "object" => v.is_object(),
        "null" => v.is_null(),
        _ => true,
    }
}

/// Validates `args` against the tool's schema (top level): unknown tool, missing required
/// arguments, unknown arguments, type and enum mismatches.
/// Tools whose list arguments must hold only items of the listed type.
const STRICT_ITEMS: [&str; 1] = ["polish"];

pub(crate) fn check_arguments(name: &str, args: &Value) -> Result<(), String> {
    let defs = tool_definitions();
    let def = defs.iter().find(|d| d["name"] == name).ok_or_else(|| format!("unknown tool `{name}`"))?;
    let schema = &def["inputSchema"];
    let Some(a) = args.as_object() else { return Err(format!("{name}: arguments must be an object")) };
    for r in schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if a.get(r).is_none_or(Value::is_null) {
            return Err(format!("{name}: missing required argument `{r}`"));
        }
    }
    let props = schema["properties"].as_object();
    for (k, v) in a {
        let Some(p) = props.and_then(|p| p.get(k)) else {
            let valid: Vec<&str> = props.map(|p| p.keys().map(String::as_str).collect()).unwrap_or_default();
            let valid = if valid.is_empty() { "none".to_string() } else { valid.join(", ") };
            let k: String = k.chars().take(64).collect();
            return Err(format!("{name}: unknown argument `{k}`; its arguments are: {valid}"));
        };
        if v.is_null() {
            continue;
        }
        let ok = match &p["type"] {
            Value::String(t) => type_matches(t, v),
            Value::Array(ts) => ts.iter().filter_map(Value::as_str).any(|t| type_matches(t, v)),
            _ => true,
        };
        if !ok {
            return Err(format!("{name}: argument `{k}` must be of type {}", p["type"]));
        }
        if let Some(allowed) = p["enum"].as_array()
            && !allowed.contains(v)
        {
            return Err(format!("{name}: `{k}` must be one of {}", Value::Array(allowed.clone())));
        }
        // A list's items, for the tools whose lists select what they change: `[null]` for a
        // list of names must not read as an empty list (every body). Other tools' lists stay
        // lenient (`select: [1]` is answered with what it missed).
        if let (true, Some(items), Some(t)) = (STRICT_ITEMS.contains(&name), v.as_array(), p["items"]["type"].as_str()) {
            if items.iter().any(|x| !type_matches(t, x)) {
                return Err(format!("{name}: every item of `{k}` must be of type {t}"));
            }
            if let Some(allowed) = p["items"]["enum"].as_array()
                && let Some(x) = items.iter().find(|x| !allowed.contains(x))
            {
                return Err(format!("{name}: `{k}` may not hold {x}; its items are one of {}", Value::Array(allowed.clone())));
            }
        }
    }
    Ok(())
}

/// A tool result: content blocks plus the error flag.
pub(crate) fn text_result(v: &Value, is_error: bool) -> Value {
    // Held to the call's budget, paged as asked (#266); errors go as they are.
    let fitted;
    let v = if !is_error && crate::reply::active() && !v.is_string() {
        fitted = crate::reply::fit(v.clone());
        &fitted
    } else {
        v
    };
    let text = match v {
        Value::String(s) => s.clone(),
        v => serde_json::to_string_pretty(v).unwrap_or_default(),
    };
    let mut r = json!({"content": [{"type": "text", "text": text}]});
    if is_error {
        r["isError"] = json!(true);
    }
    r
}

/// Run a tool. Failures are tool results with `isError: true` (the model sees the message);
/// nothing here panics on hostile arguments.
pub fn call_tool(b: &mut dyn Backend, name: &str, args: &Value) -> Value {
    let mut a = args.as_object().cloned().unwrap_or_default();
    // The reply's size and paging are the server's; the tool never sees them (#266).
    let shape = crate::reply::take(&mut a);
    crate::reply::shaped(shape, || {
        let a = &a;
        let r = match name {
            "screenshot" => return screenshot(b, a),
            "polish" => return polish_result(b, a),
            "batch" => return batch(b, a),
            _ => dispatch(b, name, a),
        };
        match r {
            Ok(v) => text_result(&with_save_hint(&*b, v), false),
            Err(e) => text_result(&Value::String(e), true),
        }
    })
}

/// After this long with unsaved changes, replies remind the agent to save (#265).
pub(crate) const SAVE_HINT_AFTER: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// A reply with `unsaved_s` and a hint once the design has gone [`SAVE_HINT_AFTER`] without a
/// save (#265): headless runs lost hours of work that only a save would have kept.
fn with_save_hint(b: &dyn Backend, mut v: Value) -> Value {
    if let Some(t) = b.unsaved_for().filter(|t| *t >= SAVE_HINT_AFTER)
        && let Some(o) = v.as_object_mut()
    {
        o.insert("unsaved_s".into(), json!(t.as_secs()));
        o.insert("save_hint".into(), json!("unsaved for a while: `save` keeps the work (autosave only helps recover it)"));
    }
    v
}

fn exec(b: &mut dyn Backend, command: &str, params: Value) -> Result<Value, String> {
    let params = agent_params(command, params);
    b.call("engine.execute", json!({"command": command, "params": params})).map_err(|error| command_error(command, error))
}

/// The defaults agents get over the engine's: Edit Sketch rolls the history marker to the
/// sketch, so each change evaluates up to it and the rest once, at Finish (#159), as the
/// window's Edit Sketch does. A `roll` the agent gives is kept.
fn agent_params(command: &str, mut params: Value) -> Value {
    if command == "sketch.edit"
        && let Some(o) = params.as_object_mut()
    {
        o.entry("roll").or_insert(Value::Bool(true));
    }
    params
}

/// Enrich an unknown-id error without retrying or changing the command that was requested.
fn command_error(command: &str, error: String) -> String {
    if !error.starts_with(&format!("unknown command `{command}`")) {
        return error;
    }
    // Bound the quadratic comparison work, including for direct callers without a line cap.
    if command.len() > 128 {
        return error;
    }
    let id = command.split(['{', '(']).next().unwrap_or(command).trim();
    let packed = id != command.trim();
    let normalize = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect::<String>();
    let normalized = normalize(id);
    if normalized.is_empty() {
        return error;
    }
    let words: Vec<_> = id.split(|c: char| !c.is_ascii_alphanumeric()).filter(|s| !s.is_empty()).collect();
    let mut closest: Vec<_> = polykrom_engine::command_specs()
        .into_iter()
        .filter_map(|spec| {
            let candidate = normalize(spec.id);
            let distance = command_distance(&normalized, &candidate);
            let shared = spec.id.split('.').filter(|word| words.iter().any(|w| w.eq_ignore_ascii_case(word))).count();
            (distance <= (normalized.len() / 4).max(2) || shared > 0).then_some((
                normalized != candidate,
                distance,
                std::cmp::Reverse(shared),
                spec.id,
            ))
        })
        .collect();
    closest.sort_unstable();
    // What the guess most likely meant comes before what it is spelled like.
    let specs: Vec<&'static str> = polykrom_engine::command_specs().into_iter().map(|spec| spec.id).collect();
    let mut ids: Vec<&str> = meant(id, &specs);
    for (_, _, _, c) in &closest {
        if !ids.contains(c) {
            ids.push(c);
        }
    }
    let hints: Vec<_> = ids.iter().take(3).map(|id| format!("`{id}`")).collect();
    let mut message = format!("unknown command `{command}`");
    if !hints.is_empty() {
        message.push_str(&format!("; closest: {}", hints.join(", ")));
    }
    if packed {
        message.push_str("; command ids have no parameters; pass them in `params`");
    }
    message
}

/// The commands an unknown id most likely meant, of `specs`: the commands under it
/// (`sketch.circle` → `sketch.circle.center`), its verb as commands name it (`joint.add` →
/// `joint.create`), and the command that answers what it names (`inspect.mass_properties` →
/// `inspect.measure`). Agents guess by meaning; spelling alone points them elsewhere.
fn meant(id: &str, specs: &[&'static str]) -> Vec<&'static str> {
    let id = id.to_ascii_lowercase();
    let exists = |c: &str| specs.iter().copied().find(|s| *s == c);
    let mut out: Vec<&'static str> = specs.iter().copied().filter(|s| s.starts_with(&format!("{id}."))).take(3).collect();
    if let Some((group, verb)) = id.rsplit_once('.') {
        let usual = match verb {
            "add" | "new" | "make" | "build" | "insert" | "place" => "create",
            "remove" | "del" | "destroy" => "delete",
            "set" | "update" | "modify" | "change" => "edit",
            _ => "",
        };
        if let Some(c) = (!usual.is_empty()).then(|| exists(&format!("{group}.{usual}"))).flatten() {
            out.push(c);
        }
    }
    let words: Vec<&str> = id.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let names = |list: &[&str]| words.iter().any(|w| list.contains(w));
    for (topic, command) in [
        (
            &["mass", "weight", "volume", "area", "centroid", "inertia", "density", "properties", "dimensions", "bbox", "bounds"][..],
            "inspect.measure",
        ),
        (&["interference", "collision", "collisions", "clash", "clashes", "overlap"][..], "inspect.interference"),
        (&["clearance", "gap", "gaps", "distance"][..], "inspect.clearance"),
        (&["bom", "bill"][..], "assembly.bom"),
    ] {
        if names(topic)
            && let Some(c) = exists(command)
        {
            out.push(c);
        }
    }
    let mut seen = Vec::new();
    out.retain(|c| {
        !seen.contains(c) && {
            seen.push(*c);
            true
        }
    });
    out
}

fn command_distance(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (1..=b.len()).collect();
    for (i, ca) in a.bytes().enumerate() {
        let mut diagonal = i;
        let mut left = i + 1;
        for (cell, cb) in row.iter_mut().zip(b.bytes()) {
            let next = (diagonal + usize::from(ca != cb)).min(*cell + 1).min(left + 1);
            diagonal = *cell;
            *cell = next;
            left = next;
        }
    }
    row.last().copied().unwrap_or(a.len())
}

fn pick(a: &Map<String, Value>, keys: &[&str]) -> Value {
    Value::Object(keys.iter().filter_map(|k| a.get(*k).filter(|v| !v.is_null()).map(|v| (k.to_string(), v.clone()))).collect())
}

fn str_arg<'a>(a: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn dispatch(b: &mut dyn Backend, name: &str, a: &Map<String, Value>) -> Result<Value, String> {
    match name {
        "list_commands" => {
            let all = b.call("engine.commands", json!({}))?;
            Ok(filter_commands(all, str_arg(a, "filter")))
        }
        "describe_command" => {
            let id = str_arg(a, "id").ok_or("`id` is required")?;
            exec(b, "engine.describe", json!({"command": id}))
        }
        "execute" => {
            let id = str_arg(a, "command").ok_or("`command` is required")?;
            // A tool's name given as a command (`execute {command: "list_commands"}`): say so.
            if tool_definitions().iter().any(|t| t["name"] == id) {
                return Err(format!("`{id}` is a tool, not a command: call the {id} tool itself"));
            }
            let params = a.get("params").cloned().filter(Value::is_object).unwrap_or(json!({}));
            let v = joint_errors_touched(id, &params, exec(b, id, params.clone())?);
            let v = if a.get("verbose").and_then(Value::as_bool).unwrap_or(true) { v } else { compact(v) };
            Ok(selected(v, a.get("select")))
        }
        "quality" => exec(b, "inspect.quality", pick(a, &["bodies", "allow_open", "allow_underconstrained", "require_parameters"])),
        // With no arguments, a summary (#266): the full design took 396 KB on a median stage.
        "inspect_design" if a.is_empty() => design_summary(b),
        "inspect_design" => {
            let measure = a.get("measure").and_then(Value::as_bool).unwrap_or(true);
            let mut d = b.call("document.inspect", json!({"measure": measure}))?;
            if a.get("sketches").and_then(Value::as_bool).unwrap_or(true) {
                let ids: Vec<Value> = d["sketches"].as_array().map(|v| v.iter().map(|s| s["id"].clone()).collect()).unwrap_or_default();
                let full: Vec<Value> = ids
                    .into_iter()
                    .map(|id| exec(b, "sketch.inspect", json!({"sketch": id.clone()})).unwrap_or_else(|e| json!({"id": id, "error": e})))
                    .collect();
                d["sketches"] = Value::Array(full);
            }
            // Joints: what each is attached to (left out when there are none, or the app is older).
            if a.get("joints").and_then(Value::as_bool).unwrap_or(true)
                && let Ok(j) = exec(b, "joint.inspect", json!({}))
                && j["joints"].as_array().is_some_and(|v| !v.is_empty())
            {
                d["joints"] = j["joints"].clone();
            }
            Ok(d)
        }
        "pending" => pending(b),
        "measure" => {
            if let Some(items) = a.get("items") {
                return exec(b, "inspect.measure", json!({ "items": items }));
            }
            let p = match str_arg(a, "body") {
                Some(body) => json!({"bodies": [body]}),
                None => json!({}),
            };
            exec(b, "inspect.measure", p)
        }
        "body_topology" => body_topology(b, a),
        "set_parameter" => {
            let mut p = pick(a, &["name", "unit", "comment"]);
            p["expression"] = parameter_value(a)?;
            exec(b, "parameters.change", p)
        }
        "export" => {
            exec(b, "file.export", pick(a, &["path", "format", "bodies", "mesh_prefix", "effort", "velocity", "floating", "scene", "kp", "actuated"]))
        }
        "crash_reports" => match a.get("id") {
            Some(_) => exec(b, "diagnostics.crash_report", pick(a, &["id"])),
            None => exec(b, "diagnostics.crash_reports", pick(a, &["limit"])),
        },
        "skill_reference" => crate::skill_reference::read(a),
        "polish" => polish(b, a).map(|(v, _)| v),
        "running" => b.call("control.running", json!({})),
        "undo" => undo_steps(b, "edit.undo", a),
        "redo" => undo_steps(b, "edit.redo", a),
        "new_design" => exec(b, "file.new", pick(a, &["name"])),
        "open" => exec(b, "doc.open", pick(a, &["path", "up", "names"])),
        "save" => exec(b, "file.save", pick(a, &["path", "format"])),
        other => Err(format!("unknown tool `{other}`")),
    }
}

/// A body's edges and faces, narrowed to what the call asks for (#266): a large body's full lists
/// run to hundreds of kilobytes, and an agent usually wants one face or edge near a point.
fn body_topology(b: &mut dyn Backend, a: &Map<String, Value>) -> Result<Value, String> {
    let p = pick(a, &["body"]);
    let kind = a.get("kind").and_then(Value::as_str);
    if let Some(k) = kind.filter(|k| !matches!(*k, "edges" | "faces")) {
        return Err(format!("`kind` is \"edges\" or \"faces\", not {k:?}"));
    }
    let point = |v: &Value| -> Option<[f64; 3]> {
        let a = v.as_array().filter(|a| a.len() == 3)?;
        Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?])
    };
    let near = match a.get("near") {
        None | Some(Value::Null) => None,
        Some(v) => Some(point(v).ok_or("`near` is a point [x, y, z]")?),
    };
    let num = |k: &str| a.get(k).and_then(Value::as_f64);
    let radius = num("radius");
    if radius.is_some() && near.is_none() {
        return Err("`radius` needs `near`, the point it is measured from".into());
    }
    let limit = a.get("limit").and_then(Value::as_u64).map(|l| l.max(1) as usize);
    let dist = |q: [f64; 3], x: [f64; 3]| ((q[0] - x[0]).powi(2) + (q[1] - x[1]).powi(2) + (q[2] - x[2]).powi(2)).sqrt();
    // Keep the items passing `keep`, nearest first when there is a point, cut to `limit`.
    let narrow = |items: &Value, keep: &dyn Fn(&Value) -> bool, spots: &dyn Fn(&Value) -> Vec<[f64; 3]>| -> Vec<Value> {
        let mut out: Vec<(f64, Value)> = Vec::new();
        for item in items.as_array().into_iter().flatten().filter(|i| keep(i)) {
            let d = near.and_then(|q| spots(item).into_iter().map(|x| dist(q, x)).min_by(f64::total_cmp));
            if let (Some(r), Some(d)) = (radius, d) {
                if d > r {
                    continue;
                }
            } else if radius.is_some() {
                continue;
            }
            let mut item = item.clone();
            if let (Some(d), Some(o)) = (d, item.as_object_mut()) {
                o.insert("distance".into(), json!(d));
            }
            out.push((d.unwrap_or(0.0), item));
        }
        if near.is_some() {
            out.sort_by(|x, y| x.0.total_cmp(&y.0));
        }
        out.into_iter().take(limit.unwrap_or(usize::MAX)).map(|(_, v)| v).collect()
    };
    let mut reply = json!({"body": a.get("body")});
    if kind != Some("faces") {
        let edges = exec(b, "model.edges", p.clone())?;
        let min = num("min_length");
        let keep = |e: &Value| min.is_none_or(|m| e["length"].as_f64().is_some_and(|l| l >= m));
        let spots = |e: &Value| ["mid", "start", "end"].iter().filter_map(|k| point(&e[*k])).collect();
        reply["edges_total"] = json!(edges["edges"].as_array().map_or(0, Vec::len));
        reply["edges"] = json!(narrow(&edges["edges"], &keep, &spots));
    }
    if kind != Some("edges") {
        let faces = exec(b, "model.faces", p)?;
        let min = num("min_area");
        let keep = |f: &Value| min.is_none_or(|m| f["area"].as_f64().is_some_and(|x| x >= m));
        // A face's point lies on it; its centroid may not (a face with a hole), so it is the fallback.
        let spots = |f: &Value| point(&f["point"]).or_else(|| point(&f["centroid"])).into_iter().collect();
        reply["faces_total"] = json!(faces["faces"].as_array().map_or(0, Vec::len));
        reply["faces"] = json!(narrow(&faces["faces"], &keep, &spots));
    }
    Ok(reply)
}

/// The command catalog filtered by a case-insensitive substring of id, label, panel or tab
/// (`list_commands`; not the parameter docs, which name other commands, #269).
fn filter_commands(all: Value, filter: Option<&str>) -> Value {
    let Some(f) = filter.map(str::to_lowercase).filter(|f| !f.is_empty()) else { return all };
    let hit =
        |c: &Value| ["id", "label", "panel", "tab"].iter().any(|k| c.get(*k).and_then(Value::as_str).is_some_and(|s| s.to_lowercase().contains(&f)));
    Value::Array(all.as_array().map(|v| v.iter().filter(|c| hit(c)).cloned().collect()).unwrap_or_default())
}

/// `inspect_design` with no arguments: what an agent needs to orient itself, small on any design
/// (#266). Any argument gives the whole design.
fn design_summary(b: &mut dyn Backend) -> Result<Value, String> {
    let d = b.call("document.inspect", json!({"measure": false}))?;
    let list = |k: &str| d[k].as_array().cloned().unwrap_or_default();
    let (timeline, bodies, sketches, params) = (list("timeline"), list("bodies"), list("sketches"), list("params"));
    let components = d["components"].as_array().cloned().unwrap_or_default();
    let joints = exec(b, "joint.inspect", json!({})).ok().and_then(|j| j["joints"].as_array().map(Vec::len)).unwrap_or(0);
    let flagged: Vec<Value> = timeline
        .iter()
        .filter(|f| !f["error"].is_null() || !f["warning"].is_null())
        .map(|f| json!({"id": f["id"], "name": f["name"], "type": f["type"], "error": f["error"], "warning": f["warning"]}))
        .collect();
    let param_rows: Vec<Value> = params
        .iter()
        .map(|p| {
            let mut r = json!({"name": p["name"], "expression": p["expression"], "value": p["value"]});
            if let Some(e) = p.get("error").filter(|e| !e.is_null()) {
                r["error"] = e.clone();
            }
            r
        })
        .collect();
    let top: Vec<Value> = components.iter().filter(|c| c["parent"].is_null()).map(|c| c["name"].clone()).collect();
    Ok(json!({
        "name": d["name"],
        "path": d["path"],
        "units": d["units"],
        "dirty": d["dirty"],
        "counts": {
            "features": timeline.len(),
            "bodies": bodies.len(),
            "sketches": sketches.len(),
            "components": components.len(),
            "joints": joints,
            "params": params.len(),
        },
        "params": param_rows,
        "features_with_errors": flagged,
        "bodies": bodies.iter().map(|x| x["name"].clone()).collect::<Vec<_>>(),
        "components": top,
        "active_sketch": d["active_sketch"],
        "undo": d["undo"],
        "summary": "inspect_design with any argument (measure, sketches, joints) gives the whole design; page_path pages one of its lists",
    }))
}

/// `pending`: the end of a request still running when its call returned (bridged only; a
/// headless session runs every request to its end).
fn pending(b: &mut dyn Backend) -> Result<Value, String> {
    let p = b.call("control.pending", json!({}))?;
    let what = p.get("pending").cloned().unwrap_or(Value::Null);
    if what.is_null() {
        return Ok(json!({"pending": null, "note": "nothing is running"}));
    }
    let name = what.as_str().unwrap_or("the request").to_string();
    if p.get("lost").and_then(Value::as_bool) == Some(true) {
        return Err(format!(
            "the connection to the app was lost while {name} was running, and its result with it: inspect the design (inspect_design) \
             to see whether it applied before sending it again"
        ));
    }
    if p.get("done").and_then(Value::as_bool) == Some(true) {
        let reply = p.get("reply").cloned().unwrap_or(Value::Null);
        return match reply.get("ok").and_then(Value::as_bool) {
            Some(true) => Ok(json!({"pending": what, "done": true, "result": reply.get("result").cloned().unwrap_or(Value::Null)})),
            _ => Err(format!("{name} failed: {}", reply.get("error").and_then(Value::as_str).unwrap_or("no reply"))),
        };
    }
    Ok(json!({"pending": what, "done": false, "running_s": p.get("running_s"), "note": "still running: call pending again to keep waiting"}))
}

/// A `batch` step as the engine command it runs: an engine command as it is, or one of these
/// tools' names (an agent writes the tool it knows) with that tool's arguments as its params.
/// `Ok((command, params, filter))`: `filter` narrows a `list_commands` step's catalog; an empty
/// `command` means the step is an engine command already.
fn batch_step(step: &Value) -> Result<(String, Value, Option<String>), String> {
    let command = step.get("command").or_else(|| step.get("id")).and_then(Value::as_str).ok_or("missing `command`")?;
    let empty = Map::new();
    let params = step.get("params").cloned().filter(|p| !p.is_null()).unwrap_or(json!({}));
    let a = params.as_object().unwrap_or(&empty);
    let mapped = |c: &str, p: Value| Ok((c.to_string(), p, None));
    match command {
        "list_commands" => Ok(("engine.commands".into(), json!({}), str_arg(a, "filter").map(str::to_string))),
        "set_parameter" => {
            let mut p = pick(a, &["name", "unit", "comment"]);
            p["expression"] = parameter_value(a)?;
            mapped("parameters.change", p)
        }
        "describe_command" => mapped("engine.describe", json!({"command": str_arg(a, "id").ok_or("a `describe_command` step needs params {id}")?})),
        "execute" => {
            let inner = str_arg(a, "command").ok_or("an `execute` step needs params {command, params}")?;
            if !inner.contains('.') && TOOL_STEPS.contains(&inner) {
                return Err(format!("an `execute` step runs an engine command, not the `{inner}` tool"));
            }
            mapped(inner, a.get("params").cloned().filter(Value::is_object).unwrap_or(json!({})))
        }
        "measure" => match a.get("items") {
            Some(items) => mapped("inspect.measure", json!({"items": items})),
            None => mapped("inspect.measure", str_arg(a, "body").map_or(json!({}), |body| json!({"bodies": [body]}))),
        },
        "inspect_design" => mapped("document.inspect", json!({"measure": a.get("measure").and_then(Value::as_bool).unwrap_or(true)})),
        "undo" => mapped("edit.undo", pick(a, &["steps", "to"])),
        "redo" => mapped("edit.redo", pick(a, &["steps"])),
        "export" => {
            mapped("file.export", pick(a, &["path", "format", "bodies", "mesh_prefix", "effort", "velocity", "floating", "scene", "kp", "actuated"]))
        }
        "save" => mapped("file.save", pick(a, &["path", "format"])),
        "open" => mapped("doc.open", pick(a, &["path", "up", "names"])),
        "new_design" => mapped("file.new", pick(a, &["name"])),
        "crash_reports" => match a.get("id") {
            Some(_) => mapped("diagnostics.crash_report", pick(a, &["id"])),
            None => mapped("diagnostics.crash_reports", pick(a, &["limit"])),
        },
        "body_topology" => Err("`body_topology` is a tool, not a command: in a batch use model.edges {body} and model.faces {body}".into()),
        "screenshot" => Err("`screenshot` is a tool, not a command: call it after the batch".into()),
        "batch" => Err("batches don't nest: put the steps in this batch".into()),
        "pending" => Err("`pending` is a tool, not a command: call it on its own".into()),
        "running" => Err("`running` is a tool, not a command: call it on its own".into()),
        "skill_reference" => Err("`skill_reference` is a tool, not a command: call it on its own".into()),
        "polish" => Err("`polish` is a tool, not a command: call it on its own (in a batch: inspect.polish or polish.apply)".into()),
        // An engine command: the step goes as it is.
        _ => Ok((String::new(), Value::Null, None)),
    }
}

/// The tool names a `batch` step may not be (or is mapped from): `execute` must not wrap them.
const TOOL_STEPS: &[&str] = &[
    "batch",
    "body_topology",
    "crash_reports",
    "describe_command",
    "execute",
    "export",
    "inspect_design",
    "list_commands",
    "measure",
    "new_design",
    "open",
    "pending",
    "polish",
    "redo",
    "running",
    "save",
    "screenshot",
    "set_parameter",
    "skill_reference",
    "undo",
];

/// `set_parameter`'s new value: `value` or `expression` (the parameters.change name), one of
/// them, or both when they agree (`50` and `"50"` do).
fn parameter_value(a: &Map<String, Value>) -> Result<Value, String> {
    let text = |v: &Value| match v {
        Value::String(s) => s.trim().to_string(),
        v => v.to_string(),
    };
    match (a.get("value").filter(|v| !v.is_null()), a.get("expression").filter(|v| !v.is_null())) {
        (Some(v), None) | (None, Some(v)) => Ok(v.clone()),
        (Some(v), Some(e)) if text(v) == text(e) => Ok(v.clone()),
        (Some(_), Some(_)) => Err("set_parameter: `value` and `expression` disagree; give one of them".into()),
        (None, None) => Err("set_parameter: give the new value as `value` (or `expression`): a number (mm) or an expression".into()),
    }
}

fn undo_depth(b: &mut dyn Backend) -> Option<u64> {
    b.call("document.inspect", json!({})).ok().and_then(|d| d["undo"].as_u64())
}

/// How a failed batch goes back to where it started.
enum Undo {
    /// An engine checkpoint: the design, active sketch and undo history exactly as they were.
    Checkpoint(u64),
    /// An app without checkpoints: undo as many steps as the batch added (wrong once the undo
    /// history is full, as its depth no longer grows).
    Depth(u64),
    None,
}

impl Undo {
    fn begin(b: &mut dyn Backend) -> Undo {
        match exec(b, "edit.checkpoint", json!({})).ok().and_then(|v| v["checkpoint"].as_u64()) {
            Some(id) => Undo::Checkpoint(id),
            None => undo_depth(b).map_or(Undo::None, Undo::Depth),
        }
    }

    /// Go back: whether the design really is back where the batch started, and the number of
    /// steps undone. A failed rollback says so rather than reporting the flag the caller asked
    /// for (#3242).
    fn rollback(self, b: &mut dyn Backend, steps: usize) -> (bool, u64) {
        match self {
            Undo::Checkpoint(id) => match exec(b, "edit.restore_checkpoint", json!({"checkpoint": id})) {
                Ok(_) => (true, steps as u64),
                Err(_) => (false, 0),
            },
            Undo::Depth(before) => {
                let Some(after) = undo_depth(b) else { return (false, 0) };
                let mut undone = 0u64;
                for _ in before..after {
                    if exec(b, "edit.undo", json!({})).is_err() {
                        break;
                    }
                    undone += 1;
                }
                (undone == after.saturating_sub(before), undone)
            }
            // Nothing remembered the design; only a first step that failed left nothing behind.
            Undo::None => (steps == 0, 0),
        }
    }

    /// A step failed and the batch keeps what it finished (`on_error: "keep"`): those steps
    /// become one undo step. `false` when they couldn't be (an app without checkpoints or
    /// `edit.squash_checkpoint`): they stay, each its own step.
    fn keep(self, b: &mut dyn Backend) -> bool {
        let Undo::Checkpoint(id) = self else { return false };
        match exec(b, "edit.squash_checkpoint", json!({"checkpoint": id, "label": "Batch"})) {
            Ok(_) => true,
            Err(_) => {
                Undo::Checkpoint(id).finish(b);
                false
            }
        }
    }

    /// The batch went through, `steps` of them: one undo step (#270), as the app's `engine.batch`
    /// makes it. A batch of one keeps its command's label, and an app that can't squash keeps
    /// the steps as they are.
    fn done(self, b: &mut dyn Backend, steps: usize) {
        if let Undo::Checkpoint(id) = self
            && steps > 1
            && exec(b, "edit.squash_checkpoint", json!({"checkpoint": id, "label": "Batch"})).is_ok()
        {
            return;
        }
        self.finish(b);
    }

    /// Forget the checkpoint.
    fn finish(self, b: &mut dyn Backend) {
        if let Undo::Checkpoint(id) = self {
            let _ = exec(b, "edit.restore_checkpoint", json!({"checkpoint": id, "forget": true}));
        }
    }
}

/// A joint change's reply with the errors and conflicts of the joints it changed, and only a count
/// of the rest under `elsewhere` (#266). The engine answers each joint command with the whole
/// assembly's, so a run of them repeated the same broken joints at every step; `joint.list` and
/// `joint.solve` still give them all.
fn joint_errors_touched(command: &str, params: &Value, mut v: Value) -> Value {
    let touched: Vec<&Value> = match command {
        "joint.create" | "joint.as_built" => vec![&v["joint"], &v["name"]],
        "joint.edit" | "joint.limits" => vec![&params["joint"], &params["name"]],
        "joint.delete" => vec![],
        _ => return v,
    };
    // A joint is named by id or name; an id may come as a string.
    let touched: Vec<Value> = touched
        .into_iter()
        .filter(|t| !t.is_null())
        .flat_map(|t| [Some(t.clone()), t.as_str().and_then(|x| x.trim().parse::<u64>().ok()).map(Value::from)])
        .flatten()
        .collect();
    let mine = |item: &Value| touched.iter().any(|t| *t == item["joint"] || *t == item["name"]);
    let Some(o) = v.as_object_mut() else { return v };
    let mut elsewhere = Map::new();
    for key in ["errors", "conflicts"] {
        let Some(Value::Array(list)) = o.get_mut(key) else { continue };
        let total = list.len();
        list.retain(|item| mine(item));
        if total > list.len() {
            elsewhere.insert(key.into(), json!(total - list.len()));
        }
    }
    if !elsewhere.is_empty() {
        elsewhere.insert("see".into(), json!("joint.list (or joint.solve) lists every joint's errors and conflicts"));
        o.insert("elsewhere".into(), Value::Object(elsewhere));
    }
    v
}

/// A result cut down to the dotted `select` paths (`*` for every item of a list or every value of
/// an object), merged into one value shaped like the result; paths that match nothing are listed in
/// `missing_paths`. No `select` (or one that is not a list of strings) leaves the result whole.
fn selected(v: Value, select: Option<&Value>) -> Value {
    let Some(paths) = select.and_then(Value::as_array).filter(|p| !p.is_empty()) else { return v };
    let paths: Vec<&str> = paths.iter().filter_map(Value::as_str).collect();
    if paths.is_empty() {
        return v;
    }
    let mut out = Value::Null;
    let mut missing = Vec::new();
    for path in paths {
        let segs: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        match take(&v, &segs) {
            Some(part) => merge(&mut out, part),
            None => missing.push(path),
        }
    }
    if !missing.is_empty() {
        if out.is_null() {
            out = json!({});
        }
        if let Some(o) = out.as_object_mut() {
            o.insert("missing_paths".into(), json!(missing));
        }
    }
    out
}

/// The part of `v` at `segs`, in the shape it has in `v`.
fn take(v: &Value, segs: &[&str]) -> Option<Value> {
    let Some((first, rest)) = segs.split_first() else { return Some(v.clone()) };
    match (first, v) {
        (&"*", Value::Array(items)) => {
            let parts: Vec<Value> = items.iter().map(|x| take(x, rest).unwrap_or(Value::Null)).collect();
            parts.iter().any(|p| !p.is_null()).then_some(Value::Array(parts))
        }
        (&"*", Value::Object(o)) => {
            let parts: Map<String, Value> = o.iter().filter_map(|(k, x)| take(x, rest).map(|x| (k.clone(), x))).collect();
            (!parts.is_empty()).then_some(Value::Object(parts))
        }
        (key, Value::Object(o)) => o.get(*key).and_then(|x| take(x, rest)).map(|x| json!({ *key: x })),
        // A name met at a list is asked of each of its items (`bodies.name` is `bodies.*.name`).
        (key, Value::Array(_)) if key.parse::<usize>().is_err() => take(v, &[&["*"], segs].concat()),
        (index, Value::Array(items)) => index.parse::<usize>().ok().and_then(|i| items.get(i)).and_then(|x| take(x, rest)).map(|x| {
            // An index keeps its place: the items before it are null.
            let i = index.parse::<usize>().unwrap_or(0);
            let mut list = vec![Value::Null; i];
            list.push(x);
            Value::Array(list)
        }),
        _ => None,
    }
}

/// Fold `add` into `into`: objects key by key, lists item by item.
fn merge(into: &mut Value, add: Value) {
    match (into, add) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, v) in b.into_iter().enumerate() {
                match a.get_mut(i) {
                    Some(slot) => merge(slot, v),
                    None => a.push(v),
                }
            }
        }
        (slot, add) => {
            if !add.is_null() || slot.is_null() {
                *slot = add;
            }
        }
    }
}

/// `batch`: run steps in order, stop at the first failure and (by default) go back to the
/// design as it was before the batch.
fn batch(b: &mut dyn Backend, a: &Map<String, Value>) -> Value {
    let Some(steps) = a.get("commands").and_then(Value::as_array) else {
        return text_result(&json!("`commands` must be an array of {command, params}"), true);
    };
    if steps.len() > MAX_BATCH {
        return text_result(&json!(format!("batch too long ({} steps, at most {MAX_BATCH})", steps.len())), true);
    }
    // `on_error: "keep"` keeps the finished steps of a failed batch as one undo step (#269).
    let keep = match a.get("on_error").and_then(Value::as_str) {
        None | Some("rollback") => false,
        Some("keep") => true,
        Some(other) => return text_result(&json!(format!("`on_error` is \"rollback\" or \"keep\", not {other:?}")), true),
    };
    if keep && a.get("rollback").and_then(Value::as_bool) == Some(true) {
        return text_result(&json!("`on_error: \"keep\"` keeps the finished steps and `rollback: true` takes them back: give one of them"), true);
    }
    let rollback = !keep && a.get("rollback").and_then(Value::as_bool).unwrap_or(true);
    let verbose = a.get("verbose").and_then(Value::as_bool).unwrap_or(true);
    let profile = a.get("profile").and_then(Value::as_bool).unwrap_or(false);
    let timed = profile || a.get("timings").and_then(Value::as_bool).unwrap_or(false);
    // Steps that name a tool run the engine command it stands for. One that can't be a step
    // fails the batch before anything is sent.
    let mut mapped: Vec<Value> = Vec::with_capacity(steps.len());
    let mut filters: Vec<(usize, String)> = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        match batch_step(step) {
            Ok((command, _, _)) if command.is_empty() => {
                let mut step = step.clone();
                let id = step.get("command").or_else(|| step.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
                // Only a step the agent defaults change is rewritten; the rest go as they are.
                if id == "sketch.edit"
                    && let Some(o) = step.as_object_mut()
                {
                    let params = o.remove("params").filter(|p| !p.is_null()).unwrap_or(json!({}));
                    o.insert("params".into(), agent_params(&id, params));
                }
                mapped.push(step);
            }
            Ok((command, params, filter)) => {
                if let Some(f) = filter {
                    filters.push((i, f));
                }
                let params = agent_params(&command, params);
                mapped.push(json!({"command": command, "params": params}));
            }
            // A step without a command fails where it stands, as before.
            Err(e) if e == "missing `command`" => mapped.push(step.clone()),
            Err(e) => {
                let command = step.get("command").or_else(|| step.get("id")).and_then(Value::as_str);
                let reply = json!({
                    "ok": false,
                    "completed": 0,
                    "failed": {"index": i, "command": command, "error": e, "kind": "not_a_step"},
                    "rolled_back": false,
                    "undone": 0,
                    "applied": [],
                    "not_run": (0..steps.len()).collect::<Vec<_>>(),
                    "results": [],
                    "note": "nothing was run",
                });
                return text_result(&reply, true);
            }
        }
    }
    // An unknown command fails the batch before anything runs (#269), not after the steps before
    // it ran and were taken back.
    if let Some((i, id)) = unknown_step(&*b, &mapped) {
        let error = command_error(&id, polykrom_engine::EngineError::UnknownCommand(id.clone()).to_string());
        let reply = json!({
            "ok": false,
            "completed": 0,
            "failed": {"index": i, "command": id, "error": error, "kind": "unknown_command"},
            "rolled_back": false,
            "undone": 0,
            "applied": [],
            "not_run": (0..steps.len()).collect::<Vec<_>>(),
            "results": [],
            "note": "nothing was run",
        });
        return text_result(&reply, true);
    }
    // Each step's `select` (as the agent wrote the step) is applied here, to its result: the app is
    // sent the steps without it.
    let selects: Vec<Option<Value>> = steps.iter().map(|st| st.get("select").cloned()).collect();
    let steps = &mapped;
    // A `list_commands` step's catalog, filtered as the tool filters it.
    // A joint change's step, with the errors of the joints it changed (the others counted).
    let narrow = |i: usize, v: Value| {
        let v = match filters.iter().find(|(j, _)| *j == i) {
            Some((_, f)) => filter_commands(v, Some(f)),
            None => v,
        };
        match steps.get(i) {
            Some(st) => {
                let command = st.get("command").or_else(|| st.get("id")).and_then(Value::as_str).unwrap_or_default();
                joint_errors_touched(command, st.get("params").unwrap_or(&Value::Null), v)
            }
            None => v,
        }
    };
    // An app that runs batches itself answers the whole batch in one request, so one frame
    // (#743). One too long for a control line goes step by step: the app would refuse it unrun.
    // The margin covers the envelope and the `allow_paths` the backend adds.
    let sent: Vec<Value> = steps
        .iter()
        .map(|st| match st.as_object() {
            Some(o) => Value::Object(o.iter().filter(|(k, _)| k.as_str() != "select").map(|(k, v)| (k.clone(), v.clone())).collect()),
            None => st.clone(),
        })
        .collect();
    let mut params = json!({"commands": sent, "rollback": rollback});
    if timed && let Some(o) = params.as_object_mut() {
        o.insert("timings".into(), Value::Bool(true));
        o.insert("profile".into(), Value::Bool(profile));
    }
    if params.to_string().len() + 64 * 1024 <= polykrom_engine::MAX_LINE {
        let kept = if keep { Undo::begin(b) } else { Undo::None };
        match b.call("engine.batch", params) {
            Ok(mut r) => {
                let kept = if r.get("failed").is_some_and(|f| !f.is_null()) {
                    keep.then(|| kept.keep(b))
                } else {
                    kept.finish(b);
                    None
                };
                // Each finished step's `removed_bodies`, to say where a body a failed step wanted went.
                let removed: Vec<Value> =
                    r["results"].as_array().map(|v| v.iter().map(|x| x["removed_bodies"].clone()).collect()).unwrap_or_default();
                if let Some(failed) = r.get_mut("failed")
                    && let (Some(command), Some(error)) = (failed.get("command").and_then(Value::as_str), failed.get("error").and_then(Value::as_str))
                {
                    let error = command_error(command, error.to_string()) + &consumed_by(error, &removed, steps);
                    if let Some(failed) = failed.as_object_mut() {
                        failed.insert("error".into(), Value::String(error));
                    }
                }
                return batch_reply(&r, steps.len(), rollback, verbose, &narrow, &selects, kept);
            }
            // Older apps and the headless session, or the app wants the steps one by one (which
            // take a checkpoint of their own).
            Err(e) if e.starts_with("unknown method") || e.starts_with("engine.batch unavailable") => kept.finish(b),
            // Never sent: no app to send it to. Or sent and still running (it was not cancelled):
            // the `pending` tool gets its result.
            Err(e) if e.starts_with(crate::backend::NOT_RUNNING) || e.starts_with(STILL_RUNNING) => {
                return text_result(&Value::String(e), true);
            }
            // The app may have run it: sending the steps again would apply them twice.
            Err(e) => {
                let msg = format!("the app did not answer engine.batch ({e}); it may have run: inspect the design before sending it again");
                return text_result(&Value::String(msg), true);
            }
        }
    }
    let start = if rollback || keep { Undo::begin(b) } else { Undo::None };
    let mut results = Vec::with_capacity(steps.len());
    // Each finished step's `removed_bodies`, whatever its `select` keeps, to say where a body went.
    let mut removed: Vec<Value> = Vec::with_capacity(steps.len());
    let mut rows = Vec::new();
    let clock = timing::Clock::start();
    // The kernel's share can only be measured where the commands run: in this process.
    let profile = profile && b.in_process();
    for (i, step) in steps.iter().enumerate() {
        let command = step.get("command").or_else(|| step.get("id")).and_then(Value::as_str);
        // Cancelled (notifications/cancelled, #264): stop before the next step and take back
        // the ones run.
        if crate::call::cancelled() {
            let (went_back, undone) = start.rollback(b, i);
            let reply = json!({
                "ok": false,
                "cancelled": true,
                "completed": i,
                "rolled_back": went_back,
                "undone": undone,
                "applied": if went_back { Vec::new() } else { (0..i).collect::<Vec<_>>() },
                "not_run": (i..steps.len()).collect::<Vec<_>>(),
                "results": results,
            });
            return text_result(&with_timings(reply, timed, rows, clock.ms()), true);
        }
        let (r, took) = timing::time(profile, || match command {
            Some(id) => exec(b, id, step.get("params").cloned().filter(|p| !p.is_null()).unwrap_or(json!({}))),
            None => Err("missing `command`".to_string()),
        });
        if timed {
            rows.push(took.to_json(i, command.unwrap_or_default()));
        }
        match r {
            Ok(v) => {
                removed.push(v.get("removed_bodies").cloned().unwrap_or(Value::Null));
                let v = narrow(i, v);
                results.push(selected(if verbose { v } else { compact(v) }, selects.get(i).and_then(Option::as_ref)))
            }
            Err(e) => {
                let hint = consumed_by(&e, &removed, steps);
                let e = e + &hint;
                // A step still running in the app was not cancelled: the rollback first waits for
                // it (the backend does), so a restored checkpoint takes it back too.
                let running = e.starts_with(STILL_RUNNING);
                let (kept, (went_back, undone)) = if keep { (Some(start.keep(b)), (false, 0)) } else { (None, start.rollback(b, i)) };
                // Only a rollback asked for is reported as one.
                let rolled_back = rollback && went_back;
                let mut failed = json!({"index": i, "command": command, "error": e});
                // Applied: the steps before it, unless rolled back; and the step itself while it
                // still runs, as it will apply when it finishes.
                let mut applied: Vec<usize> = if rolled_back { Vec::new() } else { (0..i).collect() };
                if running && !rolled_back {
                    failed["still_running"] = json!(true);
                    applied.push(i);
                }
                let reply = json!({
                    "ok": false,
                    "completed": i,
                    "failed": failed,
                    "rolled_back": rolled_back,
                    "undone": undone,
                    "applied": applied,
                    "not_run": (i + 1..steps.len()).collect::<Vec<_>>(),
                    "results": results,
                });
                let reply = with_kept(reply, kept);
                return text_result(&with_timings(reply, timed, rows, clock.ms()), true);
            }
        }
    }
    start.done(b, results.len());
    let reply = json!({"ok": true, "completed": results.len(), "results": results});
    text_result(&with_timings(reply, timed, rows, clock.ms()), false)
}

/// A failed batch's `reply` with what `on_error: "keep"` kept: the steps in `applied`, and
/// whether they are one undo step.
fn with_kept(mut reply: Value, kept: Option<bool>) -> Value {
    if let (Some(one), Some(o)) = (kept, reply.as_object_mut()) {
        o.insert("kept".into(), json!({"steps": o.get("applied").cloned().unwrap_or(json!([])), "one_undo_step": one}));
    }
    reply
}

/// `reply` with `timings` and `total_ms` when they were asked for.
fn with_timings(mut reply: Value, timed: bool, rows: Vec<Value>, total_ms: f64) -> Value {
    if timed && let Some(o) = reply.as_object_mut() {
        o.insert("timings".into(), Value::Array(rows));
        o.insert("total_ms".into(), json!(timing::round_ms(total_ms)));
    }
    reply
}

/// `undo` / `redo` with `steps` (#270). An app from before `steps` existed undoes one step and
/// answers without the `steps` list; the rest are then sent one at a time.
fn undo_steps(b: &mut dyn Backend, command: &str, a: &Map<String, Value>) -> Result<Value, String> {
    let want = a.get("steps").and_then(Value::as_u64).unwrap_or(1).max(1);
    let r = exec(b, command, pick(a, &["steps", "to"]))?;
    if want == 1 || a.contains_key("to") || r.get("steps").is_some() {
        return Ok(r);
    }
    let key = if command == "edit.undo" { "undone" } else { "redone" };
    let mut done = vec![r[key].clone()];
    for _ in 1..want {
        match exec(b, command, json!({})) {
            Ok(v) => done.push(v[key].clone()),
            Err(_) => break,
        }
    }
    Ok(json!({key: done.last(), "steps": done}))
}

/// `; `X` was consumed by step N (`command`)` for a step that failed on a body an earlier step
/// of the same batch removed (#271): agents read "no body `Arm_L`" and looked it up, when step 1's
/// `solid.combine` had joined it into another. `removed` is each finished step's
/// `removed_bodies`. Empty when the error names no body or no step removed it.
fn consumed_by(error: &str, removed: &[Value], steps: &[Value]) -> String {
    let Some(name) = error.split_once("no body `").and_then(|(_, rest)| rest.split_once('`')).map(|(n, _)| n) else {
        return String::new();
    };
    let step = removed.iter().enumerate().rev().find(|(_, r)| r.as_array().is_some_and(|r| r.iter().any(|b| b.as_str() == Some(name))));
    match step {
        Some((i, _)) => {
            let command = steps.get(i).and_then(|s| s.get("command").or_else(|| s.get("id"))).and_then(Value::as_str).unwrap_or("?");
            format!("; `{name}` was consumed by step {i} (`{command}`)")
        }
        None => String::new(),
    }
}

/// The first step naming a command that doesn't exist, as `(index, command)`, in a session this
/// process runs: its catalog is the one the steps run against. A bridged app may be of another
/// version, so its steps go as before and the app reports an unknown one where it stands.
fn unknown_step(b: &dyn Backend, steps: &[Value]) -> Option<(usize, String)> {
    if !b.in_process() {
        return None;
    }
    steps.iter().enumerate().find_map(|(i, st)| {
        let id = st.get("command").or_else(|| st.get("id"))?.as_str()?;
        polykrom_engine::find_command(id).is_none().then(|| (i, id.to_string()))
    })
}

/// The `batch` reply from the app's `engine.batch` answer, shaped as the per-step path shapes it.
#[allow(clippy::too_many_arguments)]
fn batch_reply(
    r: &Value,
    len: usize,
    rollback: bool,
    verbose: bool,
    narrow: &dyn Fn(usize, Value) -> Value,
    selects: &[Option<Value>],
    kept: Option<bool>,
) -> Value {
    let results: Vec<Value> = r["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i, narrow(i, v)))
        .map(|(i, v)| selected(if verbose { v } else { compact(v) }, selects.get(i).and_then(Option::as_ref)))
        .collect();
    let (mut reply, error) = match r.get("failed").filter(|f| !f.is_null()) {
        None => (json!({"ok": true, "completed": results.len(), "results": results}), false),
        Some(failed) => {
            let completed = r["completed"].as_u64().map_or(results.len(), |c| c as usize).min(len);
            // `rolled_back` tells whether the design went back, not whether a rollback was asked
            // for (#3242): an app that says so is believed; an older one went back when it undid
            // every completed step (none completed: nothing to take back).
            let went_back = r.get("rolled_back").and_then(Value::as_bool).unwrap_or_else(|| r["undone"].as_u64() == Some(completed as u64));
            let rolled_back = rollback && went_back;
            let applied: Vec<usize> = if rolled_back { Vec::new() } else { (0..completed).collect() };
            (
                json!({
                    "ok": false,
                    "completed": r["completed"],
                    "failed": failed,
                    "rolled_back": rolled_back,
                    "undone": r["undone"],
                    "applied": applied,
                    "not_run": (completed + 1..len).collect::<Vec<_>>(),
                    "results": results,
                }),
                true,
            )
        }
    };
    // The app timed the steps where they ran.
    if let Some(o) = reply.as_object_mut() {
        for k in ["timings", "total_ms"] {
            if let Some(v) = r.get(k).filter(|v| !v.is_null()) {
                o.insert(k.into(), v.clone());
            }
        }
    }
    let reply = with_kept(reply, kept);
    text_result(&reply, error)
}

/// A command's result without what the design already had (`execute` or `batch` with `verbose: false`): a
/// feature's full body list (its created, changed and removed bodies stay), and the sketch ids
/// a sketch command repeats outside `created`.
fn compact(mut v: Value) -> Value {
    let Some(o) = v.as_object_mut() else { return v };
    if o.contains_key("created_bodies") {
        o.remove("bodies");
    }
    if let Some(created) = o.get("created").cloned() {
        for k in ["points", "curves", "constraints", "dimensions"] {
            if o.get(k).is_some_and(|x| created.get(k) == Some(x)) {
                o.remove(k);
            }
        }
    }
    v
}

static SHOT: AtomicU64 = AtomicU64::new(0);

/// `polish`: inspect.polish, then (apply "safe") polish.apply, and with `views` four renders to
/// critique: front three-quarter, side, top and back three-quarter.
/// The `polish` tool's result: its report as text and, with `views`, the four renders as
/// images, so an agent without file access can look at them.
fn polish_result(b: &mut dyn Backend, a: &Map<String, Value>) -> Value {
    match polish(b, a) {
        Ok((v, pngs)) => {
            let mut r = text_result(&v, false);
            if let Some(content) = r["content"].as_array_mut() {
                content.extend(pngs.iter().map(|png| json!({"type": "image", "data": base64(png), "mimeType": "image/png"})));
            }
            r
        }
        Err(e) => text_result(&Value::String(e), true),
    }
}

fn polish(b: &mut dyn Backend, a: &Map<String, Value>) -> Result<(Value, Vec<Vec<u8>>), String> {
    let categories: Vec<&str> =
        a.get("categories").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let wanted = |v: &Value| categories.is_empty() || categories.iter().any(|c| v["category"] == *c);
    let mut out = match str_arg(a, "apply").unwrap_or("none") {
        "safe" => exec(b, "polish.apply", pick(a, &["bodies", "categories", "printed", "skip", "max_fixes", "style"]))?,
        // Recommendations are capped at 50: the ones asked for, before the cap.
        "none" => {
            let r = exec(b, "inspect.polish", pick(a, &["bodies", "printed", "skip", "max_fixes", "style"]))?;
            let findings: Vec<Value> = r["bodies"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|body| body["findings"].as_array().into_iter().flatten().map(move |f| json!({"body": body["body"], "category": f["category"], "severity": f["severity"], "kind": f["kind"], "message": f["message"]})))
                .chain(r["assembly"].as_array().into_iter().flatten().cloned())
                .filter(|v| wanted(v))
                .take(50)
                .collect();
            let score = json!({"score": r["score"], "scores": r["scores"]});
            json!({"before": score, "after": score, "applied": [], "skipped": [], "fixes": r["fixes"], "recommendations": findings, "bodies": r["bodies"],
                "style": r["style"], "colours": r["colours"]})
        }
        other => return Err(format!("`apply` is safe or none, not `{other}`")),
    };
    // Only the categories asked for, whichever way the pass ran.
    for k in ["fixes", "recommendations", "applied", "skipped"] {
        if let Some(list) = out.get_mut(k).and_then(Value::as_array_mut) {
            list.retain(|v| v.get("category").is_none() || wanted(v));
        }
    }
    if let Some(reference) = a.get("reference").filter(|v| !v.is_null()) {
        let mut p = json!({"reference": reference});
        if let Some(bodies) = a.get("bodies").filter(|v| !v.is_null()) {
            p["bodies"] = bodies.clone();
        }
        out["reference_match"] = exec(b, "inspect.compare_reference", p)?;
    }
    let mut pngs = Vec::new();
    if a.get("views").and_then(Value::as_bool).unwrap_or(false) {
        let dir = crate::scratch_dir().unwrap_or_else(vfs::temp_dir);
        let n = SHOT.fetch_add(1, Ordering::Relaxed);
        let mut views = Vec::new();
        for (name, how) in [("front_three_quarter", "iso"), ("side", "right"), ("top", "top"), ("back_three_quarter", "back_iso")] {
            let path = dir.join(format!("polish-{n}-{name}.png")).to_string_lossy().to_string();
            let mut args = Map::new();
            args.insert("path".into(), json!(path));
            args.insert("source".into(), json!("model"));
            if how == "back_iso" {
                // The iso camera turned half way round the model.
                let Ok(cam) = b.call("engine.execute", json!({"command": "view.apply", "params": {"view": "iso"}})) else { break };
                let cam = cam.get("camera").cloned().unwrap_or(cam);
                let mut c = json!({});
                for k in ["target", "yaw", "pitch", "distance", "fov"] {
                    if let Some(v) = cam.get(k).filter(|v| !v.is_null()) {
                        c[k] = v.clone();
                    }
                }
                c["yaw"] = json!(c["yaw"].as_f64().unwrap_or(0.0) + std::f64::consts::PI);
                args.insert("camera".into(), c);
            } else {
                args.insert("view".into(), json!(how));
            }
            // The fixes are in: a render that fails is reported, not the whole pass.
            match capture(b, &args) {
                Ok((png, _)) => pngs.push(png),
                Err(e) => {
                    out["views_error"] = json!(e);
                    break;
                }
            }
            views.push(json!({"view": name, "path": path}));
        }
        out["views"] = json!(views);
    }
    Ok((out, pngs))
}

/// `screenshot`: render or capture to a file, return it as an MCP image content block.
fn screenshot(b: &mut dyn Backend, a: &Map<String, Value>) -> Value {
    match capture(b, a) {
        Ok((png, info)) => json!({"content": [
            {"type": "image", "data": base64(&png), "mimeType": "image/png"},
            {"type": "text", "text": info.to_string()},
        ]}),
        Err(e) => text_result(&Value::String(e), true),
    }
}

fn capture(b: &mut dyn Backend, a: &Map<String, Value>) -> Result<(Vec<u8>, Value), String> {
    // `grid` and `axes` shape a model render; the window shows what it shows.
    let guides: Vec<(&str, bool)> = ["grid", "axes"].into_iter().filter_map(|k| a.get(k).and_then(Value::as_bool).map(|v| (k, v))).collect();
    let window = match str_arg(a, "source") {
        Some("model") => false,
        Some("window") if !guides.is_empty() => {
            return Err("screenshot: `grid` and `axes` apply to model renders; use source \"model\" (the window shows what it shows)".into());
        }
        Some("window") => true,
        _ => b.has_ui() && guides.is_empty(),
    };
    let apply = |b: &mut dyn Backend, params: Value| b.call("engine.execute", json!({"command": "view.apply", "params": params}));
    match (str_arg(a, "view"), a.get("camera").filter(|c| !c.is_null())) {
        (Some(_), Some(_)) => return Err("screenshot: give `view` or `camera`, not both".into()),
        (None, Some(c)) => {
            apply(b, json!({"camera": c}))?;
        }
        (Some(v), None) if VIEWS.contains(&v) => {
            b.call("ui.view", json!({"view": v}))?;
            // Turning to a standard view also frames the model (like a axis view click).
            if v != "fit" && v != "home" {
                b.call("ui.view", json!({"view": "fit"}))?;
            }
        }
        // A saved named view.
        (Some(v), None) => {
            apply(b, json!({"view": v}))?;
        }
        (None, None) => {}
    }
    let keep = str_arg(a, "path").filter(|p| !p.trim().is_empty()).map(str::to_string);
    let path = keep.clone().unwrap_or_else(|| {
        let n = SHOT.fetch_add(1, Ordering::Relaxed);
        let dir = crate::scratch_dir().unwrap_or_else(vfs::temp_dir);
        dir.join(format!("shot-{n}.png")).to_string_lossy().to_string()
    });
    let size = |k: &str, d: u64| a.get(k).and_then(Value::as_u64).unwrap_or(d).clamp(16, 4096);
    let render = |b: &mut dyn Backend| {
        let mut params = json!({"path": path, "width": size("width", 960), "height": size("height", 600)});
        for (k, v) in &guides {
            params[*k] = json!(v);
        }
        b.call("ui.render", params)
    };
    let asked_window = str_arg(a, "source") == Some("window");
    let mut fallback = None;
    let (info, source) = if window {
        match b.call("ui.screenshot", json!({"path": path})) {
            Ok(v) => (v, "window"),
            // Asked for the window: say why there is none, so the caller can show it or ask for
            // a model render (#1677).
            Err(e) if asked_window => {
                return Err(format!("screenshot: the window gave no picture ({e}); show the window, or use source \"model\""));
            }
            // No presented frame (minimised or hidden window): a model render, and the reply says so.
            Err(e) => {
                fallback = Some(e);
                (render(b)?, "model")
            }
        }
    } else {
        (render(b)?, "model")
    };
    let read = vfs::read(&path, MAX_IMAGE_BYTES).map_err(|e| format!("{path}: {e} (is the app on another machine?)"));
    if keep.is_none() {
        let _ = vfs::remove(&path);
    }
    let png = read?;
    let (w, h) = match source {
        "model" => (json!(size("width", 960)), json!(size("height", 600))),
        _ => (info.get("width").cloned().unwrap_or(Value::Null), info.get("height").cloned().unwrap_or(Value::Null)),
    };
    let bytes = png.len();
    let mut reply = json!({"source": source, "path": keep, "width": w, "height": h, "bytes": bytes});
    if let Some(why) = fallback {
        reply["fallback"] = json!(format!("the window gave no picture ({why}); this is a model render"));
    }
    // What drew a model render (gpu, gpu-software or cpu), and the warning a CPU render carries.
    for k in ["renderer", "warning"] {
        if let Some(v) = info.get(k).filter(|v| !v.is_null()) {
            reply[k] = v.clone();
        }
    }
    Ok((png, reply))
}

/// Standard base64 with padding (RFC 4648).
pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(T[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

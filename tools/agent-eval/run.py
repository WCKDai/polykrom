#!/usr/bin/env python3
"""Polykrom agent evaluation: physical-world parts and systems built by an AI companion over MCP.

Each task is a plain-language spec with an analytic answer. Claude Code runs headless with
`solidcraft-cli mcp` as its only tool, saves a design, and the harness grades the saved design
with `solidcraft-cli`: parts by body count, volume and bounding box; systems by replaying
commands on the design (joints, motion studies, interference and clearance, URDF, BOM). The
agent's own claims are never the verdict. See README.md.
"""
import argparse, json, math, os, subprocess, time
from pathlib import Path

PI = math.pi
TASKS = [
    {"id": "mounting_plate",
     "prompt": "Model a steel mounting plate: 100 mm long (X) by 60 mm wide (Y) by 6 mm thick (Z), lying on the XY plane. Round its four vertical corner edges with a 5 mm radius. Drill four 6.5 mm diameter through holes whose centres are 10 mm in from each of the two nearest edges.",
     "bodies": 1, "bbox": [100, 60, 6],
     "volume": 100*60*6 - 4*(25 - PI*25/4)*6 - 4*PI*3.25**2*6},
    {"id": "l_bracket",
     "prompt": "Model an L-shaped angle bracket as one solid body: a horizontal base 80 mm long (X), 40 mm deep (Y) and 5 mm thick, and a vertical upright along the base's back edge (the edge at the largest Y), 80 mm long, 5 mm thick and rising 45 mm above the top of the base (so the bracket is 50 mm tall overall). Put two 8 mm through holes in the base, at X = 20 mm and X = 60 mm from the base's left end, centred in the base's depth that is not under the upright.",
     "bodies": 1, "bbox": [80, 40, 50],
     "volume": 80*40*5 + 80*5*45 - 2*PI*16*5},
    {"id": "stepped_shaft",
     "prompt": "Model a stepped steel shaft along the Z axis: a 20 mm diameter section 90 mm long, followed by a 15 mm diameter section 30 mm long, as one solid.",
     "bodies": 1, "bbox": [20, 20, 120],
     "volume": PI*(100*90 + 7.5**2*30)},
    {"id": "flanged_tube",
     "prompt": "Model a flanged tube as one solid, axis along Z: the tube is 50 mm outside diameter, 40 mm inside diameter and 80 mm long overall. At its bottom end a flange 90 mm in diameter and 8 mm thick (the bore runs through it) carries six 9 mm bolt holes equally spaced on a 70 mm pitch circle.",
     "bodies": 1, "bbox": [90, 90, 80],
     "volume": PI*(45**2-20**2)*8 + PI*(25**2-20**2)*72 - 6*PI*4.5**2*8},
    {"id": "enclosure",
     "prompt": "Model an electronics enclosure: a box 120 mm by 80 mm by 40 mm tall, hollowed out with 2.5 mm walls and an open top (no lid).",
     "bodies": 1, "bbox": [120, 80, 40],
     "volume": 120*80*40 - 115*75*37.5},
    {"id": "hinge_urdf", "round": 2,
     "prompt": "Model a simple hinge as an assembly of two components for a robot simulator. Leaf A: a plate 40 mm (X) by 30 mm (Y) by 3 mm thick, grounded. Leaf B: an identical plate on the other side of the hinge line, so the two leaves lie flat side by side, meeting along a line parallel to X. Each leaf has a 8 mm diameter knuckle cylinder along that shared line (A's knuckle covers X 0-20, B's X 20-40). Join the leaves with a revolute joint about the knuckle axis, limited to 0-180 degrees, then export the assembly as URDF to {urdf}.",
     "urdf": {"links": 2, "revolute": 1}},
    {"id": "aluminium_bracket", "round": 2,
     "prompt": "Model an L-shaped angle bracket as one solid body: a horizontal base 80 mm long (X), 40 mm deep (Y) and 5 mm thick, and a vertical upright along the base's back edge (largest Y), 80 mm long, 5 mm thick, rising 45 mm above the base (50 mm tall overall). Make it of aluminium (6061 or plain aluminium from the material library) and report its mass in grams.",
     "bodies": 1, "bbox": [80, 40, 50], "volume": 80*40*5 + 80*5*45, "mass": [(80*40*5 + 80*5*45) * 2.70e-3, 0.03], "material": "alumin"},
    {"id": "pipe_elbow", "round": 2,
     "prompt": "Model a 90 degree pipe elbow as one solid: a tube 20 mm outside diameter with 2 mm walls, whose centreline is a quarter circle of 50 mm radius. The ends are flat and square to the centreline.",
     "bodies": 1, "bbox": [20, 60, 60], "volume": PI*(10**2 - 8**2) * (PI / 2 * 50)},
    {"id": "cup", "round": 2,
     "prompt": "Model a cylindrical cup by revolving its cross-section about the Z axis: 60 mm outside diameter, 80 mm tall, 3 mm side wall, 4 mm thick flat bottom, open top.",
     "bodies": 1, "bbox": [60, 60, 80], "volume": PI*(30**2*80 - 27**2*76)},
    {"id": "pocket_fillets", "round": 2,
     "prompt": "Model a block 60 mm (X) by 40 mm (Y) by 20 mm tall. Cut a rectangular pocket 30 mm by 20 mm, 10 mm deep, centred in the top face. Round the pocket's four vertical inside corner edges with a 3 mm radius (leave every other edge sharp).",
     "bodies": 1, "bbox": [60, 40, 20], "volume": 60*40*20 - 30*20*10 + 4*(9 - 9*PI/4)*10},
    {"id": "hole_pattern", "round": 2,
     "prompt": "Model a plate 120 mm (X) by 40 mm (Y) by 5 mm thick. Make one 6 mm through hole at X = 20, Y = 20 and repeat it with a linear pattern of 5 holes spaced 20 mm apart along X.",
     "bodies": 1, "bbox": [120, 40, 5], "volume": 120*40*5 - 5*PI*9*5},
    {"id": "design_arm", "round": 3,
     "prompt": "Design a steel cantilever arm that bolts to a wall: one solid body lying along +X from x = 0 mm (its wall face is the plane x = 0) to x = 150 mm, its cross-section centred on the X axis (y = 0, z = 0). A 500 N load pointing down (-Z) acts on its end face at x = 150. It must have a safety factor of at least 2 against yield and deflect less than 0.5 mm, with as little mass as you reasonably can. Make it of Steel. Check your design with the static stress analysis (analysis.static_stress: fixed on the x = 0 face, the load on the x = 150 face) and report the safety factor, the deflection and the mass.",
     "arm": {"length": 150, "load": 500, "sf": 2.0, "deflection": 0.5}},
    {"id": "spacer_param",
     "prompt": "Model a round spacer with a user parameter named `thickness` set to 3 mm: outside diameter 30 mm, a 10.5 mm centre hole, and its height driven by `thickness`. Then change `thickness` to 8 mm.",
     "bodies": 1, "bbox": [30, 30, 8],
     "volume": PI*(15**2-5.25**2)*8, "param": ["thickness", 8]},
    # Round 4: realistic parts, assemblies and editing (harder; finds the friction).
    {"id": "hex_nut", "round": 4,
     "prompt": "Model an M10 nut blank as one solid: a regular hexagonal prism 17 mm across the flats and 8 mm thick, its axis along Z, with two of its flats parallel to the X axis, and a 10 mm diameter through hole on its axis (no thread).",
     "bodies": 1, "bbox": [17 / math.cos(PI / 6), 17, 8], "volume": (math.sqrt(3) / 2 * 17**2 - PI * 25) * 8},
    {"id": "v_pulley", "round": 4,
     "prompt": "Model a V-belt pulley as one solid, axis along Z: 60 mm outside diameter, 20 mm wide (from z = 0 to z = 20), with a 10 mm centre bore. Around its rim cut a V-groove symmetric about z = 10: 10 mm wide at the outside diameter and 6 mm deep, so its two flanks meet at a point on a 48 mm diameter.",
     "bodies": 1, "bbox": [60, 60, 20], "volume": PI * (900 - 25) * 20 - 2 * PI * 28 * 30},
    {"id": "nema17_plate", "round": 4,
     "prompt": "Model a NEMA 17 stepper motor mounting plate: 60 mm by 60 mm in XY, 5 mm thick, with its four vertical corner edges rounded to a 4 mm radius. It has a 22 mm diameter hole in its centre for the motor's boss and four 3.4 mm holes on a 31 mm square bolt pattern centred on it.",
     "bodies": 1, "bbox": [60, 60, 5], "volume": (3600 - 4 * (16 - 4 * PI) - PI * 121 - 4 * PI * 1.7**2) * 5},
    {"id": "shelf_bracket", "round": 4,
     "prompt": "Model a shelf bracket as one solid: a horizontal arm 100 mm long (X from 0 to 100), 30 mm wide (Y from 0 to 30) and 6 mm thick (Z from 0 to 6); a vertical arm, the same width, at X from 0 to 6, from the horizontal arm up to Z = 100; and a triangular stiffening rib 6 mm thick, centred in the width (Y from 12 to 18), filling the inside corner, with one leg 60 mm along the horizontal arm's top (from x = 6) and the other 60 mm up the vertical arm's face (from z = 6).",
     "bodies": 1, "bbox": [100, 30, 100], "volume": 100 * 30 * 6 + 6 * 30 * 94 + 0.5 * 60 * 60 * 6},
    {"id": "board_tray", "round": 4,
     "prompt": "Model a tray for a circuit board as one solid: a box 100 mm (X) by 70 mm (Y) by 20 mm tall (Z), with one corner at the origin, hollowed out with 2 mm walls and a 2 mm floor and an open top. Inside, four cylindrical standoffs 6 mm in diameter stand on the floor and reach up to z = 8 mm, centred at x = 15 and 85 and y = 15 and 55. Each standoff has a 2.5 mm diameter hole down its axis, 6 mm deep from its top.",
     "bodies": 1, "bbox": [100, 70, 20], "volume": 100 * 70 * 20 - 96 * 66 * 18 + 4 * PI * 6 * (9 - 1.5625)},
    {"id": "box_and_lid", "round": 4,
     "prompt": "Model a small box with a separate lid as two bodies. The box: 60 mm (X) by 40 mm (Y) by 30 mm tall (Z from 0 to 30), hollowed out with 2 mm walls and a 2 mm floor and an open top. The lid: a plate 60 by 40 mm and 3 mm thick sitting on the box (Z from 30 to 33), with a lip under it: a rectangular ring 1.5 mm thick and 4 mm deep (Z from 26 to 30) whose outside fits inside the box's walls with 0.2 mm of clearance on every side.",
     "bodies": 2, "bbox": [60, 40, 33], "volume": 60 * 40 * 30 - 56 * 36 * 28 + 60 * 40 * 3 + (55.6 * 35.6 - 52.6 * 32.6) * 4},
    {"id": "lightened_wheel", "round": 4,
     "prompt": "Model a wheel as one solid, axis along Z: a disc 100 mm in diameter and 10 mm thick with a 20 mm centre bore, lightened by five identical 20 mm diameter holes equally spaced on a 60 mm diameter circle (use a circular pattern).",
     "bodies": 1, "bbox": [100, 100, 10], "volume": PI * (2500 - 100) * 10 - 5 * PI * 100 * 10},
    {"id": "param_table", "round": 4,
     "prompt": "Model a small table as one solid whose overall height is driven by a user parameter named `height`, first set to 450 mm. The top is 300 mm (X) by 200 mm (Y) by 15 mm thick, its upper face at Z = height; four square legs 20 by 20 mm stand under it from Z = 0, each flush with one corner of the top. Then change `height` to 500 mm and check the table followed.",
     "bodies": 1, "bbox": [300, 200, 500], "volume": 300 * 200 * 15 + 4 * 20 * 20 * 485, "param": ["height", 500]},
    {"id": "slider_urdf", "round": 4,
     "prompt": "Model a linear slide for a robot simulator as an assembly of two components. The rail: a bar 200 mm long (X), 20 mm wide and 10 mm tall, grounded. The carriage: a block 40 mm long, 30 mm wide and 15 mm tall sitting on top of the rail at its left end. Join them with a slider (prismatic) joint along X whose travel is limited to 0 to 150 mm, then export the assembly as URDF to {urdf}.",
     "urdf": {"links": 2, "prismatic": [0.0, 0.15]}},
    {"id": "edit_plate", "round": 4, "input": "input_mounting_plate.polykrom",
     "prompt": "Open the design {input}: a mounting plate 100 mm by 60 mm by 6 mm thick with 5 mm rounded corners and four 6.5 mm holes 10 mm in from the edges. Edit it: make the plate 10 mm thick and the four holes 8.5 mm in diameter, keeping everything else as it is.",
     "bodies": 1, "bbox": [100, 60, 10], "volume": (6000 - 4 * (25 - 25 * PI / 4) - 4 * PI * 4.25**2) * 10},

    # Round 5: systems: assemblies, joints, materials, fitment, motion and animation.
    {"id": "slider_crank", "round": 5, "system": "slider_crank",
     "prompt": "Build a slider-crank mechanism as an assembly of four components moving in the XY plane (every joint axis along Z). Base: a grounded Cast Iron plate 240 mm (X) by 60 mm (Y) by 10 mm thick, its top face at Z = 0, spanning X from -40 to 200 and Y from -30 to 30. Crank: a Steel bar pivoting on the base about the Z axis through the origin, with a 25 mm crank radius (its crank pin 25 mm from the pivot). Connecting rod: a Steel bar with 100 mm between its two pin centres, one end on the crank pin. Piston: an Aluminum block on the rod's other end (the wrist pin), guided along the X axis by a slider joint on the base. At crank angle 0 the crank points along +X, so the wrist pin is at X = 125. Make a motion study named Cycle that turns the crank one full revolution (0 to 360 degrees) over 72 steps, check that no two parts collide anywhere in the cycle, and export the cycle's animation as a GIF to {gif}. Report the piston's stroke."},
    {"id": "gear_pair", "round": 5, "system": "gear_pair",
     "prompt": "Build a spur gear test rig as an assembly. Base: a grounded Aluminum plate 140 mm (X) by 70 mm (Y) by 10 mm thick, spanning X 0 to 140, Y 0 to 70 and Z -10 to 0, with two vertical Steel shafts 10 mm in diameter standing on it from Z = 0 to Z = 40, at (35, 35) and (95, 35): a 60 mm centre distance. The shafts are part of the base (rigid with it). Driving gear: a Steel gear blank for a 40 mm pitch diameter gear, modelled as a 39 mm diameter disc 8 mm thick (Z 20 to 28) with a 10.2 mm bore, turning on the first shaft. Driven gear: the same for an 80 mm pitch diameter, a 79 mm disc with a 10.2 mm bore, on the second shaft at the same height. Each gear is its own component with a revolute joint about its shaft, and the two are coupled by a gear relation from their pitch radii, so the driven gear turns half as fast in the opposite direction. Make a motion study named Mesh that turns the driving gear two full turns over 100 steps and check that nothing collides."},
    {"id": "clevis_fit", "round": 5, "system": "clevis_fit",
     "prompt": "Model a clevis joint as an assembly for a fitment check. Fork (grounded, Steel), one body: a base block X -20 to 20, Y -16 to 16, Z 0 to 10, with two cheeks rising from its top to Z = 50, one at Y -16 to -8 and one at Y 8 to 16 (each X -20 to 20), each cheek with a 10.2 mm hole along Y centred at X = 0, Z = 35. Pin (Steel): a 10 mm diameter cylinder 32 mm long along Y, centred at X = 0, Y = 0, Z = 35, through both cheeks, rigidly joined to the fork. Link (Aluminum): a bar 15.6 mm thick (Y -7.8 to 7.8), 20 mm tall (Z 25 to 45), reaching from X = 0 to X = 80, with its end at X = 0 rounded to a 10 mm radius about the pin's axis and a 10.2 mm hole on that axis; at angle 0 it lies along +X. Join the link to the fork with a revolute joint about the pin's axis. Find, with a joint sweep, how far the link can turn each way from 0 before it touches the fork, and set the joint's limits to that range (within half a degree). Report the radial clearance between the pin and the link's hole and the two limits."},
    {"id": "bolted_plates", "round": 5, "system": "bolted_plates",
     "prompt": "Model a bolted joint as an assembly. Bottom plate: Steel, 80 mm (X) by 40 mm (Y) by 10 mm thick, spanning X 0 to 80, Y 0 to 40, Z 0 to 10, grounded, with four M6 tapped holes modelled as 5 mm diameter through holes at X = 10 and 70, Y = 10 and 30. Top plate: Aluminum, the same 80 by 40 footprint, 6 mm thick (Z 10 to 16), with four 6.4 mm clearance holes on the same centres. Fasten them with four M6 washers on the top plate's top face and four M6 x 16 socket head cap screws from the parts library, each screw seated on its washer. Export the bill of materials to {bom} as CSV."},

    # Round 6: harder systems: a closed-loop linkage with an analytic answer, and library bearings.
    {"id": "four_bar", "round": 6, "system": "four_bar",
     "prompt": "Build a Grashof crank-rocker four-bar linkage as an assembly moving in the XY plane (every joint axis along Z). Base: a grounded Aluminum plate X -20 to 100, Y -20 to 40, Z -10 to 0. Ground pivots O2 at (0, 0) and O4 at (80, 0). Crank (Steel): 30 mm between its pivot centres, from O2; coupler (Steel): 70 mm between its pivot centres; rocker (Steel): 60 mm between its pivot centres, from O4. Make each link a bar 10 mm wide that runs 5 mm past each of its pivot centres, the crank at Z 0 to 5, the coupler at Z 5 to 10 and the rocker at Z 10 to 15 so they pass over each other. Assemble it in the open configuration (the coupler-rocker pin above the X axis when the crank points along +X). Name the revolute joints O2 (base-crank), A (crank-coupler), B (coupler-rocker) and O4 (rocker-base). Make a motion study named Cycle that turns the crank one full turn in 72 steps and check that it runs clear of interference. Report the rocker's swing in degrees."},
    {"id": "bearing_shaft", "round": 6, "system": "bearing_shaft",
     "prompt": "Model a shaft running in two ball bearings as an assembly. Housing (grounded, Aluminum): a block X -40 to 40, Y -20 to 20, Z -20 to 20 with a 22 mm bore along the X axis right through it. Put a 608 ball bearing from the parts library in each end of the bore, flush with the housing's end faces. Shaft (Steel): an 8 mm diameter rod 120 mm long along the X axis, centred on the origin, through both bearings, joined to the housing by a revolute joint named Spin about the X axis. Make a motion study named Turn that turns the shaft one full turn in 36 steps and check it with motion interference (a part pressed or seated in another is not a collision of the motion). Export the bill of materials to {bom} as CSV and report the shaft's mass."},
]
SUFFIX = " Units are millimetres. When it is finished and you have checked it, save the design to {save}. Reply with one line saying what you built."

def measure(cli, design):
    run = subprocess.run([cli, "eval", str(design)], capture_output=True, text=True)
    if run.returncode != 0:
        return None, run.stderr.strip()[-300:]
    t = run.stdout; return json.loads(t[t.find("{"):]), ""

def check(task, d):
    bodies = [b for b in d.get("bodies", []) if b.get("visible", True) is not False]
    fails = []
    if len(bodies) != task["bodies"]:
        fails.append(f"bodies {len(bodies)} != {task['bodies']}")
    vol = sum(b.get("volume_mm3", 0) for b in bodies)
    if abs(vol - task["volume"]) > 2e-3 * task["volume"]:
        fails.append(f"volume {vol:.1f} != {task['volume']:.1f}")
    if bodies:
        lo = [min(b["bbox"]["min"][i] for b in bodies) for i in range(3)]
        hi = [max(b["bbox"]["max"][i] for b in bodies) for i in range(3)]
        size = sorted(h - l for h, l in zip(hi, lo))
        if any(abs(a - b) > 0.05 for a, b in zip(size, sorted(task["bbox"]))):
            fails.append(f"bbox {[round(x, 2) for x in size]} != {sorted(task['bbox'])}")
    if "mass" in task:
        want, rel = task["mass"]
        got = sum(b.get("mass_g", 0) for b in bodies)
        if abs(got - want) > rel * want:
            fails.append(f"mass {got:.1f} g != {want:.1f} g")
    if "material" in task and not any(task["material"] in str(b.get("material", "")).lower() for b in bodies):
        fails.append(f"material {[b.get('material') for b in bodies]}")
    if "param" in task:
        name, val = task["param"]
        p = next((p for p in d.get("params", []) if p.get("name") == name), None)
        if not p or abs((p.get("value") or 0) - val) > 1e-9:
            fails.append(f"parameter {name} = {p and p.get('value')}")
    return fails, round(vol, 2)

def check_arm(cli, task, save):
    """Re-run the analysis on the saved arm: fixed at x = 0, the load on the x = L end face."""
    d, err = measure(cli, save)
    if not d:
        return [f"eval failed: {err}"], None
    bodies = d.get("bodies", [])
    if len(bodies) != 1:
        return [f"bodies {len(bodies)}"], None
    b = bodies[0]; arm = task["arm"]; L = arm["length"]
    fails = []
    if "steel" not in str(b.get("material", "")).lower():
        fails.append(f"material {b.get('material')}")
    lo, hi = b["bbox"]["min"], b["bbox"]["max"]
    if abs(lo[0]) > 0.05 or abs(hi[0] - L) > 0.05:
        fails.append(f"x extent {lo[0]:.2f}..{hi[0]:.2f}")
    # A point on each end face: the centre, else just inside the section's outline (a tube's wall).
    candidates = [(0.0, 0.0), (0.0, hi[2] - 0.25), (0.0, lo[2] + 0.25), (hi[1] - 0.25, 0.0), (lo[1] + 0.25, 0.0)]
    res, last = None, ""
    for y, z in candidates:
        params = {"fixed": [[0, y, z]], "loads": [{"faces": [[L, y, z]], "force": [0, 0, -arm["load"]]}]}
        script = Path(save).with_name("check.json")
        script.write_text(json.dumps({"commands": [{"command": "analysis.static_stress", "params": params}]}))
        run = subprocess.run([cli, "run", str(script), "--in", str(save), "--results", "--quiet"], capture_output=True, text=True, env=dict(os.environ))
        if run.returncode == 0:
            t = run.stdout; res = json.loads(t[t.find("["):])[-1]; break
        last = run.stderr.strip()[-300:]
    if res is None:
        return fails + [f"analysis failed: {last}"], None
    out = {"safety_factor": res.get("safety_factor"), "deflection_mm": res.get("max_displacement_mm"), "mass_g": b.get("mass_g")}
    if not (out["safety_factor"] or 0) >= arm["sf"]:
        fails.append(f"safety factor {out['safety_factor']}")
    if not (out["deflection_mm"] or 1e9) < arm["deflection"]:
        fails.append(f"deflection {out['deflection_mm']}")
    return fails, out

def check_urdf(task, path):
    import xml.etree.ElementTree as ET
    if not path.exists():
        return ["no URDF written"]
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError as e:
        return [f"URDF does not parse: {e}"]
    links = root.findall("link"); joints = root.findall("joint")
    fails = []
    if len(links) < task["urdf"]["links"]:
        fails.append(f"links {len(links)}")
    if "prismatic" in task["urdf"]:
        lo, hi = task["urdf"]["prismatic"]
        pri = [j for j in joints if j.get("type") == "prismatic"]
        if not pri:
            fails.append(f"no prismatic joint in {[j.get('type') for j in joints]}")
        for j in pri:
            lim = j.find("limit")
            span = (float(lim.get("upper", "nan")) - float(lim.get("lower", "nan"))) if lim is not None else float("nan")
            if not abs(span - (hi - lo)) < 1e-4:
                fails.append(f"prismatic travel {span} m, want {hi - lo} m")
        return fails
    rev = [j for j in joints if j.get("type") == "revolute"]
    if len(rev) < task["urdf"]["revolute"]:
        fails.append(f"revolute joints {len(rev)} of {[j.get('type') for j in joints]}")
    for j in rev:
        lim = j.find("limit")
        if lim is None or abs(float(lim.get("upper", "nan")) - float(lim.get("lower", "nan")) - math.pi) > 1e-3:
            fails.append("revolute limit is not a 180 degree range")
    return fails

def run_cmds(cli, save, cmds):
    """Run commands on the saved design; their results, or an error string."""
    script = Path(save).with_name("grade.json"); out = Path(save).with_name("grade.out.json")
    script.write_text(json.dumps({"commands": cmds}))
    if out.exists(): out.unlink()
    run = subprocess.run([cli, "run", str(script), "--in", str(save), "--results", "--quiet", "--allow-errors", "--json-out", str(out)], capture_output=True, text=True)
    if not out.exists():
        return run.stderr.strip()[-400:] or "no results"
    return json.loads(out.read_text())

def joints(cli, save):
    r = run_cmds(cli, save, [{"command": "joint.list", "params": {}}])
    return r[0]["joints"] if isinstance(r, list) else []

def materials(d):
    return [str(b.get("material", "")).lower() for b in d.get("bodies", [])]

def need_materials(d, want):
    have = materials(d); fails = []
    for m, n in want.items():
        if sum(1 for x in have if x == m) < n:
            fails.append(f"want {n}x {m}, bodies are {have}")
    return fails

def gif_ok(path, frames):
    if not path.exists():
        return ["no GIF written"]
    b = path.read_bytes()
    if b[:6] not in (b"GIF89a", b"GIF87a"):
        return ["not a GIF"]
    n = b.count(b"\x21\xf9\x04")  # one graphic control extension per frame
    return [] if n >= frames else [f"GIF has {n} frames, want >= {frames}"]

def check_system(cli, task, save, work):
    d, err = measure(cli, save)
    if not d:
        return [f"eval failed: {err}"], None
    kind = task["system"]; js = joints(cli, save); fails = []; info = {}
    by_type = {}
    for j in js:
        by_type.setdefault(j.get("type"), []).append(j)
    info["joints"] = {k: len(v) for k, v in by_type.items()}
    if kind == "slider_crank":
        fails += need_materials(d, {"cast iron": 1, "steel": 2, "aluminum": 1})
        sl = by_type.get("slider", [])
        if len(sl) != 1 or len(by_type.get("revolute", [])) < 3:
            return fails + [f"joints {info['joints']}"], info
        r = run_cmds(cli, save, [{"command": "motion.play", "params": {"study": "Cycle", "step": 0}}, {"command": "joint.list", "params": {}},
                                 {"command": "motion.play", "params": {"study": "Cycle", "step": 36}}, {"command": "joint.list", "params": {}},
                                 {"command": "motion.interference", "params": {"study": "Cycle"}}])
        if not isinstance(r, list) or len(r) < 5:
            return fails + [f"replay failed: {r if isinstance(r, str) else r[-1]}"], info
        val = lambda lst: next(j for j in lst["joints"] if j["id"] == sl[0]["id"])["values"][0]["value"]
        stroke = abs(val(r[3]) - val(r[1])); info["stroke"] = round(stroke, 3)
        if abs(stroke - 50) > 0.05:
            fails.append(f"stroke {stroke:.3f} mm, want 50")
        i = r[4]; info["interference"] = {k: i.get(k) for k in ("clear", "samples", "stopped_at", "stopped_by_loop")}
        if not i.get("clear") or i.get("stopped_at") is not None or (i.get("samples") or 0) < 73:
            fails.append(f"interference {info['interference']}")
        fails += gif_ok(work / "cycle.gif", 50)
    elif kind == "gear_pair":
        fails += need_materials(d, {"aluminum": 1, "steel": 2})
        for vol in (PI / 4 * (39**2 - 10.2**2) * 8, PI / 4 * (79**2 - 10.2**2) * 8):
            if not any(abs(b.get("volume_mm3", 0) - vol) < 2e-3 * vol and "steel" in str(b.get("material", "")).lower() for b in d["bodies"]):
                fails.append(f"no Steel gear blank of {vol:.0f} mm3 in {[round(b.get('volume_mm3', 0)) for b in d['bodies']]}")
        rev = by_type.get("revolute", [])
        r = run_cmds(cli, save, [{"command": "motion.play", "params": {"study": "Mesh", "step": 0}}, {"command": "joint.list", "params": {}},
                                 {"command": "motion.play", "params": {"study": "Mesh", "step": 25}}, {"command": "joint.list", "params": {}},
                                 {"command": "motion.interference", "params": {"study": "Mesh"}}])
        if not isinstance(r, list) or len(r) < 5 or len(rev) < 2:
            return fails + [f"replay failed or joints {info['joints']}: {r if isinstance(r, str) else ''}"], info
        delta = sorted(abs(next(j for j in r[3]["joints"] if j["id"] == g["id"])["values"][0]["value"] - next(j for j in r[1]["joints"] if j["id"] == g["id"])["values"][0]["value"]) for g in rev)
        info["turns_deg"] = [round(x, 3) for x in delta]
        if len(delta) < 2 or abs(delta[-1] - 180) > 0.1 or abs(delta[-2] - 90) > 0.1:
            fails.append(f"quarter of the study turns the gears {info['turns_deg']} deg, want 180 and 90")
        i = r[4]; info["interference"] = {k: i.get(k) for k in ("clear", "samples", "stopped_at")}
        if not i.get("clear") or i.get("stopped_at") is not None:
            fails.append(f"interference {info['interference']}")
    elif kind == "clevis_fit":
        fails += need_materials(d, {"steel": 2, "aluminum": 1})
        rev = [j for j in by_type.get("revolute", []) if any(v.get("limits") for v in j["values"])]
        if len(rev) != 1:
            return fails + [f"want one revolute joint with limits, joints {[(j['type'], j['values']) for j in js]}"], info
        j = rev[0]; lo, hi = j["values"][0]["limits"]; info["limits"] = [lo, hi]
        r = run_cmds(cli, save, [{"command": "joint.limits", "params": {"joint": j["id"]}},
                                 {"command": "joint.sweep", "params": {"joint": j["id"], "from": 0, "to": 300}},
                                 {"command": "joint.sweep", "params": {"joint": j["id"], "from": 0, "to": -300}},
                                 {"command": "inspect.clearance", "params": {"nearest": 3}}])
        if not isinstance(r, list) or len(r) < 4:
            return fails + [f"replay failed: {r}"], info
        up, down = r[1].get("stopped_at"), r[2].get("stopped_at")
        info["stops"] = [down, up]
        if up is None or down is None:
            fails.append(f"the link meets nothing: sweeps {r[1].get('stopped_by')}, {r[2].get('stopped_by')}")
        else:
            if abs(hi - up) > 0.5 or abs(lo - down) > 0.5:
                fails.append(f"limits {[lo, hi]}, the sweep stops at {[down, up]}")
        # Pairs closer than the clearance (none at the default 0) and each body's nearest others.
        near = [n["distance_mm"] for n in r[3].get("near", [])] + [n["distance_mm"] for ns in r[3].get("nearest", {}).values() for n in ns]
        info["near_mm"] = sorted(round(x, 4) for x in near)[:6]
        if not any(abs(x - 0.1) < 0.01 for x in near):
            fails.append(f"no 0.1 mm radial clearance among {info['near_mm']}")
        if r[3].get("overlaps"):
            fails.append(f"overlaps {[(o['a'], o['b'], o['volume_mm3']) for o in r[3]['overlaps']]}")
    elif kind == "bolted_plates":
        fails += need_materials(d, {"steel": 1, "aluminum": 1})
        bom = work / "bom.csv"
        if not bom.exists():
            fails.append("no BOM written")
        else:
            import csv
            rows = list(csv.DictReader(bom.read_text().splitlines()))
            text = json.dumps(rows).lower(); info["bom"] = rows[:8]
            count = lambda word: sum(int(float(r.get("count") or r.get("Count") or r.get("qty") or 0)) for r in rows if word in json.dumps(r).lower())
            if count("washer") != 4: fails.append(f"washers in BOM {count('washer')}")
            if count("screw") != 4 or "m6" not in text: fails.append(f"M6 screws in BOM {count('screw')}")
        r = run_cmds(cli, save, [{"command": "inspect.clearance", "params": {"min_volume": 1}}])
        if isinstance(r, list) and r[0].get("overlaps"):
            fails.append(f"overlaps {[(o['a'], o['b'], round(o['volume_mm3'], 2)) for o in r[0]['overlaps']]}")
        info["bodies"] = len(d.get("bodies", []))
    elif kind == "four_bar":
        fails += need_materials(d, {"aluminum": 1, "steel": 3})
        names = {j.get("name") for j in by_type.get("revolute", [])}
        if not {"O2", "A", "B", "O4"} <= names:
            return fails + [f"revolute joints {sorted(n for n in names if n)}, want O2, A, B, O4"], info
        r = run_cmds(cli, save, [{"command": "motion.plot", "params": {"study": "Cycle", "joints": ["O2", "O4"], "samples": 73}},
                                 {"command": "motion.interference", "params": {"study": "Cycle"}}])
        if not isinstance(r, list) or len(r) < 2:
            return fails + [f"replay failed: {r if isinstance(r, str) else r[-1]}"], info
        series = {j["name"]: j.get("value", []) for j in r[0].get("joints", [])}
        span = lambda v: (max(v) - min(v)) if v else 0.0
        info["crank_deg"], info["rocker_swing_deg"] = round(span(series.get("O2", [])), 3), round(span(series.get("O4", [])), 3)
        if abs(info["crank_deg"] - 360) > 0.5:
            fails.append(f"the crank turns {info['crank_deg']} deg, want 360")
        if abs(info["rocker_swing_deg"] - 61.045) > 0.5:
            fails.append(f"rocker swing {info['rocker_swing_deg']} deg, want 61.045")
        i = r[1]; info["interference"] = {k: i.get(k) for k in ("clear", "samples", "stopped_at", "stopped_by_loop")}
        if not i.get("clear") or i.get("stopped_at") is not None:
            fails.append(f"interference {info['interference']} {[(c['a'], c['b']) for c in i.get('collisions', [])][:4]}")
    elif kind == "bearing_shaft":
        fails += need_materials(d, {"aluminum": 1, "steel": 3})
        spin = [j for j in by_type.get("revolute", []) if j.get("name") == "Spin"]
        if not spin:
            return fails + [f"no revolute joint named Spin: {info['joints']}"], info
        bodies = d.get("bodies", [])
        bearings = [b for b in bodies if "bearing" in b.get("name", "").lower() or "608" in b.get("name", "")]
        info["bearings"] = [(b["name"], [round(x, 2) for x in b["bbox"]["min"]], [round(x, 2) for x in b["bbox"]["max"]]) for b in bearings]
        ends = sorted(round(b["bbox"]["min"][0], 1) for b in bearings)
        if len(bearings) != 2 or not (abs(ends[0] + 40) < 0.5 and abs(sorted(round(b["bbox"]["max"][0], 1) for b in bearings)[-1] - 40) < 0.5):
            fails.append(f"bearings {info['bearings']}, want one flush in each end of the bore")
        shaft = [b for b in bodies if abs(b.get("volume_mm3", 0) - math.pi * 16 * 120) < 5]
        info["shaft_mass_g"] = round(shaft[0].get("mass_g", 0), 2) if shaft else None
        if not shaft or abs((shaft[0].get("mass_g") or 0) - math.pi * 16 * 120 * 7.85e-3) > 0.5:
            fails.append(f"shaft mass {info['shaft_mass_g']} g, want {math.pi * 16 * 120 * 7.85e-3:.2f}")
        r = run_cmds(cli, save, [{"command": "motion.interference", "params": {"study": "Turn"}}])
        if not isinstance(r, list):
            return fails + [f"replay failed: {r}"], info
        i = r[0]; info["interference"] = {k: i.get(k) for k in ("clear", "samples", "stopped_at", "static_overlaps", "mesh_contacts")}
        if not i.get("clear") or i.get("stopped_at") is not None:
            fails.append(f"interference clear={i.get('clear')} {[(c['a'], c['b'], c.get('volume_mm3')) for c in i.get('collisions', [])][:4]}")
        bom = work / "bom.csv"
        if not bom.exists():
            fails.append("no BOM written")
        else:
            import csv
            rows = list(csv.DictReader(bom.read_text().splitlines())); info["bom"] = rows[:6]
            n = sum(int(float(r.get("count") or 0)) for r in rows if "608" in json.dumps(r) or "bearing" in json.dumps(r).lower())
            if n != 2: fails.append(f"bearings in BOM {n}")
    return fails, info

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cli", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--only", default=""); ap.add_argument("--max-turns", type=int, default=60)
    ap.add_argument("--timeout", type=int, default=1200); ap.add_argument("--model", default=""); ap.add_argument("--round", type=int, default=0)
    a = ap.parse_args()
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    report = []
    for task in TASKS:
        if a.round and task.get("round", 1) != a.round:
            continue
        if a.only and task["id"] not in a.only.split(","):
            continue
        work = out / task["id"]; work.mkdir(exist_ok=True)
        save = work / f"{task['id']}.polykrom"
        if save.exists(): save.unlink()
        cfg = work / "mcp.json"
        cfg.write_text(json.dumps({"mcpServers": {"polykrom": {"command": a.cli, "args": ["mcp", "--allow-path", str(work)]}}}))
        if "input" in task:
            import shutil
            shutil.copy(Path(__file__).parent / task["input"], work / task["input"])
        prompt = task["prompt"].replace("{urdf}", str(work / "robot.urdf")).replace("{input}", str(work / task.get("input", ""))).replace("{gif}", str(work / "cycle.gif")).replace("{bom}", str(work / "bom.csv")) + SUFFIX.format(save=save)
        args = ["claude", "-p", prompt, "--mcp-config", str(cfg),
                "--strict-mcp-config", "--allowedTools", "mcp__polykrom", "--disallowedTools", "Bash,Read,Write,Edit,MultiEdit,NotebookEdit,Glob,Grep,WebFetch,WebSearch,Task,LSP", "--output-format", "stream-json", "--verbose",
                "--max-turns", str(a.max_turns)] + (["--model", a.model] if a.model else [])
        env = {k: v for k, v in os.environ.items() if k != "CLAUDECODE"}
        t0 = time.time()
        try:
            run = subprocess.run(args, capture_output=True, text=True, timeout=a.timeout, env=env, cwd=work)
            (work / "transcript.jsonl").write_text(run.stdout)
            events = [json.loads(l) for l in run.stdout.splitlines() if l.strip().startswith("{")]
            payload = next((e for e in reversed(events) if e.get("type") == "result"), {"error": run.stderr[-300:]})
            calls, errors = 0, []
            for e in events:
                for c in (e.get("message", {}) or {}).get("content", []) if isinstance(e.get("message"), dict) else []:
                    if isinstance(c, dict) and c.get("type") == "tool_use":
                        calls += 1
                    if isinstance(c, dict) and c.get("type") == "tool_result" and c.get("is_error"):
                        text = c.get("content")
                        errors.append(str(text if isinstance(text, str) else json.dumps(text))[:240])
            payload["tool_calls"], payload["tool_errors"] = calls, errors
        except subprocess.TimeoutExpired:
            payload = {"error": "timeout"}
        rec = {"id": task["id"], "seconds": round(time.time() - t0),
               "turns": payload.get("num_turns"), "cost": payload.get("total_cost_usd"),
               "reply": str(payload.get("result", payload.get("error", "")))[:300],
               "tool_calls": payload.get("tool_calls"), "tool_errors": payload.get("tool_errors", [])}
        if "system" in task:
            rec["fails"], rec["system"] = check_system(a.cli, task, save, work) if save.exists() else (["no design saved"], None)
        elif "arm" in task:
            rec["fails"], rec["arm"] = check_arm(a.cli, task, save) if save.exists() else (["no design saved"], None)
        elif "urdf" in task:
            rec["fails"] = check_urdf(task, work / "robot.urdf") + ([] if save.exists() else ["no design saved"])
        elif save.exists():
            d, err = measure(a.cli, save)
            rec["fails"], rec["volume"] = (check(task, d) if d else ([f"eval failed: {err}"], None))
        else:
            rec["fails"] = ["no design saved"]
        rec["ok"] = not rec["fails"]
        report.append(rec)
        print(json.dumps(rec), flush=True)
        (out / "report.json").write_text(json.dumps(report, indent=1))
    ok = sum(r["ok"] for r in report)
    print(f"{ok}/{len(report)} tasks correct")

if __name__ == "__main__":
    main()

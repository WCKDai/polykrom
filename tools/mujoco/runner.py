"""Polykrom's MuJoCo runner (docs/mujoco.md): runs one job on an exported MJCF model and prints
its result as one line of JSON. Polykrom starts it as a separate program; by hand it is
`python3 runner.py job.json` (or the job on stdin).

MuJoCo (Apache-2.0) is not part of Polykrom: `pip install mujoco numpy`, and `imageio-ffmpeg`
for videos.

Mode `stand` holds the model's `design` keyframe; mode `study` settles on the study's first pose,
then drives the position actuators along the study's tracks with the base free, step by step,
linear in between, for a number of cycles."""
import json
import math
import sys

OUT = sys.stdout
sys.stdout = sys.stderr  # stdout carries the result only


def fail(kind, message, code=2):
    print(json.dumps({"error": kind, "message": message}), file=OUT, flush=True)
    sys.exit(code)


def main():
    job = json.load(open(sys.argv[1]) if len(sys.argv) > 1 else sys.stdin)
    if job.get("mode") not in ("stand", "study"):
        fail("job", "`mode` must be stand or study")
    try:
        import mujoco
        import numpy as np
    except ImportError as e:
        fail("mujoco_missing", f"MuJoCo is not installed for {sys.executable} ({e}): pip install mujoco numpy", 3)
    writer = None
    if job.get("video"):
        try:
            import imageio_ffmpeg
        except ImportError as e:
            fail("video", f"videos need imageio-ffmpeg ({e}): pip install imageio-ffmpeg", 3)
    try:
        m = mujoco.MjModel.from_xml_path(job["mjcf"])
    except Exception as e:  # MuJoCo's own message names the file and the line
        fail("model", f"MuJoCo could not load {job['mjcf']}: {e}")
    d = mujoco.MjData(m)
    if m.nkey > 0:
        mujoco.mj_resetDataKeyframe(m, d, 0)
    mujoco.mj_forward(m, d)
    free = next((j for j in range(m.njnt) if m.jnt_type[j] == mujoco.mjtJoint.mjJNT_FREE), None)
    root = int(m.jnt_bodyid[free]) if free is not None else min(1, m.nbody - 1)
    acts = [mujoco.mj_id2name(m, mujoco.mjtObj.mjOBJ_ACTUATOR, a) for a in range(m.nu)]
    joint = [int(m.actuator_trnid[a, 0]) for a in range(m.nu)]
    slide = [m.jnt_type[j] == mujoco.mjtJoint.mjJNT_SLIDE for j in joint]
    up0, fwd0 = d.xmat[root].reshape(3, 3)[:, 2].copy(), d.xmat[root].reshape(3, 3)[:, 0].copy()
    hold = d.ctrl.copy()
    idx = {a: i for i, a in enumerate(acts)}
    tracks = []
    for t in job.get("tracks", []):
        if t["actuator"] not in idx:
            fail("track", f"the model has no actuator `{t['actuator']}`: export it again from this design")
        pts = np.array(sorted(t["points"]), dtype=float).reshape(-1, 2)
        tracks.append((idx[t["actuator"]], pts[:, 0], pts[:, 1]))
    steps = max(1, int(job.get("steps", 1)))

    def targets(k):
        c = hold.copy()
        for i, xs, ys in tracks:
            c[i] = np.interp(k % steps, xs, ys)
        return c

    if job.get("video"):
        w, h = 960, 600
        m.vis.global_.offwidth, m.vis.global_.offheight = max(w, m.vis.global_.offwidth), max(h, m.vis.global_.offheight)
        try:
            renderer = mujoco.Renderer(m, h, w)
        except Exception as e:
            fail("video", f"MuJoCo cannot render here ({e}); on a machine without a display set MUJOCO_GL=egl or osmesa")
        writer = imageio_ffmpeg.write_frames(job["video"], (w, h), fps=30, macro_block_size=8)
        writer.send(None)
        cam = mujoco.MjvCamera()
        cam.distance, cam.azimuth, cam.elevation = 1.7 * m.stat.extent, -55, -16
    stats = {"tilt": 0.0, "com_min": math.inf, "peak": np.zeros(m.nu), "err": np.zeros(m.nu), "frame": 0.0, "at_limit": np.zeros(m.nu)}
    limits = np.where(m.actuator_forcelimited, m.actuator_forcerange[:, 1], np.inf)
    resets0 = int(d.warning[mujoco.mjtWarning.mjWARN_BADQACC].number)

    def tilt():
        up = d.xmat[root].reshape(3, 3)[:, 2]
        return math.degrees(math.acos(max(-1.0, min(1.0, float(up @ up0)))))

    def step(ctrl, measure):
        d.ctrl[:] = ctrl
        mujoco.mj_step(m, d)
        stats["tilt"] = max(stats["tilt"], tilt())
        if measure:
            stats["com_min"] = min(stats["com_min"], float(d.subtree_com[root][2]))
            stats["peak"] = np.maximum(stats["peak"], np.abs(d.actuator_force))
            stats["at_limit"] += np.abs(d.actuator_force) >= 0.999 * limits
            stats["err"] = np.maximum(stats["err"], np.abs(d.ctrl - d.actuator_length))
        if writer is not None and d.time >= stats["frame"]:
            stats["frame"] += 1 / 30
            cam.lookat[:] = d.subtree_com[root]
            renderer.update_scene(d, cam)
            writer.send(np.ascontiguousarray(renderer.render()))

    dt = m.opt.timestep
    mm = lambda x: round(float(x) * 1000, 1)
    com0 = float(d.subtree_com[root][2])
    for _ in range(int(round(float(job.get("settle_s", 0.5)) / dt))):
        step(targets(0) if job["mode"] == "study" else hold, False)
    settled = float(d.subtree_com[root][2])
    pos0 = d.xpos[root].copy()
    fwd = np.array([fwd0[0], fwd0[1], 0.0])
    fwd = fwd / np.linalg.norm(fwd) if np.linalg.norm(fwd) > 1e-9 else np.array([1.0, 0.0, 0.0])
    left = np.array([-fwd[1], fwd[0], 0.0])
    out = {"mode": job["mode"], "mujoco": mujoco.__version__, "timestep_s": dt, "free_base": free is not None}
    if job["mode"] == "study":
        cycles, sub = max(1, int(job.get("cycles", 1))), max(1, int(round(float(job["step_s"]) / dt)))
        per_cycle, last = [], pos0.copy()
        for k in range(steps * cycles):
            for i in range(sub):
                step(targets(k + i / sub), True)
            if (k + 1) % steps == 0:
                per_cycle.append(mm((d.xpos[root] - last) @ fwd))
                last = d.xpos[root].copy()
        moved = d.xpos[root] - pos0
        out.update({"cycles": cycles, "advance_mm": mm(moved @ fwd), "drift_mm": mm(moved @ left), "per_cycle_mm": per_cycle,
                    "advance_x_mm": mm(moved[0]), "advance_y_mm": mm(moved[1])})
    else:
        for _ in range(int(round(float(job.get("hold_s", 2.0)) / dt))):
            step(hold, True)
        out["base_drop_mm"] = mm(com0 - d.subtree_com[root][2])
    if writer is not None:
        writer.close()
        out["video"] = job["video"]
    resets = int(d.warning[mujoco.mjtWarning.mjWARN_BADQACC].number) - resets0
    finite = bool(np.all(np.isfinite(d.qpos)))
    fall_z = job.get("fall_height_mm")
    fall_z = fall_z / 1000 if fall_z is not None else settled / 2
    fell = None
    if free is not None:
        fell = stats["tilt"] > float(job.get("fall_tilt_deg", 45)) or stats["com_min"] < fall_z
    actuators = []
    for i, a in enumerate(acts):
        limit = float(m.actuator_forcerange[i, 1]) if m.actuator_forcelimited[i] else None
        err = float(abs(d.ctrl[i] - d.actuator_length[i]))
        to = (lambda x: round(x * 1000, 2)) if slide[i] else (lambda x: round(math.degrees(x), 2))
        actuators.append({"name": a, "unit": "N" if slide[i] else "N m", "peak_force": round(float(stats["peak"][i]), 4),
                          "force_limit": limit, "utilisation": round(float(stats["peak"][i]) / limit, 3) if limit else None,
                          "at_limit_s": round(float(stats["at_limit"][i]) * dt, 3),
                          "error" + ("_mm" if slide[i] else "_deg"): to(err), "max_error" + ("_mm" if slide[i] else "_deg"): to(float(stats["err"][i]))})
    out.update({
        "seconds": round(float(d.time), 3), "tilt_max_deg": round(stats["tilt"], 2),
        "base_z_mm": [mm(pos0[2]), mm(d.xpos[root][2])], "com_z_mm": [mm(com0), mm(d.subtree_com[root][2]), mm(stats["com_min"])],
        "fell": fell, "finite": finite, "resets": resets, "ok": finite and resets == 0 and not fell,
        "saturated": [a["name"] for a in actuators if a["utilisation"] is not None and a["utilisation"] >= 0.999],
        "actuators": actuators,
    })
    print(json.dumps(out), file=OUT, flush=True)


if __name__ == "__main__":
    main()

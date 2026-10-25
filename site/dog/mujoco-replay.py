"""Replay a Polykrom motion study on its exported MJCF in MuJoCo: the joints' keys become the
position actuators' targets, the base is free, and physics decides whether the robot walks."""
import json, math, sys, zipfile
import mujoco
import numpy as np

design, mjcf, study_name, cycles, step_s = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), float(sys.argv[5])
out = sys.argv[6] if len(sys.argv) > 6 else None
doc = json.loads(zipfile.ZipFile(design).read('document.json'))
names = {j['id']: j['name'] for j in doc['assembly']['joints']}
study = next(s for s in doc['assembly']['studies'] if s['name'] == study_name)
steps = study['steps']
tracks = {names[k['joint']]: np.array(k['points']) for k in study['keys']}

m = mujoco.MjModel.from_xml_path(mjcf)
d = mujoco.MjData(m)
mujoco.mj_resetDataKeyframe(m, d, 0)
acts = [mujoco.mj_id2name(m, mujoco.mjtObj.mjOBJ_ACTUATOR, a) for a in range(m.nu)]
def targets(step):
    s = step % steps
    return np.array([np.interp(s, tracks[a][:, 0], tracks[a][:, 1]) if a in tracks else d.ctrl[i] for i, a in enumerate(acts)])

# Settle on the first pose, then walk.
d.ctrl[:] = targets(0)
for _ in range(int(0.5 / m.opt.timestep)):
    mujoco.mj_step(m, d)
x0, z0 = d.qpos[0], d.qpos[2]
m.vis.global_.offwidth, m.vis.global_.offheight = 1280, 800
renderer = mujoco.Renderer(m, 600, 960) if out else None
frames, worst_tilt, trail = [], 0.0, []
fps, sub = 25, max(1, int(round(step_s / m.opt.timestep)))
for k in range(steps * cycles):
    for i in range(sub):
        d.ctrl[:] = targets(k + i / sub)
        mujoco.mj_step(m, d)
    q = d.qpos[3:7]
    up = 1 - 2 * (q[1] ** 2 + q[2] ** 2)  # world z of the body's z axis
    worst_tilt = max(worst_tilt, math.degrees(math.acos(max(-1, min(1, up)))))
    trail.append((d.time, d.qpos[0] - x0, d.qpos[1], d.qpos[2]))
    if renderer and (k * step_s * fps) % 1 < step_s * fps:
        cam = mujoco.MjvCamera(); cam.lookat[:] = [d.qpos[0], d.qpos[1], 0.08]; cam.distance = 0.62; cam.azimuth = -55; cam.elevation = -16
        renderer.update_scene(d, cam); frames.append(renderer.render().copy())
ok = np.all(np.isfinite(d.qpos))
print(json.dumps({"study": study_name, "cycles": cycles, "seconds": round(d.time, 2), "advance_mm": round((d.qpos[0] - x0) * 1000, 1),
    "per_cycle_mm": round((d.qpos[0] - x0) * 1000 / cycles, 1), "drift_y_mm": round((d.qpos[1]) * 1000, 1),
    "base_z_mm": [round(z0 * 1000, 1), round(d.qpos[2] * 1000, 1)], "worst_tilt_deg": round(worst_tilt, 2), "finite": bool(ok)}))
if out and frames:
    import imageio.v3 as iio
    iio.imwrite(out, np.stack(frames), fps=fps)
    print("frames", len(frames), out)

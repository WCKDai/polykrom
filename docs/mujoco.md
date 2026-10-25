# Simulating in MuJoCo

`sim.mujoco` checks in physics that a robot stands and walks. It exports the design's
[MJCF model](mjcf.md) and runs it in the MuJoCo physics simulator (Apache-2.0). MuJoCo runs as a
separate program, [`tools/mujoco/runner.py`](../tools/mujoco/runner.py), the way the stress
analysis runs Nightfall. Nothing of MuJoCo is linked into Polykrom, and the browser build has
no simulation.

## Setting it up

The runner needs Python 3 with MuJoCo, and `imageio-ffmpeg` for videos:

```sh
pip install mujoco numpy imageio-ffmpeg
export POLYKROM_MUJOCO_PYTHON=/path/to/that/python   # default: python3 on PATH
```

Without MuJoCo the command says how to install it and does nothing.

## Running it

```json
{"command": "sim.mujoco", "params": {}}
{"command": "sim.mujoco", "params": {"study": "Crawl", "cycles": 3, "step_s": 0.05}}
```

The model is exported with a free base into a new folder in the temporary folder. `mjcf` runs a
model you exported from this design yourself. The folder also holds the runner and the job
(below), so `python3 runner.py job.json` runs it again.

| Parameter | Default | Meaning |
|---|---|---|
| `mode` | `study` with `study`, else `stand` | What to run |
| `study`, `cycles`, `step_s` | –, 1, 0.05 | The motion study (id or name), how often, seconds per step |
| `settle_s`, `hold_s` | 0.5, 2 | Seconds before measuring; seconds a stand is held |
| `fall_tilt_deg`, `fall_height_mm` | 45, half the settled height | It fell if the base tilts past this or the centre of mass drops below this |
| `video` | – | An `.mp4` of the run |
| `mjcf` | export | An MJCF model exported from this design |
| `floating`, `kp`, `damping`, `armature`, `effort` | `true`, … | The MJCF export's options ([mjcf.md](mjcf.md)) |
| `budget_s` | 300 | Wall-clock seconds before the run is stopped |

The reply is the runner's result (below) with `mjcf` and `folder`, where the run's files are,
and `warnings`: the export's, and any study keys left out because no actuator drives their joint
(a follower of a motion link). The study's keys become the actuators' tracks in the model's
units, radians and metres.

## The runner

`python3 tools/mujoco/runner.py job.json` from the repository root (in a run's folder, `python3 runner.py job.json`; or the job on stdin) runs one job and prints one line of JSON.

```json
{"mjcf": "dog.xml", "mode": "study", "settle_s": 0.5, "fall_tilt_deg": 45,
 "steps": 160, "step_s": 0.05, "cycles": 3,
 "tracks": [{"actuator": "FL_hip", "points": [[0, 0.52], [40, 0.31]]}],
 "video": "crawl.mp4"}
```

- **`stand`** starts from the model's `design` keyframe: the design's pose with every actuator
  holding it. It settles for `settle_s` seconds, then holds for `hold_s`.
- **`study`** settles on the tracks' first targets, then drives the position actuators along
  them with the base free, `cycles` times through `steps` steps of `step_s` seconds each. Targets
  are linear between points and held before the first and after the last. An actuator without a
  track holds the keyframe.
- **`video`** writes an MP4 at 30 frames per second of simulated time, the camera following the
  robot (needs `imageio-ffmpeg`).
- `fall_height_mm`, when given, replaces the default fall height.

Without MuJoCo it prints `{"error": "mujoco_missing", "message": …}` and exits with 3.

## The result

- `ok`: finite, never reset by MuJoCo, and not fallen. `fell`, `finite` and `resets` give the
  details. MuJoCo resets a simulation that diverges, so a finite end state alone proves nothing.
- `tilt_max_deg`: the base's largest tilt from its start. `base_z_mm` gives the base's height
  at the start and the end. `com_z_mm` gives the centre of mass at the start, the end and its
  lowest. With a fixed base, `fell` is `null`.
- `actuators`: one entry per actuator. Each gives its `peak_force` (N m, or N for sliding)
  against its `force_limit` (the rating the export wrote), the `utilisation` between them, and
  `at_limit_s`, the seconds it spent at the limit. A brief spike at the limit differs from a
  servo that can't hold. Each also gives the tracking error (`error_deg` or `error_mm`) at the
  end and its largest while measuring. `saturated` lists the actuators that reached their limit.
- `study` only: `advance_mm` along the base's start heading (its x axis on the floor) and
  `drift_mm` across it, `per_cycle_mm`, and `advance_x_mm` and `advance_y_mm` in world axes.
- `stand` only: `base_drop_mm`, how far the centre of mass sank.

The robot dog D3 (#101), from its design with the `Crawl` study, 3 cycles at 0.05 s per step:
it stands, and walks 128.5 mm (42.8 mm per cycle) with 0.1 mm of drift and at most 0.12° of
tilt. Its hip and knee servos reach their 1.91 N m rating for 24 to 54 ms of the 24 s.

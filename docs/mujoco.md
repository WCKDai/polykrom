# Simulating in MuJoCo

[`tools/mujoco/runner.py`](../tools/mujoco/runner.py) checks in physics that a robot stands and
walks: it runs a design's [MJCF export](mjcf.md) in the MuJoCo physics simulator (Apache-2.0),
as a separate program. Nothing of MuJoCo is linked into Polykrom.

## Setting it up

The runner needs Python 3 with MuJoCo, and `imageio-ffmpeg` for videos:

```sh
pip install mujoco numpy imageio-ffmpeg
```

## The runner

`python3 tools/mujoco/runner.py job.json` (or the job on stdin) runs one job and prints one line of JSON.
A study's keys go in as the actuators' tracks in the model's units, radians and metres; each
actuator is named after its joint.

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

The robot dog D3 (#101), exported, with the `Crawl` study, 3 cycles at 0.05 s per step:
it stands, and walks 128.5 mm (42.8 mm per cycle) with 0.1 mm of drift and at most 0.12° of
tilt. Its hip and knee servos reach their 1.91 N m rating for 24 to 54 ms of the 24 s.

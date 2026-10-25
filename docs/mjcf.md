# MJCF export (MuJoCo)

A design with components and joints exports as an MJCF model for the MuJoCo physics simulator.
It is built from the same link tree as the [URDF export](urdf.md): the same parts, names,
masses, inertias, joint chains and motion links, written as nested MuJoCo bodies, with a
position actuator on every moving joint, a floor and a light.

- **Commands:** `file.export {"path": "robot.xml", "format": "mjcf"}`, or a `.mjcf` path
- **Needs** MuJoCo 3.1.6 or later (the actuators' `dampratio`).
- **MCP:** the `export` tool with `format: "mjcf"`
- **CLI:** `polykrom-cli run script.json` with that `file.export` command

MJCF always exports the whole assembly, so `bodies` is refused. Next to the model the export
writes one binary STL per component in metres, named as for URDF (`robot-<component>.stl`).
Paths are checked and written as for URDF: meshes first, the model last.

## Loading it

```python
import mujoco
model = mujoco.MjModel.from_xml_path("robot.xml")
data = mujoco.MjData(model)
mujoco.mj_resetDataKeyframe(model, data, 0)   # the design's pose, actuators holding it
```

Every body is written where Polykrom shows it, and each joint's `ref` is the design's current
value, so the model loads in the design's pose with joint values equal to Polykrom's (radians
and metres). The keyframe `design` holds that pose (`qpos`) and sets each actuator's target to
it (`ctrl`). Without it the actuators' targets are 0, the joints' zero.

## Parameters

| Parameter | Default | Meaning |
|---|---|---|
| `floating` | when nothing is grounded | `true`: the root body gets a `<freejoint/>`, so the robot falls onto the floor (a walking robot). `false`: the root is fixed in the world. |
| `scene` | `true` | A floor plane at z = 0 (the design's XY plane) and a directional light. Turn it off to include the model in your own scene. |
| `kp` | 1000 | Position actuator gain, in N m/rad (turning) or N/m (sliding): a stiff servo that holds its target within a degree until its force limit is reached. The actuator's damping is critical (`dampratio="1"`), so the stiffness doesn't ring. |
| `damping` | 0.1 | Joint damping, in N m s/rad or N s/m, for every joint (a `<default>`): the passive losses of a small geared joint. The actuators damp their own motion; this slows joints no actuator holds. |
| `armature` | 0.01 | Joint armature, in kg m² or kg: the rotor inertia a gearbox reflects onto the joint, of the order of a hobby servo's. It also keeps a joint chain's massless in-between bodies stable at MuJoCo's 2 ms step. |
| `effort` | 1000 | Actuator force limit, in N m or N, of a joint no rated actuator drives. When neither applies, 1000 is written with a warning. |

A number that is not positive, or a flag that is not `true` or `false`, is refused.

## What is written

- **`<compiler angle="radian" meshdir="." balanceinertia="true">`**: meshes resolve next to the
  model. `balanceinertia` lets MuJoCo add the least inertia that makes a tensor physically
  possible when rounding leaves a thin part just outside.
- **`<option integrator="implicitfast">`**, MuJoCo's recommended integrator for damped joints
  and stiff actuators.
- **Bodies:** one per URDF link, nested as the tree is. A body with no moving joint is welded
  to its parent. Each has `<inertial pos mass fullinertia>`: the link's centre of mass in
  metres, its mass in kg and its inertia tensor about the centre in kg m², from the bodies'
  meshes and material densities. A body with no mass gets URDF's token mass (1e-6 kg).
- **Geoms:** each body's mesh as a `type="mesh"` geom in its colour (`rgba`, opacity kept). A
  component mixing closed solids with open surfaces gets a drawn geom that doesn't collide and
  an unseen collision geom (group 3) of the closed solids.
- **Joints:** a revolute step is a `hinge`, a sliding step a `slide`, with `range` and
  `limited="true"` from the joint's limits, or `limited="false"` without them. Fixed steps
  weld the bodies. Joint chains (cylindrical, pin-slot, planar, ball, parallel) are as in URDF.
- **Motion links** (gears, rack and pinion, screws) are joint equality constraints:
  `<joint joint1="follower" joint2="driver" polycoef="0 ratio 0 0 0">`. MuJoCo measures both
  joints from the pose the model loads in, where the link already holds, so the offset is zero.
  A follower has no actuator.
- **Actuators:** a `<position>` per moving joint that no motion link drives, with `ctrlrange`
  equal to the joint's range when it has one, and `forcerange` from:
  1. the rated torque of a library actuator at an end of the joint (a turning joint): the part
     the joint picks, or the part it is nested in (a horn inside a servo), whose ratings give
     `stall_torque_nm` (servos, BLDC motors) or `holding_torque_nm` (steppers);
  2. otherwise `effort`;
  3. otherwise 1000, with a warning naming the joints.
- **Collisions:** every mesh geom collides (MuJoCo uses each mesh's convex hull). MuJoCo leaves
  out pairs within one body, bodies welded together and a body and its parent, so a joint's two
  parts don't push each other apart. It doesn't do that for a parent welded to the world (a
  fixed base), so there the export lists the pairs in `<contact><exclude>`: each joint off the
  base against the base, with what is fixed to either side (at most 10 000 pairs, with a warning
  past that).

## Not covered

- Simplified collision shapes: the collision mesh is the visual mesh's convex hull.
- Velocity limits: a position actuator has none; `velocity` is URDF's.
- Actuators for library parts that drive a joint through another part (a motor fixed to one
  link turning the next through a coupling it doesn't own): give `effort`, or joint the motor
  itself.
- Contact parameters (friction, softness) and sensors: MuJoCo's defaults apply.

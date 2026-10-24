#!/usr/bin/env python3
"""A tiny robot for the viewer test: a 10 mm cube mesh (binary STL, metres) and a URDF with a
revolute joint about z at x = 0.1 m and a prismatic joint along x after it.

    fixture.py OUT_DIR
"""
import struct, sys
from pathlib import Path

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
s = 0.01
v = [(x * s, y * s, z * s) for x in (0, 1) for y in (0, 1) for z in (0, 1)]
faces = [(0, 1, 3), (0, 3, 2), (4, 6, 7), (4, 7, 5), (0, 4, 5), (0, 5, 1), (2, 3, 7), (2, 7, 6), (0, 2, 6), (0, 6, 4), (1, 5, 7), (1, 7, 3)]
data = bytearray(b"\0" * 80) + struct.pack("<I", len(faces))
for f in faces:
    data += struct.pack("<3f", 0, 0, 0)
    for i in f:
        data += struct.pack("<3f", *v[i])
    data += b"\0\0"
(out / "cube.stl").write_bytes(bytes(data))
(out / "robot.urdf").write_text("""<?xml version="1.0"?>
<robot name="t">
  <link name="base"><visual><geometry><mesh filename="cube.stl"/></geometry></visual></link>
  <link name="arm"><visual><origin xyz="0 0 0" rpy="0 0 0"/><geometry><mesh filename="cube.stl"/></geometry></visual></link>
  <link name="tool"><visual><geometry><mesh filename="cube.stl"/></geometry></visual></link>
  <joint name="turn" type="revolute"><parent link="base"/><child link="arm"/><origin xyz="0.1 0 0" rpy="0 0 0"/><axis xyz="0 0 1"/><limit lower="-3" upper="3" effort="1" velocity="1"/></joint>
  <joint name="slide" type="prismatic"><parent link="arm"/><child link="tool"/><origin xyz="0.05 0 0" rpy="0 0 0"/><axis xyz="1 0 0"/><limit lower="0" upper="0.1" effort="1" velocity="1"/></joint>
</robot>
""")

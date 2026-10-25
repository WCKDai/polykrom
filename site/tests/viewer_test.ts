// The showcases' viewer, checked without a browser: the robot it builds from a URDF, the poses it
// gives joint values (motion.plot's degrees and millimetres), and the meshes it decodes from a
// meshpack.py pack. Run: python3 site/tests/fixture.py DIR && python3 site/meshpack.py DIR DIR/pack.bin
// DIR/pack.json && FIXTURE=DIR deno test -A --import-map=site/tests/import_map.json site/tests/viewer_test.ts
import * as THREE from "three";
import { buildRobot, creased, packCache, packedGeometry, tracks } from "../shared/viewer.js";

const dir = Deno.env.get("FIXTURE") ?? "";

/** The bit of DOM the builder uses, from a small XML reader (Deno has no DOMParser for XML). */
function parseXml(text: string) {
  type El = { tagName: string; children: El[]; getAttribute(n: string): string | null };
  const root: El = mk("#doc", {});
  const stack = [root];
  for (const m of text.matchAll(/<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<\/([\w:-]+)\s*>|<([\w:-]+)((?:\s+[\w:-]+="[^"]*")*)\s*(\/?)>/g)) {
    if (m[1]) { stack.pop(); continue; }
    if (!m[2]) continue;
    const attrs: Record<string, string> = {};
    for (const a of (m[3] || "").matchAll(/([\w:-]+)="([^"]*)"/g)) attrs[a[1]] = a[2];
    const el = mk(m[2], attrs);
    stack[stack.length - 1].children.push(el);
    if (!m[4]) stack.push(el);
  }
  return root.children[0];
  function mk(tagName: string, attrs: Record<string, string>): El { return { tagName, children: [], getAttribute: (n) => attrs[n] ?? null }; }
}

function close(a: number, b: number, tol: number, what: string) {
  if (!(Math.abs(a - b) <= tol)) throw new Error(`${what}: ${a} != ${b} (±${tol})`);
}

Deno.test("joints pose their links: a turn about z, then a slide along the turned x", async () => {
  const robot = buildRobot(parseXml(await Deno.readTextFile(dir + "/robot.urdf")));
  if (robot.joints.size !== 2 || robot.roots.length !== 1) throw new Error("two joints under one root");
  const motion = { joints: [{ name: "turn", unit: "deg", value: [0, 90] }, { name: "slide", unit: "mm", value: [0, 40] }] };
  const play = tracks(motion);
  play.at(1, robot.set);
  robot.roots[0].updateMatrixWorld(true);
  // The tool's origin: 0.1 along x to the turn, then (0.05 + 0.04) along the turned x (now +y).
  const p = new THREE.Vector3().applyMatrix4(robot.links.get("tool")!.matrixWorld);
  close(p.x, 0.1, 1e-12, "x"); close(p.y, 0.09, 1e-12, "y"); close(p.z, 0, 1e-12, "z");
  play.at(0.5, robot.set); // half way: 45 degrees and 20 mm
  robot.roots[0].updateMatrixWorld(true);
  const q = new THREE.Vector3().applyMatrix4(robot.links.get("tool")!.matrixWorld);
  close(q.x, 0.1 + 0.07 * Math.SQRT1_2, 1e-12, "half x"); close(q.y, 0.07 * Math.SQRT1_2, 1e-12, "half y");
});

Deno.test("a pack decodes to its mesh within the quantisation step", async () => {
  const buf = (await Deno.readFile(dir + "/pack.bin")).buffer;
  const man = JSON.parse(await Deno.readTextFile(dir + "/pack.json"));
  const m = man.meshes["cube.stl"];
  if (!m || m.verts !== 8 || m.count !== 36) throw new Error(`cube: 8 shared vertices, 36 indices: ${JSON.stringify(m)}`);
  const geo = packedGeometry(buf, m);
  geo.computeBoundingBox();
  const b = geo.boundingBox!;
  for (const k of ["x", "y", "z"] as const) { close(b.min[k], 0, 1e-9, "min " + k); close(b.max[k], 0.01, 0.01 / 65535, "max " + k); }
});

Deno.test("a pack's mesh is built once for every part that shares it, with its bounds", async () => {
  const buf = (await Deno.readFile(dir + "/pack.bin")).buffer;
  const m = JSON.parse(await Deno.readTextFile(dir + "/pack.json")).meshes["cube.stl"];
  const geometry = packCache(buf);
  const a = geometry("cube.stl", m), b = geometry("cube.stl", m);
  if (a !== b) throw new Error("the second part rebuilt the mesh");
  // Creased (unindexed: 36 corners) and bounded: a 10 mm cube's sphere is half its diagonal.
  close(a.getAttribute("position").count, 36, 0, "corners");
  close(a.boundingSphere!.radius, (Math.sqrt(3) * 0.01) / 2, 1e-6, "bounding radius");
});

/** An indexed mesh from flat position and index lists. */
function indexed(pos: number[], idx: number[]) {
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(pos), 3));
  g.setIndex(new THREE.BufferAttribute(new Uint32Array(idx), 1));
  return g;
}

Deno.test("creased normals keep a cube's corners sharp and a cylinder's side smooth", () => {
  // A unit cube sharing its eight corners: every corner of every face keeps that face's normal.
  const p = [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1];
  const f = [0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6, 1, 2, 6, 1, 6, 5, 0, 4, 7, 0, 7, 3];
  const cube = creased(indexed(p, f), Math.PI / 6);
  const n = cube.getAttribute("normal").array, q = cube.getAttribute("position").array;
  for (let c = 0; c < f.length; c += 3) {
    const a = new THREE.Vector3(q[3 * c], q[3 * c + 1], q[3 * c + 2]);
    const b = new THREE.Vector3(q[3 * c + 3], q[3 * c + 4], q[3 * c + 5]);
    const d = new THREE.Vector3(q[3 * c + 6], q[3 * c + 7], q[3 * c + 8]);
    const face = b.sub(a).cross(d.sub(a)).normalize();
    for (let k = 0; k < 3; k++) {
      close(n[3 * (c + k)] * face.x + n[3 * (c + k) + 1] * face.y + n[3 * (c + k) + 2] * face.z, 1, 1e-6, `cube corner ${c + k}`);
    }
  }
  // A 64-sided cylinder's side: its facets meet at 5.6°, so each vertex's normal is the radial one.
  const sides = 64, pos: number[] = [], idx: number[] = [];
  for (let i = 0; i < sides; i++) {
    const t = (2 * Math.PI * i) / sides;
    pos.push(Math.cos(t), Math.sin(t), 0, Math.cos(t), Math.sin(t), 1);
  }
  for (let i = 0; i < sides; i++) {
    const a = 2 * i, b = 2 * ((i + 1) % sides);
    idx.push(a, b, b + 1, a, b + 1, a + 1);
  }
  const cyl = creased(indexed(pos, idx), Math.PI / 6);
  const cn = cyl.getAttribute("normal").array, cp = cyl.getAttribute("position").array;
  for (let c = 0; c < idx.length; c++) {
    const r = Math.hypot(cp[3 * c], cp[3 * c + 1]);
    // The radial direction, to the facets' own tilt at most (the average of two normals 5.6° apart).
    close((cn[3 * c] * cp[3 * c] + cn[3 * c + 1] * cp[3 * c + 1]) / r, 1, 1e-3, `cylinder corner ${c}`);
    close(cn[3 * c + 2], 0, 1e-6, `cylinder corner ${c} z`);
  }
});

Deno.test("a mirrored leg: the same joint value on a frame rolled half a turn swings its link the mirrored way", () => {
  // The dog's right legs are its left legs mirrored: each joint's frame is rolled by π, so its z
  // points down and the same angle swings the leg the other way, as a mirror image across y = 0.
  const robot = buildRobot(parseXml(`<robot name="m">
    <link name="body"/><link name="l"/><link name="lt"/><link name="r"/><link name="rt"/>
    <joint name="L" type="revolute"><parent link="body"/><child link="l"/><origin xyz="0 0.1 0" rpy="0 0 0"/><axis xyz="0 0 1"/></joint>
    <joint name="Lt" type="fixed"><parent link="l"/><child link="lt"/><origin xyz="0.1 0 0" rpy="0 0 0"/></joint>
    <joint name="R" type="revolute"><parent link="body"/><child link="r"/><origin xyz="0 -0.1 0" rpy="${Math.PI} 0 0"/><axis xyz="0 0 1"/></joint>
    <joint name="Rt" type="fixed"><parent link="r"/><child link="rt"/><origin xyz="0.1 0 0" rpy="0 0 0"/></joint>
  </robot>`));
  const play = tracks({ joints: [{ name: "L", unit: "deg", value: [0, 30] }, { name: "R", unit: "deg", value: [0, 30] }] });
  const tip = (name: string) => new THREE.Vector3().applyMatrix4(robot.links.get(name)!.matrixWorld);
  for (const [s, deg] of [[0, 0], [1, 30], [0.5, 15]]) {
    play.at(s, robot.set);
    robot.roots[0].updateMatrixWorld(true);
    const a = (deg * Math.PI) / 180, l = tip("lt"), r = tip("rt");
    close(l.x, 0.1 * Math.cos(a), 1e-12, `left x at ${deg}°`); close(l.y, 0.1 + 0.1 * Math.sin(a), 1e-12, `left y at ${deg}°`);
    close(r.x, l.x, 1e-12, `right x mirrors left at ${deg}°`); close(r.y, -l.y, 1e-12, `right y mirrors left at ${deg}°`);
    close(r.z, 0, 1e-12, `right z at ${deg}°`); close(l.z, 0, 1e-12, `left z at ${deg}°`);
  }
});

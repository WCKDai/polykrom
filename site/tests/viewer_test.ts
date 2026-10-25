// The showcases' viewer, checked without a browser: the robot it builds from a URDF, the poses it
// gives joint values (motion.plot's degrees and millimetres), and the meshes it decodes from a
// meshpack.py pack. Run: python3 site/tests/fixture.py DIR && python3 site/meshpack.py DIR DIR/pack.bin
// DIR/pack.json && FIXTURE=DIR deno test -A --import-map=site/tests/import_map.json site/tests/viewer_test.ts
import * as THREE from "three";
import { buildRobot, creased, packedGeometry, tracks } from "../shared/viewer.js";

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

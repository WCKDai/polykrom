// The showcases' viewer, checked without a browser: the robot it builds from a URDF, the poses it
// gives joint values (motion.plot's degrees and millimetres), and the meshes it decodes from a
// meshpack.py pack. Run: python3 site/tests/fixture.py DIR && python3 site/meshpack.py DIR DIR/pack.bin
// DIR/pack.json && FIXTURE=DIR deno test -A --import-map=site/tests/import_map.json site/tests/viewer_test.ts
import * as THREE from "three";
import { buildRobot, packedGeometry, tracks } from "../shared/viewer.js";

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

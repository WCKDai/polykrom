// Kestrel viewer: loads the URDF Polykrom exported (one STL per link, joint frames in metres)
// and plays the motion study recorded with motion.plot, joint by joint, in the browser.
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { STLLoader } from "three/addons/loaders/STLLoader.js";
import { toCreasedNormals } from "three/addons/utils/BufferGeometryUtils.js";

const safe = (n) => n.replace(/[^A-Za-z0-9_]/g, "_");
const nums = (s, d = "0 0 0") => (s || d).trim().split(/\s+/).map(Number);
const kids = (el, tag) => [...el.children].filter((c) => c.tagName.toLowerCase() === tag);
const kid = (el, tag) => kids(el, tag)[0];

function frame(el) {
  const g = new THREE.Group();
  const o = el && kid(el, "origin");
  const [x, y, z] = nums(o && o.getAttribute("xyz"));
  const [r, p, w] = nums(o && o.getAttribute("rpy"));
  g.position.set(x, y, z);
  g.quaternion.setFromEuler(new THREE.Euler(r, p, w, "ZYX"));
  return g;
}

/** The robot a URDF describes, as three.js groups: one per link, each joint a frame (its origin)
 * holding a mover (its motion) holding the child link. `visual(link, visualEl, frameGroup)` is
 * called for each mesh visual so the caller can load and add it. `set(joint, value)` poses a
 * joint in radians or metres. */
export function buildRobot(xml, visual = () => {}) {
  const robot = xml.documentElement || xml;
  const links = new Map();
  for (const l of kids(robot, "link")) {
    const g = new THREE.Group();
    g.name = l.getAttribute("name");
    links.set(g.name, g);
    for (const v of kids(l, "visual")) {
      const geo = kid(v, "geometry");
      if (!geo || !kid(geo, "mesh")) continue;
      const f = frame(v);
      g.add(f);
      visual(g.name, v, f);
    }
  }
  const joints = new Map();
  const children = new Set();
  for (const j of kids(robot, "joint")) {
    const parent = links.get(kid(j, "parent").getAttribute("link"));
    const child = links.get(kid(j, "child").getAttribute("link"));
    if (!parent || !child) continue;
    const f = frame(j);
    const mover = new THREE.Group();
    f.add(mover);
    mover.add(child);
    parent.add(f);
    children.add(child.name);
    const a = kid(j, "axis");
    joints.set(j.getAttribute("name"), { type: j.getAttribute("type"), axis: new THREE.Vector3(...nums(a && a.getAttribute("xyz"), "1 0 0")).normalize(), mover });
  }
  const roots = [...links.values()].filter((g) => !children.has(g.name));
  function set(name, value) {
    const j = joints.get(name);
    if (!j) return;
    if (j.type === "prismatic") j.mover.position.copy(j.axis).multiplyScalar(value);
    else if (j.type === "revolute" || j.type === "continuous") j.mover.quaternion.setFromAxisAngle(j.axis, value);
  }
  return { links, joints, roots, set };
}

/** One mesh of a pack (meshpack.py): 16-bit positions within the mesh's box, shared vertices,
 * 16- or 32-bit indices. Returned with creased normals, so hard edges stay sharp. */
export function packedGeometry(buffer, m) {
  const q = new Uint16Array(buffer, m.pos, m.verts * 3);
  const pos = new Float32Array(m.verts * 3);
  for (let i = 0; i < pos.length; i++) {
    const k = i % 3;
    pos[i] = m.min[k] + (q[i] / 65535) * (m.max[k] - m.min[k]);
  }
  const idx = m.wide ? new Uint32Array(buffer, m.idx, m.count) : new Uint16Array(buffer, m.idx, m.count);
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setIndex(new THREE.BufferAttribute(m.wide ? new Uint32Array(idx) : new Uint16Array(idx), 1));
  return geo;
}

/** Motion-study tracks (motion.plot's joints, degrees and millimetres) as URDF values per step. */
export function tracks(motion) {
  const ts = (motion?.joints || []).map((t) => ({ name: safe(t.name), k: t.unit === "deg" ? Math.PI / 180 : 0.001, v: t.value }));
  const steps = Math.max(1, ...ts.map((t) => t.v.length)) - 1;
  const at = (s, f) => {
    const i = Math.floor(s), u = s - i;
    for (const t of ts) {
      const a = t.v[Math.min(i, t.v.length - 1)], b = t.v[Math.min(i + 1, t.v.length - 1)];
      f(t.name, (a + (b - a) * u) * t.k);
    }
  };
  return { steps, at };
}

export async function mount(root, { urdf, motion, onStep, colors = {}, pack, camera: view, theme }) {
  const css = getComputedStyle(document.documentElement);
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  root.appendChild(renderer.domElement);
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(35, 1, 0.005, 20);
  camera.up.set(0, 0, 1);
  camera.position.set(...(view?.position || [0.55, -0.62, 0.42]));
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(...(view?.target || [0.03, 0, 0.16]));
  controls.enableDamping = true;
  // On the ink hero the page scrolls past the view: it turns by dragging but doesn't take the wheel.
  if (theme === "ink") controls.enableZoom = false;
  // The WCKD kit's neutrals: warm white light, an ash grid with Polykrom's lime on its centre lines.
  scene.add(new THREE.HemisphereLight(0xf4f1ea, 0x2a2826, 1.6));
  const key = new THREE.DirectionalLight(0xffffff, 2.2);
  key.position.set(0.6, -0.8, 1.2);
  scene.add(key);
  const rim = new THREE.DirectionalLight(0xeeebe5, 0.8);
  rim.position.set(-0.8, 0.7, 0.5);
  scene.add(rim);
  const dark = theme === "ink" || matchMedia("(prefers-color-scheme: dark)").matches;
  const grid = new THREE.GridHelper(0.6, 24, 0xccff00, dark ? 0x3a3733 : 0xd3d0c9);
  grid.rotation.x = Math.PI / 2;
  grid.material.transparent = true;
  grid.material.opacity = 0.35;
  scene.add(grid);

  const base = new URL(urdf, location.href);
  const xml = new DOMParser().parseFromString(await (await fetch(base)).text(), "application/xml");
  const loader = new STLLoader();
  const loads = [];
  // A pack (one file for every mesh) when the page gives one, else one STL per mesh.
  const packed = pack ? await Promise.all([fetch(pack.bin).then((r) => r.arrayBuffer()), fetch(pack.json).then((r) => r.json())]) : null;
  const robot = buildRobot(xml, (link, v, f) => {
    const mesh = kid(kid(v, "geometry"), "mesh");
    // The part's material look when the page gives one (by link), else the URDF's colour.
    // `colors`: looks by link name, or a function from the link's name to its look.
    const look = typeof colors === "function" ? colors(link) : colors[link];
    const c = kid(v, "material") && kid(kid(v, "material"), "color");
    const [r, gg, b] = nums(c && c.getAttribute("rgba"), "0.7 0.72 0.76 1");
    const mat = new THREE.MeshStandardMaterial(look ? { ...look, color: new THREE.Color(look.color) } : { color: new THREE.Color(r, gg, b), metalness: 0.35, roughness: 0.45 });
    const url = new URL(mesh.getAttribute("filename").replace(/^package:\/\/[^/]+\//, ""), base);
    const file = mesh.getAttribute("filename").split("/").pop();
    const entry = packed && packed[1].meshes[file];
    const geometry = entry ? Promise.resolve(toCreasedNormals(packedGeometry(packed[0], entry), Math.PI / 6)) : loader.loadAsync(url.href).then((g) => (g.computeVertexNormals(), g));
    loads.push(geometry.then((geo) => {
      const m = new THREE.Mesh(geo, mat);
      const [sx, sy, sz] = nums(mesh.getAttribute("scale"), "1 1 1");
      m.scale.set(sx, sy, sz);
      f.add(m);
    }));
  });
  for (const g of robot.roots) scene.add(g);
  await Promise.all(loads);
  // motion: {steps, joints: [{name, unit: "deg"|"mm", value: [...]}]}
  const play = tracks(motion);
  const steps = play.steps;
  function pose(s) {
    play.at(s, robot.set);
    onStep && onStep(s, steps);
  }

  function resize() {
    const w = root.clientWidth, h = root.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  new ResizeObserver(resize).observe(root);
  resize();
  // ?shot&step=N: one still frame at step N, then the loop stops (for screenshots and posters).
  const q = new URLSearchParams(location.search);
  const still = q.has("shot");
  let playing = !still && !matchMedia("(prefers-reduced-motion: reduce)").matches, t = +(q.get("step") || 0), last = performance.now(), frames = 0;
  const api = {
    steps,
    play(on) { playing = on; },
    get playing() { return playing; },
    seek(s) { t = s; pose(t); },
  };
  pose(t);
  renderer.setAnimationLoop((now) => {
    if (still && ++frames > 40) { renderer.setAnimationLoop(null); document.body.dataset.ready = "1"; return; }
    const dt = Math.min(0.1, (now - last) / 1000);
    last = now;
    if (playing && steps > 0) { t = (t + dt * 14) % steps; pose(t); }
    controls.update();
    renderer.render(scene, camera);
  });
  return api;
}

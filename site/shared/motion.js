// The site's motion, in one place (kit.css styles it): things ease into place as they come into
// view, images are drawn in behind a scan line, numbers count up to their value, the MCP prompt
// types itself and the loop's ring draws. Nothing runs, and everything shows at once, when the
// system asks for reduced motion.
const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
// Hiding things until they come into view is the script's to switch on (kit.css), so the page reads
// in full wherever it doesn't run.
if (!still) document.documentElement.classList.add("motion");
// Anything not yet shown a few seconds after load (a browser that never reports it in view) shows.
addEventListener("load", () => setTimeout(() => document.querySelectorAll(".reveal:not(.shown), .scan:not(.shown)").forEach((el) => el.classList.add("shown")), 4000));

function count(el) {
  const to = parseFloat(el.dataset.count);
  const places = (el.dataset.count.split(".")[1] || "").length;
  const fmt = (v) => v.toLocaleString("en-US", { minimumFractionDigits: places, maximumFractionDigits: places });
  if (still) { el.textContent = fmt(to) + (el.dataset.unit || ""); return; }
  const t0 = performance.now(), dur = 1400;
  const step = (now) => {
    const k = Math.min(1, (now - t0) / dur), e = 1 - Math.pow(1 - k, 3);
    el.textContent = fmt(to * e) + (el.dataset.unit || "");
    if (k < 1) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function type(el) {
  const full = el.dataset.type;
  if (still) { el.textContent = full; return; }
  el.textContent = "";
  let i = 0;
  const tick = () => {
    el.textContent = full.slice(0, ++i);
    if (i < full.length) setTimeout(tick, full[i - 1] === "\n" ? 260 : 22 + Math.random() * 30);
  };
  setTimeout(tick, 300);
}

const seen = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    const el = e.target;
    el.classList.add("shown", "in-view");
    if (el.dataset.count) count(el);
    if (el.dataset.type) type(el);
    seen.unobserve(el);
  }
}, { rootMargin: "0px 0px -12% 0px", threshold: 0.12 });

for (const el of document.querySelectorAll(".reveal, .scan, [data-count], [data-type], .loop")) {
  if (still) { el.classList.add("shown", "in-view"); if (el.dataset.count) count(el); if (el.dataset.type) type(el); }
  else seen.observe(el);
}

// Stagger the children of a [data-stagger] list.
for (const list of document.querySelectorAll("[data-stagger]")) {
  [...list.children].forEach((c, i) => { c.classList.add("reveal"); c.style.setProperty("--i", i); if (!still) seen.observe(c); else c.classList.add("shown"); });
}

// The loop's ring: its arc's length for the draw-in, and a lime runner going round it.
for (const svg of document.querySelectorAll(".loop svg")) {
  const arc = svg.querySelector(".arc"), runner = svg.querySelector(".runner");
  if (!arc) continue;
  const len = arc.getTotalLength();
  arc.style.setProperty("--len", len);
  if (!runner || still) continue;
  let t0 = null;
  const go = (now) => {
    t0 ??= now;
    const p = arc.getPointAtLength(((now - t0) / 9000 % 1) * len);
    runner.setAttribute("cx", p.x);
    runner.setAttribute("cy", p.y);
    requestAnimationFrame(go);
  };
  requestAnimationFrame(go);
}

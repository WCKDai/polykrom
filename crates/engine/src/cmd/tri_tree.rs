//! A bounding-box tree over a mesh's triangles, for the contact checks: nearest distance from a
//! point, triangles a ray or a segment may cross. It only skips triangles whose box rules them
//! out, so every answer is the one a scan of all triangles gives; `work` counts the triangles
//! actually tested, which is what the contact budgets limit.

use polycore_geom::{Aabb3, Vec3};

/// Triangles per leaf.
const LEAF: usize = 8;

pub(super) struct TriTree {
    tris: Vec<[Vec3; 3]>,
    nodes: Vec<Node>,
}

struct Node {
    bounds: Aabb3,
    /// Leaf: the triangle range in `tris`; inner: the two children's indices.
    kind: Kind,
}

enum Kind {
    Leaf(usize, usize),
    Inner(usize, usize),
}

fn tri_box(t: &[Vec3; 3]) -> Aabb3 {
    let mut b = Aabb3::EMPTY;
    for v in t {
        b.add(*v);
    }
    b
}

fn centre(t: &[Vec3; 3]) -> Vec3 {
    (t[0] + t[1] + t[2]) * (1.0 / 3.0)
}

fn axis(v: Vec3, k: usize) -> f64 {
    match k {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// How far `p` is from the box (0 inside).
fn box_distance(b: &Aabb3, p: Vec3) -> f64 {
    let d = |v: f64, lo: f64, hi: f64| (lo - v).max(0.0).max(v - hi);
    Vec3::new(d(p.x, b.min.x, b.max.x), d(p.y, b.min.y, b.max.y), d(p.z, b.min.z, b.max.z)).len()
}

impl TriTree {
    pub(super) fn new(tris: Vec<[Vec3; 3]>) -> TriTree {
        let mut tree = TriTree { tris, nodes: Vec::new() };
        let n = tree.tris.len();
        if n > 0 {
            tree.build(0, n);
        }
        tree
    }

    pub(super) fn tris(&self) -> &[[Vec3; 3]] {
        &self.tris
    }

    /// The box around every triangle (empty for no triangles).
    pub(super) fn bounds(&self) -> Aabb3 {
        self.nodes.first().map_or(Aabb3::EMPTY, |n| n.bounds)
    }

    /// Build the subtree over `tris[lo..hi]`: split at the median of the longest axis.
    fn build(&mut self, lo: usize, hi: usize) -> usize {
        let bounds = self.tris[lo..hi].iter().fold(Aabb3::EMPTY, |b, t| b.union(&tri_box(t)));
        let at = self.nodes.len();
        self.nodes.push(Node { bounds, kind: Kind::Leaf(lo, hi) });
        if hi - lo <= LEAF {
            return at;
        }
        let size = bounds.max - bounds.min;
        let k = if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        };
        let mid = (lo + hi) / 2;
        self.tris[lo..hi].select_nth_unstable_by(mid - lo, |a, b| axis(centre(a), k).total_cmp(&axis(centre(b), k)));
        let left = self.build(lo, mid);
        let right = self.build(mid, hi);
        if let Some(node) = self.nodes.get_mut(at) {
            node.kind = Kind::Inner(left, right);
        }
        at
    }

    /// The least of `f(triangle)` over the triangles whose box is nearer `p` than the best found
    /// so far; `f` must be at least the box distance (a point-to-triangle distance is).
    pub(super) fn nearest(&self, p: Vec3, work: &mut usize, f: impl Fn(&[Vec3; 3]) -> f64) -> f64 {
        let mut best = f64::MAX;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let Some(node) = self.nodes.get(i) else { continue };
            if box_distance(&node.bounds, p) >= best {
                continue;
            }
            match node.kind {
                Kind::Leaf(lo, hi) => {
                    for t in &self.tris[lo..hi] {
                        *work += 1;
                        best = best.min(f(t));
                    }
                }
                Kind::Inner(a, b) => {
                    // The nearer child first, so the farther one is more often pruned.
                    let (da, db) = (
                        self.nodes.get(a).map_or(f64::MAX, |n| box_distance(&n.bounds, p)),
                        self.nodes.get(b).map_or(f64::MAX, |n| box_distance(&n.bounds, p)),
                    );
                    if da <= db {
                        stack.extend([b, a]);
                    } else {
                        stack.extend([a, b]);
                    }
                }
            }
        }
        best
    }

    /// Every triangle whose box the segment `p`→`q` (or, `ray`, the half line from `p` through
    /// `q`) may touch, grown by `pad`: a superset of the triangles it crosses.
    pub(super) fn along(&self, p: Vec3, q: Vec3, ray: bool, pad: f64, work: &mut usize, mut f: impl FnMut(&[Vec3; 3])) {
        let d = q - p;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let Some(node) = self.nodes.get(i) else { continue };
            if !slab(&node.bounds, p, d, ray, pad) {
                continue;
            }
            match node.kind {
                Kind::Leaf(lo, hi) => {
                    for t in &self.tris[lo..hi] {
                        *work += 1;
                        f(t);
                    }
                }
                Kind::Inner(a, b) => stack.extend([a, b]),
            }
        }
    }
}

/// Whether `p + s d` meets the box grown by `pad` for some s in [0, 1] (or s ≥ 0 for a ray).
fn slab(b: &Aabb3, p: Vec3, d: Vec3, ray: bool, pad: f64) -> bool {
    let (mut lo, mut hi) = (0.0f64, if ray { f64::INFINITY } else { 1.0 });
    for k in 0..3 {
        let (o, v, mn, mx) = (axis(p, k), axis(d, k), axis(b.min, k) - pad, axis(b.max, k) + pad);
        if v.abs() < 1e-300 {
            if o < mn || o > mx {
                return false;
            }
            continue;
        }
        let (t1, t2) = ((mn - o) / v, (mx - o) / v);
        lo = lo.max(t1.min(t2));
        hi = hi.min(t1.max(t2));
        if lo > hi {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic soup of triangles: small, scattered, some long and thin.
    fn soup(n: usize, seed: u64) -> Vec<[Vec3; 3]> {
        let mut x = seed;
        let mut r = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % 10_000) as f64 / 100.0
        };
        (0..n)
            .map(|i| {
                let c = Vec3::new(r(), r(), r());
                let s = if i % 7 == 0 { 30.0 } else { 3.0 };
                [c, c + Vec3::new(r() / 100.0 * s, r() / 100.0, 0.5), c + Vec3::new(0.3, r() / 100.0 * s, r() / 100.0)]
            })
            .collect()
    }

    fn dist(p: Vec3, t: &[Vec3; 3]) -> f64 {
        // Distance to the triangle's vertices and centre: enough to compare a scan with the tree.
        [t[0], t[1], t[2], centre(t)].iter().map(|v| v.dist(p)).fold(f64::MAX, f64::min)
    }

    #[test]
    fn nearest_matches_a_full_scan_and_tests_fewer_triangles() {
        let tree = TriTree::new(soup(2000, 7));
        let mut work = 0;
        for k in 0..50 {
            let p = Vec3::new(k as f64 * 2.0, 50.0 - k as f64, (k * 3 % 100) as f64);
            let scan = tree.tris().iter().map(|t| dist(p, t).max(box_distance(&tri_box(t), p))).fold(f64::MAX, f64::min);
            let got = tree.nearest(p, &mut work, |t| dist(p, t).max(box_distance(&tri_box(t), p)));
            assert_eq!(got, scan, "point {p:?}");
        }
        assert!(work < 50 * 2000 / 5, "the tree prunes: {work} triangle tests");
    }

    #[test]
    fn segments_and_rays_reach_every_triangle_whose_box_they_touch() {
        let tree = TriTree::new(soup(1500, 11));
        for k in 0..40 {
            let p = Vec3::new(k as f64 * 2.5, (k * 7 % 100) as f64, 10.0);
            let q = p + Vec3::new(13.0, -9.0, 31.0);
            for ray in [false, true] {
                let mut seen = Vec::new();
                tree.along(p, q, ray, 1e-9, &mut 0, |t| seen.push(*t));
                for t in tree.tris() {
                    let b = tri_box(t);
                    if slab(&b, p, q - p, ray, 1e-9) {
                        assert!(seen.contains(t), "missed a triangle the {} touches", if ray { "ray" } else { "segment" });
                    }
                }
            }
        }
    }
}

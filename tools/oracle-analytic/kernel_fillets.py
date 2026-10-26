"""Reference volumes for the curved-edge fillet cases in tests/kernel-cases/fillets (#126, #184).

Plain Python 3, standard library only. Prints each case's value; the numbers are copied into the
cases' expected.json by hand (`method` there gives the same derivation). A rolling-ball round of a
convex edge is the solid opened by the ball: the union of the balls of its radius inside the solid.

    python3 tools/oracle-analytic/kernel_fillets.py            # quick resolutions
    python3 tools/oracle-analytic/kernel_fillets.py --fine     # the resolutions the cases quote
"""

import math
import sys

FINE = "--fine" in sys.argv


def corner_section(phi, r, n=20000):
    """Area and centroid (distance in from the edge along the floor) of what a round of radius r
    removes from a corner of interior angle phi: floor along +x, wall along (cos phi, sin phi)."""
    t = r / math.tan(phi / 2)
    cx, cy = t, r
    a0 = -math.pi / 2
    a1 = math.atan2(t * math.sin(phi) - cy, t * math.cos(phi) - cx)
    if a1 < a0:
        a1 += 2 * math.pi
    if a1 - a0 > math.pi:
        a1 -= 2 * math.pi
    pts = [(0.0, 0.0)] + [(cx + r * math.cos(a0 + (a1 - a0) * i / n), cy + r * math.sin(a0 + (a1 - a0) * i / n)) for i in range(n + 1)]
    area = mx = 0.0
    for (x0, y0), (x1, y1) in zip(pts, pts[1:] + pts[:1]):
        c = x0 * y1 - x1 * y0
        area += c
        mx += (x0 + x1) * c
    area *= 0.5
    return abs(area), mx / (6 * area)


def offset_loft():
    """126-fillet-loft-bottom-chain-constant-slope: 60x40 r10 lofted to the same inset 5 at z 30, R4."""
    phi = math.atan2(30, 5)
    a, xb = corner_section(phi, 4)
    a0, l0 = 2400 - (4 - math.pi) * 100, 120 + 20 * math.pi
    loft = 30 * a0 - 75 * l0 + 250 * math.pi
    return loft - a * (l0 - 2 * math.pi * xb)


def frustum():
    """126-fillet-revolved-frustum-bottom-circle: r 20 to 15 over 30, R4 (Pappus)."""
    a, xb = corner_section(math.atan2(30, 5), 4)
    return math.pi * 30 / 3 * (400 + 300 + 225) - a * 2 * math.pi * (20 - xb)


def sloped_cylinder(nt, nr):
    """126-fillet-cylinder-sloped-top-rim: r 10 cylinder cut by z = 30 + x tan 20, R3 on the rim.
    Over each point of the disk the opened solid's top is the highest ball centred under the plane
    offset by r and within r - 3 of the axis."""
    R, alpha, rho, z0 = 10.0, math.radians(20), 3.0, 30.0
    k, rc = math.tan(alpha), R - rho

    def top(px, py):
        if (px + rho * math.sin(alpha)) ** 2 + py ** 2 <= rc * rc:
            return z0 + px * k

        def g(phi):
            qx, qy = rc * math.cos(phi), rc * math.sin(phi)
            d2 = (px - qx) ** 2 + (py - qy) ** 2
            return -1e9 - d2 if d2 >= rho * rho else z0 + qx * k - rho / math.cos(alpha) + math.sqrt(rho * rho - d2)

        c = math.atan2(py, px + rho * math.sin(alpha))
        lo, hi, gr = c - 0.6, c + 0.6, (math.sqrt(5) - 1) / 2
        a, b = hi - gr * (hi - lo), lo + gr * (hi - lo)
        fa, fb = g(a), g(b)
        for _ in range(80):
            if fa < fb:
                lo, a, fa = a, b, fb
                b = lo + gr * (hi - lo)
                fb = g(b)
            else:
                hi, b, fb = b, a, fa
                a = hi - gr * (hi - lo)
                fa = g(a)
        return max(fa, fb)

    xs = [-0.9602898564975363, -0.7966664774136267, -0.5255324099163290, -0.1834346424956498, 0.1834346424956498, 0.5255324099163290, 0.7966664774136267, 0.9602898564975363]
    ws = [0.1012285362903763, 0.2223810344533745, 0.3137066458778873, 0.3626837833783620, 0.3626837833783620, 0.3137066458778873, 0.2223810344533745, 0.1012285362903763]
    r0 = R - rho - rho * math.sin(alpha) - 0.5
    h = (R - r0) / nr
    removed = 0.0
    for i in range(nt):
        th = 2 * math.pi * i / nt
        for j in range(nr):
            for x, w in zip(xs, ws):
                r = r0 + h * (j + 0.5) + 0.5 * h * x
                px, py = r * math.cos(th), r * math.sin(th)
                removed += w * 0.5 * h * (z0 + px * k - top(px, py)) * r * 2 * math.pi / nt
    return math.pi * R * R * z0 - removed


def elliptic_loft(a0, b0, a1, b1, height, r, nt, nl, ns):
    """Ruled loft W(t, s) = ((a0 + (a1-a0) s) cos t, (b0 + (b1-b0) s) sin t, height s) whose walls
    lean in all round, R r on its bottom edge: (loft volume, volume removed). The lowest balls sit
    on the floor, centred in Omega (height r, at least r from the wall; its edge is the spine).
    Over a floor point at distance d outside Omega the column is removed below r - sqrt(r^2 - d^2)
    and between r + sqrt(r^2 - d^2) and the strip of wall normals through the spine, up to the
    wall; all of it, up to the wall, where d >= r."""

    def wall(t, s):
        return ((a0 + (a1 - a0) * s) * math.cos(t), (b0 + (b1 - b0) * s) * math.sin(t), height * s)

    def inward(t, s):
        a, b = a0 + (a1 - a0) * s, b0 + (b1 - b0) * s
        wt = (-a * math.sin(t), b * math.cos(t), 0.0)
        ws = ((a1 - a0) * math.cos(t), (b1 - b0) * math.sin(t), height)
        n = (wt[1] * ws[2] - wt[2] * ws[1], wt[2] * ws[0] - wt[0] * ws[2], wt[0] * ws[1] - wt[1] * ws[0])
        length = math.sqrt(sum(x * x for x in n))
        p = wall(t, s)
        sign = -1.0 if n[0] * p[0] + n[1] * p[1] > 0 else 1.0
        return tuple(sign * x / length for x in n)

    spine, contact = [], []
    for k in range(ns):
        t = 2 * math.pi * k / ns
        lo, hi = 0.0, 0.5
        f = lambda s: wall(t, s)[2] + r * inward(t, s)[2] - r
        for _ in range(80):
            m = 0.5 * (lo + hi)
            lo, hi = (m, hi) if (f(lo) < 0) == (f(m) < 0) else (lo, m)
        s = 0.5 * (lo + hi)
        p, n = wall(t, s), inward(t, s)
        spine.append((p[0] + r * n[0], p[1] + r * n[1]))
        contact.append(p)

    def nearest(q, t):
        k0, best, at = int(round(t / (2 * math.pi) * ns)), float("inf"), (0, 0.0)
        for k in range(k0 - ns // 12, k0 + ns // 12 + 1):
            a, b = spine[k % ns], spine[(k + 1) % ns]
            dx, dy = b[0] - a[0], b[1] - a[1]
            u = min(1.0, max(0.0, ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy)))
            e = math.hypot(q[0] - a[0] - u * dx, q[1] - a[1] - u * dy)
            if e < best:
                best, at = e, (k % ns, u)
        return best, at

    def wall_height(q):
        g = lambda s: (q[0] / (a0 + (a1 - a0) * s)) ** 2 + (q[1] / (b0 + (b1 - b0) * s)) ** 2 - 1
        if g(0.0) > 0:
            return 0.0
        lo, hi = 0.0, 1.0
        for _ in range(60):
            m = 0.5 * (lo + hi)
            lo, hi = (m, hi) if g(m) <= 0 else (lo, m)
        return height * lo

    def strip_height(q, at):
        k, u = at
        k2 = (k + 1) % ns
        g = tuple(spine[k][i] + u * (spine[k2][i] - spine[k][i]) for i in range(2))
        w = tuple(contact[k][i] + u * (contact[k2][i] - contact[k][i]) for i in range(3))
        dx, dy = w[0] - g[0], w[1] - g[1]
        mu = ((q[0] - g[0]) * dx + (q[1] - g[1]) * dy) / (dx * dx + dy * dy)
        return r if mu < 0 else (float("inf") if mu > 1 else r + mu * (w[2] - r))

    lam0 = 1.0 - 2.5 * r / min(a0, b0)
    h = (1.0 - lam0) / nl
    removed = 0.0
    for i in range(nt):
        t = 2 * math.pi * (i + 0.5) / nt
        for j in range(nl):
            lam = lam0 + h * (j + 0.5)
            q = (lam * a0 * math.cos(t), lam * b0 * math.sin(t))
            d, at = nearest(q, t)
            a, b = spine[at[0]], spine[(at[0] + 1) % ns]
            if (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]) > 0:
                continue  # inside Omega: kept
            up = min(wall_height(q), strip_height(q, at))
            if d < r:
                sq = math.sqrt(r * r - d * d)
                cut = min(up, r - sq) + max(0.0, up - (r + sq))
            else:
                cut = up
            removed += h * cut * a0 * b0 * lam * 2 * math.pi / nt
    cross, quad = a0 * (b1 - b0) + b0 * (a1 - a0), (a1 - a0) * (b1 - b0)
    return math.pi * height * (a0 * b0 + cross / 2 + quad / 3), removed


if __name__ == "__main__":
    print(f"126-fillet-loft-bottom-chain-constant-slope  {offset_loft():.4f}")
    print(f"126-fillet-revolved-frustum-bottom-circle    {frustum():.4f}")
    print(f"126-fillet-cylinder-sloped-top-rim           {sloped_cylinder(*((360, 80) if FINE else (90, 20))):.4f}")
    res = (90, 8000, 2880) if FINE else (45, 500, 720)
    loft, removed = elliptic_loft(20, 20, 15, 15, 30, 4, *res)
    print(f"  check: the frustum integrated the same way removes {removed:.4f} (Pappus {29059.732045705583 - frustum():.4f})")
    loft, removed = elliptic_loft(30, 20, 10, 10, 40, 4, *res)
    print(f"184-fillet-ellipse-loft-bottom-edge          {loft - removed:.4f} (loft {loft:.4f}, removed {removed:.4f})")

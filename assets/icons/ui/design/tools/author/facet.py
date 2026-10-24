"""Polykrom's icon language, Facet: the drawing primitives. Read assets/icons/ui/STYLE_GUIDE.md.

A part is line art in the `line` role (currentColor): its silhouette and the edges where its
surface turns. One facet, the face the command makes or changes, is filled with the icon's
accent (`active-2`; `active-1` for a second, paler facet). The action is an `active-3` stroke.
No grey shading and no gradients. The accent's hue comes from the icon's kind (aliases.toml,
palette.toml), never from the drawing.

Projection: 2:1 dimetric. Screen x = (x - y) k, screen y = ((x + y) / 2 - z) k. Its 26.57°
edges fall on whole pixels at every size, so outlines stay crisp at 16 and 24 px.
"""
import math
from lib import Icon, A1, A2, A3, GAP, GUIDE, poly, f, ell

LINE = 'currentColor'
W = 3.4        # outlines: silhouettes and turning edges
WA = 4.4       # actions, sketch strokes and flat controls
WG = 2.4       # guides: references, hidden edges, dashed footprints
DASH = '3 3.2'


class Dim:
    """x to the right and down, y to the left and down, z up; k screen units per model unit."""

    def __init__(s, k=1.0, ox=32.0, oy=32.0):
        s.k, s.ox, s.oy = k, ox, oy

    def p(s, x, y, z=0.0):
        return (s.ox + (x - y) * s.k, s.oy + (x + y) * 0.5 * s.k - z * s.k)

    def poly(s, *ps, close=True):
        return poly([s.p(*q) for q in ps], close)

    def ell(s, cx, cy, z, r):
        """A horizontal circle of radius r: screen centre, rx, ry."""
        c = s.p(cx, cy, z)
        return c, r * s.k * math.sqrt(2), r * s.k * math.sqrt(2) / 2


def frame(scale=1.0, dx=0.0, dy=0.0, height=14):
    """The standard part frame: a 24 x 24 block `height` tall centred in the icon. Every
    modelling icon starts here so parts share one optical size; `scale` < 1 makes room for a cue."""
    k = scale
    top = -12 - height
    return Dim(k, 32 + dx, 32 + dy - (top + 12) / 2 * k)


# ---- solids ----

def faces(d, x0, y0, z0, x1, y1, z1):
    return {
        'top': d.poly((x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)),
        'left': d.poly((x0, y1, z1), (x1, y1, z1), (x1, y1, z0), (x0, y1, z0)),
        'right': d.poly((x1, y0, z1), (x1, y1, z1), (x1, y1, z0), (x1, y0, z0)),
    }


def block(ic, d, x0, y0, z0, x1, y1, z1, facet=None, colour=A2, w=W, line=LINE):
    """A rectangular part; `facet` ('top', 'left', 'right' or a list) is filled with the accent."""
    fs = faces(d, x0, y0, z0, x1, y1, z1)
    for name in ([facet] if isinstance(facet, str) else facet or []):
        ic.fill(fs[name], colour)
    edges(ic, d, x0, y0, z0, x1, y1, z1, w, line)


def edges(ic, d, x0, y0, z0, x1, y1, z1, w=W, line=LINE):
    ic.stroke(d.poly((x0, y0, z1), (x1, y0, z1), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0), (x0, y1, z1)), line, w)
    ic.stroke(poly([d.p(x0, y1, z1), d.p(x1, y1, z1), d.p(x1, y0, z1)], False), line, w)
    ic.stroke(poly([d.p(x1, y1, z1), d.p(x1, y1, z0)], False), line, w)


def knock(ic, d, x0, y0, z0, x1, y1, z1):
    """Erase a block's silhouette to transparency, so what is drawn next sits in front."""
    ic.fill(d.poly((x0, y0, z1), (x1, y0, z1), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0), (x0, y1, z1)), GAP)


def cylinder(ic, d, cx, cy, z0, z1, r, facet=None, colour=A2, w=W):
    """A vertical cylinder; `facet` is 'top' or 'side'."""
    (tx, ty), rx, ry = d.ell(cx, cy, z1, r)
    (bx, by), _, _ = d.ell(cx, cy, z0, r)
    side = 'M%s %sL%s %sA%s %s 0 0 0 %s %sL%s %sA%s %s 0 0 1 %s %sZ' % (
        f(tx - rx), f(ty), f(bx - rx), f(by), f(rx), f(ry), f(bx + rx), f(by), f(tx + rx), f(ty), f(rx), f(ry), f(tx - rx), f(ty))
    ic.fill(side, GAP)
    if facet == 'side':
        ic.fill(side, colour)
    if facet == 'top':
        ic.fill(ell(tx, ty, rx, ry), colour)
    ic.stroke('M%s %sL%s %sA%s %s 0 0 0 %s %sL%s %s' % (f(tx - rx), f(ty), f(bx - rx), f(by), f(rx), f(ry), f(bx + rx), f(by), f(tx + rx), f(ty)), LINE, w)
    ic.stroke(ell(tx, ty, rx, ry), LINE, w)


def hole(ic, d, cx, cy, z, r, colour=A2, depth=None):
    """A round opening in a horizontal face: the far wall shows as the facet."""
    (x, y), rx, ry = d.ell(cx, cy, z, r)
    ic.fill(ell(x, y, rx, ry), GAP)
    dz = (depth if depth is not None else r) * d.k
    wall = 'M%s %sA%s %s 0 0 1 %s %sL%s %sA%s %s 0 0 0 %s %sZ' % (
        f(x - rx), f(y), f(rx), f(ry), f(x + rx), f(y), f(x + rx), f(y + dz * 0.6), f(rx), f(ry), f(x - rx), f(y + dz * 0.6))
    ic.clip_begin('h%d%d' % (int(x), int(y)), ell(x, y, rx, ry))
    ic.fill(wall, colour)
    ic.clip_end()
    ic.stroke(ell(x, y, rx, ry), LINE, W)


def plane(ic, d, s, z=0.0, fill=A1, stroke=A3, w=W, dash=None):
    """A horizontal construction plane, s on a side."""
    q = d.poly((-s, -s, z), (s, -s, z), (s, s, z), (-s, s, z))
    if fill:
        ic.fill(q, fill)
    ic.stroke(q, stroke, w, dash=dash)


# ---- cues ----

def arrow(ic, start, tip, colour=A3, head=8.5, w=WA, gap=0.0):
    dx, dy = tip[0] - start[0], tip[1] - start[1]
    n = math.hypot(dx, dy)
    ux, uy = dx / n, dy / n
    base = (tip[0] - ux * head, tip[1] - uy * head)
    shaft = poly([start, (base[0] + ux, base[1] + uy)], False)
    tri = poly([tip, (base[0] - uy * head * 0.55, base[1] + ux * head * 0.55), (base[0] + uy * head * 0.55, base[1] - ux * head * 0.55)])
    if gap:
        ic.stroke(shaft, GAP, w + 2 * gap)
        ic.stroke(tri, GAP, 2 * gap, fill=GAP)
    ic.stroke(shaft, colour, w)
    ic.fill(tri, colour)


def arc_arrow(ic, cx, cy, rx, ry, a0, a1, colour=A3, head=8.0, w=WA, gap=0.0):
    """An arrow along an ellipse from angle a0 to a1 (degrees; 0 right, 90 down); the head
    follows the curve's final tangent."""
    def at(a):
        t = math.radians(a)
        return (cx + rx * math.cos(t), cy + ry * math.sin(t))
    sgn = 1 if a1 > a0 else -1
    # Stop the shaft where the head begins: walk back along the curve by the head's length.
    a_end, dist, prev = a1, 0.0, at(a1)
    while dist < head * 0.8 and abs(a_end - a0) > 1:
        a_end -= sgn * 1.0
        cur = at(a_end)
        dist += math.hypot(cur[0] - prev[0], cur[1] - prev[1])
        prev = cur
    p0, p1 = at(a0), at(a_end)
    large = 1 if abs(a_end - a0) > 180 else 0
    path = 'M%s %sA%s %s 0 %d %d %s %s' % (f(p0[0]), f(p0[1]), f(rx), f(ry), large, 1 if sgn > 0 else 0, f(p1[0]), f(p1[1]))
    tip = at(a1)
    ux, uy = tip[0] - p1[0], tip[1] - p1[1]
    n = math.hypot(ux, uy) or 1
    ux, uy = ux / n, uy / n
    base = (tip[0] - ux * head, tip[1] - uy * head)
    tri = poly([tip, (base[0] - uy * head * 0.55, base[1] + ux * head * 0.55), (base[0] + uy * head * 0.55, base[1] - ux * head * 0.55)])
    if gap:
        ic.stroke(path, GAP, w + 2 * gap)
        ic.stroke(tri, GAP, 2 * gap, fill=GAP)
    ic.stroke(path, colour, w)
    ic.fill(tri, colour)


def badge(ic, x, y, kind='plus', r=8.5, colour=A3):
    """A round badge in the corner: plus, minus, cross or check, punched out of the accent."""
    ic.dot(x, y, r + 2.2, GAP)
    ic.dot(x, y, r, colour)
    k = r * 0.5
    marks = {'plus': ['M%s %sH%s' % (f(x - k), f(y), f(x + k)), 'M%s %sV%s' % (f(x), f(y - k), f(y + k))],
             'minus': ['M%s %sH%s' % (f(x - k), f(y), f(x + k))],
             'cross': ['M%s %sL%s %s' % (f(x - k * .8), f(y - k * .8), f(x + k * .8), f(y + k * .8)), 'M%s %sL%s %s' % (f(x - k * .8), f(y + k * .8), f(x + k * .8), f(y - k * .8))],
             'check': ['M%s %sL%s %sL%s %s' % (f(x - k), f(y), f(x - k * .2), f(y + k * .75), f(x + k), f(y - k * .7))]}[kind]
    for m in marks:
        ic.stroke(m, GAP, 2.8)


def point(ic, x, y, r=4.4, colour=A3):
    """A sketch or construction point: an accent ring with a punched centre."""
    ic.dot(x, y, r, colour)
    ic.dot(x, y, r * 0.45, GAP)


def normalise(d):
    """Path data in the subset the app parses: absolute M, L, C, Q, A and Z only. H and V
    become L; S becomes C with its first control point reflected."""
    import re
    out, cur, start, last_c2 = [], (0.0, 0.0), (0.0, 0.0), None
    for cmd, args in re.findall(r'([MLHVCSQAZ])([^MLHVCSQAZ]*)', d):
        v = [float(x) for x in re.findall(r'-?\d*\.?\d+(?:e-?\d+)?', args)]
        if cmd == 'Z':
            out.append('Z'); cur = start; last_c2 = None; continue
        if cmd in 'ML':
            for i in range(0, len(v), 2):
                c = 'M' if (cmd == 'M' and i == 0) else 'L'
                cur = (v[i], v[i + 1]); out.append('%s%s %s' % (c, f(cur[0]), f(cur[1])))
                if c == 'M': start = cur
            last_c2 = None
        elif cmd in 'HV':
            for x in v:
                cur = (x, cur[1]) if cmd == 'H' else (cur[0], x)
                out.append('L%s %s' % (f(cur[0]), f(cur[1])))
            last_c2 = None
        elif cmd == 'C':
            for i in range(0, len(v), 6):
                q = v[i:i + 6]
                out.append('C' + ' '.join(f(x) for x in q)); last_c2 = (q[2], q[3]); cur = (q[4], q[5])
        elif cmd == 'S':
            for i in range(0, len(v), 4):
                q = v[i:i + 4]
                c1 = (2 * cur[0] - last_c2[0], 2 * cur[1] - last_c2[1]) if last_c2 else cur
                out.append('C' + ' '.join(f(x) for x in (c1[0], c1[1], q[0], q[1], q[2], q[3])))
                last_c2 = (q[0], q[1]); cur = (q[2], q[3])
        elif cmd == 'Q':
            for i in range(0, len(v), 4):
                q = v[i:i + 4]; out.append('Q' + ' '.join(f(x) for x in q)); cur = (q[2], q[3])
            last_c2 = None
        elif cmd == 'A':
            for i in range(0, len(v), 7):
                q = v[i:i + 7]
                out.append('A%s %s %s %d %d %s %s' % (f(q[0]), f(q[1]), f(q[2]), int(q[3]), int(q[4]), f(q[5]), f(q[6])))
                cur = (q[5], q[6])
            last_c2 = None
    return ''.join(out)


def svg(ic):
    import re
    ic.els = [re.sub(r' d="([^"]+)"', lambda m: ' d="%s"' % normalise(m.group(1)), e) for e in ic.els]
    if getattr(ic, 'fam', 'model') == 'flat':
        # Flat controls are one colour, tinted by the widget's state: accents become ink and
        # pale facets are left out.
        ic.els = [e for e in ic.els if not ('fill="%s"' % A1 in e and 'stroke=' not in e)]
        ic.els = [e.replace('fill="%s"' % A1, 'fill="none"').replace(A2, LINE).replace(A3, LINE).replace(GUIDE, LINE) for e in ic.els]
    body = '\n'.join('  ' + e for e in ic.els)
    return '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">\n  <title>%s</title>\n%s\n</svg>\n' % (ic.title, body)

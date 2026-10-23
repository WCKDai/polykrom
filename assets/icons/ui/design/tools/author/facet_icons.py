"""Polykrom's icons, drawn in Facet (facet.py). One function per master id.

    python3 -B facet_icons.py write [ID...]     write masters into design/svg/<family>/
    python3 -B facet_icons.py preview DIR [ID...]   write to DIR instead, for a contact sheet
"""
import math, os, sys
from lib import Icon, A1, A2, A3, GAP, GUIDE, poly, f, ell, rrect
from facet import (LINE, W, WA, WG, DASH, Dim, frame, faces, block, edges, knock, cylinder, hole,
                   plane, arrow, arc_arrow, badge, point, svg)

ICONS = {}


def icon(family, title=None):
    """Register a master. `flat/folder` and `model/folder` share an id, so a second family's
    drawing of an id is keyed `family/id`."""
    def reg(fn):
        name = fn.__name__.rstrip('_')
        ICONS[name if name not in ICONS else family + '/' + name] = (family, title, fn)
        return fn
    return reg


def new(fn_name, title):
    return Icon(fn_name, title)


# ---------------------------------------------------------------- model: create

@icon('model', 'New Sketch')
def sketch():
    ic = new('sketch', 'New Sketch')
    d = frame(1.1, dy=2, height=0)
    ic.stroke(d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0)), LINE, W)
    # A profile drawn on the plane: a circle and a line leaving it, in the sketch's stroke.
    (cx, cy), rx, ry = d.ell(-3, -3, 0, 6.5)
    ic.stroke(ell(cx, cy, rx, ry), A3, WA)
    a, b = d.p(4, 1, 0), d.p(9, 9, 0)
    ic.stroke(poly([a, b], False), A3, WA)
    point(ic, b[0], b[1])
    return ic


@icon('model')
def extrude():
    ic = new('extrude', 'Extrude')
    d = frame(1.0, dy=6, height=10)
    ic.stroke(d.poly((-12, -12, -6), (12, -12, -6), (12, 12, -6), (-12, 12, -6)), A3, WG, dash=DASH)
    for x, y in ((12, 12), (-12, 12), (12, -12)):
        ic.stroke(poly([d.p(x, y, -6), d.p(x, y, 0)], False), A3, WG, dash='2 2.6')
    block(ic, d, -12, -12, 0, 12, 12, 10, facet='top')
    arrow(ic, d.p(0, 0, 10.5), d.p(0, 0, 23))
    return ic


@icon('model')
def revolve():
    ic = new('revolve', 'Revolve')
    d = frame(1.0, dy=4, height=16)
    cylinder(ic, d, 0, 0, 0, 16, 12, facet='top')
    (cx, cy), rx, ry = d.ell(0, 0, 16, 12)
    ic.stroke(poly([(cx, cy - 14), (cx, cy + 34)], False), A3, WG, dash=DASH)
    arc_arrow(ic, cx, cy - 15, 11, 4.6, 200, 345)
    return ic


@icon('model')
def hole_():
    ic = new('hole', 'Hole')
    d = frame(1.05, dy=6, height=8)
    block(ic, d, -12, -12, 0, 12, 12, 8)
    hole(ic, d, 0, 0, 8, 6, depth=7)
    (cx, cy), rx, ry = d.ell(0, 0, 8, 6)
    arrow(ic, (cx, cy - 26), (cx, cy - 3), gap=1.2)
    return ic


@icon('model')
def fillet():
    ic = new('fillet', 'Fillet')
    d = frame(1.0, dy=2, height=18)
    r, x0, y0, x1, y1, z1 = 9, -12, -12, 12, 12, 18
    ts = [i * math.pi / 2 / 10 for i in range(11)]
    a = [d.p(x1, y1 - r + r * math.sin(t), z1 - r + r * math.cos(t)) for t in ts]
    b = [d.p(x0, y1 - r + r * math.sin(t), z1 - r + r * math.cos(t)) for t in ts]
    ic.fill(poly(a + b[::-1]), A2)
    ic.stroke(d.poly((x0, y0, z1), (x1, y0, z1), (x1, y1 - r, z1), (x0, y1 - r, z1)), LINE, W)
    ic.stroke(poly([d.p(x0, y1, z1 - r), d.p(x0, y1, 0), d.p(x1, y1, 0), d.p(x1, y0, 0), d.p(x1, y0, z1)], False), LINE, W)
    ic.stroke(poly([d.p(x1, y1, z1 - r), d.p(x1, y1, 0)], False), LINE, W)
    ic.stroke(poly(a, False), LINE, W)
    ic.stroke(poly(b, False), LINE, W)
    return ic


@icon('model')
def chamfer():
    ic = new('chamfer', 'Chamfer')
    d = frame(1.0, dy=2, height=18)
    c, x0, y0, x1, y1, z1 = 9, -12, -12, 12, 12, 18
    ic.fill(d.poly((x0, y1 - c, z1), (x1, y1 - c, z1), (x1, y1, z1 - c), (x0, y1, z1 - c)), A2)
    ic.stroke(d.poly((x0, y0, z1), (x1, y0, z1), (x1, y1 - c, z1), (x0, y1 - c, z1)), LINE, W)
    ic.stroke(d.poly((x0, y1 - c, z1), (x1, y1 - c, z1), (x1, y1, z1 - c), (x0, y1, z1 - c)), LINE, W)
    ic.stroke(poly([d.p(x0, y1, z1 - c), d.p(x0, y1, 0), d.p(x1, y1, 0), d.p(x1, y0, 0), d.p(x1, y0, z1)], False), LINE, W)
    ic.stroke(poly([d.p(x1, y1, z1 - c), d.p(x1, y1, 0)], False), LINE, W)
    return ic


@icon('model')
def shell():
    ic = new('shell', 'Shell')
    d = frame(1.0, dy=2, height=18)
    t = 4.5
    i0, i1 = -12 + t, 12 - t
    edges(ic, d, -12, -12, 0, 12, 12, 18)
    # The wall left standing is the facet: the rim around the opening.
    rim = d.poly((-12, -12, 18), (12, -12, 18), (12, 12, 18), (-12, 12, 18)) + d.poly((i0, i0, 18), (i0, i1, 18), (i1, i1, 18), (i1, i0, 18))
    ic.fill(rim, A2, rule=True)
    ic.stroke(d.poly((i0, i0, 18), (i1, i0, 18), (i1, i1, 18), (i0, i1, 18)), LINE, W)
    # Inside, the far corner runs down to the floor.
    ic.stroke(poly([d.p(i0, i0, 18), d.p(i0, i0, t), d.p(i1, i0, t)], False), LINE, WG)
    ic.stroke(poly([d.p(i0, i0, t), d.p(i0, i1, t)], False), LINE, WG)
    return ic


@icon('model', 'Press Pull')
def presspull():
    ic = new('presspull', 'Press Pull')
    d = frame(0.95, dy=8, height=8)
    block(ic, d, -12, -12, 0, 12, 12, 8, facet='top')
    c = d.p(0, 0, 8)
    arrow(ic, (c[0], c[1] - 3), (c[0], c[1] - 24))
    arrow(ic, (c[0] + 14, c[1] - 20), (c[0] + 14, c[1] - 4), head=7, w=3.6)
    return ic


@icon('model', 'Move / Copy')
def move():
    ic = new('move', 'Move / Copy')
    a = frame(0.62, dx=-10, dy=8, height=12)
    b = frame(0.62, dx=10, dy=-8, height=12)
    ic.stroke(a.poly((-12, -12, 12), (12, -12, 12), (12, -12, 0), (12, 12, 0), (-12, 12, 0), (-12, 12, 12)), GUIDE, WG, dash=DASH)
    block(ic, b, -12, -12, 0, 12, 12, 12, facet='top')
    arrow(ic, a.p(0, 0, 6), b.p(0, 0, 4), gap=1.2)
    return ic


@icon('model')
def mirror():
    ic = new('mirror', 'Mirror')
    # Seen square on: the part, the mirror line, and its image as the facet.
    ic.stroke('M32 8V56', A3, WG, dash=DASH)
    ic.stroke('M8 22H24L27 28V44H8Z', LINE, WA)
    ic.fill('M56 22H40L37 28V44H56Z', A2)
    ic.stroke('M56 22H40L37 28V44H56Z', LINE, WA)
    return ic


@icon('model', 'Pattern')
def pattern():
    ic = new('pattern', 'Pattern')
    d = frame(0.86, dy=2, height=6)
    for i, (x, y) in enumerate(((-15, -15), (4, -15), (-15, 4), (4, 4))):
        block(ic, d, x, y, 0, x + 11, y + 11, 6, facet='top' if i == 0 else None)
    return ic


@icon('model', 'Offset Plane')
def plane_():
    ic = new('plane', 'Offset Plane')
    d = frame(0.95, dy=0, height=14)
    plane(ic, d, 14, 0, fill=None, stroke=LINE)
    plane(ic, d, 14, 14)
    arrow(ic, d.p(0, 0, 0.5), d.p(0, 0, 13), head=7)
    return ic


@icon('model', 'Parameters')
def params():
    ic = new('params', 'Parameters')
    # Three dimension sliders; one set, the accent knob.
    for y, k, on in ((18, 40, False), (32, 22, True), (46, 34, False)):
        ic.stroke('M10 %sH54' % y, LINE, WA)
        ic.dot(k, y, 6.2, GAP)
        ic.dot(k, y, 5.2, A2 if on else LINE)
    return ic


@icon('model', 'Configuration Table')
def configuration_table():
    ic = new('configuration_table', 'Configuration Table')
    ic.fill('M10 12H54V22H10Z', A2)
    ic.stroke(rrect(10, 12, 54, 52, 3), LINE, WA)
    ic.stroke('M10 22H54M10 32H54M10 42H54M26 12V52', LINE, W)
    return ic


@icon('model')
def sweep():
    ic = new('sweep', 'Sweep')
    # A profile ring travels along a curved path; the swept tube is the facet.
    path = 'M14 50C14 30 30 18 50 18'
    ic.stroke(path, A2, 13)
    ic.stroke(path, LINE, W, dash=None)
    ic.stroke(ell(14, 50, 6.5, 3.3), LINE, W, fill=GAP)
    ic.stroke(ell(50, 18, 3.3, 6.5), A3, WA)
    return ic


@icon('model')
def loft():
    ic = new('loft', 'Loft')
    d = frame(0.95, dy=2, height=20)
    (cx, cy), rx, ry = d.ell(0, 0, 20, 8)
    sq = [d.p(-12, -12, 0), d.p(12, -12, 0), d.p(12, 12, 0), d.p(-12, 12, 0)]
    ic.fill(poly([sq[3], sq[2], sq[1], (cx + rx, cy), (cx - rx, cy)]), A1)
    ic.stroke(poly(sq), LINE, W)
    ic.stroke(poly([sq[3], (cx - rx, cy)], False), LINE, W)
    ic.stroke(poly([sq[1], (cx + rx, cy)], False), LINE, W)
    ic.stroke(poly([sq[2], (cx, cy + ry)], False), LINE, WG)
    ic.stroke(ell(cx, cy, rx, ry), A3, WA, fill=GAP)
    return ic



# ---------------------------------------------------------------- sketch: flat geometry
# Sketch geometry is the bold accent stroke; the points you click are accent rings; references
# are thin ink or dashed guides.

def sk(name, title):
    return Icon(name, title)


def arcpath(cx, cy, r, a0, a1, rx=None):
    rx = rx or r
    t0, t1 = math.radians(a0), math.radians(a1)
    p0 = (cx + rx * math.cos(t0), cy + r * math.sin(t0))
    p1 = (cx + rx * math.cos(t1), cy + r * math.sin(t1))
    large = 1 if abs(a1 - a0) > 180 else 0
    return 'M%s %sA%s %s 0 %d %d %s %s' % (f(p0[0]), f(p0[1]), f(rx), f(r), large, 1 if a1 > a0 else 0, f(p1[0]), f(p1[1])), p0, p1


@icon('model')
def line():
    ic = sk('line', 'Line')
    ic.stroke('M12 50L52 14', A3, WA)
    point(ic, 12, 50); point(ic, 52, 14)
    return ic


@icon('model', 'Line from Midpoint')
def line_mid():
    ic = sk('line_mid', 'Line from Midpoint')
    ic.stroke('M10 48L54 16', A3, WA)
    point(ic, 32, 32, 5.2)
    point(ic, 54, 16)
    return ic


@icon('model', 'Rectangle')
def rect():
    ic = sk('rect', 'Rectangle')
    ic.stroke(poly([(11, 17), (53, 17), (53, 47), (11, 47)]), A3, WA)
    point(ic, 11, 17); point(ic, 53, 47)
    return ic


@icon('model', 'Center Rectangle')
def rect_center():
    ic = sk('rect_center', 'Center Rectangle')
    ic.stroke(poly([(11, 17), (53, 17), (53, 47), (11, 47)]), A3, WA)
    ic.stroke('M32 32L53 47', LINE, WG, dash=DASH)
    point(ic, 32, 32); point(ic, 53, 47)
    return ic


@icon('model', 'Three-Point Rectangle')
def rect3():
    ic = sk('rect3', 'Three-Point Rectangle')
    q = [(10, 38), (36, 12), (54, 30), (28, 56)]
    ic.stroke(poly(q), A3, WA)
    for p in q[:3]:
        point(ic, *p)
    return ic


@icon('model', 'Rounded Rectangle')
def rect_rounded():
    ic = sk('rect_rounded', 'Rounded Rectangle')
    ic.stroke(rrect(11, 17, 53, 47, 9), A3, WA)
    return ic


@icon('model', 'Circle')
def circle():
    ic = sk('circle', 'Circle')
    ic.stroke(ell(32, 32, 20), A3, WA)
    ic.stroke('M32 32L46.1 17.9', LINE, WG, dash=DASH)
    point(ic, 32, 32)
    return ic


@icon('model', 'Two-Point Circle')
def circle2():
    ic = sk('circle2', 'Two-Point Circle')
    ic.stroke(ell(32, 32, 20), A3, WA)
    point(ic, 12, 32); point(ic, 52, 32)
    return ic


@icon('model', 'Three-Point Circle')
def circle3():
    ic = sk('circle3', 'Three-Point Circle')
    ic.stroke(ell(32, 32, 20), A3, WA)
    for a in (200, 300, 60):
        t = math.radians(a)
        point(ic, 32 + 20 * math.cos(t), 32 + 20 * math.sin(t))
    return ic


@icon('model', 'Tangent Circle')
def circle_tt():
    ic = sk('circle_tt', 'Two-Tangent Circle')
    ic.stroke('M8 50H56', LINE, W)
    ic.stroke('M10 10L44 56', LINE, W)
    ic.stroke(ell(36, 34, 16), A3, WA)
    return ic


@icon('model', 'Three-Tangent Circle')
def circle_ttt():
    ic = sk('circle_ttt', 'Three-Tangent Circle')
    ic.stroke(poly([(32, 6), (58, 52), (6, 52)]), LINE, W)
    ic.stroke(ell(32, 37, 13.5), A3, WA)
    return ic


@icon('model', 'Three-Point Arc')
def arc3():
    ic = sk('arc3', 'Three-Point Arc')
    d, p0, p1 = arcpath(32, 40, 22, 190, 350)
    ic.stroke(d, A3, WA)
    point(ic, *p0); point(ic, *p1); point(ic, 32, 18)
    return ic


@icon('model', 'Center Point Arc')
def arc_center():
    ic = sk('arc_center', 'Center Point Arc')
    d, p0, p1 = arcpath(28, 40, 22, 270, 360)
    ic.stroke(d, A3, WA)
    ic.stroke(poly([p0, (28, 40), p1], False), LINE, WG, dash=DASH)
    point(ic, 28, 40); point(ic, *p0); point(ic, *p1)
    return ic


@icon('model', 'Tangent Arc')
def arc_tangent():
    ic = sk('arc_tangent', 'Tangent Arc')
    ic.stroke('M8 44H30', LINE, W)
    ic.stroke('M30 44A14 14 0 0 0 44 30A14 14 0 0 0 30 16', A3, WA)
    point(ic, 30, 44); point(ic, 30, 16)
    return ic


@icon('model', 'Slot')
def slot():
    ic = sk('slot', 'Slot')
    ic.stroke('M22 20H42A12 12 0 0 1 42 44H22A12 12 0 0 1 22 20Z', A3, WA)
    point(ic, 22, 32); point(ic, 42, 32)
    return ic


@icon('model', 'Center Point Slot')
def slot_center():
    ic = sk('slot_center', 'Center Point Slot')
    ic.stroke('M22 20H42A12 12 0 0 1 42 44H22A12 12 0 0 1 22 20Z', A3, WA)
    point(ic, 32, 32); point(ic, 42, 32)
    return ic


@icon('model', 'Overall Slot')
def slot_overall():
    ic = sk('slot_overall', 'Overall Slot')
    ic.stroke('M22 20H42A12 12 0 0 1 42 44H22A12 12 0 0 1 22 20Z', A3, WA)
    point(ic, 10, 32); point(ic, 54, 32)
    return ic


@icon('model', 'Arc Slot')
def arc_slot():
    ic = sk('arc_slot', 'Arc Slot')
    ic.stroke('M12 46A24 24 0 0 1 52 46', A2, 16)
    ic.stroke('M12 46A24 24 0 0 1 52 46', LINE, WG, dash=DASH)
    point(ic, 12, 46); point(ic, 52, 46)
    return ic


@icon('model', 'Center Point Arc Slot')
def arc_slot_center():
    ic = sk('arc_slot_center', 'Center Point Arc Slot')
    ic.stroke('M12 46A24 24 0 0 1 52 46', A2, 16)
    ic.stroke('M32 46L12 46', LINE, WG, dash=DASH)
    point(ic, 32, 46); point(ic, 52, 46)
    return ic


def ngon(cx, cy, r, n, rot=-90):
    return [(cx + r * math.cos(math.radians(rot + 360 * k / n)), cy + r * math.sin(math.radians(rot + 360 * k / n))) for k in range(n)]


@icon('model', 'Inscribed Polygon')
def polygon():
    ic = sk('polygon', 'Inscribed Polygon')
    ic.stroke(ell(32, 32, 21), LINE, WG, dash=DASH)
    ic.stroke(poly(ngon(32, 32, 21, 6)), A3, WA)
    point(ic, 32, 32)
    return ic


@icon('model', 'Circumscribed Polygon')
def polygon_circumscribed():
    ic = sk('polygon_circumscribed', 'Circumscribed Polygon')
    ic.stroke(ell(32, 32, 17), LINE, WG, dash=DASH)
    ic.stroke(poly(ngon(32, 32, 17 / math.cos(math.pi / 6), 6, -60)), A3, WA)
    point(ic, 32, 32)
    return ic


@icon('model', 'Edge Polygon')
def polygon_edge():
    ic = sk('polygon_edge', 'Edge Polygon')
    h = ngon(32, 30, 21, 6, -60)
    ic.stroke(poly(h), A3, WA)
    point(ic, *h[1]); point(ic, *h[2])
    return ic


@icon('model')
def ellipse():
    ic = sk('ellipse', 'Ellipse')
    ic.stroke(ell(32, 32, 23, 14), A3, WA)
    ic.stroke('M32 32H55', LINE, WG, dash=DASH)
    point(ic, 32, 32); point(ic, 55, 32)
    return ic


@icon('model', 'Conic')
def conic():
    ic = sk('conic', 'Conic')
    ic.stroke('M10 50L32 14L54 50', LINE, WG, dash=DASH)
    ic.stroke('M10 50Q32 14 54 50', A3, WA)
    point(ic, 10, 50); point(ic, 54, 50); point(ic, 32, 32)
    return ic


@icon('model', 'Spline')
def spline():
    ic = sk('spline', 'Spline')
    ic.stroke('M8 46C16 18 28 18 32 32S48 46 56 18', A3, WA)
    for p in ((8, 46), (32, 32), (56, 18)):
        point(ic, *p)
    return ic


@icon('model', 'Control Point Spline')
def spline_cv():
    ic = sk('spline_cv', 'Control Point Spline')
    cps = [(8, 48), (18, 14), (40, 50), (56, 16)]
    ic.stroke(poly(cps, False), LINE, WG, dash=DASH)
    ic.stroke('M8 48C18 14 40 50 56 16', A3, WA)
    for p in cps:
        ic.dot(p[0], p[1], 3.6, LINE)
    return ic


@icon('model', 'Point')
def point_():
    ic = sk('point', 'Point')
    ic.stroke('M32 12V22M32 42V52M12 32H22M42 32H52', LINE, W)
    point(ic, 32, 32, 7)
    return ic


@icon('model', 'Text')
def text():
    ic = sk('text', 'Text')
    # A drawn T on a baseline: text becomes sketch outlines.
    ic.stroke('M14 14H50M32 14V50', A3, WA + 1.2)
    ic.stroke('M10 54H54', LINE, WG, dash=DASH)
    return ic


@icon('model', 'Trim')
def trim():
    ic = sk('trim', 'Trim')
    ic.stroke('M12 52L52 12', LINE, W)
    ic.stroke('M8 32H32', A3, WA)
    ic.stroke('M32 32H56', A3, WG, dash=DASH)
    ic.stroke('M40 24L50 40M50 24L40 40', LINE, W)
    return ic


@icon('model', 'Extend')
def extend():
    ic = sk('extend', 'Extend')
    ic.stroke('M50 8V56', LINE, W)
    ic.stroke('M8 32H30', A3, WA)
    arrow(ic, (30, 32), (49, 32), head=8)
    return ic


@icon('model', 'Offset')
def offset():
    ic = sk('offset', 'Offset')
    ic.stroke('M12 52V28A16 16 0 0 1 28 12H52', LINE, W)
    ic.stroke('M24 52V32A8 8 0 0 1 32 24H52', A3, WA)
    return ic


@icon('model', 'Offset Two Sides')
def offset_two_sides():
    ic = sk('offset_two_sides', 'Offset Two Sides')
    ic.stroke('M10 52V30A20 20 0 0 1 30 10H54', A3, WA)
    ic.stroke('M20 52V32A12 12 0 0 1 32 20H54', LINE, W)
    ic.stroke('M30 52V34A4 4 0 0 1 34 30H54', A3, WA)
    return ic


@icon('model', 'Symmetric Offset')
def offset_symmetric():
    ic = sk('offset_symmetric', 'Symmetric Offset')
    ic.stroke('M8 20H56M8 44H56', A3, WA)
    ic.stroke('M8 32H56', LINE, WG, dash=DASH)
    return ic


@icon('model', 'Sketch Fillet')
def sketch_fillet():
    ic = sk('sketch_fillet', 'Sketch Fillet')
    ic.stroke('M12 52V34M34 12H52', LINE, W)
    ic.stroke('M12 34V12H34', LINE, WG, dash=DASH)
    ic.stroke('M12 34A22 22 0 0 1 34 12', A3, WA)
    return ic


@icon('model', 'Sketch Chamfer')
def sketch_chamfer():
    ic = sk('sketch_chamfer', 'Sketch Chamfer')
    ic.stroke('M12 52V32M32 12H52', LINE, W)
    ic.stroke('M12 32L32 12', A3, WA)
    ic.stroke('M12 32V12H32', LINE, WG, dash=DASH)
    return ic


@icon('model', 'Chamfer by Distance and Angle')
def sketch_chamfer_angle():
    ic = sketch_chamfer()
    ic.name, ic.title = 'sketch_chamfer_angle', 'Chamfer by Distance and Angle'
    ic.stroke('M12 46A14 14 0 0 1 22 22', A3, WG)
    return ic


@icon('model', 'Chamfer by Two Distances')
def sketch_chamfer_two_distances():
    ic = sketch_chamfer()
    ic.name, ic.title = 'sketch_chamfer_two_distances', 'Chamfer by Two Distances'
    ic.stroke('M5 12V32M32 5H12', A3, WG)
    return ic


@icon('model', 'Sketch Mirror')
def sketch_mirror():
    ic = sk('sketch_mirror', 'Sketch Mirror')
    ic.stroke('M32 8V56', LINE, WG, dash=DASH)
    ic.stroke('M26 16L10 26L22 46', LINE, W)
    ic.stroke('M38 16L54 26L42 46', A3, WA)
    return ic


@icon('model', 'Project')
def project():
    ic = sk('project', 'Project')
    d = Dim(1.0, 32, 40)
    ic.stroke(d.poly((-16, -16, 0), (16, -16, 0), (16, 16, 0), (-16, 16, 0)), LINE, W)
    ic.stroke(d.poly((-7, -7, 0), (7, -7, 0), (7, 7, 0), (-7, 7, 0)), A3, WA)
    ic.stroke(poly([d.p(0, 0, 22), d.p(0, 0, 3)], False), A3, WG, dash=DASH)
    ic.stroke(d.poly((-7, -7, 22), (7, -7, 22), (7, 7, 22), (-7, 7, 22)), LINE, W)
    return ic


@icon('model', 'Dimension')
def dimension():
    ic = sk('dimension', 'Dimension')
    ic.stroke('M12 46V22M52 46V22', LINE, W)
    arrow(ic, (32, 30), (13, 30), head=7, w=3.4)
    arrow(ic, (32, 30), (51, 30), head=7, w=3.4)
    ic.stroke('M22 46H42', A3, WA)
    return ic


@icon('model', 'Centerline')
def centerline():
    ic = sk('centerline', 'Centerline')
    ic.stroke('M8 52L56 12', A3, WA, dash='12 5 2 5')
    return ic


@icon('model', 'Construction')
def construction():
    ic = sk('construction', 'Construction')
    ic.stroke(ell(32, 32, 20), A3, WA, dash='6 4.5')
    return ic


@icon('model', 'Infinite Line')
def infinite():
    ic = sk('infinite', 'Infinite Line')
    ic.stroke('M18 40L46 24', A3, WA)
    ic.stroke('M4 48L18 40M46 24L60 16', A3, WG, dash='3 3')
    return ic


@icon('model', 'Snap')
def snap():
    ic = sk('snap', 'Snap')
    ic.stroke('M10 32H54M32 10V54', LINE, WG, dash=DASH)
    ic.stroke(rrect(22, 22, 42, 42, 2), A3, WA)
    point(ic, 32, 32, 3.6)
    return ic


@icon('model', 'Finish Sketch')
def finish():
    ic = sk('finish', 'Finish Sketch')
    ic.stroke('M12 34L26 48L52 18', A3, WA + 1.4)
    return ic


# ---------------------------------------------------------------- constraints
# The two things related, in ink; the relation itself, in the accent.

@icon('model', 'Coincident')
def c_coincident():
    ic = sk('c_coincident', 'Coincident')
    ic.stroke('M10 46L32 32M32 32L54 44', LINE, W)
    point(ic, 32, 32, 7)
    return ic


@icon('model', 'Collinear')
def c_collinear():
    ic = sk('c_collinear', 'Collinear')
    ic.stroke('M8 46L26 34M38 26L56 14', LINE, WA)
    ic.stroke('M26 34L38 26', A3, WG, dash='2.5 2.5')
    return ic


@icon('model', 'Concentric')
def c_concentric():
    ic = sk('c_concentric', 'Concentric')
    ic.stroke(ell(32, 32, 21), LINE, W)
    ic.stroke(ell(32, 32, 11), LINE, W)
    point(ic, 32, 32, 5)
    return ic


@icon('model', 'Equal')
def c_equal():
    ic = sk('c_equal', 'Equal')
    ic.stroke('M10 16H54M10 48H54', LINE, W)
    ic.stroke('M22 27H42M22 37H42', A3, WA)
    return ic


@icon('model', 'Fix')
def c_fix():
    ic = sk('c_fix', 'Fix')
    ic.stroke('M10 40L54 16', LINE, W)
    # A padlock: the relation is that nothing moves.
    ic.fill(rrect(22, 30, 42, 48, 3), A3)
    ic.stroke('M26 30V25A6 6 0 0 1 38 25V30', A3, WA)
    ic.dot(32, 39, 2.4, GAP)
    return ic


@icon('model', 'Horizontal / Vertical')
def c_hv():
    ic = sk('c_hv', 'Horizontal / Vertical')
    ic.stroke('M10 48H54', LINE, W)
    ic.stroke('M48 10V40', LINE, W)
    ic.stroke('M14 40H32M22 22V40', A3, WA)
    return ic


@icon('model', 'Midpoint')
def c_midpoint():
    ic = sk('c_midpoint', 'Midpoint')
    ic.stroke('M10 46L54 18', LINE, W)
    point(ic, 32, 32, 6)
    ic.stroke('M20 52L24 46M44 24L48 18', A3, W)
    return ic


@icon('model', 'Parallel')
def c_parallel():
    ic = sk('c_parallel', 'Parallel')
    ic.stroke('M10 44L40 14M24 52L54 22', LINE, W)
    ic.stroke('M28 34L34 28M34 40L40 34', A3, WA)
    return ic


@icon('model', 'Perpendicular')
def c_perpendicular():
    ic = sk('c_perpendicular', 'Perpendicular')
    ic.stroke('M10 50H54M20 50V10', LINE, W)
    ic.stroke('M20 38H32V50', A3, WA)
    return ic


@icon('model', 'Polygon')
def c_polygon():
    ic = sk('c_polygon', 'Polygon Constraint')
    h = ngon(32, 32, 20, 6)
    ic.stroke(poly(h), LINE, W)
    for p in h[::2]:
        point(ic, *p, r=3.8)
    return ic


@icon('model', 'Smooth')
def c_smooth():
    ic = sk('c_smooth', 'Smooth (G2)')
    ic.stroke('M8 44C18 44 24 40 32 32', LINE, W)
    ic.stroke('M32 32C40 24 46 20 56 20', LINE, W)
    ic.stroke('M18 46C24 44 28 38 32 32C36 26 42 22 48 20', A3, WG)
    point(ic, 32, 32, 5)
    return ic


@icon('model', 'Symmetry')
def c_symmetry():
    ic = sk('c_symmetry', 'Symmetry')
    ic.stroke('M32 8V56', LINE, WG, dash=DASH)
    ic.stroke('M12 22L22 42M52 22L42 42', LINE, W)
    ic.stroke('M18 32H26M38 32H46', A3, WA)
    return ic


@icon('model', 'Tangent')
def c_tangent():
    ic = sk('c_tangent', 'Tangent')
    ic.stroke(ell(30, 36, 16), LINE, W)
    ic.stroke('M6 20H58', LINE, W)
    point(ic, 30, 20, 5.5)
    return ic


@icon('model', 'AutoConstrain')
def auto_constrain():
    ic = sk('auto_constrain', 'AutoConstrain')
    ic.stroke(poly([(10, 46), (30, 18), (54, 46)]), LINE, W)
    ic.stroke('M18 46H46', A3, WA)
    ic.fill(poly([(46, 6), (40, 18), (46, 18), (42, 28), (52, 14), (46, 14), (50, 6)]), A3)
    return ic


@icon('model', 'Constrainer')
def constrainer():
    ic = sk('constrainer', 'Constrainer')
    ic.stroke('M10 46L32 18L54 46Z', LINE, W)
    point(ic, 10, 46); point(ic, 32, 18); point(ic, 54, 46)
    ic.stroke('M24 40H40', A3, WA)
    return ic


# ---------------------------------------------------------------- construct: planes, axes, points
# Construction geometry is the accent: a pale plane with its accent edge, an accent axis line,
# an accent point ring. What it is built from stays in ink.

def axis_line(ic, a, b, w=WA):
    ic.stroke(poly([a, b], False), A3, w)
    for p in (a, b):
        ic.dot(p[0], p[1], 2.6, A3)


def ref_block(ic, d, z=12, x0=-12, y0=-12, x1=12, y1=12, facet=None):
    block(ic, d, x0, y0, 0, x1, y1, z, facet=facet, colour=A1)


@icon('model', 'Axis')
def axis():
    ic = new('axis', 'Axis')
    axis_line(ic, (14, 54), (50, 10))
    ic.stroke('M10 44L22 50', LINE, WG)
    return ic


@icon('model', 'Axis Through Edge')
def axis_edge():
    ic = new('axis_edge', 'Axis Through Edge')
    d = frame(0.9, dy=4, height=14)
    ref_block(ic, d, 14)
    a, b = d.p(12, -12, 14), d.p(12, 12, 14)
    dx, dy = b[0] - a[0], b[1] - a[1]
    axis_line(ic, (a[0] - dx * 0.35, a[1] - dy * 0.35), (b[0] + dx * 0.35, b[1] + dy * 0.35))
    return ic


@icon('model', 'Axis Normal to Face')
def axis_normal_to_face():
    ic = new('axis_normal_to_face', 'Axis Normal to Face')
    d = frame(0.9, dy=10, height=8)
    ref_block(ic, d, 8, facet='top')
    c = d.p(0, 0, 8)
    axis_line(ic, (c[0], c[1] + 2), (c[0], c[1] - 30))
    ic.stroke(poly([(c[0] + 6, c[1] - 3), (c[0] + 6, c[1] - 9), (c[0], c[1] - 9)], False), LINE, WG)
    return ic


@icon('model', 'Axis Through Cylinder')
def axis_through_cylinder():
    ic = new('axis_through_cylinder', 'Axis Through Cylinder')
    d = frame(0.95, dy=2, height=16)
    cylinder(ic, d, 0, 0, 0, 16, 10, facet='side', colour=A1)
    (cx, cy), rx, ry = d.ell(0, 0, 16, 10)
    axis_line(ic, (cx, cy - 12), (cx, cy + 32))
    return ic


@icon('model', 'Axis Through Two Planes')
def axis_two_planes():
    ic = new('axis_two_planes', 'Axis Through Two Planes')
    ic.fill('M8 26L34 14V50L8 58Z', A1); ic.stroke('M8 26L34 14V50L8 58Z', LINE, W)
    ic.fill('M56 26L30 14V50L56 58Z', A1); ic.stroke('M56 26L30 14V50L56 58Z', LINE, W)
    axis_line(ic, (32, 6), (32, 58))
    return ic


@icon('model', 'Axis Through Two Points')
def axis_two_points():
    ic = new('axis_two_points', 'Axis Through Two Points')
    axis_line(ic, (10, 54), (54, 10), w=WG)
    ic.stroke('M10 54L54 10', A3, WA)
    point(ic, 21, 43, 5.4); point(ic, 43, 21, 5.4)
    return ic


@icon('model', 'Midplane')
def plane_midplane():
    ic = new('plane_midplane', 'Midplane')
    d = frame(0.95, dy=0, height=20)
    plane(ic, d, 13, 0, fill=None, stroke=LINE)
    plane(ic, d, 13, 20, fill=None, stroke=LINE)
    plane(ic, d, 13, 10)
    return ic


@icon('model', 'Plane at Angle')
def plane_angle():
    ic = new('plane_angle', 'Plane at Angle')
    ic.stroke('M8 52H56', LINE, W)
    ic.fill('M20 52L44 12L56 18L32 58Z', A1)
    ic.stroke('M20 52L44 12L56 18L32 58Z', A3, W)
    ic.stroke('M8 52H56', LINE, W)
    ic.stroke('M36 52A16 16 0 0 0 28 38', A3, WA)
    return ic


@icon('model', 'Perpendicular Plane')
def plane_perpendicular():
    ic = new('plane_perpendicular', 'Perpendicular Plane')
    ic.stroke('M8 46C20 46 30 40 38 26S50 10 58 10', LINE, W)
    ic.fill('M24 22L42 16V44L24 50Z', A1)
    ic.stroke('M24 22L42 16V44L24 50Z', A3, W)
    point(ic, 33, 33, 4.4)
    return ic


@icon('model', 'Tangent Plane')
def plane_tangent():
    ic = new('plane_tangent', 'Tangent Plane')
    d = frame(0.95, dy=6, height=12)
    cylinder(ic, d, 0, 0, 0, 12, 12)
    (cx, cy), rx, ry = d.ell(0, 0, 12, 12)
    ic.fill(poly([(cx + rx - 2, cy - 22), (cx + rx + 8, cy - 14), (cx + rx + 8, cy + 26), (cx + rx - 2, cy + 18)]), A1)
    ic.stroke(poly([(cx + rx - 2, cy - 22), (cx + rx + 8, cy - 14), (cx + rx + 8, cy + 26), (cx + rx - 2, cy + 18)]), A3, W)
    return ic


@icon('model', 'Plane Through Three Points')
def plane_three_points():
    ic = new('plane_three_points', 'Plane Through Three Points')
    d = frame(1.05, dy=0, height=0)
    q = [d.p(-14, -14, 0), d.p(14, -14, 0), d.p(14, 14, 0), d.p(-14, 14, 0)]
    ic.fill(poly(q), A1); ic.stroke(poly(q), A3, W)
    for p in (d.p(-8, -6, 0), d.p(8, -8, 0), d.p(2, 9, 0)):
        point(ic, *p, r=5)
    return ic


@icon('model', 'Plane Through Two Edges')
def plane_two_edges():
    ic = new('plane_two_edges', 'Plane Through Two Edges')
    d = frame(1.05, dy=0, height=0)
    q = [d.p(-14, -14, 0), d.p(14, -14, 0), d.p(14, 14, 0), d.p(-14, 14, 0)]
    ic.fill(poly(q), A1)
    ic.stroke(poly([q[0], q[1]], False), A3, WA); ic.stroke(poly([q[3], q[2]], False), A3, WA)
    ic.stroke(poly([q[1], q[2]], False), A3, WG); ic.stroke(poly([q[3], q[0]], False), A3, WG)
    return ic


@icon('model', 'Plane Along Path')
def plane_along_path():
    ic = new('plane_along_path', 'Plane Along Path')
    ic.stroke('M6 50C20 50 26 30 36 22S50 12 58 12', LINE, W)
    ic.fill('M30 14L44 10V34L30 38Z', A1); ic.stroke('M30 14L44 10V34L30 38Z', A3, W)
    point(ic, 37, 24, 4.2)
    return ic


@icon('model', 'Point at Vertex')
def point_vertex():
    ic = new('point_vertex', 'Point at Vertex')
    d = frame(0.95, dy=2, height=14)
    ref_block(ic, d, 14)
    point(ic, *d.p(12, 12, 14), r=6)
    return ic


@icon('model', 'Point Through Two Edges')
def point_two_edges():
    ic = new('point_two_edges', 'Point Through Two Edges')
    ic.stroke('M8 48L56 20M12 14L50 54', LINE, W)
    point(ic, 33.3, 33.3, 6.4)
    return ic


@icon('model', 'Point Through Three Planes')
def point_three_planes():
    ic = new('point_three_planes', 'Point Through Three Planes')
    d = frame(1.0, dy=2, height=14)
    ic.fill(d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0)), A1)
    ic.stroke(d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0)), LINE, W)
    ic.stroke(d.poly((0, -14, 0), (0, 14, 0), (0, 14, 22), (0, -14, 22)), LINE, W)
    ic.stroke(d.poly((-14, 0, 0), (14, 0, 0), (14, 0, 22), (-14, 0, 22)), LINE, W)
    point(ic, *d.p(0, 0, 0), r=6)
    return ic


@icon('model', 'Point at Edge and Plane')
def point_edge_plane():
    ic = new('point_edge_plane', 'Point at Edge and Plane')
    d = frame(1.0, dy=4, height=0)
    q = d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0))
    ic.fill(q, A1); ic.stroke(q, LINE, W)
    c = d.p(0, 0, 0)
    ic.stroke(poly([(c[0] - 8, c[1] - 26), c], False), LINE, W)
    ic.stroke(poly([c, (c[0] + 4, c[1] + 13)], False), LINE, WG, dash=DASH)
    point(ic, *c, r=6)
    return ic


@icon('model', 'Point at Center')
def point_center():
    ic = new('point_center', 'Point at Center')
    ic.stroke(ell(32, 32, 21), LINE, W)
    ic.stroke('M32 6V16M32 48V58M6 32H16M48 32H58', LINE, WG)
    point(ic, 32, 32, 6.4)
    return ic


@icon('model', 'Point Along Path')
def point_along_path():
    ic = new('point_along_path', 'Point Along Path')
    ic.stroke('M6 50C20 50 26 30 36 22S50 12 58 12', LINE, W)
    point(ic, 36, 22, 6.4)
    ic.stroke('M6 50C14 50 19 46 23 40', A3, WG)
    return ic


@icon('model', 'Origin')
def origin():
    ic = new('origin', 'Origin')
    d = Dim(1.0, 32, 36)
    c = d.p(0, 0, 0)
    for q in ((20, 0, 0), (0, 20, 0), (0, 0, 22)):
        ic.stroke(poly([c, d.p(*q)], False), LINE, WA)
    point(ic, *c, r=6)
    return ic


# ---------------------------------------------------------------- create: primitives and features

@icon('model', 'Box')
def box():
    ic = new('box', 'Box')
    d = frame(1.0, dy=0, height=20)
    block(ic, d, -12, -12, 0, 12, 12, 20, facet=['top'])
    return ic


@icon('model', 'Cylinder')
def cylinder_():
    ic = new('cylinder', 'Cylinder')
    d = frame(1.0, dy=0, height=20)
    cylinder(ic, d, 0, 0, 0, 20, 12, facet='top')
    return ic


@icon('model', 'Sphere')
def sphere():
    ic = new('sphere', 'Sphere')
    ic.stroke(ell(32, 32, 22), LINE, W)
    ic.fill('M10 32A22 9 0 0 0 54 32A22 22 0 0 0 10 32Z', A2)
    ic.stroke('M10 32A22 9 0 0 0 54 32', LINE, W)
    ic.stroke(ell(32, 32, 22), LINE, W)
    return ic


@icon('model', 'Torus')
def torus():
    ic = new('torus', 'Torus')
    ic.fill(ell(32, 34, 25, 15), A2)
    ic.stroke(ell(32, 34, 25, 15), LINE, W)
    ic.fill(ell(32, 31, 10, 5), GAP)
    ic.stroke('M20 31A12 6 0 0 0 44 31', LINE, W)
    ic.stroke('M23 34A10 5 0 0 1 41 34', LINE, W)
    return ic


@icon('model', 'Coil')
def coil():
    ic = new('coil', 'Coil')
    for k in range(4):
        y = 14 + k * 10
        ic.stroke('M12 %sC12 %s 52 %s 52 %s' % (y, y + 7, y + 3, y + 10), A3 if k % 2 == 0 else LINE, WA if k % 2 == 0 else W)
    return ic


@icon('model', 'Pipe')
def pipe():
    ic = new('pipe', 'Pipe')
    ic.stroke('M12 52C12 30 28 14 52 14', A2, 14)
    ic.stroke('M12 52C12 30 28 14 52 14', LINE, WG, dash=DASH)
    ic.stroke(ell(52, 14, 3.5, 7), LINE, W, fill=GAP)
    ic.stroke(ell(12, 52, 7, 3.5), LINE, W)
    return ic


@icon('model', 'Rib')
def rib():
    ic = new('rib', 'Rib')
    d = frame(0.95, dy=2, height=20)
    block(ic, d, -12, -12, 0, 12, -7, 20)
    block(ic, d, -12, -7, 0, 12, 12, 5)
    g = d.poly((0, -7, 18), (0, 10, 5), (0, -7, 5))
    ic.fill(g, A2)
    ic.stroke(g, LINE, W)
    return ic


@icon('model', 'Draft')
def draft():
    ic = new('draft', 'Draft')
    d = frame(1.0, dy=2, height=18)
    t = 5
    top = d.poly((-12, -12, 18), (12 - t, -12, 18), (12 - t, 12 - t, 18), (-12, 12 - t, 18))
    left = d.poly((-12, 12 - t, 18), (12 - t, 12 - t, 18), (12, 12, 0), (-12, 12, 0))
    right = d.poly((12 - t, -12, 18), (12 - t, 12 - t, 18), (12, 12, 0), (12, -12, 0))
    ic.fill(left, A2)
    for q in (top, left, right):
        ic.stroke(q, LINE, W)
    return ic


@icon('model', 'Thread')
def thread():
    ic = new('thread', 'Thread')
    ic.stroke('M22 8V56M42 8V56', LINE, W)
    for k in range(6):
        y = 12 + k * 8
        ic.stroke('M18 %sL46 %s' % (y + 4, y), A3, WA)
    return ic


@icon('model', 'Boss')
def boss():
    ic = new('boss', 'Boss')
    d = frame(1.0, dy=6, height=6)
    block(ic, d, -14, -14, 0, 14, 14, 6)
    cylinder(ic, d, 0, 0, 6, 18, 6, facet='top')
    return ic


@icon('model', 'Emboss')
def emboss():
    ic = new('emboss', 'Emboss')
    d = frame(1.0, dy=4, height=8)
    block(ic, d, -14, -14, 0, 14, 14, 8)
    ic.fill(d.poly((-6, -6, 8), (6, -6, 8), (6, -2, 8), (-2, -2, 8), (-2, 8, 8), (-6, 8, 8)), A2)
    ic.stroke(d.poly((-6, -6, 8), (6, -6, 8), (6, -2, 8), (-2, -2, 8), (-2, 8, 8), (-6, 8, 8)), LINE, W)
    return ic


@icon('model', 'Scale')
def scale():
    ic = new('scale', 'Scale')
    ic.stroke(rrect(10, 30, 34, 54, 2), LINE, WA)
    ic.fill(rrect(10, 10, 54, 54, 3), A1)
    ic.stroke(rrect(10, 10, 54, 54, 3), A3, W, dash=DASH)
    ic.fill(rrect(10, 30, 34, 54, 2), GAP)
    ic.stroke(rrect(10, 30, 34, 54, 2), LINE, WA)
    arrow(ic, (30, 34), (48, 16), head=8)
    return ic


@icon('model', 'Combine')
def combine():
    ic = new('combine', 'Combine')
    ic.fill(ell(25, 32, 15), A2)
    ic.fill(ell(39, 32, 15), A2)
    ic.stroke('M32 18.6A15 15 0 1 0 32 45.4A15 15 0 1 0 32 18.6Z', LINE, W)
    ic.stroke(ell(25, 32, 15), LINE, W)
    ic.stroke(ell(39, 32, 15), LINE, W)
    return ic


@icon('model', 'Split Body')
def split_body():
    ic = new('split_body', 'Split Body')
    a = frame(0.85, dx=-5, dy=3, height=16)
    b = frame(0.85, dx=5, dy=-3, height=16)
    block(ic, a, -12, -12, 0, 0, 12, 16)
    block(ic, b, 0, -12, 0, 12, 12, 16, facet='right')
    return ic


@icon('model', 'Split Face')
def split_face():
    ic = new('split_face', 'Split Face')
    d = frame(1.0, dy=2, height=16)
    block(ic, d, -12, -12, 0, 12, 12, 16)
    ic.fill(d.poly((-12, 12, 16), (12, 12, 16), (12, 12, 8), (-12, 12, 8)), A2)
    ic.stroke(poly([d.p(-12, 12, 8), d.p(12, 12, 8), d.p(12, -12, 8)], False), A3, WA)
    return ic


@icon('model', 'Offset Face')
def offset_face():
    ic = new('offset_face', 'Offset Face')
    d = frame(0.95, dy=4, height=12)
    block(ic, d, -12, -12, 0, 12, 12, 12)
    ic.stroke(d.poly((-12, -12, 20), (12, -12, 20), (12, 12, 20), (-12, 12, 20)), A3, W, dash=DASH)
    ic.fill(d.poly((-12, -12, 20), (12, -12, 20), (12, 12, 20), (-12, 12, 20)), A1)
    arrow(ic, d.p(0, 0, 12.5), d.p(0, 0, 19.5), head=6, w=3.4)
    return ic


@icon('model', 'Delete Face')
def delete_face():
    ic = new('delete_face', 'Delete Face')
    d = frame(1.0, dy=2, height=16)
    edges(ic, d, -12, -12, 0, 12, 12, 16)
    ic.fill(d.poly((-12, -12, 16), (12, -12, 16), (12, 12, 16), (-12, 12, 16)), GAP)
    ic.stroke(d.poly((-12, -12, 16), (12, -12, 16), (12, 12, 16), (-12, 12, 16)), A3, W, dash=DASH)
    ic.stroke(poly([d.p(-12, 12, 16), d.p(12, 12, 16), d.p(12, -12, 16)], False), LINE, W)
    badge(ic, 48, 14, 'cross')
    return ic


@icon('model', 'Replace Face')
def replace_face():
    ic = new('replace_face', 'Replace Face')
    d = frame(0.95, dy=6, height=10)
    block(ic, d, -12, -12, 0, 12, 12, 10)
    ic.fill('M10 18C22 8 42 8 54 18L54 22C42 12 22 12 10 22Z', A2)
    ic.stroke('M10 20C22 10 42 10 54 20', A3, WA)
    arrow(ic, d.p(0, 0, 10.5), (32, 16), head=6, w=3.4)
    return ic


@icon('model', 'Thicken')
def thicken():
    ic = new('thicken', 'Thicken')
    ic.fill('M8 40C20 26 44 26 56 40L56 50C44 36 20 36 8 50Z', A2)
    ic.stroke('M8 40C20 26 44 26 56 40', LINE, W)
    ic.stroke('M8 50C20 36 44 36 56 50M8 40V50M56 40V50', LINE, W)
    arrow(ic, (32, 30), (32, 12), head=7, w=3.4)
    return ic


@icon('model', 'Patch')
def patch():
    ic = new('patch', 'Patch')
    q = 'M10 24L32 12L54 24L52 46L32 54L12 46Z'
    ic.fill(q, A1)
    ic.stroke(q, LINE, W)
    ic.stroke('M18 30C26 24 38 24 46 30C40 40 24 40 18 30Z', A3, WA)
    return ic


@icon('model', 'Stitch')
def stitch():
    ic = new('stitch', 'Stitch')
    ic.fill('M8 18L30 14V50L8 46Z', A1); ic.stroke('M8 18L30 14V50L8 46Z', LINE, W)
    ic.fill('M56 18L34 14V50L56 46Z', A1); ic.stroke('M56 18L34 14V50L56 46Z', LINE, W)
    for y in (20, 30, 40):
        ic.stroke('M26 %sL38 %s' % (y, y + 4), A3, WA)
    return ic


@icon('model', 'Unstitch')
def unstitch():
    ic = new('unstitch', 'Unstitch')
    ic.fill('M6 18L26 14V50L6 46Z', A1); ic.stroke('M6 18L26 14V50L6 46Z', LINE, W)
    ic.fill('M58 18L38 14V50L58 46Z', A1); ic.stroke('M58 18L38 14V50L58 46Z', LINE, W)
    ic.stroke('M32 14V50', A3, WG, dash=DASH)
    return ic


@icon('model', 'Boundary Surface')
def boundary():
    ic = new('boundary', 'Boundary Surface')
    q = 'M10 22C22 16 42 16 54 22C50 32 50 40 54 48C42 54 22 54 10 48C14 40 14 32 10 22Z'
    ic.fill(q, A1)
    ic.stroke(q, A3, WA)
    return ic


@icon('model', 'Ruled Surface')
def ruled():
    ic = new('ruled', 'Ruled Surface')
    ic.fill('M8 16C24 10 40 22 56 16L56 48C40 54 24 42 8 48Z', A1)
    for x in (16, 26, 36, 46):
        ic.stroke('M%s 15V47' % x, LINE, WG)
    ic.stroke('M8 16C24 10 40 22 56 16M8 48C24 42 40 54 56 48', A3, WA)
    return ic


@icon('model', 'Blend Surface')
def blend_surface():
    ic = new('blend_surface', 'Blend Surface')
    ic.stroke('M6 44H22M42 20H58', LINE, WA)
    ic.fill('M22 40C32 40 32 24 42 24V16C28 16 34 32 22 32Z', A1)
    ic.stroke('M22 44C34 44 30 20 42 20', A3, WA)
    return ic


@icon('model', 'Offset Surface')
def offset_surface():
    ic = new('offset_surface', 'Offset Surface')
    ic.stroke('M8 46C20 34 44 34 56 46', LINE, W)
    ic.fill('M8 30C20 18 44 18 56 30L56 34C44 22 20 22 8 34Z', A1)
    ic.stroke('M8 30C20 18 44 18 56 30', A3, WA)
    ic.stroke('M32 37V25', A3, WG, dash='2.5 2.5')
    return ic


@icon('model', 'Reverse Normal')
def reverse_normal():
    ic = new('reverse_normal', 'Reverse Normal')
    d = frame(1.05, dy=8, height=0)
    q = d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0))
    ic.fill(q, A1); ic.stroke(q, LINE, W)
    c = d.p(0, 0, 0)
    ic.stroke(poly([c, (c[0], c[1] - 26)], False), LINE, WG, dash=DASH)
    arrow(ic, (c[0], c[1]), (c[0], c[1] + 14), head=7)
    arc_arrow(ic, c[0] + 8, c[1] - 14, 8, 8, -90, 90, head=6, w=3.4)
    return ic


@icon('model', 'Untrim')
def untrim():
    ic = new('untrim', 'Untrim')
    q = 'M10 16H54V48H10Z'
    ic.fill(q, A1)
    ic.stroke(q, A3, W, dash=DASH)
    ic.stroke('M10 16H40C34 26 34 38 40 48H10Z', LINE, W)
    return ic


@icon('model', 'Surface')
def surface():
    ic = new('surface', 'Surface')
    q = 'M8 28C20 16 30 30 42 18C48 14 52 14 56 16L56 40C46 38 40 52 26 46C18 42 12 46 8 50Z'
    ic.fill(q, A2)
    ic.stroke(q, LINE, W)
    return ic


# ---------------------------------------------------------------- documents and utilities

def doc(ic, x0=14, y0=8, x1=46, y1=56, fold=10, fill=None):
    q = 'M%s %sH%sL%s %sV%sH%sZ' % (x0, y0, x1 - fold, x1, y0 + fold, y1, x0)
    if fill:
        ic.fill(q, fill)
    ic.stroke(q, LINE, WA)
    ic.stroke('M%s %sV%sH%s' % (x1 - fold, y0, y0 + fold, x1), LINE, W)


@icon('model', 'New')
def new_():
    ic = new('new', 'New')
    doc(ic, 12, 8, 44, 56)
    badge(ic, 46, 46, 'plus', r=10)
    return ic


@icon('model', 'Open')
def open_():
    ic = new('open', 'Open')
    ic.stroke('M8 20A4 4 0 0 1 12 16H24L29 21H46A4 4 0 0 1 50 25V30', LINE, WA)
    ic.fill('M8 52L16 30H58L50 52Z', A2)
    ic.stroke('M8 52V20M8 52L16 30H58L50 52Z', LINE, WA)
    return ic


@icon('model', 'Copy')
def copy():
    ic = new('copy', 'Copy')
    ic.stroke(rrect(10, 8, 38, 42, 3), LINE, WA)
    ic.fill(rrect(24, 22, 54, 56, 3), A1)
    ic.stroke(rrect(24, 22, 54, 56, 3), LINE, WA)
    return ic


@icon('model', 'Paste')
def paste():
    ic = new('paste', 'Paste')
    ic.stroke('M18 12H14A3 3 0 0 0 11 15V53A3 3 0 0 0 14 56H44A3 3 0 0 0 47 53V40', LINE, WA)
    ic.stroke(rrect(20, 7, 38, 16, 2), LINE, W)
    ic.fill(rrect(28, 26, 54, 50, 3), A2)
    ic.stroke(rrect(28, 26, 54, 50, 3), LINE, WA)
    return ic


@icon('model', 'Cut')
def cut():
    ic = new('cut', 'Cut')
    ic.stroke(ell(18, 46, 8), LINE, WA)
    ic.stroke(ell(46, 46, 8), LINE, WA)
    ic.stroke('M23 40L46 8M41 40L18 8', LINE, WA)
    ic.dot(32, 25, 3.5, A3)
    return ic


@icon('model', 'Delete')
def delete():
    ic = new('delete', 'Delete')
    ic.stroke('M10 15H54M25 15V10H39V15', LINE, WA)
    # The bin is the facet; its slats are punched out of it.
    ic.fill('M14 21H50L46.5 56H17.5Z', A2)
    ic.stroke('M27 29V48M37 29V48', GAP, 3.2)
    ic.stroke('M14 21H50L46.5 56H17.5Z', LINE, W)
    return ic


@icon('model', 'Edit')
def edit():
    ic = new('edit', 'Edit')
    ic.fill('M14 50L16 40L42 14L50 22L24 48Z', A2)
    ic.stroke('M14 50L16 40L42 14L50 22L24 48Z', LINE, WA)
    ic.stroke('M36 20L44 28', LINE, W)
    ic.stroke('M10 56H54', LINE, W)
    return ic


@icon('model', 'Rename')
def rename():
    ic = new('rename', 'Rename')
    ic.stroke(rrect(6, 20, 58, 44, 4), LINE, WA)
    ic.fill('M14 28H34V36H14Z', A1)
    ic.stroke('M40 14V50M36 14H44M36 50H44', A3, WA)
    return ic


@icon('model', 'Library')
def library():
    ic = new('library', 'Library')
    ic.stroke('M10 12V52M20 12V52', LINE, WA)
    ic.fill('M30 14L39 12L47 50L38 52Z', A2)
    ic.stroke('M30 14L39 12L47 50L38 52Z', LINE, WA)
    ic.stroke('M8 56H56', LINE, W)
    return ic


@icon('model', 'List')
def list_():
    ic = new('list', 'List')
    for y in (16, 32, 48):
        ic.dot(13, y, 3.8, A3)
        ic.stroke('M24 %sH54' % y, LINE, WA)
    return ic


@icon('model', 'Filter')
def filter_():
    ic = new('filter', 'Filter')
    ic.fill('M8 12H56L38 32V52L26 46V32Z', A1)
    ic.stroke('M8 12H56L38 32V52L26 46V32Z', LINE, WA)
    return ic


@icon('model', 'Folder')
def folder_():
    ic = new('folder', 'Folder')
    ic.stroke('M8 18A4 4 0 0 1 12 14H24L30 20H52A4 4 0 0 1 56 24V46A4 4 0 0 1 52 50H12A4 4 0 0 1 8 46Z', LINE, WA)
    ic.fill('M8 28H56V46A4 4 0 0 1 52 50H12A4 4 0 0 1 8 46Z', A1)
    ic.stroke('M8 28H56', LINE, W)
    return ic


@icon('model', 'Import')
def import_():
    ic = new('import', 'Import')
    doc(ic, 20, 8, 52, 56, fill=A1)
    arrow(ic, (4, 34), (32, 34), gap=1.4)
    return ic


@icon('model', 'Export')
def export():
    ic = new('export', 'Export')
    doc(ic, 10, 8, 42, 56, fill=A1)
    arrow(ic, (28, 34), (58, 34), gap=1.4)
    return ic


@icon('model', 'More')
def dots():
    ic = new('dots', 'More')
    for x in (14, 32, 50):
        ic.dot(x, 32, 5, LINE)
    return ic


@icon('model', 'Select')
def select():
    ic = new('select', 'Select')
    ic.fill('M14 8L48 34L33 36L42 54L35 57L26 39L14 48Z', A2)
    ic.stroke('M14 8L48 34L33 36L42 54L35 57L26 39L14 48Z', LINE, WA)
    return ic


@icon('model', 'Reorder')
def reorder():
    ic = new('reorder', 'Reorder')
    ic.stroke('M8 16H38M8 32H38M8 48H38', LINE, WA)
    arrow(ic, (50, 44), (50, 12), head=8)
    arrow(ic, (50, 20), (50, 52), head=8) if False else None
    return ic


@icon('model', 'Update')
def update():
    ic = new('update', 'Update')
    arc_arrow(ic, 32, 32, 19, 19, 200, 470, head=9)
    ic.dot(32, 32, 4.4, A2)
    return ic


@icon('model', 'Compute')
def compute():
    ic = new('compute', 'Compute')
    ic.stroke(rrect(12, 12, 52, 52, 5), LINE, WA)
    ic.fill(rrect(21, 21, 43, 43, 3), A2)
    ic.stroke('M22 6V12M32 6V12M42 6V12M22 52V58M32 52V58M42 52V58M6 22H12M6 32H12M6 42H12M52 22H58M52 32H58M52 42H58', LINE, W)
    return ic


@icon('model', 'Checkpoint')
def checkpoint():
    ic = new('checkpoint', 'Checkpoint')
    ic.stroke('M16 58V8', LINE, WA)
    ic.fill('M16 10H50L42 22L50 34H16Z', A2)
    ic.stroke('M16 10H50L42 22L50 34H16Z', LINE, W)
    return ic


@icon('model', 'Rollback')
def rollback():
    ic = new('rollback', 'Rollback')
    ic.stroke('M8 32H56', LINE, WG, dash=DASH)
    for x in (12, 24, 36, 48):
        ic.dot(x, 32, 3.6, LINE)
    ic.stroke('M30 12V52', A3, WA)
    ic.fill(poly([(22, 10), (38, 10), (30, 20)]), A3)
    return ic


@icon('model', 'Restore')
def restore():
    ic = new('restore', 'Restore')
    arc_arrow(ic, 32, 32, 19, 19, 470, 200, head=9)
    ic.stroke('M32 20V32L40 38', LINE, WA)
    return ic


@icon('model', 'Play')
def play():
    ic = new('play', 'Play')
    ic.fill(poly([(18, 10), (52, 32), (18, 54)]), A2)
    ic.stroke(poly([(18, 10), (52, 32), (18, 54)]), LINE, W)
    return ic


@icon('model', 'Rest')
def rest():
    ic = new('rest', 'Rest')
    ic.fill(rrect(14, 12, 26, 52, 3), A2)
    ic.fill(rrect(38, 12, 50, 52, 3), A2)
    ic.stroke(rrect(14, 12, 26, 52, 3), LINE, W)
    ic.stroke(rrect(38, 12, 50, 52, 3), LINE, W)
    return ic


@icon('model', 'Web')
def web():
    ic = new('web', 'Web')
    ic.stroke(ell(32, 32, 22), LINE, WA)
    ic.stroke('M10 32H54M32 10C22 20 22 44 32 54C42 44 42 20 32 10', LINE, W)
    ic.fill('M32 10C42 20 42 44 32 54A22 22 0 0 0 32 10Z', A1)
    return ic


@icon('model', 'Insert SVG')
def svg_():
    ic = new('svg', 'Insert SVG')
    doc(ic, 12, 8, 44, 56)
    ic.stroke('M20 46C20 30 36 30 36 18', A3, WA)
    point(ic, 20, 46, 3.6); point(ic, 36, 18, 3.6)
    return ic


@icon('model', 'DXF')
def dxf():
    ic = new('dxf', 'DXF')
    doc(ic, 12, 8, 44, 56)
    ic.stroke('M20 44H36V28', A3, WA)
    ic.stroke(ell(28, 36, 5), A3, W)
    return ic


@icon('model', 'Placeholder')
def placeholder():
    ic = new('placeholder', 'Placeholder')
    ic.stroke(rrect(12, 12, 52, 52, 6), LINE, WA, dash='6 5')
    return ic


@icon('model', 'Include')
def include():
    ic = new('include', 'Include')
    d = frame(1.0, dy=0, height=0)
    q = d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0))
    ic.stroke(q, LINE, W)
    ic.stroke(d.poly((-6, -6, 0), (6, -6, 0), (6, 6, 0), (-6, 6, 0)), A3, WA)
    ic.stroke(poly([d.p(6, -6, 18), d.p(6, -6, 1)], False), A3, WG, dash=DASH)
    arrow(ic, d.p(-6, 6, 16), d.p(-6, 6, 2), head=6, w=3.4)
    return ic


@icon('model', 'Dependency Graph')
def graph():
    ic = new('graph', 'Dependency Graph')
    ic.stroke('M16 16L32 32L48 16M32 32V50', LINE, W)
    for p in ((16, 16), (48, 16), (32, 50)):
        ic.dot(p[0], p[1], 6, LINE)
    ic.dot(32, 32, 7, A2)
    return ic


@icon('model', 'Accessibility')
def accessibility():
    ic = new('accessibility', 'Accessibility')
    ic.stroke(ell(32, 32, 23), LINE, WA)
    ic.dot(32, 18, 4.2, A3)
    ic.stroke('M18 26H46M32 26V38L24 50M32 38L40 50', A3, WA)
    return ic


@icon('model', 'Activate')
def activate():
    ic = new('activate', 'Activate')
    ic.stroke(ell(32, 32, 22), LINE, WA)
    ic.fill(ell(32, 32, 12), A2)
    return ic


@icon('model', 'Align')
def align():
    ic = new('align', 'Align')
    ic.stroke('M10 8V56', A3, WA)
    ic.fill(rrect(16, 14, 50, 26, 3), A1); ic.stroke(rrect(16, 14, 50, 26, 3), LINE, W)
    ic.fill(rrect(16, 38, 38, 50, 3), A1); ic.stroke(rrect(16, 38, 38, 50, 3), LINE, W)
    return ic


@icon('model', 'Error')
def status_error():
    ic = new('status_error', 'Error')
    oct_ = ngon(32, 32, 23, 8, -112.5)
    ic.fill(poly(oct_), A2)
    ic.stroke('M24 24L40 40M40 24L24 40', GAP, WA + 0.6)
    return ic


@icon('model', 'Warning')
def status_warning():
    ic = new('status_warning', 'Warning')
    ic.fill('M32 8L58 54H6Z', A2)
    ic.stroke('M32 8L58 54H6Z', A2, 3, fill=None)
    ic.stroke('M32 24V38', GAP, WA + 0.6)
    ic.dot(32, 46, 3.2, GAP)
    return ic


@icon('model', 'Recover Unsaved Design')
def status_recovery():
    ic = new('status_recovery', 'Recover Unsaved Design')
    doc(ic, 14, 8, 46, 56)
    arc_arrow(ic, 30, 36, 9, 9, 160, 420, head=7, w=3.6)
    return ic


# ---------------------------------------------------------------- inspect

@icon('model', 'Measure')
def measure():
    ic = new('measure', 'Measure')
    ic.fill('M8 40L40 8L56 24L24 56Z', A1)
    ic.stroke('M8 40L40 8L56 24L24 56Z', LINE, WA)
    for k in range(1, 5):
        x, y = 8 + k * 6.4, 40 - k * 6.4
        ic.stroke('M%s %sL%s %s' % (f(x), f(y), f(x + (5 if k % 2 else 9) * 0.707), f(y + (5 if k % 2 else 9) * 0.707)), LINE, W)
    return ic


@icon('model', 'Inspect')
def inspect():
    ic = new('inspect', 'Inspect')
    ic.fill(ell(27, 27, 15), A1)
    ic.stroke(ell(27, 27, 15), LINE, WA)
    ic.stroke('M38 38L54 54', LINE, WA + 1.6)
    ic.stroke('M20 30L26 24L34 30', A3, WA)
    return ic


@icon('model', 'Interference')
def interference():
    ic = new('interference', 'Interference')
    ic.stroke(rrect(8, 8, 38, 38, 3), LINE, WA)
    ic.stroke(rrect(26, 26, 56, 56, 3), LINE, WA)
    ic.fill(rrect(26, 26, 38, 38, 1), A2)
    return ic


@icon('model', 'Section')
def section():
    ic = new('section', 'Section')
    d = frame(1.0, dy=2, height=18)
    edges(ic, d, -12, -12, 0, 12, 12, 18)
    ic.fill(d.poly((0, -12, 18), (0, 12, 18), (0, 12, 0), (0, -12, 0)), A2)
    ic.stroke(d.poly((0, -12, 18), (0, 12, 18), (0, 12, 0), (0, -12, 0)), A3, W)
    return ic


@icon('model', 'Zebra')
def zebra():
    ic = new('zebra', 'Zebra')
    ic.clip_begin('zebra', ell(32, 32, 22))
    for k in (-2, 0, 2):
        x = 32 + k * 9
        ic.fill('M%s 4C%s 24 %s 40 %s 60H%sC%s 40 %s 24 %s 4Z' % (x - 9, x - 1, x - 7, x + 1, x + 10, x + 2, x + 8, x), A2)
    ic.clip_end()
    ic.stroke(ell(32, 32, 22), LINE, WA)
    return ic


@icon('model', 'Curvature Comb')
def curvature_comb():
    ic = new('curvature_comb', 'Curvature Comb')
    pts = [(8 + i * 4, 44 - 16 * math.sin(i / 12 * math.pi)) for i in range(13)]
    tips = []
    for i, (x, y) in enumerate(pts):
        k = 6 + 10 * math.sin(i / 12 * math.pi)
        tips.append((x, y - k))
        ic.stroke('M%s %sL%s %s' % (f(x), f(y), f(x), f(y - k)), A3, 2.2)
    ic.stroke(poly(tips, False), A3, W)
    ic.stroke(poly(pts, False), LINE, WA)
    return ic


@icon('model', 'Curvature Map')
def curvature_map():
    ic = new('curvature_map', 'Curvature Map')
    q = 'M8 28C20 16 30 30 42 18C48 14 52 14 56 16L56 40C46 38 40 52 26 46C18 42 12 46 8 50Z'
    ic.clip_begin('cmap', q)
    ic.fill('M0 0H22V64H0Z', A3); ic.fill('M22 0H40V64H22Z', A2); ic.fill('M40 0H64V64H40Z', A1)
    ic.clip_end()
    ic.stroke(q, LINE, W)
    return ic


@icon('model', 'Draft Analysis')
def draft_analysis():
    ic = new('draft_analysis', 'Draft Analysis')
    d = frame(1.0, dy=2, height=18)
    t = 5
    left = d.poly((-12, 12 - t, 18), (12 - t, 12 - t, 18), (12, 12, 0), (-12, 12, 0))
    right = d.poly((12 - t, -12, 18), (12 - t, 12 - t, 18), (12, 12, 0), (12, -12, 0))
    ic.fill(left, A2); ic.fill(right, A3)
    for q in (d.poly((-12, -12, 18), (12 - t, -12, 18), (12 - t, 12 - t, 18), (-12, 12 - t, 18)), left, right):
        ic.stroke(q, LINE, W)
    arrow(ic, (52, 8), (52, 22), colour=LINE, head=6, w=3)
    return ic


@icon('model', 'Continuity')
def edge_continuity():
    ic = new('edge_continuity', 'Continuity')
    ic.stroke('M6 46C16 46 24 42 32 32', LINE, WA)
    ic.stroke('M32 32C40 22 48 18 58 18', A3, WA)
    point(ic, 32, 32, 5.4)
    return ic


@icon('model', 'Center of Mass')
def center_of_mass():
    ic = new('center_of_mass', 'Center of Mass')
    ic.stroke(ell(32, 32, 20), LINE, WA)
    ic.fill('M32 12A20 20 0 0 1 52 32H32Z', A2)
    ic.fill('M32 52A20 20 0 0 1 12 32H32Z', A2)
    ic.stroke('M12 32H52M32 12V52', LINE, W)
    return ic


@icon('model', 'Compare Bodies')
def compare():
    ic = new('compare', 'Compare Bodies')
    ic.stroke(rrect(6, 14, 32, 50, 3), LINE, WA)
    ic.fill(rrect(32, 14, 58, 50, 3), A1)
    ic.stroke(rrect(32, 14, 58, 50, 3), A3, WA, dash='5 4')
    ic.stroke('M32 8V56', LINE, W)
    return ic


@icon('model', 'Isoparametric Analysis')
def iso_analysis():
    ic = new('iso_analysis', 'Isoparametric Analysis')
    q = 'M8 18C24 10 40 24 56 16L56 48C40 56 24 42 8 50Z'
    ic.fill(q, A1)
    for x in (20, 32, 44):
        ic.stroke('M%s %sV%s' % (x, 14 + (x - 8) / 4 * 0.0, 50), A3, WG)
    ic.stroke('M8 34C24 26 40 40 56 32', A3, WG)
    ic.stroke(q, LINE, W)
    return ic


@icon('model', 'Isoparametric Curve')
def iso_curve():
    ic = new('iso_curve', 'Isoparametric Curve')
    q = 'M8 18C24 10 40 24 56 16L56 48C40 56 24 42 8 50Z'
    ic.stroke(q, LINE, W)
    ic.stroke('M8 34C24 26 40 40 56 32', A3, WA)
    return ic


@icon('model', 'Intersection Curve')
def intersection_curve():
    ic = new('intersection_curve', 'Intersection Curve')
    ic.stroke('M6 42C20 30 44 30 58 42', LINE, W)
    ic.stroke('M22 8C30 20 30 44 22 56', LINE, W)
    ic.stroke('M27 18C29 26 29 36 27 46', A3, WA)
    return ic


@icon('model', 'Intersect')
def intersect():
    ic = new('intersect', 'Intersect')
    ic.stroke(ell(25, 32, 15), LINE, W)
    ic.stroke(ell(39, 32, 15), LINE, W)
    ic.clip_begin('isect', ell(25, 32, 15))
    ic.fill(ell(39, 32, 15), A2)
    ic.clip_end()
    ic.stroke('M32 18.6A15 15 0 0 1 32 45.4A15 15 0 0 1 32 18.6Z', LINE, W)
    return ic


@icon('model', 'Static Stress')
def analysis_stress():
    ic = new('analysis_stress', 'Static Stress')
    d = Dim(1.1, 34, 40)
    # A beam held at its left end: its top carries the stress as three bands, highest at the
    # root, and the load presses on the free end.
    for a, b, c in ((-16, -6, A3), (-6, 5, A2), (5, 16, A1)):
        ic.fill(d.poly((a, -5, 8), (b, -5, 8), (b, 5, 8), (a, 5, 8)), c)
    edges(ic, d, -16, -5, 0, 16, 5, 8)
    w = poly([d.p(-17.5, -9, -4), d.p(-17.5, 9, -4), d.p(-17.5, 9, 14), d.p(-17.5, -9, 14)])
    ic.stroke(w, LINE, WG)
    arrow(ic, d.p(13, 0, 24), d.p(13, 0, 9), head=7)
    return ic


@icon('model', 'Minimum Radius')
def min_radius():
    ic = new('min_radius', 'Minimum Radius')
    ic.stroke('M8 50C20 50 26 14 34 14S46 50 56 50', LINE, WA)
    ic.stroke(ell(34, 22, 8), A3, WG)
    ic.stroke('M34 22L40 16', A3, WG)
    point(ic, 34, 14, 3.6)
    return ic


@icon('model', 'Normal Line')
def normal():
    ic = new('normal', 'Normal Line')
    ic.stroke('M6 46C20 30 44 30 58 46', LINE, WA)
    arrow(ic, (32, 34), (32, 8), head=8)
    point(ic, 32, 34, 4)
    return ic


@icon('model', 'Silhouette')
def silhouette():
    ic = new('silhouette', 'Silhouette')
    ic.fill(ell(32, 32, 22), A1)
    ic.stroke(ell(32, 32, 22), A3, WA)
    ic.stroke('M10 32A22 9 0 0 0 54 32', LINE, WG, dash=DASH)
    return ic


@icon('model', 'Fit Curves to Section')
def fit_section():
    ic = new('fit_section', 'Fit Curves to Section')
    for x, y in ((10, 40), (16, 30), (24, 24), (34, 24), (44, 30), (52, 40)):
        ic.dot(x, y, 2.6, LINE)
    ic.stroke('M8 44C14 22 50 22 56 44', A3, WA)
    return ic


@icon('model', 'Project to Surface')
def project_surface():
    ic = new('project_surface', 'Project to Surface')
    ic.fill('M6 44C20 34 44 34 58 44L58 54C44 44 20 44 6 54Z', A1)
    ic.stroke('M6 44C20 34 44 34 58 44', LINE, W)
    ic.stroke(ell(32, 14, 10, 4), A3, W)
    ic.stroke('M22 14V36M42 14V36', A3, WG, dash='2.5 2.5')
    ic.stroke('M22 39C28 35 36 35 42 39', A3, WA)
    return ic


@icon('model', 'Spun Profile')
def spun():
    ic = new('spun', 'Spun Profile')
    ic.stroke('M32 6V58', LINE, WG, dash=DASH)
    ic.stroke('M32 12H44L48 30L42 52H32', A3, WA)
    ic.stroke('M32 12H20L16 30L22 52H32', LINE, W, dash='4 3.5')
    return ic


@icon('model', 'Break')
def break_():
    ic = new('break', 'Break')
    ic.stroke('M8 44L28 32M36 28L56 16', A3, WA)
    point(ic, 32, 30, 4.6)
    return ic


# ---------------------------------------------------------------- assemblies

@icon('model', 'Component')
def component():
    ic = new('component', 'Component')
    d = frame(0.9, dy=2, height=16)
    block(ic, d, -12, -12, 0, 12, 12, 16, facet='top')
    ic.stroke(rrect(4, 4, 60, 60, 6), LINE, WG, dash=DASH)
    return ic


@icon('model', 'Joint')
def joint():
    ic = new('joint', 'Joint')
    a = frame(0.62, dx=-11, dy=10, height=10)
    b = frame(0.62, dx=11, dy=-6, height=10)
    block(ic, a, -12, -12, 0, 12, 12, 10)
    block(ic, b, -12, -12, 0, 12, 12, 10, facet='top')
    ic.stroke(ell(32, 32, 7), A3, WA, fill=GAP)
    ic.dot(32, 32, 2.6, A3)
    return ic


@icon('model', 'Joint Origin')
def joint_origin():
    ic = new('joint_origin', 'Joint Origin')
    d = frame(0.9, dy=6, height=10)
    block(ic, d, -12, -12, 0, 12, 12, 10)
    c = d.p(0, 0, 10)
    for q in ((10, 0, 10), (0, 10, 10)):
        ic.stroke(poly([c, d.p(*q)], False), A3, WA)
    ic.stroke(poly([c, (c[0], c[1] - 22)], False), A3, WA)
    ic.stroke(ell(c[0], c[1], 5), A3, W, fill=GAP)
    return ic


@icon('model', 'As-Built Joint')
def as_built():
    ic = new('as_built', 'As-Built Joint')
    a = frame(0.62, dx=-9, dy=8, height=10)
    b = frame(0.62, dx=9, dy=-4, height=10)
    block(ic, a, -12, -12, 0, 12, 12, 10)
    block(ic, b, -12, -12, 0, 12, 12, 10)
    ic.stroke(ell(32, 32, 7), A3, WA, fill=GAP)
    ic.stroke('M29 32L31.5 34.5L36 29.5', A3, 2.6)
    return ic


@icon('model', 'Rigid Group')
def rigid_group():
    ic = new('rigid_group', 'Rigid Group')
    a = frame(0.6, dx=-10, dy=6, height=10)
    b = frame(0.6, dx=10, dy=-2, height=10)
    block(ic, a, -12, -12, 0, 12, 12, 10)
    block(ic, b, -12, -12, 0, 12, 12, 10)
    ic.stroke('M20 30H44M20 36H44', A3, WA)
    return ic


@icon('model', 'Motion Link')
def motion_link():
    ic = new('motion_link', 'Motion Link')
    ic.stroke(ell(18, 40, 10), LINE, WA)
    ic.stroke(ell(46, 24, 10), LINE, WA)
    ic.stroke('M25 33L39 31', A3, WA)
    arc_arrow(ic, 46, 24, 15, 15, 120, 200, head=6, w=3)
    return ic


@icon('model', 'Contact Sets')
def contact_sets():
    ic = new('contact_sets', 'Contact Sets')
    ic.stroke(rrect(8, 10, 30, 54, 3), LINE, WA)
    ic.stroke(rrect(34, 10, 56, 54, 3), LINE, WA)
    ic.stroke('M32 14V50', A3, WA)
    return ic


@icon('model', 'New Contact Set')
def contact_new():
    ic = contact_sets()
    ic.name, ic.title = 'contact_new', 'New Contact Set'
    badge(ic, 48, 48, 'plus')
    return ic


@icon('model', 'Enable All Contact')
def contact_all():
    ic = contact_sets()
    ic.name, ic.title = 'contact_all', 'Enable All Contact'
    badge(ic, 48, 48, 'check')
    return ic


@icon('model', 'Disable Contact')
def contact_off():
    ic = new('contact_off', 'Disable Contact')
    ic.stroke(rrect(8, 10, 30, 54, 3), LINE, WA)
    ic.stroke(rrect(34, 10, 56, 54, 3), LINE, WA)
    ic.stroke('M32 14V50', LINE, WG, dash=DASH)
    badge(ic, 48, 48, 'cross')
    return ic


@icon('model', 'Drive')
def drive():
    ic = new('drive', 'Drive')
    ic.stroke(ell(32, 32, 10), LINE, WA, fill=A2)
    arc_arrow(ic, 32, 32, 21, 21, 150, 390, head=9)
    return ic


@icon('model', 'Explode')
def explode():
    ic = new('explode', 'Explode')
    ic.fill(rrect(24, 24, 40, 40, 2), A2)
    ic.stroke(rrect(24, 24, 40, 40, 2), LINE, W)
    for (x, y) in ((10, 10), (54, 10), (10, 54), (54, 54)):
        arrow(ic, (32 + (x - 32) * 0.45, 32 + (y - 32) * 0.45), (x, y), head=7, w=3.4)
    return ic


@icon('model', 'Capture Position')
def capture():
    ic = new('capture', 'Capture Position')
    for sx, sy in ((1, 1), (-1, 1), (1, -1), (-1, -1)):
        ic.stroke('M%s %sV%sH%s' % (32 - sx * 22, 32 - sy * 12, 32 - sy * 22, 32 - sx * 12), LINE, WA)
    point(ic, 32, 32, 7)
    return ic


@icon('model', 'Revert Position')
def revert():
    ic = new('revert', 'Revert Position')
    ic.stroke(rrect(8, 26, 26, 44, 2), LINE, WG, dash=DASH)
    ic.fill(rrect(38, 14, 56, 32, 2), A1); ic.stroke(rrect(38, 14, 56, 32, 2), LINE, W)
    arrow(ic, (40, 34), (24, 42), head=7, w=3.4)
    return ic


@icon('model', 'Motion Trace')
def trace():
    ic = new('trace', 'Motion Trace')
    ic.stroke('M8 50C16 20 30 20 36 34S52 46 56 14', A3, WA, dash='5 4')
    ic.dot(56, 14, 6, A2)
    ic.stroke(ell(56, 14, 6), LINE, W)
    return ic


@icon('model', 'Reach Point')
def reach():
    ic = new('reach', 'Reach Point')
    ic.stroke('M10 52L24 30L40 34', LINE, WA)
    for p in ((10, 52), (24, 30)):
        ic.dot(p[0], p[1], 4, LINE)
    ic.stroke(ell(48, 18, 8), A3, WA)
    ic.dot(48, 18, 2.6, A3)
    ic.stroke('M40 34L45 23', LINE, WG, dash='2.5 2.5')
    return ic


def gear_path(cx, cy, r, teeth, depth=4):
    pts = []
    for k in range(teeth * 4):
        a = 2 * math.pi * k / (teeth * 4)
        rr = r if (k % 4) in (0, 1) else r - depth
        pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
    return poly(pts)


@icon('model', 'Gear Coupling')
def gear():
    ic = new('gear', 'Gear Coupling')
    ic.fill(gear_path(22, 36, 15, 8), A2)
    ic.stroke(gear_path(22, 36, 15, 8), LINE, W)
    ic.stroke(gear_path(46, 20, 11, 6, 3.5), LINE, W)
    ic.dot(22, 36, 3.4, GAP); ic.dot(46, 20, 2.8, LINE)
    return ic


@icon('model', 'Rack and Pinion')
def rack_pinion():
    ic = new('rack_pinion', 'Rack and Pinion')
    ic.fill(gear_path(32, 24, 15, 8), A2)
    ic.stroke(gear_path(32, 24, 15, 8), LINE, W)
    ic.stroke('M6 52H58M6 44H58', LINE, W)
    for x in range(10, 58, 8):
        ic.stroke('M%s 44V40H%sV44' % (x, x + 4), LINE, W)
    return ic


@icon('model', 'Screw Coupling')
def screw():
    ic = new('screw', 'Screw Coupling')
    ic.stroke('M24 6V58M40 6V58', LINE, W)
    for k in range(6):
        y = 10 + k * 8
        ic.stroke('M20 %sL44 %s' % (y + 4, y), LINE, W)
    ic.fill(rrect(14, 26, 50, 38, 2), A2)
    ic.stroke(rrect(14, 26, 50, 38, 2), LINE, W)
    return ic


@icon('model', 'Cam Coupling')
def cam():
    ic = new('cam', 'Cam Coupling')
    ic.fill('M20 40A14 14 0 1 1 44 40Q38 58 32 58Q26 58 20 40Z', A2)
    ic.stroke('M20 40A14 14 0 1 1 44 40Q38 58 32 58Q26 58 20 40Z', LINE, W)
    ic.dot(32, 36, 3.2, GAP)
    ic.stroke(rrect(26, 4, 38, 18, 2), LINE, W)
    return ic


@icon('model', 'Chain')
def chain():
    ic = new('chain', 'Chain')
    ic.stroke('M14 28H26A8 8 0 0 1 26 44H14A8 8 0 0 1 14 28Z', LINE, WA)
    ic.stroke('M38 20H50A8 8 0 0 1 50 36H38A8 8 0 0 1 38 20Z', A3, WA)
    ic.stroke('M24 36H40', LINE, WA)
    return ic


@icon('model', 'Fastener')
def fastener():
    ic = new('fastener', 'Fastener')
    hx = ngon(32, 16, 14, 6, 0)
    ic.fill(poly(hx), A2)
    ic.stroke(poly(hx), LINE, W)
    ic.stroke('M26 26V56H38V26', LINE, W)
    for y in (32, 40, 48):
        ic.stroke('M24 %sL40 %s' % (y + 3, y), LINE, 2.4)
    return ic


# ---------------------------------------------------------------- patterns

@icon('model', 'Rectangular Pattern')
def pattern_rect():
    ic = new('pattern_rect', 'Rectangular Pattern')
    for i, (x, y) in enumerate(((10, 10), (36, 10), (10, 36), (36, 36))):
        q = rrect(x, y, x + 18, y + 18, 3)
        if i == 0:
            ic.fill(q, A2)
        ic.stroke(q, LINE if i else A3, WA if i == 0 else W)
    return ic


@icon('model', 'Circular Pattern')
def pattern_circ():
    ic = new('pattern_circ', 'Circular Pattern')
    ic.stroke(ell(32, 32, 19), LINE, WG, dash=DASH)
    for k in range(6):
        a = math.radians(-90 + 60 * k)
        x, y = 32 + 19 * math.cos(a), 32 + 19 * math.sin(a)
        ic.dot(x, y, 6.5, GAP)
        ic.dot(x, y, 5.5, A2 if k == 0 else LINE)
    return ic


@icon('model', 'Path Pattern')
def pattern_path():
    ic = new('pattern_path', 'Path Pattern')
    ic.stroke('M6 50C18 50 22 18 34 18S50 40 58 30', LINE, WG, dash=DASH)
    for i, (x, y) in enumerate(((8, 50), (22, 36), (34, 18), (48, 32))):
        ic.fill(rrect(x - 5, y - 5, x + 5, y + 5, 2), A2 if i == 0 else GAP)
        ic.stroke(rrect(x - 5, y - 5, x + 5, y + 5, 2), A3 if i == 0 else LINE, W)
    return ic


@icon('model', 'Centre Pattern')
def pattern_center():
    ic = new('pattern_center', 'Centre Pattern')
    ic.fill(rrect(24, 24, 40, 40, 3), A2)
    ic.stroke(rrect(24, 24, 40, 40, 3), A3, WA)
    for x, y in ((6, 24), (42, 24)):
        ic.stroke(rrect(x, y, x + 16, y + 16, 3), LINE, W)
    return ic


@icon('model', 'Reverse Pattern Direction')
def pattern_flip():
    ic = new('pattern_flip', 'Reverse Pattern Direction')
    for x in (8, 26, 44):
        ic.stroke(rrect(x, 14, x + 12, 26, 2), LINE, W)
    arrow(ic, (48, 44), (16, 44), head=8)
    return ic


# ---------------------------------------------------------------- bodies, curves, misc

@icon('model', 'Body')
def body():
    ic = new('body', 'Body')
    d = frame(1.0, dy=0, height=18)
    block(ic, d, -12, -12, 0, 12, 12, 18, facet=['top', 'left'], colour=A1)
    return ic


@icon('model', 'Bounding Solid')
def bounding_solid():
    ic = new('bounding_solid', 'Bounding Solid')
    d = frame(1.0, dy=0, height=18)
    ic.stroke(d.poly((-12, -12, 18), (12, -12, 18), (12, -12, 0), (12, 12, 0), (-12, 12, 0), (-12, 12, 18)), A3, W, dash=DASH)
    ic.stroke(poly([d.p(-12, 12, 18), d.p(12, 12, 18), d.p(12, -12, 18)], False), A3, W, dash=DASH)
    ic.stroke(poly([d.p(12, 12, 18), d.p(12, 12, 0)], False), A3, W, dash=DASH)
    cylinder(ic, d, 0, 0, 2, 14, 7, facet='top', colour=A1)
    return ic


@icon('model', 'Boundary Fill')
def boundary_fill():
    ic = new('boundary_fill', 'Boundary Fill')
    ic.stroke(ell(24, 32, 15), LINE, W)
    ic.stroke(ell(40, 32, 15), LINE, W)
    ic.clip_begin('bfill', ell(24, 32, 15))
    ic.fill(ell(40, 32, 15), GAP)
    ic.clip_end()
    ic.fill('M32 18.6A15 15 0 0 0 32 45.4A15 15 0 0 1 9 32A15 15 0 0 1 32 18.6Z', A2)
    ic.stroke(ell(24, 32, 15), LINE, W)
    ic.stroke(ell(40, 32, 15), LINE, W)
    return ic


@icon('model', 'Blend Curve')
def blend():
    ic = new('blend', 'Blend Curve')
    ic.stroke('M6 46H22M42 18H58', LINE, WA)
    ic.stroke('M22 46C34 46 30 18 42 18', A3, WA)
    point(ic, 22, 46, 4); point(ic, 42, 18, 4)
    return ic


@icon('model', 'Favorite')
def star():
    ic = new('star', 'Favorite')
    pts = []
    for k in range(10):
        r = 22 if k % 2 == 0 else 9.5
        a = math.radians(-90 + 36 * k)
        pts.append((32 + r * math.cos(a), 34 + r * math.sin(a)))
    ic.fill(poly(pts), A2)
    ic.stroke(poly(pts), LINE, W)
    return ic


@icon('model', 'Assistant')
def agent_chat():
    ic = new('agent_chat', 'Assistant')
    ic.stroke('M15 13H49A5 5 0 0 1 54 18V40A5 5 0 0 1 49 45H30L19 54V45H15A5 5 0 0 1 10 40V18A5 5 0 0 1 15 13Z', LINE, WA)
    t = [(32, 19), (41, 35), (23, 35)]
    ic.fill(poly(t), A2)
    ic.stroke(poly(t), A3, 2.4)
    return ic


# ---------------------------------------------------------------- sheet metal and plastic parts
# A sheet is a thin plate; the bend or flange the command makes is the facet.

@icon('model', 'Sheet Metal')
def sheet():
    ic = new('sheet', 'Sheet Metal')
    d = frame(1.05, dy=4, height=2)
    block(ic, d, -14, -14, 0, 14, 14, 2.4, facet='top', colour=A1)
    return ic


@icon('model', 'Flange')
def flange():
    ic = new('flange', 'Flange')
    d = frame(1.0, dy=6, height=18)
    block(ic, d, -14, -14, 0, 14, 10, 2.4)
    block(ic, d, -14, 10, 0, 14, 12.4, 18, facet='left')
    return ic


@icon('model', 'Hem')
def hem():
    ic = new('hem', 'Hem')
    ic.stroke('M6 42H44A6 6 0 0 0 44 30H24', LINE, WA)
    ic.stroke('M44 42A6 6 0 0 0 44 30', A3, WA + 2)
    return ic


@icon('model', 'Fold')
def fold():
    ic = new('fold', 'Fold')
    ic.stroke('M6 46H30', LINE, WA)
    ic.stroke('M30 46L48 20', A3, WA)
    ic.stroke('M30 46H54', LINE, WG, dash=DASH)
    arc_arrow(ic, 30, 46, 16, 16, -5, -50, head=6, w=3)
    return ic


@icon('model', 'Unfold')
def unfold():
    ic = new('unfold', 'Unfold')
    ic.stroke('M6 46H30', LINE, WA)
    ic.stroke('M30 46L48 20', LINE, WG, dash=DASH)
    ic.stroke('M30 46H56', A3, WA)
    arc_arrow(ic, 30, 46, 18, 18, -52, -10, head=6, w=3)
    return ic


@icon('model', 'Refold')
def refold():
    ic = new('refold', 'Refold')
    ic.stroke('M6 46H30', LINE, WA)
    ic.stroke('M30 46L48 20', A3, WA)
    arc_arrow(ic, 30, 46, 18, 18, -10, -52, head=6, w=3)
    arc_arrow(ic, 30, 46, 24, 24, -58, -14, head=6, w=3)
    return ic


@icon('model', 'Flat Pattern')
def flat_():
    ic = new('flat', 'Flat Pattern')
    q = 'M20 8H44V20H56V44H44V56H20V44H8V20H20Z'
    ic.fill('M20 20H44V44H20Z', A1)
    ic.stroke(q, LINE, WA)
    ic.stroke('M20 20H44V44H20Z', A3, WG, dash=DASH)
    return ic


@icon('model', 'Bend Table')
def bendtable():
    ic = new('bendtable', 'Bend Table')
    ic.fill('M10 12H54V22H10Z', A2)
    ic.stroke(rrect(10, 12, 54, 52, 3), LINE, WA)
    ic.stroke('M10 22H54M10 32H54M10 42H54M30 12V52', LINE, W)
    return ic


@icon('model', 'Sheet Metal Rules')
def sheet_rules():
    ic = new('sheet_rules', 'Sheet Metal Rules')
    ic.stroke('M6 50H28L44 28', LINE, WA)
    ic.stroke('M28 50A10 10 0 0 0 34 42', A3, WA)
    ic.stroke('M38 50H58M48 44V56', LINE, W)
    return ic


@icon('model', 'Convert to Sheet')
def convert_sheet():
    ic = new('convert_sheet', 'Convert to Sheet')
    d = frame(0.7, dx=-12, dy=8, height=14)
    block(ic, d, -12, -12, 0, 12, 12, 14)
    e = frame(0.7, dx=12, dy=-6, height=2)
    block(ic, e, -12, -12, 0, 12, 12, 2.4, facet='top')
    arrow(ic, (30, 34), (40, 28), head=6, w=3.4)
    return ic


@icon('model', 'Perforate')
def perforate():
    ic = new('perforate', 'Perforate')
    ic.stroke(rrect(8, 8, 56, 56, 4), LINE, WA)
    for x in (20, 32, 44):
        for y in (20, 32, 44):
            ic.dot(x, y, 4, A2)
    return ic


@icon('model', 'Lip')
def lip():
    ic = new('lip', 'Lip')
    ic.stroke('M10 54V20H24V54', LINE, WA)
    ic.fill('M24 20H30V12H24Z', A2)
    ic.stroke('M24 20H30V12H18V20', A3, W)
    ic.stroke('M40 54V12H54V54', LINE, WA)
    return ic


@icon('model', 'Snap Fit')
def snap_fit():
    ic = new('snap_fit', 'Snap Fit')
    ic.stroke('M10 56V12', LINE, WA)
    ic.stroke('M10 12H26V40L36 48H26V56', LINE, WA)
    ic.fill('M26 40L36 48H26Z', A2)
    ic.stroke('M44 30H56M44 30V56', LINE, W)
    return ic


@icon('model', 'Plastic Assign')
def plastic_assign():
    ic = new('plastic_assign', 'Plastic Assign')
    ic.fill('M32 8C32 8 14 30 14 40A18 18 0 0 0 50 40C50 30 32 8 32 8Z', A2)
    ic.stroke('M32 8C32 8 14 30 14 40A18 18 0 0 0 50 40C50 30 32 8 32 8Z', LINE, WA)
    return ic


@icon('model', 'Plastic Rule')
def plastic_rule():
    ic = new('plastic_rule', 'Plastic Rule')
    ic.fill('M24 8C24 8 10 26 10 34A14 14 0 0 0 38 34C38 26 24 8 24 8Z', A1)
    ic.stroke('M24 8C24 8 10 26 10 34A14 14 0 0 0 38 34C38 26 24 8 24 8Z', LINE, W)
    ic.stroke('M38 50H58M48 44V56', A3, WA)
    return ic


# ---------------------------------------------------------------- appearance and render

@icon('model', 'Material')
def material():
    ic = new('material', 'Material')
    ic.stroke(ell(32, 32, 21), LINE, WA)
    ic.fill('M32 11A21 21 0 0 1 32 53Z', A2)
    ic.dot(24, 24, 3.6, LINE)
    return ic


@icon('model', 'Appearance')
def appearance():
    ic = new('appearance', 'Appearance')
    ic.fill(ell(32, 32, 21), A1)
    ic.stroke(ell(32, 32, 21), LINE, WA)
    ic.fill(ell(25, 25, 6), GAP)
    return ic


@icon('model', 'Decal')
def decal():
    ic = new('decal', 'Decal')
    ic.stroke(rrect(8, 8, 56, 56, 4), LINE, WA)
    ic.fill('M16 46L28 30L36 40L42 34L50 46Z', A2)
    ic.dot(42, 20, 4, A3)
    return ic


@icon('model', 'Environment Map')
def environment_map():
    ic = new('environment_map', 'Environment Map')
    ic.stroke(ell(32, 32, 22), LINE, WA)
    ic.fill('M10 32A22 22 0 0 1 54 32Z', A1)
    ic.stroke('M10 32H54', LINE, W)
    ic.dot(40, 22, 4, A3)
    return ic


@icon('model', 'Camera')
def camera():
    ic = new('camera', 'Camera')
    ic.stroke('M8 20H20L24 14H40L44 20H56V50H8Z', LINE, WA)
    ic.fill(ell(32, 34, 10), A2)
    ic.stroke(ell(32, 34, 10), LINE, W)
    return ic


@icon('model', 'Canvas')
def canvas():
    ic = new('canvas', 'Canvas')
    d = frame(1.05, dy=0, height=0)
    q = d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0))
    ic.stroke(q, LINE, W)
    ic.fill(d.poly((-9, -9, 0), (9, -9, 0), (9, 9, 0), (-9, 9, 0)), A1)
    ic.stroke(d.poly((-9, -9, 0), (9, -9, 0), (9, 9, 0), (-9, 9, 0)), A3, W)
    return ic


@icon('model', 'Render Image')
def render_image():
    ic = new('render_image', 'Render Image')
    ic.stroke(rrect(8, 12, 56, 52, 4), LINE, WA)
    ic.fill(ell(32, 34, 11), A2)
    ic.fill(ell(28, 30, 4), GAP)
    return ic


@icon('model', 'Render Scene')
def render_scene():
    ic = new('render_scene', 'Render Scene')
    ic.fill('M6 50L32 40L58 50L32 60Z', A1)
    ic.stroke('M6 50L32 40L58 50L32 60Z', LINE, W)
    ic.fill(ell(32, 30, 10), A2)
    ic.stroke(ell(32, 30, 10), LINE, W)
    ic.stroke('M50 8L44 16M54 18H46', A3, W)
    return ic


# ---------------------------------------------------------------- workspaces

def ws(name, title):
    ic = Icon(name, title)
    ic.fam = 'workspace'
    return ic


@icon('workspace', 'Design')
def ws_design():
    ic = ws('ws_design', 'Design')
    d = frame(1.0, dy=0, height=16)
    block(ic, d, -12, -12, 0, 12, 12, 16, facet='top')
    return ic


@icon('workspace', 'Drawing')
def ws_drawing():
    ic = ws('ws_drawing', 'Drawing')
    ic.stroke(rrect(8, 10, 56, 54, 3), LINE, WA)
    ic.stroke('M18 42V22H38', A3, WA)
    ic.stroke('M18 48H38M44 22V42', LINE, W)
    ic.stroke('M42 54V46H56', LINE, W)
    return ic


@icon('workspace', 'Render')
def ws_render():
    ic = ws('ws_render', 'Render')
    ic.fill(ell(32, 32, 20), A2)
    ic.stroke(ell(32, 32, 20), LINE, WA)
    ic.fill(ell(25, 25, 6), GAP)
    return ic


@icon('workspace', 'Animate')
def ws_animate():
    ic = ws('ws_animate', 'Animate')
    ic.stroke(rrect(22, 14, 54, 46, 3), A3, WA)
    ic.stroke(rrect(14, 20, 46, 52, 3), LINE, W, dash='4 3.5')
    ic.stroke(rrect(8, 26, 40, 58, 3), LINE, WG, dash='2.5 3')
    return ic

# ---------------------------------------------------------------- flat: application bar and navigation

def flat(name, title):
    ic = Icon(name, title)
    ic.fam = 'flat'
    return ic


@icon('flat')
def save():
    ic = flat('save', 'Save')
    ic.stroke('M15 11H42L53 22V49A4 4 0 0 1 49 53H15A4 4 0 0 1 11 49V15A4 4 0 0 1 15 11Z', LINE, WA)
    ic.stroke('M22 11V22H40V11', LINE, WA)
    ic.fill('M21 36H43V53H21Z', A2)
    ic.stroke('M21 53V36H43V53', LINE, WA)
    return ic


@icon('flat')
def undo():
    ic = flat('undo', 'Undo')
    ic.stroke('M22 25H39A12 12 0 0 1 39 49H24', LINE, WA)
    ic.fill(poly([(9, 25), (22, 14), (22, 36)]), LINE)
    return ic


@icon('flat')
def redo():
    ic = flat('redo', 'Redo')
    ic.stroke('M42 25H25A12 12 0 0 0 25 49H40', LINE, WA)
    ic.fill(poly([(55, 25), (42, 14), (42, 36)]), LINE)
    return ic


@icon('flat')
def home():
    ic = flat('home', 'Home')
    ic.stroke('M10 30L32 11L54 30', LINE, WA)
    ic.stroke('M16 25V51H48V25', LINE, WA)
    ic.fill('M27 51V37H37V51Z', A2)
    return ic


@icon('flat')
def folder():
    ic = flat('folder', 'Folder')
    ic.stroke('M9 18A4 4 0 0 1 13 14H25L31 20H51A4 4 0 0 1 55 24V46A4 4 0 0 1 51 50H13A4 4 0 0 1 9 46Z', LINE, WA)
    ic.fill('M9 28H55V46A4 4 0 0 1 51 50H13A4 4 0 0 1 9 46Z', A1)
    ic.stroke('M9 28H55', LINE, W)
    return ic


@icon('flat')
def plus():
    ic = flat('plus', 'New')
    ic.stroke('M32 12V52M12 32H52', LINE, WA + 0.8)
    return ic


@icon('flat')
def close():
    ic = flat('close', 'Close')
    ic.stroke('M17 17L47 47M47 17L17 47', LINE, WA + 0.6)
    return ic


@icon('flat')
def chevron():
    ic = flat('chevron', 'More')
    ic.stroke('M18 25L32 39L46 25', LINE, WA + 0.6)
    return ic


@icon('flat')
def chevron_right():
    ic = flat('chevron_right', 'Next')
    ic.stroke('M25 18L39 32L25 46', LINE, WA + 0.6)
    return ic


@icon('flat')
def disclosure_closed():
    ic = flat('disclosure_closed', 'Expand')
    ic.fill(poly([(24, 16), (44, 32), (24, 48)]), LINE)
    return ic


@icon('flat')
def disclosure_open():
    ic = flat('disclosure_open', 'Collapse')
    ic.fill(poly([(16, 24), (48, 24), (32, 44)]), LINE)
    return ic


@icon('flat')
def settings():
    ic = flat('settings', 'Settings')
    # A hexagonal nut seen face on: Polykrom's settings mark is a part, not a gear.
    hexa = [(32 + 20 * math.cos(math.radians(30 + 60 * k)), 32 + 20 * math.sin(math.radians(30 + 60 * k))) for k in range(6)]
    ic.stroke(poly(hexa), LINE, WA)
    ic.fill(ell(32, 32, 8.5), A2)
    ic.stroke(ell(32, 32, 8.5), LINE, WA)
    return ic


@icon('flat')
def help():
    ic = flat('help', 'Help')
    ic.stroke(ell(32, 32, 21), LINE, WA)
    ic.stroke('M24.5 26A7.5 7.5 0 1 1 35 32.8C33 33.8 32 35 32 37.5V39', LINE, WA)
    ic.dot(32, 46.5, 3.1, LINE)
    return ic


@icon('flat')
def orbit():
    ic = flat('orbit', 'Orbit')
    ic.stroke(ell(32, 32, 7.5), LINE, WA, fill=A2)
    arc_arrow(ic, 32, 32, 22, 11, 20, 320, colour=LINE, head=8)
    return ic


@icon('flat')
def pan():
    ic = flat('pan', 'Pan')
    for (x, y) in ((32, 9), (55, 32), (32, 55), (9, 32)):
        arrow(ic, (32 + (x - 32) * 0.25, 32 + (y - 32) * 0.25), (x, y), colour=LINE, head=8.5)
    ic.dot(32, 32, 4.5, A2)
    return ic


@icon('flat')
def zoom():
    ic = flat('zoom', 'Zoom')
    ic.stroke(ell(27, 27, 15), LINE, WA, fill=A1)
    ic.stroke('M38 38L53 53', LINE, WA + 1.6)
    ic.stroke('M20 27H34M27 20V34', LINE, WA)
    return ic


@icon('flat')
def fit():
    ic = flat('fit', 'Fit All')
    for sx, sy in ((1, 1), (-1, 1), (1, -1), (-1, -1)):
        ic.stroke('M%s %sV%sH%s' % (32 - sx * 21, 32 - sy * 10, 32 - sy * 21, 32 - sx * 10), LINE, WA)
    ic.fill(rrect(23, 23, 41, 41, 2), A2)
    return ic


@icon('flat')
def eye():
    ic = flat('eye', 'Show')
    ic.stroke('M8 32C14 20 22 15 32 15C42 15 50 20 56 32C50 44 42 49 32 49C22 49 14 44 8 32Z', LINE, WA)
    ic.fill(ell(32, 32, 8), A2)
    ic.stroke(ell(32, 32, 8), LINE, W)
    return ic


@icon('flat')
def lookat():
    ic = flat('lookat', 'Look At')
    d = Dim(1.0, 32, 36)
    ic.stroke(d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0)), LINE, W)
    ic.fill(d.poly((-14, -14, 0), (14, -14, 0), (14, 14, 0), (-14, 14, 0)), A1)
    arrow(ic, (32, 6), (32, 31), colour=A3)
    return ic


@icon('flat')
def perspective():
    ic = flat('perspective', 'Perspective')
    ic.stroke('M10 14L54 22V42L10 50Z', LINE, WA)
    ic.fill('M10 14L54 22V42L10 50Z', A1)
    ic.stroke('M10 32H54', LINE, WG, dash=DASH)
    return ic


@icon('flat')
def spin():
    ic = flat('spin', 'Spin')
    arc_arrow(ic, 32, 32, 19, 19, -60, 230, colour=LINE, head=9)
    ic.dot(32, 32, 4.5, A2)
    return ic


@icon('flat')
def import_tray():
    ic = flat('import_tray', 'Import')
    ic.stroke('M10 38V50A3 3 0 0 0 13 53H51A3 3 0 0 0 54 50V38', LINE, WA)
    arrow(ic, (32, 8), (32, 41), colour=A3)
    return ic


@icon('flat')
def export_tray():
    ic = flat('export_tray', 'Export')
    ic.stroke('M10 38V50A3 3 0 0 0 13 53H51A3 3 0 0 0 54 50V38', LINE, WA)
    arrow(ic, (32, 43), (32, 8), colour=A3)
    return ic


@icon('flat', 'Assistant')
def assistant():
    ic = flat('assistant', 'Assistant')
    ic.stroke('M15 13H49A5 5 0 0 1 54 18V40A5 5 0 0 1 49 45H30L19 54V45H15A5 5 0 0 1 10 40V18A5 5 0 0 1 15 13Z', LINE, WA)
    ic.fill(poly([(32, 19), (41, 35), (23, 35)]), A2)
    return ic


@icon('flat')
def send():
    ic = flat('send', 'Send')
    ic.fill(poly([(10, 12), (56, 32), (10, 52), (16, 32)]), A2)
    ic.stroke(poly([(10, 12), (56, 32), (10, 52), (16, 32)]), LINE, W)
    return ic


@icon('flat')
def stop():
    ic = flat('stop', 'Stop')
    ic.fill(rrect(16, 16, 48, 48, 5), A2)
    ic.stroke(rrect(16, 16, 48, 48, 5), LINE, W)
    return ic


@icon('flat', 'Report a Bug')
def bug_report():
    ic = flat('bug_report', 'Report a Bug')
    ic.stroke('M20 30A12 12 0 0 1 44 30V40A12 12 0 0 1 20 40Z', LINE, WA)
    ic.fill('M20 30A12 12 0 0 1 44 30Z', A2)
    ic.stroke('M10 26L20 31M54 26L44 31M9 42H20M55 42H44M12 55L21 47M52 55L43 47M26 16L23 10M38 16L41 10', LINE, W)
    return ic


@icon('flat')
def moon():
    ic = flat('moon', 'Dark Theme')
    ic.fill('M40 10A23 23 0 1 0 54 42A19 19 0 0 1 40 10Z', A1)
    ic.stroke('M40 10A23 23 0 1 0 54 42A19 19 0 0 1 40 10Z', LINE, WA)
    return ic



@icon('flat', 'Connected Agent')
def agent_link():
    ic = flat('agent_link', 'Connected Agent')
    ic.stroke('M26 22L22 18A9 9 0 0 0 9 31L15 37A9 9 0 0 0 28 37', LINE, WA)
    ic.stroke('M38 42L42 46A9 9 0 0 0 55 33L49 27A9 9 0 0 0 36 27', LINE, WA)
    ic.fill(poly([(32, 24), (38, 35), (26, 35)]), A2)
    return ic


@icon('flat', 'Navigation Settings')
def nav_settings():
    ic = flat('nav_settings', 'Navigation Settings')
    ic.stroke(ell(26, 30, 7), LINE, WA, fill=A2)
    arc_arrow(ic, 26, 30, 18, 9, 30, 300, colour=LINE, head=7, w=3.8)
    hexa = [(48 + 9 * math.cos(math.radians(30 + 60 * k)), 48 + 9 * math.sin(math.radians(30 + 60 * k))) for k in range(6)]
    ic.fill(poly(hexa), GAP)
    ic.stroke(poly(hexa), LINE, W)
    return ic


@icon('flat', 'Approval Required')
def shield():
    ic = flat('shield', 'Approval Required')
    q = 'M32 6L52 14V30C52 44 42 52 32 58C22 52 12 44 12 30V14Z'
    ic.fill(q, A1)
    ic.stroke(q, LINE, WA)
    ic.stroke('M23 32L30 39L42 25', A3, WA)
    return ic


@icon('flat', 'Style')
def style():
    ic = flat('style', 'Style')
    ic.stroke('M10 54L14 42L42 14L50 22L22 50Z', LINE, WA)
    ic.fill(ell(46, 46, 9), A2)
    ic.stroke(ell(46, 46, 9), LINE, W)
    return ic


@icon('flat', 'Suppress')
def suppress():
    ic = flat('suppress', 'Suppress')
    ic.stroke(ell(32, 32, 21), LINE, WA)
    ic.stroke('M17 47L47 17', A3, WA)
    return ic


@icon('flat', 'Keyboard')
def settings_keyboard():
    ic = flat('settings_keyboard', 'Keyboard')
    ic.stroke(rrect(6, 16, 58, 48, 4), LINE, WA)
    for y in (25, 33):
        for x in range(14, 54, 8):
            ic.dot(x, y, 2.2, LINE)
    ic.stroke('M20 41H44', A3, WA)
    return ic


@icon('flat', 'Display')
def settings_monitor():
    ic = flat('settings_monitor', 'Display')
    ic.stroke(rrect(6, 10, 58, 44, 4), LINE, WA)
    ic.fill(rrect(12, 16, 52, 38, 2), A1)
    ic.stroke('M26 54H38M32 44V54', LINE, WA)
    return ic


@icon('flat', 'Mouse')
def settings_mouse():
    ic = flat('settings_mouse', 'Mouse')
    ic.stroke('M18 24A14 14 0 0 1 46 24V40A14 14 0 0 1 18 40Z', LINE, WA)
    ic.fill('M32 10A14 14 0 0 1 46 24V28H32Z', A2)
    ic.stroke('M32 10V28M18 28H46', LINE, W)
    return ic


@icon('flat', 'Palette')
def settings_palette():
    ic = flat('settings_palette', 'Palette')
    ic.stroke('M32 8A24 24 0 1 0 32 56C36 56 38 52 36 48C34 44 36 40 40 40H48A8 8 0 0 0 56 32A24 24 0 0 0 32 8Z', LINE, WA)
    ic.dot(20, 30, 4, A3); ic.dot(28, 19, 4, A2); ic.dot(41, 19, 4, A1); ic.dot(22, 43, 4, LINE)
    return ic


@icon('flat', 'Plug-ins')
def settings_plug():
    ic = flat('settings_plug', 'Plug-ins')
    ic.stroke('M24 8V20M40 8V20', LINE, WA)
    ic.fill('M16 20H48V30A16 16 0 0 1 16 30Z', A2)
    ic.stroke('M16 20H48V30A16 16 0 0 1 16 30Z', LINE, WA)
    ic.stroke('M32 46V58', LINE, WA)
    return ic


if __name__ == '__main__':
    mode, rest = sys.argv[1], sys.argv[2:]
    root = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), 'svg')
    out = rest.pop(0) if mode == 'preview' else None
    names = rest or sorted(ICONS)
    for name in names:
        family, _, fn = ICONS[name]
        ic = fn()
        base = name.split('/')[-1]
        target = os.path.join(out, name.replace('/', '_') + '.svg') if out else os.path.join(root, family, base + '.svg')
        open(target, 'w').write(svg(ic))
    print(len(names), 'icons')

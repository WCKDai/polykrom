"""Facet's standard bases. Read assets/icons/ui/STYLE_GUIDE.md before drawing an icon.

The functions in facet_icons.py are the masters' source; these give a new drawing the standard
part or plate to start from, in the same frame as every other feature icon.
"""
from lib import Icon
from facet import frame, block, svg


def part(icon, facet=None):
    """The standard part: a 24 x 24 x 14 block in 2:1 dimetric. Returns its projection."""
    d = frame(1.0, height=14)
    block(icon, d, -12, -12, 0, 12, 12, 14, facet=facet)
    return d


def plate(icon, facet=None):
    """A thin plate, for sheet metal and faces: 28 x 28 x 3."""
    d = frame(1.05, height=3)
    block(icon, d, -14, -14, 0, 14, 14, 3, facet=facet)
    return d


def examples():
    a = Icon('part', 'Standard part'); part(a, 'top')
    b = Icon('plate', 'Standard plate'); plate(b, 'top')
    return {'Standard part': svg(a), 'Standard plate': svg(b)}

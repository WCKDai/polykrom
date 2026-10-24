"""Move a SolidCraft-derived palette onto Polykrom's (assets/styles/ui/README: "Polykrom palette").

Run once per palette file; it keeps every value's perceptual lightness (OKLab L), so the
contrast relationships the colour-vision tests measure hold, and changes what makes the look:
 - greys lose their navy-slate cast and become graphite (a faint violet bias, hue 285);
 - the brand blue becomes Polykrom cyan (hue 205);
 - other hues are semantic and stay until they are retuned with the colour-vision tests.
Usage: python3 krom_palette.py FILE...   (rewrites Color32::from_rgb(..) and "#rrggbb" in place)
"""
import math, re, sys

def to_lin(c):
    c /= 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

def to_srgb(x):
    x = max(0.0, min(1.0, x))
    return round((12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055) * 255)

def oklch(rgb):
    r, g, b = (to_lin(v) for v in rgb)
    l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
    m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
    s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
    L = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
    A = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
    B = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
    return L, math.hypot(A, B), math.degrees(math.atan2(B, A)) % 360

def rgb_of(L, C, h):
    def raw(C):
        a, b = C * math.cos(math.radians(h)), C * math.sin(math.radians(h))
        l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
        m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
        s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
        return (4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
                -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
                -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s)
    # Reduce chroma until the colour is inside sRGB, so the lightness is kept exactly.
    for _ in range(40):
        v = raw(C)
        if all(-1e-4 <= x <= 1 + 1e-4 for x in v):
            break
        C *= 0.95
    return tuple(to_srgb(x) for x in raw(C))

# (hue range, Polykrom hue, chroma scale). Only the brand blue moves for now: the other hues are
# semantic (sketch, constraint, surface, form, status) and tied to the icon accent ramps in
# palette.toml, and they are retuned there, with the colour-vision tests, as their own step.
SNAP = [((225, 280), 205, 0.92)]   # SolidCraft blue -> Polykrom cyan


def krom(rgb):
    L, C, h = oklch(rgb)
    # Greys with SolidCraft's navy cast lose it; pale tints of other hues (success, error and
    # warning fills) keep theirs, so their meaning survives.
    # Very light blue-cast values with some chroma are tints (hover, selection, tab grounds) and
    # become pale cyan; darker or flatter ones are greys.
    tint = L >= 0.9 and C >= 0.012
    if C < 0.003 or (C < 0.03 and 195 <= h < 290 and not tint):
        return rgb_of(L, min(C, 0.008) if C > 0.003 else 0.0, 285)
    if tint and 195 <= h < 290:
        return rgb_of(L, C, 205)
    for (lo, hi), to, k in SNAP:
        if lo <= h < hi:
            return rgb_of(L, C * k, to)
    return rgb

# Values whose colour is a convention or a status, not SolidCraft's look: kept as they are.
KEEP = re.compile(r'axis|AXIS|triad|gizmo|curvature|interference|draft|access|error|success|warning|valid|sketch_lost|SKETCH_LOST|form_accent')


def migrate_rust(text):
    """palettes.rs: every literal outside the colour-vision tables and the KEEP fields, plus the
    continuity array, which is a convention (bluish green, blue, yellow, reddish purple)."""
    cut = text.find('    pub(super) const fn red_green')
    head, tail = (text, '') if cut < 0 else (text[:cut], text[cut:])
    out, keep_block = [], False
    for line in head.split('\n'):
        if 'continuity: [' in line:
            keep_block = True
        if keep_block or KEEP.search(line):
            out.append(line)
        else:
            out.append(re.sub(r'Color32::from_rgb\((\d+),\s*(\d+),\s*(\d+)\)',
                              lambda m: 'Color32::from_rgb(%d, %d, %d)' % krom(tuple(int(x) for x in m.groups())), line))
        if keep_block and line.strip().startswith('],'):
            keep_block = False
    return '\n'.join(out) + tail


def main(paths):
    for p in paths:
        s = open(p).read()
        s = re.sub(r'Color32::from_rgb\((\d+),\s*(\d+),\s*(\d+)\)',
                   lambda m: 'Color32::from_rgb(%d, %d, %d)' % krom(tuple(int(x) for x in m.groups())), s)
        s = re.sub(r'Color32::from_rgba_unmultiplied\((\d+),\s*(\d+),\s*(\d+),',
                   lambda m: 'Color32::from_rgba_unmultiplied(%d, %d, %d,' % krom(tuple(int(x) for x in m.groups())), s)
        s = re.sub(r'(?<!source = )"#([0-9a-fA-F]{6})"',
                   lambda m: '"#%02x%02x%02x"' % krom(tuple(int(m.group(1)[i:i + 2], 16) for i in (0, 2, 4))), s)
        open(p, 'w').write(s)

if __name__ == '__main__':
    main(sys.argv[1:])

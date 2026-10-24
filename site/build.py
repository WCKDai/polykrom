#!/usr/bin/env python3
"""Assemble cad.polykrom.app into dist/site: the landing page, the browser app and the book.

    python3 site/build.py            after `trunk build --release` (dist/web) and `cargo xtask book`

The page's icons are the Facet masters themselves, recoloured with CSS variables, and its fonts
are the app's own; nothing here is a second copy to keep in step.
"""
import math, re, shutil, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'dist' / 'site'
SVG = ROOT / 'assets' / 'icons' / 'ui' / 'design' / 'svg'
ROLE = {'#b9ddf8': 'var(--a1)', '#328bd3': 'var(--a2)', '#236ead': 'var(--a3)', '#ffffff': 'var(--gp)', '#6f879c': 'var(--muted)'}
STRIP = ['extrude', 'revolve', 'fillet', 'shell', 'hole', 'loft', 'sweep', 'plane_midplane', 'axis_two_planes',
         'rect', 'circle3', 'slot', 'c_parallel', 'c_tangent', 'joint', 'interference', 'section', 'measure']
# The facets of the mark, in the brand's cyan-to-violet range (theme `logo_facets`).
MIDS = ['#2ec4d6', '#5a6ee8', '#8b62ea']
CORNERS = ['#4146c2', '#1a8fab', '#13748c', '#5e36a8', '#7442c4', '#353aa6']


def icon(name, scope):
    path = next(SVG.glob('*/%s.svg' % name))
    s = path.read_text()
    title = re.search(r'<title>(.*?)</title>', s).group(1)
    s = re.sub(r'<title>.*?</title>', '', s).replace('currentColor', 'var(--ln)')
    for src, var in ROLE.items():
        s = re.sub(re.escape(src), var, s, flags=re.I)
    s = re.sub(r'id="([^"]+)"', lambda m: 'id="%s-%s"' % (scope, m.group(1)), s)
    s = re.sub(r'url\(#([^)]+)\)', lambda m: 'url(#%s-%s)' % (scope, m.group(1)), s)
    s = s.replace('<svg ', '<svg aria-hidden="true" ')
    return re.sub(r'\s+', ' ', s).strip(), title


def gem(size=100, seams=True):
    r, c = size / 2, size / 2
    h = [(c + r * math.cos(math.pi / 2 - k * math.pi / 3), c - r * math.sin(math.pi / 2 - k * math.pi / 3)) for k in range(6)]
    t = [(c + r / 2 * math.cos(math.pi / 2 - k * 2 * math.pi / 3), c - r / 2 * math.sin(math.pi / 2 - k * 2 * math.pi / 3)) for k in range(3)]
    pts = lambda ps: ' '.join('%.2f,%.2f' % p for p in ps)
    polys = ['<polygon points="%s" fill="%s"/>' % (pts([h[k], h[(k + 1) % 6], t[n]]), CORNERS[k])
             for k, n in [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2), (5, 0)]]
    polys += ['<polygon points="%s" fill="%s"/>' % (pts(p), f) for p, f in
              (([t[0], t[1], h[1]], MIDS[0]), ([t[1], t[2], h[3]], MIDS[1]), ([t[2], t[0], h[5]], MIDS[2]), (t, '#c9f6fa'))]
    lines = ''
    if seams:
        segs = [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] + [(h[k], t[n]) for k, n in [(1, 0), (1, 1), (3, 1), (3, 2), (5, 2), (5, 0), (0, 0), (2, 1), (4, 2)]]
        lines = '<g stroke="var(--bg)" stroke-width="%.2f" stroke-linecap="round">%s</g>' % (
            size / 60, ''.join('<line x1="%.2f" y1="%.2f" x2="%.2f" y2="%.2f"/>' % (a[0], a[1], b[0], b[1]) for a, b in segs))
    return '<svg viewBox="0 0 %d %d" role="img" aria-label="Polykrom">%s%s</svg>' % (size, size, ''.join(polys), lines)


def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    (OUT / 'fonts').mkdir(parents=True)
    page = (ROOT / 'site' / 'index.html').read_text()
    page = page.replace('{{MARK}}', gem(100, False)).replace('{{GEM}}', gem(100))
    page = re.sub(r'\{\{ICON:(\w+)\}\}', lambda m: icon(m.group(1), 'c-' + m.group(1))[0], page)
    strip = ''.join('<figure>%s<figcaption>%s</figcaption></figure>' % icon(n, 's-' + n) for n in STRIP)
    page = page.replace('{{STRIP}}', strip)
    if '{{' in page:
        sys.exit('unfilled placeholder in site/index.html')
    (OUT / 'index.html').write_text(page)
    shutil.copy2(ROOT / 'site' / '404.html', OUT / '404.html')
    (OUT / 'mark.svg').write_text(gem(100, False).replace('role="img" aria-label="Polykrom"', 'xmlns="http://www.w3.org/2000/svg"'))
    shutil.copytree(ROOT / 'site' / 'screens', OUT / 'screens')
    # The showcases' 3D viewer, shared by their pages.
    shutil.copytree(ROOT / 'site' / 'shared', OUT / 'shared')
    for f in ('atkinson-hyperlegible-next/AtkinsonHyperlegibleNext-Variable.ttf', 'atkinson-hyperlegible-mono/AtkinsonHyperlegibleMono-Regular.ttf'):
        shutil.copy2(ROOT / 'assets' / 'fonts' / f, OUT / 'fonts')
    for src, dst in ((ROOT / 'dist' / 'web', 'app'), (ROOT / 'target' / 'book', 'docs')):
        if src.exists():
            shutil.copytree(src, OUT / dst)
        else:
            print('note: %s is missing, so /%s/ is not in this build' % (src.relative_to(ROOT), dst))
    precompress(OUT / 'app')
    kestrel()
    so101()


# The SO-101 showcase design: LeRobot's arm (TheRobotStudio SO-ARM100, Apache-2.0, commit a758567c)
# as the agent left it. It is 10 MB, so it lives on the media branch, pinned by its hash.
SO101_URL = 'https://raw.githubusercontent.com/wckdboy/polykrom/66e346a6/so101.polykrom'
SO101_SHA256 = 'b8d6e9c87f12d318e46c2128f35b028b0dc978579312d5b977ba8b9a8df79edb'
SO101_LICENSE_URL = 'https://raw.githubusercontent.com/wckdboy/polykrom/66e346a6/so101-SO-ARM100-LICENSE.txt'


def so101():
    """The SO-101 showcase: its page in site/so101; the design fetched (and checked) from the media
    branch; the URDF, a mesh pack for the viewer, the motion data, the GIF and a poster made from it
    by the CLI, so they always match it."""
    import hashlib, json, os, subprocess, tempfile, urllib.request, zipfile
    out = OUT / 'so101'
    shutil.copytree(ROOT / 'site' / 'so101', out)
    cli = Path(os.environ.get('POLYKROM_CLI', ROOT / 'target' / 'release' / 'solidcraft-cli'))
    if not cli.exists():
        print('note: %s is missing, so the SO-101 showcase has no model in this build' % cli)
        return
    cache = ROOT / 'target' / 'showcase-cache'
    cache.mkdir(parents=True, exist_ok=True)
    design = cache / ('so101-%s.polykrom' % SO101_SHA256[:12])
    if not design.exists():
        data = urllib.request.urlopen(SO101_URL, timeout=120).read()
        if hashlib.sha256(data).hexdigest() != SO101_SHA256:
            sys.exit('so101.polykrom from the media branch does not match its pinned hash')
        design.write_bytes(data)
    shutil.copy2(design, out / 'so101.polykrom')
    (out / 'SO-ARM100-LICENSE.txt').write_bytes(urllib.request.urlopen(SO101_LICENSE_URL, timeout=60).read())
    with tempfile.TemporaryDirectory() as tmp:
        urdf = Path(tmp) / 'urdf'
        urdf.mkdir()
        script = {'commands': [
            {'command': 'file.export', 'params': {'path': str(urdf / 'so101.urdf')}},
            {'command': 'motion.plot', 'params': {'study': 'Pick and Place'}},
            {'command': 'motion.export_animation', 'params': {'study': 'Pick and Place', 'path': str(out / 'so101.gif'), 'width': 800, 'height': 500}},
        ]}
        sc, res = Path(tmp) / 'so101.json', Path(tmp) / 'so101.out.json'
        sc.write_text(json.dumps(script))
        subprocess.run([str(cli), 'run', str(sc), '--in', str(design), '--quiet', '--results', '--json-out', str(res)], check=True)
        plot = json.loads(res.read_text())[1]
        (out / 'motion.json').write_text(json.dumps({'steps': plot['steps'], 'joints': [
            {'name': j['name'], 'unit': j['unit'], 'value': [round(v, 4) for v in j['value']]} for j in plot['joints']]}, separators=(',', ':')))
        sys.path.insert(0, str(ROOT / 'site'))
        import meshpack
        (out / 'urdf').mkdir()
        shutil.copy2(urdf / 'so101.urdf', out / 'urdf' / 'so101.urdf')
        meshpack.pack(urdf, out / 'meshes.bin', out / 'meshes.json')
        # Served gzip-compressed by worker.js (Cloudflare does not compress binary files).
        import gzip
        (out / 'meshes.bin.gz').write_bytes(gzip.compress((out / 'meshes.bin').read_bytes(), 9))
        (out / 'meshes.bin').unlink()
        with zipfile.ZipFile(out / 'so101-urdf.zip', 'w', zipfile.ZIP_DEFLATED) as z:
            for f in sorted(urdf.iterdir()):
                z.write(f, 'so101/' + f.name)
    subprocess.run([str(cli), 'snapshot', '--in', str(design), '--out', str(out / 'so101-poster.png'), '--width', '1200', '--height', '630', '--view', 'iso', '--no-grid'], check=True)


def kestrel():
    """The Kestrel showcase. Its page and data are in site/kestrel. The files it shows (the URDF and
    its meshes, the GIF, the BOM and a poster) are made here from kestrel.polykrom by Polykrom's CLI,
    so they always match the design (POLYKROM_CLI, else target/release/solidcraft-cli)."""
    import json, os, subprocess, tempfile
    src, out = ROOT / 'site' / 'kestrel', OUT / 'kestrel'
    shutil.copytree(src, out)
    cli = Path(os.environ.get('POLYKROM_CLI', ROOT / 'target' / 'release' / 'solidcraft-cli'))
    if not cli.exists():
        print('note: %s is missing, so the Kestrel downloads and animation are not in this build' % cli)
        return
    design = out / 'kestrel.polykrom'
    (out / 'urdf').mkdir()
    script = {'commands': [
        {'command': 'file.export', 'params': {'path': str(out / 'urdf' / 'kestrel.urdf')}},
        {'command': 'assembly.bom', 'params': {'path': str(out / 'kestrel-bom.csv')}},
        {'command': 'motion.export_animation', 'params': {'study': 'Pick and Place', 'path': str(out / 'kestrel.gif'), 'width': 800, 'height': 500}},
    ]}
    with tempfile.TemporaryDirectory() as tmp:
        sc = Path(tmp) / 'kestrel.json'
        sc.write_text(json.dumps(script))
        subprocess.run([str(cli), 'run', str(sc), '--in', str(design), '--quiet'], check=True)
        subprocess.run([str(cli), 'snapshot', '--in', str(design), '--out', str(out / 'kestrel-poster.png'), '--width', '1200', '--height', '630', '--view', 'iso', '--no-grid'], check=True)


def precompress(app):
    """Cloudflare serves static files up to 25 MiB; the app's wasm module is bigger. It ships as
    NAME.wasm.gz, and site/worker.js serves it as NAME.wasm with Content-Encoding: gzip, so the
    bytes the app receives are unchanged."""
    import gzip
    for wasm in sorted(app.glob('*.wasm')):
        data = wasm.read_bytes()
        if len(data) <= 24 * 1024 * 1024:
            continue
        wasm.with_name(wasm.name + '.gz').write_bytes(gzip.compress(data, 9, mtime=0))
        wasm.unlink()
    print(OUT)


if __name__ == '__main__':
    main()

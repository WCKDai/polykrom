#!/usr/bin/env python3
"""Assemble cad.polykrom.app into dist/site: the landing page, the browser app and the book.

    python3 site/build.py            after `trunk build --release` (dist/web) and `cargo xtask book`

The page's icons are the Facet masters themselves, recoloured with CSS variables; its mark and type
follow the WCKD family's kit (the pixel p, JetBrains Mono); nothing here is a second copy to keep in step.
"""
import re, shutil, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'dist' / 'site'
SVG = ROOT / 'assets' / 'icons' / 'ui' / 'design' / 'svg'
ROLE = {'#b9ddf8': 'var(--a1)', '#328bd3': 'var(--a2)', '#236ead': 'var(--a3)', '#ffffff': 'var(--gp)', '#6f879c': 'var(--muted)'}
STRIP = ['extrude', 'revolve', 'fillet', 'shell', 'hole', 'loft', 'sweep', 'plane_midplane', 'axis_two_planes',
         'rect', 'circle3', 'slot', 'c_parallel', 'c_tangent', 'joint', 'interference', 'section', 'measure']
# Polykrom's letter in the WCKD family (autobot-www brand/kit.json): a lowercase p, six cells tall.
P_GLYPH = ['#####.', '##..##', '##..##', '#####.', '##....', '##....']
LIME, INK = '#CCFF00', '#111111'


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


def p_mark(fill='currentColor', ground=None, animate=False):
    """The pixel p as SVG: one rect per cell (delayed one after another when `animate`), on a
    rounded `ground` tile when given (the favicon)."""
    cells = ''.join('<rect x="%d" y="%d" width="1" height="1"%s/>' % (x, y, ' style="animation-delay:%dms"' % ((x + y) * 70) if animate else '')
                    for y, row in enumerate(P_GLYPH) for x, ch in enumerate(row) if ch == '#')
    if ground:
        return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="-3 -3 12 12" shape-rendering="crispEdges">'
                '<rect x="-3" y="-3" width="12" height="12" rx="2.7" fill="%s"/><g fill="%s">%s</g></svg>' % (ground, fill, cells))
    return '<svg viewBox="0 0 6 6" role="img" aria-label="Polykrom" shape-rendering="crispEdges"><g fill="%s">%s</g></svg>' % (fill, cells)


# The masthead of every page: the p and the name, then the links split by rules (the WCKD kit).
PAGES = [('home', 'overview', ''), ('so101', 'so-101', 'so101/'), ('kestrel', 'kestrel', 'kestrel/'), (None, 'open the app', 'app/'),
         (None, 'docs', 'docs/'), (None, 'roadmap', 'https://github.com/wckdboy/polykrom/blob/main/docs/ROADMAP.md'),
         (None, 'source', 'https://github.com/wckdboy/polykrom')]


def nav(current, root):
    """The masthead for page `current`, with links relative to `root` ('' at the top, '../' a level
    down, '/' for pages served at any path)."""
    links = ''.join('<a href="%s"%s>%s</a>' % (href if href.startswith('http') else root + href, ' aria-current="page"' if key == current else '', label)
                    for key, label, href in PAGES)
    return ('<nav class="band"><div class="in"><a class="brand" href="%s">%s polykrom <small>/ polykrom.app</small></a>'
            '<div class="links">%s</div></div></nav>' % (root or './', p_mark(), links))


# The loop on the home page: its stages round a circle, the ones that exist today filled in.
LOOP = [('idea', True), ('design', True), ('verify', True), ('render', True), ('animate', True), ('simulate', True), ('make', False), ('measure', False)]


def loop_svg():
    import math
    out = []
    for i, (name, has) in enumerate(LOOP):
        a = -math.pi / 2 + i * 2 * math.pi / len(LOOP)
        x, y = 200 + 140 * math.cos(a), 200 + 140 * math.sin(a)
        lx, ly = 200 + 178 * math.cos(a), 200 + 178 * math.sin(a) + 4
        anchor = 'middle' if abs(math.cos(a)) < 0.3 else ('start' if math.cos(a) > 0 else 'end')
        out.append('<rect class="node%s" x="%.1f" y="%.1f" width="12" height="12"/><text class="lbl" x="%.1f" y="%.1f" text-anchor="%s">%s</text>'
                   % (' has' if has else '', x - 6, y - 6, lx, ly, anchor, name))
    return ''.join(out)


# Pictures on the home page, from the media branch (screenshots aren't committed), pinned by hash.
MEDIA_AT = 'https://raw.githubusercontent.com/wckdboy/polykrom/9242139a/'
MEDIA = {
    'site-lime-workspace.jpg': 'ebaf83fb49e7092e19ab710a989da6914a518d77ae84b39ce6c43be7a0c8274a',
    'site-follow-along-agent.jpg': '9750599c533542fc5de52cd527e4accf17c1fe0392923ede2859350ae42b912f',
    'site-parts-extrusions-steppers.jpg': '1baa7c5b8297f706b026158c423a1515ebd9f7f50484f25b3209a99d14f52ec4',
    'site-parts-rails-pulleys-leadscrews.jpg': '89d99f268879ca8a9cc1b942a91d874d9dd40cf6565a304b64f0edbe95a5b9af',
}


def media():
    import hashlib, urllib.request
    cache = ROOT / 'target' / 'showcase-cache'
    cache.mkdir(parents=True, exist_ok=True)
    (OUT / 'media').mkdir(parents=True, exist_ok=True)
    for name, sha in MEDIA.items():
        local = cache / ('%s-%s' % (sha[:12], name))
        if not local.exists():
            data = urllib.request.urlopen(MEDIA_AT + name, timeout=60).read()
            if hashlib.sha256(data).hexdigest() != sha:
                sys.exit('%s from the media branch does not match its pinned hash' % name)
            local.write_bytes(data)
        shutil.copy2(local, OUT / 'media' / name)


def fill_navs():
    """Every page's {{NAV:page}}: the copied showcases' too."""
    for page in OUT.rglob('*.html'):
        if page.is_relative_to(OUT / 'app') or page.is_relative_to(OUT / 'docs'):
            continue
        text = page.read_text()
        if '{{NAV:' not in text:
            continue
        depth = len(page.relative_to(OUT).parts) - 1
        root = '/' if page.name == '404.html' else '../' * depth
        page.write_text(re.sub(r'\{\{NAV:(\w+)\}\}', lambda m: nav(m.group(1), root), text))


def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    (OUT / 'fonts').mkdir(parents=True)
    page = (ROOT / 'site' / 'index.html').read_text()
    page = page.replace('{{MARK}}', p_mark()).replace('{{GEM}}', p_mark(animate=True))
    page = re.sub(r'\{\{ICON:(\w+)\}\}', lambda m: icon(m.group(1), 'c-' + m.group(1))[0], page)
    strip = ''.join('<figure>%s<figcaption>%s</figcaption></figure>' % icon(n, 's-' + n) for n in STRIP)
    page = page.replace('{{STRIP}}', strip).replace('{{LOOP}}', loop_svg())
    if re.search(r'\{\{(?!NAV:)', page):
        sys.exit('unfilled placeholder in site/index.html')
    (OUT / 'index.html').write_text(page)
    shutil.copy2(ROOT / 'site' / '404.html', OUT / '404.html')
    (OUT / 'mark.svg').write_text(p_mark(LIME, INK))
    shutil.copytree(ROOT / 'site' / 'screens', OUT / 'screens')
    # The showcases' 3D viewer, shared by their pages.
    shutil.copytree(ROOT / 'site' / 'shared', OUT / 'shared')
    for f in ('jetbrains-mono/jetbrains-mono-latin.woff2',):
        shutil.copy2(ROOT / 'assets' / 'fonts' / f, OUT / 'fonts')
    for src, dst in ((ROOT / 'dist' / 'web', 'app'), (ROOT / 'target' / 'book', 'docs')):
        if src.exists():
            shutil.copytree(src, OUT / dst)
        else:
            print('note: %s is missing, so /%s/ is not in this build' % (src.relative_to(ROOT), dst))
    precompress(OUT / 'app')
    media()
    kestrel()
    so101()
    fill_navs()


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
    cli = Path(os.environ.get('POLYKROM_CLI', ROOT / 'target' / 'release' / 'polykrom-cli'))
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
    so they always match the design (POLYKROM_CLI, else target/release/polykrom-cli)."""
    import json, os, subprocess, tempfile
    src, out = ROOT / 'site' / 'kestrel', OUT / 'kestrel'
    shutil.copytree(src, out)
    cli = Path(os.environ.get('POLYKROM_CLI', ROOT / 'target' / 'release' / 'polykrom-cli'))
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

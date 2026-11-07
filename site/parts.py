#!/usr/bin/env python3
"""Store the files Cloudflare cannot hold whole in pieces: split every file under DIR larger than
its 25 MiB per-file limit into NAME.part0, NAME.part1, ... (20 MiB each) plus a NAME.parts manifest
({"size": bytes, "parts": count}), and remove the original. site/worker.js serves NAME by joining
them in order, so clients receive the file unchanged.

    python3 site/parts.py DIR

The Flatpak repository (dist/site/flatpak/repo, packaging/flatpak-site.sh) needs this: OSTree
stores the app's binaries as single zlib-compressed objects over 25 MiB that compress no further.
Already-split files are left alone, so a second run changes nothing.
"""
import json, sys
from pathlib import Path

LIMIT = 24 * 1024 * 1024  # the same margin as site/build.py's precompress
PART = 20 * 1024 * 1024


def split(path):
    """Replace `path` with its pieces and manifest; returns the piece count."""
    size = path.stat().st_size
    count = 0
    with path.open('rb') as f:
        while True:
            chunk = f.read(PART)
            if not chunk:
                break
            path.with_name('%s.part%d' % (path.name, count)).write_bytes(chunk)
            count += 1
    path.with_name(path.name + '.parts').write_text(json.dumps({'size': size, 'parts': count}))
    path.unlink()
    return count


def main(root):
    if not root.is_dir():
        print('note: %s is missing, so there is nothing to split' % root)
        return
    for path in sorted(p for p in root.rglob('*') if p.is_file() and p.stat().st_size > LIMIT):
        if path.suffix == '.parts' or (path.suffix.startswith('.part') and path.suffix[5:].isdigit()):
            continue
        print('%s: %d bytes in %d parts' % (path.relative_to(root), path.stat().st_size, split(path)))


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(Path(sys.argv[1]))

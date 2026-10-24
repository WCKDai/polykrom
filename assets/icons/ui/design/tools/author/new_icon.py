#!/usr/bin/env python3
"""Start a new, unaccepted Facet icon from a standard base; never overwrite artwork.

Then move the drawing into facet_icons.py as a function, so the code stays the source.
"""
import argparse
from pathlib import Path
import re
import tomllib
from lib import Icon
from facet import svg
from bases import part, plate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('name', help='lower_snake_case app icon id')
    parser.add_argument('--base', choices=('part', 'plate', 'empty'), default='part')
    parser.add_argument('--family', choices=('model', 'flat', 'workspace'), default='model')
    args = parser.parse_args()
    design = Path(__file__).resolve().parents[2]
    if not re.fullmatch('[a-z][a-z0-9_]*', args.name): parser.error('use a lower_snake_case icon id')
    aliases = tomllib.loads((design / 'aliases.toml').read_text()).get('alias', {})
    if args.name in aliases or list((design / 'svg').glob('*/' + args.name + '.svg')):
        parser.error('id already exists; edit its function in facet_icons.py after checking review.json')
    if args.family != 'model' and args.base != 'empty': parser.error('part/plate bases belong to the model family')
    icon = Icon(args.name)
    if args.base == 'part': part(icon)
    if args.base == 'plate': plate(icon)
    path = design / 'svg' / args.family / (args.name + '.svg')
    path.write_text(svg(icon))
    root = design.parents[3]
    rel = f'assets/icons/ui/design/svg/{args.family}/{args.name}.svg'
    attribution = root / 'ATTRIBUTION.md'
    row = f'| {rel} | Polykrom contributors | original Facet icon master (64 grid) | MIT OR Apache-2.0 |\n'
    text = attribution.read_text()
    if '| ' + rel + ' |' not in text:
        anchor = '| assets/icons/ui/design/svg/model/placeholder.svg'
        if anchor in text: text = text.replace(anchor, row + anchor, 1)
        else: text += '\n' + row
        attribution.write_text(text)
    print(rel)
    print('Starter only: finish it in facet_icons.py using STYLE_GUIDE.md; review status is Open.')


if __name__ == '__main__': main()

"""The icon lint refuses an alias it cannot read exactly, as build.rs does: a quoted "true" or a
misspelt key used to pass both and draw the wrong master (PK-90 review, 2026-11-04)."""
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DESIGN = REPO / 'assets/icons/ui/design'


def lint(aliases_edit):
    # The lint reads the repository around the design folder (the kit sources of supplied masters,
    # assets/styles), so the copy keeps that layout and links the rest.
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        design = root / 'assets/icons/ui/design'
        shutil.copytree(DESIGN, design)
        for linked in ('assets/styles', 'assets/design-kit'):
            (root / linked).symlink_to(REPO / linked)
        path = design / 'aliases.toml'
        path.write_text(aliases_edit(path.read_text(encoding='utf-8')), encoding='utf-8')
        p = subprocess.run([sys.executable, '-B', str(design / 'tools/lint_icons.py'), str(design)], capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr


def facet_line(text):
    # An alias line, not the comment that documents the form.
    return re.search(r'^\w+ = \{.*facet = true.*$', text, re.M).group(0)


class AliasLint(unittest.TestCase):
    def test_the_committed_aliases_pass(self):
        code, out = lint(lambda t: t)
        self.assertEqual(code, 0, out)

    def test_a_quoted_facet_is_refused(self):
        code, out = lint(lambda t: t.replace(facet_line(t), facet_line(t).replace('facet = true', 'facet = "true"'), 1))
        self.assertNotEqual(code, 0)
        self.assertIn('facet must be true or false', out)

    def test_a_misspelt_key_is_refused(self):
        code, out = lint(lambda t: t.replace(facet_line(t), facet_line(t).replace('facet = true', 'facte = true'), 1))
        self.assertNotEqual(code, 0)
        self.assertIn('unknown key facte', out)


if __name__ == '__main__':
    unittest.main()

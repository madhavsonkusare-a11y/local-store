"""Offline importer regression tests: identity, provenance and untrusted inputs."""
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import yaml
import catalog_pipeline as pipeline
import v2_catalog_icons as v2_icons

spec = importlib.util.spec_from_file_location('icons', Path(__file__).with_name('cache-catalog-icons.py'))
icons = importlib.util.module_from_spec(spec)
spec.loader.exec_module(icons)


class CatalogTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'catalog/sources').mkdir(parents=True)
        (self.root / 'catalog/legacy.json').write_text('[]')
        self.root_patch = patch.object(pipeline, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)

    def generate(self, records):
        source = {'id': 'awesome-selfhosted', 'repository': 'example/catalog', 'revision': 'a' * 40}
        data = pipeline.encode({'source': source['id'], 'revision': source['revision'], 'records': records, 'excluded': []})
        (self.root / 'catalog/sources/awesome-selfhosted.json').write_bytes(data)
        source['sha256'] = pipeline.hashlib.sha256(data).hexdigest()
        self.lock = {'snapshot_date': '2026-09-05', 'sources': [source]}
        return pipeline.generate(self.lock)

    def row(self, name, source, website=None):
        return pipeline.row(name, source, website, 'A useful project.', ['analytics'], upstream_id=name)

    def test_distinct_projects_do_not_merge_by_name(self):
        catalog, _, _ = self.generate([
            self.row('Memex', 'https://github.com/one/memex'),
            self.row('Memex', 'https://github.com/two/memex'),
        ])
        self.assertEqual(len(catalog['entries']), 2)
        self.assertEqual(len({entry['id'] for entry in catalog['entries']}), 2)

    def test_repository_and_documentation_variants_merge_with_provenance(self):
        catalog, report, _ = self.generate([
            self.row('Notes', 'https://github.com/team/notes', 'https://notes.example.com'),
            self.row('Notes', 'https://docs.notes.example.com/setup'),
            self.row('Notes Docker', 'https://github.com/team/notes/tree/main/docker'),
        ])
        self.assertEqual(len(catalog['entries']), 1)
        self.assertEqual(len(catalog['entries'][0]['sources']), 3)
        self.assertEqual(len(report['merges']), 1)

    def test_stable_identity_survives_project_rename(self):
        catalog, _, ids = self.generate([self.row('Old Name', 'https://github.com/team/tool')])
        (self.root / 'catalog/ids.json').write_bytes(pipeline.encode(ids))
        renamed, _, _ = self.generate([self.row('New Name', 'https://github.com/team/tool')])
        self.assertEqual(catalog['entries'][0]['id'], renamed['entries'][0]['id'])
        self.assertEqual(renamed['entries'][0]['name'], 'New Name')

    def test_modified_snapshot_is_rejected(self):
        self.generate([self.row('Notes', 'https://github.com/team/notes')])
        (self.root / 'catalog/sources/awesome-selfhosted.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            pipeline.generate(self.lock)

    def test_categories_and_urls_are_normalized_without_guessing_instances(self):
        self.assertEqual(pipeline.normalize_category('analytics'), 'Analytics')
        self.assertEqual(pipeline.normalize_category('AI'), 'Artificial Intelligence')
        self.assertEqual(pipeline.normalize_category('Money, Budgeting & Management'), 'Money, Budgeting & Management')
        self.assertNotEqual(pipeline.identity('https://gitlab.com/team/group/one'), pipeline.identity('https://gitlab.com/team/group/two'))
        for url in ['file:///tmp/icon', 'https://user:secret@example.com', 'https://exa mple.com']:
            self.assertIsNone(pipeline.http_url(url))
        with self.assertRaises(yaml.constructor.ConstructorError):
            pipeline.adapter({'id': 'awesome-selfhosted'}, 'bad.yml', '!!python/object/apply:os.system [echo nope]')

    def test_icons_reject_active_external_and_oversized_content(self):
        icons.validate_svg(b'<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0"/></svg>')
        for svg in [b'<svg><script>bad()</script></svg>', b'<svg onload="bad()"/>',
                    b'<svg><use href="https://example.com/a.svg"/></svg>',
                    b'<!DOCTYPE svg [<!ENTITY a "x">]><svg/>',
                    b'<svg><style>@import "https://example.com";</style></svg>',
                    b' ' * (icons.MAX_BYTES + 1)]:
            with self.assertRaises(ValueError):
                icons.validate_svg(svg)
        with self.assertRaisesRegex(ValueError, 'Unapproved icon host'):
            icons.fetch('http://127.0.0.1/private', 100)

    def test_png_icons_require_valid_bounded_static_images(self):
        from PIL import Image
        def png(size=(32, 32), **options):
            output = io.BytesIO()
            Image.new('RGBA', size, 'orange').save(output, format='PNG', **options)
            return output.getvalue()
        data = png()
        icons.validate_icon(data, '.png')
        for invalid in [b'<svg/>', data[:30], png((2049, 1)),
                        b'\x89PNG\r\n\x1a\n' + bytes(icons.MAX_BYTES),
                        png(save_all=True, append_images=[Image.new('RGBA', (32, 32), 'blue')])]:
            with self.assertRaises((ValueError, OSError, SyntaxError)):
                icons.validate_icon(invalid, '.png')
        with self.assertRaises(ValueError):
            icons.validate_icon(data, '.html')

    def test_icon_matching_uses_reviewed_aliases_and_png_fallback(self):
        paths = {'svg/actual-budget.svg', 'png/actual-budget.png',
                 'svg/actual-budget-light.svg', 'png/dashy.png', 'svg/readwise-reader.svg'}
        self.assertEqual(icons.homarr_candidates('actual', paths, {'actual':'actual-budget'}),
                         ['svg/actual-budget-light.svg', 'svg/actual-budget.svg', 'png/actual-budget.png'])
        self.assertEqual(icons.homarr_candidates('dashy', paths, {}), ['png/dashy.png'])
        # The Python feed reader is not Readwise Reader, despite a shared word.
        self.assertEqual(icons.homarr_candidates('reader', paths, {}), [])
        self.assertEqual(icons.homarr_candidates('actual', paths, {'actual':'svg/actual-budget.svg'}),
                         ['svg/actual-budget.svg'])

    def test_v2_monograms_are_deterministic_safe_and_square(self):
        entry = {'id': 'missing-app', 'name': 'Missing App'}
        first = v2_icons.monogram_svg(entry)
        second = v2_icons.monogram_svg(entry)
        self.assertEqual(first, second)
        self.assertNotIn(b'<text', first)
        self.assertNotIn(b'<image', first)
        metadata = v2_icons.validate_icon(first, '.svg')
        v2_icons.validate_catalog_suitability(metadata)
        self.assertEqual((metadata['width'], metadata['height']), (64, 64))

    def test_icon_overrides_must_name_a_locked_source_and_a_plain_file(self):
        import json, tempfile
        from pathlib import Path as P
        sources = {"paperclip": {"license": "MIT"}, "unlicensed": {"license": "NOASSERTION"}}
        original = v2_icons.OVERRIDES
        try:
            with tempfile.TemporaryDirectory() as folder:
                v2_icons.OVERRIDES = P(folder) / "overrides.json"
                for override in ({"source": "unlocked", "path": "logo.svg"},
                                 {"source": "unlicensed", "path": "logo.svg"},
                                 {"source": "paperclip", "path": "../escape.svg"},
                                 {"source": "paperclip", "path": "logo.exe"}):
                    v2_icons.OVERRIDES.write_text(json.dumps({"app": override}))
                    with self.assertRaises(ValueError):
                        v2_icons.load_overrides(sources)
                v2_icons.OVERRIDES.write_text(json.dumps({"_comment": "x", "app": {"source": "paperclip", "path": "docs/favicon.svg"}}))
                self.assertEqual(list(v2_icons.load_overrides(sources)), ["app"])
        finally:
            v2_icons.OVERRIDES = original

    def test_icon_manifest_refuses_unlicensed_artwork_before_publication(self):
        for license_name in (None, "", "NONE", "NOASSERTION", "UNLICENSED"):
            manifest = {"schema_version": 2, "icons": {"app": {"license": license_name}}}
            with self.assertRaisesRegex(ValueError, "no reviewed artwork license"):
                v2_icons.validate_manifest(manifest, [{"id": "app"}], {})

    def test_v2_matching_never_uses_unreviewed_fuzzy_names(self):
        paths = {'netbird/icon.svg', 'reader/icon.svg', 'readwise-reader/icon.svg'}
        self.assertEqual(v2_icons.umbrel_candidates('netbird-client', paths, {'netbird-client': 'netbird'}),
                         [('netbird/icon.svg', 'reviewed-alias')])
        self.assertEqual(v2_icons.umbrel_candidates('reader', paths, {}),
                         [('reader/icon.svg', 'exact-id')])
        self.assertEqual(v2_icons.umbrel_candidates('readwise', paths, {}), [])


if __name__ == '__main__':
    unittest.main()

"""Small draft-generation boundaries, without registry or app installation."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('template_generator',Path(__file__).with_name('generate-template.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class DraftTests(unittest.TestCase):
    def test_pin_changes_only_one_declared_image_tag(self):
        images = ['mtlynch/picoshare:1.4.4']
        pins = module.reviewed_image_pins(['mtlynch/picoshare:1.4.4=mtlynch/picoshare:v1.5.4'],images)
        self.assertEqual(pins[0]['replacement'],'mtlynch/picoshare:v1.5.4')
        self.assertTrue(pins[0]['reason'].startswith('REVIEW:'))
        for values in [
            ['mtlynch/picoshare:1.4.4=attacker/picoshare:1.5.4'],
            ['mtlynch/picoshare:1.4.4=mtlynch/picoshare:latest'],
            ['mtlynch/picoshare:1.0=mtlynch/picoshare:v1.5.4'],
            ['mtlynch/picoshare:1.4.4=mtlynch/picoshare:v1.5.4']*2,
        ]:
            with self.assertRaises(SystemExit):
                module.reviewed_image_pins(values,images)

    def test_draft_requires_new_proposal_output_before_any_network(self):
        for argv in [
            ['generate-template.py','picoshare','--draft'],
            ['generate-template.py','picoshare','--draft','--output','src/templates/picoshare.json'],
            ['generate-template.py','../picoshare','--draft','--output','catalog/promotion-proposals/x.json'],
            ['generate-template.py','picoshare','--image-pin','a:b=a:c'],
        ]:
            with patch('sys.argv',argv), patch.object(module,'fetch',side_effect=AssertionError('network must not run')):
                with self.assertRaises(SystemExit):
                    module.main()

    def test_actual_draft_is_withheld_and_has_no_fabricated_proof(self):
        proposal = json.loads((module.ROOT / 'catalog/promotion-proposals/picoshare-withheld.json').read_text())
        self.assertEqual(proposal['promotion']['state'],'withheld')
        self.assertEqual(proposal['lifecycle_proof'],'')
        self.assertEqual(proposal['origin']['license'],'Apache-2.0')
        self.assertTrue(proposal['fields']['CAP_PS_SHARED_SECRET']['required'])
        self.assertTrue(proposal['fields']['CAP_PS_SHARED_SECRET']['sensitive'])
        self.assertFalse(any(note.startswith('Proven on Windows') for note in proposal['risk_notes']))
        roster = json.loads((module.ROOT / 'catalog/v1-roster.json').read_text())
        self.assertEqual(len(roster['apps']),100)
        self.assertEqual(next(row for row in roster['apps'] if row['id']=='picoshare')['cohort'],'expansion_candidate')
        launch = json.loads((module.ROOT / 'catalog/v1-qualified-apps.json').read_text())
        self.assertEqual(launch['target'],10)
        self.assertNotIn('picoshare',[row['id'] for row in launch['apps']])


if __name__ == '__main__':
    unittest.main()

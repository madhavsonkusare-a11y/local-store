"""Targeted source drift and approval boundary checks; fixtures are not app proof."""
import copy
from hashlib import sha256
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('promotion_receipt', Path(__file__).with_name('github-promotion-receipt.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PromotionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = json.loads((module.ROOT / 'src/templates/privatebin.json').read_text())
        self.proof = json.loads((module.ROOT / 'docs/evidence/privatebin-managed-paste-2026-09-30.json').read_text())
        self.now = self.proof['recorded_at_unix'] + 30
        origin = self.manifest['origin']
        archive = self.root / f".cache/catalog/runtipi-{origin['revision']}.zip"
        archive.parent.mkdir(parents=True)
        with zipfile.ZipFile(archive, 'w') as bundle:
            bundle.writestr('fixture/' + origin['path'], self.manifest['definition'])
            bundle.writestr('fixture/' + self.manifest['config']['path'], self.manifest['config']['content'])
        self.pin = {key: origin[key] for key in ('repository','revision','license')}
        self.pin['sha256'] = sha256(archive.read_bytes()).hexdigest()
        self.candidate = {'source':'runtipi','id':'privatebin','identity':'github:privatebin/privatebin',
                          'identity_status':'reviewed_repository_match','importable':True,
                          'maintenance_status':'pinned_release_matches_latest_project_release',
                          'provenance':{key:origin[key] for key in ('repository','revision','path')}}
        self.candidate['provenance']['archive_sha256'] = self.pin['sha256']
        self.facts = {'schema_version':1,'observed_at_unix':self.now,'app':'privatebin',
                      'plan_sha256':self.proof['identity']['plan_sha256'],
                      'imported_definition_sha256':sha256(self.manifest['definition'].encode()).hexdigest(),
                      'source_resolution':{'kind':'approved_match','offering_id':'privatebin','repository':'https://github.com/privatebin/privatebin'},
                      'launch_readiness':{'current_evidence':True,'task_verified':True,'lifecycle_proven':True}}
        self.write('catalog/import-audit-sources.json',{'runtipi':self.pin})
        self.write('catalog/candidate-queue.json',{'candidates':[self.candidate]})
        self.write('catalog/v1-roster.json',{'apps':[{'id':'privatebin','cohort':'existing_offering','manifest':'src/templates/privatebin.json'}]})
        self.entry = next(row for row in json.loads((module.ROOT / 'catalog/v1-qualified-apps.json').read_text())['apps'] if row['id']=='privatebin')
        probe = module.ROOT / self.entry['probe']
        (self.root / self.entry['probe']).parent.mkdir(parents=True,exist_ok=True)
        (self.root / self.entry['probe']).write_bytes(probe.read_bytes())
        self.sync()

    def write(self, path, value):
        target = self.root / path
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_text(json.dumps(value),encoding='utf-8')

    def sync(self):
        self.write(self.entry['manifest'],self.manifest)
        self.write(self.entry['evidence'],self.proof)
        self.entry['manifest_sha256'] = sha256((self.root / self.entry['manifest']).read_bytes()).hexdigest()
        self.entry['evidence_sha256'] = sha256((self.root / self.entry['evidence']).read_bytes()).hexdigest()
        self.write('catalog/v1-qualified-apps.json',{'apps':[self.entry]})

    def build(self, facts=None):
        return module.build(self.root,'privatebin',facts or self.facts,self.now)

    def test_receipt_only_records_existing_promotion(self):
        before = (self.root / self.entry['manifest']).read_bytes()
        result = self.build()
        self.assertIn('no automatic approval',result['scope'])
        self.assertEqual(result['source']['archive_sha256'],self.pin['sha256'])
        self.assertEqual((self.root / self.entry['manifest']).read_bytes(),before)

    def test_definition_change_and_archive_corruption_refuse(self):
        self.manifest['definition'] += '\n '
        self.sync()
        changed = copy.deepcopy(self.facts)
        changed['imported_definition_sha256'] = sha256(self.manifest['definition'].encode()).hexdigest()
        self.build(changed)  # trailing whitespace does not alter the definition
        self.manifest['definition'] = self.manifest['definition'].replace('2.0.6','unreviewed')
        self.sync()
        with self.assertRaisesRegex(ValueError,'definition bytes'):
            self.build()
        archive = next((self.root / '.cache/catalog').glob('*.zip'))
        archive.write_bytes(archive.read_bytes() + b'corruption')
        with self.assertRaisesRegex(ValueError,'source archive hash changed'):
            self.build()

    def test_withheld_or_maintenance_blocker_never_becomes_install_approval(self):
        self.manifest['promotion']['state'] = 'withheld'
        self.sync()
        with self.assertRaisesRegex(ValueError,'approved template'):
            self.build()
        self.manifest['promotion']['state'] = 'approved'
        self.sync()
        self.candidate['maintenance_status'] = 'withhold_stale_pinned_image'
        self.write('catalog/candidate-queue.json',{'candidates':[self.candidate]})
        with self.assertRaisesRegex(ValueError,'blocker'):
            self.build()

    def test_stale_changed_plan_and_missing_launcher_proof_refuse(self):
        for field,value in [('observed_at_unix',self.now-901),('plan_sha256','f'*64)]:
            facts = copy.deepcopy(self.facts)
            facts[field] = value
            with self.assertRaises(ValueError):
                self.build(facts)
        for flag in ('current_evidence','task_verified','lifecycle_proven'):
            facts = copy.deepcopy(self.facts)
            facts['launch_readiness'][flag] = False
            with self.assertRaisesRegex(ValueError,'proof projection'):
                self.build(facts)

    def test_repository_alias_or_missing_review_cannot_inherit_approval(self):
        facts = copy.deepcopy(self.facts)
        facts['source_resolution']['repository'] = 'https://github.com/attacker/privatebin'
        with self.assertRaisesRegex(ValueError,'repository must resolve'):
            self.build(facts)
        self.candidate['identity_status'] = 'source_declared_repository'
        self.write('catalog/candidate-queue.json',{'candidates':[self.candidate]})
        with self.assertRaisesRegex(ValueError,'identity review'):
            self.build()

    def test_failed_task_proof_refuses_even_with_updated_ledger_digest(self):
        self.proof['passed'] = False
        self.sync()
        with self.assertRaisesRegex(ValueError,'passing schema-2 proof'):
            self.build()


if __name__ == '__main__':
    unittest.main()

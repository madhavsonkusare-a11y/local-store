"""Synthetic promotion fixtures test guards; they never qualify an app."""
import copy
from hashlib import sha256
import importlib.util
import json
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location('candidate_promotion',Path(__file__).with_name('candidate-promotion.py'))
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)


class CandidatePromotionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name); self.now = int(time.time())
        self.proposal_path = 'catalog/promotion-proposals/picoshare-withheld.json'
        self.evidence_path = 'docs/evidence/synthetic-candidate-proof.json'
        self.probe_path = 'scripts/picoshare-file-probe.mjs'
        self.proposal = json.loads((module.ROOT/self.proposal_path).read_text())
        self.evidence = json.loads((module.ROOT/'docs/evidence/privatebin-managed-paste-2026-09-30.json').read_text())
        self.probe = (module.ROOT/self.probe_path).read_bytes()
        self.origin = self.proposal['origin']
        self.write(f".cache/definitions/{self.origin['revision']}/public/v4/apps/picoshare.json",json.loads(self.proposal['definition']))
        archive = self.root/f".cache/catalog/caprover-{self.origin['revision']}.zip"; archive.parent.mkdir(parents=True,exist_ok=True)
        with zipfile.ZipFile(archive,'w') as bundle:
            bundle.writestr('fixture/'+self.origin['path'],self.proposal['definition'])
        archive_sha = sha256(archive.read_bytes()).hexdigest()
        pin = {key:self.origin[key] for key in ('repository','revision','license')}; pin['sha256'] = archive_sha
        self.write('catalog/import-audit-sources.json',{'caprover':pin})
        candidate = {'id':'picoshare','source':'caprover','importable':True,'provenance':{key:self.origin[key] for key in ('repository','revision','path')}}
        candidate['provenance']['archive_sha256'] = archive_sha
        self.write('catalog/candidate-queue.json',{'candidates':[candidate]})
        self.write('catalog/v1-roster.json',{'apps':[{'id':'picoshare','cohort':'expansion_candidate','source_url':self.proposal['source_url']}]})
        task = 'A synthetic test fixture binds a chosen passphrase, exact uploaded file, expiration and byte-identical download across lifecycle phases.'
        self.probe_sha = sha256(task.encode()+b'\0script-source:'+self.probe).hexdigest()
        self.evidence.update(app='picoshare',recorded_at_unix=self.now,first_use=task,promotion='withheld')
        self.evidence['identity'].update(source_adapter='caprover',source_locator=f"{self.origin['repository']}#{self.origin['path']}",source_revision=self.origin['revision'],source_observed_on=self.proposal['verified_at'],images_observed_on=self.proposal['requirements']['images'][0]['checked_at'],requested_images=['mtlynch/picoshare:v1.5.4'],resolved_image_ids={'mtlynch/picoshare:v1.5.4':self.proposal['requirements']['images'][0]['index_digest']},probe_sha256=self.probe_sha)
        self.facts = {'schema_version':1,'observed_at_unix':self.now,'app':'picoshare','installable':False,'plan_sha256':self.evidence['identity']['plan_sha256'],'github_inspection':{'canonical_repository':self.proposal['source_url'],'archived':False,'repository_id':1,'commit_sha':'1'*40}}
        target = self.root/self.probe_path; target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(self.probe)
        self.sync()
        self.addCleanup(patch.stopall)
        patch.object(module,'ROOT',self.root).start(); patch.object(module.generator,'ROOT',self.root).start()

    def write(self,path,value):
        target = self.root/path; target.parent.mkdir(parents=True,exist_ok=True)
        target.write_text(json.dumps(value),encoding='utf-8')

    def sync(self):
        self.write(self.proposal_path,self.proposal); self.write(self.evidence_path,self.evidence)
        self.facts['proposal_sha256'] = sha256((self.root/self.proposal_path).read_bytes()).hexdigest()

    def prepare(self,decision=None):
        return module.prepare(self.root,self.proposal_path,self.evidence_path,self.probe_path,self.facts,self.now,decision)

    def decision(self):
        request,_ = self.prepare()
        return {'schema_version':1,'app':'picoshare','decision':'approved','bindings':request['bindings'],'reviewer':'Synthetic unit-test reviewer','reason':'This is only a synthetic decision fixture testing immutable review bindings; it is never published or registered.','reviewed_at_unix':self.now}

    def test_valid_proof_creates_review_request_without_approval(self):
        request,approved = self.prepare()
        self.assertIsNone(approved); self.assertFalse(request['installable'])
        self.assertEqual(request['decision'],'pending_explicit_review')
        self.assertEqual(json.loads((self.root/self.proposal_path).read_text())['promotion']['state'],'withheld')

    def test_bound_explicit_decision_only_produces_artifact_not_registration(self):
        request,approved = self.prepare(self.decision())
        self.assertEqual(approved['promotion']['state'],'approved')
        self.assertEqual(approved['lifecycle_proof'],self.evidence_path)
        self.assertFalse(request['installable']); self.assertIn('registration_required',request)
        self.assertFalse((self.root/'src/templates/picoshare.json').exists())

    def test_failed_proof_and_changed_plan_refuse(self):
        self.evidence['passed'] = False; self.sync()
        with self.assertRaisesRegex(ValueError,'passing schema-2'):
            self.prepare()
        self.evidence['passed'] = True; self.sync(); self.facts['plan_sha256'] = '0'*64
        with self.assertRaisesRegex(ValueError,'normalized candidate plan'):
            self.prepare()

    def test_unreviewed_source_edit_is_refused_before_promotion(self):
        self.proposal['definition'] = self.proposal['definition'].replace('4001','4444'); self.sync()
        with self.assertRaisesRegex(ValueError,'rewrites the pinned'):
            self.prepare()

    def test_stale_or_mismatched_explicit_decision_refuses(self):
        for change in ('bindings','reviewed_at_unix'):
            decision = self.decision()
            if change == 'bindings': decision['bindings']['evidence_sha256'] = '0'*64
            else: decision['reviewed_at_unix'] = self.now-86401
            with self.assertRaises(ValueError): self.prepare(decision)

    def test_missing_image_pin_or_unfinished_review_refuses(self):
        self.proposal['requirements']['images'][0]['index_digest'] = None; self.sync()
        with self.assertRaises(ValueError): self.prepare()
        self.proposal['risk_notes'].append('REVIEW: unfinished task'); self.sync()
        with self.assertRaisesRegex(ValueError,'unfinished'): self.prepare()

    def test_changed_live_repository_and_unrelated_image_cannot_reuse_approval(self):
        decision = self.decision()
        self.facts['github_inspection']['commit_sha'] = '2'*40
        with self.assertRaisesRegex(ValueError,'bind these exact'): self.prepare(decision)
        self.evidence['identity']['resolved_image_ids'] = {'unrelated/image:tag':'sha256:'+'a'*64}
        self.sync()
        with self.assertRaisesRegex(ValueError,'image identities'): self.prepare()


if __name__ == '__main__': unittest.main()

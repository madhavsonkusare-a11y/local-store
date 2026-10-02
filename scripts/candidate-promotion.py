#!/usr/bin/env python3
"""Prepare a review request, then an approved artifact only from bound review.

No catalog/manifest registration, app execution or automatic approval occurs.
The operator must review the concrete request and provide a separate decision
that names the exact proposal, task proof, probe and normalized plan hashes.
"""
import argparse
from datetime import datetime, timezone
from hashlib import sha256
import importlib.util
import json
from pathlib import Path
import re
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
def module(name, filename):
    spec = importlib.util.spec_from_file_location(name,ROOT / 'scripts' / filename)
    result = importlib.util.module_from_spec(spec); spec.loader.exec_module(result)
    return result
proofs = module('candidate_proof_rules','check-v1-qualified.py')
generator = module('candidate_generator','generate-template.py')


def prepare(root, proposal_path, evidence_path, probe_path, facts, now, decision=None):
    proposal_bytes = proofs.file_bytes(root,proposal_path,'catalog/promotion-proposals/')
    proposal = json.loads(proposal_bytes)
    app_id = proposal.get('id')
    if not isinstance(app_id,str) or not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,79}',app_id):
        raise ValueError('invalid proposal app identity')
    roster = proofs.read_json(root,'catalog/v1-roster.json')
    matches = [row for row in roster['apps'] if row.get('id') == app_id]
    if len(matches) != 1 or matches[0].get('cohort') != 'expansion_candidate':
        raise ValueError('one frozen expansion candidate required; do not change the roster silently')
    if proposal.get('promotion',{}).get('state') != 'withheld':
        raise ValueError('input must remain a withheld proposal')
    if 'REVIEW:' in proposal_bytes.decode():
        raise ValueError('unfinished source/setup/image review remains')
    origin = proposal['origin']
    queue = proofs.read_json(root,'catalog/candidate-queue.json')
    candidates = [row for row in queue['candidates'] if row.get('source') == origin['importer'] and row.get('provenance',{}).get('path') == origin['path'] and row.get('provenance',{}).get('revision') == origin['revision']]
    if len(candidates) != 1 or candidates[0].get('importable') is not True:
        raise ValueError('exact structurally supported source candidate required')
    if origin.get('repository') != candidates[0]['provenance'].get('repository'):
        raise ValueError('proposal source repository changed')
    if root.resolve() == ROOT.resolve():
        generator.verify_candidate_source(candidates[0])
    else:
        raise ValueError('promotion source check requires the actual project root')
    source_pin = proofs.read_json(root,'catalog/import-audit-sources.json')[origin['importer']]
    if origin.get('license') != source_pin.get('license') or proposal.get('license') in (None,'','NOASSERTION'):
        raise ValueError('reviewed app and definition licenses are required')
    definition_path = root/'.cache/definitions'/origin['revision']/origin['path']
    if definition_path.suffix in ('.yaml','.yml'):
        definition_path = definition_path.with_suffix('.json')
    if json.loads(definition_path.read_text()) != json.loads(proposal['definition']):
        raise ValueError('proposal rewrites the pinned source definition')
    if origin['importer'] == 'runtipi':
        if proposal.get('config',{}).get('content','').strip() != definition_path.with_name('config.json').read_text().strip():
            raise ValueError('proposal rewrites the pinned companion configuration')
    evidence_bytes = proofs.file_bytes(root,evidence_path,'docs/evidence/')
    probe_bytes = proofs.file_bytes(root,probe_path,'scripts/')
    evidence = json.loads(evidence_bytes); identity = evidence.get('identity',{})
    if evidence.get('app') != app_id or evidence.get('schema_version') != 2 or evidence.get('passed') is not True:
        raise ValueError('passing schema-2 candidate proof required')
    if identity.get('host_os') != 'windows' or identity.get('host_arch') != 'x86_64' or identity.get('engine') != {'schema_version':2,'program':'wsl.exe','endpoint':'wsl://local-store-engine-v1'}:
        raise ValueError('managed Windows engine proof required')
    if identity.get('source_locator') != f"{origin['repository']}#{origin['path']}" or identity.get('source_adapter') != origin['importer'] or identity.get('source_revision') != origin['revision']:
        raise ValueError('task proof does not name the reviewed source')
    expected_images = [image['image'] for image in proposal['requirements']['images']]
    for image in proposal['requirements']['images']:
        pin = image.get('index_digest','')
        if not isinstance(pin,str) or not re.fullmatch(r'sha256:[0-9a-f]{64}',pin) or 'linux/amd64' not in image.get('container_platforms',[]):
            raise ValueError('every image needs an immutable supported Windows-engine platform pin')
    resolved = identity.get('resolved_image_ids')
    if (set(identity.get('requested_images',[])) != set(expected_images)
            or not isinstance(resolved,dict) or set(resolved) != set(expected_images)
            or any(not isinstance(value,str) or not re.fullmatch(r'sha256:[0-9a-f]{64}',value) for value in resolved.values())):
        raise ValueError('task proof image identities differ from the proposal')
    today = datetime.fromtimestamp(now,timezone.utc).date()
    recorded = evidence.get('recorded_at_unix')
    if not isinstance(recorded,int) or not 0 <= now-recorded <= 30*86400:
        raise ValueError('candidate task proof is expired or from the future')
    image_day = min(image['checked_at'] for image in proposal['requirements']['images'])
    if identity.get('source_observed_on') != proposal['verified_at'] or identity.get('images_observed_on') != image_day or not proofs.fresh_day(image_day,30,today) or not proofs.fresh_day(proposal['verified_at'],90,today):
        raise ValueError('source or image observation changed or expired')
    steps = evidence.get('steps')
    if not isinstance(steps,list) or not all(isinstance(row,dict) and row.get('passed') is True for row in steps) or not proofs.REQUIRED_STEPS.issubset({row.get('step') for row in steps}):
        raise ValueError('meaningful task, restart, reinstall or safe cleanup proof is missing')
    measurements = evidence.get('measurements',{})
    if measurements.get('samples',0) < 3 or measurements.get('idle_named_volume_bytes') is None:
        raise ValueError('resource and owned-volume measurements required')
    task = evidence.get('first_use')
    if not isinstance(task,str) or len(task) < 40:
        raise ValueError('a meaningful exact-state app task is required')
    probe_sha = sha256(task.encode()+b'\0script-source:'+probe_bytes).hexdigest()
    proofs.require_digest(probe_sha,identity.get('probe_sha256'),'task probe')
    proposal_sha = sha256(proposal_bytes).hexdigest()
    if facts.get('schema_version') != 1 or facts.get('app') != app_id or facts.get('proposal_sha256') != proposal_sha or facts.get('installable') is not False:
        raise ValueError('same withheld proposal facts required')
    if not isinstance(facts.get('observed_at_unix'),int) or not 0 <= now-facts['observed_at_unix'] <= 900:
        raise ValueError('fresh compiled plan facts required')
    proofs.require_digest(facts.get('plan_sha256'),identity.get('plan_sha256'),'normalized candidate plan')
    inspection = facts.get('github_inspection',{})
    canonical = matches[0]['source_url'].lower()
    if (inspection.get('canonical_repository') != canonical or inspection.get('archived') is not False
            or type(inspection.get('repository_id')) is not int or inspection['repository_id'] <= 0
            or not re.fullmatch(r'[0-9a-f]{40}',inspection.get('commit_sha',''))):
        raise ValueError('live canonical public repository review required')
    bindings = {'proposal_sha256':proposal_sha,'evidence_sha256':sha256(evidence_bytes).hexdigest(),'plan_sha256':facts['plan_sha256'],'probe_sha256':probe_sha,'source_archive_sha256':candidates[0]['provenance']['archive_sha256'],'github_inspection_sha256':sha256(json.dumps(inspection,sort_keys=True,separators=(',',':')).encode()).hexdigest()}
    request = {'schema_version':1,'app':app_id,'installable':False,'decision':'pending_explicit_review','bindings':bindings,'proposal':proposal_path,'evidence':evidence_path,'probe':probe_path,'task':task,'source_archive_sha256':candidates[0]['provenance']['archive_sha256'],'github_inspection':inspection,'review_needs':['source and license obligations','supported runtime base and image maintenance; task success does not prove patched packages','same-image version compatibility','required secret setup','loopback exposure and owned storage','meaningful task and measured resource ceilings'],'launch_roster_change':False}
    if decision is None:
        return request,None
    if decision.get('schema_version') != 1 or decision.get('app') != app_id or decision.get('decision') != 'approved' or decision.get('bindings') != bindings:
        raise ValueError('explicit decision must bind these exact reviewed inputs')
    if not isinstance(decision.get('reviewer'),str) or not decision['reviewer'].strip() or not isinstance(decision.get('reason'),str) or len(decision['reason'].strip()) < 40:
        raise ValueError('named reviewer and concrete promotion reason required')
    at = decision.get('reviewed_at_unix')
    if not isinstance(at,int) or not 0 <= now-at <= 86400:
        raise ValueError('review decision must be current and not future-dated')
    approved = dict(proposal)
    approved['promotion'] = {'state':'approved','reason':decision['reason']}
    approved['lifecycle_proof'] = evidence_path
    approved['risk_notes'] = [note for note in proposal['risk_notes'] if not note.startswith('Not qualified on the Local Store managed engine.')]
    approved['risk_notes'].append('A real managed Windows task, restart and keep-data reinstall passed for this exact reviewed plan. macOS and Linux host proof and agent content access remain unverified.')
    request['decision'] = 'approved_artifact_only'
    request['review'] = decision
    request['registration_required'] = ['explicit reviewed template registration','agent coverage row with accurate unavailable capabilities','frozen roster cohort reconciliation','catalog regeneration']
    return request,approved


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('proposal','evidence','probe','facts','output'):
        parser.add_argument('--'+name,required=True)
    parser.add_argument('--decision',help='separate explicit review decision; never inferred from app proof')
    parser.add_argument('--approved-output',help='new approved artifact under catalog/promotion-proposals/, not automatic registration')
    args = parser.parse_args()
    try:
        if bool(args.decision) != bool(args.approved_output):
            raise ValueError('an approved artifact requires a separate explicit decision')
        facts = json.loads(Path(args.facts).read_text(encoding='utf-8-sig'))
        decision = json.loads(Path(args.decision).read_text(encoding='utf-8-sig')) if args.decision else None
        request,approved = prepare(ROOT,args.proposal,args.evidence,args.probe,facts,int(time.time()),decision)
        outputs = [(args.output,request)] + ([(args.approved_output,approved)] if approved else [])
        targets = [(ROOT/path).resolve() for path,_ in outputs]
        if len(set(targets)) != len(targets) or any(not target.is_relative_to((ROOT/'catalog/promotion-proposals').resolve()) or target.exists() for target in targets):
            raise ValueError('outputs must be distinct new files under catalog/promotion-proposals/')
        for target,(_,value) in zip(targets,outputs):
            target.write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8',newline='\n')
    except (ValueError,KeyError,OSError,TypeError,AttributeError,SystemExit) as failure:
        print(f'Candidate promotion refused: {failure}',file=sys.stderr); return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())

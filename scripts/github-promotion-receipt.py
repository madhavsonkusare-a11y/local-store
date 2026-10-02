#!/usr/bin/env python3
"""Audit an existing reviewed candidate -> current proof -> approved offering.

This emits evidence only. It never edits a template, accepts a source URL as an
executable instruction, installs an app, or changes the approved offering list.
New promotions still require an explicit reviewed manifest and meaningful proof.
"""
import argparse
from hashlib import sha256
import importlib.util
import json
from pathlib import Path
import re
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('v1_qualified', ROOT / 'scripts/check-v1-qualified.py')
proofs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proofs)


def digest(data):
    return sha256(data).hexdigest()


def archive_file(bundle, relative):
    # Exactly one archive root plus the pinned relative path. Never extract.
    matches = [item for item in bundle.infolist()
               if item.filename.split('/', 1)[-1] == relative]
    if len(matches) != 1 or matches[0].is_dir() or matches[0].file_size > 1024 * 1024:
        raise ValueError('source archive definition is missing, duplicated or oversized')
    return bundle.read(matches[0])


def build(root, app_id, facts, now):
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,79}', app_id):
        raise ValueError('invalid offering ID')
    manifest_path = f'src/templates/{app_id}.json'
    manifest_bytes = proofs.file_bytes(root, manifest_path, 'src/templates/')
    manifest = json.loads(manifest_bytes)
    if manifest.get('id') != app_id or manifest.get('promotion', {}).get('state') != 'approved':
        raise ValueError('explicit approved template review required')
    reason = manifest['promotion'].get('reason', '')
    if not isinstance(reason, str) or len(reason) < 40:
        raise ValueError('promotion review must explain its decision')
    origin = manifest.get('origin', {})
    source = origin.get('importer')
    if source not in ('runtipi', 'caprover'):
        raise ValueError('unsupported pinned source')
    pins = proofs.read_json(root, 'catalog/import-audit-sources.json')
    pin = pins[source]
    if any(origin.get(key) != pin.get(key) for key in ('revision', 'repository', 'license')):
        raise ValueError('reviewed origin differs from pinned source archive')
    queue = proofs.read_json(root, 'catalog/candidate-queue.json')
    rows = [row for row in queue['candidates'] if row.get('source') == source and row.get('id') == app_id]
    if len(rows) != 1:
        raise ValueError('exact candidate definition required')
    candidate = rows[0]
    expected_repository = 'github:' + manifest['source_url'].removeprefix('https://github.com/').lower()
    if candidate.get('identity') != expected_repository or candidate.get('identity_status') != 'reviewed_repository_match':
        raise ValueError('explicit repository identity review required')
    if not candidate.get('importable') or candidate.get('maintenance_status', '').startswith('withhold_'):
        raise ValueError('structural or maintenance blocker remains')
    provenance = candidate.get('provenance', {})
    if any(provenance.get(key) != origin.get(key) for key in ('revision', 'repository', 'path')) or provenance.get('archive_sha256') != pin.get('sha256'):
        raise ValueError('candidate and reviewed source provenance differ')
    archive = proofs.file_bytes(root, f".cache/catalog/{source}-{pin['revision']}.zip", '.cache/catalog/')
    proofs.require_digest(digest(archive), pin['sha256'], 'source archive')
    with zipfile.ZipFile(root / f".cache/catalog/{source}-{pin['revision']}.zip") as bundle:
        definition = archive_file(bundle, origin['path'])
        # Runtipi keeps JSON text verbatim. CapRover review uses normalized JSON
        # from YAML, so compare the parsed structure without interpreting hooks.
        if source == 'runtipi':
            if definition.decode('utf-8').strip() != manifest['definition'].strip():
                raise ValueError('reviewed definition bytes differ from pinned archive')
        else:
            import yaml
            if yaml.safe_load(definition) != json.loads(manifest['definition']):
                raise ValueError('reviewed mapping differs from pinned archive')
        config = manifest.get('config')
        if config and archive_file(bundle, config['path']).decode('utf-8').strip() != config['content'].strip():
            raise ValueError('reviewed configuration differs from pinned archive')
    ledger = proofs.read_json(root, 'catalog/v1-qualified-apps.json')
    entries = [entry for entry in ledger['apps'] if entry.get('id') == app_id]
    if len(entries) != 1:
        raise ValueError('current curated Windows task evidence required')
    roster = {row['id']: row for row in proofs.read_json(root, 'catalog/v1-roster.json')['apps'] if row.get('cohort') == 'existing_offering'}
    entry = entries[0]
    proofs.validate_entry(root, entry, roster, now)
    evidence = proofs.read_json(root, entry['evidence'])
    if facts.get('schema_version') != 1 or facts.get('app') != app_id or not isinstance(facts.get('observed_at_unix'), int) or not 0 <= now - facts['observed_at_unix'] <= 900:
        raise ValueError('fresh Rust launcher facts required (15 minute maximum)')
    resolution = facts.get('source_resolution', {})
    if resolution.get('kind') != 'approved_match' or resolution.get('offering_id') != app_id or resolution.get('repository') != expected_repository.replace('github:', 'https://github.com/', 1):
        raise ValueError('repository must resolve uniquely to the approved offering')
    for flag in ('current_evidence', 'task_verified', 'lifecycle_proven'):
        if facts.get('launch_readiness', {}).get(flag) is not True:
            raise ValueError('launcher exact-input proof projection refuses this app')
    proofs.require_digest(facts.get('plan_sha256'), evidence['identity']['plan_sha256'], 'normalized offering plan')
    proofs.require_digest(digest(manifest['definition'].encode()), facts.get('imported_definition_sha256'), 'reviewed importer definition')
    inspection = facts.get('github_inspection')
    if inspection:
        live = inspection.get('resolution', {})
        if inspection.get('canonical_repository') != resolution['repository'] or live.get('kind') != 'approved_match' or live.get('offering_id') != app_id:
            raise ValueError('live canonical repository differs from reviewed offering')
    return {
        'schema_version': 1, 'recorded_at_unix': now, 'app': app_id,
        'scope': 'existing reviewed promotion audit; no automatic approval or new offering',
        'source': {'candidate': f'{source}:{app_id}', 'repository': origin['repository'], 'revision': origin['revision'], 'definition_path': origin['path'], 'archive_sha256': pin['sha256'], 'definition_sha256': digest(definition)},
        'review': {'manifest': manifest_path, 'manifest_sha256': digest(manifest_bytes), 'decision': manifest['promotion'], 'license': origin['license']},
        'qualification': entry, 'normalized_plan_sha256': facts['plan_sha256'],
        'approved_repository_match': resolution,
        'live_github_inspection': inspection,
        'gates': ['checksum-pinned archive bytes', 'reviewed exact definition and configuration', 'explicit source identity and promotion decision', 'current managed Windows task and lifecycle proof', 'same normalized launcher install plan', 'exact approved repository lookup'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', required=True)
    parser.add_argument('--facts', required=True, help='JSON emitted by the Rust github_review_facts example')
    parser.add_argument('--output', help='optional new evidence receipt under docs/evidence/')
    args = parser.parse_args()
    try:
        facts = json.loads(Path(args.facts).read_text(encoding='utf-8-sig'))
        receipt = build(ROOT, args.app, facts, int(time.time()))
        encoded = json.dumps(receipt, indent=2) + '\n'
        if args.output:
            target = (ROOT / args.output).resolve()
            if not target.is_relative_to((ROOT / 'docs/evidence').resolve()) or target.exists():
                raise ValueError('receipt output must be a new file under docs/evidence/')
            target.write_text(encoded, encoding='utf-8', newline='\n')
        else:
            print(encoded, end='')
    except (ValueError, KeyError, OSError, zipfile.BadZipFile) as failure:
        print(f'Promotion receipt refused: {failure}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())

"""Build an exact-payload notices and source companion without Docker or WSL.

Uses the independent verifier's reviewed Ubuntu trust anchors and installed gpgv.
Public source downloads are streamed, bounded and checked against signed Sources.
All source identities are collected conservatively; this is not legal approval.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import importlib.util
import io
import json
import lzma
from pathlib import Path, PurePosixPath
import re
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / '.cache/engine-source-companion'
EVIDENCE = ROOT / 'docs/evidence/engine-source-companion-2026-10-03.json'
BUILD = ROOT / '.cache/engine/build-644d5c2b5ccf44abad695ac79b06d8d2'
PAYLOAD = 'ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32'
MAX_FILE = 180 * 1024 * 1024
MAX_TOTAL = 1600 * 1024 * 1024
spec = importlib.util.spec_from_file_location('provenance', ROOT / 'scripts/verify-engine-provenance.py')
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)

def charge_budget(budget, amount):
    if budget:
        with budget['lock']:
            if budget['used'] + amount > budget['maximum']:
                raise ValueError('aggregate source download/cache budget exceeded')
            budget['used'] += amount


def acquire(url, path, bound, fetch, expected=None, budget=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    cached = path.is_file()
    if not cached:
        if not fetch:
            raise ValueError('missing capture: ' + str(path))
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        request = urllib.request.Request(url, headers={'User-Agent': 'dockwrap-source-review/1'})
        temp = path.with_suffix(path.suffix + '.partial')
        try:
            with opener.open(request, timeout=60) as response, temp.open('wb') as output:
                if not response.url.startswith(('https://snapshot.ubuntu.com/', 'https://archive.ubuntu.com/', 'https://raw.githubusercontent.com/', 'https://codeload.github.com/', 'https://api.github.com/', 'https://proxy.golang.org/', 'https://storage.googleapis.com/')):
                    raise ValueError('unexpected source origin redirect')
                size = 0
                while chunk := response.read(1024 * 1024):
                    size += len(chunk)
                    if size > bound:
                        raise ValueError('download exceeds bound')
                    charge_budget(budget, len(chunk))
                    output.write(chunk)
            if expected and (p.file_sha(temp), temp.stat().st_size) != expected:
                raise ValueError('download differs from signed source digest/size')
            temp.replace(path)
        finally:
            temp.unlink(missing_ok=True)
    if path.stat().st_size > bound or (expected and (p.file_sha(path), path.stat().st_size) != expected):
        raise ValueError('cached source differs from bound/digest/size: ' + str(path))
    if cached:
        charge_budget(budget, path.stat().st_size)
    return {'url': url, 'cache_path': path.relative_to(ROOT).as_posix(), 'sha256': p.file_sha(path), 'size_bytes': path.stat().st_size}

def source_records(fetch):
    inventory = json.loads((ROOT / 'docs/evidence/engine-independent-provenance-2026-10-02.json').read_text())['package_source_notice_inventory']
    wanted = {(x['source'], x['source_version']) for x in inventory if not x['package'].startswith(('docker-', 'containerd.'))}
    metadata, notices, links = p.root_metadata(BUILD / 'rootfs.tar')
    keyring = CACHE / 'ubuntu-keyring.gpg'
    keyring.write_bytes(metadata['usr/share/keyrings/ubuntu-archive-keyring.gpg'])
    indexes = p.members(BUILD / 'ubuntu-indexes.tar', p.MAX_INDEX)
    selected, captures = {}, []
    # The earlier snapshot covers inherited September 7 base identities which
    # have since been superseded; it is verified through the same Ubuntu key.
    for stamp in ('20260913T120000Z', '20260907T120000Z'):
        for suite in ('noble', 'noble-updates', 'noble-security'):
            prefix = 'https://snapshot.ubuntu.com/ubuntu/' + stamp + '/'
            cached_name = 'lists/snapshot.ubuntu.com_ubuntu_' + stamp + '_dists_' + suite + '_InRelease'
            release_path = CACHE / 'indexes' / (stamp + '-' + suite + '.InRelease')
            if cached_name in indexes:
                release_path.parent.mkdir(parents=True, exist_ok=True)
                release_path.write_bytes(indexes[cached_name])
            release_capture = acquire(prefix + 'dists/' + suite + '/InRelease', release_path, 1024*1024, fetch)
            release, hashes, signature = p.verify_signature(release_path.read_bytes(), keyring, 'source-' + stamp + '-' + suite, p.UBUNTU_KEYS)
            for component in ('main', 'universe'):
                relative = component + '/source/Sources.xz'
                digest, length = hashes[relative]
                source_path = CACHE / 'indexes' / (stamp + '-' + suite + '-' + component + '.Sources.xz')
                capture = acquire(prefix + 'dists/' + suite + '/' + relative, source_path, 32*1024*1024, fetch, (digest, length))
                plain_digest, plain_length = hashes[component + '/source/Sources']
                if plain_length > p.MAX_INDEX:
                    raise ValueError('signed uncompressed Sources size exceeds bound')
                with lzma.LZMAFile(source_path) as stream:
                    decoded = stream.read(plain_length + 1)
                if (p.sha(decoded), len(decoded)) != (plain_digest, plain_length):
                    raise ValueError('Sources differs from signed uncompressed hash')
                capture.update({'signature': signature, 'inrelease': release_capture, 'signed_path': relative, 'uncompressed_sha256': plain_digest, 'uncompressed_size_bytes': plain_length})
                captures.append(capture)
                for record in p.paragraphs(decoded):
                    identity = (record['Package'], record['Version'])
                    if identity in wanted and identity not in selected:
                        files = []
                        for line in record['Checksums-Sha256'].splitlines():
                            if not line.strip(): continue
                            artifact_digest, size, name = line.split()
                            if not re.fullmatch(r'[0-9a-f]{64}', artifact_digest) or int(size) > MAX_FILE or PurePosixPath(name).name != name:
                                raise ValueError('unsafe/oversized signed source artifact')
                            directory = record['Directory']
                            p.safe_name(directory)
                            files.append({'name': name, 'sha256': artifact_digest, 'size_bytes': int(size), 'url': prefix + directory + '/' + name})
                        selected[identity] = {'source': identity[0], 'version': identity[1], 'packages': sorted(x['package'] for x in inventory if (x['source'], x['source_version']) == identity), 'sources_index_sha256': digest, 'sources_index_cache_path': capture['cache_path'], 'artifacts': files}
        if wanted <= selected.keys(): break
    return inventory, notices, selected, sorted(wanted - selected.keys()), captures

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fetch-indexes', action='store_true')
    parser.add_argument('--fetch-sources', action='store_true')
    parser.add_argument('--plan-only', action='store_true')
    args = parser.parse_args()
    CACHE.mkdir(parents=True, exist_ok=True)
    if p.file_sha(BUILD / 'rootfs.tar') != PAYLOAD:
        raise ValueError('selected rootfs has changed')
    inventory, notices, records, missing, captures = source_records(args.fetch_indexes)
    planned = sum(a['size_bytes'] for r in records.values() for a in r['artifacts'])
    if planned > MAX_TOTAL: raise ValueError('source plan exceeds total budget')
    print(json.dumps({'ubuntu_source_identities': len(records), 'missing': missing, 'planned_source_bytes': planned}), flush=True)
    futures = {}
    pool = ThreadPoolExecutor(max_workers=4)
    if args.fetch_sources and not args.plan_only:
        for record in records.values():
            for artifact in record['artifacts']:
                path = CACHE / 'ubuntu' / artifact['name']
                futures[artifact['name']] = pool.submit(acquire, artifact['url'], path, artifact['size_bytes'], True, (artifact['sha256'], artifact['size_bytes']))
    rows = []
    for record in sorted(records.values(), key=lambda r: (r['source'], r['version'])):
        record = dict(record)
        retained = []
        for artifact in record['artifacts']:
            path = CACHE / 'ubuntu' / artifact['name']
            if args.plan_only:
                retained.append(dict(artifact, retained=False))
            else:
                capture = futures[artifact['name']].result() if artifact['name'] in futures else acquire(artifact['url'], path, artifact['size_bytes'], False, (artifact['sha256'], artifact['size_bytes']))
                retained.append(dict(artifact, **{'cache_path': capture['cache_path'], 'retained': True}))
        record['artifacts'] = retained
        rows.append(record)
        print(record['source'] + ' ' + record['version'] + ': ' + str(sum(a['retained'] for a in retained)) + '/' + str(len(retained)), flush=True)
    pool.shutdown(wait=True)
    # Include ALL canonical packaged copyright texts AND full common licenses.
    notice_path = CACHE / 'notices.tar'
    with tarfile.open(notice_path, 'w') as archive:
        for name, data in sorted(notices.items()):
            info = tarfile.TarInfo(name)
            info.size, info.mtime, info.mode = len(data), 0, 0o644
            archive.addfile(info, io.BytesIO(data))
    result = {'schema_version': 1, 'recorded_at': datetime.now(timezone.utc).isoformat(), 'rootfs_sha256': PAYLOAD, 'package_count': len(inventory), 'distribution_approved': False, 'source_policy': 'Conservatively retain exact Ubuntu source artifacts for every Ubuntu package, including permissively licensed material; automated license hints do not decide obligations.', 'ubuntu_sources': rows, 'missing_ubuntu_source_identities': missing, 'verified_signed_sources_indexes': captures, 'planned_ubuntu_source_bytes': planned, 'all_ubuntu_sources_retained': not missing and all(a['retained'] for r in rows for a in r['artifacts']), 'notice_bundle': {'cache_path': notice_path.relative_to(ROOT).as_posix(), 'sha256': p.file_sha(notice_path), 'size_bytes': notice_path.stat().st_size, 'copyright_package_count': sum(x['copyright_present'] for x in inventory), 'common_license_count': sum(name.startswith('usr/share/common-licenses/') for name in notices)}, 'limitations': ['Docker exact upstream/dependency notice mappings are a separate companion collection.', 'Source publication and continued equivalent access beside the binary must be completed before binary distribution.', 'This evidence is not a legal certification, binary reproducibility claim or clean Windows acceptance.']}
    EVIDENCE.write_text(json.dumps(result, indent=2) + '\n')
    print('Evidence: ' + str(EVIDENCE), flush=True)

if __name__ == '__main__': main()

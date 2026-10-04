#!/usr/bin/env python3
"""Inspect the retained private NSIS bundle without executing its installer.

Uses the official portable 7-Zip 26.03 static Linux reader in the existing
owned WSL engine. No package installation, distro changes or binary publication.
Download/source identity is documented in the inspection receipt.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
READER_SHA256 = 'eab4c8d7f193e3d6d3237370bbcaa879a160a3f1dc82202207e27baeab79b6ac'
PACKAGE_SHA256 = 'dc99eff5008f1ab79bd7084c68513701547a808a89502bf4133683535ab3c695'
PREFIX = ['wsl.exe', '--distribution', 'local-store-engine-v1', '--user', 'root', '--exec']


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def linux(path):
    path = path.resolve()
    if path.drive.lower() != 'd:' or not path.is_relative_to(ROOT.resolve()):
        raise RuntimeError('Review files must remain on the workspace drive')
    return '/mnt/d/' + '/'.join(path.parts[1:])


def run(args):
    result = subprocess.run(args, capture_output=True, text=True, encoding='utf-8', timeout=90)
    if result.returncode or result.stderr.strip():
        raise RuntimeError('Archive reader command failed; inspect its isolated review directory')
    if len(result.stdout) > 1024 * 1024:
        raise RuntimeError('Archive diagnostic exceeded its bound')
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', action='store_true')
    args = parser.parse_args()
    if not args.run or os.name != 'nt':
        parser.error('Use --run on the existing Windows/owned-WSL host')
    cache = ROOT / '.cache/installer-extraction-2026-10-04'
    if not cache.resolve().is_relative_to(ROOT.resolve()):
        raise RuntimeError('Review cache escaped the workspace')
    for path in [cache, cache.parent]:
        if path.is_symlink() or path.is_junction():
            raise RuntimeError('Linked review cache refused')
    tool = cache / '7zzs'
    package = cache / '7z2603-linux-x64.tar.xz'
    if tool.is_symlink() or package.is_symlink():
        raise RuntimeError('Linked tool files refused')
    if sha(tool) != READER_SHA256 or sha(package) != PACKAGE_SHA256:
        raise RuntimeError('Portable reader differs from the reviewed official release')
    native = Path(os.environ['LOCALAPPDATA']) / 'local-store/engine/wsl-state'
    names = ['bootstrap.json', 'ownership-token', 'selected-engine.json', 'engine-selection-configured']
    original = {name: (native / name).read_bytes() for name in names}
    journal = json.loads(original['bootstrap.json'])
    if journal.get('state') != 'verified' or journal.get('distro') != PREFIX[2] or original['ownership-token'] != journal.get('ownership_token', '').encode():
        raise RuntimeError('Existing engine ownership is not verified')
    actual_token = run([*PREFIX, '/bin/cat', '/usr/share/local-store/ownership-token']).strip()
    if actual_token != original['ownership-token'].decode('utf-8'):
        raise RuntimeError('In-distro ownership marker differs')
    bundle = json.loads((ROOT / 'docs/evidence/windows-private-bundle-2026-10-04.json').read_text())
    artifact = next(row for row in bundle['artifacts'] if row['path'].endswith('setup.exe'))
    installer = ROOT / artifact['path']
    if sha(installer) != artifact['sha256'] or installer.stat().st_size != artifact['size_bytes']:
        raise RuntimeError('Private installer differs from its retained build receipt')
    run([*PREFIX, '/bin/chmod', '700', linux(tool)])
    listing = run([*PREFIX, linux(tool), 'l', '-slt', '--', linux(installer)])
    if 'Type = Nsis' not in listing or '\n----------\n' not in listing:
        raise RuntimeError('Expected an NSIS archive')
    entries = []
    for block in listing.split('\n----------\n', 1)[1].strip().split('\n\n'):
        fields = dict(line.split(' = ', 1) for line in block.splitlines() if ' = ' in line)
        name = fields.get('Path', '')
        path = PurePosixPath(name)
        if not name or '\\' in name or ':' in name or path.is_absolute() or any(part in ['', '.', '..'] for part in name.split('/')) or 'Symbolic Link' in fields or 'Hard Link' in fields:
            raise RuntimeError('Unsafe archive entry refused')
        size = fields.get('Size', '')
        # NSIS reconstructs its uninstaller from a stub plus patches; the
        # archive listing cannot give its resulting size. Allow only that
        # exact generated entry, then bound its actual extracted size below.
        if not size and name != 'uninstall.exe':
            raise RuntimeError('Unspecified archive entry size refused')
        entries.append({'path': name, 'size_bytes': int(size) if size else None})
    if not entries or len(entries) > 256 or len({row['path'].casefold() for row in entries}) != len(entries) or sum(row['size_bytes'] or 5 * 1024 * 1024 for row in entries) > 150 * 1024 * 1024:
        raise RuntimeError('Archive entry count, aliases or size refused')
    output = Path(tempfile.mkdtemp(prefix='extracted-', dir=cache)).resolve()
    if output.parent != cache.resolve() or output.is_symlink():
        raise RuntimeError('Extraction escaped its owned review cache')
    run([*PREFIX, linux(tool), 'x', '-y', '-o' + linux(output), '--', linux(installer)])
    size_discrepancies = []
    for row in entries:
        extracted = output / row['path']
        if not extracted.is_file() or extracted.is_symlink() or not extracted.resolve().is_relative_to(output):
            raise RuntimeError('Extracted entry identity/size mismatch')
        size = extracted.stat().st_size
        if row['size_bytes'] is not None and size != row['size_bytes']:
            # Archive readers can reconstruct NSIS helper/stub material rather
            # than copy a literal member. Never exempt product resources or
            # entrypoints. Record the discrepancy, not an exact plugin claim.
            if not row['path'].startswith('$PLUGINSDIR/') or size > 5 * 1024 * 1024:
                raise RuntimeError('Extracted entry size mismatch or bound exceeded')
            size_discrepancies.append({'path': row['path'], 'listed_size_bytes': row['size_bytes'], 'extracted_size_bytes': size})
        if row['size_bytes'] is None and size > 5 * 1024 * 1024:
            raise RuntimeError('Generated stub size bound exceeded')
        row['generated_nsis_stub'] = row['size_bytes'] is None
        row['size_bytes'] = size
        row['sha256'] = sha(extracted)
    spec = importlib.util.spec_from_file_location('notices', ROOT / 'scripts/check-packaged-notices.py')
    notices = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(notices)
    result = notices.verify(output)
    if not result['passed']:
        raise RuntimeError('Extracted notice bytes failed: ' + '; '.join(result['errors']))
    resources = json.loads((ROOT / 'tauri.conf.json').read_text())['bundle']['resources']
    resource_names = sorted({p.relative_to(ROOT).as_posix() for pattern in resources for p in ROOT.glob(pattern) if p.is_file()})
    for name in resource_names:
        if not (output / name).is_file() or sha(output / name) != sha(ROOT / name):
            raise RuntimeError('Extracted resource differs: ' + name)
    entrypoints = []
    for name in ['local-store.exe', 'local-store-mcp.exe']:
        expected = next(row for row in bundle['artifacts'] if row['path'] == 'target/release/' + name)
        source = ROOT / expected['path']
        if sha(source) != expected['sha256']:
            raise RuntimeError('Retained entrypoint changed since bundle build')
        wanted = source.read_bytes()
        patch = None
        if name == 'local-store.exe':
            # Official tauri-cli 2.11.5 bundle.rs patches only this marker for
            # NSIS, then restores the original build output. Require the full
            # extracted binary to equal that one exact transformation.
            marker = b'__TAURI_BUNDLE_TYPE_VAR_UNK'
            if wanted.count(marker) != 1:
                raise RuntimeError('Expected exactly one Tauri bundle type marker')
            patch = {'offset': wanted.index(marker), 'from': marker.decode(), 'to': '__TAURI_BUNDLE_TYPE_VAR_NSS',
                'source_url': 'https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.5/crates/tauri-bundler/src/bundle.rs',
                'source_sha256': 'a4622ce32d88b99e8a785d57cfd96ebc2aa99d066a3c7bb17c0634a8a943aa3d'}
            wanted = wanted.replace(marker, b'__TAURI_BUNDLE_TYPE_VAR_NSS', 1)
        if (output / name).read_bytes() != wanted:
            raise RuntimeError('Extracted executable differs: ' + name)
        entrypoints.append({'path': name, 'retained_binary_sha256': expected['sha256'], 'extracted_sha256': sha(output / name), 'exact_framework_patch': patch})
    if any((native / name).read_bytes() != value for name, value in original.items()):
        raise RuntimeError('Native engine identity changed')
    receipt = {'schema_version': 1, 'recorded_at': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), 'passed': True,
        'scope': 'Actual retained private installer extraction and exact resources; no installer execution, installation, clean-host or public delivery claim',
        'installer': artifact, 'reader': {'version': '26.03', 'url': 'https://github.com/ip7z/7zip/releases/download/26.03/7z2603-linux-x64.tar.xz', 'package_sha256': PACKAGE_SHA256, 'tool_sha256': READER_SHA256, 'digest_origin': 'Official GitHub release asset digest; not a signature'},
        'script_sha256': sha(Path(__file__)), 'extracted_file_count': len(entries), 'configured_resource_count': len(resource_names), 'exact_notice_file_count': result['configured_notice_count'],
        'entrypoints_match_exact_framework_transformation': True, 'entrypoints': entrypoints, 'native_engine_identity_unchanged': True, 'installer_executed': False, 'published': False,
        'extraction_path': output.relative_to(ROOT).as_posix(), 'entries': entries, 'archive_size_discrepancies': size_discrepancies,
        'limits': ['Archive extraction does not test installer destination, registry, shortcuts, prerequisite downloads or uninstaller behavior', 'NSIS plugin/stub reconstruction is not certified; reader listing/extraction size discrepancies remain explicitly recorded', 'Existing WSL-ready host, not clean Windows', 'Launcher bundle excludes engine payload and source companions']}
    target = ROOT / ('docs/evidence/windows-private-installer-extraction-' + time.strftime('%Y-%m-%d-%H%M%S', time.gmtime()) + '.json')
    with target.open('x', encoding='utf-8', newline='\n') as handle:
        handle.write(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({'passed': True, 'files': len(entries), 'resources': len(resource_names), 'notices': result['configured_notice_count'], 'receipt': str(target)}))


if __name__ == '__main__':
    main()

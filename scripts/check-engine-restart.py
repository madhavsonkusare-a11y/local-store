#!/usr/bin/env python3
"""Actual owner-worker stop/restart and singleton proof in an isolated profile.

No distro import/unregister, host sleep, app mutation, or native-user state edit.
Reserve the serial engine proof lane. Uses the reviewed bounded subprocess
helper; exact fixture selection revocation is the cleanup authority.
"""
from __future__ import annotations
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',action='store_true')
    if not parser.parse_args().run or os.name!='nt':
        parser.error('Use --run on Windows after reserving the serial engine lane')
    root=Path(__file__).resolve().parents[1]
    helper_path=root/'scripts/check-managed-supervisor.py'
    spec=importlib.util.spec_from_file_location('supervisor_proof',helper_path)
    helper=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    binary=root/'target/release/local-store.exe'
    native=Path(os.environ['LOCALAPPDATA'])/'local-store/engine/wsl-state'
    original={name:helper.bounded(native/name,16384) for name in helper.STATE_FILES}
    journal=json.loads(original['bootstrap.json'])
    if journal.get('state')!='verified' or journal.get('distro')!=helper.DISTRO or original['ownership-token']!=journal.get('ownership_token','').encode():
        raise RuntimeError('Current native engine ownership is not verified')
    cache=(root/'.cache/engine').resolve()
    fixture=Path(tempfile.mkdtemp(prefix='restart-proof-',dir=cache)).resolve()
    if fixture.parent!=cache:
        raise RuntimeError('Fixture escaped owned cache')
    state=fixture/'local/local-store/engine/wsl-state'
    state.mkdir(parents=True)
    for name,value in original.items(): (state/name).write_bytes(value)
    env=dict(os.environ,LOCALAPPDATA=str(fixture/'local'),APPDATA=str(fixture/'roaming'),XDG_CONFIG_HOME=str(fixture/'xdg'))
    env.pop('WSLENV',None)
    env.pop('LOCAL_STORE_ENGINE_SUPERVISOR_PROCESS_ONLY',None)
    def status(): return helper.product_json(binary,['engine','supervisor-status'],env)
    def wait_stopped():
        deadline=time.monotonic()+40
        while time.monotonic()<deadline:
            if status() is None: return
            time.sleep(.5)
        raise RuntimeError('Fixture worker did not acknowledge stop')
    receipt=None
    crash_worker=None
    try:
        before=helper.containers(env)
        if status() is not None: raise RuntimeError('Fresh fixture worker already exists')
        if not helper.product_json(binary,['doctor','--json'],env).get('ready'):
            raise RuntimeError('Owned engine is not ready')
        first=status()
        if not first or not first['running']: raise RuntimeError('Worker did not start')
        stopped=helper.run([str(binary),'engine','stop-supervisor'],env)
        if stopped.returncode: raise RuntimeError('Owner-only worker stop failed')
        wait_stopped()
        # The normal launcher clears a completed stop request before spawning.
        # Reproduce only that preparation in this private, stopped fixture.
        (state/'supervisor-stop').unlink(missing_ok=True)
        # Simulate a crash only through a child handle created by this proof.
        # Never open/kill an external PID from a diagnostic status file.
        crash_worker=subprocess.Popen([str(binary),'engine','supervise'],env=env,
            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW)
        deadline=time.monotonic()+40
        while time.monotonic()<deadline:
            crash_status=status()
            if crash_status and crash_status['process_id']==crash_worker.pid: break
            if crash_worker.poll() is not None: raise RuntimeError('Owned crash fixture exited before acknowledgment')
            time.sleep(.5)
        else: raise RuntimeError('Owned crash fixture did not acknowledge startup')
        crash_worker.kill()
        crash_worker.wait(timeout=5)
        wait_stopped()
        # Concurrent actual launcher calls must share one acknowledged worker.
        def launch(_): return helper.product_json(binary,['doctor','--json'],env)
        with ThreadPoolExecutor(max_workers=3) as pool:
            reports=list(pool.map(launch,range(3)))
        if not all(report.get('ready') for report in reports):
            raise RuntimeError('Concurrent restart did not preserve ready engine')
        restarted=status()
        if not restarted or not restarted['running']:
            raise RuntimeError('Explicit restart did not acknowledge a worker')
        if restarted['process_id']==first['process_id']:
            raise RuntimeError('Restart did not replace the exited fixture worker')
        observed=[status() for _ in range(3)]
        if any(not row or row['process_id']!=restarted['process_id'] for row in observed):
            raise RuntimeError('Concurrent launch did not retain a singleton worker')
        preview=helper.product_json(binary,['engine','setup-preview'],env)
        if preview.get('payload')!='verified_development':
            raise RuntimeError('Actual packaged payload preview failed')
        after=helper.containers(env)
        if after!=before: raise RuntimeError('Existing containers changed')
        if any(helper.bounded(native/name,16384)!=value for name,value in original.items()):
            raise RuntimeError('Native owner state changed')
        receipt={'schema_version':1,'passed':True,'proof':'actual_owner_worker_stop_restart_singleton',
            'recorded_at':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),
            'launcher_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
            'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'helper_sha256':hashlib.sha256(helper_path.read_bytes()).hexdigest(),
            'explicit_stop_acknowledged':True,'explicit_restart_acknowledged':True,
            'exact_owned_child_crash_recovered':True,
            'concurrent_launcher_count':3,'same_restarted_singleton_observed':True,
            'existing_container_count':len(before),'container_identities_start_times_unchanged':True,
            'native_engine_state_unchanged':True,'packaged_payload_preview_verified':True,
            'supervisor_source_sha256':hashlib.sha256((root/'src/runtime/engine/wsl/supervisor.rs').read_bytes()).hexdigest(),
            'limits':['Existing WSL-ready host, not clean Windows','Crash of a proof-created worker handle, not arbitrary launcher/app processes',
                'No native WebView close or sleep/wake','No production engine removal']}
    finally:
        if crash_worker is not None and crash_worker.poll() is None:
            crash_worker.kill()
            crash_worker.wait(timeout=5)
        (state/'selected-engine.json').unlink(missing_ok=True)
        wait_stopped()
        if fixture.parent!=cache or not fixture.name.startswith('restart-proof-'):
            raise RuntimeError('Refusing unsafe fixture cleanup')
        shutil.rmtree(fixture)
    if receipt:
        receipt['fixture_selection_revocation_stopped_worker']=True
        output=root/'docs/evidence/windows-engine-restart-2026-10-03.json'
        output.write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
        print(json.dumps({'passed':True,'receipt':str(output)}))


if __name__=='__main__': main()

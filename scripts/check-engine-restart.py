#!/usr/bin/env python3
"""Actual owner-worker stop/restart and singleton proof in an isolated profile.

No distro import/unregister, host sleep, or native-user state edit.
Optional --with-memos installs only a temporary fixture using the product CLI.
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
    parser.add_argument('--with-memos',action='store_true')
    args=parser.parse_args()
    if not args.run or os.name!='nt':
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
    installation_attempted=False
    workload=None
    output=root/('docs/evidence/windows-engine-workload-restart-'+time.strftime('%Y-%m-%d')+'.json' if args.with_memos else 'docs/evidence/windows-engine-restart-'+time.strftime('%Y-%m-%d')+'.json')
    if output.exists():
        output=output.with_name(output.stem+'-'+time.strftime('%H%M%S',time.gmtime())+'.json')
    prefix=['wsl.exe','--distribution',helper.DISTRO,'--user','root','--exec','/usr/bin/env','-i','PATH=/usr/sbin:/usr/bin:/sbin:/bin','/usr/bin/docker','--host','unix:///var/run/docker.sock']
    def require_empty_project():
        named=helper.run([*prefix,'ps','--all','--quiet','--filter','name=^/local-store-memos$'],env)
        if named.returncode or named.stdout.strip() or named.stderr.strip():
            raise RuntimeError('Memos container name is occupied or its inventory is uncertain')
        for command in [['ps','--all','--quiet'],['volume','ls','--quiet'],['network','ls','--quiet']]:
            found=helper.run([*prefix,*command,'--filter','label=com.docker.compose.project=local-store-memos'],env)
            if found.returncode or found.stdout.strip() or found.stderr.strip():
                raise RuntimeError('Memos project is occupied or its inventory is uncertain')
    def product_command(command):
        if helper.run([str(binary),*command],env).returncode:
            raise RuntimeError('Fixture product command failed: '+command[0])
    def probe(phase):
        if helper.run(['node',str(root/'scripts/memos-content-probe.mjs'),phase,workload['address'],str(fixture/'memo-proof.json')],env).returncode:
            raise RuntimeError('Exact private memo task failed')
    try:
        before=helper.containers(env)
        if status() is not None: raise RuntimeError('Fresh fixture worker already exists')
        if not helper.product_json(binary,['doctor','--json'],env).get('ready'):
            raise RuntimeError('Owned engine is not ready')
        first=status()
        if not first or not first['running']: raise RuntimeError('Worker did not start')
        if args.with_memos:
            # The product currently uses a fixed recipe-derived Compose name.
            # Refuse even stopped containers, volumes or networks in that name;
            # never adopt or clean up a bystander's project.
            require_empty_project()
            installation_attempted=True
            product_command(['install','memos'])
            registry=json.loads(helper.bounded(fixture/'roaming/local-store/registry-v2.json',65536))
            apps=registry.get('apps',[])
            if len(apps)!=1 or apps[0].get('id')!='memos':
                raise RuntimeError('Fixture registry did not contain exactly its Memos app')
            app=apps[0]
            workload={'address':app['launch_url'],'app_id':'memos'}
            probe('first-use')
            running=helper.containers(env)
            owned={key:value for key,value in running.items() if key not in before}
            if len(owned)!=1 or not all(value.endswith('|running') for value in owned.values()):
                raise RuntimeError('Expected one running fixture container')
            workload['container_count']=len(owned)
            workload['before_recovery']=owned
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
        if workload:
            probe('verify')
            current=helper.containers(env)
            if any(current.get(key)!=value for key,value in workload['before_recovery'].items()):
                raise RuntimeError('Fixture app restarted or disappeared during worker recovery')
            workload['private_memo_identical_after_worker_crash']=True
            workload['container_identity_and_start_time_unchanged']=True
            product_command(['uninstall','memos','--delete-data'])
            installation_attempted=False
            require_empty_project()
            workload['production_uninstall_cleanup_passed']=True
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
        if workload:
            # Do not publish fixture private content, account/password or IDs.
            receipt['proof']='actual_owner_worker_restart_with_managed_memos_task'
            receipt['workload']={key:value for key,value in workload.items() if key not in ['address','before_recovery']}
            receipt['probe_sha256']=hashlib.sha256((root/'scripts/memos-content-probe.mjs').read_bytes()).hexdigest()
            receipt['recipe_sha256']=hashlib.sha256((root/'src/recipes/memos.json').read_bytes()).hexdigest()
    finally:
        if crash_worker is not None and crash_worker.poll() is None:
            crash_worker.kill()
            crash_worker.wait(timeout=5)
        try:
            if installation_attempted:
                # Only the isolated profile and previously empty project are in scope.
                # Leave the fixture for review if normal ownership-safe cleanup fails.
                registry_file=fixture/'roaming/local-store/registry-v2.json'
                if registry_file.exists():
                    product_command(['uninstall','memos','--delete-data'])
                else:
                    product_command(['recover','memos','--delete-data'])
                require_empty_project()
        finally:
            # Revoke even when app cleanup refuses; retain its files for review.
            (state/'selected-engine.json').unlink(missing_ok=True)
            wait_stopped()
        if fixture.parent!=cache or not fixture.name.startswith('restart-proof-'):
            raise RuntimeError('Refusing unsafe fixture cleanup')
        shutil.rmtree(fixture)
    if receipt:
        receipt['fixture_selection_revocation_stopped_worker']=True
        with output.open('x',encoding='utf-8',newline='\n') as handle:
            handle.write(json.dumps(receipt,indent=2)+'\n')
        print(json.dumps({'passed':True,'receipt':str(output)}))


if __name__=='__main__': main()

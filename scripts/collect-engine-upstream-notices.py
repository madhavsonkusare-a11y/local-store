"""Collect exact upstream Docker source archives and their preserved license notices.
No tar member is extracted to the host; notices are read with fixed size bounds.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import io
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('companion',ROOT/'scripts/build-engine-source-companion.py')
c=importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
CACHE=c.CACHE
COMPONENTS=[
 ('docker-ce','moby/moby','docker-v29.8.0','3ce5872'),
 ('docker-ce-cli','docker/cli','v29.8.0','88096ef'),
 ('containerd.io','containerd/containerd','v2.3.5','1294c24a7da8e5a793ed378161673abe94118892'),
 ('docker-buildx-plugin','docker/buildx','v0.37.1','0b265a9f62db554fa9aba6dd19e1bd5704bc7d8a'),
 ('docker-compose-plugin','docker/compose','v5.5.1','5f94fb0aa42a2cd1248c6e6c7fafb87546b9c8de'),
 ('containerd.io/runc','opencontainers/runc','v1.5.1','8f2685a471d3347a686ad3909783d8aafc6bb208'),
 ('docker-ce/docker-init','krallin/tini','v0.19.0','de40ad0'),
 ('Go standard library/containerd-docker-buildx-runc','golang/go','go1.26.8',None),
 ('Go standard library/compose','golang/go','go1.26.6',None),
]

def main():
 parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--fetch',action='store_true'); args=parser.parse_args()
 CACHE.mkdir(parents=True,exist_ok=True)
 receipt_path=ROOT/'docs/evidence/engine-upstream-notices-2026-10-03.json'
 previous={}
 if receipt_path.is_file():
  previous={(r['repository'],r['tag']):r for r in json.loads(receipt_path.read_text())['upstream_components']}
 records=[]; notice_files={}; module_review=[]
 for package,repo,tag,observed in COMPONENTS:
  prior=previous.get((repo,tag),{})
  expected_commit=prior.get('commit_capture')
  capture=c.acquire('https://api.github.com/repos/'+repo+'/commits/'+tag,CACHE/'upstream'/(repo.replace('/','-')+'-'+tag+'.commit.json'),256*1024,args.fetch,(expected_commit['sha256'],expected_commit['size_bytes']) if expected_commit else None)
  commit=json.loads((ROOT/capture['cache_path']).read_text())['sha']
  if not re.fullmatch('[0-9a-f]{40}',commit) or (observed and not commit.startswith(observed)):
   raise ValueError('upstream tag disagrees with retained binary revision: '+repo)
  expected_archive=prior.get('source_archive')
  artifact=c.acquire('https://codeload.github.com/'+repo+'/tar.gz/'+commit,CACHE/'upstream'/(repo.replace('/','-')+'-'+commit+'.tar.gz'),100*1024*1024,args.fetch,(expected_archive['sha256'],expected_archive['size_bytes']) if expected_archive else None)
  collected=[]; module_text=None; prefixes=set(); expanded=0
  with tarfile.open(ROOT/artifact['cache_path'],'r:gz') as archive:
   for member in archive:
    name=c.p.safe_name(member.name)
    if not member.isfile(): continue
    expanded+=member.size
    if expanded>800*1024*1024: raise ValueError('upstream archive expanded bound exceeded')
    relative='/'.join(PurePosixPath(name).parts[1:]); base=PurePosixPath(relative).name
    if relative=='vendor/modules.txt':
     if member.size>2*1024*1024: raise ValueError('modules inventory oversized')
     module_text=archive.extractfile(member).read().decode()
    if re.match(r'(?i)^(licen[sc]e|notice|copying|copyright|patents|authors)(?:[._-].*)?$',base) or '/LICENSES/' in '/'+relative:
     if member.size>2*1024*1024: raise ValueError('upstream notice oversized')
     data=archive.extractfile(member).read(); destination='upstream/'+repo+'/'+tag+'/'+relative
     notice_files[destination]=data; collected.append({'path':relative,'sha256':c.p.sha(data),'size_bytes':len(data)})
     if relative.startswith('vendor/'): prefixes.add(str(PurePosixPath(relative).parent)[7:])
  modules=[]
  if module_text:
   for line in module_text.splitlines():
    if line.startswith('# ') and not line.startswith('##') and ' => ' not in line:
     fields=line[2:].split()
     if len(fields)>=2 and fields[1].startswith('v'):
      module,version=fields[:2]
      ancestors=[str(parent) for parent in PurePosixPath(module).parents]
      found=sorted(prefix for prefix in prefixes if prefix==module or prefix in ancestors or prefix.startswith(module+'/'))
      modules.append({'module':module,'version':version,'vendored_notice_paths':found})
  record={'package':package,'repository':repo,'tag':tag,'resolved_commit':commit,'retained_binary_revision':observed,'commit_capture':capture,'source_archive':artifact,'notice_count':len(collected),'notices':collected,'vendored_modules':modules,'modules_without_notice_path':[m for m in modules if not m['vendored_notice_paths']]}
  records.append(record)
  print(repo+' '+tag+': '+str(len(collected))+' notices, '+str(len(modules))+' modules, '+str(len(record['modules_without_notice_path']))+' missing module notice paths',flush=True)
 bundle=CACHE/'upstream-notices.tar'
 with tarfile.open(bundle,'w') as archive:
  for name,data in sorted(notice_files.items()):
   info=tarfile.TarInfo(name); info.size=len(data); info.mtime=0; info.mode=0o644; archive.addfile(info,io.BytesIO(data))
 result={'schema_version':1,'recorded_at':datetime.now(timezone.utc).isoformat(),'rootfs_sha256':c.PAYLOAD,'distribution_approved':False,'upstream_components':records,'notice_bundle':{'cache_path':bundle.relative_to(ROOT).as_posix(),'sha256':c.p.file_sha(bundle),'size_bytes':bundle.stat().st_size},'limitations':['HTTPS GitHub origins and commit-addressed archives are retained with digests; these are not signed upstream attestations.','Docker packaging recipe and all binary dependency mappings require explicit comparison before distribution.','A preserved vendor notice path inventory is not a legal certification.']}
 (ROOT/'docs/evidence/engine-upstream-notices-2026-10-03.json').write_text(json.dumps(result,indent=2)+'\n')

if __name__=='__main__': main()

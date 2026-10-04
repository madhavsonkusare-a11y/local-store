"""Preserve exact Compose dependency sources and missing vendored module notices.

The standard library hashlib implements SHA-256. The module content hash uses
Go's documented Hash1 format: https://go.dev/ref/mod#go-sum-files and
https://github.com/golang/mod/blob/master/sumdb/dirhash/hash.go.
No archive is extracted; all member paths, reads and total sizes are bounded.
"""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import importlib.util
import io
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
import threading
import zipfile

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('companion',ROOT/'scripts/build-engine-source-companion.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
CACHE=c.CACHE
MAX_ZIP=40*1024*1024
MAX_EXPANDED=180*1024*1024
MAX_TOTAL=550*1024*1024

def escaped(value):
 return ''.join('!'+ch.lower() if ch.isupper() else ch for ch in value)

def hash_zip(archive):
 summary=hashlib.sha256();size=0;seen=set()
 for member in sorted(archive.infolist(),key=lambda x:x.filename):
  name=c.p.safe_name(member.filename)
  if '\n' in name or '\r' in name or name in seen: raise ValueError('invalid module zip name')
  seen.add(name);size+=member.file_size
  if member.file_size>MAX_EXPANDED or size>MAX_EXPANDED: raise ValueError('module expanded size exceeds bound')
  with archive.open(member) as stream: digest=hashlib.file_digest(stream,'sha256').hexdigest()
  summary.update((digest+'  '+member.filename+'\n').encode())
 return 'h1:'+base64.b64encode(summary.digest()).decode()

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--fetch',action='store_true');args=parser.parse_args()
 upstream=json.loads((ROOT/'docs/evidence/engine-upstream-notices-2026-10-03.json').read_text())
 if c.p.file_sha(c.BUILD/'rootfs.tar')!=c.PAYLOAD: raise ValueError('retained payload changed')
 wanted={};binary=None
 with tarfile.open(c.BUILD/'rootfs.tar') as archive:
  member=archive.getmember('usr/libexec/docker/cli-plugins/docker-compose');binary=archive.extractfile(member).read()
  for line in set(re.findall(rb'(?m)^dep\t([^\n]+)',binary)):
   module,version,digest=line.decode().split('\t');wanted[(module,version)]={'module':module,'version':version,'expected_h1':digest,'scope':['embedded Compose Go build metadata']}
 for component in upstream['upstream_components']:
  with tarfile.open(ROOT/component['source_archive']['cache_path'],'r:gz') as archive:
   sums={}
   for member in archive:
    if member.isfile() and member.name.endswith(('/go.sum','/go.work.sum')) and '/vendor/' not in member.name:
     if member.size>2*1024*1024:raise ValueError('oversized go.sum')
     for line in archive.extractfile(member).read().decode().splitlines():
      module,version,digest=line.split();sums[(module,version)]=digest
   if component['package']=='docker-compose-plugin':
    for identity,record in wanted.items():
     if 'embedded Compose Go build metadata' in record['scope'] and sums.get(identity)!=record['expected_h1']: raise ValueError('Compose binary module hash disagrees with exact upstream go.sum: '+str(identity))
   for missing in component['modules_without_notice_path']:
    identity=(missing['module'],missing['version'])
    if identity not in sums:raise ValueError('missing dependency has no exact go.sum hash: '+str(identity))
    record=wanted.setdefault(identity,{'module':identity[0],'version':identity[1],'expected_h1':sums[identity],'scope':[]})
    record['scope'].append(component['repository']+' vendored module notice supplement')
 print('Exact module source archives planned: '+str(len(wanted))+'; hard downloaded budget '+str(MAX_TOTAL)+' bytes',flush=True)
 futures={}
 budget={'lock':threading.Lock(),'used':0,'maximum':MAX_TOTAL}
 with ThreadPoolExecutor(max_workers=4) as pool:
  for identity,record in wanted.items():
   module,version=identity;url='https://proxy.golang.org/'+escaped(module)+'/@v/'+escaped(version)+'.zip';path=CACHE/'modules'/(hashlib.sha256((module+'@'+version).encode()).hexdigest()+'.zip')
   futures[identity]=pool.submit(c.acquire,url,path,MAX_ZIP,args.fetch,budget=budget)
  records=[];notices={};total=0
  for identity,record in sorted(wanted.items()):
   capture=futures[identity].result();total+=capture['size_bytes']
   if total>MAX_TOTAL:raise ValueError('module source total budget exceeded')
   files=[]
   with zipfile.ZipFile(ROOT/capture['cache_path']) as archive:
    actual=hash_zip(archive)
    if actual!=record['expected_h1']:raise ValueError('module source differs from binary/exact-upstream module content hash: '+str(identity))
    prefix=identity[0]+'@'+identity[1]+'/'
    for member in archive.infolist():
     if not member.filename.startswith(prefix):raise ValueError('module zip prefix differs from identity')
     relative=member.filename[len(prefix):];name=PurePosixPath(relative).name
     if re.match(r'(?i)^(licen[sc]e|notice|copying|copyright|patents|authors)(?:[._-].*)?$',name) or '/LICENSES/' in '/'+relative:
      if member.file_size>2*1024*1024:raise ValueError('module notice size exceeds bound')
      data=archive.read(member);destination='modules/'+prefix+relative;notices[destination]=data;files.append({'path':relative,'sha256':c.p.sha(data),'size_bytes':len(data)})
   records.append(dict(record,source_archive=capture,verified_h1=actual,notices=files))
   print(identity[0]+'@'+identity[1]+': '+str(len(files))+' notices',flush=True)
 bundle=CACHE/'module-notices.tar'
 with tarfile.open(bundle,'w') as archive:
  for name,data in sorted(notices.items()):
   info=tarfile.TarInfo(name);info.size=len(data);info.mtime=0;info.mode=0o644;archive.addfile(info,io.BytesIO(data))
 result={'schema_version':1,'recorded_at':datetime.now(timezone.utc).isoformat(),'rootfs_sha256':c.PAYLOAD,'distribution_approved':False,'compose_binary_sha256':c.p.sha(binary),'modules':records,'module_source_bytes':total,'modules_without_notice_files':[{'module':r['module'],'version':r['version']} for r in records if not r['notices']],'notice_bundle':{'cache_path':bundle.relative_to(ROOT).as_posix(),'sha256':c.p.file_sha(bundle),'size_bytes':bundle.stat().st_size},'verification':'Every module ZIP content hash matches exact upstream go.sum; Compose module identities/hashes also match retained binary build metadata. HTTPS proxy transport and retained raw SHA-256 are recorded; no checksum-database signature is claimed.'}
 (ROOT/'docs/evidence/engine-module-notices-2026-10-03.json').write_text(json.dumps(result,indent=2)+'\n')

if __name__=='__main__':main()

"""Package retained exact sources and readable notices beside their public manifest."""
from datetime import datetime, timezone
import importlib.util
import io
import json
from pathlib import Path
import tarfile
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('companion',ROOT/'scripts/build-engine-source-companion.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)

def add_bytes(archive,name,data):
 c.p.safe_name(name);info=tarfile.TarInfo(name);info.size=len(data);info.mtime=0;info.mode=0o644;archive.addfile(info,io.BytesIO(data))

def main():
 receipts={}
 for stem in ('source-companion','upstream-notices','module-notices','binary-source-mapping'):
  path=ROOT/'docs/evidence'/('engine-'+stem+'-2026-10-03.json');receipt=json.loads(path.read_text())
  if receipt['rootfs_sha256']!=c.PAYLOAD or receipt['distribution_approved']:raise ValueError('receipt payload/status differs')
  receipts[stem]=(path,receipt)
 initial_path=ROOT/'docs/evidence/engine-independent-provenance-2026-10-02.json'
 initial=json.loads(initial_path.read_text())
 if initial['rootfs_sha256']!=c.PAYLOAD:raise ValueError('initial provenance receipt differs')
 receipts['initial-provenance']=(initial_path,initial)
 ubuntu=receipts['source-companion'][1];upstream=receipts['upstream-notices'][1];modules=receipts['module-notices'][1]
 if not ubuntu['all_ubuntu_sources_retained']:raise ValueError('incomplete Ubuntu source retention')
 material={}
 for row in ubuntu['ubuntu_sources']:
  for artifact in row['artifacts']:
   path=ROOT/artifact['cache_path']
   if (c.p.file_sha(path),path.stat().st_size)!=(artifact['sha256'],artifact['size_bytes']):raise ValueError('Ubuntu source artifact changed')
   material['sources/ubuntu/'+artifact['name']]=dict(artifact)
 for row in upstream['upstream_components']:
  artifact=row['source_archive'];material['sources/upstream/'+Path(artifact['cache_path']).name]=dict(artifact)
 for row in modules['modules']:
  artifact=row['source_archive'];material['sources/modules/'+Path(artifact['cache_path']).name]=dict(artifact,module=row['module'],version=row['version'],verified_h1=row['verified_h1'])
 notices={}
 for receipt in (ubuntu,upstream,modules):
  artifact=receipt['notice_bundle'];path=ROOT/artifact['cache_path']
  if (c.p.file_sha(path),path.stat().st_size)!=(artifact['sha256'],artifact['size_bytes']):raise ValueError('notice capture changed')
  with tarfile.open(path,'r:') as archive:
   for member in archive:
    name=c.p.safe_name(member.name)
    if not member.isfile() or member.size>2*1024*1024:raise ValueError('invalid notice tar member')
    data=archive.extractfile(member).read()
    if name in notices and data!=notices[name]:raise ValueError('notice path collision')
    notices[name]=data
 notice_tar=c.CACHE/'engine-notices.tar'
 with tarfile.open(notice_tar,'w') as archive:
  for name,data in sorted(notices.items()):add_bytes(archive,name,data)
 readable=('Engine notices for rootfs SHA-256 '+c.PAYLOAD+'\n'
           'Distribution remains unapproved. Exact Moby extensions licensing and static docker-init library/build sources remain unresolved.\n'
           'Text below preserves collected upstream/package notices; no permission grant is inferred from missing material.\n\n')
 for name,data in sorted(notices.items()):readable+='='*78+'\nSOURCE PATH: '+name+'\n'+'='*78+'\n'+data.decode('utf-8',errors='replace')+'\n\n'
 readable_path=c.CACHE/'NOTICES.txt';readable_path.write_text(readable,encoding='utf-8')
 manifest={'schema_version':1,'rootfs_sha256':c.PAYLOAD,'distribution_approved':False,'ubuntu_source_identities':len(ubuntu['ubuntu_sources']),'ubuntu_artifacts':sum(len(r['artifacts']) for r in ubuntu['ubuntu_sources']),'upstream_archives':len(upstream['upstream_components']),'module_archives':len(modules['modules']),'notice_files':len(notices),'artifacts':material,'remaining_blockers':['Exact linked Moby extensions source has no license grant recorded.','Static docker-init embedded C library and relinking/build source inputs are not established.','Equivalent public source/notices access and authenticated release metadata must accompany any binary distribution.']}
 source_tar=c.CACHE/'engine-source-companion.tar'
 with tarfile.open(source_tar,'w') as archive:
  add_bytes(archive,'MANIFEST.json',(json.dumps(manifest,indent=2)+'\n').encode())
  for name,artifact in sorted(material.items()):
   path=ROOT/artifact['cache_path']
   if (c.p.file_sha(path),path.stat().st_size)!=(artifact['sha256'],artifact['size_bytes']):raise ValueError('source archive changed')
   info=tarfile.TarInfo(name);info.size=path.stat().st_size;info.mtime=0;info.mode=0o644
   with path.open('rb') as stream:archive.addfile(info,stream)
  for path in sorted((c.CACHE/'indexes').iterdir()):
   if path.is_file():add_bytes(archive,'metadata/indexes/'+path.name,path.read_bytes())
  add_bytes(archive,'metadata/ubuntu-keyring.gpg',(c.CACHE/'ubuntu-keyring.gpg').read_bytes())
  for path,receipt in receipts.values():add_bytes(archive,'metadata/'+path.name,path.read_bytes())
  for name in ('build-engine-source-companion.py','collect-engine-upstream-notices.py','collect-engine-module-notices.py','inspect-engine-source-mapping.py','package-engine-source-companion.py','test_engine_source_companion.py','verify-engine-provenance.py'):add_bytes(archive,'verification/'+name,(ROOT/'scripts'/name).read_bytes())
  add_bytes(archive,'SOURCE_DELIVERY.md',(ROOT/'engine/SOURCE_DELIVERY.md').read_bytes())
  add_bytes(archive,'NOTICES.txt',readable_path.read_bytes())
  info=tarfile.TarInfo('engine-notices.tar');info.size=notice_tar.stat().st_size;info.mtime=0;info.mode=0o644
  with notice_tar.open('rb') as stream:archive.addfile(info,stream)
 result=dict(manifest,recorded_at=datetime.now(timezone.utc).isoformat(),source_archive={'cache_path':source_tar.relative_to(ROOT).as_posix(),'sha256':c.p.file_sha(source_tar),'size_bytes':source_tar.stat().st_size},notice_archive={'cache_path':notice_tar.relative_to(ROOT).as_posix(),'sha256':c.p.file_sha(notice_tar),'size_bytes':notice_tar.stat().st_size},readable_notices={'cache_path':readable_path.relative_to(ROOT).as_posix(),'sha256':c.p.file_sha(readable_path),'size_bytes':readable_path.stat().st_size},input_receipts=[{'path':str(path.relative_to(ROOT)).replace('\\','/'),'sha256':c.p.file_sha(path)} for path,receipt in receipts.values()])
 (ROOT/'docs/evidence/engine-companion-bundle-2026-10-03.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({k:result[k] for k in ('ubuntu_source_identities','ubuntu_artifacts','upstream_archives','module_archives','notice_files','source_archive','notice_archive','readable_notices')},indent=2))
if __name__=='__main__':main()

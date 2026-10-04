"""Inspect retained engine linkage/source metadata with pyelftools; never execute it."""
import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import sys
import tarfile
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('companion',ROOT/'scripts/build-engine-source-companion.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)

def main():
 sys.path.insert(0,str(c.CACHE/'vendor'))
 from elftools.elf.elffile import ELFFile
 if c.p.file_sha(c.BUILD/'rootfs.tar')!=c.PAYLOAD:raise ValueError('retained payload changed')
 paths={'usr/bin/docker','usr/bin/dockerd','usr/bin/containerd','usr/bin/ctr','usr/bin/containerd-shim-runc-v2','usr/bin/runc','usr/bin/docker-proxy','usr/libexec/docker/docker-init','usr/libexec/docker/cli-plugins/docker-buildx','usr/libexec/docker/cli-plugins/docker-compose'}
 rows=[]
 with tarfile.open(c.BUILD/'rootfs.tar') as archive:
  for member in archive:
   if member.name in paths and member.isfile():
    if member.size>128*1024*1024:raise ValueError('binary metadata inspection size exceeds bound')
    data=archive.extractfile(member).read();elf=ELFFile(io.BytesIO(data));dynamic=elf.get_section_by_name('.dynamic')
    needed=[tag.needed for tag in dynamic.iter_tags() if tag.entry.d_tag=='DT_NEEDED'] if dynamic else []
    row={'path':member.name,'size_bytes':len(data),'sha256':c.p.sha(data),'shared_libraries':needed,'has_dynamic_section':dynamic is not None,'go_versions':sorted(set(x.decode() for x in re.findall(rb'go1\.\d+\.\d+',data))),'embedded_revision_lines':sorted(set(x.decode(errors='replace') for x in re.findall(rb'(?m)^build\t(?:vcs[^\n]+|-ldflags=[^\n]+)',data))),'extension_module_metadata':sorted(set(x.decode() for x in re.findall(rb'(?m)^dep\tgithub.com/moby/extensions[^\n]+',data))),'c_compiler_strings':sorted(set(x.decode() for x in re.findall(rb'GCC: [^\x00]+',data))),'tini_version_strings':sorted(set(x.decode() for x in re.findall(rb'tini version [^\x00\n]{1,80}',data))) if member.name.endswith('docker-init') else []}
    rows.append(row)
 if paths!={row['path'] for row in rows}:raise ValueError('expected retained binary missing')
 result={'schema_version':1,'rootfs_sha256':c.PAYLOAD,'parser':'pyelftools 0.32; embedded textual Go build metadata observed without execution','binaries':rows,'distribution_approved':False,'limitations':['ELF linkage and build metadata are observations, not proof of every input used in upstream compilation.','Static docker-init has tini 0.19.0 git.de40ad0 and Ubuntu GCC 13.3.0 metadata; the exact embedded C library revision and relinking/build inputs have not been established.']}
 (ROOT/'docs/evidence/engine-binary-source-mapping-2026-10-03.json').write_text(json.dumps(result,indent=2)+'\n')
 print('Inspected '+str(len(rows))+' binaries without execution; dockerd links unlicensed-source-metadata extensions; static docker-init C library remains unproved.')
if __name__=='__main__':main()

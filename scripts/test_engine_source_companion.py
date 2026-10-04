import importlib.util
import io
from pathlib import Path
import tempfile
import threading
import unittest
import warnings
import zipfile

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('modules',ROOT/'scripts/collect-engine-module-notices.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)

class SourceBoundaryTests(unittest.TestCase):
 def test_retained_bytes_require_exact_digest_and_length(self):
  with tempfile.TemporaryDirectory(dir=m.CACHE) as folder:
   path=Path(folder)/'artifact';path.write_bytes(b'altered')
   with self.assertRaisesRegex(ValueError,'cached source differs'):
    m.c.acquire('https://snapshot.ubuntu.com/artifact',path,100,False,(m.c.p.sha(b'genuine'),7))
 def test_missing_offline_artifact_refused(self):
  with tempfile.TemporaryDirectory(dir=m.CACHE) as folder:
   with self.assertRaisesRegex(ValueError,'missing capture'):
    m.c.acquire('https://snapshot.ubuntu.com/artifact',Path(folder)/'missing',100,False)
 def test_cached_size_bound_refused(self):
  with tempfile.TemporaryDirectory(dir=m.CACHE) as folder:
   path=Path(folder)/'artifact';path.write_bytes(b'large')
   with self.assertRaisesRegex(ValueError,'cached source differs'):
    m.c.acquire('https://snapshot.ubuntu.com/artifact',path,1,False)
 def test_aggregate_budget_refused(self):
  budget={'lock':threading.Lock(),'used':8,'maximum':10}
  with self.assertRaisesRegex(ValueError,'aggregate'):
   m.c.charge_budget(budget,3)
  self.assertEqual(budget['used'],8)
 def zip_hash(self,data,name='example.com/module@v1.0.0/LICENSE'):
  buf=io.BytesIO()
  with zipfile.ZipFile(buf,'w') as archive:archive.writestr(name,data)
  buf.seek(0)
  with zipfile.ZipFile(buf) as archive:return m.hash_zip(archive)
 def test_module_source_content_change_changes_h1(self):
  self.assertNotEqual(self.zip_hash(b'genuine'),self.zip_hash(b'altered'))
 def test_module_unsafe_path_refused(self):
  with self.assertRaisesRegex(ValueError,'unsafe tar member'):
   self.zip_hash(b'content','../escape')
 def test_module_duplicate_member_refused(self):
  buf=io.BytesIO()
  with warnings.catch_warnings():
   warnings.simplefilter('ignore',UserWarning)
   with zipfile.ZipFile(buf,'w') as archive:
    archive.writestr('module/LICENSE',b'a');archive.writestr('module/LICENSE',b'b')
  buf.seek(0)
  with zipfile.ZipFile(buf) as archive:
   with self.assertRaisesRegex(ValueError,'invalid module zip name'):m.hash_zip(archive)
 def test_module_empty_zip_matches_official_hash1_format(self):
  buf=io.BytesIO()
  with zipfile.ZipFile(buf,'w'):pass
  buf.seek(0)
  with zipfile.ZipFile(buf) as archive:
   self.assertEqual(m.hash_zip(archive),'h1:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=')

if __name__=='__main__':
 m.CACHE.mkdir(parents=True,exist_ok=True)
 unittest.main()

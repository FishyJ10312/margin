"""Real source-package CLI boundaries, using owned synthetic source trees only."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]

# Control timing at the ZIP/filesystem boundary without replacing the packager
# or its publication code. Both real archives must be sealed before either
# process is allowed to publish to their common destination.
GATED_CLI = r'''
from pathlib import Path
import runpy, sys, time, zipfile
script, target, ready, release = map(Path, sys.argv[1:])
original_close = zipfile.ZipFile.close
def gated_close(archive):
    original_close(archive)
    if archive.mode != 'w' or getattr(archive, '_publication_gate_used', False):
        return
    archive._publication_gate_used = True
    if not zipfile.is_zipfile(archive.filename):
        raise RuntimeError('The staged archive was not sealed')
    ready.write_text('sealed')
    end = time.monotonic() + 8
    while not release.exists():
        if time.monotonic() >= end:
            raise RuntimeError('Publication gate was not released')
        time.sleep(.005)
zipfile.ZipFile.close = gated_close
sys.path.insert(0, str(script.parent))
sys.argv = [str(script), str(target)]
runpy.run_path(str(script), run_name='__main__')
'''


class SourcePackageBoundaries(unittest.TestCase):
    def source_tree(self, parent, name):
        root = parent / name
        (root / 'scripts').mkdir(parents=True)
        (root / 'src-tauri').mkdir()
        for filename in ['package-source.py', 'distribution_privacy.py']:
            shutil.copy2(ROOT / 'scripts' / filename, root / 'scripts' / filename)
        (root / 'scripts/source-assets.json').write_text('{}')
        (root / 'package.json').write_text(json.dumps({'version': '0.0.0-test'}))
        (root / 'src-tauri/tauri.conf.json').write_text(json.dumps({'productName': 'Margin'}))
        (root / 'sample.txt').write_text(name)
        (root / '.env').write_text('excluded synthetic settings')
        return root

    def test_concurrent_source_packages_do_not_overwrite_the_accepted_archive(self):
        with tempfile.TemporaryDirectory(prefix='margin-source-collision-') as temporary:
            root = Path(temporary)
            runner = root / 'gated-cli.py'
            runner.write_text(GATED_CLI)
            target = root / 'source.zip'
            processes = []
            releases = []
            try:
                ready_paths = []
                for name in ['first', 'second']:
                    project = self.source_tree(root, name)
                    ready = root / (name + '.ready')
                    release = root / (name + '.release')
                    releases.append(release)
                    ready_paths.append(ready)
                    processes.append(subprocess.Popen(
                        [sys.executable, str(runner), str(project / 'scripts/package-source.py'),
                         str(target), str(ready), str(release)],
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True))
                end = time.monotonic() + 6
                while not all(path.exists() for path in ready_paths):
                    self.assertTrue(all(p.poll() is None for p in processes),
                                    'A packager stopped before its sealed-archive gate')
                    self.assertLess(time.monotonic(), end, 'Both packagers must reach the gate')
                    time.sleep(.005)
                self.assertFalse(target.exists(), 'An unfinished archive became visible')
                releases[0].touch()
                stdout, stderr = processes[0].communicate(timeout=4)
                self.assertEqual(processes[0].returncode, 0, stdout + stderr)
                accepted = target.read_bytes()
                with zipfile.ZipFile(target) as archive:
                    self.assertIsNone(archive.testzip())
                    self.assertEqual(archive.read('Margin-0.0.0-test/sample.txt'), b'first')
                    self.assertFalse(any(name.endswith('/.env') for name in archive.namelist()))
                releases[1].touch()
                stdout, stderr = processes[1].communicate(timeout=4)
                self.assertNotEqual(
                    processes[1].returncode, 0,
                    'Both publishers succeeded; accepted archive changed=' +
                    str(target.read_bytes() != accepted))
                self.assertEqual(target.read_bytes(), accepted)
                self.assertIn('Refusing to replace an existing source package', stdout + stderr)
                self.assertEqual((root / 'second/sample.txt').read_text(), 'second')
            finally:
                for release in releases:
                    release.touch(exist_ok=True)
                for process in processes:
                    if process.poll() is None:
                        process.kill()
                    process.wait(timeout=3)
                    process.stdout.close()
                    process.stderr.close()


if __name__ == '__main__':
    unittest.main()

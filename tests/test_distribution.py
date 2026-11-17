"""Synthetic packaging boundaries; no user data or external services."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from distribution_privacy import findings, verify_source
spec = importlib.util.spec_from_file_location('diagnostics', ROOT / 'scripts/export-diagnostics.py')
diagnostics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostics)

class DistributionBoundaries(unittest.TestCase):
    def test_private_paths_and_credentials_are_rejected_without_echo(self):
        for payload in [b'/Users/' + b'fixture/private.jpg', b'C:\\Users\\' + b'fixture\\photo.jpg', b'sk-' + b'x' * 32]:
            with self.assertRaises(ValueError) as error:
                findings(payload, 'fixture')
            self.assertNotIn(payload.decode(), str(error.exception))
        findings(b'/build/home/.cargo/registry/src/lib.rs', 'generic-build-path')
        with self.assertRaises(ValueError):
            findings(b'/build/home/Users/' + b'fixture/private.jpg', 'nested-private-path')

    def test_only_pinned_synthetic_oauth_key_is_allowed_in_source(self):
        relative = Path('src-tauri/test-fixtures/oauth-test-only.pem')
        data = (ROOT / relative).read_bytes()
        verify_source(relative, data)
        with self.assertRaises(ValueError):
            findings(data, 'production-binary')
        with self.assertRaises(ValueError):
            verify_source(relative, data + b'changed')

    def test_unreviewed_binary_and_auth_store_are_not_source_material(self):
        for name, data in [('photo.jpg', b'not-approved'), ('auth.json', b'{}'), ('extra.blob', b'\0binary')]:
            with self.assertRaises(ValueError):
                verify_source(Path(name), data)

    def test_export_omits_private_neighbors_and_rejects_tampering(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            reports = root / 'reports'; reports.mkdir()
            allowlist = {}
            for name in diagnostics.REPORTS:
                data = b'{"fixture":true}\n'
                (reports / name).write_bytes(data)
                allowlist[name] = hashlib.sha256(data).hexdigest()
            (reports / 'diagnostics-allowlist.json').write_text(json.dumps(allowlist))
            private = reports / 'real-local-screenshot.png'
            private.write_bytes(b'synthetic stand-in for an excluded private neighbor')
            before = private.read_bytes()
            destination = root / 'share.zip'
            diagnostics.export(reports, destination)
            with zipfile.ZipFile(destination) as archive:
                self.assertEqual(set(archive.namelist()), diagnostics.REPORTS | {'diagnostics-allowlist.json'})
            self.assertEqual(private.read_bytes(), before)
            (reports / 'preservation.json').write_text('{"changed":true}')
            with self.assertRaises(ValueError):
                diagnostics.export(reports, root / 'rejected.zip')
            self.assertFalse((root / 'rejected.zip').exists())

    def test_even_a_hash_matched_report_cannot_contain_a_photo_or_account(self):
        for data in [b'{"photo":"data:image/png;base64,AAAA"}', b'{"account":"fixture@example.invalid"}']:
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); allowlist = {}
                for name in diagnostics.REPORTS:
                    content = data if name == 'preservation.json' else b'{}'
                    (root / name).write_bytes(content)
                    allowlist[name] = hashlib.sha256(content).hexdigest()
                (root / 'diagnostics-allowlist.json').write_text(json.dumps(allowlist))
                with self.assertRaises(ValueError):
                    diagnostics.export(root, root / 'rejected.zip')

if __name__ == '__main__':
    unittest.main()

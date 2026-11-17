#!/usr/bin/env python3
"""Export reviewed JSON reports only; never recursively copy a diagnostics folder."""
import argparse
import hashlib
import json
import re
import tempfile
import zipfile
from pathlib import Path
from distribution_privacy import findings

REPORTS = frozenset({
    'artifact-review.json', 'privacy-checks.json', 'offline-boundaries.json',
    'component-review.json', 'preservation.json', 'limitations.json',
})

def export(directory, destination):
    if destination.exists():
        raise ValueError('Refusing to replace an existing diagnostic export')
    manifest_path = directory / 'diagnostics-allowlist.json'
    if manifest_path.is_symlink():
        raise ValueError('The diagnostic allowlist must be a regular local file')
    manifest = json.loads(manifest_path.read_text())
    if set(manifest) != REPORTS:
        raise ValueError('The reviewed JSON report allowlist does not match')
    entries = {}
    for name in sorted(REPORTS):
        source = directory / name
        if source.is_symlink() or not source.is_file():
            raise ValueError(f'Expected regular report: {name}')
        data = source.read_bytes()
        if len(data) > 512 * 1024 or hashlib.sha256(data).hexdigest() != manifest[name]:
            raise ValueError(f'Changed or oversized report: {name}')
        findings(data, name)
        text = data.decode('utf-8')
        if re.search(r'[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}|data:image/|file://|/(?:var/folders|private|Volumes)/', text):
            raise ValueError(f'Non-shareable content in report: {name}')
        json.loads(text)
        entries[name] = data
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.margin-diagnostics-', dir=destination.parent) as staging:
        temporary = Path(staging) / 'reports.zip'
        with zipfile.ZipFile(temporary, 'w', zipfile.ZIP_DEFLATED) as archive:
            for name, data in entries.items():
                archive.writestr(name, data)
            archive.writestr('diagnostics-allowlist.json', json.dumps(manifest, indent=2) + '\n')
        temporary.rename(destination)
    return {'reports': len(entries), 'includesScreenshots': False, 'includesRawLogs': False}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reviewed_reports', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    print(json.dumps(export(args.reviewed_reports.resolve(), args.destination.resolve())))

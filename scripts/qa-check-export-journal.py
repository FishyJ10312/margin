#!/usr/bin/env python3
"""Check validated export runs for redundant journal writes, without flaky timing limits."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('report', type=Path)
args = parser.parse_args()
report = json.loads(args.report.read_text())
assert report['sourceFilesUnchanged'] and report['representativeOutputPixelsMatch']
assert all(r['allOutputsValidated'] for r in report['measurements'])
failures = []
for name, data in report['datasets'].items():
    baseline = max(data['journalSaves']['baseline'])
    candidate = max(data['journalSaves']['candidate'])
    if candidate > baseline:
        failures.append(f'{name}: {candidate} saves exceeds the validated queue baseline of {baseline}')
assert not failures, '; '.join(failures)
print('PASS: validated exports retain the baseline journal-write budget')

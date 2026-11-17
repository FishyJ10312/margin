#!/usr/bin/env python3
"""Run existing UI assertions in one Ego task space, using fixture replies only."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--space', default='Margin isolated UI acceptance')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
cases = []
names = ['preview-recovery-acceptance', 'drop-acceptance', 'font-scroll-acceptance',
         'watermark-workspace-acceptance', 'login-acceptance']
for name in names:
    source = (root / 'tests/ui' / f'{name}.mjs').read_text()
    script, count = re.subn(r"taskSpace\('[^']*'\)",
                            lambda _: f'taskSpace({json.dumps(args.space)})',
                            source, count=1)
    if count != 1:
        raise SystemExit(f'Expected exactly one task space in {name}')
    script = script.replace(
        "(await import('node:os')).tmpdir()",
        json.dumps(str(output)))
    script = script.replace('/tmp/liubai-corner-watermark-verified.png',
                            str(output / 'watermark-corner.png'))
    (output / f'{name}.mjs').write_text(script)
    started = time.monotonic()
    run = subprocess.run(['ego-browser', 'nodejs'], input=script,
                         text=True, capture_output=True)
    (output / f'{name}.log').write_text(run.stdout + run.stderr)
    case = {'name': name, 'exit': run.returncode,
            'seconds': round(time.monotonic() - started, 3),
            'sourceSha256': hashlib.sha256(source.encode()).hexdigest(),
            'scope': 'Real UI, mocked native/photo/auth replies; no live model calls'}
    cases.append(case)
    print(json.dumps(case), flush=True)
    if run.returncode:
        (output / 'report.json').write_text(json.dumps({
            'passed': False, 'cases': cases,
            'spaceFinished': False, 'failure': run.stderr[-4000:]}, indent=2))
        raise SystemExit(run.returncode)

# Finish once, only after all assertions passed. On a failure the same space
# remains available for inspection; do not recover by creating another space.
finish = subprocess.run(['ego-browser', 'nodejs'], text=True, capture_output=True,
                        input=f'const task=await taskSpace({json.dumps(args.space)});'
                              'await task.finish({keep:[]});')
(output / 'finish.log').write_text(finish.stdout + finish.stderr)
(output / 'report.json').write_text(json.dumps({
    'passed': finish.returncode == 0, 'cases': cases,
    'spaceFinished': finish.returncode == 0,
    'scope': 'Fixture assertions unchanged; only task-space and screenshot paths rewritten'}, indent=2))
raise SystemExit(finish.returncode)

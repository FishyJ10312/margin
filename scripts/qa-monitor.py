#!/usr/bin/env python3
"""Bounded read-only native backend monitor; does not drive UI or contact models."""
import argparse
import json
import subprocess
import time
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--pid', type=int, required=True)
p.add_argument('--seconds', type=int, default=3600)
p.add_argument('--state-dir', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
if not 1 <= a.seconds <= 7200:
    raise SystemExit('Use a bounded 1–7200 second observation')
a.output.mkdir(parents=True, exist_ok=True)
def process():
    r = subprocess.run(['ps', '-p', str(a.pid), '-o', 'lstart=,rss=,%cpu=,command='], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None
initial = process()
if not initial or 'liubai' not in initial:
    raise SystemExit('Expected the explicitly launched Margin QA backend')
started = time.monotonic()
rows = []
while True:
    line = process()
    elapsed = time.monotonic() - started
    if not line or line.split()[:5] != initial.split()[:5]:
        raise SystemExit('Observed process exited or PID was reused')
    files = [f for f in a.state_dir.rglob('*') if f.is_file()]
    row = {'elapsedSeconds': round(elapsed, 2), 'rssKiB': int(line.split()[5]),
           'cpuPercent': float(line.split()[6]), 'stateFiles': len(files),
           'stateBytes': sum(f.stat().st_size for f in files)}
    rows.append(row)
    (a.output / 'native-samples.json').write_text(json.dumps(rows, indent=2) + '\n')
    if elapsed >= a.seconds:
        break
    if len(rows) % 15 == 1:
        print(json.dumps(row), flush=True)
    time.sleep(min(20, a.seconds - elapsed))
report = {'durationSeconds': rows[-1]['elapsedSeconds'], 'samples': len(rows),
          'backendSurvived': True, 'rssMaxKiB': max(r['rssKiB'] for r in rows),
          'rssLastKiB': rows[-1]['rssKiB'], 'stateBytesFirst': rows[0]['stateBytes'],
          'stateBytesLast': rows[-1]['stateBytes'],
          'limits': 'Backend RSS only. UI responsiveness requires separate native observations; WebKit/GPU, real low memory and physical disk-full are not measured by this script.'}
(a.output / 'native-monitor-report.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report), flush=True)

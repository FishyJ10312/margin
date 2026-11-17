#!/usr/bin/env python3
"""Read-only Mac QA memory sampling, including the app's PID-domain XPC services."""
import argparse
import ctypes
import hashlib
import json
from pathlib import Path
import re
import stat
import subprocess
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument('--pid', type=int, required=True)
parser.add_argument('--seconds', type=int, default=1200)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--state-dir', type=Path, required=True)
args = parser.parse_args()
if sys.platform != 'darwin' or not 1 <= args.seconds <= 3600:
    raise SystemExit('Use a Mac and a bounded observation of at most one hour')
if 'com.fishyj.liubai' in str(args.state_dir):
    raise SystemExit('Use an isolated QA profile, not the user workspace')
args.output.mkdir(parents=True, exist_ok=False)


def processes():
    result = subprocess.run(['ps', '-ax', '-o', 'pid=,ppid=,lstart=,rss=,%cpu=,comm='],
                            capture_output=True, text=True, check=True)
    rows = {}
    for line in result.stdout.splitlines():
        fields = line.split(None, 9)
        if len(fields) == 10:
            pid, ppid = map(int, fields[:2])
            rows[pid] = {'pid': pid, 'ppid': ppid,
                         'start': ' '.join(fields[2:7]),
                         'rssKiB': int(fields[7]), 'cpuPercent': float(fields[8]),
                         'executable': fields[9]}
    return rows


# Public rusage_info_v0 layout from the installed SDK's sys/resource.h.
class Usage(ctypes.Structure):
    _fields_ = [('uuid', ctypes.c_uint8 * 16)] + [
        (name, ctypes.c_uint64) for name in ['user_time', 'system_time',
        'pkg_idle_wkups', 'interrupt_wkups', 'pageins', 'wired_size',
        'resident_size', 'phys_footprint', 'proc_start_abstime', 'proc_exit_abstime']]


system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
usage = system.proc_pid_rusage
usage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
usage.restype = ctypes.c_int
responsibility = getattr(system, 'responsibility_get_pid_responsible_for_pid', None)
if responsibility:
    responsibility.argtypes = [ctypes.c_int]
    responsibility.restype = ctypes.c_int

initial = processes().get(args.pid)
if not initial or '/Margin Stability RC.app/Contents/MacOS/liubai' not in initial['executable']:
    raise SystemExit('Expected the explicitly launched Margin Stability RC app')
# This optional private SPI is diagnostic-only and never ships in the app.
# A self-responsible LaunchServices start makes its association unambiguous.
self_responsible = responsibility is not None and responsibility(args.pid) == args.pid
started = time.monotonic()
script_hash = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
samples = []
reason = 'duration-completed'
while True:
    all_rows = processes()
    backend = all_rows.get(args.pid)
    if not backend or backend['start'] != initial['start']:
        reason = 'backend-exited-or-pid-reused'
        break
    domain = subprocess.run(['launchctl', 'print', f'pid/{args.pid}'],
                            text=True, capture_output=True)
    services = {}
    # Read just the services section; never persist launch-domain environment.
    match = re.search(r'\n\s*services = \{(.*?)\n\s*\}', domain.stdout, re.S)
    if match:
        for line in match.group(1).splitlines():
            service = re.match(r'\s*(\d+)\s+\S+\s+(\S+)', line)
            if service and int(service[1]):
                services[int(service[1])] = service[2]
    selected = {args.pid: 'backend'}
    changed = True
    while changed:
        changed = False
        for pid, row in all_rows.items():
            if pid not in selected and row['ppid'] in selected:
                selected[pid] = 'direct-process-descendant'
                changed = True
    for pid, label in services.items():
        selected.setdefault(pid, f'pid-domain:{label}')
    members = []
    for pid, association in selected.items():
        if pid not in all_rows:
            continue
        row = all_rows[pid]
        data = Usage()
        footprint = data.phys_footprint if usage(pid, 0, ctypes.byref(data)) == 0 else None
        responsible_pid = responsibility(pid) if responsibility else None
        members.append({'pid': pid, 'ppid': row['ppid'], 'start': row['start'],
                        'name': Path(row['executable']).name,
                        'association': association, 'rssKiB': row['rssKiB'],
                        'physicalFootprintBytes': footprint,
                        'responsiblePid': responsible_pid,
                        'responsibilityMatches': responsible_pid == args.pid})
    # Atomic journal/export commits can rename a temp between enumeration and stat.
    # Keep one successful stat per file so a normal commit cannot abort sampling.
    files = []
    for path in args.state_dir.rglob('*'):
        try:
            metadata = path.stat()
        except FileNotFoundError:
            continue
        if stat.S_ISREG(metadata.st_mode):
            files.append((path, metadata.st_size))
    storage_groups = {}
    for path, size in files:
        first = path.relative_to(args.state_dir).parts[0]
        group = ('syntheticInputs' if first == 'qa-media' else
                 'qaExports' if first.startswith('qa-exports-') else
                 'thumbnailCache' if first == 'thumbnails' else 'settingsAndAssets')
        item = storage_groups.setdefault(group, {'files': 0, 'bytes': 0})
        item['files'] += 1
        item['bytes'] += size
    sample = {'elapsedSeconds': round(time.monotonic() - started, 3),
              'members': members, 'domainQuerySucceeded': domain.returncode == 0,
              'rssSumKiB': sum(p['rssKiB'] for p in members),
              'footprintSumBytes': sum(p['physicalFootprintBytes'] or 0 for p in members),
              'stateFiles': len(files), 'stateBytes': sum(size for _, size in files),
              'storageGroups': storage_groups}
    samples.append(sample)
    (args.output / 'samples.json').write_text(json.dumps(samples, indent=2))
    if len(samples) % 30 == 1:
        print(json.dumps({k: sample[k] for k in ['elapsedSeconds', 'rssSumKiB', 'footprintSumBytes', 'stateBytes']}), flush=True)
    if sample['rssSumKiB'] > 2 * 1024 * 1024:
        reason = 'qa-group-exceeded-2-gib-guard'
        break
    if sample['elapsedSeconds'] >= args.seconds:
        break
    time.sleep(min(2, args.seconds - sample['elapsedSeconds']))

report = {'reason': reason, 'sampleCount': len(samples),
          'scriptSha256': script_hash,
          'durationSeconds': samples[-1]['elapsedSeconds'] if samples else 0,
          'backendSelfResponsibleAtStart': self_responsible,
          'rssSumMaxKiB': max((s['rssSumKiB'] for s in samples), default=0),
          'footprintSumMaxBytes': max((s['footprintSumBytes'] for s in samples), default=0),
          'domainQueryFailures': sum(not s['domainQuerySucceeded'] for s in samples),
          'storageGroupsFirst': samples[0]['storageGroups'] if samples else {},
          'storageGroupsLast': samples[-1]['storageGroups'] if samples else {},
          'scope': 'Observed backend + process descendants + services explicitly listed in this PID launch domain. RSS sums can double-count shared pages; footprint sum is per-process OS accounting, not physical RAM/VRAM or a universal whole-app peak.',
          'attribution': 'launchctl PID-domain association, with optional local private responsibility SPI cross-check; not a new app/runtime dependency',
          'limits': '2-second samples; GPU service process memory is included when associated, but GPU device allocations, shared system services and OS-wide pressure are not measured'}
(args.output / 'report.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report), flush=True)
raise SystemExit(0 if reason == 'duration-completed' else 1)

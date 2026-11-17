#!/usr/bin/env python3
"""Kill only the explicitly launched QA child after observing a partial export."""
import argparse
import hashlib
import json
import os
import signal
import subprocess
import time
from pathlib import Path

p=argparse.ArgumentParser();p.add_argument('runner',type=Path);p.add_argument('output',type=Path);p.add_argument('--native-library-directory',type=Path);a=p.parse_args()
runner=a.runner.resolve();root=a.output.resolve()
if runner.name!='verify_crash_recovery' or root.exists():raise SystemExit('Expected the named QA runner and a new output directory')
env=dict(os.environ)
if a.native_library_directory:
    env['DYLD_FALLBACK_LIBRARY_PATH']=str(a.native_library_directory.resolve())
subprocess.run([str(runner),'init',str(root)],check=True,env=env)
hashes={n:hashlib.sha256((root/n).read_bytes()).hexdigest()for n in ['one.png','two.png']}
child=subprocess.Popen([str(runner),'start',str(root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
started=time.monotonic();observed=[]
try:
    while child.poll() is None and time.monotonic()-started<30:
        observed=[]
        for f in (root/'exports').glob('*.tmp'):
            try:
                if f.stat().st_size>=8192:observed.append(f)
            except FileNotFoundError:pass
        if observed:
            os.kill(child.pid,signal.SIGKILL);break
        time.sleep(.002)
    child.communicate(timeout=5)
finally:
    if child.poll() is None:child.kill();child.communicate()
if not observed or child.returncode!=-signal.SIGKILL:raise SystemExit('No real interrupted write observed; do not claim a crash test')
before=[{'file':f.name,'bytes':f.stat().st_size} for f in observed if f.exists()]
result=subprocess.run([str(runner),'resume',str(root)],capture_output=True,text=True,env=env)
report={'killedOnlyQaChild':True,'childExit':child.returncode,'observedPartialFiles':before,
        'resumeExit':result.returncode,'resumeOutput':result.stdout.strip(),'resumeError':result.stderr.strip(),
        'sourcesUnchanged':all(hashlib.sha256((root/n).read_bytes()).hexdigest()==digest for n,digest in hashes.items()),
        'scope':'Real SIGKILL during a private export; not a power-loss test or a kill of the user app'}
(root/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
raise SystemExit(result.returncode)

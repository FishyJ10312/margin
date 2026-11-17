#!/usr/bin/env python3
"""Deterministic, bounded malformed-image smoke corpus. Isolated children only."""
import argparse
import json
import os
import random
import struct
import subprocess
import time
from pathlib import Path

p=argparse.ArgumentParser();p.add_argument('runner',type=Path);p.add_argument('heic',type=Path);p.add_argument('png',type=Path);p.add_argument('output',type=Path);p.add_argument('--native-library-directory',type=Path,required=True);a=p.parse_args()
runner=a.runner.resolve();root=a.output.resolve()
if runner.name!='probe_input' or root.exists():raise SystemExit('Use the QA probe and a new output directory')
root.mkdir(parents=True);inputs=root/'inputs';inputs.mkdir()
cases=[]
for extension,source in [('heic',a.heic),('png',a.png)]:
    data=source.read_bytes()
    for cut in sorted({0,1,8,32,len(data)//2,len(data)-1}):cases.append((f'{extension}-truncated-{cut}.{extension}',data[:cut]))
    # Header mutations include lengths, color metadata, rotation and HEVC config.
    if extension=='heic':
        for label in [b'meta',b'colr',b'ispe',b'irot',b'hvcC',b'nclx']:
            n=data.find(label)
            if n<0:continue
            for v in [0,255]:
                b=bytearray(data);b[n+4:min(n+12,len(b))]=bytes([v])*min(8,len(b)-n-4);cases.append((f'heic-{label.decode()}-{v}.heic',bytes(b)))
    else:
        for value in [0,32769,2**31-1]:
            b=bytearray(data);b[16:24]=struct.pack('>II',value,value);cases.append((f'png-dimension-{value}.png',bytes(b)))
    rng=random.Random(1117)
    for i in range(30):
        b=bytearray(data);position=rng.randrange(min(len(b),850));b[position]^=1<<rng.randrange(8);cases.append((f'{extension}-bit-{i:02d}.{extension}',bytes(b)))
env=dict(os.environ);env['DYLD_FALLBACK_LIBRARY_PATH']=str(a.native_library_directory.resolve())
rows=[];failures=[];started=time.monotonic()
for name,data in cases:
    path=inputs/name;path.write_bytes(data)
    try:
        r=subprocess.run([str(runner),str(path)],env=env,text=True,capture_output=True,timeout=3)
        if r.returncode!=0:
            failures.append({'file':name,'exit':r.returncode,'error':r.stderr[-1000:]});continue
        result=json.loads(r.stdout);result['file']=name
        if not result['sourceUnchanged'] or result.get('peakRssKiB',0)>256*1024:failures.append({'file':name,'error':'source mutation or excessive tiny-input RSS'})
        rows.append(result)
    except (subprocess.TimeoutExpired,json.JSONDecodeError)as e:failures.append({'file':name,'error':str(e)[:500]})
report={'inputs':len(cases),'accepted':sum(r['accepted']for r in rows),'rejected':sum(not r['accepted']for r in rows),'crashesTimeoutsOrInvariantFailures':failures,'maxChildPeakRssKiB':max((r.get('peakRssKiB',0)for r in rows),default=0),'seconds':time.monotonic()-started,'cases':rows,'limits':'Small deterministic mutations only, not exhaustive fuzzing, exploit testing or a whole-app soak'}
(root/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k!='cases'}));raise SystemExit(bool(failures))

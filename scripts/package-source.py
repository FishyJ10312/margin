#!/usr/bin/env python3
"""Package reproducible sources, excluding build caches and personal test media."""
from pathlib import Path
import json, sys, zipfile
root = Path(__file__).resolve().parents[1]
version = json.loads((root / 'package.json').read_text())['version']
target = Path(sys.argv[1]).resolve()
exclude = {'node_modules', 'target', 'dist', '.native', '.git', '__pycache__', 'gen', 'deliverables'}
with zipfile.ZipFile(target, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
    for path in sorted(root.rglob('*')):
        relative = path.relative_to(root)
        if not path.is_file() or any(part in exclude for part in relative.parts) or path.name in {'.DS_Store', 'ui-qa.png', 'ui-qa-narrow.png'}:
            continue
        if path.resolve() == target: continue
        if (relative.as_posix().startswith('tests/ui-') and path.suffix == '.png') or relative.as_posix() == 'tests/apple-generated.heic' or path.name == '.env' or (path.name.startswith('.env.') and path.name != '.env.example'):
            continue
        data = path.read_bytes()
        archive.writestr(f'Liubai-{version}/{relative.as_posix()}', data)
print(f'{target.name}: {target.stat().st_size:,} bytes')

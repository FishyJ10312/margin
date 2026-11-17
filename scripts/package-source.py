#!/usr/bin/env python3
"""Package reproducible sources, excluding build caches and personal test media."""
from pathlib import Path
import json, os, sys, tempfile, zipfile
from distribution_privacy import verify_source
root = Path(__file__).resolve().parents[1]
version = json.loads((root / 'package.json').read_text())['version']
product_name = json.loads((root / 'src-tauri/tauri.conf.json').read_text())['productName']
target = Path(sys.argv[1]).resolve()
exclude = {'node_modules', 'target', 'dist', '.native', '.git', '__pycache__', 'gen', 'generated', 'deliverables'}
if target.exists():
    raise SystemExit('Refusing to replace an existing source package')
target.parent.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='.margin-source-', dir=target.parent) as staging:
  temporary = Path(staging) / target.name
  with zipfile.ZipFile(temporary, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
    for path in sorted(root.rglob('*')):
        relative = path.relative_to(root)
        if not path.is_file() or path.is_symlink() or any(part in exclude for part in relative.parts) or path.name in {'.DS_Store', 'ui-qa.png', 'ui-qa-narrow.png'}:
            continue
        if path.resolve() == target: continue
        if (relative.as_posix().startswith('tests/ui-') and path.suffix == '.png') or relative.as_posix() == 'tests/apple-generated.heic' or path.name == '.env' or (path.name.startswith('.env.') and path.name != '.env.example'):
            continue
        data = path.read_bytes()
        verify_source(relative, data)
        archive.writestr(f'{product_name}-{version}/{relative.as_posix()}', data)
  # The completed ZIP and destination share a filesystem. Linking publishes
  # the sealed archive atomically and refuses a destination created meanwhile.
  try:
    os.link(temporary, target)
  except FileExistsError:
    raise SystemExit('Refusing to replace an existing source package') from None
print(f'{target.name}: {target.stat().st_size:,} bytes')

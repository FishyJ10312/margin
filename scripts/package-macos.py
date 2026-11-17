"""Make a standalone Mac app: embed non-system dylibs, relocate and sign them."""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from distribution_privacy import verify_app

def command(*args):
    return subprocess.check_output([str(x) for x in args], text=True).strip()

def dependencies(path):
    lines = command('otool', '-L', path).splitlines()[1:]
    if path.suffix == '.dylib':
        lines = lines[1:]
    return [line.strip().split(' (')[0] for line in lines]

parser = argparse.ArgumentParser()
parser.add_argument('app', type=Path)
parser.add_argument('--destination', type=Path, required=True)
parser.add_argument('--native-dir', type=Path, action='append', default=[])
parser.add_argument('--native-sources', type=Path, help='Directory containing the verified libheif/libde265 source archives')
args = parser.parse_args()
app = args.app.resolve()
output = args.destination.resolve()
target = output / app.name
project = Path(__file__).resolve().parents[1]
native_sources = args.native_sources or project / '.native/macos'
source_records = {
    'libde265.tar.gz': ('libde265-1.1.3.tar.gz', '554228bd17788c99a7e63b37ab5634722190e6e2bf60c1dcb01cef328e133905'),
    'libheif.tar.gz': ('libheif-1.23.7.tar.gz', 'f7483e818503ea6224c8863e4a56ac02c7089021a98472c811c12aa19faf2475'),
}
for filename, (_, expected) in source_records.items():
    source = native_sources / filename
    if not source.is_file() or hashlib.sha256(source.read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'Missing or mismatched corresponding native source archive: {filename}')
if not (project / 'licenses/THIRD-PARTY-NOTICES.txt').is_file():
    raise RuntimeError('Collect dependency notices before packaging')
if (not (app / 'Contents/MacOS/liubai').is_file() or target == app
        or output.is_relative_to(app) or app.is_relative_to(target)):
    raise RuntimeError('Expected a built app and a separate packaging destination')
if target.is_symlink() or (target.exists() and not target.is_dir()):
    raise RuntimeError('Packaging destination must be an app directory, not a link or file')
for required in ['licenses/dependency-inventory.json', 'docs/native-components.md']:
    if not (project / required).is_file():
        raise RuntimeError(f'Missing release material: {required}')
output.mkdir(parents=True, exist_ok=True)
published_target = target
staging = tempfile.TemporaryDirectory(prefix='.margin-package-', dir=output)
target = Path(staging.name) / app.name
shutil.copytree(app, target, symlinks=True)
executable = target / 'Contents' / 'MacOS' / 'liubai'
notices = target / 'Contents' / 'Resources' / 'Licenses'
notices.mkdir(parents=True, exist_ok=True)
for source in [project / 'THIRD-PARTY.md', project / 'src-tauri/resources/FONT-LICENSE.txt', *(project / 'licenses').glob('*.txt')]:
    if source.is_file():
        shutil.copy2(source, notices / source.name)
shutil.copy2(project / 'licenses/dependency-inventory.json', notices / 'dependency-inventory.json')
sources = notices / 'NativeSources'
sources.mkdir(exist_ok=True)
for filename, (output_name, _) in source_records.items():
    shutil.copy2(native_sources / filename, sources / output_name)
shutil.copy2(project / 'scripts/build-macos.sh', sources / 'build-macos.sh')
shutil.copy2(project / 'docs/native-components.md', sources / 'README.md')
(sources / 'SHA256SUMS.txt').write_text(''.join(f'{digest}  {name}\n' for name, digest in source_records.values()))
frameworks = target / 'Contents' / 'Frameworks'
frameworks.mkdir(exist_ok=True)
copied = {}
pending = [(executable, executable)]
while pending:
    binary, origin = pending.pop()
    for dep in dependencies(binary):
        if dep.startswith(('/usr/lib/', '/System/Library/')):
            continue
        if dep.startswith('@rpath/'):
            candidates = [directory / dep.removeprefix('@rpath/') for directory in args.native_dir]
            candidates.extend([origin.parent / Path(dep).name, frameworks / Path(dep).name])
            source = next((p.resolve() for p in candidates if p.is_file()), None)
            if source is None:
                raise RuntimeError(f'Cannot resolve native dependency: {dep}')
        elif dep.startswith('@loader_path/'):
            source = (origin.parent / dep.removeprefix('@loader_path/')).resolve()
        elif dep.startswith('/'):
            source = Path(dep).resolve()
        else:
            raise RuntimeError(f'Unsupported native dependency: {dep}')
        name = Path(dep).name
        destination = frameworks / name
        if str(source) not in copied:
            if destination.exists():
                raise RuntimeError(f'Conflicting native library: {name}')
            shutil.copy2(source, destination)
            destination.chmod(0o755)
            copied[str(source)] = destination
            command('install_name_tool', '-id', f'@rpath/{name}', destination)
            pending.append((destination, source))
        command('install_name_tool', '-change', dep, f'@rpath/{copied[str(source)].name}', binary)

if '@executable_path/../Frameworks' not in command('otool', '-l', executable):
    command('install_name_tool', '-add_rpath', '@executable_path/../Frameworks', executable)
for library in frameworks.glob('*.dylib'):
    if '@loader_path' not in command('otool', '-l', library):
        command('install_name_tool', '-add_rpath', '@loader_path', library)
    command('codesign', '--force', '--sign', '-', library)
command('codesign', '--force', '--deep', '--sign', '-', target)
command('codesign', '--verify', '--deep', '--strict', target)
remaining = {str(published_target / p.relative_to(target)): dependencies(p) for p in [executable, *frameworks.glob('*.dylib')]}
for deps in remaining.values():
    if any(x.startswith(('/opt/', '/Users/')) for x in deps):
        raise RuntimeError('App still depends on development-machine paths')
verify_app(target)
# Expose the finished name only after dependency relocation and signing pass.
# Keep the previous package intact on all preparation failures, and restore it
# if the final move fails. This is a packaging transaction, not an app updater.
previous = Path(staging.name) / 'previous.app'
if published_target.exists():
    published_target.rename(previous)
try:
    target.rename(published_target)
except BaseException:
    if previous.exists():
        previous.rename(published_target)
    raise
staging.cleanup()
print(json.dumps({'app': str(published_target), 'nativeLibraries': remaining}, ensure_ascii=False, indent=2))

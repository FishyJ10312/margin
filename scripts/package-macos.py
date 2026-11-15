"""Make a standalone Mac app: embed non-system dylibs, relocate and sign them."""
from pathlib import Path
import argparse
import json
import shutil
import subprocess

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
args = parser.parse_args()
app = args.app.resolve()
output = args.destination.resolve()
output.mkdir(parents=True, exist_ok=True)
target = output / app.name
if target.exists():
    shutil.rmtree(target)
shutil.copytree(app, target, symlinks=True)
executable = target / 'Contents' / 'MacOS' / 'liubai'
project = Path(__file__).resolve().parents[1]
notices = target / 'Contents' / 'Resources' / 'Licenses'
notices.mkdir(parents=True, exist_ok=True)
for source in [project / 'THIRD-PARTY.md', project / 'src-tauri/resources/FONT-LICENSE.txt', *(project / 'licenses').glob('*.txt')]:
    if source.is_file():
        shutil.copy2(source, notices / source.name)
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
remaining = {str(p): dependencies(p) for p in [executable, *frameworks.glob('*.dylib')]}
for deps in remaining.values():
    if any(x.startswith(('/opt/', '/Users/')) for x in deps):
        raise RuntimeError('App still depends on development-machine paths')
print(json.dumps({'app': str(target), 'nativeLibraries': remaining}, ensure_ascii=False, indent=2))

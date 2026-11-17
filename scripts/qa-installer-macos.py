#!/usr/bin/env python3
"""Create, mount and copy a local DMG without launching or installing its app.

The output must be a new directory. Native operation checks use a separately
built QA identity; never launch a copied production identity over a user session.
Reports contain local paths and are not approved for automatic upload.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import stat
import subprocess
import sys

from distribution_privacy import verify_app


def command(*arguments, timeout=90):
    return subprocess.check_output(
        [str(value) for value in arguments], stderr=subprocess.STDOUT,
        timeout=timeout).decode('utf-8')


def tree(root):
    records = {}
    for path in sorted(root.rglob('*')):
        name = path.relative_to(root).as_posix()
        if path.is_symlink():
            records[name] = {'link': os.readlink(path)}
        elif path.is_file():
            records[name] = {
                'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                'mode': stat.S_IMODE(path.stat().st_mode),
                'bytes': path.stat().st_size,
            }
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != 'darwin':
        raise SystemExit('This acceptance requires a real Mac')
    app = args.app.resolve()
    output = args.out.resolve()
    if output.exists() or output.is_relative_to(app) or app.is_relative_to(output):
        raise SystemExit('Use a new output directory separate from the input app')
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    if info['CFBundleIdentifier'] != 'com.fishyj.liubai' and not info['CFBundleIdentifier'].startswith('com.fishyj.margin.qa.'):
        raise SystemExit('Expected an explicitly identified Margin build')
    output.mkdir(parents=True, mode=0o700)
    staged = output / 'staged'
    staged.mkdir()
    command('ditto', app, staged / app.name)
    (staged / 'Applications').symlink_to('/Applications')
    image = output / (app.stem + '-' + info['CFBundleShortVersionString'] +
                      '-' + platform.machine() + '-local-candidate.dmg')
    command('hdiutil', 'create', '-volname', app.stem, '-srcfolder', staged,
            '-format', 'UDZO', '-imagekey', 'zlib-level=9', image)
    (output / 'dmg-verify.log').write_text(command('hdiutil', 'verify', image))
    mount = output / 'mounted'
    mount.mkdir()
    copied = output / 'copied' / app.name
    try:
        attach = subprocess.check_output([
            'hdiutil', 'attach', '-readonly', '-nobrowse', '-noautoopen',
            '-mountpoint', str(mount), '-plist', str(image)], timeout=90)
        entities = plistlib.loads(attach)['system-entities']
        if not any(entity.get('mount-point') == str(mount) for entity in entities):
            raise RuntimeError('The image did not mount at the owned mountpoint')
        baseline = tree(app)
        if tree(mount / app.name) != baseline:
            raise RuntimeError('Mounted app differs from the prepared build')
        if os.readlink(mount / 'Applications') != '/Applications':
            raise RuntimeError('The installer shortcut has an unexpected target')
        copied.parent.mkdir()
        command('ditto', mount / app.name, copied)
        if tree(copied) != baseline:
            raise RuntimeError('Copied app differs from the image')
        command('codesign', '--verify', '--deep', '--strict', copied)
        executable = copied / 'Contents/MacOS' / info['CFBundleExecutable']
        architectures = command('lipo', '-archs', executable).strip()
        signature = command('codesign', '--display', '--verbose=2', copied)
        dependencies = {}
        for binary in [executable, *sorted((copied / 'Contents/Frameworks').glob('*.dylib'))]:
            values = [line.strip().split(' (')[0] for line in command('otool', '-L', binary).splitlines()[1:]]
            if any(value.startswith(('/Users/', '/opt/', '/usr/local/')) for value in values):
                raise RuntimeError('Copied app depends on a development-machine library')
            dependencies[binary.relative_to(copied).as_posix()] = values
        verify_app(copied)
        if tree(app) != baseline:
            raise RuntimeError('Prepared source app changed during acceptance')
    finally:
        if os.path.ismount(mount):
            command('hdiutil', 'detach', mount)
    if os.path.ismount(mount):
        raise RuntimeError('Owned image is still mounted')
    report = {
        'version': info['CFBundleShortVersionString'],
        'identifier': info['CFBundleIdentifier'],
        'image': str(image), 'copiedApp': str(copied),
        'dmgSha256': hashlib.sha256(image.read_bytes()).hexdigest(),
        'dmgBytes': image.stat().st_size,
        'executableSha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
        'appBytes': sum(record.get('bytes', 0) for record in baseline.values()),
        'architectures': architectures, 'signature': signature.strip(),
        'mountedAndCopiedFilesMatch': True, 'sourceAppUnchanged': True,
        'signatureVerified': True, 'binaryPrivacyPassed': True,
        'applicationsShortcutVerified': True, 'imageDetached': True,
        'nativeDependencies': dependencies, 'appLaunchedByThisScript': False,
        'installedIntoApplications': False,
    }
    (output / 'installer-report.private.json').write_text(json.dumps(report, indent=2))
    (output / 'app-tree-manifest.json').write_text(json.dumps(baseline, indent=2))
    print(json.dumps({key: report[key] for key in [
        'version', 'identifier', 'dmgBytes', 'architectures',
        'mountedAndCopiedFilesMatch', 'signatureVerified', 'imageDetached']}))


if __name__ == '__main__':
    main()

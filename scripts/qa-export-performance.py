#!/usr/bin/env python3
"""Bounded paired queue comparison, using one current renderer and owned inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import shutil
import statistics
import subprocess
import time
import zipfile
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('--baseline-source', type=Path, required=True)
parser.add_argument('--native-prefix', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--pairs', type=int, default=5)
args = parser.parse_args()
if not 3 <= args.pairs <= 7:
    raise SystemExit('Use a finite 3-7 paired measurements per dataset')
root = Path(__file__).resolve().parents[1]
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
if shutil.disk_usage(output).free < 2 * 1024 ** 3:
    raise SystemExit('Keep at least 2 GiB free before this bounded test')
native = args.native_prefix.resolve() / 'lib'
env = dict(os.environ, PKG_CONFIG_PATH=str(native / 'pkgconfig'),
           DYLD_FALLBACK_LIBRARY_PATH=str(native), MACOSX_DEPLOYMENT_TARGET='12.0')
lock = (root / 'src-tauri/Cargo.lock').read_text()


def version(name):
    return re.search(r'\[\[package\]\]\nname = "' + name + r'"\nversion = "([^"]+)"', lock)[1]


def instrument_storage(text):
    needle = '    pub fn save(&self, state: &StoredState) -> Result<(), String> {'
    assert text.count(needle) == 1
    wrapper = '''    pub fn save(&self, state: &StoredState) -> Result<(), String> {
        let started = std::time::Instant::now();
        let result = self.qa_save_impl(state);
        QA_SAVE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        QA_SAVE_NANOS.fetch_add(started.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        result
    }
    fn qa_save_impl(&self, state: &StoredState) -> Result<(), String> {'''
    return text.replace(needle, wrapper) + '''
static QA_SAVE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static QA_SAVE_NANOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn qa_reset_metrics() {
    QA_SAVE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    QA_SAVE_NANOS.store(0, std::sync::atomic::Ordering::Relaxed);
}
pub fn qa_metrics() -> (u64, u64) {
    (QA_SAVE_COUNT.load(std::sync::atomic::Ordering::Relaxed), QA_SAVE_NANOS.load(std::sync::atomic::Ordering::Relaxed))
}
'''


subjects = {}
provenance = {}
with zipfile.ZipFile(args.baseline_source) as archive:
    for subject in ['baseline', 'candidate']:
        folder = output / subject
        (folder / 'src').mkdir(parents=True)
        (folder / 'src/main.rs').write_bytes((root / 'scripts/qa-export-bench.rs').read_bytes())
        records = {}
        for filename in ['jobs.rs', 'app_models.rs', 'storage.rs']:
            relative = f'src-tauri/src/{filename}'
            original = (archive.read('Margin-0.11.2/' + relative).decode()
                        if subject == 'baseline' else (root / relative).read_text())
            text = instrument_storage(original) if filename == 'storage.rs' else original
            (folder / 'src' / filename).write_text(text)
            records[filename] = {'originalSha256': hashlib.sha256(original.encode()).hexdigest(),
                                 'compiledSha256': hashlib.sha256(text.encode()).hexdigest(),
                                 'instrumentation': 'save duration/counter wrapper only' if filename == 'storage.rs' else None}
        provenance[subject] = records
        deps = '\n'.join([
            f'liubai_lib = {{ package = "liubai", path = {json.dumps(str(root / "src-tauri"))} }}',
            f'serde = {{ version = "={version("serde")}", features = ["derive"] }}',
            f'serde_json = "={version("serde_json")}"',
            f'uuid = {{ version = "={version("uuid")}", features = ["v4", "serde"] }}',
            f'tempfile = "={version("tempfile")}"',
            f'image = {{ version = "={version("image")}", default-features = false, features = ["jpeg", "png"] }}'])
        package = 'margin-queue-' + subject
        (folder / 'Cargo.toml').write_text(f'[package]\nname="{package}"\nversion="0.0.0"\nedition="2021"\n[dependencies]\n{deps}\n[profile.release]\nstrip=true\nlto="thin"\ncodegen-units=1\n')
        print(json.dumps({'stage': 'building', 'subject': subject}), flush=True)
        build = subprocess.run(['cargo', 'build', '--offline', '--release', '--manifest-path', str(folder / 'Cargo.toml'),
                                '--target-dir', str(root / 'src-tauri/target')], env=env, text=True, capture_output=True)
        (output / f'build-{subject}.log').write_text(build.stdout + build.stderr)
        if build.returncode:
            raise SystemExit(f'{subject} build failed; see its log')
        subjects[subject] = root / 'src-tauri/target/release' / package
(output / 'provenance.json').write_text(json.dumps(provenance, indent=2))

datasets = {}
for label, count, size in [('small', 12, (320, 240)), ('photo', 6, (2400, 1600))]:
    folder = output / 'inputs' / label
    folder.mkdir(parents=True)
    for index in range(count):
        # Seeded texture is owned test data; identical inputs feed both subjects.
        data = random.Random(731 + index).randbytes(size[0] * size[1] * 3)
        Image.frombytes('RGB', size, data).save(folder / f'input-{index:02}.jpg', quality=85)
    datasets[label] = folder
originals = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (output / 'inputs').rglob('*.jpg')}
results = []
pixel_reference = {}
for dataset, inputs in datasets.items():
    order = ['baseline', 'candidate']
    for pair in range(-1, args.pairs):
        for subject in order if pair % 2 == 0 else list(reversed(order)):
            label = f'{dataset}-warm-{subject}' if pair < 0 else f'{dataset}-pair-{pair}-{subject}'
            destination = output / 'runs' / label
            print(json.dumps({'stage': 'measuring', 'run': label}), flush=True)
            run = subprocess.run([str(subjects[subject]), str(inputs), str(destination)],
                                 env=env, text=True, capture_output=True, timeout=100)
            (output / f'{label}.log').write_text(run.stdout + run.stderr)
            if run.returncode:
                raise SystemExit(f'{label} failed; see log')
            report = json.loads((destination / 'report.json').read_text())
            with Image.open(next((destination / 'exports').glob('input-00-framed.jpg'))) as image:
                pixel_hash = hashlib.sha256(image.tobytes()).hexdigest()
            if dataset in pixel_reference:
                assert pixel_hash == pixel_reference[dataset], 'Queue variants must not change rendered pixels'
            pixel_reference[dataset] = pixel_hash
            if pair >= 0:
                results.append({'dataset': dataset, 'pair': pair, 'subject': subject, **report})
            (output / 'measurements.json').write_text(json.dumps(results, indent=2))
            retained = sum(p.stat().st_size for p in (output / 'runs').rglob('*') if p.is_file())
            if retained > 1024 ** 3:
                raise SystemExit('Stopped at the 1 GiB retained-output guard')
summary = {}
for dataset in datasets:
    rows = [r for r in results if r['dataset'] == dataset]
    medians = {s: statistics.median(r['exportMs'] for r in rows if r['subject'] == s) for s in subjects}
    ratios = []
    for pair in range(args.pairs):
        two = {r['subject']: r['exportMs'] for r in rows if r['pair'] == pair}
        ratios.append(two['candidate'] / two['baseline'])
    summary[dataset] = {'medianExportMs': medians, 'medianPairedRatio': statistics.median(ratios),
                        'pairedRatios': ratios, 'samplesPerSubject': args.pairs,
                        'medianJournalMs': {s: statistics.median(r['journalMs'] for r in rows if r['subject'] == s) for s in subjects},
                        'journalSaves': {s: sorted(set(r['journalSaves'] for r in rows if r['subject'] == s)) for s in subjects}}
assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest() == h for p, h in originals.items())
(output / 'report.json').write_text(json.dumps({'datasets': summary, 'sourceFilesUnchanged': True,
    'representativeOutputPixelsMatch': True, 'measurements': results,
    'scope': '0.11.2 queue/storage versus current queue/storage, both using the exact current renderer/library; save timing instrumentation only in isolated benchmark copies. Imports excluded; one warm-up each; alternated order; finite paired runs; shared host, not whole-version/native-UI performance.'}, indent=2))
print(json.dumps({'stage': 'complete', 'datasets': summary}), flush=True)

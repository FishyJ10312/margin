# Margin 0.11.3 working revision: interaction and export acceptance

Measured 2026-11-17 on Apple M4 Pro / 48 GB / macOS 27.2 / arm64.
This continues [the first stability phase](stability-0.11.3.md). Version remains
0.11.3; this is a separately pinned working-source revision. The earlier
production DMG/source archive were preserved and do **not** contain this phase's
queue optimization. Only the isolated RC was rebuilt. The user's installed
0.11.2 app and actual workspace were not replaced.

## Resulting production change

Starting each photo previously saved its running state, then immediately saved
again to register the private temporary output. The second save already
contains both fields. The queue now commits them together, before any image
pixels are written. Before that commit, the durable item can remain pending and
resume safely. The source change removes one redundant save per photo, without
removing the temporary-file ownership record or making output commits weaker.

This is the only production-code change in this phase (`src-tauri/src/jobs.rs`).
All 37 frontend/package files compared with the pre-phase source archive match
byte for byte. Pure-white template samples, the gallery UI, watermark preview
and authentication UI are unchanged. New QA programs are development tools,
not bundled runtimes or application dependencies.

## Current fixture and core coverage

- Startup / optional Pi fixtures: 7 passed, no live credentials or model calls.
- Four existing browser suites passed: grouped drop/import, font keyboard and
  scroll containment, contextual watermark workspace, and offline login /
  outage / cancel / account switching. The runner rewrites only the task-space
  name and screenshot destinations, retaining all original assertions. One Ego
  space was used and finished once. These are real UI with mocked native replies,
  not native file-drop or real sign-in acceptance. Ego Lite 0.6.2.9 reported an
  available update; no upgrade was performed.
- After the queue change: 112 Rust tests passed, 0 failed, 1 optional legacy
  OS-vault test remains intentionally ignored. Includes persistence failures,
  cancellation, restart, retry, colors, fonts and original-file preservation.
- The journal-write check was red on the measured pre-change report (50 versus
  38 writes, and 26 versus 20). It passes on the post-change report.
- Real SIGKILL of a private QA export child after observing a 7,605,544-byte
  temporary write: both files recovered, existing output preserved, zero
  remaining temporary files, original copies unchanged. Not a power-loss test.
- Child-only 8192-byte file-size quota: one export failed with EFBIG, the next
  small photo completed, and retry succeeded after restoring the quota. No
  physical disk was filled or system RAM exhausted.

## Paired export measurements

The comparator uses actual 0.11.2 queue/storage modules from the preserved source
archive and the working queue/storage modules, both linked to the **same current
image renderer**. This isolates queue cost; it is not a whole-version or native
UI benchmark. A timing/counter wrapper exists only in isolated copies of
`Storage::save`. Imports and output validation are excluded from the timed
export interval. Instrumented source snapshots and hashes are retained.

Each dataset has one warm-up per subject followed by five paired runs, with
alternating order, identical seeded JPEG inputs and fresh output directories.
No logged native UI actions overlap the conservative measurement windows.
The host is shared, so these are bounded local comparisons, not a dedicated
performance lab or a universal speed claim.

| Dataset | Before consolidation | After consolidation | Journal saves per batch |
| --- | --- | --- | --- |
| 12 × 320×240 JPEG | Median paired ratio 1.263; candidate median 493.07 ms | Ratio 1.028; candidate median 414.16 ms | Candidate 50 → 38; baseline 38 |
| 6 × 2400×1600 JPEG | Ratio 1.040; candidate median 915.43 ms | Ratio 0.990; candidate median 888.70 ms | Candidate 26 → 20; baseline 20 |

Ratios are candidate/baseline within each paired comparison. After-consolidation
baseline medians were 397.01 ms and 892.38 ms. The remaining small-image ratios
range 0.956–1.064; do not interpret a few percent as a proven regression or
improvement. Every output's dimensions were validated, a representative decoded
output per trial matched pixels across subjects, and input hashes were unchanged.
Each measurement set retained about 412 MiB, within its 1 GiB guard; no cleanup
of user data was performed.

## Native interaction and process memory

The RC was launched through the normal Mac application mechanism rather than
as a shell child. Its backend was self-responsible, and `launchctl print
pid/<backend>` explicitly identified its WebContent, GPU, Networking and other
XPC services. A normal PPID tree alone misses launchd-managed services; see
[Apple's XPC overview](https://developer.apple.com/documentation/xpc).

The QA monitor samples that PID domain plus direct descendants every two
seconds. Public `proc_pid_rusage` supplies per-process physical-footprint
accounting. A dynamically resolved private responsibility query is used only
by this local QA tool as a cross-check; it is not added to the product.
Both completed runs had zero PID-domain query failures, zero observed
responsibility mismatches and no missing footprint observations.

| Run | Work actually observed | Sampled RSS sum max | Per-process footprint sum max / last |
| --- | --- | --- | --- |
| Pre-consolidation RC, 20 minutes | Repeated fractional zoom and padding, Chinese keyboard font changes, watermark size/offset/folding, ratio changes, 30 white template cards, keyboard pixel picking, 64 real native exports | 770.02 MiB | 808.60 / 351.46 MiB |
| Rebuilt consolidated RC, 10 minutes | Native cancel after 7 complete, 57 cancelled, resume all 64; fractional zoom, HEIC preview and Chinese font changes; ready preview after the session | 747.27 MiB | 576.99 / 299.80 MiB |

These are sessions with intermittent interaction bursts, not continuous-input
stress. The sequences and durations differ; no memory improvement is attributed
to the queue change. RSS sums can double-count shared pages. Footprint sums are
OS process accounting, not total physical RAM or VRAM. Associated GPU-process
memory is observed, but complete GPU device allocations, unassociated shared
system services, sub-sample peaks and OS-wide memory pressure remain unmeasured.
Automation round-trip durations include CUA overhead and are not input latency
or frame-rate measurements. No crash or 2 GiB QA-group guard trip occurred in
either completed run; this does not establish a universal whole-app ceiling.

In the optimized native run, the UI padding was changed from 5% to 7% while
paused. Resume preserved the original 5% batch snapshot: the first output stayed
4300×3300. All 64 final files have the recorded dimensions and an sRGB ICC
profile, with no duplicates or temporary files. The thumbnail cache remained
64 files / 159,240 bytes. Inputs remained 64 files / 561,548 bytes. Expected
export files grew from 64 to 128 across two separate QA output folders; settings
and journal remained two files (92,434 to 92,304 bytes). Export-output growth
must not be described as a growing preview cache.

## Protected-path observation at the end of phase2

The subsequent [read-recovery phase](read-recovery-0.11.3.md) now records a
controlled path/restart/chooser comparison and bounded recovery. The text below
retains the limitations of this earlier phase; it is not the latest gate status.

One LaunchServices start with manually seeded state pointing at synthetic
files under Documents stayed at “Updating preview” for more than seven minutes.
A sample showed one worker in `open` and another waiting for its image gate;
the same file was immediately readable by the development process. The app was
intentionally quit, not observed to crash. An unprivileged debugger attachment
timed out; no access rights were expanded.

The same synthetic bytes, names and settings under the isolated app-data folder
produced a ready preview after relaunch, and completed the native tests above.
However, a file-picker selection and restart also occurred during investigation.
There is no restart-only A/B or confirmed TCC denial, so a permission/path cause
is **not established**. No system permission was changed. This phase does not
claim to have fixed that stall. Normal selected-file restore on a suitable clean
test environment, and recoverable handling of blocked source reads, remain
release gates. Do not infer general native startup readiness from the successful
app-data fixture runs.

Two automation issues are recorded separately from app defects: the first batch
finished before a later Cancel click, so its stale target was rejected; a helper
retained an old app binding after swapping RC copies and timed out. Fresh
bindings completed the actual cancel/resume and setting checks. Neither event
is counted as successful cancellation or as an application crash.

## Evidence and reproduction

Evidence root: `outputs/Margin-active-2026-11-17/` beside the project workspace.
It includes fixture logs/screenshots, paired-run reports and source provenance,
native event logs and process samples, crash/I/O proofs, build identities and
workspace fingerprints. The rebuilt native executable's SHA256 is
`d18c3c53413cdbadcf36ba53b3658dc99119bd0674f005af669238b23a52ca50`.

```sh
node --test tests/pi-bridge.test.mjs tests/startup.test.ts
# Start the local Vite fixture server first.
python3 scripts/qa-fixture-suite.py --output /absolute/new-fixture-report --space "Margin isolated acceptance"
python3 scripts/qa-export-performance.py --baseline-source /absolute/Margin-0.11.2-source.zip --native-prefix /absolute/native/install --output /absolute/new-benchmark --pairs 5
python3 scripts/qa-check-export-journal.py /absolute/new-benchmark/report.json
```

Source/patch artifacts have a `phase2` qualifier to distinguish them from the
earlier 0.11.3 installer. The native RC has a separate bundle identifier and no
authentication records. Current-user app-data fingerprints match for all
57 files / 202,390,800 bytes. No model request, photo upload, credits, payment,
new OS account, privilege expansion, GitHub push or public release occurred.

Public release is still gated by the protected-path observation, current-build
consented live model acceptance, actual iPhone/gain-map inputs, clean-user /
minimum-OS and supported-architecture decisions, signing/notarization and a real
Windows device. See [the release checklist](release-readiness.md).

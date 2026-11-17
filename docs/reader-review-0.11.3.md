# Margin local candidate: reader review, 2026-11-17

This is a separate local candidate of the existing 0.11.3 working source. The
version number was not bumped. `Margin Candidate.app` uses the independent
identity `com.fishyj.margin.qa.candidate20261117` and window title
`Margin · Local Candidate`. The installed 0.11.2 application and the older
production 0.11.3 DMG were preserved. The older DMG does not contain this revision.

The baseline for this review is `Margin-0.11.3-read-recovery-source.zip`, not the
production installer. Measurements use the current Apple M4 Pro / 48 GB /
macOS 27.2 / arm64 Mac. No external model, private-photo upload, credit, payment,
publication or system privacy-setting change was involved.

## Findings fixed

1. **Retired reader records accumulated during repeated failures.** The helper
   processes, pipe descriptors and threads were reaped, but weak buffer records
   were swept only when a later successful decode checked its memory budget.
   Twenty-four timeout/cancel cycles followed by metadata/byte retries left 24
   dead records. Retirement and slot release now sweep them; the final retained
   allocation is dropped before making the slot available.
2. **An obsolete successful preview could be returned or displayed.** The backend
   checked its generation only before entering the renderer. The frontend
   accepted a completed render when the photo ID matched, even if settings had
   changed or selection had gone A → B → A. Native generation checks now also
   guard completion. Frontend result and thumbnail publication require the exact
   current revision. The previous valid view stays visible while its replacement
   is pending; an obsolete image is never published.
3. **An old timeout could undo explicit retry.** Clearing the one-second failure
   cache did not prevent an older in-flight failure from filling it again after
   the source had already been successfully reselected. Cache generations now
   discard failures from requests preceding the explicit clear.

Each finding has failure evidence against the pre-fix call path and passing
regression evidence. The backend stale-render test releases a real blocked OS
open only after a newer request arrives. The frontend test holds native replies
at the real Svelte preview pump and observes actual image-source mutations,
including A → B → A. It is a browser fixture with mocked native replies, not a
claim about real account access or native drag events.

No product layout, markup, styles or model UI was changed. The App markup/style
suffix is byte-identical to the baseline. The candidate's name, title and bundle
identity come from `src-tauri/tauri.candidate.json`. No dependency was added.

## Bounded resource regression

- Final Rust suite: **119 passed, 0 failed, 1 optional legacy vault test ignored**.
- Five fresh-process repeats each perform 12 real blocked-open timeouts, 12
  cancellations and 40 healthy byte retries. All 120 retired children were
  confirmed gone. Each repeat starts and ends at 6 file descriptors, 3 threads,
  1 idle reusable helper, and 0 retired records. Before the fix, records grew
  0 → 24 while descriptors and threads stayed at 6 / 3.
- Sixteen additional partial-body cases use a valid JPEG with a bounded 4 MiB
  trailing payload. The worker streams a non-empty chunk before blocking in an
  owned FIFO. Eight timeouts and eight cancellations reap the child, release the
  partial input allocation, and leave zero abandoned buffer bytes. A subsequent
  complete read matches the source bytes.
- A reselect test succeeds through a second reader before the older blocked
  request times out, then verifies the old failure cannot poison the new read.
- A cancelled batch test explicitly reselects its original path without adding
  a duplicate, changes current UI settings, reloads state, and verifies resume
  still uses the frozen 5% batch snapshot.

These are bounded tests with owned synthetic files, at most two reader slots and
small fixed allocations. They do not exhaust host RAM, fill a physical disk or
change OS permissions. Descriptor/thread counts apply to the isolated test
process; they are not Windows HANDLE or all Mac Mach-port counts. They do not
prove every kernel operation, worker startup or complete preview has an
eight-second wall-clock ceiling. The implemented deadline bounds the controlled
source-reader wait; decoding and output operations retain their own boundaries.

## Current candidate acceptance

- Svelte check: 0 errors / 0 warnings; seven startup/Pi fixtures passed.
- Five browser suites passed in one finished Ego task space: stale preview,
  grouped import, contained font scrolling/keyboard changes, contextual watermark
  workspace and offline login/cancellation/account switching. Existing four
  suites retained their assertions; only task-space/screenshot destinations were
  adapted. Ego Lite reported an available update; it was not upgraded.
- The actual candidate from the DMG was launched with its independent identity.
  Its initial workspace was empty and language English. The real native chooser
  imported 64 synthetic JPG/PNG/HEIC files, including eight 4000×3000 sources.
- Cancel left 57 completed / 7 cancelled. Current padding changed 5% → 7%.
  Reselecting the existing source added zero photos and retained the frozen 5%
  queue. The app was quit normally and reopened; Resume completed only the seven
  remaining files. All 64 output dimensions and ICC profiles were checked, the
  original 57 output hashes stayed identical, and no duplicate or temporary file
  remained. Every source copy's hash matched its recorded baseline.
- The UI preview at 7% is 4420×3420, while the large batch outputs stay 4300×3300.
  Further native 6.5%, 7.5%, 7% changes produced 4390×3390, 4450×3450, 4420×3420
  previews. These are functional checks, not frame-rate or input-latency measures.
- The native template library still displays pure-white sample centres. The
  candidate was closed and the user's existing 0.11.2 template library reopened.
- Current optimized core acceptance also passed: 53 mixed-image exports with
  cancel/reload/retry, 80 previews without disk-cache growth, the 14-case HEIC
  matrix, child-only 8192-byte EFBIG isolation/retry, and real private SIGKILL
  recovery after observing a 7,605,543-byte export temp. Two outputs recovered,
  the prior output and sources survived, and temporary files were removed.

The first multi-select automation attempt selected one row; Shift-range selected
the full set and the native import reported 64. It is not counted as an import
failure. A first observer stopped when an atomic journal rename removed a temp
between enumeration and `stat`; it was an observation-tool race, not an app
crash. The QA collector now records one successful stat per file and skips only
files that disappear. Original partial samples and the error are preserved.

A corrected 90-second post-resume observation completed with 45 samples and zero
PID-domain query failures. It includes three native fractional-padding changes.
The backend plus attributed WebKit/GPU/networking services and helper descendants
peaked at a sampled RSS sum of 508.52 MiB and footprint sum of 532.57 MiB. The
thumbnail cache stayed 64 files / 160,532 bytes. This does not cover the batch's
peak, all physical RAM/VRAM, OS pressure, sub-sample peaks or a universal ceiling.
RSS can double-count shared pages. No performance improvement is inferred from
different earlier workloads.

## Candidate and source provenance

Local DMG: `Margin-0.11.3-local-candidate-reader-review-20261117-arm64.dmg`.
The image passed `hdiutil verify`, was mounted read-only, and its application was
copied into a separate QA folder. The copied bundle matches the staged bundle
byte for byte and passes strict ad-hoc signature verification. Native acceptance
above used that copy, not an older package or the user's installed app.

- DMG SHA256: `aa4528b05bd5847df0df4b0b8c9115e6e773258e65796e197c725c67e1a514e0`
- Executable SHA256: `d644f90acd5ab41e0d5c52d8ae89a92afac5ef68c805306cb579739b1f4f249e`
- Build-input manifest SHA256: `a859e6d8062cd73b6ef219ecf981cf841ecbb9340d8ed28cb4c0d860702af917`
- DMG bytes: 27,479,775. Native code and component source/license archives remain
  bundled; no Node/Pi runtime or additional executable payload was introduced.

The embedded `LOCAL-CANDIDATE.json` names the candidate and pins 91 build inputs.
The external source archive, patch, change manifest and SHA list bind its exact
source to those inputs. The default candidate successfully streams bytes with
both QA fault markers present, proving the fault hooks are absent in its build.
The production version number and old artifacts were not rewritten.

Evidence directory: `outputs/Margin-reader-review-2026-11-17/` beside the workspace.
It retains red/green logs, fresh-process resource records, all five UI results,
native screenshots/journals, source/output hashes, package provenance and the
separate phase status. No personal photos or credentials belong in the archive.

This is a locally verified candidate, not a public-ready release. Clean OS-user
and minimum-macOS/Intel decisions, actual iPhone/gain-map and Windows devices,
consented current-build model acceptance, distribution review and Developer ID /
notarization remain external release conditions. No automatic replacement of
the user's application or system-security change was used to pass a gate.

## Reproduce without real accounts

Use the native build environment from `scripts/build-macos.sh`. On the current
Mac, the complete core regressions run with:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1
# Run the local fixture server in another terminal.
npm run dev -- --port 1420
python3 scripts/qa-fixture-suite.py --output /absolute/new-qa-folder --space "Margin fresh reader review"
# A separate candidate name/identity; no version-number edit is needed.
npm run tauri -- build --bundles app --config src-tauri/tauri.candidate.json
```

The fixture runner now includes all five suites and closes its one task space
once. Native libraries and matching component sources are packaged with
`scripts/package-macos.py`; its source/archive guards must remain enabled.

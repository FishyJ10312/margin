# Margin 0.11.3 local stability record

Follow-up working-source checks and queue-write consolidation are recorded in
[active-acceptance-0.11.3.md](active-acceptance-0.11.3.md). The production DMG
described below was preserved; it predates that separately pinned source change.

Measured on 2026-11-17, Apple M4 Pro / 48 GB / macOS 27.2 / arm64.
This is an internal candidate, not public-release acceptance. The current user
installation remains 0.11.2; candidate testing used separate app identities and
synthetic-only profiles. No user photo was uploaded and no model request or
credit expenditure was made in this phase.

## Changes backed by failing cases

1. **PNG color interpretation.** An HDR PNG was previously accepted as SDR,
   and linear sample value 128 was exported near 128 instead of sRGB 188.
   Import and rendering now read bounded PNG metadata, reject unsupported
   PQ/HLG, and convert supported Display P3 / gAMA / cHRM data through Little
   CMS. cICP takes precedence over ICC, then sRGB, then legacy gAMA/cHRM,
   following the [PNG specification](https://www.w3.org/TR/png-3/#colour-chunk-precedence).
   Metadata parsing has an 8 MiB budget. The directly pinned png 0.18.1 was
   already a transitive dependency; no additional runtime or package was added.
2. **Ambiguous high-bit-depth HEIC.** A real, synthetic Apple ImageIO 10-bit
   file with unspecified color information was previously treated as sRGB.
   Above 8 bits, input now requires reliable ICC or explicit supported color
   information. A matching positive 10-bit sRGB case still succeeds. Unsupported
   input is isolated with an English or Chinese explanation.
3. **Interrupted exports.** A QA process killed during a 7,605,542-byte write
   left its temporary output behind after restart. Each new export now records
   its exact temporary path in the queue before writing pixels. On recovery,
   cleanup accepts only a regular file in the recorded output directory with
   that job's UUID prefix. Symlinks, directories, originals, foreign paths and
   unrecorded legacy files are left alone. The repeated kill/resume case left
   zero temporary files and completed both exports.
4. **Packaging failure.** A missing native library previously replaced the
   prior package with an incomplete copy. The packaging script now relocates,
   signs and verifies a private staged app before publishing its final name;
   preparation failures preserve the previous app. An injected missing-library
   case verifies that behavior. This is a packaging transaction, not an app
   updater or a power-loss guarantee.

The scope of the source changes can be reviewed against the 0.11.2 source
archive using the separate patch and manifest beside the candidate artifacts.
The gallery layout, authentication UI and white template samples are unchanged.

## Automated and native checks

| Check | Observed result | Limit |
| --- | --- | --- |
| Rust library regression | 112 passed, 0 failed, 1 intentionally ignored legacy OS-vault test | Current Mac build only; optional vault test was not run |
| Frontend / release build | Svelte 0 errors, 0 warnings; production and isolated RC app built | Does not substitute for native UI testing |
| HEIC matrix | 14 Apple-generated files: 11 accepted, 3 rejected as intended; 8 orientation references match with mean absolute pixel error 0 | Synthetic Apple ImageIO output, not actual iPhone capture or HDR gain-map coverage |
| HEIC color / alpha | P3 ICC, alpha and explicit 10-bit sRGB accepted; ambiguous 10-bit, PQ and HLG rejected | Supported static primary images only |
| PNG color | P3 output agrees with independent matrix reference within tolerance; linear 0.5 becomes about sRGB 188; ICC/sRGB precedence checked | Unsupported color encodings are rejected, not tone mapped |
| Native RC batch | Actual Mac import and export of 64 valid JPG / PNG / HEIC files; 5 unsupported inputs rejected independently; all 64 outputs checked for dimensions and sRGB ICC | Synthetic media and isolated profile; no new live model inference |
| Workspace recovery | 53 valid mixed inputs; corrupt/text/directory/duplicate isolation; cancel after one, reopen, resume, damage/repair/retry; 53 final outputs, no duplicate or temporary files | Backend reload test, separate from real process kill |
| Real process interruption | Private QA child SIGKILL during observed temporary write; new process resumed successfully; original copies and existing output sentinel unchanged | Not the user's app, not power loss |
| Storage error | JPG/PNG writers injected with StorageFull error; child-only 8192-byte file-size limit causes EFBIG, next small photo succeeds, retry succeeds after restoring limit | No physical disk was filled; injected ENOSPC-like writer and EFBIG are separate mechanisms |
| Admission / allocation guards | Sparse 129 MiB input, excessive PNG dimensions, allocation capacity overflow rejected | System RAM was not exhausted |
| Bounded malformed corpus | 87 deterministic PNG/HEIC header, truncation and bit mutations: 24 accepted, 63 rejected, 0 crashes / timeouts / invariant failures; highest child peak RSS 15.69 MiB | Tiny owned inputs; 3 s / 256 MiB per-case guard; not exhaustive fuzzing or a security audit |

## Duration, memory and disk growth

Three different measurements must not be combined into a claim that the final
packaged app passed a full hour of active use:

| Run | Duration / work | Backend RSS | Storage / interaction |
| --- | --- | --- | --- |
| 0.11.2 native control, isolated clean profile | 60 minutes; 51 synthetic photos, Chinese text, keyboard font changes and preview operations | Sampled max 279.27 MiB; last 119.39 MiB | State 136,042 to 146,920 bytes after deliberate setting changes; native zoom still responds after the hour |
| Fixed renderer, release core | 30 minutes; 1,793 previews, contextual watermark details and sampler tiles, paced at about one iteration/second, periodic exports | Sampled max 161.80 MiB; late warm 161.27 MiB; last 142.03 MiB | Exactly 2 retained outputs, 14,124,261 bytes; original copies unchanged |
| 0.11.3 native RC, synthetic-only profile | 20 minutes; mixed import, 64-photo export, history and subsequent HEIC / 150% zoom interaction | Sampled max 262.80 MiB; last 195.69 MiB | State 146,929 to 242,860 bytes while adding 13 photos and 64 export records; all exports finished, preview still responds |

These RSS values exclude WebKit/GPU and are samples, not whole-app peaks or a
universal memory ceiling. A separate vmmap observation of the RC backend gave
physical footprint 63.5M and peak 172.5M in that tool's units. A short high-rate
sampling window ended before the first export, so it is deliberately excluded
as evidence of export peak memory. Native UI survival and successful operations
are recorded separately from backend memory.

The active core's post-warm sampled preview median was 65.98 ms and p95 74.67 ms;
this includes the renderer and is not an input-to-display latency measurement.
An 80-preview workspace check added no disk cache files or bytes. State growth
in the native runs corresponds to intentional photo / settings / queue changes;
these short runs do not establish a long-term storage ceiling. No broad user
storage cleanup was performed.

The same 53-photo workspace check took 9.03 s with the retained 0.11.2 runner and
9.94 s with 0.11.3 on a shared, uncontrolled host. This is approximately 10%
slower in that spot check, not evidence of a performance improvement. Recording
temporary-file ownership adds one atomic journal save per export, a deliberate
correctness cost. A controlled performance comparison remains outstanding.

## Packaging and evidence

Production 0.11.3 is packaged separately; the user's installed 0.11.2 app and
workspace were not replaced. The RC uses the same production source with only
bundle identity / window-title overrides for isolation. The candidate is ad-hoc
signed and is not notarized. The final 26,507,003-byte DMG passed checksum
verification, was mounted read-only, and copied to a separate test directory.
All 50 regular files and any symlinks matched the mounted app; the copied app's
ad-hoc signature verified. The app contains 43,740,611 bytes. This copy was not
launched under the production identity because the user's existing app remains
open. Native startup/operation evidence uses the separate RC identity.

One early QA launch was attempted before packaging relocation had finished and
failed with missing rpath. This was a test scheduling mistake. The finished
package passed dependency / signature checks and was then successfully launched
for the native RC run. It is not counted as a successful first launch.

Local evidence root, relative to the enclosing workspace:
`outputs/Margin-stability-2026-11-17/`.

- `core-tests-0.11.3.log`, `build-0.11.3.log`, `build-rc-0.11.3.log`.
- `heic-results-0.11.3/report.json`, `native-export-verification.json` and
  `native-rc-format-errors.png`.
- `crash-before-fix/report.json`, `crash-after-fix/report.json`,
  `io-boundary-0.11.3/report.json`, `malformed-corpus/report.json`.
- `soak/native-monitor-report.json`, `core-soak/report.json`,
  `rc-soak/native-monitor-report.json`, and native screenshots after each soak.
- `workspace-0.11.3/report.json`, the two workspace timing records and
  `packaging-failure-before.json` / `packaging-failure-after.json`.
- `user-data-before.json` / `user-data-after.json` contain hashes and metadata,
  not copied credentials. They are local QA records, excluded from source ZIP.

Reproducers live in `scripts/qa-monitor.py`, `scripts/qa-crash-recovery.py`,
`scripts/qa-image-corpus.py`, `scripts/make-heic-fixtures.swift` and the associated
Rust examples. They use fresh isolated directories and owned synthetic data.
Native library environment setup follows `scripts/build-macos.sh`. The Python
crash runner requires its native-library-directory option on Mac because
protected Python may strip inherited DYLD environment variables.

## Still required before a public release

- Current-build consented analysis / refinement with a real model; no credits
  were used in this phase.
- Actual iPhone / gain-map inputs, whole-app active long sessions and controlled
  performance measurements; real device pressure if an appropriate test host
  becomes available.
- Fresh-user Gatekeeper, minimum supported macOS and Intel acceptance, or an
  explicit arm64-only support decision.
- User-provided Developer ID signing / notarization arrangements and final
  distribution-rights review.
- A connected Windows device for build, installation and real operation.

No paid enrollment, notarization upload, GitHub push, public publication, user
account creation, privilege expansion or unapproved cleanup was performed.

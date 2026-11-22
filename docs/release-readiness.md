# Margin 0.11.3 release candidate audit

The installed trial has since advanced to 0.11.4. The current evidence audit and
remaining gates are in [0.11.4 release gaps](release-gaps-0.11.4.md). The named
0.11.3 artifacts and table below remain historical evidence; they do not certify
the current version for public release.

Updated 2026-11-17. This is a local acceptance record, not a public release announcement.
“Passed” applies only to the evidence and platform named in that row.
The latest local artifact is the separately identified **privacy-review-20261117**
candidate. Its exact hashes, reproduced privacy/dispatch fixes, 120-test core
result, redacted diagnostic export boundary and remaining external decisions are
in [privacy-distribution-0.11.3.md](privacy-distribution-0.11.3.md). The reader-review
table below is historical evidence for its named artifacts; it must not be read
as fresh long-session or Windows acceptance of the newer candidate.

The current 0.11.3 working revision consolidates queue writes and bounds source-photo
reads; the existing production 0.11.3 DMG predates these changes. A separately named local candidate DMG now contains those changes plus the
[reader review fixes](reader-review-0.11.3.md). It has a separate application
identity and was not installed over the user app. Older production artifacts
remain historical evidence, not evidence for this candidate.

| Gate | Status | Evidence / next action |
| --- | --- | --- |
| Pure-white template samples | Passed, native Mac | All 30 generated sample centers are exactly RGB 255/255/255; actual Mac library inspected. Current-photo preview still shows the user's photograph. |
| Existing workspace preservation | Passed, native Mac | Current phase before/after fingerprints match for all 57 app-data files (202,390,800 bytes), including settings and saved-template records. No original photo or authorization was replaced. |
| Core regression | Passed, current Mac build | 119 Rust tests passed, 0 failed; 1 optional legacy OS-vault test intentionally ignored. Includes PNG HDR / color precedence, high-bit-depth HEIC admission, scoped temporary-file recovery, encoder I/O and allocation boundaries, plus existing framing / watermark / queue coverage. |
| Frontend checks | Current build and fixtures passed | Production frontend / isolated app rebuilt; 7 startup/Pi tests rerun and passed. Current Svelte check is 0 errors / 0 warnings. Current event logic also rejects obsolete preview/thumbnail completions. App markup/styles remain byte-identical to the read-recovery baseline; no UI arrangement or dependency changed. Five current browser suites and seven startup/Pi tests pass. These checks do not substitute for native operation. |
| Mixed image export | Passed, release core and current isolated native Mac | 53-photo core workspace completed. Initial 0.11.3 RC isolated 5 unsupported color/HDR inputs. Current consolidated RC cancelled after 7 exports and resumed the remaining 57: 64 final outputs, all dimensions / sRGB ICC verified, no duplicates or temporary files. Synthetic sources unchanged; actual iPhone capture / gain-map coverage remains separate. |
| Native current-photo batch export | Passed on installed 0.11.2; not repeated on 0.11.3 | Both current imported photos exported to separate QA directory: 8784×8784 and 8727×8727, embedded sRGB profiles; source SHA256 unchanged. 0.11.3 native acceptance used synthetic data and a separate profile. |
| Import/keyboard/watermark/login UI fixtures | Passed on current source, isolated fixtures | All four existing scripts rerun: drop, font-scroll, watermark-workspace and login. Original assertions retained; one browser task space closed after completion. Includes serial rapid font changes, contained scrolling, compact layout, cancellation, account isolation and outage/retry. No live credentials, photo uploads or model calls. |
| Mixed workspace, cancel/resume/retry | Passed, release core | New `verify_workspace` runner imports 53 valid mixed images plus corrupt/text/directory/duplicate inputs into isolated state; cancels after one completion, reloads saved queue, resumes, isolates an image damaged after import and retries it. 53 final outputs, original copies and pre-existing output preserved, no duplicate or temporary output left. Reloading Backend is not a forced OS crash. |
| Earlier preview/export memory and storage | 0.11.2 evidence retained | 4000×3000 synthetic source: 60 previews, 60 color samples, 120 tiles, 30 watermark details, 24 alternating JPG/PNG exports. Warm median 38.5 ms, p95 41.1 ms; RSS after previews 165 MiB, after exports 346 MiB; final nine export samples flat. Native real-photo backend sampled about 676 MiB after export. This excludes WebKit and is not peak/whole-app memory. Current duration checks appear below. No broad cleanup performed. |
| Installer contents, copy and launch | Separate local candidate verified; user 0.11.2 preserved | New Margin-0.11.3-local-candidate-reader-review-20261117-arm64.dmg verified, read-only mounted, copied byte-identically, ad-hoc signature checked and actually launched. Native 64-photo import/cancel/reselect/restart/resume passed using that exact copy. Candidate identity/name are separate; no automatic user-app replacement. Earlier production 0.11.3 DMG is unchanged and lacks recent revisions. Clean-user/Gatekeeper/minimum-OS acceptance remains open. |
| Native component source and notices | Fixed; distribution review incomplete | Matching SHA256-verified libheif/libde265 source archives, LGPL texts and build instructions now travel inside the app, with a conservative 365-package notice inventory including build dependencies. Packaging fails on missing/mismatched native archives or notice files. This collection is not a completed license/IP/patent audit or a license grant for Margin's own source. |
| ChatGPT connection | Native catalog verified on 0.11.2; 0.11.3 inference untested | After installing and re-signing 0.11.2, actual Mac configuration showed available models verified and no Keychain error. Core/fixture regressions cover credential loss/outage/cancel/rotation. No new live inference or generation in this phase. Repeat current-build analysis/refinement with explicit photo/request consent before claiming that path release-ready. |
| HEIC / PNG color boundaries | Passed within synthetic matrix | 14 Apple ImageIO HEIC cases: 11 accepted, ambiguous 10-bit / PQ / HLG rejected; all 8 orientation references match. PNG cICP HDR rejected, P3 and gAMA converted and precedence verified. No actual iPhone / gain-map coverage claimed. |
| Long session / memory / cache growth | Measured within bounded scope | Separate 60-minute native 0.11.2 control, 30-minute active fixed renderer (1,793 cycles) and 20-minute native 0.11.3 RC completed; UI zoom responded afterwards. Backend sampled peaks: 279.27 / 161.80 / 262.80 MiB respectively. These exclude WebKit/GPU and are not whole-app peak or OS memory-pressure acceptance. See detailed stability record. |
| Native active sessions including attributed XPC memory | Passed within new measured scope | Separate 20-minute pre-consolidation and 10-minute consolidated RC sessions completed. Backend / PID-domain WebContent, GPU, Networking and helpers cross-checked; zero observed attribution/query failures. Sampled RSS sums peaked at 770.02 / 747.27 MiB; per-process footprint sums 808.60 / 576.99 MiB. Different workloads; no memory improvement or universal whole-app ceiling claimed. Thumbnail cache stayed 64 files / 159,240 bytes in the consolidated run. |
| Crash / storage / allocation boundaries | Passed fault checks; physical pressure untested | Real SIGKILL of a private QA export child recovered with zero remaining temporary files. Child-only 8 KiB file-size quota and injected StorageFull writers isolated errors and allowed subsequent/retried exports. Sparse-file / excessive-dimension / capacity guards passed. No physical disk fill, system RAM exhaustion or power loss tested. |
| Malformed input | Passed bounded corpus | 87 deterministic owned PNG/HEIC mutations: 24 accepted, 63 rejected, no crashes/timeouts/invariant failures. This is not exhaustive fuzzing or a security audit. |
| Packaging failure preservation | Fixed and checked | Missing native-library injection now preserves the previous package. Only fully relocated and signed staged apps are published by the local packaging script. |
| Paired queue export performance | Measured; redundant writes fixed | Same renderer, seeded inputs, warm-up and 5 alternating pairs per dataset. Consolidating running state with temporary ownership changed small-batch saves 50→38 and larger-photo saves 26→20, matching baseline. Median paired ratios changed 1.263→1.028 and 1.040→0.990. Shared host and finite samples; not whole-version/UI speed or a universal peak guarantee. |
| Reader follow-up review / bounded recovery | Fixed and passed current local candidate | Failure evidence caught accumulated retired records, obsolete preview completion/publication, and older errors repopulating the cleared retry cache. All are fixed. Five fresh-process repeats retire 120 helpers and perform 200 healthy retries with descriptors/threads unchanged at 6/3 and records returning to zero; 16 partial-body cancellation/timeout cases release buffers and helpers. Native candidate cancels 57 complete / 7 cancelled, reselects while paused, restarts and resumes seven only: all 64 dimensions/ICC/source hashes checked, existing 57 outputs unchanged, no temps/duplicates, batch 5% despite UI 7%. See reader-review-0.11.3.md. |
| LaunchServices restore / blocked source reads | Bounded recovery passed on isolated Mac; clean-user policy gate remains | Controlled A path-only ABA and restart-only cases reproduced the stall. B selected-file restore passed. New C returns a recoverable timeout, previews another photo and restores the original after reselection/restart. A source com.apple.macl attribute appeared after the chooser; no system privacy setting or permission prompt was changed/accepted, and no specific TCC policy was proved. Final E checks same-photo automatic retry. Native D cancelled a blocked 64-photo batch, resumed 63 while one timed out, then retried only the failure to finish all 64. No duplicates/temp files; 5% snapshot preserved despite 7% UI. See the read-recovery record. |
| Fresh Mac user / Intel Mac / minimum macOS | Not measured | Current hardware: Apple Silicon Mac. No clean-user, Intel or macOS 12 device acceptance. Current package is arm64. |
| Developer ID signing and notarization | Blocked at last inventory | Last local signing inventory contained 0 valid Developer ID identities; not refreshed in this phase. Candidate is ad-hoc signed and not notarized. No paid enrollment or public upload is authorized. |
| Windows build/install/core UI | Blocked / not tested | At the last connected-device check only an offline Mac was available; no DRAKE/Windows device. Inventory was not refreshed in this local Mac phase. No Windows installer for this revision may be claimed tested. |

Detailed current evidence and limitations are in [stability-0.11.3.md](stability-0.11.3.md)
and `outputs/Margin-stability-2026-11-17` beside the project workspace. Earlier
white-sample, real-photo and login evidence remains under `outputs/Margin-0.11.2-qa`.
The follow-up source revision, paired comparison and attributed native sessions
are in [active-acceptance-0.11.3.md](active-acceptance-0.11.3.md) and
`outputs/Margin-active-2026-11-17`.
The bounded read follow-up and native controls are in
[read-recovery-0.11.3.md](read-recovery-0.11.3.md) and
`outputs/Margin-read-recovery-2026-11-17`.
The independently reviewed local candidate, exact package hashes and current
regression evidence are in [reader-review-0.11.3.md](reader-review-0.11.3.md) and
`outputs/Margin-reader-review-2026-11-17`.
Personal media and credentials must not be copied into the source archive.

Reproduce the isolated workspace acceptance with:

```sh
cargo run --release --manifest-path src-tauri/Cargo.toml --example verify_workspace -- tests/generated /absolute/path/to/a-new-qa-directory
```

The generated fixture directory is prepared by `scripts/make-fixtures.py`, with
the small `tests/fixtures/apple-synthetic.heic` copied to its root. The acceptance
runner refuses an existing output directory. Native HEIC build environment
variables follow `scripts/build-macos.sh`.

Next release actions: clean-user selected-file restore and access-policy acceptance;
current-build consented model analysis/refinement; actual
iPhone/gain-map, whole-app active-session and controlled performance checks;
target-device/clean-user acceptance; final
distribution-rights review; then user-provided signing/notarization credentials
and the Windows device. Do not perform paid enrollment or publish automatically.

Release requires resolving the failures above and deciding the supported release platforms.
Do not label a compiled or locally launched package “ready to launch.”

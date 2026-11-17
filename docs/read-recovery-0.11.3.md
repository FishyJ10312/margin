# Margin 0.11.3 working revision: recoverable photo reads

Measured 2026-11-17 on Apple M4 Pro / 48 GB / macOS 27.2 / arm64.
The installed user app remains 0.11.2, showing the pure-white template library.
This source revision continues the queue consolidation in
[active-acceptance-0.11.3.md](active-acceptance-0.11.3.md). Existing installers and
source archives were preserved. No public release or Windows acceptance is claimed.

## Problem and resulting behavior

A source file waiting inside an OS `open` could hold the image gate indefinitely.
Returning an error from a frontend timer alone would leave that native read and
its locks running. The renderer now obtains photo metadata and bytes through a
bounded local reader. Each read request has an eight-second deadline; cancelling
a batch interrupts its pending read. A timed-out photo reports a recoverable
error, and the next photo can render or export normally.

An explicit re-selection through Import photos clears transient read failures.
If the selected photo had a preview error and the same path is selected again,
the frontend schedules another preview even when deduplication adds zero files.
There is no new layout, style, control or dependency. All template centres remain
white; actual-photo previews continue to use the user's photo.

## Controlled native observations

Two baseline app identities were built from the preserved phase2 source.
They used the same synthetic 640×480 JPEG bytes and settings, without accounts.
An initial schemaVersion=3 seed was rejected and is excluded; all valid cases
used the supported schemaVersion=1.

| Case | Observation |
| --- | --- |
| A, Documents source restored without choosing it | Updating preview persisted |
| A, restart only, same path and settings | Still updating |
| A, byte-identical source copied inside its app data | Ready 688×528 preview |
| A, path changed back to Documents, no chooser | Still updating |
| B, normal file chooser selecting that Documents source | Import and preview ready |
| B, restart after normal selection | Preview ready |
| C, new reader, Documents source without prior selection | Recoverable timeout, no endless spinner |
| C, next healthy app-data source | Preview ready after the error |
| C, original source reselected and thumbnail selected | Original ready; ready again after restart |

Only the path changed in A's app-data / Documents comparison. Restart alone did
not explain A's result. The chooser comparison uses a different identity, so it
does not isolate app identity from selection or authorization. JPEG bytes stayed
identical, but `com.apple.macl` appeared among the source's extended attributes
after selection. Thus it would be inaccurate to say the chooser changed no OS
access state. No system privacy setting was changed, no OS permission prompt was
accepted, and no access database or private entitlement was inspected. These
observations do not prove a specific TCC denial or permission policy.

The final E build passed automatic retry when the same current photo was
reselected: zero files added, one photo retained, ready 688×528 preview without
switching thumbnails. Restart was also ready. Results and screenshots are in
`native-events.jsonl` in the evidence directory.

## Reader boundary and resource ownership

`source_io` owns the blocking source operations behind metadata/read functions.
It reuses at most two private instances of the current executable, entered
before Tauri initialization. There is no Node, Pi, additional executable payload,
network request, source spool, or full-resolution input cache on disk.

Only explicitly spawned children are terminated. On timeout or cancellation, a
slot stays occupied until its child and pipe driver are reaped, including an OS
wait that does not stop immediately. This bounds worker and cleanup-thread
growth. If both slots remain unavailable, later requests time out rather than
creating more workers. Child stdin is monitored independently of its filesystem
thread; parent exit closes the pipe and ends a blocked reader.

The local protocol streams 32 KiB chunks with request IDs, bounded headers,
length and footer validation. Inputs remain capped at 128 MiB. Allocation is
fallible; partially retained retired-reader buffers count against image admission
budgets. Source size/mtime changes during reading produce an error. Cache
validation, standard/HEIC decoding, PNG profile inspection and PNG watermarks use
the bounded reader. Imported paths are canonicalized there; deduplication no
longer performs filesystem I/O under the workspace state lock.

Recent errors are bounded to eight paths with a one-second lifetime, preventing
an immediately queued detail request from repeating a full wait. Explicit
import, batch start/resume/retry clears them. Cancelled reads are not cached.
Batch settings and temporary-output ownership remain journaled before pixels
are written. Cancelled reads mark their active item cancelled, not failed.

This boundary covers source-photo reads. It is not a deadline for every filesystem
operation, output-directory access, font discovery, image decoding, or a whole
preview. It does not guarantee GPU memory or an OS-wide RAM ceiling.

## Verification

- The real Backend preview regression was red twice before the boundary: a
  regular JPEG's controlled FIFO `open` held the renderer. It is green with a
  150 ms test deadline, while a healthy photo completes before the fault is
  released. This is a controlled blocked-read test, not a reproduction of TCC.
- Final Rust suite: 115 passed, 0 failed, 1 optional legacy OS-vault test ignored.
  The batch regression cancels an actually blocked read within its one-second
  test bound, reloads state and resumes with the original 5% settings.
- Svelte check: 0 errors / 0 warnings. Seven startup/Pi fixtures passed without
  live credentials or model requests. Previous interaction suites are retained;
  they are not relabelled as rerun native acceptance in this phase.
- Native D QA feature: 64 real exports across JPG, PNG and Apple-encoded HEIC.
  Cancellation stopped the blocked first item and left 64 cancelled / zero
  output files. Resume isolated the one timed-out input and completed the other
  63. After removing the owned fault marker, Retry failed completed only that
  item: 64 final outputs, no duplicates or temporary files, all dimensions and
  embedded ICC profiles checked. Source copies match their synthetic baseline.
- The UI changed padding to 7% while paused; batch outputs kept 5%. The first
  export is 4300×3300; the current 7% preview is 4420×3420.
- Release-core workspace: 53 valid imports, corrupt/text/directory/duplicate
  handling, 80 previews without storage growth, cancel/reload/resume, one failure
  and retry. Final 53 exports and existing output preserved; no temporary files.
- Release HEIC matrix: 14 cases, expected color/HDR admission and orientation
  behavior retained. Child-only 8192-byte file-size quota still isolates EFBIG,
  completes the next photo and retries after quota restoration.
- Real SIGKILL after observing a 7,605,543-byte owned export temp: two exports
  recovered, previous output and originals preserved, zero temporary files.
- Standalone reader EOF check: both default and QA helpers exited normally in
  about 1.3 ms after closing their parent's input, including a blocked QA open.
  The normal build ignored the fault marker; only `qa-source-block` enables it.

The unoptimized workspace QA executable was stopped by its 180-second harness
deadline while CPU-bound in repeated previews; it is not counted as a passed
suite or as a native app crash. The optimized release runner completed the full
workspace check in 9.23 seconds. This is not a before/after speed comparison.
All image fixtures are owned synthetic media. No physical disk fill, system RAM
exhaustion, power loss, actual iPhone capture or new external model call was used.

## Native memory and package scope

A 150-second native D observation produced 74 process-group samples. Associated
WebKit/GPU/networking services and reader descendants were included. Sampled RSS
sum max was 532.38 MiB; per-process footprint sum max 446.00 MiB; no PID-domain
query failures. At most one reader helper was observed; the pool hard cap is two.
The two-second samples are not frame timing or a universal peak.
RSS can double-count shared pages; footprint is OS accounting rather than total
physical RAM or VRAM. The monitor ended before the final retry/large-photo view.
Expected output files are separate from the thumbnail cache. The monitor's
settings/assets category also includes the private blocked-source fixture.

Baseline and recovery unrelocated native app builds rounded to 33.51 and 33.57
MiB respectively. This excludes the packaging script's embedded native libraries
and matching source archives; it is not a downloadable-installer size claim.

## Preservation and remaining gates

All 57 real app-data files / 202,390,800 bytes match their pre-phase hashes, sizes
and modes. Both actual source-photo hashes are unchanged. Installed 0.11.2 is
preserved and its template library was reopened. No real photo, setting or
credential was replaced. No user data cleanup, model request, photo upload,
subscription charge, new permission prompt, public push or publication occurred.

Evidence: `outputs/Margin-read-recovery-2026-11-17/` beside the workspace. Separate
identities A/B/C/D/E keep the native controls and fault feature out of the user's
app. C/D native tests preceded final source formatting and poison-lock cleanup;
the final Rust suite and E build use the final source and automatic retry handler.

General release gates still include clean-user/minimum-OS/Intel decisions,
actual iPhone/gain-map inputs, consented current-build model acceptance,
Developer ID signing/notarization, distribution review and a real Windows
device. No production installer was refreshed in this phase.

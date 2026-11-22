# Margin 0.11.4: isolated Mac installer acceptance

Date: 2026-11-18, Apple Silicon Mac. Source-package overwrite protection and
release-gate notes were pushed as `47ad4eacd0a07b7527c3c347ac7d70fee031e4ab`
before this acceptance. The user's installed app was not closed, replaced or
operated. Its executable, Info.plist and all existing application-data file
hashes matched before/after. No model inference or private-photo upload occurred.

## Artifacts and identity separation

| Artifact | Identity | Scope checked |
| --- | --- | --- |
| `Margin-0.11.4-arm64-local-candidate.dmg` | `com.fishyj.liubai` | Checksum, read-only mount, Applications shortcut, full app-tree equality after copying to a private test directory, ad-hoc signature, arm64 architecture, embedded dependency/privacy checks, detach |
| `Margin Installer QA-0.11.4-arm64-local-candidate.dmg` | `com.fishyj.margin.qa.installer202611181d3a90` | The same installer checks, then actual native operation and process restart from that exact DMG copy using a fresh, separate data directory |

The production-identity app was never launched from the installer copy: doing so
could address the user's current catalog/session. Its executable matches the
previously built 0.11.4 runtime. Runtime/frontend sources, Cargo configuration and
dependency locks are unchanged between that build's `ac5d1cf` source and
`47ad4ea`; the intervening fix is a developer source-packaging script.

The QA app was rebuilt from that same application code with only its Tauri
product name, identifier and window title overridden. It shares neither the
production single-instance identity nor data directory. MCP was enabled only
inside the newly created QA profile, and no real account was added to it.

Exact SHA256 bindings:

- Production DMG: `4ffb4d57a9ea803a6a8f23ee0bf381fb3182ef18a7447ebabb231f672eebe5e9`
- Production executable: `a11f0ed5085de932e8e3f21ec4fb293b846c111587fb269d81dbdf5525bafe59`
- QA DMG: `b6bf691ff9aedde1eede059126e922eaece515fea4399cdc3441f5e117ca3f32`
- QA executable: `b6c8936d61be360795a335b0b0c97b561c62aff7e35864c63362dc5d262a78b4`

The production DMG is 26,312,656 bytes and its app files total 44,340,876 bytes;
the QA DMG is 26,312,292 bytes. The installer copy was used after unmounting the
image, so operation did not depend on a still-mounted source volume. No package
was copied into the user's Applications directory.

## Native operation results

- The fresh QA window opened in English with an empty catalog. A real macOS file
  chooser imported a six-file range containing JPG, PNG, a synthetic HEIC and a
  Chinese filename. All six appeared in the native window without an import error.
- The packaged stdio connector controlled that QA window over its private local
  connection. It selected two photos, set 4:5 / 5% cream framing and a Chinese
  Noto Sans SC watermark. The native main preview and collapsible watermark
  close-up displayed it; both selected JPEGs were 688 x 860 with sRGB profiles.
- An initial switch to automatic 5% padding did not leave enough lower-border
  room for that watermark and its margins. All six items returned the explicit
  space error; no partial output was published and the app stayed usable. The
  initial harness assertion/failure record is retained, not counted as a pass.
  The HEIC fixture is 420 x 640; the harness's initial 640 x 480 assumption was
  corrected before the successful full-batch check.
- After explicitly increasing padding to 8%, all six mixed-format photos exported
  with embedded sRGB profiles. Worked dimensions were 716 x 556 for 640 x 480
  landscape, 556 x 716 for 480 x 640 portrait, 594 x 594 for 512-square and
  488 x 708 for the 420 x 640 HEIC. The two-photo selection remained intact.
- A further six-photo batch was actually cancelled with two complete/four
  cancelled. Clicking **Resume** in the native window finished the same batch:
  six JPEGs, no duplicates and no temporary export files.
- Catalog remove / undo / redo changed counts 6 -> 4 -> 6 -> 4. The QA process
  was then quit and relaunched from the same DMG copy. A different native PID was
  verified, and the same stdio client reconnected. Undo restored all six photos,
  original order and the two-photo selection; frame and watermark settings were
  retained. This tests one persisted removal record, not a new twenty-record soak.
- All six synthetic source hashes matched. Final native observation showed six
  photos/two selected and redo available. The QA GUI, reader and stdio processes
  were stopped, and both owned disk images were detached.

## Reproduction and evidence

`scripts/qa-installer-macos.py` creates a DMG from a prepared standalone app,
mounts it read-only, verifies and copies the app, then detaches it. It refuses an
existing output directory and never launches or globally installs an app:

```sh
python3 scripts/qa-installer-macos.py \
  --app /absolute/path/to/prepared/Margin.app \
  --out /absolute/path/to/a-new-installer-test-directory
```

Use a separate Tauri QA identifier before any native launch. The existing
`qa-mcp-native.py` / `qa-mcp-reconnect.py` describe the isolated stdio boundary;
this phase's mixed-format/native-Resume harness and exact receipts remain in
the private `Margin-installer-0.11.4-20261118-*` output folder. Raw reports include
local paths; the evidence directory is not an upload artifact.

The local OS emitted an `hdiutil attach` deprecation notice. The observed
read-only mounts, copies and detaches succeeded; this does not establish support
for a future removal of that command.

## Remaining limits

This closes the scoped current-version DMG-copy and independent-QA-operation
gap. It does not establish downloaded/quarantined first launch, a clean macOS
user, Intel/minimum-macOS compatibility, Developer ID/notarization, Windows,
actual iPhone HDR/gain-map fidelity, current-version live model inference or a
long-session/physical-pressure guarantee. Native import here used the chooser;
Finder drag/drop was not newly exercised in this round. Prior tests retain their
own scope. See [remaining release gates](release-gaps-0.11.4.md).

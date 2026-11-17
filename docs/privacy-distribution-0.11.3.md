# Local privacy and distribution review, 2026-11-17

This review covers the separately named **privacy-review-20261117** candidate,
version 0.11.3, on this Apple Silicon Mac. It is a local technical review, not a
security certification, legal opinion, public release or Windows acceptance.
The installed Margin 0.11.2 and prior candidate packages are preserved.

## Reproduced findings and fixes

- The reader-review candidate contained personal Rust compile paths even though
  release symbols were stripped. `scripts/desktop-build.mjs` now remaps the
  builder home to `/build/home` and the project to `/src/margin`; the specific
  project mapping is last. Existing compiler flags are preserved through Cargo's
  encoded argument format, including paths with spaces. Both platform build
  scripts use it. Binary output is still checked after dependency relocation;
  mapping paths is not a promise of byte-identical builds.
- Two acceptance scripts used a specific machine's screenshot destination.
  The standalone login check now uses the OS temporary directory; the suite can
  substitute its own output directory. Generated acceptance media is excluded
  from the source archive. Unreviewed binary/media assets, credential-store
  filenames and detected credential/path patterns make source packaging fail.
- Cancelling during configuration saving previously allowed the subsequent
  model command to run. The UI now checks cancellation at each async boundary,
  discards late results, clears consent and prepares a fresh token for an
  explicitly requested retry. Native cancellation also invalidates the prepared
  token before a queued task starts. Loopback and mocked UI regressions cover it.
- The optional Pi bridge now supplies `maxRetries: 0`; all three native HTTP
  clients explicitly disable reqwest's built-in retries. This controls the app's
  own dispatch policy, not a provider's billing or internal behavior, nor all
  behavior of a separately installed Codex/Pi version.
- Passive inspection of rendered frontend modules found missing clsx and
  esm-env notices. The corrected conservative inventory contains 367 packages;
  all six packages contributing frontend code are covered. The passive audit's
  output assets match the actual production build byte for byte.
- One full regression run exposed a test-only race: the test read a worker
  receipt after the file appeared but before its contents were complete. It now
  waits for a complete receipt. This did not establish an application crash.

The first new packaging attempt correctly stopped before publishing, but its
generic path pattern mistakenly classified `/build/home/.cargo` as a personal
home. The matcher now recognizes that deliberate mapping and independently
checks the actual builder/project prefixes. Old personal paths still fail its
regressions. The failed log remains local evidence.

## Exact candidate and checks

- DMG: `Margin-0.11.3-local-candidate-privacy-review-20261117-arm64.dmg`.
  SHA256: `86024589d22fa5d0d199362cffd5a457f1b108c862a9a7012a013cb030d56f6e`.
- Copied app executable SHA256:
  `b5b990f9a4ca8ffafbb9e820b59bd4bc41282c02128972f2c63ea7382ff8ff91`.
  Identity: `com.fishyj.margin.qa.privacy20261117`.
- The DMG was verified, mounted read-only, copied and detached. Its copy matched
  the staged app; ad-hoc signature verification passed. No Developer ID signature
  or notarization was performed.
- 120 Rust tests passed; one optional legacy OS-vault test was ignored. Ten
  startup/Pi/build-script tests and five distribution boundary tests passed.
  Svelte checks report zero errors and warnings. Targeted browser cancel/failure
  and offline framing/watermark/template checks use mocked native calls.
- That exact DMG copy imported a synthetic 640 x 480 image, displayed a Chinese
  watermark in context and exported a 688 x 688 JPEG with an embedded sRGB
  profile. The input was unchanged. Both fault-injection sidecars were ignored
  by its default executable. Native white library thumbnails were inspected;
  all 30 fixture thumbnail centers are exactly RGB 255/255/255.
- Three Chinese fonts are present in the executable. Loaded library versions
  are libheif 1.23.7 and libde265 1.1.3. Bundled native source archives match the
  delivered, pinned upstream archives; notices match the source inventory.

These targeted checks do not repeat or extend the previous long session,
64-photo, crash recovery or Windows measurements. Earlier results retain their
original package and platform scope.

## Network and credential boundaries

Ordinary import, layout, watermark, preview, templates and export call the local
Rust core. The frontend CSP permits IPC, not arbitrary external fetches. No
telemetry/updater client is configured in this revision. Constructors do not
issue model requests. Opening model/account settings can check local CLI status
and, for an account the user connected, refresh credentials or request its model
catalog. Account connection/disconnection has its own explicit network actions.
These are separate from image inference.

Inference requires the displayed per-request consent and Generate/Refine action.
ChatGPT sends bounded JPEG thumbnails with `store: false`; compatible vision and
image APIs use the configured endpoint and reject redirects. Image responses
must contain bounded base64 data, not a remotely fetched arbitrary image URL.
Changing endpoint origin clears the corresponding in-memory key. Settings do
not contain API keys. Native cancellation stops new app dispatches; a request
already sent may still run or be billed by its provider. Token rotation already
in progress may finish its local commit after cancellation, without resending
the inference. There is no automatic service fallback.

This phase made **no real model calls**, parsed/displayed/sent no real login
credentials and uploaded no photos. Preservation checks hash local files without
interpreting their contents. Loopback tests, mock SDKs, offline browser checks and source
inspection provide the evidence. They are not a full OS egress trace or proof
of provider behavior. The optional external SDK/CLI must be assessed for its
own version before broader claims.

The fixed `oauth-test-only.pem` is a public synthetic signing fixture paired with
a test JWKS. It is only referenced by a `cfg(test)` module, is hash-pinned for
source packaging and is absent from the production executable. It is not a
real account secret; arbitrary private keys are still rejected.

## Sharing boundaries and patch reproduction

`scripts/export-diagnostics.py` exports exactly six reviewed JSON files, pinned
by `diagnostics-allowlist.json`. It never recursively copies a diagnostic folder,
never includes images/raw logs, rejects changed/symlinked/oversized reports and
checks for paths, photo data URLs, account emails and credential patterns.
Regressions prove private neighboring files remain untouched and excluded.

Only the new allowlisted diagnostic ZIP is approved by this technical review as
redacted review material. The full local evidence folders are **local only**:
the earlier preserved-user screenshot contains a real photograph, and raw logs,
Cargo metadata, workspace fingerprints and QA profiles can contain local paths
or account/workspace metadata. Those originals remain preserved. Do not upload
either entire diagnostics folder or prior candidates with known personal paths.

A normal diff that removes a personal path would expose it in its removed lines.
The delivered patch therefore uses a separately supplied sanitized reader-review
baseline, rather than the untouched historical archive. Only the two known
machine destinations are replaced and generated fixture media is omitted before
comparison. The baseline conversion manifest binds the original archive SHA,
the transformations and both file trees. Applying the patch with `git apply -p2`
to the supplied sanitized baseline was checked against every file in the new
source ZIP. This reproduces source files, not compiler/package timestamps.
The untouched historical source and evidence remain local.

## Open release decisions and limits

Margin's own source does not yet have a selected LICENSE. The user must choose
the intended proprietary/open-source distribution terms before public source
publication; no license was assigned on their behalf. The component inventory,
font texts, native source archives and asset provenance are technical evidence,
not a completed legal/IP/patent clearance for codecs, generated art or the app.

External gates remain: distribution-rights decision/review, Developer ID signing
and notarization, clean-user/Gatekeeper and supported minimum-OS/Intel acceptance,
real Windows build/install verification, and any explicitly consented live-model
acceptance required for release. This phase did not pay, publish, change system
permissions, install over the user's app or interact with Claude.

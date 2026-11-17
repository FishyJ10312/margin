# Margin 0.11.4: remaining release gates

Audit date: 2026-11-17. Baseline: installed/private-GitHub trial commit
`ac5d1cffb8781ab4460d9b6c83f2a563cb29f6c4`. This audit was first completed as a
local source candidate. Publishing that source was subsequently authorized on
2026-11-18, followed by separate isolated Mac-installer acceptance. Neither step
updates the installed user app. Earlier results retain their exact version,
platform and workload scope.

## Verified evidence already available

- The 0.11.4 delivery receipt binds the installed executable to the build and
  pushed commit. Photo order/selection, settings, presets/templates, removal
  history and account/model files were preserved. The original photo hash
  matched, the real-photo preview worked, and the model dialog still displayed
  its verified connection. This is not a new inference/generation test.
- That version passed 149 library tests with one optional legacy OS-vault test
  skipped, 19 JavaScript tests, five distribution tests, Svelte diagnostics,
  the production build, native dependency relocation and ad-hoc signature checks.
  Those complete suites are not rerun by this audit.
- Before delivery, the parallel-test fix passed five full default-parallel runs
  and six targeted concurrent runs. MCP reconnect and Svelte conflict results
  remain scoped as recorded in their linked reports.

## Isolated gap advanced here

The source ZIP command refused an existing destination only before constructing
its ZIP. Two concurrent builds could both pass that check, then both report
success while the later `os.replace` overwrote the earlier accepted archive.
A real two-process CLI regression reproduced exactly that outcome.

The local candidate atomically links the sealed archive to its final name. Its
staging directory shares the destination filesystem. If the destination appears
meanwhile, the later publisher fails and the accepted bytes remain unchanged.
No overwrite fallback is used. A filesystem that cannot create a hard link
will fail rather than silently weaken the no-replace behavior; only this Mac's
filesystem is validated here, not every removable/Windows filesystem.

The regression controls scheduling after each real ZIP is sealed, before
publication. It does not replace the packaging or publication implementation.
Both processes must reach that point while the destination is still absent;
then the first succeeds, the second refuses, the accepted archive remains valid
and identical, and excluded synthetic `.env` data is absent. Test processes and
synthetic source trees are owned by the test; no app, account or photo is used.

## Unverified and owner-dependent gates

| Gate | Current evidence / gap | Required next step |
| --- | --- | --- |
| Current-version installer | 0.11.4 standalone `.app` copied and launched; earlier DMG checks cover separately named 0.11.3 candidates | Build and verify a version-matched installer in isolation before distributing it |
| Developer ID / notarization / Gatekeeper | Current trial is ad-hoc signed; no notarized 0.11.4 artifact or clean-user Gatekeeper pass | Owner supplies the intended signing access and authorizes the final distribution flow; no paid enrollment here |
| Fresh user, Intel, minimum macOS | Current hardware acceptance is Apple Silicon; macOS 12 is configured, not device-verified | Decide the supported platforms, then use matching isolated users/devices |
| Windows | Source/build scripts exist; no current-revision actual-device build/install result | A connected Windows machine and real install/operation checks |
| Protected-folder restore and native dialogs | Prior isolated permission/read differences remain scoped; modal conflict tests do not prove all OS pickers | Separate synthetic-profile native tests; handle any required permission through the normal user flow |
| Actual iPhone HEIC/HDR/color | Synthetic HEIC/PNG/orientation matrices passed previously; actual iPhone/gain-map coverage remains open | Use specifically authorized local test photos; do not generalize synthetic acceptance |
| Current-version live models | Connected account/catalog state preserved; 0.11.4 inference/refinement not newly exercised | Explicit authorization for a bounded image/request and its model usage; no inference or uploads in this round |
| Long sessions and physical pressure | Earlier finite runs retain their named artifact/workload; no current-version universal peak or physical RAM/disk/power-loss guarantee | A separately scoped isolated workload; do not stress or operate the user's current session |
| Distribution rights | Dependency/source notices exist; Margin's own distribution license has not been selected | Owner chooses source/app distribution terms and completes the remaining rights review before public distribution |

## Change and evidence boundaries

Only the source-package CLI, its focused regression and release documentation
are candidates in this round. Application code, frontend design, installed app,
Codex/MCP configuration, source photos and user credentials are not modified.
The independently arranged frontend redesign is not replaced by this work.

Private local evidence is under `Margin-source-package-review-20261117-*` in the
task's output folder: the original failing regression, fixed regression,
focused distribution results, complete-source archive inspection and
preservation fingerprints. No raw evidence folder is approved for upload.

See [0.11.4 delivery](update-0.11.4.md),
[MCP reliability](mcp-reliability-0.11.3.md),
[parallel-test recovery](parallel-test-recovery-0.11.3.md), and the historical
[0.11.3 platform audit](release-readiness.md).

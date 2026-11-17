# Margin 0.11.4

The project and GitHub repository use the name **Margin** (`FishyJ10312/margin`).
The macOS bundle identity `com.fishyj.liubai`, executable name and data directory
remain compatible with the installed app so an update keeps the photo catalog,
settings, selected photos, twenty-entry undo/redo history and model sign-in data.
Internal legacy identifiers are compatibility details, not another product.

## Included fixes

- Keyed MCP mutations can replay their completed result after a lost reply.
- Accepted mutations retain their lock until the window finishes, even if the
  caller times out; status reads remain available.
- The window rechecks conflicting dialogs after asynchronous saves and reads.
- Fault-injection tests have independent reader state and a bounded FIFO
  handshake. The ordinary test harness remains parallel; product reader limits,
  deadlines and preview rendering are unchanged by that test-only correction.

Retry IDs remain bounded to 128 records and 16 MiB for the running process.
They do not persist across app restarts. After restarting, inspect saved catalog
and export status before deciding whether to start another operation.

The retained failing and passing evidence is described in
[MCP reliability](mcp-reliability-0.11.3.md) and
[parallel test recovery](parallel-test-recovery-0.11.3.md).

## Validation and delivery

The prior fixed source passed five ordinary parallel library runs (149 passed,
one optional OS-vault test ignored per run), six targeted concurrent runs and
both Svelte modal-conflict checks. The versioned 0.11.4 source also passed the
ordinary parallel library suite (149 passed, one optional test ignored), all 19
JavaScript tests, five distribution tests, Svelte diagnostics and the production
build. The standalone Mac package passed dependency relocation, privacy checks
and signature verification. Installed-app preservation checks are retained in
the private local delivery receipt; personal paths and account records are not
published with the source.

This is a local Mac trial update. Windows still needs actual-device validation;
public Mac distribution still needs Developer ID signing and notarization.

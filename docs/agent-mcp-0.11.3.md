# Margin agent connection

Open Margin and click **Agent connection** (the plug in the top bar). Enable the
local connection, choose whether editing/import/export is allowed, then copy the
Codex TOML or MCP JSON configuration into your agent's MCP settings. Refresh the
agent's connection and keep Margin open. The installed Margin executable is also
the stdio connector; users do not need Node, Pi, Python or another helper install.

Try: “Read the current Margin catalog, add an 8% cream border and a small text
watermark, show me the preview, then export the two selected photos to …”.
The agent should read the workspace first and use its current photo IDs.

## Tools

| Tool | Purpose |
| --- | --- |
| `margin_get_workspace` | Current photos (paged), selection, settings and history counts |
| `margin_import_photos` | Up to 100 absolute JPG/PNG/HEIC/HEIF paths; deduplicated |
| `margin_select_photos` | Explicit IDs, all or none, active photo and Shift anchor |
| `margin_update_settings` | Partial frame, ratio, pattern, watermark or export edits |
| `margin_get_preview` | Actual bounded framed image, including the watermark |
| `margin_list_fonts` | Available built-in/system fonts, including Chinese fonts |
| `margin_list_templates` | Built-in and saved template IDs/descriptions |
| `margin_apply_template` | Apply a template while keeping watermark/export settings |
| `margin_export_photos` | Start selected/all/explicit-ID export to an absolute directory |
| `margin_get_export_status` | Per-photo batch progress, outputs and failures |
| `margin_cancel_export` | Cancel through the ordinary safe batch boundary |
| `margin_remove_photos` | Remove catalog entries; originals remain on disk |
| `margin_undo_removal` | Persistent catalog undo, including order and selection |
| `margin_redo_removal` | Replay the recorded removal; originals remain intact |

Selecting/exporting uses catalog IDs, never guessed file names. Settings patches
preserve omitted fields. Percentage means minimum padding; ratios expand the
canvas and do not crop or stretch the photo. For a text watermark, choose a
`fontFamily` from the font tool; built-in fonts resolve without a file path.
Custom font/PNG paths must be absolute local paths. Exports return a batch ID
immediately; poll status rather than resubmitting. Collisions get numbered names.

The local reliability revision adds optional `operationId` values to mutating
tools so a lost reply can be retried without repeating an accepted operation.
Reuse the same ID, tool and arguments within the same running app; after an app
restart, inspect the saved workspace/batch first. Limits, reproductions and the
separate native test results are recorded in [MCP reliability](mcp-reliability-0.11.3.md).
That revision is included in the separately authorized [0.11.4 update](update-0.11.4.md).

## Boundaries

- The connection starts disabled. The user's enable/editing preferences persist.
- Only 127.0.0.1 is bound. Every connection needs an app-generated random token;
  browser `Origin` requests and unexpected hosts are rejected.
- Credentials live in an app-owned `agent-mcp` folder, private to the user (0700
  directory, 0600 files on Unix). Configuration snippets contain the connector
  and connection-file path, not the token. The connector bypasses system proxies
  and redirects. Symlinked or public credential files are refused.
- The same connector reads refreshed connection information on each request,
  allowing a running agent to reconnect after Margin restarts. A closed/disabled
  app gives an actionable connection error; the connector does not launch the app
  or enable access on the user's behalf.
- The allowlist provides photo workflows only. It does not expose model sign-in
  records, model generation, shell execution, arbitrary Tauri commands or a
  generic file-reading operation. Photo previews requested by an agent are sent
  to that connected agent; Margin's own processing remains local.
- Writes are serialized. The UI flushes prior user edits before an agent change,
  visibly marks the operation and prevents conflicting photo edits. Other dialogs
  and a native photo chooser block agent writes until the user finishes them.
- Source photos remain intact. Catalog removals use the same atomic persistent
  twenty-operation history. Already-started work remains owned by the visible
  app if an agent disconnects; inspect progress before retrying an uncertain call.
- Requests, connections, arguments, results and waiting time are bounded. Preview
  uses the existing bounded render pipeline instead of decoding another full-size
  image for every agent read.

## Protocol and verification

The bundled executable exposes newline-delimited stdio, forwarding authenticated
requests to the open app's loopback JSON endpoint. Legacy initialization supports
2025-03-26, 2025-06-18 and 2025-11-25. The 2026-07-28 per-request metadata flow,
discovery and header/body validation are also supported. JSON responses are used;
there is no standalone SSE stream. Long exports use the app's explicit status
and cancel tools. Codex's generated configuration sets a 120-second tool timeout.

Sources: [MCP stdio and earlier HTTP transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
[current HTTP transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http),
[discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover),
[Codex MCP configuration](https://developers.openai.com/codex/mcp).

Checks include the Rust library tests, `tests/agent-settings.test.ts`, the isolated
`tests/ui/agent-acceptance.mjs` fixture, and `scripts/qa-mcp-native.py` against a
packaged standalone app with a separate synthetic-photo profile. The fixture
only verifies UI routing; native acceptance verifies actual stdio messages,
visible app changes, real exports and source-file preservation. No model calls
are required for these checks. Windows compilation/operation remains unverified.

The latest Mac source check passed 143 Rust library tests (one optional OS-vault
test ignored), 19 JavaScript/TypeScript tests, five distribution-boundary tests,
Svelte diagnostics and a production frontend build. Codex's own app-server
discovery loaded this server through the saved user configuration and returned
all 14 tools with advertised capabilities and no discovery error. This discovery
check does not establish that an already-running chat has refreshed its tool set.

A user-authorized real-photo check used MCP to apply a frame, edit a text
watermark, return the actual preview and export one full-resolution JPEG. The
7705 × 5384 output decoded successfully; source-file SHA-256, catalog order and
selection remained unchanged. A subsequent read-only workspace/preview check
also passed. Personal photos, account state, connection tokens and local Codex
configuration backups are kept outside this source repository.

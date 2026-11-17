# MCP uncertain-result reliability

This follow-up started from `8531bbf` and was first validated locally without
changing the installed app or GitHub. It is included in the subsequently
authorized [Margin 0.11.4 update](update-0.11.4.md). The observations below retain
the original test scope and limitations.

## Reproduced defects and changes

1. An export that completed after its client lost the reply could be exported
   again by a repeated tool call. The regression test first failed with two
   different batch IDs. Mutating tools now accept an optional `operationId`.
   Reusing the same ID, tool and arguments returns the original result without
   another dispatch. Different arguments with an existing ID are rejected.
   Protocol clients may also send the ID in
   `params._meta["com.fishyj.margin/operationId"]`.
2. A native-command response deadline released the mutation permit before the
   window had finished its accepted work. Pending writes now retain their permit
   until final window completion; read-only status queries remain available.
   A late result can finish the operation and be replayed to a new client.
3. The window only checked for modal conflicts before waiting for a pending user
   save. A dialog opened during that wait did not prevent the agent's later
   change. The Svelte regression reproduced this hidden update. The window now
   rechecks after saving and after asynchronous font/template discovery, before
   submitting the native write. A previously submitted native write still owns
   its result; opening a dialog does not roll back accepted work.

The first and third defects have saved failing and passing logs. The deadline
behavior is covered with a deliberately held GUI completion and a 30 ms test
deadline; the production deadline remains 110 seconds.

## Safe-retry contract

Generate a unique `operationId` before the initial mutation, and reuse it with
exactly the same tool/arguments when the result is uncertain. Ordinary JSON-RPC
request IDs are not operation IDs. Poll `margin_get_export_status` or
`margin_get_workspace` while a write is pending. An export replay returns the
original start receipt, so poll status for its current completion state.

The running app retains at most 128 operation records and 16 MiB of replay
payloads. Records are not silently evicted: a full ID table rejects new keyed
operations, and an oversized replay payload leaves a terminal receipt explaining
that the current state must be queried. Read-only calls and ordinary UI operations
remain available. No dependencies were added.

This is an in-process guarantee. It does not make an unkeyed request safe to
blindly repeat, and it does not persist operation IDs across a Margin restart.
After an app restart, inspect the saved catalog/batch before deciding on a new
operation. Do not retry an uncertain action with a new ID.

## Added checks

- `src-tauri/src/mcp_reliability_tests.rs`: real loopback HTTP, actual catalog
  journals and JPEG export, with a synthetic stand-in at the window-command
  boundary. Covers lost export result, late completion after deadline, unchanged
  twenty-entry removal history/selection on replay, conflicting ID arguments and
  a full replay table that preserves old receipts.
- `tests/ui/agent-conflict-acceptance.mjs`: actual Svelte UI with synthetic native
  boundaries. A modal opened during a held user save blocks the agent write;
  without a conflict, the user edit and agent patch both survive. Photo order,
  selection and all twenty history records are checked in both cases. This does
  not establish OS-native picker behavior.
- `scripts/qa-mcp-reconnect.py`: packaged Mac test app with a different bundle ID
  and 26 synthetic photos. One stdio client sends a mutation and never consumes
  its result; a separate observer sees completion, then a newly launched client
  replays it. Exactly one export batch/two output files remain. The same flow
  for the twentieth removal preserves six remaining photos, selection and all
  twenty undo records. All 26 source hashes remain unchanged. No model calls.

The isolated app was built and packaged successfully. Svelte diagnostics reported
zero errors/warnings; the three settings tests passed. The complete Rust library
suite passed with **147 passed, one optional OS-vault test ignored** using:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1
```

One default parallel run failed the existing preview-obsolescence test's
two-second “reader entered” wait while fault-injection/image tests and a build
overlapped. It passed in the serial suite. This change does not claim to fix that
parallel test timing sensitivity; both logs are retained.

A subsequent local, test-only follow-up addressed shared fixture state and a
missed FIFO wakeup. The final default-parallel command passed five bounded runs,
and the targeted concurrent regressions passed six runs. See
[default-parallel test recovery](parallel-test-recovery-0.11.3.md) for the retained
failures, diagnosis, unchanged assertions and measured fixture-process overlap.

The first native setup attempted synthetic files inside a protected user folder:
all 26 reads timed out, and the MCP caller reached its deadline. The visible app
eventually returned to an idle error summary. The same photo imported from a
temporary directory in 0.06 s; the reconnect tests passed there. Folder access is
an environment boundary requiring separate verification, not a confirmed cause
or a reason to change permissions automatically. No permissions were changed for
the installed app. Only the isolated test app's connection was enabled.

## Scope left for a later acceptance

- Protected-folder access needs the user to select the folder/photo or confirm
  the app's macOS permission before that location can be accepted.
- OS-native file/font/color-picker conflicts need native interaction coverage;
  the modal race above was validated at the Svelte/native-command boundary.
- Windows behavior needs a connected Windows machine with a real build/run.
- Public Mac distribution needs the owner's Developer ID signing/notarization
  access and release authorization. The original follow-up did not replace the
  installed app or push GitHub; that work was authorized separately for 0.11.4.

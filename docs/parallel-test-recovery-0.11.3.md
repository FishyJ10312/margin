# Default-parallel library test recovery

This records the original local-only test recovery. These changes are included
in the subsequently authorized [Margin 0.11.4 update](update-0.11.4.md).

This changes test orchestration only. The production reader implementation,
two-reader limit, normal read deadline, preview implementation, previous local
MCP retry/timeout changes, and Svelte modal-conflict fix remain unchanged. The
installed app, Codex configuration, credentials, system permissions and GitHub
have not been modified. Same-process-only retry semantics are unchanged.

## Observed failures and diagnosis

Three fresh runs used the normal command with no `RUST_TEST_THREADS` override:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

The third failed in
`repeated_timeouts_cancellation_and_retries_reclaim_reader_resources` while
reading an expected worker marker: the file did not exist. The earlier
preview-obsolescence failure was also retained from the reliability run.

The fault tests shared a process-wide, two-slot source-reader pool, failure cache
and process-wide FD/thread statistics. They assumed exclusive use of those
resources. A controlled regression occupied both reader slots and showed that a
third call times out **before dispatch**, with no worker marker. After releasing
the two blocked readers, retry succeeded and all synthetic input bytes matched.
The temporary diagnostic recorded `live=2`, `idle=0`, `deadline_ms=100`.

Isolating the fault fixtures removed that interference and passed five default
runs, but a subsequent targeted concurrent run still caught a second race. It
must not be treated as an all-green intermediate result. The captured state was:

- old input reached its fault marker;
- the newer preview was already registered (`generation=2`);
- the one-shot nonblocking FIFO writer had not connected;
- the old request returned a read timeout, not a successful stale preview.

The fault marker is written **before** the worker opens the FIFO. A writer can
therefore get no reader and miss the wakeup. The previous assertion's failure
message misleadingly described every non-superseded result as a success.

## Minimal test-only changes

- `test_support.rs`, compiled only for Unix library tests, runs each resource/
  fault fixture in an independent copy of the same test binary. It checks that
  exactly one test actually ran and passed; a zero-test invocation cannot count
  as success. Each child has a 30-second outer bound. The outer harness remains
  parallel; no global serial lock or `--test-threads=1` was introduced.
- The original fault assertions and their 100/150/200/250 ms and 1/2-second limits
  remain. The preview fixture also asserts that the newer request was registered
  and that its release actually connected to the reader.
- FIFO release now retries the nonblocking connection with a one-second bound,
  completing the real handshake rather than assuming a marker means the reader
  is already waiting. A separate delayed-reader regression failed with the
  original one-shot release and passed with the handshake.
- The original assertions that the obsolete preview returns “settings updated”
  and that the current preview succeeds are retained. Failure output now includes
  the actual old result without dumping image bytes.

Temporary `DEBUG-PT1` and `DEBUG-PT2` probes were removed from the source. This
does not change product reader scheduling, increase reader capacity, extend the
product deadline, or alter the application's retry semantics.

## Final bounded verification

After both fixes, on the current Mac with Rust 1.99.1:

| Gate | Result |
| --- | --- |
| Normal default-parallel command | 5/5 runs passed; 149 passed and one optional OS-vault test ignored per run |
| Targeted concurrent reader, stale-preview, MCP-reliability and FIFO regressions | 6/6 runs passed; 13 tests per run |
| Fixture process overlap | Peak of six fixture children observed in every targeted run |
| Existing Svelte modal/save conflict checks | Both passed; photo order, selection and twenty history entries preserved |
| Prior MCP/guard source and installed executable hashes | Unchanged |
| Diagnostic cleanup and diff whitespace check | Passed |

The targeted command also uses default parallelism:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib -- \
  source_io::tests:: \
  an_in_flight_preview_is_obsolete_when_a_later_request_arrives \
  mcp::reliability_tests:: \
  test_support::tests::
```

The targeted set retains real overlapping reads, bounded queue timeout, process
reaping, delayed FIFO release, stale-preview rejection, lost-result replay,
late-completion locking, bounded retry receipts and twenty-entry history checks.
No long soak or repeated installed-app/export acceptance was performed.

Private local evidence is in `Margin-parallel-tests-2026-11-17` under the task's
output folder: baseline and intermediate failed runs, controlled queue and FIFO
red/green logs, all final runs, process-overlap samples, and `verify-final.py` for
bounded repetition. Preserve the failed intermediate logs when reviewing the
result. These finite runs establish the observed correction, not a guarantee for
every possible host-resource failure.

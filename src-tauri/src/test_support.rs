//! Only resource/fault fixtures use this boundary. Each keeps the real reader
//! pool, cache, FD/thread counters and original deadlines in its own process.
//! The outer Rust harness and all other tests retain their default parallelism.
use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const FIXTURE_ENV: &str = "MARGIN_ISOLATED_SOURCE_FIXTURE";

/// Return true in the outer harness after the exact same case passed in its
/// child; return false in that child so the original test body actually runs.
pub(crate) fn isolated_source_fixture() -> bool {
    let current = thread::current();
    let name = current
        .name()
        .expect("A source fixture must run as a named Rust test");
    if std::env::var(FIXTURE_ENV).ok().as_deref() == Some(name) {
        return false;
    }
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", name, "--nocapture"])
        .env(FIXTURE_ENV, name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Start an isolated source fixture");
    let started = Instant::now();
    let mut expired = false;
    loop {
        if child.try_wait().expect("Poll source fixture").is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            expired = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = child
        .wait_with_output()
        .expect("Reap isolated source fixture");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !expired && output.status.success(),
        "Isolated fixture {name} failed (deadline={expired}, status={}):\n{}\n{}",
        output.status,
        stdout.chars().take(8192).collect::<String>(),
        stderr.chars().take(8192).collect::<String>()
    );
    assert!(stdout.contains("test result: ok. 1 passed;"),
        "The exact fixture must execute its assertions rather than pass with zero selected tests: {stdout}");
    true
}

/// Test-fixture FIFO release. A marker can be written before a reader opens it.
pub(crate) fn connect_fifo_writer(fifo: &std::ffi::CStr, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        let fd = unsafe { libc::open(fifo.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
        if fd >= 0 {
            unsafe {
                libc::close(fd);
            }
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::CString, fs, os::unix::fs::OpenOptionsExt, sync::mpsc};

    #[test]
    fn fifo_release_waits_for_reader_instead_of_losing_the_wakeup() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("synthetic-handshake");
        let fifo = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let writer = thread::spawn(move || {
            started_tx.send(()).unwrap();
            result_tx
                .send(connect_fifo_writer(&fifo, Duration::from_secs(1)))
                .unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let early = result_rx.recv_timeout(Duration::from_millis(20));
        let finished_early = early.is_ok();
        // Deliberately make the actual reader available only after the first
        // connection attempt. Nonblocking open keeps red-run cleanup bounded.
        let _reader = if finished_early {
            None
        } else {
            Some(
                fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&path)
                    .unwrap(),
            )
        };
        let connected =
            early.unwrap_or_else(|_| result_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        writer.join().unwrap();
        assert!(
            !finished_early,
            "A one-shot release loses the wakeup before the reader is ready"
        );
        assert!(
            connected,
            "The delayed reader must receive the fixture release"
        );
    }
}

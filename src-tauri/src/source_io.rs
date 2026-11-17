//! Bounded, cancellable source reads in at most two private copies of this executable.
//! Workers stream bytes through local pipes; no source spool or external runtime is used.
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex, OnceLock, Weak,
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

const WORKER_FLAG: &str = "--margin-source-reader-v1";
const HEADER: &str = "MARGIN_SOURCE_HEADER ";
const FOOTER: &str = "MARGIN_SOURCE_END ";
const MAX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_WORKERS: usize = 2;
const CHUNK: usize = 32 * 1024;
pub const TIMEOUT: &str = "读取照片超时，请重新选择照片或稍后重试。";
pub const CANCELLED: &str = "已取消读取照片。";
const INTERRUPTED: &str = "读取照片意外中断，请重新选择照片或重试。";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub canonical: PathBuf,
    pub len: u64,
    pub modified: Option<u128>,
    pub is_file: bool,
    pub is_dir: bool,
}
pub(crate) struct Input {
    pub metadata: Metadata,
    pub bytes: Vec<u8>,
}
#[derive(Clone)]
struct Control {
    timeout: Duration,
    cancelled: Option<Arc<AtomicBool>>,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(8),
            cancelled: None,
        }
    }
}
thread_local! { static CONTROL: RefCell<Control> = RefCell::new(Control::default()); }
struct Scope(Control);
impl Drop for Scope {
    fn drop(&mut self) {
        CONTROL.with(|c| *c.borrow_mut() = self.0.clone());
    }
}
pub fn with_cancel<T>(cancelled: Arc<AtomicBool>, operation: impl FnOnce() -> T) -> T {
    let old = CONTROL.with(|c| {
        let old = c.borrow().clone();
        c.borrow_mut().cancelled = Some(cancelled);
        old
    });
    let _restore = Scope(old);
    operation()
}
#[cfg(test)]
pub(crate) fn with_timeout<T>(timeout: Duration, operation: impl FnOnce() -> T) -> T {
    let old = CONTROL.with(|c| {
        let old = c.borrow().clone();
        c.borrow_mut().timeout = timeout;
        old
    });
    let _restore = Scope(old);
    operation()
}

#[derive(Serialize, Deserialize)]
struct Request {
    id: u64,
    path: PathBuf,
    read: bool,
}
#[derive(Serialize, Deserialize)]
struct Head {
    id: u64,
    metadata: Option<Metadata>,
    error: Option<String>,
    body: bool,
}
#[derive(Serialize, Deserialize)]
struct End {
    id: u64,
    error: Option<String>,
}
struct Delivered {
    result: Result<Input, String>,
    reusable: bool,
}
struct Job {
    request: Request,
    reply: mpsc::Sender<Delivered>,
}
struct Worker {
    child: Child,
    jobs: mpsc::SyncSender<Job>,
    driver: Option<thread::JoinHandle<()>>,
    retained: Arc<AtomicUsize>,
}
#[derive(Default)]
struct State {
    idle: Vec<Worker>,
    live: usize,
    abandoned: Vec<Weak<AtomicUsize>>,
}
struct Pool {
    state: Mutex<State>,
    available: Condvar,
}
fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(|| Pool {
        state: Mutex::new(State::default()),
        available: Condvar::new(),
    })
}
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn release_slot() {
    let p = pool();
    if let Ok(mut state) = p.state.lock() {
        state.live = state.live.saturating_sub(1);
        state.abandoned.retain(|s| s.strong_count() > 0);
        p.available.notify_all();
    }
}
fn retire(mut worker: Worker) {
    if let Ok(mut state) = pool().state.lock() {
        state.abandoned.retain(|s| s.strong_count() > 0);
        state.abandoned.push(Arc::downgrade(&worker.retained));
    }
    // Only this explicitly spawned child is killed. Hold its slot until it is
    // actually reaped, including a kernel wait that does not stop immediately.
    let _ = worker.child.kill();
    thread::spawn(move || {
        loop {
            match worker.child.try_wait() {
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                _ => break,
            }
        }
        drop(worker.jobs);
        if let Some(driver) = worker.driver.take() {
            let _ = driver.join();
        }
        drop(worker.retained);
        release_slot();
    });
}
struct Lease(Option<Worker>);
impl Lease {
    fn reuse(mut self) {
        if let Some(worker) = self.0.take() {
            match pool().state.lock() {
                Ok(mut state) => {
                    state.idle.push(worker);
                    pool().available.notify_one();
                }
                Err(poisoned) => {
                    drop(poisoned.into_inner());
                    retire(worker);
                }
            }
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(worker) = self.0.take() {
            retire(worker);
        }
    }
}
fn is_cancelled(control: &Control) -> bool {
    control
        .cancelled
        .as_ref()
        .is_some_and(|c| c.load(Ordering::Acquire))
}
fn checkout(control: &Control, deadline: Instant) -> Result<Lease, String> {
    loop {
        if is_cancelled(control) {
            return Err(CANCELLED.into());
        }
        let mut state = pool().state.lock().map_err(|_| INTERRUPTED)?;
        if let Some(worker) = state.idle.pop() {
            return Ok(Lease(Some(worker)));
        }
        if state.live < MAX_WORKERS {
            state.live += 1;
            drop(state);
            return match spawn_worker() {
                Ok(worker) => Ok(Lease(Some(worker))),
                Err(error) => {
                    release_slot();
                    Err(error)
                }
            };
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(TIMEOUT.into());
        }
        let _guard = pool()
            .available
            .wait_timeout(state, left.min(Duration::from_millis(20)))
            .map_err(|_| INTERRUPTED)?;
    }
}
fn line<T: for<'de> Deserialize<'de>>(reader: &mut impl BufRead, marker: &str) -> io::Result<T> {
    let mut skipped = 0;
    loop {
        let mut bytes = Vec::new();
        let n = (&mut *reader).take(65537).read_until(b'\n', &mut bytes)?;
        if n == 0 || n > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source protocol line",
            ));
        }
        let text = String::from_utf8_lossy(&bytes);
        if let Some((_, json)) = text.split_once(marker) {
            return serde_json::from_str(json.trim()).map_err(io::Error::other);
        }
        skipped += n;
        if skipped > 65536 {
            return Err(io::Error::other("source protocol preamble"));
        }
    }
}
fn receive(
    reader: &mut impl BufRead,
    request: &Request,
    retained: &AtomicUsize,
) -> io::Result<Delivered> {
    let header: Head = line(reader, HEADER)?;
    if header.id != request.id {
        return Err(io::Error::other("source request mismatch"));
    }
    if let Some(error) = header.error {
        return Ok(Delivered {
            result: Err(error),
            reusable: true,
        });
    }
    let metadata = header
        .metadata
        .ok_or_else(|| io::Error::other("source metadata missing"))?;
    let mut bytes = Vec::new();
    if header.body {
        if !request.read || metadata.len > MAX_BYTES {
            return Err(io::Error::other("source payload limit"));
        }
        bytes
            .try_reserve_exact(metadata.len as usize)
            .map_err(io::Error::other)?;
        retained.store(bytes.capacity(), Ordering::Release);
        loop {
            let mut size = [0; 4];
            reader.read_exact(&mut size)?;
            let size = u32::from_le_bytes(size) as usize;
            if size == 0 {
                break;
            }
            if size > CHUNK || bytes.len() as u64 + size as u64 > MAX_BYTES {
                return Err(io::Error::other("source chunk limit"));
            }
            bytes.try_reserve_exact(size).map_err(io::Error::other)?;
            retained.store(bytes.capacity(), Ordering::Release);
            let start = bytes.len();
            bytes.resize(start + size, 0);
            reader.read_exact(&mut bytes[start..])?;
        }
        let end: End = line(reader, FOOTER)?;
        if end.id != request.id {
            return Err(io::Error::other("source footer mismatch"));
        }
        if let Some(error) = end.error {
            return Ok(Delivered {
                result: Err(error),
                reusable: true,
            });
        }
        if bytes.len() as u64 != metadata.len {
            return Err(io::Error::other("source length mismatch"));
        }
    }
    Ok(Delivered {
        result: Ok(Input { metadata, bytes }),
        reusable: true,
    })
}
fn spawn_worker() -> Result<Worker, String> {
    let mut command = Command::new(std::env::current_exe().map_err(|_| INTERRUPTED)?);
    #[cfg(test)]
    command
        .args(["--exact", "source_io::tests::worker_entry", "--nocapture"])
        .env("MARGIN_TEST_SOURCE_WORKER", "1");
    #[cfg(not(test))]
    command.arg(WORKER_FLAG);
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| INTERRUPTED)?;
    let mut input = child.stdin.take().ok_or(INTERRUPTED)?;
    let mut output = BufReader::new(child.stdout.take().ok_or(INTERRUPTED)?);
    let (jobs, requests) = mpsc::sync_channel::<Job>(1);
    let retained = Arc::new(AtomicUsize::new(0));
    let stats = retained.clone();
    let driver = thread::spawn(move || {
        while let Ok(job) = requests.recv() {
            stats.store(0, Ordering::Release);
            let result = (|| {
                let data = serde_json::to_vec(&job.request).map_err(io::Error::other)?;
                if data.len() > 65535 {
                    return Err(io::Error::other("source path limit"));
                }
                input.write_all(&data)?;
                input.write_all(b"\n")?;
                input.flush()?;
                receive(&mut output, &job.request, &stats)
            })();
            let failed = result.is_err();
            let delivered = result.unwrap_or_else(|_| Delivered {
                result: Err(INTERRUPTED.into()),
                reusable: false,
            });
            stats.store(0, Ordering::Release);
            if job.reply.send(delivered).is_err() || failed {
                break;
            }
        }
    });
    Ok(Worker {
        child,
        jobs,
        driver: Some(driver),
        retained,
    })
}
#[derive(Default)]
struct RecentErrors {
    generation: u64,
    entries: VecDeque<(PathBuf, Instant, String)>,
}
fn recent_errors() -> &'static Mutex<RecentErrors> {
    static ERRORS: OnceLock<Mutex<RecentErrors>> = OnceLock::new();
    ERRORS.get_or_init(|| Mutex::new(RecentErrors::default()))
}
pub fn clear_failures() {
    if let Ok(mut errors) = recent_errors().lock() {
        errors.generation = errors.generation.wrapping_add(1);
        errors.entries.clear();
    }
}
fn request(path: &Path, read: bool) -> Result<Input, String> {
    let control = CONTROL.with(|c| c.borrow().clone());
    if is_cancelled(&control) {
        return Err(CANCELLED.into());
    }
    let error_generation = if let Ok(mut errors) = recent_errors().lock() {
        errors
            .entries
            .retain(|(_, at, _)| at.elapsed() < Duration::from_secs(1));
        if let Some((_, _, error)) = errors.entries.iter().find(|(p, _, _)| p == path) {
            return Err(error.clone());
        }
        Some(errors.generation)
    } else {
        None
    };
    let result = (|| {
        let deadline = Instant::now() + control.timeout;
        let lease = checkout(&control, deadline)?;
        let (sender, receiver) = mpsc::channel();
        lease
            .0
            .as_ref()
            .unwrap()
            .jobs
            .send(Job {
                request: Request {
                    id: REQUEST_ID.fetch_add(1, Ordering::Relaxed),
                    path: path.into(),
                    read,
                },
                reply: sender,
            })
            .map_err(|_| INTERRUPTED)?;
        loop {
            if is_cancelled(&control) {
                return Err(CANCELLED.into());
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match receiver.recv_timeout(left.min(Duration::from_millis(20))) {
                Ok(delivered) => {
                    if delivered.reusable {
                        lease.reuse();
                    }
                    return delivered.result;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(INTERRUPTED.into()),
                Err(mpsc::RecvTimeoutError::Timeout) if left.is_zero() => {
                    return Err(TIMEOUT.into())
                }
                _ => {}
            }
        }
    })();
    if let Err(error) = &result {
        if error != CANCELLED {
            if let Ok(mut errors) = recent_errors().lock() {
                // An explicit reselect/retry supersedes failures still in flight.
                if error_generation == Some(errors.generation) {
                    errors
                        .entries
                        .push_back((path.into(), Instant::now(), error.clone()));
                    while errors.entries.len() > 8 {
                        errors.entries.pop_front();
                    }
                }
            }
        }
    }
    result
}
pub fn metadata(path: &Path) -> Result<Metadata, String> {
    request(path, false).map(|r| r.metadata)
}
pub(crate) fn read(path: &Path) -> Result<Input, String> {
    request(path, true)
}
pub(crate) fn abandoned_buffer_bytes() -> u64 {
    let Ok(mut state) = pool().state.lock() else {
        return MAX_BYTES;
    };
    state.abandoned.retain(|s| s.strong_count() > 0);
    state
        .abandoned
        .iter()
        .filter_map(Weak::upgrade)
        .map(|s| s.load(Ordering::Acquire) as u64)
        .sum()
}
fn native_metadata(path: &Path) -> Result<Metadata, String> {
    #[cfg(all(unix, any(test, feature = "qa-source-block")))]
    fault_wait(path);
    let meta = fs::metadata(path).map_err(|e| format!("无法读取照片：{e}"))?;
    Ok(Metadata {
        canonical: fs::canonicalize(path).map_err(|e| format!("照片路径无效：{e}"))?,
        len: meta.len(),
        modified: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos()),
        is_file: meta.is_file(),
        is_dir: meta.is_dir(),
    })
}
fn write_line(writer: &mut impl Write, marker: &str, value: &impl Serialize) -> io::Result<()> {
    writer.write_all(marker.as_bytes())?;
    serde_json::to_writer(&mut *writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}
fn handle(request: Request) -> io::Result<()> {
    let mut output = io::stdout().lock();
    let metadata = native_metadata(&request.path).and_then(|m| {
        if request.read && !m.is_file {
            Err("请选择图片文件，不能直接处理目录".into())
        } else if request.read && m.len > MAX_BYTES {
            Err("单张照片文件超过 128 MB，请先缩小文件".into())
        } else {
            Ok(m)
        }
    });
    let metadata = match metadata {
        Ok(meta) => meta,
        Err(error) => {
            return write_line(
                &mut output,
                HEADER,
                &Head {
                    id: request.id,
                    metadata: None,
                    error: Some(error),
                    body: false,
                },
            )
        }
    };
    let file = if request.read {
        Some(File::open(&metadata.canonical))
    } else {
        None
    };
    if let Some(Err(error)) = &file {
        return write_line(
            &mut output,
            HEADER,
            &Head {
                id: request.id,
                metadata: None,
                error: Some(format!("无法打开照片：{error}")),
                body: false,
            },
        );
    }
    write_line(
        &mut output,
        HEADER,
        &Head {
            id: request.id,
            metadata: Some(metadata.clone()),
            error: None,
            body: request.read,
        },
    )?;
    if let Some(Ok(mut file)) = file {
        let transfer = (|| -> Result<(), String> {
            let mut buffer = [0; CHUNK];
            let mut total = 0u64;
            loop {
                let n = file
                    .read(&mut buffer)
                    .map_err(|e| format!("无法读取照片：{e}"))?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > MAX_BYTES {
                    return Err("单张照片文件超过 128 MB，请先缩小文件".into());
                }
                output
                    .write_all(&(n as u32).to_le_bytes())
                    .map_err(|e| e.to_string())?;
                output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                #[cfg(all(unix, any(test, feature = "qa-source-block")))]
                if request
                    .path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|p| p.to_str())
                    == Some("margin-read-fixtures")
                    && request
                        .path
                        .with_extension("margin-qa-body-block")
                        .is_file()
                {
                    output.flush().map_err(|e| e.to_string())?;
                    let _ = fs::write(
                        request.path.with_extension("margin-qa-body-entered"),
                        format!("{},{}", std::process::id(), total),
                    );
                    let _ = File::open(request.path.with_extension("margin-qa-pipe"));
                }
            }
            let after = file.metadata().map_err(|e| e.to_string())?;
            let modified = after
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos());
            if total != metadata.len || after.len() != metadata.len || modified != metadata.modified
            {
                return Err("读取期间照片已更改，请重新选择照片或重试。".into());
            }
            Ok(())
        })();
        output.write_all(&0u32.to_le_bytes())?;
        write_line(
            &mut output,
            FOOTER,
            &End {
                id: request.id,
                error: transfer.err(),
            },
        )?;
    }
    Ok(())
}
fn exit_worker() -> ! {
    #[cfg(unix)]
    unsafe {
        libc::_exit(0)
    }
    #[cfg(not(unix))]
    std::process::exit(0)
}
fn worker_loop() -> ! {
    let (jobs, requests) = mpsc::sync_channel::<Request>(1);
    thread::spawn(move || {
        while let Ok(request) = requests.recv() {
            if handle(request).is_err() {
                exit_worker();
            }
        }
    });
    // Keep stdin independent of the filesystem operation. Parent exit closes
    // this pipe and terminates even a reader blocked in open/read.
    let mut input = io::stdin().lock();
    loop {
        let mut data = Vec::new();
        let n = (&mut input)
            .take(65537)
            .read_until(b'\n', &mut data)
            .unwrap_or(0);
        if n == 0 || n > 65536 {
            exit_worker();
        }
        match serde_json::from_slice::<Request>(&data) {
            Ok(request) => {
                if jobs.send(request).is_err() {
                    exit_worker();
                }
            }
            Err(_) => exit_worker(),
        }
    }
}
pub fn worker_entry_if_requested() -> bool {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == WORKER_FLAG)
    {
        worker_loop();
    }
    false
}

#[cfg(all(unix, any(test, feature = "qa-source-block")))]
pub(crate) fn fault_wait(path: &std::path::Path) {
    if path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        != Some("margin-read-fixtures")
    {
        return;
    }
    if path.with_extension("margin-qa-block").is_file() {
        let _ = std::fs::write(
            path.with_extension("margin-qa-entered"),
            std::process::id().to_string(),
        );
        let _ = std::fs::File::open(path.with_extension("margin-qa-pipe"));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{imaging, jobs::Backend, model::RenderSettings};
    use std::{
        ffi::CString,
        fs,
        sync::mpsc,
        time::{Duration, Instant},
    };

    #[test]
    fn worker_entry() {
        if std::env::var_os("MARGIN_TEST_SOURCE_WORKER").is_some() {
            worker_loop();
        }
    }

    #[test]
    fn blocked_source_read_has_a_deadline_and_releases_the_renderer() {
        let root = tempfile::tempdir().unwrap();
        let fixtures = root.path().join("margin-read-fixtures");
        fs::create_dir(&fixtures).unwrap();
        let bad = fixtures.join("blocked.jpg");
        let good = fixtures.join("healthy.jpg");
        for p in [&bad, &good] {
            image::RgbImage::from_pixel(320, 240, image::Rgb([208, 166, 125]))
                .save(p)
                .unwrap();
        }
        let backend = Backend::new(root.path().join("state")).unwrap();
        let photos = backend
            .import_photos(vec![
                bad.to_string_lossy().into(),
                good.to_string_lossy().into(),
            ])
            .unwrap()
            .photos;
        imaging::clear_image_cache();
        let fifo = CString::new(
            bad.with_extension("margin-qa-pipe")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        fs::write(bad.with_extension("margin-qa-block"), b"QA").unwrap();
        let (first_tx, first_rx) = mpsc::channel();
        let first_backend = backend.clone();
        let first_id = photos[0].id.clone();
        let first = std::thread::spawn(move || {
            let result = with_timeout(Duration::from_millis(150), || {
                first_backend.preview(&first_id, &RenderSettings::default())
            });
            first_tx.send(result).unwrap();
        });
        let wait = Instant::now();
        while !bad.with_extension("margin-qa-entered").is_file()
            && wait.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        let entered = bad.with_extension("margin-qa-entered").is_file();
        let first_before_release = first_rx.recv_timeout(Duration::from_millis(350)).ok();
        let (next_tx, next_rx) = mpsc::channel();
        let next_backend = backend.clone();
        let next_id = photos[1].id.clone();
        let next = std::thread::spawn(move || {
            next_tx
                .send(next_backend.preview(&next_id, &RenderSettings::default()))
                .unwrap();
        });
        let next_before_release = next_rx.recv_timeout(Duration::from_millis(350)).ok();
        // Always release the owned FIFO before asserting, including on red runs.
        let _ = fs::remove_file(bad.with_extension("margin-qa-block"));
        let fd = unsafe { libc::open(fifo.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
        if fd >= 0 {
            unsafe {
                libc::close(fd);
            }
        }
        first.join().unwrap();
        next.join().unwrap();
        assert!(
            entered,
            "The controlled regular-photo read must reach the real FIFO open"
        );
        assert!(
            first_before_release.is_some_and(|r| r.is_err_and(|e| e.contains("读取照片超时"))),
            "A blocked source kept the renderer waiting instead of returning a recoverable timeout"
        );
        assert!(
            next_before_release.is_some_and(|r| r.is_ok()),
            "A later healthy photo must finish while the faulty source remains blocked"
        );
    }
    #[test]
    fn cancelling_a_blocked_batch_releases_input_and_resumes_frozen_settings() {
        let root = tempfile::tempdir().unwrap();
        let fixtures = root.path().join("margin-read-fixtures");
        fs::create_dir(&fixtures).unwrap();
        let blocked = fixtures.join("blocked.jpg");
        let healthy = fixtures.join("healthy.jpg");
        for p in [&blocked, &healthy] {
            image::RgbImage::from_pixel(320, 240, image::Rgb([208, 166, 125]))
                .save(p)
                .unwrap();
        }
        let before = fs::read(&blocked).unwrap();
        let state = root.path().join("state");
        let backend = Backend::new(state.clone()).unwrap();
        let photos = backend
            .import_photos(vec![
                blocked.to_string_lossy().into(),
                healthy.to_string_lossy().into(),
            ])
            .unwrap()
            .photos;
        let fifo = CString::new(
            blocked
                .with_extension("margin-qa-pipe")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        fs::write(blocked.with_extension("margin-qa-block"), b"QA").unwrap();
        let output = root.path().join("out");
        let (tx, rx) = mpsc::channel();
        let sink: crate::jobs::ProgressSink = Arc::new(move |batch| {
            if batch.status != "running" {
                let _ = tx.send(batch);
            }
        });
        backend
            .start_batch(
                output.to_string_lossy().into(),
                RenderSettings::default(),
                photos.iter().map(|p| p.id.clone()).collect(),
                sink,
            )
            .unwrap();
        let wait = Instant::now();
        while !blocked.with_extension("margin-qa-entered").is_file()
            && wait.elapsed() < Duration::from_secs(10)
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(blocked.with_extension("margin-qa-entered").is_file());
        let at = Instant::now();
        backend.cancel_batch().unwrap();
        let batch = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("Cancellation must interrupt a blocked input rather than wait eight seconds");
        assert!(at.elapsed() < Duration::from_secs(1));
        assert!(batch.items.iter().all(|i| i.status == "cancelled"));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        fs::remove_file(blocked.with_extension("margin-qa-block")).unwrap();
        let selected_again = backend
            .import_photos(vec![blocked.to_string_lossy().into()])
            .unwrap();
        assert!(selected_again.photos.is_empty());
        assert_eq!(
            backend
                .snapshot()
                .unwrap()
                .batch
                .unwrap()
                .settings
                .border_percent,
            5.0
        );
        let mut changed = RenderSettings::default();
        changed.border_percent = 17.0;
        backend.save_settings(changed).unwrap();
        drop(backend);
        let restored = Backend::new(state).unwrap();
        let (tx, rx) = mpsc::channel();
        let sink: crate::jobs::ProgressSink = Arc::new(move |batch| {
            if batch.status != "running" {
                let _ = tx.send(batch);
            }
        });
        restored.resume_batch(false, sink).unwrap();
        let batch = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(batch.items.iter().all(|i| i.status == "completed"));
        assert_eq!(batch.settings.border_percent, 5.0);
        assert!(batch
            .items
            .iter()
            .all(|i| (i.width, i.height) == (Some(344), Some(264))));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 2);
        assert_eq!(fs::read(&blocked).unwrap(), before);
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn repeated_timeouts_cancellation_and_retries_reclaim_reader_resources() {
        let root = tempfile::tempdir().unwrap();
        let fixtures = root.path().join("margin-read-fixtures");
        fs::create_dir(&fixtures).unwrap();
        let path = fixtures.join("regular.jpg");
        image::RgbImage::from_pixel(32, 24, image::Rgb([20, 40, 60]))
            .save(&path)
            .unwrap();
        let fifo = CString::new(
            path.with_extension("margin-qa-pipe")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        metadata(&path).unwrap();
        fn settled() {
            let at = Instant::now();
            loop {
                let s = pool().state.lock().unwrap();
                if s.live == s.idle.len() {
                    break;
                }
                drop(s);
                assert!(
                    at.elapsed() < Duration::from_secs(2),
                    "Reader or pipe driver did not reap"
                );
                thread::sleep(Duration::from_millis(5));
            }
        }
        fn snapshot() -> serde_json::Value {
            let threads = std::process::Command::new("ps")
                .args(["-M", "-p", &std::process::id().to_string()])
                .output()
                .unwrap();
            let state = pool().state.lock().unwrap();
            serde_json::json!({"live":state.live,"idle":state.idle.len(),"retiredRefs":state.abandoned.len(),"fds":fs::read_dir("/dev/fd").unwrap().count(),"threads":String::from_utf8_lossy(&threads.stdout).lines().skip(1).filter(|l|!l.trim().is_empty()).count()})
        }
        let before = snapshot();
        let mut samples = vec![before.clone()];
        let mut retired_pids = Vec::new();
        // No healthy decode in this interval: it must not be needed to sweep dead reader records.
        for round in 0..24 {
            clear_failures();
            let entered = path.with_extension("margin-qa-entered");
            let _ = fs::remove_file(&entered);
            fs::write(path.with_extension("margin-qa-block"), b"QA").unwrap();
            let at = Instant::now();
            let result = if round < 12 {
                with_timeout(Duration::from_millis(100), || metadata(&path))
            } else {
                let cancel = Arc::new(AtomicBool::new(false));
                let flag = cancel.clone();
                let ready = entered.clone();
                let trigger = thread::spawn(move || {
                    let at = Instant::now();
                    while !ready.is_file() && at.elapsed() < Duration::from_secs(1) {
                        thread::sleep(Duration::from_millis(2));
                    }
                    flag.store(true, Ordering::Release);
                });
                let result = with_cancel(cancel, || {
                    with_timeout(Duration::from_millis(250), || metadata(&path))
                });
                trigger.join().unwrap();
                result
            };
            assert!(result.is_err_and(|e| e == if round < 12 { TIMEOUT } else { CANCELLED }));
            assert!(at.elapsed() < Duration::from_secs(1));
            let pid = fs::read_to_string(&entered)
                .unwrap()
                .parse::<i32>()
                .unwrap();
            retired_pids.push(pid);
            settled();
            assert_eq!(
                unsafe { libc::kill(pid, 0) },
                -1,
                "Retired helper still exists"
            );
            samples.push(snapshot());
        }
        fs::remove_file(path.with_extension("margin-qa-block")).unwrap();
        for _ in 0..40 {
            clear_failures();
            assert_eq!(read(&path).unwrap().bytes, fs::read(&path).unwrap());
        }
        settled();
        let after = snapshot();
        samples.push(after.clone());
        if let Ok(report) = std::env::var("MARGIN_SOURCE_STRESS_REPORT") {
            fs::write(report,serde_json::to_vec_pretty(&serde_json::json!({"timeoutCycles":12,"cancelCycles":12,"healthyRetries":40,"retiredPids":retired_pids,"samples":samples,"before":before,"after":after})).unwrap()).unwrap();
        }
        assert!(samples.iter().all(|s| s["live"].as_u64().unwrap() <= 2));
        assert!(
            samples
                .iter()
                .all(|s| s["retiredRefs"].as_u64().unwrap() <= 2),
            "Retired weak-reader records grow without a healthy decode"
        );
        assert!(
            after["fds"].as_u64().unwrap() <= before["fds"].as_u64().unwrap() + 2,
            "File descriptors grew after retrying"
        );
        assert!(
            after["threads"].as_u64().unwrap() <= before["threads"].as_u64().unwrap() + 2,
            "Worker/driver threads grew after retrying"
        );
    }

    #[test]
    fn an_older_failure_cannot_repopulate_the_cache_after_explicit_retry() {
        let root = tempfile::tempdir().unwrap();
        let fixtures = root.path().join("margin-read-fixtures");
        fs::create_dir(&fixtures).unwrap();
        let path = fixtures.join("regular.jpg");
        image::RgbImage::from_pixel(32, 24, image::Rgb([20, 40, 60]))
            .save(&path)
            .unwrap();
        let fifo = CString::new(
            path.with_extension("margin-qa-pipe")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        fs::write(path.with_extension("margin-qa-block"), b"QA").unwrap();
        let oldpath = path.clone();
        let old =
            thread::spawn(move || with_timeout(Duration::from_millis(250), || metadata(&oldpath)));
        let at = Instant::now();
        while !path.with_extension("margin-qa-entered").is_file()
            && at.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(2));
        }
        assert!(path.with_extension("margin-qa-entered").is_file());
        fs::remove_file(path.with_extension("margin-qa-block")).unwrap();
        clear_failures();
        assert!(
            metadata(&path).is_ok(),
            "A reselected source must be readable before the old call finishes"
        );
        assert!(old.join().unwrap().is_err_and(|e| e == TIMEOUT));
        assert!(
            metadata(&path).is_ok(),
            "An obsolete timeout poisoned a successful reselect with a new cached failure"
        );
    }
    #[test]
    fn partial_body_timeouts_and_cancellation_release_buffers_and_helpers() {
        let root = tempfile::tempdir().unwrap();
        let fixtures = root.path().join("margin-read-fixtures");
        fs::create_dir(&fixtures).unwrap();
        let path = fixtures.join("regular.jpg");
        image::RgbImage::from_pixel(32, 24, image::Rgb([20, 40, 60]))
            .save(&path)
            .unwrap();
        // A valid JPEG with a bounded trailing payload exercises an allocated input buffer.
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(4 * 1024 * 1024).unwrap();
        drop(file);
        let fifo = CString::new(
            path.with_extension("margin-qa-pipe")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        fs::write(path.with_extension("margin-qa-body-block"), b"QA").unwrap();
        for round in 0..16 {
            clear_failures();
            let entered = path.with_extension("margin-qa-body-entered");
            let _ = fs::remove_file(&entered);
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = cancel.clone();
            let source = path.clone();
            let reader = thread::spawn(move || {
                with_cancel(flag, || {
                    with_timeout(Duration::from_millis(200), || read(&source))
                })
            });
            let at = Instant::now();
            // File existence can precede the writer's first bytes. Wait for a
            // complete receipt, rather than racing fs::write on a busy host.
            let (pid, bytes) = loop {
                if let Ok(receipt) = fs::read_to_string(&entered) {
                    if let Some((pid, bytes)) = receipt.trim().split_once(',') {
                        if let (Ok(pid), Ok(bytes)) = (pid.parse::<i32>(), bytes.parse::<u64>()) {
                            break (pid, bytes);
                        }
                    }
                }
                assert!(at.elapsed() < Duration::from_secs(1),
                    "The actual worker must stream part of the body before blocking");
                thread::sleep(Duration::from_millis(2));
            };
            assert!(bytes > 0 && bytes < 4 * 1024 * 1024);
            if round % 2 == 0 {
                cancel.store(true, Ordering::Release);
            }
            assert!(reader
                .join()
                .unwrap()
                .is_err_and(|e| e == if round % 2 == 0 { CANCELLED } else { TIMEOUT }));
            let at = Instant::now();
            loop {
                let state = pool().state.lock().unwrap();
                if state.live == state.idle.len() {
                    break;
                }
                drop(state);
                assert!(at.elapsed() < Duration::from_secs(2));
                thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
            assert_eq!(
                abandoned_buffer_bytes(),
                0,
                "Retired partial allocation must be released"
            );
        }
        fs::remove_file(path.with_extension("margin-qa-body-block")).unwrap();
        clear_failures();
        assert_eq!(read(&path).unwrap().bytes, fs::read(&path).unwrap());
    }
}

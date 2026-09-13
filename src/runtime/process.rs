//! Bounded, cancellable child-process execution for the Docker and Compose CLIs.
//!
//! Every command carries an explicit deadline, captures stdout and stderr
//! concurrently into bounded buffers, and terminates the process tree that
//! Local Store created when that deadline passes or the caller cancels. The
//! Docker daemon and its containers are never inside that tree: the CLI talks
//! to the daemon over a socket or named pipe, and containers are children of
//! the daemon, so terminating our own group never stops a running container.
use serde::Serialize;
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// Read-only diagnostics: `docker version`, `compose ps`, `compose config`.
pub const DIAGNOSTIC_TIMEOUT: Duration = Duration::from_secs(30);
/// Local lifecycle changes that do not download images.
pub const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(180);
/// First start and image downloads, which are bounded by network speed.
pub const PROVISION_TIMEOUT: Duration = Duration::from_secs(900);

/// How long a freshly installed app has to start answering.
///
/// A minute is enough for something that only has to open a port, and not
/// enough for the apps people actually install: WordPress runs its own
/// installer on first boot, Metabase initialises a schema, and a batch run
/// found both timing out at sixty seconds while they were still working. It is
/// cancellable and the interface says what it is waiting for, so the cost of
/// waiting longer is bounded and visible — whereas the cost of giving up too
/// early is an install that rolls back a perfectly good app.
pub const FIRST_START_TIMEOUT: Duration = Duration::from_secs(180);

/// Maximum bytes retained per stream. Reading continues past this point and is
/// discarded, so the child never blocks writing into a full pipe.
pub const MAX_CAPTURED_BYTES: usize = 256 * 1024;

/// How long a finished command may wait for its readers to reach end of file.
/// A grandchild holding an inherited pipe can keep it open indefinitely, so
/// the wait is always bounded and partial output is used when it expires.
const READER_GRACE: Duration = Duration::from_secs(2);
/// The same bound after the tree has been terminated, where output is already
/// complete enough to explain the failure.
const ABANDONED_READER_GRACE: Duration = Duration::from_millis(200);
/// How long a terminated process tree may take to be reaped.
const REAP_GRACE: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Cooperative cancellation shared with a running command.
///
/// This is the runner boundary only. Cancelling an in-flight *operation* — an
/// install transaction, a health wait, or a button in the launcher — is
/// separate work under tasks 14 and 24 and is not wired through yet.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}
impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub remove_env: Vec<String>,
}
impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        timeout: Duration,
    ) -> Self {
        Self {
            program: program.into(),
            args,
            cwd,
            timeout,
            remove_env: Vec::new(),
        }
    }
    pub(crate) fn docker(args: Vec<String>, cwd: Option<PathBuf>, timeout: Duration) -> Self {
        Self::new("docker", args, cwd, timeout)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ProcessOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    /// At least one stream exceeded [`MAX_CAPTURED_BYTES`] and was cut short.
    pub truncated: bool,
}

/// Stable diagnostic codes; callers must never classify human-readable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessErrorCode {
    ProcessUnavailable,
    ProcessFailed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcessError {
    pub code: ProcessErrorCode,
    pub message: String,
}
impl ProcessError {
    pub fn new(code: ProcessErrorCode, message: impl AsRef<str>) -> Self {
        Self {
            code,
            message: redact(message.as_ref()),
        }
    }
}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProcessError {}

pub trait ProcessRunner: Send + Sync {
    fn engine_binding(&self) -> crate::error::AppResult<Option<super::engine::EngineBinding>> {
        Ok(None)
    }
    /// Run `command`, abandoning it when `cancel` is set or the deadline passes.
    fn run_cancellable(
        &self,
        command: &CommandSpec,
        cancel: &CancelToken,
    ) -> Result<ProcessOutput, ProcessError>;

    /// Run `command` to its own deadline with no external cancellation.
    fn run(&self, command: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
        self.run_cancellable(command, &CancelToken::new())
    }
}

pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn engine_binding(&self) -> crate::error::AppResult<Option<super::engine::EngineBinding>> {
        super::engine::EngineBinding::discover(self).map(Some)
    }
    fn run_cancellable(
        &self,
        spec: &CommandSpec,
        cancel: &CancelToken,
    ) -> Result<ProcessOutput, ProcessError> {
        let mut command = Command::new(&spec.program);
        for key in &spec.remove_env {
            command.env_remove(key);
        }
        command
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        group::prepare(&mut command);

        let mut child = command.spawn().map_err(|error| {
            ProcessError::new(
                if error.kind() == std::io::ErrorKind::NotFound {
                    ProcessErrorCode::ProcessUnavailable
                } else {
                    ProcessErrorCode::ProcessFailed
                },
                format!("failed to run {}: {error}", spec.program),
            )
        })?;
        let group = group::Group::attach(&child);

        let stdout = Capture::start(child.stdout.take());
        let stderr = Capture::start(child.stderr.take());

        let deadline = Instant::now() + spec.timeout;
        let outcome = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Outcome::Exited(status.success()),
                Ok(None) => {}
                Err(error) => {
                    break Outcome::Failed(format!("failed to wait for {}: {error}", spec.program))
                }
            }
            if cancel.is_cancelled() {
                break Outcome::Cancelled;
            }
            if Instant::now() >= deadline {
                break Outcome::TimedOut;
            }
            std::thread::sleep(POLL_INTERVAL);
        };

        // Readers are never joined without a bound: a process can exit while a
        // grandchild still holds the write end of the pipe, so end of file may
        // never arrive. Whatever has been captured by then is used instead.
        let grace = match outcome {
            Outcome::Exited(_) => READER_GRACE,
            // A command that timed out, was cancelled, or whose status could
            // not be read is abandoned: its result is unusable either way, so
            // the tree is stopped rather than left running unattended.
            Outcome::TimedOut | Outcome::Cancelled | Outcome::Failed(_) => {
                match &group {
                    Some(group) => group.terminate(),
                    // Without a job object or process group only the direct
                    // child can be stopped; anything it started may survive.
                    None => {
                        let _ = child.kill();
                    }
                }
                reap(&mut child);
                ABANDONED_READER_GRACE
            }
        };
        // One deadline covers both readers, so a stalled pair cannot spend the
        // grace twice over.
        let readers_by = Instant::now() + grace;
        let (out_text, out_cut) = stdout.finish(readers_by);
        let (err_text, err_cut) = stderr.finish(readers_by);

        match outcome {
            Outcome::Exited(success) => Ok(ProcessOutput {
                success,
                stdout: out_text,
                stderr: err_text,
                truncated: out_cut || err_cut,
            }),
            Outcome::TimedOut => Err(ProcessError::new(
                ProcessErrorCode::TimedOut,
                abandoned(
                    &format!(
                        "{} timed out after {} seconds",
                        spec.program,
                        spec.timeout.as_secs()
                    ),
                    &out_text,
                    &err_text,
                ),
            )),
            Outcome::Cancelled => Err(ProcessError::new(
                ProcessErrorCode::Cancelled,
                abandoned(
                    &format!("{} was cancelled", spec.program),
                    &out_text,
                    &err_text,
                ),
            )),
            Outcome::Failed(error) => {
                Err(ProcessError::new(ProcessErrorCode::ProcessFailed, error))
            }
        }
    }
}

enum Outcome {
    Exited(bool),
    TimedOut,
    Cancelled,
    Failed(String),
}

/// Keep the last useful lines of a command that never reported an exit status.
fn abandoned(reason: &str, stdout: &str, stderr: &str) -> String {
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        return reason.to_owned();
    }
    let mut tail = detail.lines().rev().take(5).collect::<Vec<_>>();
    tail.reverse();
    format!("{reason}: {}", redact(&tail.join("\n")))
}

fn reap(child: &mut std::process::Child) {
    let deadline = Instant::now() + REAP_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// A bounded sink filled by a detached reader thread.
#[derive(Default)]
struct Bounded {
    bytes: Vec<u8>,
    dropped: usize,
}
impl Bounded {
    fn push(&mut self, chunk: &[u8]) {
        let room = MAX_CAPTURED_BYTES.saturating_sub(self.bytes.len());
        let keep = room.min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..keep]);
        self.dropped += chunk.len() - keep;
    }
}

struct Capture {
    sink: Arc<Mutex<Bounded>>,
    done: mpsc::Receiver<()>,
}
impl Capture {
    fn start<R: Read + Send + 'static>(reader: Option<R>) -> Self {
        let sink = Arc::new(Mutex::new(Bounded::default()));
        let (finished, done) = mpsc::channel();
        if let Some(mut reader) = reader {
            let sink = Arc::clone(&sink);
            std::thread::spawn(move || {
                let mut chunk = [0_u8; 8192];
                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        // Reading continues past the cap so the pipe keeps
                        // draining and the child is never blocked on a write.
                        Ok(read) => lock(&sink).push(&chunk[..read]),
                    }
                }
                let _ = finished.send(());
            });
        }
        Self { sink, done }
    }
    /// Wait until `deadline` for end of file, then take whatever was captured.
    fn finish(self, deadline: Instant) -> (String, bool) {
        let _ = self
            .done
            .recv_timeout(deadline.saturating_duration_since(Instant::now()));
        let captured = lock(&self.sink);
        (decode(&captured.bytes), captured.dropped > 0)
    }
}

fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Decode captured bytes, dropping a trailing UTF-8 sequence that the capture
/// bound cut in half rather than turning it into a replacement character.
fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(without_incomplete_tail(bytes)).into_owned()
}

fn without_incomplete_tail(bytes: &[u8]) -> &[u8] {
    let mut index = bytes.len();
    let mut continuations = 0;
    while index > 0 && continuations < 3 {
        let byte = bytes[index - 1];
        if byte & 0b1100_0000 == 0b1000_0000 {
            index -= 1;
            continuations += 1;
            continue;
        }
        let needed = if byte & 0b1000_0000 == 0 {
            1
        } else if byte & 0b1110_0000 == 0b1100_0000 {
            2
        } else if byte & 0b1111_0000 == 0b1110_0000 {
            3
        } else if byte & 0b1111_1000 == 0b1111_0000 {
            4
        } else {
            return bytes;
        };
        return if continuations + 1 < needed {
            &bytes[..index - 1]
        } else {
            bytes
        };
    }
    bytes
}

/// Placeholder written in place of a credential-shaped value.
pub const REDACTED: &str = "***";

/// Key fragments whose value is treated as a secret wherever they appear.
const SENSITIVE_FRAGMENTS: &[&str] = &[
    "PASSWORD",
    "PASSWD",
    "SECRET",
    "TOKEN",
    "APIKEY",
    "API_KEY",
    "ACCESSKEY",
    "ACCESS_KEY",
    "PRIVATEKEY",
    "PRIVATE_KEY",
    "CREDENTIAL",
];
/// Keys that are secrets exactly, but whose fragments are too common to match
/// loosely — "AUTH" must not redact "AUTHOR" or "AUTHORITY".
const SENSITIVE_KEYS: &[&str] = &["AUTH", "PASS", "PWD"];

/// Remove credential-shaped values from text that will be shown as a
/// diagnostic: a Doctor detail, a failure message, or a future operation
/// event. Such text is routinely pasted into bug reports.
///
/// This is deliberately **not** applied to `logs`, which the user explicitly
/// asked to see. `docker compose logs` shows them the same bytes, so redacting
/// there would hide real debugging detail while protecting nothing.
pub fn redact(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let line = redact_url_userinfo(line);
            let line = redact_authorization_header(&line);
            redact_assignments(&redact_auth_schemes(&line))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Redact diagnostic text, dropping a trailing partial line when the capture
/// bound was reached. A cut can land inside a URL before its `@` arrives, which
/// would otherwise leave the visible half of a credential behind.
pub fn redact_diagnostic(text: &str, truncated: bool) -> String {
    match (truncated, text.rfind('\n')) {
        (true, Some(last)) => redact(&text[..last]),
        _ => redact(text),
    }
}

/// `scheme://user:password@host` becomes `scheme://***@host`.
fn redact_url_userinfo(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(scheme_end) = rest.find("://") {
        let authority_start = scheme_end + 3;
        out.push_str(&rest[..authority_start]);
        let tail = &rest[authority_start..];
        let end = tail
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '/' | '?' | '#' | '"' | '\'' | ',' | ')')
            })
            .unwrap_or(tail.len());
        let authority = &tail[..end];
        match authority.rfind('@') {
            Some(at) => {
                out.push_str(REDACTED);
                out.push_str(&authority[at..]);
            }
            None => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// An `Authorization:` header keeps its scheme name and loses everything after
/// it. The scheme is not a secret and says a great deal about a failure.
fn redact_authorization_header(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let Some(at) = lower.find("authorization:") else {
        return line.to_owned();
    };
    let bytes = line.as_bytes();
    let mut cursor = at + "authorization:".len();
    while cursor < bytes.len() && matches!(bytes[cursor], b' ' | b'\t') {
        cursor += 1;
    }
    for scheme in ["bearer", "basic", "digest", "negotiate"] {
        if lower[cursor..].starts_with(scheme) {
            cursor += scheme.len();
            break;
        }
    }
    let mut out = String::with_capacity(line.len());
    out.push_str(&line[..cursor]);
    if line[cursor..].trim_start_matches([' ', '\t']).is_empty() {
        out.push_str(&line[cursor..]);
    } else {
        out.push(' ');
        out.push_str(REDACTED);
    }
    out
}

/// `Authorization: Bearer <token>` and `Basic <token>` lose their token.
fn redact_auth_schemes(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut out = String::with_capacity(line.len());
    let mut index = 0;
    while index < line.len() {
        let next = ["bearer ", "basic "]
            .iter()
            .filter_map(|keyword| {
                lower[index..]
                    .find(keyword)
                    .map(|at| (index + at, keyword.len()))
            })
            .min_by_key(|(at, _)| *at);
        let Some((at, keyword_len)) = next else {
            break;
        };
        let value_start = at + keyword_len;
        out.push_str(&line[index..value_start]);
        let tail = &line[value_start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | ',' | ';'))
            .unwrap_or(tail.len());
        if end > 0 {
            out.push_str(REDACTED);
        }
        index = value_start + end;
    }
    out.push_str(&line[index..]);
    out
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.trim_matches('"');
    if key.is_empty() {
        return false;
    }
    let upper = key.to_ascii_uppercase();
    SENSITIVE_KEYS.contains(&upper.as_str())
        || SENSITIVE_FRAGMENTS
            .iter()
            .any(|fragment| upper.contains(fragment))
}

fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

/// `PASSWORD=hunter2`, `"token": "abc"` and similar lose their value. The
/// separator may be `=` or `:`, so a value cut short by the capture bound is
/// replaced through to the end of the line rather than partly exposed.
fn redact_assignments(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut copied = 0;
    let mut index = 0;
    while index < bytes.len() {
        if !matches!(bytes[index], b'=' | b':') {
            index += 1;
            continue;
        }
        let mut key_end = index;
        while key_end > copied && bytes[key_end - 1].is_ascii_whitespace() {
            key_end -= 1;
        }
        if key_end > copied && bytes[key_end - 1] == b'"' {
            key_end -= 1;
        }
        let mut key_start = key_end;
        while key_start > copied && is_key_byte(bytes[key_start - 1]) {
            key_start -= 1;
        }
        if !is_sensitive_key(&line[key_start..key_end]) {
            index += 1;
            continue;
        }
        out.push_str(&line[copied..=index]);
        let mut value = index + 1;
        while value < bytes.len() && matches!(bytes[value], b' ' | b'\t') {
            value += 1;
        }
        out.push_str(&line[index + 1..value]);
        if value < bytes.len() && bytes[value] == b'"' {
            if let Some(close) = line[value + 1..].find('"') {
                out.push('"');
                out.push_str(REDACTED);
                out.push('"');
                copied = value + 1 + close + 1;
                index = copied;
                continue;
            }
        }
        let mut end = value;
        while end < bytes.len()
            && !matches!(
                bytes[end],
                b' ' | b'\t' | b',' | b';' | b'}' | b')' | b'"' | b'\''
            )
        {
            end += 1;
        }
        if end > value {
            out.push_str(REDACTED);
        }
        copied = end;
        index = end.max(index + 1);
    }
    out.push_str(&line[copied..]);
    out
}

#[cfg(windows)]
mod group {
    use std::{
        os::windows::{io::AsRawHandle, process::CommandExt},
        process::{Child, Command},
        ptr::null,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject},
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Preserve the hidden console the packaged Windows app relies on.
    pub fn prepare(command: &mut Command) {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    /// A job object owning the process Local Store spawned and everything it
    /// goes on to create. No kill-on-close limit is set, so the tree survives
    /// a successful run and is terminated only on timeout or cancellation.
    pub struct Group(HANDLE);
    // The handle is owned by this value alone and closed once, on drop.
    unsafe impl Send for Group {}
    unsafe impl Sync for Group {}

    impl Group {
        pub fn attach(child: &Child) -> Option<Self> {
            unsafe {
                let job = CreateJobObjectW(null(), null());
                if job.is_null() {
                    return None;
                }
                if AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(Self(job))
            }
        }
        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }
    impl Drop for Group {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(unix)]
mod group {
    use std::{
        os::unix::process::CommandExt,
        process::{Child, Command},
        thread::sleep,
        time::Duration,
    };

    /// Put the child in its own process group so the tree can be signalled
    /// without reaching the shell, the Docker daemon or unrelated processes.
    pub fn prepare(command: &mut Command) {
        command.process_group(0);
    }

    pub struct Group(i32);

    impl Group {
        pub fn attach(child: &Child) -> Option<Self> {
            i32::try_from(child.id())
                .ok()
                .filter(|id| *id > 0)
                .map(Group)
        }
        pub fn terminate(&self) {
            unsafe {
                libc::kill(-self.0, libc::SIGTERM);
            }
            sleep(Duration::from_millis(250));
            unsafe {
                libc::kill(-self.0, libc::SIGKILL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_drops_a_half_written_character_instead_of_corrupting_it() {
        let mut cut = b"ok ".to_vec();
        cut.extend_from_slice(&"e\u{0301}".as_bytes()[..2]);
        assert_eq!(decode(&cut), "ok e");
        assert_eq!(decode("ok \u{00e9}".as_bytes()), "ok \u{00e9}");
        assert_eq!(decode(b"plain"), "plain");
    }

    #[test]
    fn bounded_buffers_stop_growing_and_record_the_loss() {
        let mut bounded = Bounded::default();
        bounded.push(&vec![b'x'; MAX_CAPTURED_BYTES + 10]);
        bounded.push(b"more");
        assert_eq!(bounded.bytes.len(), MAX_CAPTURED_BYTES);
        assert_eq!(bounded.dropped, 14);
    }

    #[test]
    fn credentials_in_diagnostics_are_replaced() {
        assert_eq!(
            redact("Error response from postgres://admin:hunter2@db:5432/app"),
            "Error response from postgres://***@db:5432/app"
        );
        assert_eq!(
            redact("POSTGRES_PASSWORD=hunter2 stays out of reports"),
            "POSTGRES_PASSWORD=*** stays out of reports"
        );
        assert_eq!(
            redact("{\"identitytoken\": \"abc.def\", \"name\": \"registry\"}"),
            "{\"identitytoken\": \"***\", \"name\": \"registry\"}"
        );
        assert_eq!(
            redact("Authorization: Bearer sk-live-123"),
            "Authorization: Bearer ***"
        );
    }

    #[test]
    fn ordinary_diagnostic_text_is_left_alone() {
        for line in [
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
            "author: Local Store contributors",
            "listening on http://127.0.0.1:5230 at 12:04:31",
            "service memos failed to start: exit status 1",
        ] {
            assert_eq!(redact(line), line, "redaction damaged: {line}");
        }
    }

    #[test]
    fn a_credential_cut_by_the_capture_bound_is_not_left_half_exposed() {
        // The value runs to the end of the line, so a cut inside it is covered.
        assert_eq!(redact("DB_PASSWORD=hunt"), "DB_PASSWORD=***");
        // A URL cut before its "@" has no marker left to match, so the partial
        // final line is dropped instead.
        let cut = "connecting
to postgres://admin:hunt";
        assert_eq!(redact_diagnostic(cut, true), "connecting");
        assert_eq!(
            redact_diagnostic(cut, false),
            "connecting
to postgres://admin:hunt"
        );
    }

    #[test]
    fn redaction_never_loses_the_only_line_it_has() {
        assert_eq!(
            redact_diagnostic("docker: command not found", true),
            "docker: command not found"
        );
    }

    #[test]
    fn a_cancel_token_is_shared_by_every_clone() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}

#[cfg(test)]
mod error_contract_tests {
    use super::*;

    #[test]
    fn serialized_codes_are_stable_and_diagnostics_are_redacted() {
        for (code, wire) in [
            (ProcessErrorCode::ProcessUnavailable, "process_unavailable"),
            (ProcessErrorCode::ProcessFailed, "process_failed"),
            (ProcessErrorCode::TimedOut, "timed_out"),
            (ProcessErrorCode::Cancelled, "cancelled"),
        ] {
            let error = ProcessError::new(code, "Get https://admin:secret@example.com failed");
            let json = serde_json::to_value(&error).unwrap();
            assert_eq!(json["code"], wire);
            assert!(!json["message"].as_str().unwrap().contains("secret"));
            assert_eq!(json["message"], error.to_string());
        }
    }
}

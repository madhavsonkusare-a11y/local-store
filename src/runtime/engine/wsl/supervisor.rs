//! Per-user background owner of the selected engine's pipe lease.
//!
//! This is a launcher child, not a Windows service or login task. It never
//! terminates WSL, stops Docker, changes contexts, or signals processes by PID.
//! The singleton lock establishes liveness; the bounded status is diagnostic.
use super::bootstrap;
#[cfg(windows)]
use super::lease;
#[cfg(windows)]
mod windows_process;
use crate::{
    error::{AppError, AppResult},
    runtime::ProcessRunner,
    storage,
};
use fs4::{FileExt, TryLockError};
use serde::{Deserialize, Serialize};
#[cfg(any(windows, test))]
use std::path::PathBuf;
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const WORKER_LOCK: &str = "supervisor.lock";
const SPAWN_LOCK: &str = "supervisor-spawn.lock";
const STATUS_FILE: &str = "supervisor-status.json";
const STOP_FILE: &str = "supervisor-stop";
const STATUS_MAX_BYTES: u64 = 2048;
const FRESH_MS: u64 = 120_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisorStatus {
    pub schema_version: u32,
    pub running: bool,
    pub process_id: u32,
    pub last_verified_at_ms: u64,
}

fn now_ms() -> AppResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(u64::MAX as u128) as u64)
        .map_err(AppError::internal)
}

fn fresh(status: &SupervisorStatus, now: u64) -> bool {
    status.schema_version == 1
        && status.running
        && status.process_id != 0
        && now
            .checked_sub(status.last_verified_at_ms)
            .is_some_and(|age| age <= FRESH_MS)
}

fn lock_file(state: &Path, name: &str) -> AppResult<File> {
    // The caller verified an existing journal. Do not create missing state.
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state.join(name))?)
}

fn try_claim(file: &File) -> AppResult<bool> {
    match FileExt::try_lock(file) {
        Ok(()) => Ok(true),
        Err(TryLockError::WouldBlock) => Ok(false),
        Err(error) => Err(AppError::internal(error)),
    }
}

fn read_status(state: &Path) -> AppResult<Option<SupervisorStatus>> {
    let mut file = match File::open(state.join(STATUS_FILE)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.by_ref()
        .take(STATUS_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > STATUS_MAX_BYTES {
        return Err(AppError::invalid("Engine supervisor status is too large."));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| AppError::invalid("Engine supervisor status is invalid."))
}

/// Read-only liveness. A stale PID or status file alone is never accepted.
pub fn status() -> AppResult<Option<SupervisorStatus>> {
    status_at(&storage::managed_engine_state_root())
}

fn status_at(state: &Path) -> AppResult<Option<SupervisorStatus>> {
    let lock = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(state.join(WORKER_LOCK))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if try_claim(&lock)? {
        FileExt::unlock(&lock)?;
        return Ok(None);
    }
    Ok(read_status(state)?.filter(|value| fresh(value, now_ms().unwrap_or(0))))
}

fn verify_selection(
    runner: &dyn ProcessRunner,
    state: &Path,
) -> AppResult<bootstrap::BootstrapJournal> {
    if !crate::engine_setup::selected_at(state)?.is_some_and(|value| value.is_wsl()) {
        return Err(AppError::invalid(
            "Engine supervision requires the explicitly selected Local Store engine.",
        ));
    }
    if !matches!(
        bootstrap::inspect(runner, state)?,
        bootstrap::BootstrapStatus::Recovery {
            disposition: bootstrap::RecoveryDisposition::Ready,
            ..
        }
    ) {
        return Err(AppError::invalid(
            "Engine supervision requires freshly verified Local Store ownership.",
        ));
    }
    bootstrap::load(state)?.ok_or_else(|| AppError::invalid("Engine ownership journal is missing."))
}

#[cfg(windows)]
fn launcher_executable() -> AppResult<Option<PathBuf>> {
    let executable = std::env::current_exe()?;
    let filename = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if filename.eq_ignore_ascii_case("local-store.exe") {
        return Ok(Some(executable));
    }
    if filename.eq_ignore_ascii_case("local-store-mcp.exe") {
        let launcher = executable.with_file_name("local-store.exe");
        if !launcher.is_file() {
            return Err(AppError::invalid("Keep the Local Store launcher beside the agent connector to maintain its background engine."));
        }
        return Ok(Some(launcher));
    }
    // Library test/example processes retain a bounded process-local session;
    // they must never recursively launch themselves as a supervisor.
    Ok(None)
}

/// Called after obtaining a local lease. Only the product's selected native
/// state may acquire persistent supervision; custom state paths and non-product
/// test/example binaries cannot. Product-binary proofs use explicit child mode.
pub(crate) fn ensure_background(runner: &dyn ProcessRunner, state: &Path) -> AppResult<()> {
    #[cfg(windows)]
    {
        use std::time::{Duration, Instant};
        if state != storage::managed_engine_state_root() {
            return Ok(());
        }
        // An explicit qualification-harness setting, applied only to its child
        // process environment. Ownership and selection checks remain required
        // by the caller; this changes the helper lifetime, never its target.
        if std::env::var_os("LOCAL_STORE_ENGINE_SUPERVISOR_PROCESS_ONLY")
            .is_some_and(|value| value == "1")
        {
            return Ok(());
        }
        let Some(executable) = launcher_executable()? else {
            return Ok(());
        };
        verify_selection(runner, state)?;
        if status_at(state)?.is_some() {
            return Ok(());
        }
        let spawn_lock = lock_file(state, SPAWN_LOCK)?;
        if !try_claim(&spawn_lock)? {
            return Err(AppError::invalid(
                "The owned engine supervisor is starting. Retry after it finishes.",
            ));
        }
        if status_at(state)?.is_some() {
            return Ok(());
        }
        let worker_lock = lock_file(state, WORKER_LOCK)?;
        if !try_claim(&worker_lock)? {
            return Err(AppError::invalid("The owned engine supervisor is not reporting fresh ownership. Check its status before retrying."));
        }
        FileExt::unlock(&worker_lock)?;
        for name in [STOP_FILE, STATUS_FILE] {
            match fs::remove_file(state.join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let child = windows_process::Worker::spawn(&executable).map_err(|_| AppError::invalid("Windows could not start the background engine supervisor independently. Open Local Store normally and retry."))?;
        let deadline = Instant::now() + Duration::from_secs(35);
        loop {
            if child.has_exited()? {
                return Err(AppError::invalid(
                    "The background engine supervisor exited before confirming ownership.",
                ));
            }
            if status_at(state)?.is_some_and(|value| value.process_id == child.id()) {
                // Dropping Child does not terminate it. Its stdin WSL lease
                // will survive launcher closure; no handles or secrets shared.
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = child.stop_unacknowledged();
                return Err(AppError::invalid(
                    "The background engine supervisor did not confirm ownership in time.",
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (runner, state);
        Ok(())
    }
}

/// Fixed internal CLI worker. No caller-supplied paths, scripts or process IDs.
pub fn run() -> AppResult<()> {
    #[cfg(windows)]
    {
        use std::time::Duration;
        let state = storage::managed_engine_state_root();
        let runner = crate::runtime::SystemProcessRunner;
        let original = verify_selection(&runner, &state)?;
        let lock = lock_file(&state, WORKER_LOCK)?;
        if !try_claim(&lock)? {
            return Ok(());
        }
        let result = (|| {
            loop {
                if state.join(STOP_FILE).exists() {
                    return Ok(());
                }
                // Journal changes do not become a different engine silently.
                if bootstrap::load(&state)?.as_ref() != Some(&original) {
                    return Err(AppError::invalid(
                        "Engine ownership changed; background supervision stopped.",
                    ));
                }
                verify_selection(&runner, &state)?;
                lease::ensure_session_owned(&runner, &state)?;
                let value = SupervisorStatus {
                    schema_version: 1,
                    running: true,
                    process_id: std::process::id(),
                    last_verified_at_ms: now_ms()?,
                };
                storage::write_file_atomically(
                    &state.join(STATUS_FILE),
                    &serde_json::to_vec(&value).map_err(AppError::internal)?,
                )?;
                // Respond to an explicit stop or removed selection promptly;
                // full distro ownership and exited-helper checks every 15s.
                for _ in 0..30 {
                    std::thread::sleep(Duration::from_millis(500));
                    if state.join(STOP_FILE).exists()
                        || crate::engine_setup::selected_at(&state)?.is_none()
                    {
                        return Ok(());
                    }
                }
            }
        })();
        let _ = lease::release_session();
        // Delete only our own status while the singleton lock is still held.
        if bootstrap::load(&state).ok().flatten().as_ref() == Some(&original) {
            let _ = fs::remove_file(state.join(STATUS_FILE));
        }
        drop(lock);
        result
    }
    #[cfg(not(windows))]
    {
        Err(AppError::invalid(
            "The owned engine supervisor requires Windows.",
        ))
    }
}

/// Request only our lease worker to exit; app containers and WSL are untouched.
/// The next explicit app/engine operation may start it again.
pub fn stop() -> AppResult<()> {
    let state = storage::managed_engine_state_root();
    verify_selection(&crate::runtime::SystemProcessRunner, &state)?;
    let spawn_lock = lock_file(&state, SPAWN_LOCK)?;
    if !try_claim(&spawn_lock)? {
        return Err(AppError::invalid(
            "The engine supervisor is starting. Retry shortly.",
        ));
    }
    if status_at(&state)?.is_some() {
        storage::write_file_atomically(&state.join(STOP_FILE), b"stop\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "local-store-supervisor-{}-{}-{}",
            std::process::id(),
            now_ms().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn status_requires_recent_verification_and_rejects_future_or_invalid_records() {
        let mut value = SupervisorStatus {
            schema_version: 1,
            running: true,
            process_id: 7,
            last_verified_at_ms: 1000,
        };
        assert!(fresh(&value, 1001));
        assert!(!fresh(&value, 999));
        assert!(!fresh(&value, FRESH_MS + 1001));
        value.schema_version = 2;
        assert!(!fresh(&value, 1001));
        value.schema_version = 1;
        value.running = false;
        assert!(!fresh(&value, 1001));
    }
    #[test]
    fn singleton_lock_and_live_status_required_not_a_reused_pid() {
        let state = temp();
        let lock = lock_file(&state, WORKER_LOCK).unwrap();
        assert!(try_claim(&lock).unwrap());
        let second = lock_file(&state, WORKER_LOCK).unwrap();
        assert!(!try_claim(&second).unwrap());
        let value = SupervisorStatus {
            schema_version: 1,
            running: true,
            process_id: 7,
            last_verified_at_ms: now_ms().unwrap(),
        };
        storage::write_file_atomically(
            &state.join(STATUS_FILE),
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        assert!(status_at(&state).unwrap().is_some());
        FileExt::unlock(&lock).unwrap();
        assert!(status_at(&state).unwrap().is_none());
        drop(second);
        drop(lock);
        fs::remove_dir_all(state).unwrap();
    }
    #[test]
    fn oversized_status_is_refused_and_missing_state_is_never_created() {
        let state = temp();
        fs::write(
            state.join(STATUS_FILE),
            vec![b'x'; STATUS_MAX_BYTES as usize + 1],
        )
        .unwrap();
        assert!(read_status(&state).is_err());
        fs::remove_dir_all(&state).unwrap();
        assert!(status_at(&state).unwrap().is_none());
        assert!(!state.exists());
    }
}

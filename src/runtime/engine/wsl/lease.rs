//! Exact owned WSL pipe lease, shared by launcher and background supervisor.
//! Windows closes stdin when its owner exits; no orphaned infinite-sleep helper.
use super::bootstrap::{self, BootstrapStatus, RecoveryDisposition};
use crate::{
    error::{AppError, AppResult},
    runtime::ProcessRunner,
};
use std::path::Path;

#[cfg(any(windows, test))]
trait LeaseProcess {
    fn exited(&mut self) -> AppResult<bool>;
}

#[cfg(any(windows, test))]
fn ensure_with<P: LeaseProcess>(
    slot: &mut Option<P>,
    verify: impl FnOnce() -> AppResult<()>,
    spawn: impl FnOnce() -> AppResult<P>,
) -> AppResult<()> {
    if let Err(error) = verify() {
        // End only the helper this process created. A revoked ownership proof
        // must not leave a stale local lease holding that session open.
        *slot = None;
        return Err(error);
    }
    if let Some(process) = slot.as_mut() {
        if !process.exited()? {
            return Ok(());
        }
    }
    let mut process = spawn()?;
    if process.exited()? {
        return Err(AppError::invalid(
            "The owned engine session exited before it could be held open.",
        ));
    }
    *slot = Some(process);
    Ok(())
}

#[cfg(windows)]
struct NativeLease {
    process: std::process::Child,
    _input: std::process::ChildStdin,
}
#[cfg(windows)]
impl LeaseProcess for NativeLease {
    fn exited(&mut self) -> AppResult<bool> {
        Ok(self.process.try_wait()?.is_some())
    }
}
#[cfg(windows)]
impl Drop for NativeLease {
    fn drop(&mut self) {
        // Only this exact pipe helper. Never terminate the distribution or apps.
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Ownership is freshly checked even when a process-local lease already exists.
pub fn ensure_owned(runner: &dyn ProcessRunner, state: &Path) -> AppResult<()> {
    ensure_session_owned(runner, state)?;
    super::supervisor::ensure_background(runner, state)
}

/// The supervisor uses this boundary directly to avoid recursively spawning.
pub(crate) fn ensure_session_owned(runner: &dyn ProcessRunner, state: &Path) -> AppResult<()> {
    let verify = || {
        if !matches!(
            bootstrap::inspect(runner, state)?,
            BootstrapStatus::Recovery {
                disposition: RecoveryDisposition::Ready,
                ..
            }
        ) {
            return Err(AppError::invalid(
                "Engine session requires verified Local Store ownership.",
            ));
        }
        Ok(())
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        let mut slot = session_slot()
            .lock()
            .map_err(|_| AppError::internal("Engine session lock unavailable."))?;
        ensure_with(&mut slot, verify, || {
            let mut process = Command::new("wsl.exe")
                .args([
                    "--distribution",
                    super::DISTRO,
                    "--user",
                    "root",
                    "--exec",
                    "/usr/bin/env",
                    "-i",
                    "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
                    "/usr/bin/cat",
                ])
                .env_remove("WSLENV")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                // CREATE_NO_WINDOW; same Windows flag as the existing runner.
                .creation_flags(0x0800_0000)
                .spawn()?;
            let input = process
                .stdin
                .take()
                .ok_or_else(|| AppError::internal("Engine session pipe unavailable."))?;
            Ok(NativeLease {
                process,
                _input: input,
            })
        })
    }
    #[cfg(not(windows))]
    {
        verify()?;
        Err(AppError::invalid("The owned WSL engine requires Windows."))
    }
}

/// Release only this process's pipe helper. Never stop WSL or app containers.
#[cfg(windows)]
pub(crate) fn release_session() -> AppResult<()> {
    let mut slot = session_slot()
        .lock()
        .map_err(|_| AppError::internal("Engine session lock unavailable."))?;
    *slot = None;
    Ok(())
}

#[cfg(windows)]
fn session_slot() -> &'static std::sync::Mutex<Option<NativeLease>> {
    use std::sync::{Mutex, OnceLock};
    static LEASE: OnceLock<Mutex<Option<NativeLease>>> = OnceLock::new();
    LEASE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(bool);
    impl LeaseProcess for Fake {
        fn exited(&mut self) -> AppResult<bool> {
            Ok(self.0)
        }
    }
    #[test]
    fn fresh_ownership_precedes_spawn_and_live_session_reuse() {
        let mut slot = None;
        assert!(ensure_with(
            &mut slot,
            || Err(AppError::invalid("foreign")),
            || panic!("must not spawn")
        )
        .is_err());
        assert!(slot.is_none());
        ensure_with(&mut slot, || Ok(()), || Ok(Fake(false))).unwrap();
        ensure_with(&mut slot, || Ok(()), || panic!("reuse live helper")).unwrap();
        assert!(ensure_with(
            &mut slot,
            || Err(AppError::invalid("revoked ownership")),
            || panic!("must not spawn")
        )
        .is_err());
        assert!(slot.is_none());
    }
    #[test]
    fn exited_helper_requires_fresh_proof_and_successful_replacement() {
        let mut slot = Some(Fake(true));
        assert!(ensure_with(&mut slot, || Ok(()), || Ok(Fake(true))).is_err());
        ensure_with(&mut slot, || Ok(()), || Ok(Fake(false))).unwrap();
        assert!(!slot.as_mut().unwrap().exited().unwrap());
    }
}

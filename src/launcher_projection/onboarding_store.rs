//! Per-user presentation progress. This is never proof of WSL readiness,
//! engine ownership, an app installation, or permission for agent actions.
use crate::{
    error::{AppError, AppResult, ErrorCode},
    storage,
};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 4096;
const FILE: &str = "onboarding-v1.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    WelcomeViewed,
    EngineExplanationViewed,
    FirstAppExplanationViewed,
    AgentAccessExplanationViewed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnboardingProgress {
    pub version: u32,
    /// Display progress only. No value here is approval for engine setup,
    /// destructive recovery, app installation, or an agent grant.
    pub viewed: Vec<OnboardingStep>,
}

impl Default for OnboardingProgress {
    fn default() -> Self {
        Self {
            version: VERSION,
            viewed: Vec::new(),
        }
    }
}

fn corrupt() -> AppError {
    AppError::new(
        ErrorCode::StorageCorrupt,
        "Onboarding progress is invalid or from an unsupported version. No progress was changed.",
    )
}

fn validate(state: OnboardingProgress) -> AppResult<OnboardingProgress> {
    if state.version != VERSION || state.viewed.len() > 4 {
        return Err(corrupt());
    }
    for (index, step) in state.viewed.iter().enumerate() {
        if state.viewed[..index].contains(step) {
            return Err(corrupt());
        }
    }
    Ok(state)
}

/// Exclusive cross-process writer. An absent file is the only supported
/// migration from the pre-onboarding release; malformed/unknown versions are
/// refused instead of silently replacing a user's state.
pub struct OnboardingStore {
    path: PathBuf,
    state: OnboardingProgress,
    _lock: fs::File,
}

impl OnboardingStore {
    /// Read without creating state or an onboarding lock file.
    pub fn read(root: &Path) -> AppResult<OnboardingProgress> {
        let path = root.join(FILE);
        if !path.exists() {
            return Ok(OnboardingProgress::default());
        }
        if fs::metadata(&path)?.len() > MAX_BYTES {
            return Err(corrupt());
        }
        let bytes = fs::read(&path)?;
        validate(serde_json::from_slice(&bytes).map_err(|_| corrupt())?)
    }
    pub fn open(root: &Path) -> AppResult<Self> {
        fs::create_dir_all(root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("onboarding.lock"))?;
        FileExt::try_lock(&lock).map_err(|_| {
            AppError::new(
                ErrorCode::OperationBusy,
                "Onboarding progress is in use by another process.",
            )
        })?;
        let path = root.join(FILE);
        let state = Self::read(root)?;
        Ok(Self {
            path,
            state,
            _lock: lock,
        })
    }

    pub fn state(&self) -> &OnboardingProgress {
        &self.state
    }

    pub fn mark_viewed(&mut self, step: OnboardingStep) -> AppResult<&OnboardingProgress> {
        if self.state.viewed.contains(&step) {
            return Ok(&self.state);
        }
        let mut next = self.state.clone();
        next.viewed.push(step);
        let bytes = serde_json::to_vec_pretty(&next).map_err(AppError::internal)?;
        storage::write_file_atomically(&self.path, &bytes).map_err(AppError::from)?;
        self.state = next;
        Ok(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("local-store-onboarding-{name}-{nonce}"))
    }

    #[test]
    fn absent_state_starts_empty_and_viewed_steps_survive_reopen() {
        let dir = root("reopen");
        assert!(OnboardingStore::read(&dir).unwrap().viewed.is_empty());
        assert!(!dir.exists());
        {
            let mut store = OnboardingStore::open(&dir).unwrap();
            assert!(store.state().viewed.is_empty());
            store
                .mark_viewed(OnboardingStep::EngineExplanationViewed)
                .unwrap();
            store
                .mark_viewed(OnboardingStep::EngineExplanationViewed)
                .unwrap();
            assert_eq!(store.state().viewed.len(), 1);
        }
        let store = OnboardingStore::open(&dir).unwrap();
        assert_eq!(
            store.state().viewed,
            [OnboardingStep::EngineExplanationViewed]
        );
        drop(store);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_version_and_duplicate_steps_fail_without_overwriting() {
        let dir = root("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(FILE);
        for bytes in [
            br#"{"version":2,"viewed":[]}"#.as_slice(),
            br#"{"version":1,"viewed":["welcome_viewed","welcome_viewed"]}"#.as_slice(),
            br#"{"version":1,"viewed":[],"consent":true}"#.as_slice(),
        ] {
            fs::write(&file, bytes).unwrap();
            assert_eq!(
                OnboardingStore::open(&dir).err().unwrap().code,
                ErrorCode::StorageCorrupt
            );
            assert_eq!(fs::read(&file).unwrap(), bytes);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_second_writer_is_refused() {
        let dir = root("lock");
        let first = OnboardingStore::open(&dir).unwrap();
        assert_eq!(
            OnboardingStore::open(&dir).err().unwrap().code,
            ErrorCode::OperationBusy
        );
        drop(first);
        fs::remove_dir_all(dir).unwrap();
    }
}

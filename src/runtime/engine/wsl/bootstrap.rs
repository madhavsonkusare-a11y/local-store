//! Durable ownership record for a future WSL import transaction.
//!
//! This module does not call WSL. The caller must first inventory registered
//! distros, then reserve a fresh journal before importing the fixed distro.
use super::DISTRO;
use crate::{
    error::{AppError, AppResult},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const JOURNAL_FILE: &str = "bootstrap.json";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapState {
    Reserved,
    Imported,
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapJournal {
    pub schema_version: u32,
    pub distro: String,
    pub install_dir: PathBuf,
    pub rootfs_sha256: String,
    /// Random identifier generated once by the future bootstrap coordinator.
    pub ownership_token: String,
    pub state: BootstrapState,
}

impl BootstrapJournal {
    pub fn new(
        install_dir: PathBuf,
        rootfs_sha256: String,
        ownership_token: String,
    ) -> AppResult<Self> {
        let value = Self {
            schema_version: 1,
            distro: DISTRO.into(),
            install_dir,
            rootfs_sha256,
            ownership_token,
            state: BootstrapState::Reserved,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> AppResult<()> {
        let path = self.install_dir.to_string_lossy();
        let bytes = path.as_bytes();
        let local_absolute = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        let digest = self.rootfs_sha256.len() == 64
            && self
                .rootfs_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
        let token = self.ownership_token.len() >= 32
            && self.ownership_token.len() <= 128
            && self
                .ownership_token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if self.schema_version != 1
            || self.distro != DISTRO
            || !local_absolute
            || path.starts_with(r"\\")
            || path.contains("..")
            || path.chars().any(char::is_control)
            || path.len() > 4096
            || !digest
            || !token
        {
            return Err(AppError::invalid(
                "Invalid managed-engine bootstrap ownership journal.",
            ));
        }
        Ok(())
    }

    pub fn advance(&mut self, next: BootstrapState) -> AppResult<()> {
        let valid = matches!(
            (self.state, next),
            (BootstrapState::Reserved, BootstrapState::Imported)
                | (BootstrapState::Imported, BootstrapState::Verified)
        );
        if !valid {
            return Err(AppError::invalid(
                "Invalid managed-engine bootstrap state transition.",
            ));
        }
        self.state = next;
        Ok(())
    }
}

pub fn load(directory: &Path) -> AppResult<Option<BootstrapJournal>> {
    let path = directory.join(JOURNAL_FILE);
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut data = Vec::new();
    file.by_ref()
        .take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(AppError::invalid(
            "Managed-engine bootstrap journal is too large.",
        ));
    }
    let journal: BootstrapJournal = serde_json::from_slice(&data)
        .map_err(|_| AppError::invalid("Managed-engine bootstrap journal is corrupt."))?;
    journal.validate()?;
    Ok(Some(journal))
}

/// First writer owns the reservation. Existing, corrupt, or mismatched state is
/// never overwritten, which lets a later coordinator recover deliberately.
pub fn reserve(directory: &Path, journal: &BootstrapJournal) -> AppResult<()> {
    journal.validate()?;
    fs::create_dir_all(directory)?;
    if load(directory)?.is_some() {
        return Err(AppError::invalid(
            "A managed-engine bootstrap reservation already exists.",
        ));
    }
    let path = directory.join(JOURNAL_FILE);
    let bytes = serde_json::to_vec_pretty(journal).map_err(AppError::internal)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    use std::io::Write;
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            AppError::invalid("A managed-engine bootstrap reservation already exists.")
        } else {
            error.into()
        }
    })?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        // The caller sees failure and the possibly partial reservation remains;
        // load will reject it. Never erase evidence another process may own.
        return Err(error.into());
    }
    Ok(())
}

pub fn save(directory: &Path, journal: &BootstrapJournal) -> AppResult<()> {
    journal.validate()?;
    let previous = load(directory)?
        .ok_or_else(|| AppError::invalid("Managed-engine bootstrap reservation is missing."))?;
    if previous.distro != journal.distro
        || previous.install_dir != journal.install_dir
        || previous.rootfs_sha256 != journal.rootfs_sha256
        || previous.ownership_token != journal.ownership_token
    {
        return Err(AppError::invalid(
            "Refusing to replace managed-engine ownership identity.",
        ));
    }
    storage::write_file_atomically(
        &directory.join(JOURNAL_FILE),
        &serde_json::to_vec_pretty(journal).map_err(AppError::internal)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "local-store-bootstrap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    fn journal() -> BootstrapJournal {
        BootstrapJournal::new(
            PathBuf::from(r"C:\ProgramData\Local Store\engine-v1"),
            "a".repeat(64),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap()
    }

    #[test]
    fn reservation_is_exclusive_and_identity_is_immutable() {
        let root = temp();
        let mut value = journal();
        reserve(&root, &value).unwrap();
        assert_eq!(load(&root).unwrap(), Some(value.clone()));
        assert!(reserve(&root, &value).is_err());
        value.advance(BootstrapState::Imported).unwrap();
        save(&root, &value).unwrap();
        value.advance(BootstrapState::Verified).unwrap();
        save(&root, &value).unwrap();
        assert_eq!(
            load(&root).unwrap().unwrap().state,
            BootstrapState::Verified
        );
        let mut changed = value.clone();
        changed.rootfs_sha256 = "b".repeat(64);
        assert!(save(&root, &changed).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_oversized_or_ambiguous_state_is_never_replaced() {
        for path in [
            PathBuf::from(r"\\server\share"),
            PathBuf::from("relative"),
            PathBuf::from(r"C:\a\..\b"),
        ] {
            assert!(BootstrapJournal::new(
                path,
                "a".repeat(64),
                "01234567-89ab-cdef-0123-456789abcdef".into()
            )
            .is_err());
        }
        let mut value = journal();
        assert!(value.advance(BootstrapState::Verified).is_err());
        let root = temp();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(JOURNAL_FILE), "{").unwrap();
        assert!(load(&root).is_err());
        assert!(reserve(&root, &journal()).is_err());
        fs::write(
            root.join(JOURNAL_FILE),
            vec![b'x'; MAX_JOURNAL_BYTES as usize + 1],
        )
        .unwrap();
        assert!(load(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

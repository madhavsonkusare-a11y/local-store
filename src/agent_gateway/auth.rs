//! Owner-enrolled bearer credentials. MCP client metadata never establishes identity.
use super::VerifiedClient;
use crate::{
    error::{AppError, AppResult, ErrorCode},
    storage,
};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};
use subtle::ConstantTimeEq;

const VERSION: u32 = 1;
const MAX_CLIENTS: usize = 256;
const MAX_BYTES: u64 = 64 * 1024;
const DOMAIN: &[u8] = b"local-store-agent-client-v1\0";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientRecord {
    id: String,
    sha256_hex: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientFile {
    version: u32,
    clients: Vec<ClientRecord>,
}

// Owner enrollment is intentionally not wired to an agent-reachable command yet.
/// Returned once to a trusted owner flow; deliberately neither Debug nor Clone.
#[allow(dead_code)]
pub(crate) struct EnrolledSecret(String);
#[allow(dead_code)]
impl EnrolledSecret {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

/// Stores hashes only. Local-profile ACLs protect against other OS users, but
/// cannot isolate an agent with arbitrary file access under the same account.
pub(crate) struct ClientCredentialStore {
    #[allow(dead_code)]
    path: PathBuf,
    clients: Vec<ClientRecord>,
    _lock: fs::File,
}
impl ClientCredentialStore {
    pub(crate) fn open(root: &Path) -> AppResult<Self> {
        fs::create_dir_all(root).map_err(AppError::from)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("agent-clients.lock"))
            .map_err(AppError::from)?;
        FileExt::try_lock(&lock).map_err(|_| {
            AppError::new(
                ErrorCode::OperationBusy,
                "Agent credentials are in use by another process.",
            )
        })?;
        let path = root.join("agent-clients-v1.json");
        let clients = if path.exists() {
            if fs::metadata(&path).map_err(AppError::from)?.len() > MAX_BYTES {
                return Err(corrupt());
            }
            let bytes = fs::read(&path).map_err(AppError::from)?;
            let file: ClientFile = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            if file.version != VERSION || file.clients.len() > MAX_CLIENTS {
                return Err(corrupt());
            }
            let mut seen = BTreeSet::new();
            for row in &file.clients {
                if !valid_id(&row.id)
                    || !valid_hash(&row.sha256_hex)
                    || !seen.insert(row.id.clone())
                {
                    return Err(corrupt());
                }
            }
            file.clients
        } else {
            Vec::new()
        };
        Ok(Self {
            path,
            clients,
            _lock: lock,
        })
    }

    #[allow(dead_code)]
    fn save(&self, clients: &[ClientRecord]) -> AppResult<()> {
        if clients.len() > MAX_CLIENTS {
            return Err(AppError::invalid("Too many agent clients."));
        }
        let bytes = serde_json::to_vec_pretty(&ClientFile {
            version: VERSION,
            clients: clients.to_vec(),
        })
        .map_err(|_| AppError::internal("Could not encode agent credentials."))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(AppError::invalid("Agent credential store is too large."));
        }
        storage::write_file_atomically(&self.path, &bytes).map_err(AppError::from)
    }

    /// Trusted owner flow only. Never offer enrollment as an MCP tool.
    #[allow(dead_code)]
    pub(crate) fn enroll(&mut self, id: &str) -> AppResult<EnrolledSecret> {
        if !valid_id(id) {
            return Err(AppError::invalid("Invalid agent client ID."));
        }
        if self.clients.iter().any(|row| row.id == id) {
            return Err(AppError::new(
                ErrorCode::AlreadyExists,
                "Agent client is already enrolled.",
            ));
        }
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| AppError::internal("Could not create agent credential."))?;
        let secret = hex(&random);
        let mut next = self.clients.clone();
        next.push(ClientRecord {
            id: id.to_owned(),
            sha256_hex: hash_secret(&secret),
        });
        self.save(&next)?;
        self.clients = next;
        Ok(EnrolledSecret(secret))
    }

    /// Trusted owner flow only. Future requests must verify again.
    #[allow(dead_code)]
    pub(crate) fn revoke(&mut self, id: &str) -> AppResult<()> {
        if !valid_id(id) {
            return Err(AppError::invalid("Invalid agent client ID."));
        }
        let next = self
            .clients
            .iter()
            .filter(|row| row.id != id)
            .cloned()
            .collect::<Vec<_>>();
        let saved = self.save(&next);
        self.clients = next;
        saved
    }

    /// Metadata can narrow a verified identity, never choose it.
    pub(super) fn verify(&self, secret: &str, claim: Option<&str>) -> AppResult<VerifiedClient> {
        if !valid_hash(secret) {
            return Err(forbidden());
        }
        let hash = hash_secret(secret);
        let mut found = None;
        for row in &self.clients {
            if hash.as_bytes().ct_eq(row.sha256_hex.as_bytes()).unwrap_u8() == 1 {
                found = Some(row.id.as_str());
            }
        }
        let id = found.ok_or_else(forbidden)?;
        if claim.is_some_and(|value| value != id) {
            return Err(forbidden());
        }
        Ok(VerifiedClient {
            client_id: id.to_owned(),
        })
    }
}
fn hash_secret(secret: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(secret.as_bytes());
    hex(&digest.finalize())
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
fn corrupt() -> AppError {
    AppError::new(
        ErrorCode::StorageCorrupt,
        "Agent credential store is invalid or unsupported.",
    )
}
fn forbidden() -> AppError {
    AppError::new(ErrorCode::Forbidden, "Agent client authentication failed.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "local-store-agent-credentials-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
    #[test]
    fn enrollment_hashes_secret_and_rejects_spoofed_metadata() {
        let root = scratch();
        let mut store = ClientCredentialStore::open(&root).unwrap();
        let token = store.enroll("client").unwrap().into_string();
        let disk = fs::read_to_string(root.join("agent-clients-v1.json")).unwrap();
        assert!(!disk.contains(&token));
        assert!(store.verify(&token, Some("client")).is_ok());
        assert_eq!(
            store.verify(&token, Some("other")).err().unwrap().code,
            ErrorCode::Forbidden
        );
        assert_eq!(
            store
                .verify(&"f".repeat(64), Some("client"))
                .err()
                .unwrap()
                .code,
            ErrorCode::Forbidden
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn revocation_survives_reopen() {
        let root = scratch();
        let token = {
            let mut store = ClientCredentialStore::open(&root).unwrap();
            store.enroll("client").unwrap().into_string()
        };
        let mut store = ClientCredentialStore::open(&root).unwrap();
        store.revoke("client").unwrap();
        drop(store);
        let store = ClientCredentialStore::open(&root).unwrap();
        assert_eq!(
            store.verify(&token, Some("client")).err().unwrap().code,
            ErrorCode::Forbidden
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_and_future_versions_fail_closed() {
        let root = scratch();
        let path = root.join("agent-clients-v1.json");
        for bytes in [
            b"{".as_slice(),
            br#"{"version":99,"clients":[]}"#,
            br#"{"version":1,"clients":[],"extra":true}"#,
            br#"{"version":1,"clients":[{"id":"x","sha256_hex":"bad"}]}"#,
        ] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(
                ClientCredentialStore::open(&root).err().unwrap().code,
                ErrorCode::StorageCorrupt
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}

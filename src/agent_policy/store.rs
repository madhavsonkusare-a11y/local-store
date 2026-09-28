//! Durable owner-side grants and bounded, redacted authorization metadata.
//! Approval tickets deliberately remain process-local and disappear on restart.

use super::{valid_id, AgentAction, AgentPolicy, AuditEvent, Grant};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::storage;
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;
const MAX_GRANTS: usize = 4096;
const MAX_AUDIT: usize = 2048;
const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    version: u32,
    grants: Vec<Grant>,
    audit: Vec<AuditEvent>,
}

/// Holds an exclusive sidecar lock for the lifetime of one policy instance.
/// This is an internal owner-side API, not an authenticated agent transport.
pub struct AgentPolicyStore {
    path: PathBuf,
    policy: AgentPolicy,
    _lock: fs::File,
}

impl AgentPolicyStore {
    pub fn open(root: &Path) -> AppResult<Self> {
        fs::create_dir_all(root).map_err(AppError::from)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("agent-policy.lock"))
            .map_err(AppError::from)?;
        FileExt::try_lock(&lock).map_err(|_| {
            AppError::new(
                ErrorCode::OperationBusy,
                "Agent policy is in use by another process.",
            )
        })?;
        let path = root.join("agent-policy-v1.json");
        let policy = if path.exists() {
            if fs::metadata(&path).map_err(AppError::from)?.len() > MAX_BYTES {
                return Err(corrupt());
            }
            let bytes = fs::read(&path).map_err(AppError::from)?;
            let file: PolicyFile = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            Self::validate(file)?
        } else {
            AgentPolicy::default()
        };
        Ok(Self {
            path,
            policy,
            _lock: lock,
        })
    }

    fn validate(file: PolicyFile) -> AppResult<AgentPolicy> {
        if file.version != VERSION || file.grants.len() > MAX_GRANTS || file.audit.len() > MAX_AUDIT
        {
            return Err(corrupt());
        }
        let mut policy = AgentPolicy::default();
        for grant in file.grants {
            let key = (grant.client_id.clone(), grant.app_id.clone());
            if policy.grants.contains_key(&key) || policy.grant(grant).is_err() {
                return Err(corrupt());
            }
        }
        if file
            .audit
            .iter()
            .any(|event| !valid_id(&event.client_id) || !valid_id(&event.app_id))
        {
            return Err(corrupt());
        }
        policy.audit = file.audit;
        Ok(policy)
    }

    fn save(&self, next: &AgentPolicy) -> AppResult<()> {
        if next.grants.len() > MAX_GRANTS {
            return Err(AppError::invalid("Too many agent grants."));
        }
        let file = PolicyFile {
            version: VERSION,
            grants: next.grants.values().cloned().collect(),
            audit: next.audit.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|_| AppError::internal("Could not encode agent policy."))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(AppError::invalid("Agent policy is too large."));
        }
        storage::write_file_atomically(&self.path, &bytes).map_err(AppError::from)
    }

    pub fn grant(&mut self, grant: Grant) -> AppResult<()> {
        let mut next = self.policy.clone();
        next.grant(grant)?;
        self.save(&next)?;
        self.policy = next;
        Ok(())
    }

    pub fn revoke(&mut self, client_id: &str, app_id: &str) -> AppResult<()> {
        if !valid_id(client_id) || !valid_id(app_id) {
            return Err(AppError::invalid("Invalid agent grant scope."));
        }
        let mut next = self.policy.clone();
        next.revoke(client_id, app_id);
        let saved = self.save(&next);
        // A failed disk write must not leave access usable in this process.
        self.policy = next;
        saved
    }

    pub fn revoke_client(&mut self, client_id: &str) -> AppResult<()> {
        if !valid_id(client_id) {
            return Err(AppError::invalid("Invalid agent client ID."));
        }
        let mut next = self.policy.clone();
        next.revoke_client(client_id);
        let saved = self.save(&next);
        self.policy = next;
        saved
    }

    /// One-use tickets are never serialized. A restart requires fresh approval.
    pub fn approve(
        &mut self,
        client_id: &str,
        app_id: &str,
        action: AgentAction,
        expires_at_unix: u64,
    ) -> AppResult<String> {
        self.policy
            .approve(client_id, app_id, action, expires_at_unix)
    }

    pub fn authorize(
        &mut self,
        client_id: &str,
        app_id: &str,
        action: AgentAction,
        ticket: Option<&str>,
        now_unix: u64,
    ) -> AppResult<()> {
        let outcome = self
            .policy
            .authorize(client_id, app_id, action, ticket, now_unix);
        // Never return a permit to dispatch work before its audit is saved.
        self.save(&self.policy)?;
        outcome
    }

    pub fn status_app_ids(
        &self,
        client_id: &str,
        now_unix: u64,
    ) -> std::collections::BTreeSet<String> {
        self.policy.status_app_ids(client_id, now_unix)
    }

    pub fn audit(&self) -> &[AuditEvent] {
        self.policy.audit()
    }
}

fn corrupt() -> AppError {
    AppError::new(
        ErrorCode::StorageCorrupt,
        "Agent policy is invalid or unsupported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "local-store-agent-policy-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn grant(action: AgentAction) -> Grant {
        Grant {
            client_id: "client".into(),
            app_id: "memos".into(),
            actions: BTreeSet::from([action]),
            expires_at_unix: 200,
        }
    }

    #[test]
    fn grants_round_trip_but_approval_tickets_do_not() {
        let root = scratch();
        let ticket = {
            let mut store = AgentPolicyStore::open(&root).unwrap();
            store.grant(grant(AgentAction::WriteContent)).unwrap();
            store
                .approve("client", "memos", AgentAction::WriteContent, 150)
                .unwrap()
        };
        let mut store = AgentPolicyStore::open(&root).unwrap();
        assert!(store
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&ticket),
                100
            )
            .is_err());
        let fresh = store
            .approve("client", "memos", AgentAction::WriteContent, 150)
            .unwrap();
        assert!(store
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&fresh),
                100
            )
            .is_ok());
        assert!(store
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&fresh),
                100
            )
            .is_err());
        drop(store);
        let store = AgentPolicyStore::open(&root).unwrap();
        assert_eq!(store.audit().len(), 3);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn revocation_survives_reopen() {
        let root = scratch();
        {
            let mut store = AgentPolicyStore::open(&root).unwrap();
            store.grant(grant(AgentAction::Status)).unwrap();
            assert!(store
                .authorize("client", "memos", AgentAction::Status, None, 100)
                .is_ok());
            store.revoke("client", "memos").unwrap();
        }
        let mut store = AgentPolicyStore::open(&root).unwrap();
        assert_eq!(
            store
                .authorize("client", "memos", AgentAction::Status, None, 100)
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_and_future_versions_refuse_to_open() {
        let root = scratch();
        let path = root.join("agent-policy-v1.json");
        for bytes in [
            b"{".as_slice(),
            br#"{"version":99,"grants":[],"audit":[]}"#,
            br#"{"version":1,"grants":[],"audit":[],"unexpected":true}"#,
        ] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(
                AgentPolicyStore::open(&root).err().unwrap().code,
                ErrorCode::StorageCorrupt
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn audit_redacts_invalid_identity_and_stays_bounded() {
        let root = scratch();
        {
            let mut store = AgentPolicyStore::open(&root).unwrap();
            for n in 0..(MAX_AUDIT + 3) {
                let _ = store.authorize(
                    "secret:credential",
                    "memos",
                    AgentAction::Status,
                    None,
                    n as u64,
                );
            }
            assert_eq!(store.audit().len(), MAX_AUDIT);
        }
        let bytes = fs::read_to_string(root.join("agent-policy-v1.json")).unwrap();
        assert!(!bytes.contains("secret"));
        let store = AgentPolicyStore::open(&root).unwrap();
        assert_eq!(store.audit().len(), MAX_AUDIT);
        assert!(store
            .audit()
            .iter()
            .all(|event| event.client_id == "invalid"));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

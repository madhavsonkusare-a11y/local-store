//! Read-only agent tool boundary for a future authenticated transport.
//! Client identity is resolved from an owner-enrolled bearer secret on every
//! request. MCP client metadata is only an optional consistency check.

mod auth;

use crate::{
    agent_access::ToolResult,
    agent_policy::{AgentAction, AgentPolicyStore},
    brand::CONFIG_SLUG,
    error::{AppError, AppResult, ErrorCode},
    model::InstalledApp,
    runtime::{self, AppStatus},
    storage::{self, RegistryV2},
};
use auth::ClientCredentialStore;
use std::path::{Path, PathBuf};

/// Only the credential verifier in this module can mint this identity.
struct VerifiedClient {
    client_id: String,
}

/// No grant, approval, install, content, or destructive methods are exposed.
/// Root paths are trusted owner configuration, never MCP request arguments.
pub struct AgentGateway {
    policy_root: PathBuf,
    credentials_root: PathBuf,
}

impl AgentGateway {
    /// Resolve private machine-local state from the owner profile.
    pub fn open_local() -> AppResult<Self> {
        let base = std::env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::PrerequisiteUnavailable,
                    "Local app data is unavailable.",
                )
            })?;
        let base = PathBuf::from(base).join(CONFIG_SLUG);
        Self::open(&base.join("agent-policy"), &base.join("agent-auth"))
    }

    pub fn open(policy_root: &Path, credentials_root: &Path) -> AppResult<Self> {
        // Validate both stores now; reopen for each request so owner revocation
        // is visible without restarting a long-lived future gateway.
        drop(AgentPolicyStore::open(policy_root)?);
        drop(ClientCredentialStore::open(credentials_root)?);
        Ok(Self {
            policy_root: policy_root.to_owned(),
            credentials_root: credentials_root.to_owned(),
        })
    }

    pub fn get_status(
        &self,
        presented_secret: &str,
        claimed_client_id: Option<&str>,
        app_id: &str,
        now_unix: u64,
    ) -> AppResult<ToolResult> {
        self.get_status_with(
            presented_secret,
            claimed_client_id,
            app_id,
            now_unix,
            || storage::load_or_migrate_registry().map_err(AppError::from),
            runtime::status,
        )
    }

    fn get_status_with(
        &self,
        presented_secret: &str,
        claimed_client_id: Option<&str>,
        app_id: &str,
        now_unix: u64,
        load: impl FnOnce() -> AppResult<RegistryV2>,
        status: impl FnOnce(&InstalledApp) -> AppResult<AppStatus>,
    ) -> AppResult<ToolResult> {
        // Credential and policy locks are never held simultaneously. A request
        // already verified at the instant of revocation may finish; every
        // subsequent request rechecks the current credential store.
        let caller = {
            let credentials = ClientCredentialStore::open(&self.credentials_root)?;
            credentials.verify(presented_secret, claimed_client_id)?
        };
        {
            let mut policy = AgentPolicyStore::open(&self.policy_root)?;
            // The policy persists its audit before permitting a registry read.
            policy.authorize(
                &caller.client_id,
                app_id,
                AgentAction::Status,
                None,
                now_unix,
            )?;
        }
        let registry = load()?;
        let app = registry
            .apps
            .iter()
            .find(|app| app.id == app_id)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "That app is not installed."))?;
        Ok(ToolResult::Status {
            status: status(app)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{agent_policy::Grant, model::RuntimeSpec};
    use std::{
        collections::BTreeSet,
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "local-store-agent-gateway-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn roots(root: &Path) -> (PathBuf, PathBuf) {
        (root.join("policy"), root.join("credentials"))
    }
    fn enroll(root: &Path, id: &str) -> String {
        let (_, credentials_root) = roots(root);
        ClientCredentialStore::open(&credentials_root)
            .unwrap()
            .enroll(id)
            .unwrap()
            .into_string()
    }
    fn grant(root: &Path, id: &str, app_id: &str) {
        let (policy_root, _) = roots(root);
        AgentPolicyStore::open(&policy_root)
            .unwrap()
            .grant(Grant {
                client_id: id.into(),
                app_id: app_id.into(),
                actions: BTreeSet::from([AgentAction::Status]),
                expires_at_unix: 200,
            })
            .unwrap();
    }
    fn installed(id: &str) -> InstalledApp {
        InstalledApp {
            id: id.into(),
            catalog_id: None,
            display_name: id.into(),
            launch_url: "http://127.0.0.1:5000".into(),
            icon_path: None,
            runtime: RuntimeSpec::External,
            created_at_unix: 0,
            updated_at_unix: 0,
        }
    }

    #[test]
    fn absent_secret_or_spoofed_client_claim_denies_before_dispatch() {
        let root = scratch();
        let secret = enroll(&root, "client");
        grant(&root, "client", "memos");
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let wrong = "f".repeat(64);
        for (token, claim) in [
            ("", Some("client")),
            (secret.as_str(), Some("other")),
            (wrong.as_str(), Some("client")),
        ] {
            let denied = gateway.get_status_with(
                token,
                claim,
                "memos",
                100,
                || panic!("unverified request read registry"),
                |_| panic!("unverified request dispatched"),
            );
            assert_eq!(denied.unwrap_err().code, ErrorCode::Forbidden);
        }
        let store = AgentPolicyStore::open(&policy_root).unwrap();
        assert!(store.audit().is_empty());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_scope_denies_before_registry_and_persists_audit() {
        let root = scratch();
        let secret = enroll(&root, "client");
        grant(&root, "client", "other");
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let denied = gateway.get_status_with(
            &secret,
            Some("client"),
            "memos",
            100,
            || panic!("unauthorized registry read"),
            |_| panic!("unauthorized dispatch"),
        );
        assert_eq!(denied.unwrap_err().code, ErrorCode::Forbidden);
        let store = AgentPolicyStore::open(&policy_root).unwrap();
        assert_eq!(store.audit().len(), 1);
        assert!(!store.audit()[0].allowed);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exact_grant_reads_installed_app_then_credential_revoke_blocks_next_call() {
        let root = scratch();
        let secret = enroll(&root, "client");
        grant(&root, "client", "memos");
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let result = gateway
            .get_status_with(
                &secret,
                Some("client"),
                "memos",
                100,
                || Ok(RegistryV2::new(vec![installed("memos")])),
                |app| {
                    assert_eq!(app.id, "memos");
                    Ok(AppStatus::Running)
                },
            )
            .unwrap();
        assert_eq!(
            result,
            ToolResult::Status {
                status: AppStatus::Running
            }
        );
        ClientCredentialStore::open(&credentials_root)
            .unwrap()
            .revoke("client")
            .unwrap();
        let denied = gateway.get_status_with(
            &secret,
            Some("client"),
            "memos",
            100,
            || panic!("revoked credential read registry"),
            |_| panic!("revoked credential dispatched"),
        );
        assert_eq!(denied.unwrap_err().code, ErrorCode::Forbidden);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        assert_eq!(
            gateway
                .get_status_with(
                    &secret,
                    Some("client"),
                    "memos",
                    100,
                    || panic!("revoked after reopen"),
                    |_| panic!("revoked after reopen")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        let store = AgentPolicyStore::open(&policy_root).unwrap();
        assert_eq!(store.audit().len(), 1);
        assert!(store.audit()[0].allowed);
        assert!(!serde_json::to_string(store.audit())
            .unwrap()
            .contains(&secret));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn revoked_grant_is_visible_without_restarting_gateway() {
        let root = scratch();
        let secret = enroll(&root, "client");
        grant(&root, "client", "memos");
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        AgentPolicyStore::open(&policy_root)
            .unwrap()
            .revoke("client", "memos")
            .unwrap();
        let denied = gateway.get_status_with(
            &secret,
            Some("client"),
            "memos",
            100,
            || panic!("revoked grant read registry"),
            |_| panic!("revoked grant dispatched"),
        );
        assert_eq!(denied.unwrap_err().code, ErrorCode::Forbidden);
        fs::remove_dir_all(root).unwrap();
    }
}

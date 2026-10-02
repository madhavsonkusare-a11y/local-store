//! Read-only agent tool boundary for a future authenticated transport.
//! Client identity is resolved from an owner-enrolled bearer secret on every
//! request. MCP client metadata is only an optional consistency check.

mod auth;

use crate::{
    agent_access::ToolResult,
    agent_policy::{AgentAction, AgentPolicyStore, Grant},
    brand::CONFIG_SLUG,
    error::{AppError, AppResult, ErrorCode},
    model::InstalledApp,
    runtime::{self, AppStatus},
    storage::{self, RegistryV2},
};
use auth::ClientCredentialStore;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const MAX_STATUS_GRANT_HOURS: u64 = 24 * 30;
const MAX_LIFECYCLE_GRANT_HOURS: u64 = 24;

#[derive(Serialize)]
pub struct OwnerPolicySnapshot {
    pub clients: Vec<String>,
    pub grants: Vec<Grant>,
    pub audit: Vec<crate::agent_policy::AuditEvent>,
}

/// Only the credential verifier in this module can mint this identity.
struct VerifiedClient {
    client_id: String,
}

struct LifecycleCall<'a> {
    app_id: &'a str,
    start: bool,
    now_unix: u64,
}

/// No grant, approval, install, content, or destructive methods are exposed.
/// Root paths are trusted owner configuration, never MCP request arguments.
#[derive(Clone)]
pub struct AgentGateway {
    policy_root: PathBuf,
    credentials_root: PathBuf,
}

impl AgentGateway {
    pub(crate) fn grant_files_for_owner(
        &self,
        client_id: &str,
        app_id: &str,
        hours: u64,
        now: u64,
    ) -> AppResult<u64> {
        if !(1..=24).contains(&hours) {
            return Err(AppError::invalid("File grants last 1 to 24 hours."));
        }
        let expires = now
            .checked_add(hours * 3600)
            .ok_or_else(|| AppError::invalid("Grant expiry is invalid."))?;
        if !ClientCredentialStore::open(&self.credentials_root)?.contains(client_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Agent client is not enrolled.",
            ));
        }
        AgentPolicyStore::open(&self.policy_root)?.grant(Grant {
            client_id: client_id.into(),
            app_id: app_id.into(),
            actions: BTreeSet::from([AgentAction::Status, AgentAction::ReadFiles]),
            expires_at_unix: expires,
        })?;
        Ok(expires)
    }
    pub(crate) fn authorize_files(&self, bearer: &str, app_id: &str, now: u64) -> AppResult<()> {
        let (client, _) = self.authenticate_mutation(bearer)?;
        AgentPolicyStore::open(&self.policy_root)?.authorize(
            &client,
            app_id,
            AgentAction::ReadFiles,
            None,
            now,
        )
    }
    /// Launcher-only content grant. Scoped app validation and protected token
    /// connection are checked by AgentContent before this owner method.
    pub(crate) fn grant_content_for_owner(
        &self,
        client_id: &str,
        app_id: &str,
        hours: u64,
        write: bool,
        now: u64,
    ) -> AppResult<u64> {
        if !(1..=24).contains(&hours) {
            return Err(AppError::invalid("Content grants last 1 to 24 hours."));
        }
        let expires = now
            .checked_add(hours * 3600)
            .ok_or_else(|| AppError::invalid("Grant expiry is invalid."))?;
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        if !credentials.contains(client_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Agent client is not enrolled.",
            ));
        }
        let mut actions = BTreeSet::from([AgentAction::Status, AgentAction::ReadContent]);
        if write {
            actions.insert(AgentAction::WriteContent);
        }
        AgentPolicyStore::open(&self.policy_root)?.grant(Grant {
            client_id: client_id.into(),
            app_id: app_id.into(),
            actions,
            expires_at_unix: expires,
        })?;
        Ok(expires)
    }

    pub(crate) fn check_content_write_request(
        &self,
        bearer: &str,
        app_id: &str,
        now: u64,
    ) -> AppResult<()> {
        let (client, _) = self.authenticate_mutation(bearer)?;
        let mut policy = AgentPolicyStore::open(&self.policy_root)?;
        let permitted = policy.grants_for_owner().iter().any(|g| {
            g.client_id == client
                && g.app_id == app_id
                && now < g.expires_at_unix
                && g.actions.contains(&AgentAction::WriteContent)
        });
        policy.record_request_audit(&client, app_id, AgentAction::WriteContent, false, now)?;
        if permitted {
            Ok(())
        } else {
            Err(AppError::new(
                ErrorCode::Forbidden,
                "App content access denied.",
            ))
        }
    }

    /// The write branch is crate-private and only used after AgentContent has
    /// atomically consumed an exact, owner-approved argument-hash proposal.
    pub(crate) fn authorize_content(
        &self,
        bearer: &str,
        app_id: &str,
        write: bool,
        now: u64,
    ) -> AppResult<()> {
        let (client, _) = self.authenticate_mutation(bearer)?;
        let mut policy = AgentPolicyStore::open(&self.policy_root)?;
        if write {
            let approval = policy.approve(&client, app_id, AgentAction::WriteContent, now + 1)?;
            policy.authorize(
                &client,
                app_id,
                AgentAction::WriteContent,
                Some(&approval),
                now,
            )
        } else {
            policy.authorize(&client, app_id, AgentAction::ReadContent, None, now)
        }
    }
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

    /// Trusted local owner CLI only. The secret is returned once and never stored
    /// as plaintext. The caller must treat stdout as sensitive.
    pub fn enroll_client_for_owner(&self, id: &str) -> AppResult<String> {
        let mut credentials = ClientCredentialStore::open(&self.credentials_root)?;
        if credentials.contains(id) {
            return Err(AppError::new(
                ErrorCode::AlreadyExists,
                "Agent client is already enrolled.",
            ));
        }
        // A previously revoked ID must not inherit any surviving grants.
        AgentPolicyStore::open(&self.policy_root)?.revoke_client(id)?;
        credentials
            .enroll(id)
            .map(auth::EnrolledSecret::into_string)
    }

    /// Remove grants first, so a credential-store write failure cannot leave
    /// the old credential authorized and a later enrollment cannot revive access.
    pub fn revoke_client_for_owner(&self, id: &str) -> AppResult<()> {
        let mut credentials = ClientCredentialStore::open(&self.credentials_root)?;
        AgentPolicyStore::open(&self.policy_root)?.revoke_client(id)?;
        credentials.revoke(id)
    }

    pub fn list_clients_for_owner(&self) -> AppResult<Vec<String>> {
        Ok(ClientCredentialStore::open(&self.credentials_root)?.client_ids())
    }

    /// Owner inspection is never exposed as an MCP discovery tool. Hold the
    /// credential lock before the policy lock, matching grant/revoke ordering.
    pub fn snapshot_for_owner(&self) -> AppResult<OwnerPolicySnapshot> {
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        let policy = AgentPolicyStore::open(&self.policy_root)?;
        Ok(OwnerPolicySnapshot {
            clients: credentials.client_ids(),
            grants: policy.grants_for_owner(),
            audit: policy.audit().to_vec(),
        })
    }

    pub fn credential_matches_for_owner(&self, id: &str, secret: &str) -> AppResult<bool> {
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        Ok(credentials.verify(secret, Some(id)).is_ok())
    }

    pub(crate) fn authenticate_mutation(&self, secret: &str) -> AppResult<(String, String)> {
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        let client = credentials.verify(secret, None)?;
        let generation = credentials
            .generation(&client.client_id)
            .ok_or_else(|| AppError::new(ErrorCode::Forbidden, "Agent access denied."))?;
        Ok((client.client_id, generation))
    }

    pub(crate) fn owner_client_generation(&self, id: &str) -> AppResult<String> {
        ClientCredentialStore::open(&self.credentials_root)?
            .generation(id)
            .ok_or_else(|| AppError::new(ErrorCode::Forbidden, "Agent access denied."))
    }

    pub(crate) fn audit_mutation(
        &self,
        client_id: &str,
        app_id: &str,
        action: AgentAction,
        allowed: bool,
        now: u64,
    ) -> AppResult<()> {
        AgentPolicyStore::open(&self.policy_root)?
            .record_request_audit(client_id, app_id, action, allowed, now)
    }

    pub(crate) fn authorize_status_request(
        &self,
        secret: &str,
        app_id: &str,
        now: u64,
    ) -> AppResult<()> {
        let (client, _) = self.authenticate_mutation(secret)?;
        AgentPolicyStore::open(&self.policy_root)?.authorize(
            &client,
            app_id,
            AgentAction::Status,
            None,
            now,
        )
    }

    /// Trusted local owner CLI only. Grants exactly status for an enrolled
    /// client and an exact installed app ID, for at most thirty days.
    pub fn grant_status_for_owner(
        &self,
        client_id: &str,
        app_id: &str,
        hours: u64,
        now_unix: u64,
    ) -> AppResult<u64> {
        self.grant_status_for_owner_with(client_id, app_id, hours, now_unix, || {
            storage::load_or_migrate_registry().map_err(AppError::from)
        })
    }

    /// Explicit, short-lived owner grant for reversible start/stop actions.
    /// It includes status so the app stays discoverable to this client.
    pub fn grant_lifecycle_for_owner(
        &self,
        client_id: &str,
        app_id: &str,
        hours: u64,
        now_unix: u64,
    ) -> AppResult<u64> {
        self.grant_lifecycle_for_owner_with(client_id, app_id, hours, now_unix, || {
            storage::load_or_migrate_registry().map_err(AppError::from)
        })
    }

    fn grant_lifecycle_for_owner_with(
        &self,
        client_id: &str,
        app_id: &str,
        hours: u64,
        now_unix: u64,
        load: impl FnOnce() -> AppResult<RegistryV2>,
    ) -> AppResult<u64> {
        if !(1..=MAX_LIFECYCLE_GRANT_HOURS).contains(&hours) {
            return Err(AppError::invalid(
                "Lifecycle grant must last 1 to 24 hours.",
            ));
        }
        let expires_at_unix = now_unix
            .checked_add(hours * 3600)
            .ok_or_else(|| AppError::invalid("Lifecycle grant expiry is out of range."))?;
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        if !credentials.contains(client_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Agent client is not enrolled.",
            ));
        }
        if !load()?.apps.iter().any(|app| app.id == app_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "That app is not installed.",
            ));
        }
        AgentPolicyStore::open(&self.policy_root)?.grant(Grant {
            client_id: client_id.to_owned(),
            app_id: app_id.to_owned(),
            actions: BTreeSet::from([AgentAction::Status, AgentAction::Start, AgentAction::Stop]),
            expires_at_unix,
        })?;
        Ok(expires_at_unix)
    }

    fn grant_status_for_owner_with(
        &self,
        client_id: &str,
        app_id: &str,
        hours: u64,
        now_unix: u64,
        load: impl FnOnce() -> AppResult<RegistryV2>,
    ) -> AppResult<u64> {
        if !(1..=MAX_STATUS_GRANT_HOURS).contains(&hours) {
            return Err(AppError::invalid("Status grant must last 1 to 720 hours."));
        }
        let expires_at_unix = now_unix
            .checked_add(hours * 3600)
            .ok_or_else(|| AppError::invalid("Status grant expiry is out of range."))?;
        // Keep the credential lock through the policy write: concurrent client
        // revocation cannot be followed by a stale status grant.
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        if !credentials.contains(client_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Agent client is not enrolled.",
            ));
        }
        if !load()?.apps.iter().any(|app| app.id == app_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "That app is not installed.",
            ));
        }
        AgentPolicyStore::open(&self.policy_root)?.grant(Grant {
            client_id: client_id.to_owned(),
            app_id: app_id.to_owned(),
            actions: BTreeSet::from([AgentAction::Status]),
            expires_at_unix,
        })?;
        Ok(expires_at_unix)
    }

    /// Revocation accepts a previously installed app ID so its stale grant can
    /// still be removed after uninstall. A client must remain enrolled.
    pub fn revoke_status_for_owner(&self, client_id: &str, app_id: &str) -> AppResult<()> {
        // Keep enrollment and the policy change coherent across processes.
        let credentials = ClientCredentialStore::open(&self.credentials_root)?;
        if !credentials.contains(client_id) {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Agent client is not enrolled.",
            ));
        }
        AgentPolicyStore::open(&self.policy_root)?.revoke(client_id, app_id)
    }

    /// Discover only installed apps with a live status grant for this credential.
    pub fn list_granted_apps(
        &self,
        presented_secret: &str,
        now_unix: u64,
    ) -> AppResult<Vec<String>> {
        self.list_granted_apps_with(presented_secret, now_unix, || {
            storage::load_or_migrate_registry().map_err(AppError::from)
        })
    }

    fn list_granted_apps_with(
        &self,
        presented_secret: &str,
        now_unix: u64,
        load: impl FnOnce() -> AppResult<RegistryV2>,
    ) -> AppResult<Vec<String>> {
        let caller =
            ClientCredentialStore::open(&self.credentials_root)?.verify(presented_secret, None)?;
        let granted =
            AgentPolicyStore::open(&self.policy_root)?.status_app_ids(&caller.client_id, now_unix);
        if granted.is_empty() {
            return Ok(Vec::new());
        }
        let registry = load()?;
        Ok(registry
            .apps
            .iter()
            .filter(|app| granted.contains(&app.id))
            .map(|app| app.id.clone())
            .collect())
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

    pub fn lifecycle(
        &self,
        presented_secret: &str,
        app_id: &str,
        start: bool,
        now_unix: u64,
    ) -> AppResult<ToolResult> {
        self.lifecycle_with(
            presented_secret,
            LifecycleCall {
                app_id,
                start,
                now_unix,
            },
            runtime::lock_operation,
            || storage::load_or_migrate_registry().map_err(AppError::from),
            |app, start| {
                if start {
                    runtime::start_with(&runtime::SystemProcessRunner, app)
                } else {
                    runtime::stop_with(&runtime::SystemProcessRunner, app)
                }
            },
        )
    }

    fn lifecycle_with<G>(
        &self,
        presented_secret: &str,
        call: LifecycleCall<'_>,
        lock: impl FnOnce(&str) -> AppResult<G>,
        load: impl FnOnce() -> AppResult<RegistryV2>,
        run: impl FnOnce(&InstalledApp, bool) -> AppResult<()>,
    ) -> AppResult<ToolResult> {
        let caller =
            ClientCredentialStore::open(&self.credentials_root)?.verify(presented_secret, None)?;
        AgentPolicyStore::open(&self.policy_root)?.authorize(
            &caller.client_id,
            call.app_id,
            if call.start {
                AgentAction::Start
            } else {
                AgentAction::Stop
            },
            None,
            call.now_unix,
        )?;
        // Share the owner's per-app operation slot through registry read and
        // runtime dispatch. A denied request never locks or reads the registry.
        let _lock = lock(call.app_id)?;
        let registry = load()?;
        let app = registry
            .apps
            .iter()
            .find(|app| app.id == call.app_id)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "That app is not installed."))?;
        run(app, call.start)?;
        Ok(if call.start {
            ToolResult::Started {
                app_id: call.app_id.to_owned(),
            }
        } else {
            ToolResult::Stopped {
                app_id: call.app_id.to_owned(),
            }
        })
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
    #[test]
    fn owner_reenrollment_does_not_restore_old_grants() {
        let root = scratch();
        let policy_root = root.join("policy");
        let credentials_root = root.join("credentials");
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let first = gateway.enroll_client_for_owner("client").unwrap();
        assert_eq!(gateway.list_clients_for_owner().unwrap(), vec!["client"]);
        {
            let mut policy = AgentPolicyStore::open(&policy_root).unwrap();
            policy
                .grant(Grant {
                    client_id: "client".into(),
                    app_id: "memos".into(),
                    actions: BTreeSet::from([AgentAction::Status]),
                    expires_at_unix: u64::MAX,
                })
                .unwrap();
        }
        gateway.revoke_client_for_owner("client").unwrap();
        assert!(gateway.list_clients_for_owner().unwrap().is_empty());
        let second = gateway.enroll_client_for_owner("client").unwrap();
        assert_ne!(first, second);
        let credentials = ClientCredentialStore::open(&credentials_root).unwrap();
        assert_eq!(
            credentials
                .verify(&first, Some("client"))
                .err()
                .unwrap()
                .code,
            ErrorCode::Forbidden
        );
        assert!(credentials.verify(&second, Some("client")).is_ok());
        drop(credentials);
        let mut policy = AgentPolicyStore::open(&policy_root).unwrap();
        assert_eq!(
            policy
                .authorize("client", "memos", AgentAction::Status, None, 1)
                .err()
                .unwrap()
                .code,
            ErrorCode::Forbidden
        );
        drop(policy);
        fs::remove_dir_all(root).unwrap();
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
    fn owner_status_grant_requires_enrolled_client_and_installed_app() {
        let root = scratch();
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let registry = || Ok(RegistryV2::new(vec![installed("memos")]));
        assert_eq!(
            gateway
                .grant_status_for_owner_with("missing", "memos", 1, 100, registry)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        let secret = gateway.enroll_client_for_owner("client").unwrap();
        assert_eq!(
            gateway
                .grant_status_for_owner_with("client", "missing", 1, 100, registry)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        for hours in [0, 721] {
            assert!(gateway
                .grant_status_for_owner_with("client", "memos", hours, 100, registry)
                .is_err());
        }
        assert_eq!(
            gateway
                .grant_status_for_owner_with("client", "memos", 1, 100, registry)
                .unwrap(),
            3700
        );
        assert!(gateway
            .get_status_with(&secret, Some("client"), "memos", 3699, registry, |_| Ok(
                AppStatus::Running
            ))
            .is_ok());
        assert_eq!(
            gateway
                .get_status_with(
                    &secret,
                    Some("client"),
                    "memos",
                    3700,
                    || panic!("expired grant read registry"),
                    |_| panic!("expired grant dispatched")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        gateway.revoke_status_for_owner("client", "memos").unwrap();
        assert_eq!(
            gateway
                .get_status_with(
                    &secret,
                    Some("client"),
                    "memos",
                    101,
                    || panic!("revoked grant read registry"),
                    |_| panic!("revoked grant dispatched")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        fs::remove_dir_all(root).unwrap();
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
    fn discovery_returns_only_live_granted_installed_apps() {
        let root = scratch();
        let secret = enroll(&root, "client");
        grant(&root, "client", "memos");
        grant(&root, "client", "uninstalled");
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let registry = || {
            Ok(RegistryV2::new(vec![
                installed("memos"),
                installed("private"),
            ]))
        };
        assert_eq!(
            gateway
                .list_granted_apps_with(&secret, 100, registry)
                .unwrap(),
            vec!["memos"]
        );
        assert!(gateway
            .list_granted_apps_with(&"f".repeat(64), 100, || panic!(
                "unverified discovery read registry"
            ))
            .is_err());
        AgentPolicyStore::open(&policy_root)
            .unwrap()
            .revoke("client", "memos")
            .unwrap();
        assert!(gateway
            .list_granted_apps_with(&secret, 100, registry)
            .unwrap()
            .is_empty());
        ClientCredentialStore::open(&credentials_root)
            .unwrap()
            .revoke("client")
            .unwrap();
        assert!(gateway
            .list_granted_apps_with(&secret, 100, || panic!("revoked discovery read registry"))
            .is_err());
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

    #[test]
    fn lifecycle_requires_exact_live_scope_and_audits_before_dispatch() {
        let root = scratch();
        let secret = enroll(&root, "client");
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let registry = || Ok(RegistryV2::new(vec![installed("memos")]));
        grant(&root, "client", "memos");
        assert_eq!(
            gateway
                .lifecycle_with(
                    &secret,
                    LifecycleCall {
                        app_id: "memos",
                        start: true,
                        now_unix: 100
                    },
                    |_| -> AppResult<()> { panic!("denied scope acquired lock") },
                    || panic!("status scope read registry"),
                    |_, _| panic!("status scope started app")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        AgentPolicyStore::open(&policy_root)
            .unwrap()
            .grant(Grant {
                client_id: "client".into(),
                app_id: "memos".into(),
                actions: BTreeSet::from([
                    AgentAction::Status,
                    AgentAction::Start,
                    AgentAction::Stop,
                ]),
                expires_at_unix: 200,
            })
            .unwrap();
        assert_eq!(
            gateway
                .lifecycle_with(
                    &secret,
                    LifecycleCall {
                        app_id: "memos",
                        start: true,
                        now_unix: 100
                    },
                    |_| Ok(()),
                    registry,
                    |app, start| {
                        assert_eq!(app.id, "memos");
                        assert!(start);
                        Ok(())
                    }
                )
                .unwrap(),
            ToolResult::Started {
                app_id: "memos".into()
            }
        );
        assert_eq!(
            gateway
                .lifecycle_with(
                    &secret,
                    LifecycleCall {
                        app_id: "memos",
                        start: false,
                        now_unix: 100
                    },
                    |_| Ok(()),
                    registry,
                    |_, start| {
                        assert!(!start);
                        Ok(())
                    }
                )
                .unwrap(),
            ToolResult::Stopped {
                app_id: "memos".into()
            }
        );
        assert_eq!(
            gateway
                .lifecycle_with(
                    &secret,
                    LifecycleCall {
                        app_id: "memos",
                        start: true,
                        now_unix: 200
                    },
                    |_| -> AppResult<()> { panic!("expired scope acquired lock") },
                    || panic!("expired scope read registry"),
                    |_, _| panic!("expired scope started app")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        gateway.revoke_status_for_owner("client", "memos").unwrap();
        assert_eq!(
            gateway
                .lifecycle_with(
                    &secret,
                    LifecycleCall {
                        app_id: "memos",
                        start: false,
                        now_unix: 100
                    },
                    |_| -> AppResult<()> { panic!("revoked scope acquired lock") },
                    || panic!("revoked scope read registry"),
                    |_, _| panic!("revoked scope stopped app")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        let audit = AgentPolicyStore::open(&policy_root).unwrap();
        assert_eq!(audit.audit().len(), 5);
        assert_eq!(
            audit.audit().iter().filter(|event| event.allowed).count(),
            2
        );
        drop(audit);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn owner_lifecycle_grant_is_bounded_and_requires_an_installed_app() {
        let root = scratch();
        let (policy_root, credentials_root) = roots(&root);
        let gateway = AgentGateway::open(&policy_root, &credentials_root).unwrap();
        let registry = || Ok(RegistryV2::new(vec![installed("memos")]));
        assert_eq!(
            gateway
                .grant_lifecycle_for_owner_with("missing", "memos", 1, 100, registry)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        let secret = gateway.enroll_client_for_owner("client").unwrap();
        for hours in [0, 25] {
            assert!(gateway
                .grant_lifecycle_for_owner_with("client", "memos", hours, 100, registry)
                .is_err());
        }
        assert_eq!(
            gateway
                .grant_lifecycle_for_owner_with("client", "absent", 1, 100, registry)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        assert_eq!(
            gateway
                .grant_lifecycle_for_owner_with("client", "memos", 1, 100, registry)
                .unwrap(),
            3700
        );
        assert!(gateway
            .lifecycle_with(
                &secret,
                LifecycleCall {
                    app_id: "memos",
                    start: true,
                    now_unix: 3699
                },
                |_| Ok(()),
                registry,
                |_, _| Ok(())
            )
            .is_ok());
        assert_eq!(
            gateway
                .lifecycle_with(
                    &secret,
                    LifecycleCall {
                        app_id: "memos",
                        start: true,
                        now_unix: 3700
                    },
                    |_| -> AppResult<()> { panic!("expired grant acquired lock") },
                    || panic!("expired grant read registry"),
                    |_, _| panic!("expired grant dispatched")
                )
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        fs::remove_dir_all(root).unwrap();
    }
}

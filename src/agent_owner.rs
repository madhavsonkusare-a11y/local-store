//! Launcher-only agent enrollment and permission controls. Sensitive connection
//! exports require explicit consent; previews and durable state contain no bearer.
use crate::{
    agent_gateway::AgentGateway,
    agent_policy::{AuditEvent, Grant},
    error::{AppError, AppResult, ErrorCode},
    model::InstalledApp,
    storage,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const EXPORT_SECONDS: u64 = 600;
pub const SAME_USER_LIMITATION: &str = "These permissions control the Local Store connection. An agent with unrestricted access to your Windows account can still access local files and Docker directly.";

#[derive(Serialize)]
pub struct ConnectionGrant {
    #[serde(flatten)]
    pub grant: Grant,
    pub expired: bool,
    pub installed: bool,
}
#[derive(Serialize)]
pub struct ConnectionApp {
    pub id: String,
    pub display_name: String,
    pub managed: bool,
}
#[derive(Serialize)]
pub struct AgentConnections {
    pub clients: Vec<String>,
    pub apps: Vec<ConnectionApp>,
    pub grants: Vec<ConnectionGrant>,
    pub audit: Vec<AuditEvent>,
    pub pending_approvals: Vec<String>,
    pub supported_scopes: Vec<&'static str>,
    pub sidecar_available: bool,
    pub same_user_limitation: &'static str,
}
#[derive(Serialize)]
pub struct EnrollmentPreview {
    pub enrollment_id: String,
    pub client_id: String,
    pub command: String,
    pub expires_at_unix: u64,
    pub scopes: Vec<String>,
    pub credential_placeholder: &'static str,
}
/// Never log this response; its configuration contains a bearer credential.
#[derive(Serialize)]
pub struct EnrollmentExport {
    pub client_id: String,
    pub configuration: serde_json::Value,
}
struct PendingEnrollment {
    client_id: String,
    secret: String,
    command: String,
    expires_at_unix: u64,
}
#[derive(Default)]
pub struct OwnerEnrollments {
    pending: BTreeMap<String, PendingEnrollment>,
}

impl OwnerEnrollments {
    pub fn begin(
        &mut self,
        gateway: &AgentGateway,
        client_id: &str,
        command: &Path,
        consent: bool,
        rotate: bool,
        now: u64,
    ) -> AppResult<EnrollmentPreview> {
        require_consent(consent)?;
        if client_id.len() > 80 || !crate::model::is_valid_installed_app_id(client_id) {
            return Err(AppError::invalid(
                "Use a short lowercase client name with letters, numbers and hyphens.",
            ));
        }
        if !command.is_file() {
            return Err(AppError::new(ErrorCode::PrerequisiteUnavailable, "The Local Store MCP connector is not installed. Build or install the sidecar first."));
        }
        self.pending.retain(|_, row| now < row.expires_at_unix);
        if self.pending.len() >= 16 {
            return Err(AppError::new(
                ErrorCode::OperationBusy,
                "Finish or cancel an existing connection first.",
            ));
        }
        let expires_at_unix = now
            .checked_add(EXPORT_SECONDS)
            .ok_or_else(|| AppError::invalid("Connection expiry is out of range."))?;
        let command = command
            .to_str()
            .ok_or_else(|| AppError::invalid("Connector path is not valid text."))?
            .to_owned();
        let mut random = [0_u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| AppError::internal("Could not create connection preview."))?;
        let enrollment_id: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        if rotate {
            gateway.revoke_client_for_owner(client_id)?;
            self.forget_client(client_id);
        }
        let secret = gateway.enroll_client_for_owner(client_id)?;
        self.pending.insert(
            enrollment_id.clone(),
            PendingEnrollment {
                client_id: client_id.to_owned(),
                secret,
                command: command.clone(),
                expires_at_unix,
            },
        );
        Ok(EnrollmentPreview {
            enrollment_id,
            client_id: client_id.to_owned(),
            command,
            expires_at_unix,
            scopes: Vec::new(),
            credential_placeholder: "<export after confirmation>",
        })
    }

    pub fn export(
        &mut self,
        gateway: &AgentGateway,
        enrollment_id: &str,
        consent: bool,
        now: u64,
    ) -> AppResult<EnrollmentExport> {
        require_consent(consent)?;
        self.pending.retain(|_, row| now < row.expires_at_unix);
        let row = self.pending.get(enrollment_id).ok_or_else(|| AppError::new(ErrorCode::NotFound, "Preview expired or was already exported. Rotate the connection to create new configuration."))?;
        if !gateway.credential_matches_for_owner(&row.client_id, &row.secret)? {
            self.pending.remove(enrollment_id);
            return Err(AppError::new(
                ErrorCode::Forbidden,
                "The connection was revoked or rotated.",
            ));
        }
        let row = self
            .pending
            .remove(enrollment_id)
            .expect("validated pending enrollment");
        Ok(EnrollmentExport {
            client_id: row.client_id,
            configuration: serde_json::json!({"mcpServers": {"local-store": {"command": row.command, "args": [], "env": {"LOCAL_STORE_AGENT_BEARER": row.secret}}}}),
        })
    }

    pub fn cancel(&mut self, gateway: &AgentGateway, enrollment_id: &str) -> AppResult<()> {
        if let Some(row) = self.pending.get(enrollment_id) {
            // A separate owner process may have rotated this client. Cancel
            // only the exact pending credential, never a newer enrollment.
            if gateway.credential_matches_for_owner(&row.client_id, &row.secret)? {
                gateway.revoke_client_for_owner(&row.client_id)?;
            }
            self.pending.remove(enrollment_id);
        }
        Ok(())
    }
    pub fn forget_client(&mut self, client_id: &str) {
        self.pending.retain(|_, row| row.client_id != client_id);
    }
}

pub fn connections_with(
    gateway: &AgentGateway,
    apps: &[InstalledApp],
    command: &Path,
    now: u64,
) -> AppResult<AgentConnections> {
    let snapshot = gateway.snapshot_for_owner()?;
    Ok(AgentConnections {
        clients: snapshot.clients,
        apps: apps
            .iter()
            .map(|app| ConnectionApp {
                id: app.id.clone(),
                display_name: app.display_name.clone(),
                managed: app.is_managed(),
            })
            .collect(),
        grants: snapshot
            .grants
            .into_iter()
            .map(|grant| ConnectionGrant {
                expired: now >= grant.expires_at_unix,
                installed: apps.iter().any(|app| app.id == grant.app_id),
                grant,
            })
            .collect(),
        audit: snapshot.audit.into_iter().rev().take(100).collect(),
        // This helper projects an injected policy store. The native command
        // adds pending IDs from its independently locked mutation queue.
        pending_approvals: Vec::new(),
        supported_scopes: vec!["status", "lifecycle"],
        sidecar_available: command.is_file(),
        same_user_limitation: SAME_USER_LIMITATION,
    })
}
fn require_consent(consent: bool) -> AppResult<()> {
    if consent {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::Forbidden,
            "Confirm this connection or permission change before continuing.",
        ))
    }
}
fn now() -> AppResult<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::internal("System clock is unavailable."))?
        .as_secs())
}
fn sidecar_path() -> AppResult<PathBuf> {
    let executable = std::env::current_exe().map_err(AppError::from)?;
    Ok(executable
        .parent()
        .ok_or_else(|| AppError::internal("Launcher folder is unavailable."))?
        .join(if cfg!(windows) {
            "local-store-mcp.exe"
        } else {
            "local-store-mcp"
        }))
}
fn sessions() -> AppResult<std::sync::MutexGuard<'static, OwnerEnrollments>> {
    static SESSIONS: OnceLock<Mutex<OwnerEnrollments>> = OnceLock::new();
    SESSIONS
        .get_or_init(|| Mutex::new(OwnerEnrollments::default()))
        .lock()
        .map_err(|_| AppError::internal("Connection onboarding is unavailable."))
}
#[tauri::command]
pub fn agent_connections(window: tauri::WebviewWindow) -> AppResult<AgentConnections> {
    crate::commands::require_launcher(&window)?;
    let mut connections = connections_with(
        &AgentGateway::open_local()?,
        &storage::load_or_migrate_registry()
            .map_err(AppError::from)?
            .apps,
        &sidecar_path()?,
        now()?,
    )?;
    connections.pending_approvals = crate::agent_requests::AgentRequests::open_local()?
        .list_for_owner()?
        .into_iter()
        .filter(|request| request.state == crate::agent_requests::MutationState::Pending)
        .map(|request| request.id)
        .collect();
    Ok(connections)
}
#[tauri::command]
pub fn agent_enrollment_begin(
    window: tauri::WebviewWindow,
    client_id: String,
    consent: bool,
    rotate: bool,
) -> AppResult<EnrollmentPreview> {
    crate::commands::require_launcher(&window)?;
    sessions()?.begin(
        &AgentGateway::open_local()?,
        &client_id,
        &sidecar_path()?,
        consent,
        rotate,
        now()?,
    )
}
#[tauri::command]
pub fn agent_enrollment_export(
    window: tauri::WebviewWindow,
    enrollment_id: String,
    consent: bool,
) -> AppResult<EnrollmentExport> {
    crate::commands::require_launcher(&window)?;
    sessions()?.export(
        &AgentGateway::open_local()?,
        &enrollment_id,
        consent,
        now()?,
    )
}
#[tauri::command]
pub fn agent_enrollment_cancel(
    window: tauri::WebviewWindow,
    enrollment_id: String,
) -> AppResult<()> {
    crate::commands::require_launcher(&window)?;
    sessions()?.cancel(&AgentGateway::open_local()?, &enrollment_id)
}
#[tauri::command]
pub fn agent_client_revoke(
    window: tauri::WebviewWindow,
    client_id: String,
    consent: bool,
) -> AppResult<()> {
    crate::commands::require_launcher(&window)?;
    require_consent(consent)?;
    AgentGateway::open_local()?.revoke_client_for_owner(&client_id)?;
    sessions()?.forget_client(&client_id);
    Ok(())
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerGrantScope {
    Status,
    Lifecycle,
}
#[tauri::command]
pub fn agent_grant_set(
    window: tauri::WebviewWindow,
    client_id: String,
    app_id: String,
    scope: OwnerGrantScope,
    hours: u64,
    consent: bool,
) -> AppResult<u64> {
    crate::commands::require_launcher(&window)?;
    require_consent(consent)?;
    let gateway = AgentGateway::open_local()?;
    match scope {
        OwnerGrantScope::Status => {
            gateway.grant_status_for_owner(&client_id, &app_id, hours, now()?)
        }
        OwnerGrantScope::Lifecycle => {
            gateway.grant_lifecycle_for_owner(&client_id, &app_id, hours, now()?)
        }
    }
}
#[tauri::command]
pub fn agent_grant_revoke(
    window: tauri::WebviewWindow,
    client_id: String,
    app_id: String,
) -> AppResult<()> {
    crate::commands::require_launcher(&window)?;
    AgentGateway::open_local()?.revoke_status_for_owner(&client_id, &app_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agent_policy::{AgentAction, AgentPolicyStore},
        model::RuntimeSpec,
    };
    use std::{
        collections::BTreeSet,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn setup() -> (PathBuf, AgentGateway, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "local-store-owner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let command = root.join("local-store-mcp.exe");
        std::fs::write(&command, "test sidecar").unwrap();
        let gateway = AgentGateway::open(&root.join("policy"), &root.join("auth")).unwrap();
        (root, gateway, command)
    }
    #[test]
    fn consent_export_one_use_and_redaction() {
        let (root, gateway, command) = setup();
        let mut sessions = OwnerEnrollments::default();
        assert!(sessions
            .begin(&gateway, "assistant", &command, false, false, 100)
            .is_err());
        assert!(gateway.list_clients_for_owner().unwrap().is_empty());
        let preview = sessions
            .begin(&gateway, "assistant", &command, true, false, 100)
            .unwrap();
        let secret = sessions.pending[&preview.enrollment_id].secret.clone();
        assert!(preview.scopes.is_empty());
        assert!(!serde_json::to_string(&preview).unwrap().contains(&secret));
        assert!(
            !serde_json::to_string(&gateway.snapshot_for_owner().unwrap())
                .unwrap()
                .contains(&secret)
        );
        assert!(sessions
            .export(&gateway, &preview.enrollment_id, false, 101)
            .is_err());
        let exported = sessions
            .export(&gateway, &preview.enrollment_id, true, 101)
            .unwrap();
        assert_eq!(
            exported.configuration["mcpServers"]["local-store"]["env"]["LOCAL_STORE_AGENT_BEARER"],
            secret
        );
        assert_eq!(
            exported.configuration["mcpServers"]["local-store"]["command"],
            command.to_str().unwrap()
        );
        assert!(sessions
            .export(&gateway, &preview.enrollment_id, true, 102)
            .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn expiry_revocation_restart_and_cancel_fail_closed() {
        let (root, gateway, command) = setup();
        let mut sessions = OwnerEnrollments::default();
        let expired = sessions
            .begin(&gateway, "expired", &command, true, false, 100)
            .unwrap();
        assert!(sessions
            .export(&gateway, &expired.enrollment_id, true, 700)
            .is_err());
        let revoked = sessions
            .begin(&gateway, "revoked", &command, true, false, 100)
            .unwrap();
        gateway.revoke_client_for_owner("revoked").unwrap();
        assert!(sessions
            .export(&gateway, &revoked.enrollment_id, true, 101)
            .is_err());
        let restarted = sessions
            .begin(&gateway, "restart", &command, true, false, 100)
            .unwrap();
        assert!(OwnerEnrollments::default()
            .export(&gateway, &restarted.enrollment_id, true, 101)
            .is_err());
        sessions.cancel(&gateway, &restarted.enrollment_id).unwrap();
        assert!(!gateway
            .list_clients_for_owner()
            .unwrap()
            .contains(&"restart".into()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_sidecar_has_no_enrollment_side_effect_and_cancel_preserves_new_token() {
        let (root, gateway, command) = setup();
        let mut sessions = OwnerEnrollments::default();
        assert!(sessions
            .begin(
                &gateway,
                "assistant",
                &root.join("absent.exe"),
                true,
                false,
                100
            )
            .is_err());
        assert!(gateway.list_clients_for_owner().unwrap().is_empty());
        let preview = sessions
            .begin(&gateway, "assistant", &command, true, false, 100)
            .unwrap();
        gateway.revoke_client_for_owner("assistant").unwrap();
        let replacement = gateway.enroll_client_for_owner("assistant").unwrap();
        sessions.cancel(&gateway, &preview.enrollment_id).unwrap();
        assert!(gateway
            .credential_matches_for_owner("assistant", &replacement)
            .unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rotation_drops_old_grants_and_tokens() {
        let (root, gateway, command) = setup();
        let mut sessions = OwnerEnrollments::default();
        let first = sessions
            .begin(&gateway, "assistant", &command, true, false, 100)
            .unwrap();
        let old = sessions.pending[&first.enrollment_id].secret.clone();
        AgentPolicyStore::open(&root.join("policy"))
            .unwrap()
            .grant(Grant {
                client_id: "assistant".into(),
                app_id: "memos".into(),
                actions: BTreeSet::from([AgentAction::Status]),
                expires_at_unix: 1000,
            })
            .unwrap();
        assert!(sessions
            .begin(&gateway, "assistant", &command, true, false, 101)
            .is_err());
        let rotated = sessions
            .begin(&gateway, "assistant", &command, true, true, 102)
            .unwrap();
        assert!(sessions
            .export(&gateway, &first.enrollment_id, true, 103)
            .is_err());
        assert!(!gateway
            .credential_matches_for_owner("assistant", &old)
            .unwrap());
        assert!(gateway.snapshot_for_owner().unwrap().grants.is_empty());
        assert!(sessions
            .export(&gateway, &rotated.enrollment_id, true, 103)
            .is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn snapshot_marks_expired_and_missing_apps_without_fake_approvals() {
        let (root, gateway, command) = setup();
        let secret = gateway.enroll_client_for_owner("assistant").unwrap();
        let mut policy = AgentPolicyStore::open(&root.join("policy")).unwrap();
        for app_id in ["memos", "missing"] {
            policy
                .grant(Grant {
                    client_id: "assistant".into(),
                    app_id: app_id.into(),
                    actions: BTreeSet::from([AgentAction::Status]),
                    expires_at_unix: 200,
                })
                .unwrap();
        }
        drop(policy);
        let app = InstalledApp {
            id: "memos".into(),
            catalog_id: Some("memos".into()),
            display_name: "Memos".into(),
            launch_url: "http://localhost:1234".into(),
            icon_path: None,
            runtime: RuntimeSpec::External,
            created_at_unix: 1,
            updated_at_unix: 1,
        };
        let snapshot = connections_with(&gateway, &[app], &command, 200).unwrap();
        assert!(snapshot.grants.iter().all(|row| row.expired));
        assert!(snapshot.grants.iter().any(|row| !row.installed));
        assert!(snapshot.pending_approvals.is_empty());
        assert!(!serde_json::to_string(&snapshot).unwrap().contains(&secret));
        std::fs::remove_dir_all(root).unwrap();
    }
}

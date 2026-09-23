//! The installed-app access directory consumed by a future authenticated broker.
//! This module is internal Rust API; it grants no agent or WebView access.

use crate::{
    agent_policy::{AgentAction, AgentPolicy},
    error::{AppError, AppResult, ErrorCode},
    model::InstalledApp,
    offerings::offerings,
    runtime::{self, AppStatus},
    storage::{self, RegistryV2},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const DIRECTORY: &str = include_str!("../catalog/agent-access.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessDirectory {
    schema_version: u32,
    apps: Vec<AccessRow>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessRow {
    offering_id: String,
    content_access: ContentAccess,
    required_setup: RequiredSetup,
    methods: Vec<ContentMethod>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentAccess {
    Unverified,
    VerifiedRead,
    VerifiedReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredSetup {
    ProviderReview,
    UserLogin,
    CredentialGrant,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentMethod {
    OfficialMcp,
    TypedApi,
    IsolatedBrowser,
    AppFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppAccess {
    /// Registry identity. Never infer an install from a catalog entry alone.
    pub installed_id: String,
    pub display_name: String,
    pub offering_id: Option<String>,
    pub content_access: Option<ContentAccess>,
    pub required_setup: Option<RequiredSetup>,
    /// There is no agent identity or grant store yet (A02).
    pub grant_state: GrantState,
    /// A local address does not establish that the app accepted a login.
    pub login_state: LoginState,
    pub content_methods: Vec<ContentMethod>,
    pub management_tools: Vec<StoreTool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantState {
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreTool {
    GetStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolResult {
    Status { status: AppStatus },
}

impl AccessDirectory {
    fn load() -> Result<Self, String> {
        let directory: Self = serde_json::from_str(DIRECTORY).map_err(|e| e.to_string())?;
        if directory.schema_version != 1 {
            return Err("unsupported agent-access directory version".into());
        }
        let declared: BTreeSet<_> = directory
            .apps
            .iter()
            .map(|row| row.offering_id.as_str())
            .collect();
        let offered: BTreeSet<_> = offerings().iter().map(|app| app.id().to_owned()).collect();
        if declared.len() != directory.apps.len()
            || declared
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
                != offered
        {
            return Err(
                "agent-access directory must cover each approved offering exactly once".into(),
            );
        }
        for row in &directory.apps {
            if row.content_access == ContentAccess::Unverified && !row.methods.is_empty() {
                return Err(format!(
                    "{} has methods without verified access",
                    row.offering_id
                ));
            }
            if row.content_access != ContentAccess::Unverified && row.methods.is_empty() {
                return Err(format!(
                    "{} claims access without a method",
                    row.offering_id
                ));
            }
        }
        Ok(directory)
    }

    fn describe(&self, app: &InstalledApp) -> AppAccess {
        let row = app
            .is_managed()
            .then(|| self.apps.iter().find(|row| row.offering_id == app.id))
            .flatten();
        AppAccess {
            installed_id: app.id.clone(),
            display_name: app.display_name.clone(),
            offering_id: row.map(|row| row.offering_id.clone()),
            content_access: row.map(|row| row.content_access),
            required_setup: row.map(|row| row.required_setup),
            grant_state: GrantState::Unavailable,
            login_state: LoginState::Unknown,
            content_methods: row.map(|row| row.methods.clone()).unwrap_or_default(),
            management_tools: vec![StoreTool::GetStatus],
        }
    }
}

fn directory() -> AppResult<AccessDirectory> {
    AccessDirectory::load().map_err(AppError::internal)
}

pub fn list_apps() -> AppResult<Vec<AppAccess>> {
    let registry = storage::load_or_migrate_registry().map_err(AppError::from)?;
    list_from(&registry)
}

pub fn list_from(registry: &RegistryV2) -> AppResult<Vec<AppAccess>> {
    let directory = directory()?;
    Ok(registry
        .apps
        .iter()
        .map(|app| directory.describe(app))
        .collect())
}

pub fn describe_app(app_id: &str) -> AppResult<AppAccess> {
    let registry = storage::load_or_migrate_registry().map_err(AppError::from)?;
    let app = registry
        .apps
        .iter()
        .find(|app| app.id == app_id)
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "That app is not installed."))?;
    Ok(directory()?.describe(app))
}

pub fn list_app_tools(app_id: &str) -> AppResult<Vec<StoreTool>> {
    Ok(describe_app(app_id)?.management_tools)
}

/// The typed invocation seam. An external transport must authenticate its
/// caller; no IPC/MCP command exposes this function yet.
pub fn call_app_tool(
    policy: &mut AgentPolicy,
    client_id: &str,
    app_id: &str,
    tool: StoreTool,
    now_unix: u64,
) -> AppResult<ToolResult> {
    let registry = storage::load_or_migrate_registry().map_err(AppError::from)?;
    call_authorized_with(
        &registry,
        policy,
        client_id,
        app_id,
        tool,
        now_unix,
        runtime::status,
    )
}

fn call_authorized_with(
    registry: &RegistryV2,
    policy: &mut AgentPolicy,
    client_id: &str,
    app_id: &str,
    tool: StoreTool,
    now_unix: u64,
    status: impl FnOnce(&InstalledApp) -> AppResult<AppStatus>,
) -> AppResult<ToolResult> {
    if !registry.apps.iter().any(|app| app.id == app_id) {
        return Err(AppError::new(
            ErrorCode::NotFound,
            "That app is not installed.",
        ));
    }
    let action = match tool {
        StoreTool::GetStatus => AgentAction::Status,
    };
    policy.authorize(client_id, app_id, action, None, now_unix)?;
    call_with(registry, app_id, tool, status)
}

fn call_with(
    registry: &RegistryV2,
    app_id: &str,
    tool: StoreTool,
    status: impl FnOnce(&InstalledApp) -> AppResult<AppStatus>,
) -> AppResult<ToolResult> {
    let app = registry
        .apps
        .iter()
        .find(|app| app.id == app_id)
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "That app is not installed."))?;
    match tool {
        StoreTool::GetStatus => Ok(ToolResult::Status {
            status: status(app)?,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RuntimeSpec;

    fn installed(id: &str, managed: bool) -> InstalledApp {
        InstalledApp {
            id: id.into(),
            catalog_id: None,
            display_name: id.into(),
            launch_url: "http://127.0.0.1:5000".into(),
            icon_path: None,
            runtime: if managed {
                RuntimeSpec::Compose {
                    project_name: id.into(),
                    project_dir: "app".into(),
                    compose_file: "app/compose.yaml".into(),
                }
            } else {
                RuntimeSpec::External
            },
            created_at_unix: 0,
            updated_at_unix: 0,
        }
    }

    #[test]
    fn directory_covers_exactly_the_approved_offer_ids() {
        let directory = AccessDirectory::load().unwrap();
        assert_eq!(directory.apps.len(), 52);
    }

    #[test]
    fn discovery_follows_installed_state_and_does_not_invent_content_access() {
        let registry =
            RegistryV2::new(vec![installed("memos", true), installed("external", false)]);
        let apps = list_from(&registry).unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].offering_id.as_deref(), Some("memos"));
        assert_eq!(apps[0].content_access, Some(ContentAccess::Unverified));
        assert!(apps[0].content_methods.is_empty());
        assert_eq!(apps[0].grant_state, GrantState::Unavailable);
        assert_eq!(apps[0].login_state, LoginState::Unknown);
        assert_eq!(apps[1].offering_id, None);
        assert_eq!(apps[1].content_access, None);
    }

    #[test]
    fn typed_invocation_refuses_absent_apps_before_dispatch() {
        let registry = RegistryV2::new(vec![installed("memos", true)]);
        let absent = call_with(&registry, "other", StoreTool::GetStatus, |_| {
            panic!("dispatched")
        });
        assert!(absent.is_err());
        assert_eq!(
            call_with(&registry, "memos", StoreTool::GetStatus, |_| Ok(
                AppStatus::Running
            ))
            .unwrap(),
            ToolResult::Status {
                status: AppStatus::Running
            }
        );
    }

    #[test]
    fn typed_invocation_refuses_an_unauthorized_client_before_dispatch() {
        let registry = RegistryV2::new(vec![installed("memos", true)]);
        let mut policy = AgentPolicy::default();
        let result = call_authorized_with(
            &registry,
            &mut policy,
            "client",
            "memos",
            StoreTool::GetStatus,
            100,
            |_| panic!("dispatched without a grant"),
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::Forbidden);
    }
}

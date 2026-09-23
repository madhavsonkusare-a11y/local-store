//! In-process authorization for a future authenticated agent transport.
//! The caller identity must be established outside this module; app content,
//! credentials and raw arguments never enter its audit records.

use crate::error::{AppError, AppResult, ErrorCode};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentAction {
    Status,
    ReadContent,
    WriteContent,
    Destructive,
}

impl AgentAction {
    pub fn needs_approval(self) -> bool {
        matches!(self, Self::WriteContent | Self::Destructive)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub client_id: String,
    pub app_id: String,
    pub actions: BTreeSet<AgentAction>,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Approval {
    client_id: String,
    app_id: String,
    action: AgentAction,
    expires_at_unix: u64,
}

/// Audit metadata only. No prompt, tool arguments, response or credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditEvent {
    pub client_id: String,
    pub app_id: String,
    pub action: AgentAction,
    pub allowed: bool,
    pub at_unix: u64,
}

#[derive(Default)]
pub struct AgentPolicy {
    grants: BTreeMap<(String, String), Grant>,
    approvals: BTreeMap<String, Approval>,
    audit: Vec<AuditEvent>,
}

impl AgentPolicy {
    /// Owner-side mutation only. A03 must never expose this to an agent client.
    pub fn grant(&mut self, grant: Grant) -> AppResult<()> {
        if !valid_id(&grant.client_id)
            || !valid_id(&grant.app_id)
            || grant.actions.is_empty()
            || grant.expires_at_unix == 0
        {
            return Err(AppError::invalid("Invalid agent grant."));
        }
        self.grants
            .insert((grant.client_id.clone(), grant.app_id.clone()), grant);
        Ok(())
    }

    pub fn revoke(&mut self, client_id: &str, app_id: &str) {
        self.grants
            .remove(&(client_id.to_owned(), app_id.to_owned()));
        self.approvals
            .retain(|_, value| value.client_id != client_id || value.app_id != app_id);
    }

    /// Registers a one-use owner approval. This method must only be reached
    /// from a trusted owner flow; agent transports must not expose it.
    pub fn approve(
        &mut self,
        client_id: &str,
        app_id: &str,
        action: AgentAction,
        expires_at_unix: u64,
    ) -> AppResult<String> {
        if !valid_id(client_id)
            || !valid_id(app_id)
            || expires_at_unix == 0
            || !action.needs_approval()
        {
            return Err(AppError::invalid("Invalid agent approval."));
        }
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| AppError::internal("Could not create an agent approval."))?;
        let ticket = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.approvals.insert(
            ticket.clone(),
            Approval {
                client_id: client_id.to_owned(),
                app_id: app_id.to_owned(),
                action,
                expires_at_unix,
            },
        );
        Ok(ticket)
    }

    /// Fail closed. A denied attempt records only scope and result.
    pub fn authorize(
        &mut self,
        client_id: &str,
        app_id: &str,
        action: AgentAction,
        ticket: Option<&str>,
        now_unix: u64,
    ) -> AppResult<()> {
        let granted = self
            .grants
            .get(&(client_id.to_owned(), app_id.to_owned()))
            .is_some_and(|grant| {
                now_unix < grant.expires_at_unix && grant.actions.contains(&action)
            });
        let approved = if action.needs_approval() {
            ticket
                .and_then(|value| self.approvals.remove(value))
                .is_some_and(|approval| {
                    approval.client_id == client_id
                        && approval.app_id == app_id
                        && approval.action == action
                        && now_unix < approval.expires_at_unix
                })
        } else {
            ticket.is_none()
        };
        let allowed = granted && approved && valid_id(client_id) && valid_id(app_id);
        self.audit.push(AuditEvent {
            client_id: bounded_id(client_id),
            app_id: bounded_id(app_id),
            action,
            allowed,
            at_unix: now_unix,
        });
        if allowed {
            Ok(())
        } else {
            Err(AppError::new(ErrorCode::Forbidden, "Agent access denied."))
        }
    }

    pub fn audit(&self) -> &[AuditEvent] {
        &self.audit
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn bounded_id(value: &str) -> String {
    if valid_id(value) {
        value.to_owned()
    } else {
        "invalid".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(client: &str, app: &str, action: AgentAction) -> Grant {
        Grant {
            client_id: client.into(),
            app_id: app.into(),
            actions: BTreeSet::from([action]),
            expires_at_unix: 200,
        }
    }

    #[test]
    fn scope_expiry_and_revocation_fail_closed() {
        let mut policy = AgentPolicy::default();
        policy
            .grant(grant("client", "memos", AgentAction::Status))
            .unwrap();
        assert!(policy
            .authorize("client", "memos", AgentAction::Status, None, 100)
            .is_ok());
        for (client, app, now) in [
            ("other", "memos", 100),
            ("client", "other", 100),
            ("client", "memos", 200),
        ] {
            assert!(policy
                .authorize(client, app, AgentAction::Status, None, now)
                .is_err());
        }
        policy.revoke("client", "memos");
        assert!(policy
            .authorize("client", "memos", AgentAction::Status, None, 100)
            .is_err());
        assert_eq!(policy.audit().len(), 5);
    }

    #[test]
    fn write_needs_exact_one_use_approval() {
        let mut policy = AgentPolicy::default();
        policy
            .grant(grant("client", "memos", AgentAction::WriteContent))
            .unwrap();
        assert!(policy
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&"f".repeat(64)),
                100
            )
            .is_err());
        let ticket = policy
            .approve("client", "memos", AgentAction::WriteContent, 150)
            .unwrap();
        assert!(policy
            .authorize(
                "client",
                "other",
                AgentAction::WriteContent,
                Some(&ticket),
                100
            )
            .is_err());
        assert!(policy
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&ticket),
                100
            )
            .is_err());
        let ticket = policy
            .approve("client", "memos", AgentAction::WriteContent, 150)
            .unwrap();
        assert!(policy
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&ticket),
                100
            )
            .is_ok());
        assert!(policy
            .authorize(
                "client",
                "memos",
                AgentAction::WriteContent,
                Some(&ticket),
                100
            )
            .is_err());
    }

    #[test]
    fn audit_contains_only_bounded_scope_metadata() {
        let mut policy = AgentPolicy::default();
        let _ = policy.authorize(
            "secret:credential",
            "memos",
            AgentAction::ReadContent,
            None,
            10,
        );
        let json = serde_json::to_string(policy.audit()).unwrap();
        assert!(!json.contains("secret"));
        assert!(json.contains("\"client_id\":\"invalid\""));
    }
}

//! Read-only, typed launcher state assembled from backend facts. The V3 UI
//! should consume this contract instead of inferring setup from Compose files
//! or treating an operation notification as a durable audit record.
use crate::{
    model::InstalledApp,
    recovery::{OwnershipStatus, RecoveryCandidate},
    runtime::engine::wsl::bootstrap::{
        BootstrapStatus, ManagedDaemonState, ManagedEngineStatus, RecoveryDisposition,
        WslPrerequisiteState,
    },
};
use serde::Serialize;

mod onboarding_store;
pub use onboarding_store::{OnboardingProgress, OnboardingStep, OnboardingStore};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineStage {
    PrerequisitesRequired,
    NotConfigured,
    ImportRetryPossible,
    VerificationPending,
    ManualReview,
    DaemonUnchecked,
    DaemonUnresponsive,
    Ready,
}

/// The stage is a status, not permission to execute recovery. Bootstrap and
/// daemon mutations must continue to recheck ownership under their locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineProjection {
    pub stage: EngineStage,
    pub disk_available_bytes: Option<u64>,
}

pub fn engine(status: &ManagedEngineStatus) -> EngineProjection {
    let stage = match status.bootstrap {
        BootstrapStatus::Recovery {
            disposition: RecoveryDisposition::ManualReview,
            ..
        } => EngineStage::ManualReview,
        _ if status.prerequisites.state != WslPrerequisiteState::Ready => {
            EngineStage::PrerequisitesRequired
        }
        BootstrapStatus::NotConfigured => EngineStage::NotConfigured,
        BootstrapStatus::Recovery {
            disposition: RecoveryDisposition::RetryImport,
            ..
        } => EngineStage::ImportRetryPossible,
        BootstrapStatus::Recovery {
            disposition: RecoveryDisposition::ResumeVerification,
            ..
        } => EngineStage::VerificationPending,
        BootstrapStatus::Recovery {
            disposition: RecoveryDisposition::Ready,
            ..
        } => match status.daemon {
            ManagedDaemonState::Responsive => EngineStage::Ready,
            ManagedDaemonState::NotChecked => EngineStage::DaemonUnchecked,
            ManagedDaemonState::Unresponsive => EngineStage::DaemonUnresponsive,
        },
    };
    EngineProjection {
        stage,
        disk_available_bytes: status.disk_available_bytes,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OnboardingProjection {
    pub engine: EngineProjection,
    pub managed_apps: usize,
    pub external_apps: usize,
    pub retained_setups: usize,
}

/// Current facts only. A later F02 batch must persist the person's onboarding
/// progress; an installed app count is not a substitute for consent history.
pub fn onboarding(
    status: &ManagedEngineStatus,
    installed: &[InstalledApp],
    retained: &[RecoveryCandidate],
) -> OnboardingProjection {
    OnboardingProjection {
        engine: engine(status),
        managed_apps: installed.iter().filter(|app| app.is_managed()).count(),
        external_apps: installed.iter().filter(|app| !app.is_managed()).count(),
        retained_setups: retained.len(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStage {
    NoContainers,
    OwnershipVerified,
    OwnershipUncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryProjection {
    pub app_id: String,
    pub display_name: String,
    pub location: PathBuf,
    pub stage: RecoveryStage,
}

/// A verification snapshot for display only. No projected value authorizes
/// discard, import, repair, or removal without a fresh locked backend check.
pub fn recovery(candidate: &RecoveryCandidate) -> RecoveryProjection {
    let stage = match candidate.ownership_status {
        OwnershipStatus::NoContainers => RecoveryStage::NoContainers,
        OwnershipStatus::Verified if candidate.docker_ownership_verified => {
            RecoveryStage::OwnershipVerified
        }
        OwnershipStatus::Verified | OwnershipStatus::Mismatch | OwnershipStatus::NotChecked => {
            RecoveryStage::OwnershipUncertain
        }
    };
    RecoveryProjection {
        app_id: candidate.recipe_id.clone(),
        display_name: candidate.display_name.clone(),
        location: candidate.compose_file.clone(),
        stage,
    }
}

/// One read-only response for V3. Every field is derived from a backend
/// snapshot; callers must refresh before showing a later state or taking an
/// action. No field in this response is an authorization token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LauncherState {
    pub onboarding: OnboardingProjection,
    pub progress: OnboardingProgress,
    pub recovery: Vec<RecoveryProjection>,
}

pub fn launcher_state(
    status: &ManagedEngineStatus,
    installed: &[InstalledApp],
    retained: &[RecoveryCandidate],
    progress: &OnboardingProgress,
) -> LauncherState {
    LauncherState {
        onboarding: onboarding(status, installed, retained),
        progress: progress.clone(),
        recovery: retained.iter().map(recovery).collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::RuntimeSpec,
        runtime::engine::wsl::bootstrap::{
            BootstrapState, WslPrerequisiteReason, WslPrerequisites,
        },
    };

    fn status(disposition: RecoveryDisposition, daemon: ManagedDaemonState) -> ManagedEngineStatus {
        ManagedEngineStatus {
            bootstrap: BootstrapStatus::Recovery {
                phase: BootstrapState::Verified,
                disposition,
            },
            prerequisites: WslPrerequisites {
                state: WslPrerequisiteState::Ready,
                reason: None,
                setup_requires_elevation: false,
                restart_may_be_required: false,
            },
            daemon,
            disk_available_bytes: None,
        }
    }

    #[test]
    fn readiness_requires_verified_ownership_prerequisites_and_responsive_daemon() {
        let ready = status(RecoveryDisposition::Ready, ManagedDaemonState::Responsive);
        assert_eq!(engine(&ready).stage, EngineStage::Ready);
        let mut unknown = ready;
        unknown.daemon = ManagedDaemonState::NotChecked;
        assert_eq!(engine(&unknown).stage, EngineStage::DaemonUnchecked);
        unknown.prerequisites.state = WslPrerequisiteState::SetupRequired;
        unknown.prerequisites.reason = Some(WslPrerequisiteReason::StatusRejected);
        assert_eq!(engine(&unknown).stage, EngineStage::PrerequisitesRequired);
        unknown.bootstrap = BootstrapStatus::Recovery {
            phase: BootstrapState::Verified,
            disposition: RecoveryDisposition::ManualReview,
        };
        assert_eq!(engine(&unknown).stage, EngineStage::ManualReview);
    }

    #[test]
    fn onboarding_counts_existing_registry_records_without_claiming_persistence() {
        let app = |id: &str, runtime| InstalledApp {
            id: id.into(),
            catalog_id: None,
            display_name: id.into(),
            launch_url: "http://127.0.0.1:1/".into(),
            icon_path: None,
            runtime,
            created_at_unix: 1,
            updated_at_unix: 1,
        };
        let installed = [
            app(
                "managed",
                RuntimeSpec::Compose {
                    project_name: "local-store-managed".into(),
                    project_dir: PathBuf::from("/managed"),
                    compose_file: PathBuf::from("/managed/compose.yaml"),
                },
            ),
            app("external", RuntimeSpec::External),
        ];
        let projection = onboarding(
            &status(RecoveryDisposition::Ready, ManagedDaemonState::Responsive),
            &installed,
            &[],
        );
        assert_eq!(projection.managed_apps, 1);
        assert_eq!(projection.external_apps, 1);
        assert_eq!(projection.retained_setups, 0);
        assert_eq!(projection.engine.stage, EngineStage::Ready);
    }

    #[test]
    fn recovery_projection_never_promotes_unverified_label_claims() {
        let mut candidate = RecoveryCandidate {
            recipe_id: "memos".into(),
            display_name: "Memos".into(),
            compose_file: PathBuf::from("/apps/memos/compose.yaml"),
            project_name: "local-store-memos".into(),
            docker_ownership_verified: false,
            ownership_status: OwnershipStatus::Verified,
        };
        assert_eq!(
            recovery(&candidate).stage,
            RecoveryStage::OwnershipUncertain
        );
        candidate.docker_ownership_verified = true;
        assert_eq!(recovery(&candidate).stage, RecoveryStage::OwnershipVerified);
        candidate.ownership_status = OwnershipStatus::Mismatch;
        assert_eq!(
            recovery(&candidate).stage,
            RecoveryStage::OwnershipUncertain
        );
    }
}

#[cfg(test)]
mod snapshot_contract_tests {
    use super::*;
    use crate::runtime::engine::wsl::bootstrap::{BootstrapState, WslPrerequisites};

    #[test]
    fn launcher_snapshot_serializes_real_state_without_claiming_an_action() {
        let status = ManagedEngineStatus {
            bootstrap: BootstrapStatus::Recovery {
                phase: BootstrapState::Verified,
                disposition: RecoveryDisposition::ManualReview,
            },
            prerequisites: WslPrerequisites {
                state: WslPrerequisiteState::Ready,
                reason: None,
                setup_requires_elevation: false,
                restart_may_be_required: false,
            },
            daemon: ManagedDaemonState::NotChecked,
            disk_available_bytes: None,
        };
        let progress = OnboardingProgress::default();
        let state = launcher_state(&status, &[], &[], &progress);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["onboarding"]["engine"]["stage"], "manual_review");
        assert_eq!(
            json["onboarding"]["engine"]["disk_available_bytes"],
            serde_json::Value::Null
        );
        assert_eq!(json["progress"]["viewed"], serde_json::json!([]));
        assert_eq!(json["recovery"], serde_json::json!([]));
        assert!(json.get("action_available").is_none());
    }
}

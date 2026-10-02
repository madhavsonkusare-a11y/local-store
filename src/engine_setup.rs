//! Owner-only engine setup. Native code supplies resource and ownership paths;
//! browser callers supply only an action and explicit consent. The pinned
//! payload is a development build, not an authenticated remote download.
use crate::{
    error::{AppError, AppResult},
    runtime::{
        self,
        engine::{
            wsl::bootstrap::{
                self, BootstrapJournal, BootstrapStatus, ManagedEngineStatus, RecoveryDisposition,
                WslPrerequisiteState,
            },
            EngineBinding,
        },
        ProcessRunner,
    },
    storage,
};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

const ROOTFS_BYTES: u64 = 572_798_976;
const ROOTFS_SHA256: &str = "ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32";
const SELECTION_FILE: &str = "selected-engine.json";
const SELECTION_MARKER: &str = "engine-selection-configured";
/// Conservative admission budget based on the measured archive: three copies
/// for import/extraction and 512 MiB of setup headroom. This is not a measured
/// clean-machine support minimum, or capacity for application images/data.
pub const SETUP_REQUIRED_BYTES: u64 = ROOTFS_BYTES * 3 + 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSetupAction {
    Install,
    RetryImport,
    ResumeVerification,
    Repair,
    SelectManaged,
    AdoptDevelopment,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSetupConsent {
    pub create_owned_engine: bool,
    pub use_disk_space: bool,
    pub acknowledge_existing_apps_unchanged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadState {
    Missing,
    VerifiedDevelopment,
    Invalid,
}

#[derive(Debug, Serialize)]
pub struct EngineSetupPreview {
    pub engine: ManagedEngineStatus,
    pub payload: PayloadState,
    pub payload_bytes: u64,
    pub required_disk_bytes: u64,
    pub disk_sufficient: Option<bool>,
    pub payload_release_approved: bool,
    pub setup_requires_administrator: bool,
    pub restart_may_be_required: bool,
    pub selected_engine: Option<EngineBinding>,
    /// Existing explicitly owned developer engine; copying its ownership
    /// record does not move its virtual disk or adopt another product's distro.
    pub development_engine_available: bool,
}

/// Native resource resolution must pass the fixed `engine/rootfs.tar` path.
/// No remote URL, browser-provided digest or browser-provided path is accepted.
pub fn preview(
    runner: &dyn ProcessRunner,
    packaged_rootfs: &Path,
) -> AppResult<EngineSetupPreview> {
    let engine = bootstrap::status(
        runner,
        &storage::managed_engine_state_root(),
        &storage::managed_engine_data_root(),
    )?;
    let payload = if !packaged_rootfs.is_file() {
        PayloadState::Missing
    } else if bootstrap::verify_rootfs(packaged_rootfs, ROOTFS_BYTES, ROOTFS_SHA256).is_ok() {
        PayloadState::VerifiedDevelopment
    } else {
        PayloadState::Invalid
    };
    Ok(EngineSetupPreview {
        disk_sufficient: engine
            .disk_available_bytes
            .map(|bytes| bytes >= SETUP_REQUIRED_BYTES),
        setup_requires_administrator: engine.prerequisites.setup_requires_elevation,
        restart_may_be_required: engine.prerequisites.restart_may_be_required,
        engine,
        payload,
        payload_bytes: ROOTFS_BYTES,
        required_disk_bytes: SETUP_REQUIRED_BYTES,
        payload_release_approved: false,
        selected_engine: selected_at(&storage::managed_engine_state_root())?,
        development_engine_available: development_source_ready(runner),
    })
}

fn check_consent(action: EngineSetupAction, consent: EngineSetupConsent) -> AppResult<()> {
    if !consent.acknowledge_existing_apps_unchanged {
        return Err(AppError::invalid("Confirm that engine selection applies to new installs and does not migrate existing apps."));
    }
    if matches!(
        action,
        EngineSetupAction::Install
            | EngineSetupAction::RetryImport
            | EngineSetupAction::ResumeVerification
            | EngineSetupAction::AdoptDevelopment
    ) && !(consent.create_owned_engine && consent.use_disk_space)
    {
        return Err(AppError::invalid("Creating or completing the Local Store engine requires explicit engine and disk-space consent."));
    }
    Ok(())
}

fn admit_disk(available: Option<u64>) -> AppResult<()> {
    match available {
        Some(bytes) if bytes >= SETUP_REQUIRED_BYTES => Ok(()),
        Some(_) => Err(AppError::invalid(format!("Engine setup needs at least {SETUP_REQUIRED_BYTES} available bytes on its data volume."))),
        None => Err(AppError::invalid("Engine setup cannot measure available disk space. Resolve volume access before retrying.")),
    }
}

/// Serializes with launcher/CLI repair. Every action re-reads ownership; a
/// preview or viewed onboarding step is never permission to mutate the engine.
pub fn execute(
    runner: &dyn ProcessRunner,
    packaged_rootfs: &Path,
    action: EngineSetupAction,
    consent: EngineSetupConsent,
) -> AppResult<ManagedEngineStatus> {
    check_consent(action, consent)?;
    let _lock = runtime::lock_operation("managed-engine")?;
    let state = storage::managed_engine_state_root();
    let data = storage::managed_engine_data_root();
    let prerequisites = bootstrap::prerequisites(runner)?;
    if prerequisites.state != WslPrerequisiteState::Ready {
        return Err(AppError::invalid("Windows needs WSL 2 before engine setup. Administrator approval and a restart may be required."));
    }
    match action {
        EngineSetupAction::Install => {
            if bootstrap::load(&state)?.is_some() {
                return Err(AppError::invalid(
                    "An engine setup already exists. Use its explicit recovery action.",
                ));
            }
            admit_disk(bootstrap::available_disk_bytes(&data))?;
            bootstrap::verify_rootfs(packaged_rootfs, ROOTFS_BYTES, ROOTFS_SHA256)?;
            let mut token = [0_u8; 32];
            getrandom::fill(&mut token).map_err(AppError::internal)?;
            let token = token
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let journal = BootstrapJournal::new(data.clone(), ROOTFS_SHA256.into(), token)?;
            bootstrap::import(runner, &state, &journal, packaged_rootfs, ROOTFS_BYTES)?;
            bootstrap::verify_imported(runner, &state)?;
            persist_selection(&state, &EngineBinding::managed_wsl())?;
        }
        EngineSetupAction::RetryImport => {
            admit_disk(bootstrap::available_disk_bytes(&data))?;
            let journal = bootstrap::load(&state)?
                .ok_or_else(|| AppError::invalid("Engine setup is missing."))?;
            if journal.rootfs_sha256 != ROOTFS_SHA256 {
                return Err(AppError::invalid(
                    "The recovery payload differs from this build's locked engine.",
                ));
            }
            bootstrap::retry_import(runner, &state, packaged_rootfs, ROOTFS_BYTES)?;
            bootstrap::verify_imported(runner, &state)?;
            persist_selection(&state, &EngineBinding::managed_wsl())?;
        }
        EngineSetupAction::ResumeVerification => {
            if bootstrap::classify_recovery(runner, &state)?
                != RecoveryDisposition::ResumeVerification
            {
                return Err(AppError::invalid(
                    "This engine footprint cannot safely resume verification.",
                ));
            }
            bootstrap::verify_imported(runner, &state)?;
            persist_selection(&state, &EngineBinding::managed_wsl())?;
        }
        EngineSetupAction::Repair => {
            bootstrap::repair_daemon(runner, &state)?;
        }
        EngineSetupAction::SelectManaged => {
            ensure_managed_ready(runner, &state, &data)?;
            persist_selection(&state, &EngineBinding::managed_wsl())?;
        }
        EngineSetupAction::AdoptDevelopment => {
            adopt_development_record(runner, &development_source(), &state)?;
            ensure_managed_ready(runner, &state, &data)?;
            persist_selection(&state, &EngineBinding::managed_wsl())?;
        }
    }
    bootstrap::status(runner, &state, &data)
}

fn ensure_managed_ready(runner: &dyn ProcessRunner, state: &Path, data: &Path) -> AppResult<()> {
    let status = bootstrap::status(runner, state, data)?;
    if !matches!(
        status.bootstrap,
        BootstrapStatus::Recovery {
            disposition: RecoveryDisposition::Ready,
            ..
        }
    ) || status.daemon != bootstrap::ManagedDaemonState::Responsive
    {
        return Err(AppError::invalid("The selected Local Store engine needs ownership verification or repair before a new install."));
    }
    Ok(())
}

fn development_source() -> std::path::PathBuf {
    // Fixed native source-build location, never a path accepted from JavaScript.
    Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache/engine/real-wsl-proof/state")
}

fn development_source_ready(runner: &dyn ProcessRunner) -> bool {
    let state = development_source();
    bootstrap::load(&state)
        .ok()
        .flatten()
        .is_some_and(|journal| {
            journal.state == bootstrap::BootstrapState::Verified
                && development_disk_matches(&state, &journal)
                && bootstrap::classify_recovery(runner, &state).ok()
                    == Some(RecoveryDisposition::Ready)
        })
}

fn development_disk_matches(source: &Path, journal: &BootstrapJournal) -> bool {
    let Some(parent) = source.parent() else {
        return false;
    };
    let expected = parent.join("data").canonicalize();
    match (expected, journal.install_dir.canonicalize()) {
        (Ok(expected), Ok(actual)) => expected == actual,
        _ => false,
    }
}

fn adopt_development_record(
    runner: &dyn ProcessRunner,
    source: &Path,
    target: &Path,
) -> AppResult<()> {
    let journal = bootstrap::load(source)?
        .ok_or_else(|| AppError::invalid("There is no owned development engine to reuse."))?;
    if journal.state != bootstrap::BootstrapState::Verified
        || !development_disk_matches(source, &journal)
        || bootstrap::classify_recovery(runner, source)? != RecoveryDisposition::Ready
    {
        return Err(AppError::invalid("The development engine's external and in-distro ownership records must agree before reuse."));
    }
    if let Some(existing) = bootstrap::load(target)? {
        if existing != journal {
            return Err(AppError::invalid("The native engine already has a different ownership record. No state was replaced."));
        }
        // Only complete a missing token from the freshly proven original.
        let token = target.join(bootstrap::TOKEN_FILE);
        if !token.exists() {
            storage::write_file_atomically(&token, journal.ownership_token.as_bytes())?;
        }
    } else {
        bootstrap::reserve(target, &journal)?;
    }
    if bootstrap::classify_recovery(runner, target)? != RecoveryDisposition::Ready {
        return Err(AppError::invalid(
            "The copied engine ownership record needs manual review.",
        ));
    }
    Ok(())
}

fn persist_selection(state: &Path, binding: &EngineBinding) -> AppResult<()> {
    binding.validate()?;
    if !binding.is_wsl() {
        return Err(AppError::invalid("New installs require the owned Local Store engine. Existing app bindings are preserved."));
    }
    fs::create_dir_all(state)?;
    // Durable marker precedes the choice: a crash during Windows file replace
    // must refuse installs instead of silently restoring ambient Docker routing.
    let marker = state.join(SELECTION_MARKER);
    if !marker.exists() {
        storage::write_file_atomically(&marker, b"1\n")?;
    }
    let bytes = serde_json::to_vec_pretty(binding).map_err(AppError::internal)?;
    storage::write_file_atomically(&state.join(SELECTION_FILE), &bytes)?;
    Ok(())
}

pub(crate) fn selected_at(state: &Path) -> AppResult<Option<EngineBinding>> {
    let mut file = match fs::File::open(state.join(SELECTION_FILE)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !state.join(SELECTION_MARKER).exists() => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(AppError::invalid("Saved engine selection is missing. Choose an engine explicitly; no fallback was used.")),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.by_ref().take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err(AppError::invalid("Saved engine selection is too large."));
    }
    let binding: EngineBinding = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::invalid("Saved engine selection is invalid; no fallback engine was chosen.")
    })?;
    binding.validate()?;
    if !binding.is_wsl() {
        return Err(AppError::invalid("The saved selection is not the owned Local Store engine. Choose the Local Store engine explicitly."));
    }
    Ok(Some(binding))
}

/// Applies only to new installs. Existing immutable app bindings retain their
/// engine. Selecting an engine never changes global Docker context or defaults.
pub(crate) fn selected(runner: &dyn ProcessRunner) -> AppResult<Option<EngineBinding>> {
    let state = storage::managed_engine_state_root();
    let selected = selected_at(&state)?;
    if selected.as_ref().is_some_and(EngineBinding::is_wsl) {
        ensure_managed_ready(runner, &state, &storage::managed_engine_data_root())?;
        bootstrap_lease(runner, &state)?;
    }
    Ok(selected)
}

fn bootstrap_lease(runner: &dyn ProcessRunner, state: &Path) -> AppResult<()> {
    crate::runtime::engine::wsl::lease::ensure_owned(runner, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn consent() -> EngineSetupConsent {
        EngineSetupConsent {
            create_owned_engine: true,
            use_disk_space: true,
            acknowledge_existing_apps_unchanged: true,
        }
    }
    #[test]
    fn consent_is_specific_and_viewing_setup_never_grants_it() {
        let mut value = consent();
        value.use_disk_space = false;
        assert!(check_consent(EngineSetupAction::Install, value).is_err());
        assert!(check_consent(EngineSetupAction::RetryImport, value).is_err());
        assert!(check_consent(EngineSetupAction::SelectManaged, value).is_ok());
        value.acknowledge_existing_apps_unchanged = false;
        assert!(check_consent(EngineSetupAction::SelectManaged, value).is_err());
        assert!(serde_json::from_str::<EngineSetupConsent>(r#"{"create_owned_engine":true,"use_disk_space":true,"acknowledge_existing_apps_unchanged":true,"payload":"evil"}"#).is_err());
    }
    #[test]
    fn unknown_or_low_disk_refuses_before_import() {
        assert!(admit_disk(None).is_err());
        assert!(admit_disk(Some(SETUP_REQUIRED_BYTES - 1)).is_err());
        assert!(admit_disk(Some(SETUP_REQUIRED_BYTES)).is_ok());
    }
    #[test]
    fn selection_is_bounded_validated_and_refuses_missing_after_commit() {
        let state =
            std::env::temp_dir().join(format!("local-store-selection-{}", std::process::id()));
        fs::create_dir_all(&state).unwrap();
        persist_selection(&state, &EngineBinding::managed_wsl()).unwrap();
        assert_eq!(
            selected_at(&state).unwrap(),
            Some(EngineBinding::managed_wsl())
        );
        fs::write(
            state.join(SELECTION_FILE),
            br#"{"schema_version":1,"program":"docker","endpoint":"tcp://remote:2375"}"#,
        )
        .unwrap();
        assert!(selected_at(&state).is_err());
        fs::write(state.join(SELECTION_FILE), [b'x'; 8193]).unwrap();
        assert!(selected_at(&state).is_err());
        fs::remove_file(state.join(SELECTION_FILE)).unwrap();
        assert!(selected_at(&state).is_err());
        fs::remove_file(state.join(SELECTION_MARKER)).unwrap();
        assert!(selected_at(&state).unwrap().is_none());
        fs::remove_dir(state).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn development_adoption_copies_only_proven_identity_and_refuses_foreign_state() {
        use crate::runtime::{CancelToken, CommandSpec, ProcessError, ProcessOutput};
        use std::sync::Mutex;
        struct OwnerRunner(Mutex<Vec<CommandSpec>>);
        impl ProcessRunner for OwnerRunner {
            fn run_cancellable(
                &self,
                command: &CommandSpec,
                _: &CancelToken,
            ) -> Result<ProcessOutput, ProcessError> {
                assert_eq!(command.program, "wsl.exe");
                assert!(!command
                    .args
                    .iter()
                    .any(|arg| matches!(arg.as_str(), "--import" | "--unregister" | "--shutdown")));
                self.0.lock().unwrap().push(command.clone());
                Ok(ProcessOutput {
                    success: true,
                    stdout: if command.args == ["--list", "--quiet"] {
                        "local-store-engine-v1\n".into()
                    } else {
                        String::new()
                    },
                    stderr: String::new(),
                    truncated: false,
                })
            }
        }
        let root = std::env::temp_dir().join(format!(
            "local-store-engine-adoption-{}",
            std::process::id()
        ));
        let source = root.join("source");
        let target = root.join("target");
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        let mut journal =
            BootstrapJournal::new(data.clone(), ROOTFS_SHA256.into(), "a".repeat(64)).unwrap();
        journal.state = bootstrap::BootstrapState::Verified;
        bootstrap::reserve(&source, &journal).unwrap();
        let runner = OwnerRunner(Mutex::new(Vec::new()));
        adopt_development_record(&runner, &source, &target).unwrap();
        assert_eq!(bootstrap::load(&target).unwrap().unwrap(), journal);
        assert!(data.is_dir());
        fs::remove_file(target.join(bootstrap::TOKEN_FILE)).unwrap();
        adopt_development_record(&runner, &source, &target).unwrap();
        assert_eq!(
            fs::read(target.join(bootstrap::TOKEN_FILE)).unwrap(),
            journal.ownership_token.as_bytes()
        );
        let mut foreign = journal.clone();
        foreign.ownership_token = "b".repeat(64);
        let other = root.join("foreign");
        bootstrap::reserve(&other, &foreign).unwrap();
        assert!(adopt_development_record(&runner, &source, &other).is_err());
        assert_eq!(bootstrap::load(&other).unwrap().unwrap(), foreign);
        fs::write(source.join(bootstrap::TOKEN_FILE), b"tampered").unwrap();
        let absent = root.join("absent");
        assert!(adopt_development_record(&runner, &source, &absent).is_err());
        assert!(!absent.exists());
        assert!(!runner.0.lock().unwrap().is_empty());
        // Remove the exact fixture paths; never the distro or its real disk.
        for state in [&source, &target, &other] {
            fs::remove_file(state.join(bootstrap::TOKEN_FILE)).unwrap();
            fs::remove_file(state.join(bootstrap::JOURNAL_FILE)).unwrap();
            fs::remove_dir(state).unwrap();
        }
        fs::remove_dir(data).unwrap();
        fs::remove_dir(root).unwrap();
    }
}

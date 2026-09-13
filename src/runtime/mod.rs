use crate::error::{AppError, AppResult, ErrorCode};
// Docker Compose runtime behind testable process and health-check abstractions.
use crate::{
    model::{InstalledApp, RuntimeSpec},
    recipes::Recipe,
    setup::PlanTemplate,
    storage,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub mod engine;
mod process;
pub use process::{
    redact, redact_diagnostic, CancelToken, CommandSpec, ProcessError, ProcessErrorCode,
    ProcessOutput, ProcessRunner, SystemProcessRunner, DIAGNOSTIC_TIMEOUT, FIRST_START_TIMEOUT,
    LIFECYCLE_TIMEOUT, MAX_CAPTURED_BYTES, PROVISION_TIMEOUT, REDACTED,
};

/// The operation in flight for an app. One mutating operation at a time: two
/// concurrent `up`/`down` runs on the same project corrupt each other's
/// containers and can interleave registry writes.
struct ActiveOperation {
    id: u64,
    cancel: CancelToken,
}
fn active_operations() -> &'static Mutex<HashMap<String, ActiveOperation>> {
    static ACTIVE: OnceLock<Mutex<HashMap<String, ActiveOperation>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}
fn operations() -> std::sync::MutexGuard<'static, HashMap<String, ActiveOperation>> {
    active_operations()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Holds an app's operation slot, and releases it when dropped — including on
/// an early return or a panic.
#[derive(Debug)]
pub struct OperationLock {
    app_id: String,
    id: u64,
    // Kept outside the managed app directory, which uninstall can delete.
    // Never unlink a lock file: another process may still hold its inode.
    _file: fs::File,
}
impl OperationLock {
    /// Identifies this operation. A cancel request carries the id it means to
    /// stop, so it cannot land on whatever operation replaced this one.
    pub fn id(&self) -> u64 {
        self.id
    }
    /// The token this operation should thread through its cancellable work.
    pub fn token(&self) -> CancelToken {
        operations()
            .get(&self.app_id)
            .filter(|active| active.id == self.id)
            .map(|active| active.cancel.clone())
            .unwrap_or_default()
    }
}
impl Drop for OperationLock {
    fn drop(&mut self) {
        // Only clear the slot if it is still ours.
        let mut operations = operations();
        if operations
            .get(&self.app_id)
            .is_some_and(|active| active.id == self.id)
        {
            operations.remove(&self.app_id);
        }
    }
}

/// Claim the operation slot for `app_id`, or report that one is already held.
///
/// This never waits. A queued second operation would be indistinguishable from
/// a hung one, and the caller can retry once the first finishes.
pub fn lock_operation(app_id: &str) -> AppResult<OperationLock> {
    use fs4::FileExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    if !crate::model::is_valid_installed_app_id(app_id) {
        return Err(AppError::invalid("Invalid app ID for operation lock."));
    }
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let mut operations = operations();
    if operations.contains_key(app_id) {
        return Err(AppError::new(
            ErrorCode::OperationBusy,
            format!(
                "Another operation is already running for \"{app_id}\". Wait for it to finish."
            ),
        ));
    }
    #[cfg(not(test))]
    let root = storage::managed_apps_root().with_file_name("operation-locks");
    #[cfg(test)]
    let root = std::env::temp_dir().join(format!(
        "local-store-operation-tests-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(format!("{app_id}.lock")))?;
    FileExt::try_lock(&file).map_err(|error| {
        AppError::new(
            ErrorCode::OperationBusy,
            format!("Another process may be operating on \"{app_id}\": {error}"),
        )
    })?;
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    operations.insert(
        app_id.to_owned(),
        ActiveOperation {
            id,
            cancel: CancelToken::new(),
        },
    );
    Ok(OperationLock {
        app_id: app_id.to_owned(),
        id,
        _file: file,
    })
}

/// Ask the operation identified by `operation_id` to stop.
///
/// Returns whether a matching operation was running. A stale id — one whose
/// operation already finished — cancels nothing, so a late request cannot stop
/// the operation that took its place.
pub fn cancel_operation(app_id: &str, operation_id: u64) -> bool {
    operations()
        .get(app_id)
        .filter(|active| active.id == operation_id)
        .map(|active| {
            active.cancel.cancel();
            true
        })
        .unwrap_or(false)
}

/// Whether a published port is free to bind.
pub trait PortProbe: Send + Sync {
    fn available(&self, port: u16) -> bool;
}
pub struct LocalPortProbe;
impl PortProbe for LocalPortProbe {
    fn available(&self, port: u16) -> bool {
        // Binding, not connecting: a listener that accepts nothing still owns
        // the port, and Docker would fail to publish it.
        TcpListener::bind(("127.0.0.1", port)).is_ok()
    }
}

pub trait HealthProbe: Send + Sync {
    fn ready(&self, url: &str) -> bool;
}
pub struct HttpHealthProbe;
impl HealthProbe for HttpHealthProbe {
    fn ready(&self, url: &str) -> bool {
        let Ok(parsed) = tauri::Url::parse(url) else {
            return false;
        };
        if parsed.scheme() != "http" {
            return false;
        }
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let port = parsed.port().unwrap_or(80);
        let Ok(addresses) = (host.trim_matches(['[', ']']), port).to_socket_addrs() else {
            return false;
        };
        // localhost commonly resolves to ::1 first, while recipes deliberately
        // publish on 127.0.0.1. Try alternate addresses within a bounded count.
        let Some(mut stream) = addresses
            .take(4)
            .find_map(|address| TcpStream::connect_timeout(&address, Duration::from_secs(2)).ok())
        else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        // The port belongs in Host whenever it is not the scheme's default;
        // that is what every browser sends. Joplin routes on the whole Host
        // and answered `Host: localhost` with 404 — so an app that was up and
        // serving `/login` never passed its health check.
        let authority = match parsed.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let target = if parsed.path().is_empty() {
            "/"
        } else {
            parsed.path()
        };
        if write!(
            stream,
            "GET {target} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
        )
        .is_err()
        {
            return false;
        }
        let mut status = [0_u8; 64];
        let Ok(read) = stream.read(&mut status) else {
            return false;
        };
        let line = String::from_utf8_lossy(&status[..read]);
        line.starts_with("HTTP/1.0 2")
            || line.starts_with("HTTP/1.1 2")
            || line.starts_with("HTTP/1.0 3")
            || line.starts_with("HTTP/1.1 3")
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DoctorCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub ok: bool,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProcessError>,
}
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DoctorReport {
    pub ready: bool,
    pub checks: Vec<DoctorCheck>,
}
pub fn doctor_with(runner: &dyn ProcessRunner) -> DoctorReport {
    let specs = [
        (
            "docker",
            "Docker engine",
            CommandSpec::docker(
                vec![
                    "version".into(),
                    "--format".into(),
                    "{{.Server.Version}}".into(),
                ],
                None,
                DIAGNOSTIC_TIMEOUT,
            ),
        ),
        (
            "compose",
            "Docker Compose",
            CommandSpec::docker(
                vec!["compose".into(), "version".into(), "--short".into()],
                None,
                DIAGNOSTIC_TIMEOUT,
            ),
        ),
    ];
    let checks = specs
        .into_iter()
        .map(|(id, label, spec)| match runner.run(&spec) {
            Ok(output) if output.success => DoctorCheck {
                id,
                label,
                ok: true,
                detail: output.stdout.trim().to_owned(),
                error: None,
            },
            Ok(output) => DoctorCheck {
                id,
                label,
                ok: false,
                detail: concise_error(&output),
                error: Some(ProcessError::new(
                    ProcessErrorCode::ProcessFailed,
                    concise_error(&output),
                )),
            },
            Err(error) => DoctorCheck {
                id,
                label,
                ok: false,
                detail: error.to_string(),
                error: Some(error),
            },
        })
        .collect::<Vec<_>>();
    DoctorReport {
        ready: checks.iter().all(|check| check.ok),
        checks,
    }
}
pub fn doctor() -> DoctorReport {
    match engine::EngineBinding::discover(&SystemProcessRunner) {
        Ok(binding) => doctor_with(&engine::EngineRunner {
            inner: &SystemProcessRunner,
            binding: Some(binding),
        }),
        Err(error) => DoctorReport {
            ready: false,
            checks: vec![DoctorCheck {
                id: "docker",
                label: "Docker engine",
                ok: false,
                detail: error.message,
                error: None,
            }],
        },
    }
}

fn compose_command(app: &InstalledApp, args: &[&str], timeout: Duration) -> AppResult<CommandSpec> {
    match &app.runtime {
        RuntimeSpec::Compose {
            project_name,
            project_dir,
            compose_file,
        } => {
            let mut all = vec![
                "compose".into(),
                "-f".into(),
                compose_file.to_string_lossy().into_owned(),
                "-p".into(),
                project_name.clone(),
            ];
            all.extend(args.iter().map(|arg| (*arg).to_owned()));
            engine::project_command(
                project_dir,
                CommandSpec::docker(all, Some(project_dir.clone()), timeout),
            )
        }
        RuntimeSpec::External => Err(AppError::new(
            ErrorCode::UnsupportedOperation,
            "This is a connected app; Local Store does not manage its server.",
        )),
    }
}
/// Run a command that cannot be cancelled. Used by lifecycle commands and by
/// rollback, which must finish even when the operation that triggered it was
/// cancelled.
fn checked_run(
    runner: &dyn ProcessRunner,
    spec: &CommandSpec,
    action: &str,
) -> AppResult<ProcessOutput> {
    checked_run_cancellable(runner, spec, action, &CancelToken::new())
}
fn checked_run_cancellable(
    runner: &dyn ProcessRunner,
    spec: &CommandSpec,
    action: &str,
    cancel: &CancelToken,
) -> AppResult<ProcessOutput> {
    let output = runner
        .run_cancellable(spec, cancel)
        .map_err(AppError::from)?;
    if output.success {
        Ok(output)
    } else {
        Err(AppError::new(
            ErrorCode::ProcessFailed,
            format!("{action} failed: {}", concise_error(&output)),
        ))
    }
}
/// The end of a long message, which is where a failing tool says why.
///
/// Compose prints a progress line per network and container and the error
/// last. Keeping the first thousand characters kept the progress: Notemark's
/// failed start was recorded as eleven lines of "Creating", "Created",
/// "Starting" and none of the reason.
fn last_chars(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let start = text
        .char_indices()
        .nth(count - limit)
        .map_or(0, |(index, _)| index);
    format!("…{}", &text[start..])
}
fn concise_error(output: &ProcessOutput) -> String {
    let value = if output.stderr.trim().is_empty() {
        output.stdout.trim()
    } else {
        output.stderr.trim()
    };
    // Redaction runs before the length cap so a credential can never be cut
    // in half and survive as a readable prefix.
    let mut detail: String = if value.is_empty() {
        "the command returned an error".into()
    } else {
        last_chars(&redact_diagnostic(value, output.truncated), 1000)
    };
    if output.truncated {
        detail.push_str("\n(output was truncated)");
    }
    detail
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    Connected,
    Running,
    Stopped,
    Error,
}
/// Whether an app is actually answering on its address, as opposed to merely
/// having containers up.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// The address answered.
    Ready,
    /// The address did not answer. The app may still be starting.
    Unreachable,
    /// Not checked. The probe speaks plain HTTP only, so an HTTPS address is
    /// reported as unknown rather than wrongly called unreachable.
    Unknown,
}

pub fn readiness_with(probe: &dyn HealthProbe, app: &InstalledApp) -> Readiness {
    address_readiness_with(probe, &app.launch_url)
}
/// Readiness of a bare address, for an app that is not saved yet.
pub fn address_readiness_with(probe: &dyn HealthProbe, url: &str) -> Readiness {
    if !url.starts_with("http://") {
        return Readiness::Unknown;
    }
    if probe.ready(url) {
        Readiness::Ready
    } else {
        Readiness::Unreachable
    }
}
pub fn address_readiness(url: &str) -> Readiness {
    address_readiness_with(&HttpHealthProbe, url)
}
pub fn readiness(app: &InstalledApp) -> Readiness {
    readiness_with(&HttpHealthProbe, app)
}

pub fn status_with(runner: &dyn ProcessRunner, app: &InstalledApp) -> AppResult<AppStatus> {
    if !app.is_managed() {
        return Ok(AppStatus::Connected);
    }
    let output = runner
        .run(&compose_command(
            app,
            &["ps", "--status", "running", "--quiet"],
            DIAGNOSTIC_TIMEOUT,
        )?)
        .map_err(AppError::from)?;
    if !output.success {
        Err(AppError::new(
            ErrorCode::ProcessFailed,
            concise_error(&output),
        ))
    } else if output.stdout.trim().is_empty() {
        Ok(AppStatus::Stopped)
    } else {
        Ok(AppStatus::Running)
    }
}
pub fn start_with(runner: &dyn ProcessRunner, app: &InstalledApp) -> AppResult<()> {
    start_with_cancel(runner, app, &CancelToken::new())
}
pub fn start_with_cancel(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    cancel: &CancelToken,
) -> AppResult<()> {
    checked_run_cancellable(
        runner,
        &compose_command(app, &["up", "-d"], PROVISION_TIMEOUT)?,
        "Start",
        cancel,
    )
    .map(|_| ())
}
pub fn stop_with(runner: &dyn ProcessRunner, app: &InstalledApp) -> AppResult<()> {
    checked_run(
        runner,
        &compose_command(app, &["stop"], LIFECYCLE_TIMEOUT)?,
        "Stop",
    )
    .map(|_| ())
}
pub fn logs_with(runner: &dyn ProcessRunner, app: &InstalledApp) -> AppResult<String> {
    let output = checked_run(
        runner,
        &compose_command(
            app,
            &["logs", "--tail", "200", "--no-color"],
            DIAGNOSTIC_TIMEOUT,
        )?,
        "Logs",
    )?;
    let mut logs = output.stdout;
    logs.push_str(&output.stderr);
    if logs.len() > 32_768 {
        let minimum = logs.len() - 32_768;
        let start = logs
            .char_indices()
            .find_map(|(index, _)| (index >= minimum).then_some(index))
            .unwrap_or(0);
        logs = logs[start..].to_owned();
    }
    Ok(logs)
}
/// Mutating lifecycle entry points take the app's operation slot first. The
/// `*_with` variants stay lock-free so tests can drive them directly.
pub fn start(app: &InstalledApp) -> AppResult<()> {
    let _lock = lock_operation(&app.id)?;
    start_with(&SystemProcessRunner, app)
}
pub fn stop(app: &InstalledApp) -> AppResult<()> {
    let _lock = lock_operation(&app.id)?;
    stop_with(&SystemProcessRunner, app)
}
pub fn status(app: &InstalledApp) -> AppResult<AppStatus> {
    status_with(&SystemProcessRunner, app)
}
pub fn logs(app: &InstalledApp) -> AppResult<String> {
    logs_with(&SystemProcessRunner, app)
}

pub fn wait_for_health_with(
    probe: &dyn HealthProbe,
    target: &str,
    timeout: Duration,
    cancel: &CancelToken,
) -> AppResult<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if probe.ready(target) {
            return Ok(());
        }
        // Checked between polls so a cancelled install stops waiting instead of
        // running the full timeout out.
        if cancel.is_cancelled() {
            return Err(AppError::new(
                ErrorCode::Cancelled,
                format!("Setup was cancelled while waiting for {target}"),
            ));
        }
        if Instant::now() >= deadline {
            return Err(AppError::new(
                ErrorCode::TimedOut,
                format!("Health check timed out for {target}"),
            ));
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}
pub fn wait_for_health(target: &str, timeout: Duration) -> AppResult<()> {
    wait_for_health_with(&HttpHealthProbe, target, timeout, &CancelToken::new())
}

/// Everything an install needs from the outside world, so a test can supply
/// each part without Docker, a network or a real clock.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallStage {
    CheckingSystem,
    PreparingFiles,
    ValidatingRecipe,
    StartingContainers,
    WaitingForHealth,
    SavingApp,
    RollingBack,
}

pub struct InstallContext<'a> {
    pub runner: &'a dyn ProcessRunner,
    pub health: &'a dyn HealthProbe,
    pub ports: &'a dyn PortProbe,
    pub cancel: &'a CancelToken,
    pub progress: Option<&'a (dyn Fn(InstallStage) + Send + Sync)>,
}
impl<'a> InstallContext<'a> {
    pub fn new(
        runner: &'a dyn ProcessRunner,
        health: &'a dyn HealthProbe,
        ports: &'a dyn PortProbe,
        cancel: &'a CancelToken,
    ) -> Self {
        Self {
            runner,
            health,
            ports,
            cancel,
            progress: None,
        }
    }
}

impl InstallContext<'_> {
    fn report(&self, stage: InstallStage) {
        if let Some(progress) = self.progress {
            progress(stage);
        }
    }
}

/// Stop before the next irreversible step when the operation was cancelled.
///
/// Cancellation is only honoured at these checkpoints and inside the health
/// wait. Once the registry commit begins the operation is past its cutoff and
/// runs to completion; see `install_and_commit`.
fn check_cancelled(cancel: &CancelToken) -> AppResult<()> {
    if cancel.is_cancelled() {
        return Err(AppError::new(
            ErrorCode::Cancelled,
            "Setup was cancelled before it finished.",
        ));
    }
    Ok(())
}

/// Everything the installer needs, however it was produced.
///
/// A reviewed recipe and a resolved deployment plan differ in where they come
/// from, not in what installing them requires.
#[derive(Debug, Clone)]
pub struct InstallSource {
    pub id: String,
    pub display_name: String,
    pub catalog_id: Option<String>,
    pub launch_url: String,
    pub health_url: String,
    /// Checked before anything is written, so a busy port fails early.
    pub host_port: u16,
    /// Second addresses, checked the same way.
    pub companion_ports: Vec<u16>,
    pub compose: String,
    /// Created inside the project directory before the containers start.
    pub data_directories: Vec<String>,
    /// Written beside the Compose file, and permitted to already exist when a
    /// preserved directory is reused. Generated secrets travel this way.
    pub extra_files: Vec<(String, String)>,
    /// Starting files inside `data/`, written only where nothing exists yet:
    /// a keep-data reinstall must not put back a config file somebody edited.
    pub seed_files: Vec<(String, String)>,
    /// Longer than the default when a review found the app needs it.
    pub first_start_timeout: Option<Duration>,
}

impl Recipe {
    /// Describe this reviewed recipe as something installable.
    pub fn install_source(&self) -> AppResult<InstallSource> {
        self.validate().map_err(AppError::invalid)?;
        Ok(InstallSource {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            catalog_id: crate::catalog::catalog_id(&self.catalog_name),
            launch_url: self.launch_url.clone(),
            health_url: self.health_url.clone(),
            host_port: self.host_port,
            companion_ports: Vec::new(),
            compose: self.compose.clone(),
            data_directories: self.data_directories.clone(),
            extra_files: Vec::new(),
            seed_files: Vec::new(),
            first_start_timeout: None,
        })
    }
}

/// Where a managed app keeps the secrets generated for it.
///
/// Beside the Compose file rather than in the registry: the registry lists
/// every app, and a credential belongs with the data it unlocks. Uninstalling
/// while keeping data keeps this file, which is what makes a reinstall work.
pub const SECRETS_FILE: &str = "local-store-secrets.json";

/// How far above a plan's preferred host port to look for a free one. Twenty
/// is enough to step over a handful of neighbours without wandering so far
/// that the resulting address bears no relation to what the app asked for.
const PORT_SEARCH_ATTEMPTS: u16 = 20;

/// Secrets a previous install generated, or nothing.
///
/// Only a missing file is treated as absent. Unreadable or corrupt credentials
/// must not trigger regeneration that locks preserved data behind a new secret.
fn retained_secrets(project_dir: &Path) -> AppResult<std::collections::BTreeMap<String, String>> {
    let text = match fs::read_to_string(project_dir.join(SECRETS_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(_) => return Err(AppError::invalid("Stored setup credentials could not be read. Restore access before reinstalling; existing credentials were not replaced.")),
    };
    serde_json::from_str(&text).map_err(|_| AppError::invalid("Stored setup credentials are invalid. Restore the credential file before reinstalling; existing credentials were not replaced."))
}

/// Install from a deployment plan, supplying the answers a person gave and
/// reusing or generating the secrets it declares.
pub fn install_template_with(
    context: &InstallContext<'_>,
    template: &PlanTemplate,
    display_name: &str,
    answers: &std::collections::BTreeMap<String, String>,
    root: &Path,
    now: u64,
) -> AppResult<InstalledApp> {
    let project_dir = root.join(&template.plan.id);
    // Reuse whatever a previous install generated. Minting a fresh database
    // password here would leave the app unable to open the data it is being
    // reinstalled onto, which looks like data loss to the person using it.
    let mut secrets = retained_secrets(&project_dir)?;
    for spec in &template.secrets {
        if !secrets.contains_key(&spec.key) {
            let value = spec.generate().map_err(AppError::internal)?;
            secrets.insert(spec.key.clone(), value);
        }
    }
    // A secret the template no longer declares should not linger on disk.
    secrets.retain(|key, _| template.secrets.iter().any(|spec| &spec.key == key));

    let (_, declared) = template.plan.published().ok_or_else(|| {
        AppError::new(
            ErrorCode::InvalidInput,
            "This plan publishes no address to open.",
        )
    })?;

    // A plan's host port is a preference, not a promise. An imported
    // definition that listens on a privileged port has no host port of its
    // own, and two unrelated apps may well prefer the same one.
    //
    // A reinstall is the opposite case: the app already has an address, and
    // moving it would break every link the person saved. The retained Compose
    // file records it, and it is reused even when the port is busy — the
    // likeliest occupant is a still-running copy of this very app, and
    // starting a second one beside it is worse than reporting the conflict,
    // which install_source_with does with recovery steps.
    let retained = fs::read_to_string(project_dir.join("compose.yaml")).ok();
    let host_port = match retained
        .as_deref()
        .and_then(crate::plan::published_host_port)
    {
        Some(previous) => previous,
        None => crate::plan::choose_free_port(context.ports, declared.host, PORT_SEARCH_ATTEMPTS)
            .map_err(|reason| AppError::new(ErrorCode::PortInUse, reason))?,
    };
    // The address has to be settled before anything reads it: a definition
    // that tells the app where it lives must be told the port that was
    // actually taken, not the one the plan asked for.
    let mut template = template.clone();
    template.plan.set_published_host(host_port);
    // Second addresses follow the same rules as the main one — kept on a
    // reinstall, chosen near the declared port otherwise — and must not land
    // on a port this install has already taken for something else.
    let previous = retained
        .as_deref()
        .map(crate::plan::companion_host_ports)
        .unwrap_or_default();
    let mut taken = vec![host_port];
    let wanted: Vec<(String, u16)> = template
        .plan
        .companions()
        .iter()
        .map(|(service, port)| (service.name.clone(), port.host))
        .collect();
    for (service, declared_host) in wanted {
        let port = match previous.get(&service) {
            Some(previous) => *previous,
            None => {
                let probe = ExceptTaken {
                    inner: context.ports,
                    taken: &taken,
                };
                crate::plan::choose_free_port(&probe, declared_host, PORT_SEARCH_ATTEMPTS)
                    .map_err(|reason| AppError::new(ErrorCode::PortInUse, reason))?
            }
        };
        if taken.contains(&port) {
            return Err(AppError::new(
                ErrorCode::PortInUse,
                format!("Port {port} would be published twice by this app."),
            ));
        }
        taken.push(port);
        template.plan.set_companion_host(&service, port);
    }
    crate::setup::fill_platform_values(&mut template.plan, host_port);
    let plan = template
        .resolve(answers, &secrets)
        .map_err(AppError::invalid)?;

    let compose = plan.to_compose().map_err(AppError::invalid)?;
    let address = format!("http://localhost:{host_port}");
    let extra_files = if secrets.is_empty() {
        Vec::new()
    } else {
        let encoded = serde_json::to_string_pretty(&secrets).map_err(AppError::internal)?;
        vec![(SECRETS_FILE.to_owned(), encoded)]
    };
    let source = InstallSource {
        id: plan.id.clone(),
        display_name: display_name.to_owned(),
        // An installed app gets its icon from the catalog entry its offering
        // names, the same way a recipe does. Looked up by the id the plan
        // carries rather than by the display name, so a test plan or an
        // unoffered id resolves to no icon instead of somebody else's.
        catalog_id: crate::offerings::offering(&plan.id)
            .and_then(|offering| crate::catalog::catalog_id(offering.catalog_name())),
        launch_url: address.clone(),
        health_url: address,
        host_port,
        companion_ports: taken[1..].to_vec(),
        compose,
        data_directories: plan
            .data_directories()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        extra_files,
        seed_files: template
            .seeds
            .iter()
            .map(|seed| (seed.path.clone(), seed.content.clone()))
            .collect(),
        first_start_timeout: template.first_start,
    };
    install_source_with(context, &source, root, now)
}

/// A port probe that also refuses ports this install has already chosen,
/// which nothing is listening on yet.
struct ExceptTaken<'a> {
    inner: &'a dyn PortProbe,
    taken: &'a [u16],
}
impl PortProbe for ExceptTaken<'_> {
    fn available(&self, port: u16) -> bool {
        !self.taken.contains(&port) && self.inner.available(port)
    }
}

/// Install a reviewed recipe, unchanged: recipes carry their own reviewed
/// Compose file rather than rendering one.
pub fn install_recipe_with(
    context: &InstallContext<'_>,
    recipe: &Recipe,
    root: &Path,
    now: u64,
) -> AppResult<InstalledApp> {
    install_source_with(context, &recipe.install_source()?, root, now)
}

/// Install whatever an [`InstallSource`] describes.
///
/// Recipes and resolved plans both reduce to a source, so there is one install
/// path with one rollback, one confinement check and one cancellation story
/// rather than a second copy that drifts from it.
pub fn install_source_with(
    context: &InstallContext<'_>,
    source: &InstallSource,
    root: &Path,
    now: u64,
) -> AppResult<InstalledApp> {
    context.report(InstallStage::CheckingSystem);
    check_cancelled(context.cancel)?;
    let project_dir = root.join(&source.id);
    let binding = match engine::retained(&project_dir)? {
        Some(binding) => Some(binding),
        None => context.runner.engine_binding()?,
    };
    let selected = engine::EngineRunner {
        inner: context.runner,
        binding,
    };
    let runner: &dyn ProcessRunner = &selected;
    check_cancelled(context.cancel)?;
    let report = doctor_with(runner);
    if !report.ready {
        return Err(AppError::new(
            ErrorCode::PrerequisiteUnavailable,
            "Docker and Docker Compose must be running before installation.",
        ));
    }
    // Checked before anything is written: a port already in use would otherwise
    // surface as an opaque Compose failure after the files exist.
    let busy = std::iter::once(source.host_port)
        .chain(source.companion_ports.iter().copied())
        .find(|port| !context.ports.available(*port));
    if let Some(busy) = busy {
        let retained = root.join(&source.id).join("compose.yaml");
        let recovery = if retained.is_file() {
            format!(" Setup files remain at {}. If an earlier setup was interrupted, stop that retained Compose project without deleting its data, then retry.", retained.display())
        } else {
            String::new()
        };
        return Err(AppError::new(
            ErrorCode::PortInUse,
            format!(
                "Port {busy} is already in use. Stop whatever is using it and try again.{recovery}"
            ),
        ));
    }
    let project_dir = root.join(&source.id);
    let created_project = !project_dir.exists();
    if !created_project {
        // Compared against the first segment of each declared directory, not
        // the whole relative path. An app that keeps its data in `data/.ollama`
        // puts a `data` directory here, and matching on the full path made the
        // listing and the list disagree — so every keep-data reinstall of any
        // app with a nested data path was refused as unrecognised.
        let allowed = source
            .data_directories
            .iter()
            .filter_map(|directory| {
                directory
                    .split(['/', '\\'])
                    .find(|segment| !segment.is_empty())
            })
            .chain(std::iter::once("compose.yaml"))
            .chain(std::iter::once(engine::ENGINE_FILE))
            .chain(source.extra_files.iter().map(|(name, _)| name.as_str()))
            .collect::<Vec<_>>();
        for entry in fs::read_dir(&project_dir).map_err(AppError::from)? {
            let entry = entry.map_err(AppError::from)?;
            let name = entry.file_name();
            if !allowed.iter().any(|allowed| name == *allowed) {
                return Err(AppError::new(ErrorCode::UnsafePath, "The preserved app directory contains unexpected files; review it before reinstalling."));
            }
        }
    }
    context.report(InstallStage::PreparingFiles);
    // The last checkpoint before anything is written to disk.
    check_cancelled(context.cancel)?;
    fs::create_dir_all(&project_dir).map_err(AppError::from)?;
    let compose_file = project_dir.join("compose.yaml");
    let previous_compose = fs::read(&compose_file).ok();
    let install: AppResult<InstalledApp> = (|| {
        if let Some(binding) = &selected.binding {
            engine::save(&project_dir, binding)?;
        }
        // Atomic: Docker must never read a half-written Compose file.
        let compose = if selected.binding.is_some() {
            format!("{}{}", engine::COMPOSE_BINDING_MARKER, source.compose)
        } else {
            source.compose.clone()
        };
        storage::write_file_atomically(&compose_file, compose.as_bytes())
            .map_err(AppError::from)?;
        for (name, contents) in &source.extra_files {
            storage::write_file_atomically(&project_dir.join(name), contents.as_bytes())
                .map_err(AppError::from)?;
        }
        for (path, contents) in &source.seed_files {
            // Checked again here, not only when the template was validated:
            // this is the line that actually writes to disk.
            if !crate::setup::is_confined_seed_path(path) {
                return Err(AppError::new(
                    ErrorCode::UnsafePath,
                    format!("seed file {path:?} is not inside the app's data folder"),
                ));
            }
            let target = project_dir.join(path);
            if target.exists() {
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(AppError::from)?;
            }
            storage::write_file_atomically(&target, contents.as_bytes()).map_err(AppError::from)?;
        }
        for directory in &source.data_directories {
            // A mount of a seeded file is a file, not a folder to create:
            // making a directory there is exactly how an nginx config came to
            // be an empty folder and the proxy refused to start.
            if source.seed_files.iter().any(|(path, _)| path == directory) {
                continue;
            }
            fs::create_dir_all(project_dir.join(directory)).map_err(AppError::from)?;
        }
        let app = InstalledApp {
            id: source.id.clone(),
            catalog_id: source.catalog_id.clone(),
            display_name: source.display_name.clone(),
            launch_url: source.launch_url.clone(),
            icon_path: None,
            runtime: RuntimeSpec::Compose {
                project_name: format!("local-store-{}", source.id),
                project_dir: project_dir.clone(),
                compose_file: compose_file.clone(),
            },
            created_at_unix: now,
            updated_at_unix: now,
        };
        context.report(InstallStage::ValidatingRecipe);
        check_cancelled(context.cancel)?;
        checked_run_cancellable(
            runner,
            &compose_command(&app, &["config", "--quiet"], DIAGNOSTIC_TIMEOUT)?,
            "Recipe validation",
            context.cancel,
        )?;
        context.report(InstallStage::StartingContainers);
        check_cancelled(context.cancel)?;
        start_with_cancel(runner, &app, context.cancel)?;
        context.report(InstallStage::WaitingForHealth);
        wait_for_health_with(
            context.health,
            &source.health_url,
            source.first_start_timeout.unwrap_or(FIRST_START_TIMEOUT),
            context.cancel,
        )?;
        Ok(app)
    })();
    if let Err(failure) = &install {
        let original = failure.clone();
        context.report(InstallStage::RollingBack);
        let fallback = InstalledApp {
            id: source.id.clone(),
            catalog_id: None,
            display_name: source.display_name.clone(),
            launch_url: source.launch_url.clone(),
            icon_path: None,
            runtime: RuntimeSpec::Compose {
                project_name: format!("local-store-{}", source.id),
                project_dir: project_dir.clone(),
                compose_file: compose_file.clone(),
            },
            created_at_unix: now,
            updated_at_unix: now,
        };
        // Every other failure says what went wrong: Compose reports a bad
        // file, a pull reports a missing image. A timeout reports only that
        // time passed, and the reason is inside containers the cleanup below
        // is about to remove — so that is the one worth asking about.
        let last_words = (original.code == ErrorCode::TimedOut)
            .then(|| last_words(runner, &fallback))
            .flatten();
        // Attached before cleanup runs, because a cleanup that fails returns
        // early — and a failed cleanup is exactly when somebody needs to know
        // what the containers were doing.
        let original = match last_words {
            Some(said) => AppError::new(original.code, format!("{original} {said}")),
            None => original,
        };
        // Keep the Compose file and data if Docker cleanup fails: they may
        // still be needed by running containers and for manual recovery.
        // `checked_run` deliberately uses a fresh token: a cancelled install
        // must still be cleaned up, and reusing the cancelled one would abort
        // the cleanup immediately.
        checked_run(
            runner,
            &compose_command(&fallback, &["down"], LIFECYCLE_TIMEOUT)?,
            "Install cleanup",
        )
        .map_err(|cleanup| AppError::rollback(&original, cleanup))?;
        let cleanup = if created_project {
            fs::remove_dir_all(&project_dir).map_err(AppError::from)
        } else if let Some(previous) = previous_compose {
            storage::write_file_atomically(&compose_file, &previous).map_err(AppError::from)
        } else {
            match fs::remove_file(&compose_file) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(AppError::from(error)),
            }
        };
        cleanup.map_err(|cleanup| AppError::rollback(&original, cleanup))?;
        return Err(original);
    }
    install
}

/// Colour codes out of a log, so a diagnosis reads as text: Langflow's
/// last words arrived with `ESC[31m` and `ESC[0m` around every word.
fn strip_terminal_colours(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            // A control sequence ends at its first letter.
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// What the containers said before a failed install removed them.
///
/// Compose keeps the two halves of the answer apart: `ps` knows which service
/// stopped and how, and the log holds the reason it gives. Neither survives
/// the cleanup, so both are read while they still exist. Best effort by
/// design — a diagnosis that fails must not replace the failure it explains.
fn last_words(runner: &dyn ProcessRunner, app: &InstalledApp) -> Option<String> {
    let mut parts = Vec::new();
    if let Ok(command) = compose_command(app, &["ps", "--all"], DIAGNOSTIC_TIMEOUT) {
        if let Ok(output) = runner.run(&command) {
            let states: Vec<&str> = output
                .stdout
                .lines()
                .skip(1) // the header
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect();
            if !states.is_empty() {
                parts.push(format!("Containers: {}.", states.join("; ")));
            }
        }
    }
    if let Ok(logs) = logs_with(runner, app) {
        // Per container, not overall: Compose interleaves every service's
        // output, and a chatty database drowned out Nextcloud's own last words
        // entirely.
        let mut by_service: Vec<(String, Vec<&str>)> = Vec::new();
        let plain = strip_terminal_colours(&logs);
        for line in plain.lines().map(str::trim).filter(|line| !line.is_empty()) {
            // `name | text`, or `name |` alone for a blank line — Postgres
            // prints several, and they are not the service's last words.
            let (service, said) = match line.split_once(" | ") {
                Some((prefix, rest)) => (prefix.trim().to_owned(), rest.trim()),
                None => match line.strip_suffix(" |").or_else(|| line.strip_suffix('|')) {
                    Some(prefix) => (prefix.trim().to_owned(), ""),
                    None => (String::new(), line),
                },
            };
            if said.is_empty() {
                continue;
            }
            match by_service.iter_mut().find(|(name, _)| *name == service) {
                Some((_, lines)) => lines.push(said),
                None => by_service.push((service, vec![said])),
            }
        }
        // The app's own service first — it is almost always the shortest
        // name (`nextcloud-mini` before `nextcloud-mini-db`) — so a cap never
        // cuts the lines that matter most.
        by_service.sort_by_key(|(service, _)| service.len());
        for (service, lines) in &by_service {
            let tail: Vec<String> = lines[lines.len().saturating_sub(5)..]
                .iter()
                .map(|line| match line.char_indices().nth(200) {
                    Some((cut, _)) => format!("{}…", &line[..cut]),
                    None => (*line).to_owned(),
                })
                .collect();
            // Container names are `<project>-<service>`; the service is the
            // part a person recognises.
            let name = match &app.runtime {
                RuntimeSpec::Compose { project_name, .. } => service
                    .strip_prefix(project_name.as_str())
                    .map(|rest| rest.trim_start_matches('-'))
                    .filter(|rest| !rest.is_empty())
                    .unwrap_or(service),
                _ => service,
            };
            parts.push(format!("{name} last said: {}", tail.join(" | ")));
        }
    }
    if parts.is_empty() {
        return None;
    }
    let mut said = parts.join(" ");
    // Room for a few lines from each container, short enough that the
    // failure it explains is still the first thing read.
    if said.chars().count() > 2000 {
        let cut = said
            .char_indices()
            .nth(2000)
            .map(|(index, _)| index)
            .unwrap_or(said.len());
        said.truncate(cut);
        said.push('…');
    }
    Some(said)
}
/// An install whose containers are up and healthy but which is not yet in the
/// registry.
///
/// It keeps holding the app's operation lock, so nothing else can act on the
/// app in the window between the install finishing and the commit landing.
#[must_use = "an uncommitted install leaves containers running with no registry entry"]
pub struct PendingInstall {
    app: InstalledApp,
    preserved_data: bool,
    lock: OperationLock,
}
impl PendingInstall {
    pub fn app(&self) -> &InstalledApp {
        &self.app
    }
    /// Identifies the operation, for a cancel request that races with it.
    pub fn operation_id(&self) -> u64 {
        self.lock.id()
    }
    /// Write the app to the registry.
    ///
    /// **This is the cancellation cutoff.** The token is deliberately not
    /// consulted here: past this point the containers are already running, and
    /// honouring a late cancel would leave them up with no registry entry —
    /// invisible to the user and unmanageable from the app. A commit failure
    /// rolls back on a fresh token, because a cancelled one would abort the
    /// cleanup immediately, and never deletes data that predates the install.
    pub fn commit(
        self,
        progress: &(dyn Fn(InstallStage) + Send + Sync),
    ) -> AppResult<InstalledApp> {
        progress(InstallStage::SavingApp);
        match storage::insert_installed_app(self.app.clone()) {
            Ok(()) => Ok(self.app),
            Err(error) => {
                let error = AppError::from(error);
                progress(InstallStage::RollingBack);
                // The lock-free variant: this transaction already holds the lock.
                rollback_install_with(&SystemProcessRunner, &self.app, self.preserved_data)
                    .map_err(|cleanup| AppError::rollback(&error, cleanup))?;
                Err(error)
            }
        }
    }
}

/// Install a recipe and hold its operation lock open for the caller to commit.
/// The caller acquires the lock first, so it can report the operation id — and
/// a busy app — before any worker thread is spawned.
pub fn begin_install(
    recipe: &Recipe,
    lock: OperationLock,
    progress: &(dyn Fn(InstallStage) + Send + Sync),
) -> AppResult<PendingInstall> {
    begin_pending_install(&recipe.id, lock, progress, |context, root, now| {
        install_recipe_with(context, recipe, root, now)
    })
}

/// Resolve a plan under the same lock that remains held through registry commit
/// and rollback. Callers must acquire the matching app lock before spawning work.
pub fn begin_template_install(
    template: &PlanTemplate,
    display_name: &str,
    answers: &std::collections::BTreeMap<String, String>,
    lock: OperationLock,
    progress: &(dyn Fn(InstallStage) + Send + Sync),
) -> AppResult<PendingInstall> {
    begin_pending_install(&template.plan.id, lock, progress, |context, root, now| {
        install_template_with(context, template, display_name, answers, root, now)
    })
}

fn begin_pending_install(
    app_id: &str,
    lock: OperationLock,
    progress: &(dyn Fn(InstallStage) + Send + Sync),
    install: impl FnOnce(&InstallContext<'_>, &Path, u64) -> AppResult<InstalledApp>,
) -> AppResult<PendingInstall> {
    begin_pending_install_on_engine(app_id, lock, progress, None, install)
}

fn begin_pending_install_on_engine(
    app_id: &str,
    lock: OperationLock,
    progress: &(dyn Fn(InstallStage) + Send + Sync),
    binding: Option<&engine::EngineBinding>,
    install: impl FnOnce(&InstallContext<'_>, &Path, u64) -> AppResult<InstalledApp>,
) -> AppResult<PendingInstall> {
    if lock.app_id != app_id {
        return Err(AppError::invalid(
            "The operation lock does not match the app being installed.",
        ));
    }
    let cancel = lock.token();
    // Recorded before the install, which deliberately reuses a project
    // directory left behind by an uninstall that kept its data.
    let preserved_data = managed_project_exists(app_id);
    let selected = engine::EngineRunner {
        inner: &SystemProcessRunner,
        binding: binding.cloned(),
    };
    let runner: &dyn ProcessRunner = if binding.is_some() {
        &selected
    } else {
        &SystemProcessRunner
    };
    let mut context = InstallContext::new(runner, &HttpHealthProbe, &LocalPortProbe, &cancel);
    context.progress = Some(progress);
    let app = install(
        &context,
        &storage::managed_apps_root(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(AppError::internal)?
            .as_secs(),
    )?;
    Ok(PendingInstall {
        app,
        preserved_data,
        lock,
    })
}

/// Plan counterpart of install_recipe; shares PendingInstall's commit/rollback.
pub fn install_template(
    template: &PlanTemplate,
    display_name: &str,
    answers: &std::collections::BTreeMap<String, String>,
) -> AppResult<InstalledApp> {
    let lock = lock_operation(&template.plan.id)?;
    begin_template_install(template, display_name, answers, lock, &|_| {})?.commit(&|_| {})
}

/// Install and commit in one step, for callers with no progress reporting.
pub fn install_recipe(recipe: &Recipe) -> AppResult<InstalledApp> {
    let lock = lock_operation(&recipe.id)?;
    begin_install(recipe, lock, &|_| {})?.commit(&|_| {})
}

pub(crate) fn install_template_on_engine(
    template: &PlanTemplate,
    display_name: &str,
    answers: &std::collections::BTreeMap<String, String>,
    binding: &engine::EngineBinding,
) -> AppResult<InstalledApp> {
    let lock = lock_operation(&template.plan.id)?;
    begin_pending_install_on_engine(
        &template.plan.id,
        lock,
        &|_| {},
        Some(binding),
        |context, root, now| {
            install_template_with(context, template, display_name, answers, root, now)
        },
    )?
    .commit(&|_| {})
}

pub fn uninstall_with(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    delete_data: bool,
) -> AppResult<()> {
    let args = if delete_data {
        &["down", "--volumes"][..]
    } else {
        &["down"][..]
    };
    checked_run(
        runner,
        &compose_command(app, args, LIFECYCLE_TIMEOUT)?,
        "Uninstall",
    )?;
    if delete_data {
        if let RuntimeSpec::Compose { project_dir, .. } = &app.runtime {
            fs::remove_dir_all(project_dir).map_err(AppError::from)?;
        }
    }
    Ok(())
}
/// Undo an install whose later commit failed.
///
/// `preserved_data` records whether the project directory already existed
/// before the install ran. When it did, the containers are removed but the
/// directory is kept: an app uninstalled with "keep data" and then reinstalled
/// must not lose that data to a failure after the containers came up.
pub fn rollback_install_with(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    preserved_data: bool,
) -> AppResult<()> {
    uninstall_with(runner, app, !preserved_data)
}
pub fn rollback_install(app: &InstalledApp, preserved_data: bool) -> AppResult<()> {
    uninstall(app, !preserved_data)
}

/// Whether a managed project directory already exists for `recipe_id`.
///
/// A caller that has to undo a completed install uses this to decide whether
/// deleting the project directory is safe. Data that predates the install —
/// an app uninstalled with "keep data" and then reinstalled — must survive.
pub fn managed_project_exists(recipe_id: &str) -> bool {
    storage::managed_apps_root().join(recipe_id).exists()
}
/// Refuse to delete anything that is not this app's own managed directory.
///
/// Deletion is recursive and permanent, so the path is checked twice: it must
/// be exactly `<root>/<app id>` as recorded, and its *resolved* location must
/// sit directly inside the resolved managed root. The second check is what
/// stops a link or a `..` in the stored path from redirecting the delete
/// somewhere else entirely.
pub(crate) fn confined_to_managed_root(
    project_dir: &Path,
    app_id: &str,
    root: &Path,
) -> AppResult<()> {
    let refuse = || {
        AppError::new(
            ErrorCode::UnsafePath,
            "Data deletion is available only for directories created by Local Store.",
        )
    };
    if project_dir != root.join(app_id) {
        return Err(refuse());
    }
    let canonical_root = root.canonicalize().map_err(AppError::from)?;
    let canonical_project = project_dir.canonicalize().map_err(AppError::from)?;
    if canonical_project.parent() != Some(canonical_root.as_path())
        || canonical_project.file_name() != Some(std::ffi::OsStr::new(app_id))
    {
        return Err(refuse());
    }
    Ok(())
}

pub fn uninstall(app: &InstalledApp, delete_data: bool) -> AppResult<()> {
    let _lock = lock_operation(&app.id)?;
    uninstall_locked(app, delete_data)
}

/// Keep the operation lock until registry removal completes, so a reinstall
/// cannot commit between Docker cleanup and removal of the old registry entry.
pub fn uninstall_and_remove(app: &InstalledApp, delete_data: bool) -> AppResult<()> {
    let _lock = lock_operation(&app.id)?;
    uninstall_locked(app, delete_data)?;
    storage::remove_installed_app(&app.id)
        .map(|_| ())
        .map_err(AppError::from)
}

fn uninstall_locked(app: &InstalledApp, delete_data: bool) -> AppResult<()> {
    if delete_data {
        let RuntimeSpec::Compose { project_dir, .. } = &app.runtime else {
            return Err(AppError::new(
                ErrorCode::UnsupportedOperation,
                "Connected apps do not have Local Store managed data.",
            ));
        };
        confined_to_managed_root(project_dir, &app.id, &storage::managed_apps_root())?;
    }
    uninstall_with(&SystemProcessRunner, app, delete_data)
}

/// Compatibility path for one release while old CLI registrations are imported.
pub fn boot_and_wait(app: &crate::model::AppDef) -> AppResult<()> {
    if let Some(compose) = &app.compose {
        checked_run(
            &SystemProcessRunner,
            &CommandSpec::docker(
                vec![
                    "compose".into(),
                    "-f".into(),
                    compose.clone(),
                    "up".into(),
                    "-d".into(),
                ],
                None,
                PROVISION_TIMEOUT,
            ),
            "Start",
        )?;
    }
    wait_for_health(
        app.health.as_deref().unwrap_or(&app.url),
        Duration::from_secs(60),
    )
}
/// Hand a URL to the operating system's default browser.
///
/// Returns the failure instead of printing it: an app window has no console,
/// so a link that silently does nothing is indistinguishable from a broken
/// page. The caller reports it where the user can see it.
pub fn launch_browser(url: &str) -> AppResult<()> {
    let url = crate::windowing::validated_external_url(url).map_err(AppError::invalid)?;
    open::that_detached(url).map_err(|error| {
        AppError::new(
            ErrorCode::BrowserOpenFailed,
            format!("Could not open your browser: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, path::PathBuf, sync::Mutex};
    struct FakeRunner {
        outputs: Mutex<VecDeque<Result<ProcessOutput, ProcessError>>>,
        calls: Mutex<Vec<CommandSpec>>,
    }
    impl FakeRunner {
        fn passing(count: usize) -> Self {
            Self {
                outputs: Mutex::new(
                    (0..count)
                        .map(|_| {
                            Ok(ProcessOutput {
                                success: true,
                                stdout: "1".into(),
                                stderr: String::new(),
                                truncated: false,
                            })
                        })
                        .collect(),
                ),
                calls: Mutex::new(Vec::new()),
            }
        }
    }
    impl ProcessRunner for FakeRunner {
        fn run_cancellable(
            &self,
            spec: &CommandSpec,
            _cancel: &CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            self.calls.lock().unwrap().push(spec.clone());
            self.outputs.lock().unwrap().pop_front().unwrap_or_else(|| {
                Err(ProcessError::new(
                    ProcessErrorCode::ProcessFailed,
                    "unexpected command",
                ))
            })
        }
    }
    struct Ready(bool);
    #[test]
    fn install_and_lifecycle_use_the_saved_alternate_engine() {
        let root = std::env::temp_dir().join(format!(
            "engine-install-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let fake = FakeRunner::passing(20);
        let binding = engine::EngineBinding {
            schema_version: 1,
            program: "alternate-docker".into(),
            endpoint: "unix:///owned.sock".into(),
        };
        let selected = engine::EngineRunner {
            inner: &fake,
            binding: Some(binding.clone()),
        };
        let template = crate::offerings::offering("memos")
            .unwrap()
            .plan_template(None)
            .unwrap();
        let app = install_template_with(
            &context(&selected, &Ready(true), &Ports(true), &CancelToken::new()),
            &template,
            "Memos",
            &Default::default(),
            &root,
            1,
        )
        .unwrap();
        assert_eq!(
            engine::retained(&root.join("memos")).unwrap(),
            Some(binding)
        );
        stop_with(&fake, &app).unwrap();
        start_with(&fake, &app).unwrap();
        logs_with(&fake, &app).unwrap();
        let calls = fake.calls.lock().unwrap();
        assert!(calls.len() >= 7);
        assert!(calls.iter().all(|call| call.program == "alternate-docker"
            && call.args[..2] == ["--host", "unix:///owned.sock"]));
        let count = calls.len();
        drop(calls);
        fs::remove_file(root.join("memos").join(engine::ENGINE_FILE)).unwrap();
        assert!(start_with(&fake, &app).is_err());
        assert_eq!(fake.calls.lock().unwrap().len(), count);
        fs::remove_dir_all(root).unwrap();
    }
    impl HealthProbe for Ready {
        fn ready(&self, _: &str) -> bool {
            self.0
        }
    }
    struct Ports(bool);
    impl PortProbe for Ports {
        fn available(&self, _: u16) -> bool {
            self.0
        }
    }
    /// Everything is free except the ports named here.
    struct BusyPorts(Vec<u16>);
    impl PortProbe for BusyPorts {
        fn available(&self, port: u16) -> bool {
            !self.0.contains(&port)
        }
    }
    fn context_with_ports<'a>(
        runner: &'a dyn ProcessRunner,
        health: &'a Ready,
        ports: &'a dyn PortProbe,
        cancel: &'a CancelToken,
    ) -> InstallContext<'a> {
        InstallContext::new(runner, health, ports, cancel)
    }
    /// The usual install surroundings: Docker answers, the port is free and
    /// nothing has been cancelled.
    fn context<'a>(
        runner: &'a dyn ProcessRunner,
        health: &'a Ready,
        ports: &'a Ports,
        cancel: &'a CancelToken,
    ) -> InstallContext<'a> {
        InstallContext::new(runner, health, ports, cancel)
    }
    fn managed(root: &Path) -> InstalledApp {
        InstalledApp {
            id: "memos".into(),
            catalog_id: Some("Memos".into()),
            display_name: "Memos".into(),
            launch_url: "http://localhost:5230".into(),
            icon_path: None,
            runtime: RuntimeSpec::Compose {
                project_name: "local-store-memos".into(),
                project_dir: root.into(),
                compose_file: root.join("compose.yaml"),
            },
            created_at_unix: 1,
            updated_at_unix: 1,
        }
    }
    #[test]
    fn a_long_failure_keeps_the_reason_at_its_end() {
        let progress = "Container app Creating\n".repeat(80);
        let output = ProcessOutput {
            success: false,
            stdout: String::new(),
            stderr: format!("{progress}Error response from daemon: port is already allocated"),
            truncated: false,
        };
        let detail = concise_error(&output);
        assert!(detail.ends_with("port is already allocated"), "{detail}");
        assert!(
            detail.starts_with('…'),
            "a cut message should say it was cut"
        );
        assert!(detail.chars().count() <= 1001);
    }

    /// A timeout is the one failure that explains nothing by itself, and the
    /// containers holding the explanation are removed moments later. Five
    /// candidates in a row failed with nothing but "Health check timed out",
    /// which is why this exists.
    #[test]
    fn a_timed_out_install_says_what_the_containers_said() {
        let root =
            std::env::temp_dir().join(format!("local-store-lastwords-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner {
            outputs: Mutex::new(VecDeque::from([
                Ok(ProcessOutput {
                    success: true,
                    stdout: "NAME       STATUS\nglance-1   Exited (1)\n".into(),
                    stderr: String::new(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: true,
                    stdout: String::new(),
                    stderr: "glance-1 | failed to read config: no such file\n".into(),
                    truncated: false,
                }),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        let said = last_words(&runner, &managed(&root)).expect("both halves answered");
        assert!(said.contains("Exited (1)"), "no container state: {said}");
        assert!(said.contains("no such file"), "no log line: {said}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn colour_codes_are_stripped_from_a_diagnosis() {
        let coloured = "\u{1b}[31mApplication \u{1b}[1;34mstartup\u{1b}[0m failed";
        assert_eq!(
            strip_terminal_colours(coloured),
            "Application startup failed"
        );
    }

    /// Best effort means best effort: a diagnosis that cannot be gathered must
    /// leave the failure it was meant to explain exactly as it was.
    #[test]
    fn a_diagnosis_that_cannot_be_gathered_reports_nothing() {
        let root = std::env::temp_dir().join(format!("local-store-nowords-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner {
            outputs: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        };
        assert_eq!(last_words(&runner, &managed(&root)), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unsafe_link_is_refused_before_any_browser_is_launched() {
        // Every rejection here is decided by validation alone, so no browser is
        // ever started by this test.
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<script>",
            "https://user:secret@example.com",
            "http://exa mple.com",
            "not-a-url",
        ] {
            let error = launch_browser(url).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidInput, "{url} was allowed");
        }
    }

    #[test]
    fn readiness_reports_unknown_rather_than_guessing_about_https() {
        let mut app = managed(Path::new("apps/memos"));

        app.launch_url = "http://localhost:5230".into();
        assert_eq!(readiness_with(&Ready(true), &app), Readiness::Ready);
        assert_eq!(readiness_with(&Ready(false), &app), Readiness::Unreachable);

        // The probe speaks plain HTTP only. Calling a reachable HTTPS app
        // "not responding" would be worse than admitting we did not check.
        app.launch_url = "https://apps.example.com".into();
        assert_eq!(readiness_with(&Ready(false), &app), Readiness::Unknown);
        assert_eq!(readiness_with(&Ready(true), &app), Readiness::Unknown);
    }

    #[test]
    fn an_unsaved_address_is_checked_on_the_same_terms_as_a_saved_one() {
        assert_eq!(
            address_readiness_with(&Ready(true), "http://localhost:5230"),
            Readiness::Ready
        );
        assert_eq!(
            address_readiness_with(&Ready(false), "http://localhost:5230"),
            Readiness::Unreachable
        );
        // Same reasoning as a saved app: an unchecked HTTPS address must not be
        // reported as unreachable, which would read as "your address is wrong".
        assert_eq!(
            address_readiness_with(&Ready(false), "https://apps.example.com"),
            Readiness::Unknown
        );
    }

    #[test]
    fn doctor_reports_both_prerequisites() {
        let runner = FakeRunner::passing(2);
        let report = doctor_with(&runner);
        assert!(report.ready);
        assert_eq!(report.checks.len(), 2);
    }
    #[test]
    fn doctor_details_never_repeat_a_credential_docker_printed() {
        let runner = FakeRunner {
            outputs: Mutex::new(VecDeque::from([
                Ok(ProcessOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "error during connect: Get \"https://admin:hunter2@docker.example:2376/v1.47/version\": forbidden".into(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: true,
                    stdout: "2.29.0".into(),
                    stderr: String::new(),
                    truncated: false,
                }),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        let report = doctor_with(&runner);
        assert!(!report.ready);
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("hunter2"));
        assert_eq!(
            report.checks[0].error.as_ref().unwrap().code,
            ProcessErrorCode::ProcessFailed
        );
        let detail = &report.checks[0].detail;
        assert!(!detail.contains("hunter2"), "credential survived: {detail}");
        assert!(detail.contains("***@docker.example"), "{detail}");
        // The rest of the message has to stay useful.
        assert!(detail.contains("forbidden"), "{detail}");
    }

    #[test]
    fn lifecycle_commands_are_bounded_and_project_scoped() {
        let root = PathBuf::from("apps/memos");
        let runner = FakeRunner::passing(4);
        let app = managed(&root);
        start_with(&runner, &app).unwrap();
        stop_with(&runner, &app).unwrap();
        logs_with(&runner, &app).unwrap();
        assert_eq!(status_with(&runner, &app).unwrap(), AppStatus::Running);
        let calls = runner.calls.lock().unwrap();
        assert!(calls
            .iter()
            .all(|call| call.cwd.as_deref() == Some(root.as_path())));
        assert!(calls[2]
            .args
            .ends_with(&["logs", "--tail", "200", "--no-color"].map(str::to_owned)));
    }
    #[test]
    fn failed_install_rolls_back_created_directory() {
        let root =
            std::env::temp_dir().join(format!("local-store-install-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner {
            outputs: Mutex::new(VecDeque::from([
                Ok(ProcessOutput {
                    success: true,
                    stdout: "1".into(),
                    stderr: String::new(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: true,
                    stdout: "1".into(),
                    stderr: String::new(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "invalid".into(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                }),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        assert!(install_recipe_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &crate::recipes::recipe("memos").unwrap(),
            &root,
            5
        )
        .is_err());
        assert!(!root.join("memos").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn successful_install_supports_every_reviewed_recipe() {
        let root =
            std::env::temp_dir().join(format!("local-store-install-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for recipe in crate::recipes::reviewed_recipes() {
            let runner = FakeRunner::passing(4);
            let app = install_recipe_with(
                &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
                &recipe,
                &root,
                5,
            )
            .unwrap();
            for directory in &recipe.data_directories {
                assert!(root.join(&recipe.id).join(directory).is_dir());
            }
            assert_eq!(app.created_at_unix, 5);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_second_operation_on_one_app_is_refused_while_the_first_holds_it() {
        let held = lock_operation("memos").unwrap();
        let refused = lock_operation("memos").unwrap_err();
        assert_eq!(refused.code, ErrorCode::OperationBusy);
        assert!(
            refused.to_string().contains("Another operation"),
            "{refused}"
        );
        // A different app is unaffected.
        let other = lock_operation("n8n").unwrap();
        drop(other);
        // The slot is returned when the guard goes out of scope.
        drop(held);
        let again = lock_operation("memos");
        assert!(again.is_ok(), "the slot was never released");
    }

    #[test]
    fn a_stale_cancel_cannot_stop_the_operation_that_replaced_it() {
        let first = lock_operation("cancel-successor").unwrap();
        let stale_id = first.id();
        assert!(cancel_operation("cancel-successor", stale_id));
        assert!(first.token().is_cancelled());
        drop(first);

        // The successor is a different operation and must be untouched by a
        // cancel aimed at the one before it.
        let second = lock_operation("cancel-successor").unwrap();
        assert_ne!(second.id(), stale_id, "operation ids must not repeat");
        assert!(!cancel_operation("cancel-successor", stale_id));
        assert!(
            !second.token().is_cancelled(),
            "a stale cancel reached the successor"
        );
        assert!(cancel_operation("cancel-successor", second.id()));
        assert!(second.token().is_cancelled());
    }

    #[test]
    fn cancelling_an_app_with_no_operation_running_does_nothing() {
        assert!(!cancel_operation("nothing-running", 1));
    }

    #[test]
    fn cancelling_before_the_compose_work_leaves_no_project_directory() {
        let root =
            std::env::temp_dir().join(format!("local-store-cancel-early-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        // Only the two Doctor probes may run: the checkpoint after them stops
        // the install before any file is written.
        let runner = FakeRunner::passing(2);
        let error = install_recipe_with(
            &context(&runner, &Ready(true), &Ports(true), &cancel),
            &crate::recipes::recipe("memos").unwrap(),
            &root,
            5,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(
            !root.join("memos").exists(),
            "a cancelled install wrote a project directory"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_still_runs_when_the_operation_was_cancelled() {
        let root = std::env::temp_dir().join(format!(
            "local-store-cancel-rollback-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cancel = CancelToken::new();
        // Doctor twice, config, up, then the health wait observes the cancel.
        // The fifth response is the rollback `down`, which must still be
        // attempted on a fresh token rather than aborted by the cancelled one.
        let runner = FakeRunner::passing(5);
        let recipe = crate::recipes::recipe("memos").unwrap();
        let health = Ready(false);
        let ports = Ports(true);
        let mut ctx = InstallContext::new(&runner, &health, &ports, &cancel);
        let cancel_at_health = |stage: InstallStage| {
            if matches!(stage, InstallStage::WaitingForHealth) {
                cancel.cancel();
            }
        };
        ctx.progress = Some(&cancel_at_health);
        let error = install_recipe_with(&ctx, &recipe, &root, 5).unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        let calls = runner.calls.lock().unwrap();
        assert!(
            calls.last().unwrap().args.ends_with(&["down".to_owned()]),
            "cleanup did not run after cancellation: {calls:?}"
        );
        drop(calls);
        assert!(!root.join("memos").exists(), "cancelled install left files");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_operation_slot_is_released_even_when_the_operation_panics() {
        let panicked = std::panic::catch_unwind(|| {
            let _lock = lock_operation("uptime-kuma").unwrap();
            panic!("operation failed");
        });
        assert!(panicked.is_err());
        assert!(
            lock_operation("uptime-kuma").is_ok(),
            "a panic leaked the operation slot"
        );
    }

    #[test]
    fn an_occupied_port_stops_the_install_before_anything_is_written() {
        let root = std::env::temp_dir().join(format!("local-store-port-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner::passing(2);
        let error = install_recipe_with(
            &context(&runner, &Ready(true), &Ports(false), &CancelToken::new()),
            &crate::recipes::recipe("memos").unwrap(),
            &root,
            5,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::PortInUse);
        assert!(error.to_string().contains("already in use"), "{error}");
        assert!(
            !root.join("memos").exists(),
            "a refused install must leave no project directory"
        );
        // Only the two Doctor probes ran; nothing was started.
        assert_eq!(runner.calls.lock().unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelling_stops_the_health_wait_instead_of_running_out_the_timeout() {
        let cancel = CancelToken::new();
        cancel.cancel();
        let started = Instant::now();
        let error = wait_for_health_with(
            &Ready(false),
            "http://127.0.0.1:1",
            Duration::from_secs(600),
            &cancel,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(error.to_string().contains("cancelled"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn a_cancelled_install_rolls_back_and_keeps_nothing_behind() {
        let root = std::env::temp_dir().join(format!("local-store-cancel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        // Doctor, config and start succeed; the health wait is cancelled.
        let runner = FakeRunner::passing(5);
        let error = install_recipe_with(
            &context(&runner, &Ready(false), &Ports(true), &cancel),
            &crate::recipes::recipe("memos").unwrap(),
            &root,
            5,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(error.to_string().contains("cancelled"), "{error}");
        assert!(
            !root.join("memos").exists(),
            "rollback left the project behind"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_compose_file_is_written_atomically_and_leaves_no_temporary_behind() {
        let root = std::env::temp_dir().join(format!("local-store-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner::passing(4);
        let recipe = crate::recipes::recipe("memos").unwrap();
        install_recipe_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &recipe,
            &root,
            5,
        )
        .unwrap();
        let project = root.join("memos");
        assert_eq!(
            fs::read_to_string(project.join("compose.yaml")).unwrap(),
            recipe.compose
        );
        let leftovers: Vec<_> = fs::read_dir(&project)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temporary files remained: {leftovers:?}"
        );
        fs::remove_dir_all(root).unwrap();
    }

    /// A scratch managed root that is removed when the test finishes.
    fn scratch_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("local-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn data_deletion_is_refused_for_anything_but_the_app_s_own_directory() {
        let root = scratch_root("confine");
        let project = root.join("memos");
        fs::create_dir_all(&project).unwrap();
        confined_to_managed_root(&project, "memos", &root).unwrap();

        // A directory that exists but belongs to a different app.
        let other = root.join("n8n");
        fs::create_dir_all(&other).unwrap();
        assert_eq!(
            confined_to_managed_root(&other, "memos", &root)
                .unwrap_err()
                .code,
            ErrorCode::UnsafePath
        );

        // Somewhere else entirely, and the managed root itself.
        let outside = scratch_root("confine-outside");
        assert!(confined_to_managed_root(&outside, "memos", &root).is_err());
        assert!(confined_to_managed_root(&root, "memos", &root).is_err());

        // A path that resolves out of the root by traversal, even though it is
        // spelled as if it were inside it.
        let escape = root.join("memos").join("..").join("..");
        assert!(confined_to_managed_root(&escape, "memos", &root).is_err());

        // Nested deeper than one level below the root.
        let nested = root.join("memos").join("memos");
        fs::create_dir_all(&nested).unwrap();
        assert!(confined_to_managed_root(&nested, "memos", &root).is_err());

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    /// A directory link, which is what makes the resolved-path half of the
    /// deletion guard necessary. Junctions need no privileges on Windows.
    fn link_dir(link: &Path, target: &Path) -> bool {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args([
                    "/C",
                    "mklink",
                    "/J",
                    &link.to_string_lossy(),
                    &target.to_string_lossy(),
                ])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }

    #[test]
    fn a_link_wearing_the_managed_directory_s_name_cannot_be_deleted() {
        let root = scratch_root("link-root");
        let outside = scratch_root("link-target");
        fs::write(outside.join("important.db"), b"not ours").unwrap();
        let link = root.join("memos");
        if !link_dir(&link, &outside) {
            eprintln!("skipped: this machine would not create a directory link");
            let _ = fs::remove_dir_all(&root);
            let _ = fs::remove_dir_all(&outside);
            return;
        }

        // Spelled exactly like the app's managed directory, so the name check
        // passes; only resolving the path reveals it points somewhere else.
        assert_eq!(link, root.join("memos"));
        assert_eq!(
            confined_to_managed_root(&link, "memos", &root)
                .unwrap_err()
                .code,
            ErrorCode::UnsafePath
        );
        assert!(
            outside.join("important.db").exists(),
            "the guard let a link redirect a delete outside the managed root"
        );

        // Remove the link itself, never the directory it points at. A Windows
        // junction is removed as a directory and a Unix symlink as a file;
        // using the wrong one fails rather than following the link, which is
        // how this went unnoticed until it first ran on Linux.
        #[cfg(windows)]
        fs::remove_dir(&link).unwrap();
        #[cfg(unix)]
        fs::remove_file(&link).unwrap();
        assert!(outside.join("important.db").exists());
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn uninstall_keeps_managed_data_unless_deletion_is_asked_for() {
        let project = scratch_root("uninstall-keep");
        fs::create_dir_all(project.join("data")).unwrap();
        fs::write(project.join("data/notes.db"), b"user data").unwrap();
        let app = managed(&project);

        let runner = FakeRunner::passing(1);
        uninstall_with(&runner, &app, false).unwrap();
        assert!(
            project.join("data/notes.db").exists(),
            "uninstall without deletion removed user data"
        );
        assert!(runner.calls.lock().unwrap()[0]
            .args
            .ends_with(&["down".to_owned()]));

        // Asking for deletion removes the volumes as well as the directory.
        let runner = FakeRunner::passing(1);
        uninstall_with(&runner, &app, true).unwrap();
        assert!(!project.exists(), "deletion left the project directory");
        assert!(runner.calls.lock().unwrap()[0]
            .args
            .ends_with(&["down".to_owned(), "--volumes".to_owned()]));
    }

    /// An app that keeps its data somewhere nested — `data/.ollama`, say —
    /// still puts a single `data` directory here. Matching the listing against
    /// the full relative path made the two disagree, so every keep-data
    /// reinstall of such an app was refused as unrecognised. A batch run found
    /// it on the second and third apps it tried.
    #[test]
    fn a_nested_data_path_is_recognised_by_the_directory_that_holds_it() {
        let root = scratch_root("nested");
        let project = root.join("nested-app");
        fs::create_dir_all(project.join("data/.ollama")).unwrap();
        fs::write(project.join("data/.ollama/model.bin"), b"weights").unwrap();
        fs::write(
            project.join("compose.yaml"),
            b"services: {}
",
        )
        .unwrap();

        let source = InstallSource {
            id: "nested-app".into(),
            display_name: "Nested".into(),
            catalog_id: None,
            launch_url: "http://localhost:11434".into(),
            health_url: "http://localhost:11434".into(),
            host_port: 11434,
            companion_ports: Vec::new(),
            compose: "services: {}
"
            .into(),
            data_directories: vec!["data/.ollama".into()],
            extra_files: Vec::new(),
            seed_files: Vec::new(),
            first_start_timeout: None,
        };
        let runner = FakeRunner::passing(6);
        let installed = install_source_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &source,
            &root,
            7,
        )
        .expect("a nested data path must not block a reinstall");
        assert_eq!(installed.id, "nested-app");
        // The person's data is still there, untouched.
        assert!(project.join("data/.ollama/model.bin").exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_preserved_directory_with_unexpected_files_is_not_reused() {
        let root = scratch_root("unexpected");
        let project = root.join("memos");
        fs::create_dir_all(project.join("data")).unwrap();
        // Something Local Store did not put there: refuse rather than adopt a
        // directory whose contents are not understood, and never delete it.
        fs::write(project.join("secrets.env"), b"API_TOKEN=live").unwrap();

        let runner = FakeRunner::passing(4);
        let error = install_recipe_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &crate::recipes::recipe("memos").unwrap(),
            &root,
            5,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::UnsafePath);
        assert!(
            project.join("secrets.env").exists(),
            "a refused reinstall removed a file it did not recognise"
        );
        // Refused before Docker was asked to do anything.
        assert_eq!(runner.calls.lock().unwrap().len(), 2);
        fs::remove_dir_all(&root).unwrap();
    }

    fn sample_template() -> PlanTemplate {
        use crate::plan::{PlanService, PublishedPort};
        use crate::setup::{FieldKind, SecretSpec, SetupField};
        PlanTemplate {
            first_start: None,
            seeds: Vec::new(),
            plan: crate::plan::DeploymentPlan {
                id: "memos".into(),
                services: vec![PlanService {
                    name: "memos".into(),
                    image: "example/app:1.0.0".into(),
                    digest: None,
                    environment: vec![
                        ("DB_PASSWORD".into(), "${DB_PASSWORD}".into()),
                        ("SITE".into(), "${SITE_NAME}".into()),
                    ],
                    companion: None,
                    networks: Vec::new(),
                    published: Some(PublishedPort {
                        host: 5230,
                        container: 5230,
                    }),
                    mounts: vec![crate::plan::PlanMount::directory("data", "/data")],
                    depends_on: Vec::new(),
                    overrides: crate::plan::PlanOverrides::default(),
                }],
                named_volumes: Vec::new(),
                internal_networks: Vec::new(),
            },
            fields: vec![SetupField {
                key: "SITE_NAME".into(),
                label: "Site name".into(),
                kind: FieldKind::Text {
                    min_len: 1,
                    max_len: 60,
                },
                required: true,
                default: None,
                sensitive: false,
            }],
            secrets: vec![SecretSpec {
                key: "DB_PASSWORD".into(),
                length: 32,
                format: crate::setup::SecretFormat::Alphanumeric,
            }],
        }
    }

    fn answers_for(name: &str) -> std::collections::BTreeMap<String, String> {
        [("SITE_NAME".to_owned(), name.to_owned())]
            .into_iter()
            .collect()
    }

    fn stored_secrets(project: &Path) -> std::collections::BTreeMap<String, String> {
        serde_json::from_str(&fs::read_to_string(project.join(SECRETS_FILE)).unwrap()).unwrap()
    }

    #[test]
    fn installing_from_a_plan_resolves_the_compose_file_and_keeps_its_secret() {
        let root = scratch_root("plan-install");
        let runner = FakeRunner::passing(4);
        let app = install_template_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .expect("plan install should succeed");

        assert_eq!(app.launch_url, "http://localhost:5230");
        let project = root.join("memos");
        let compose = fs::read_to_string(project.join("compose.yaml")).unwrap();
        // The answer and the secret are both resolved; no placeholder ships.
        assert!(compose.contains("My notes"), "{compose}");
        assert!(
            !compose.contains("${"),
            "a placeholder reached the container"
        );

        let generated = stored_secrets(&project)["DB_PASSWORD"].clone();
        assert_eq!(generated.chars().count(), 32);
        assert!(
            compose.contains(&generated),
            "the secret never reached Compose"
        );
        assert!(project.join("data").is_dir());
        fs::remove_dir_all(&root).unwrap();
    }

    /// Notemark mounts `data/proxy/nginx.conf`, a file Runtipi ships beside
    /// the definition. Without it Docker made an empty directory there and the
    /// proxy refused to start.
    #[test]
    fn seed_files_are_written_once_and_a_mounted_seed_stays_a_file() {
        let root = scratch_root("plan-seeds");
        let mut template = sample_template();
        template.plan.services[0]
            .mounts
            .push(crate::plan::PlanMount::directory(
                "data/proxy/nginx.conf",
                "/etc/nginx/conf.d/default.conf",
            ));
        template.seeds = vec![crate::setup::SeedFile {
            path: "data/proxy/nginx.conf".into(),
            content: "server { listen 80; }\n".into(),
        }];
        let install = |runner: &FakeRunner| {
            install_template_with(
                &context(runner, &Ready(true), &Ports(true), &CancelToken::new()),
                &template,
                "Memos",
                &answers_for("My notes"),
                &root,
                7,
            )
        };
        let app = install(&FakeRunner::passing(4)).expect("install with a seed");
        let seeded = root.join("memos/data/proxy/nginx.conf");
        assert!(seeded.is_file(), "the seed was not written as a file");
        assert_eq!(
            fs::read_to_string(&seeded).unwrap(),
            "server { listen 80; }\n"
        );

        // Somebody edits it; a keep-data reinstall must not put the original back.
        fs::write(&seeded, "server { listen 8080; }\n").unwrap();
        uninstall_with(&FakeRunner::passing(1), &app, false).unwrap();
        install(&FakeRunner::passing(4)).expect("reinstall over kept data");
        assert_eq!(
            fs::read_to_string(&seeded).unwrap(),
            "server { listen 8080; }\n",
            "a reinstall overwrote a seed file somebody had edited"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// Sim's realtime server: a second address the app's pages call. It
    /// must dodge a busy port and the one the main address took, and a
    /// reinstall must keep whatever it got.
    #[test]
    fn a_second_address_is_chosen_beside_the_main_one_and_kept_on_reinstall() {
        let root = scratch_root("plan-companion");
        let mut template = sample_template();
        template.plan.services[0]
            .environment
            .push(("SOCKET".into(), "${LOCAL_STORE_URL_REALTIME}".into()));
        template.plan.services.push(crate::plan::PlanService {
            name: "realtime".into(),
            image: "example/realtime:1.0.0".into(),
            digest: None,
            environment: vec![("PORT".into(), "${LOCAL_STORE_PORT_REALTIME}".into())],
            // Declared on the port the main address will take, which is
            // busy anyway: the installer has to step past both.
            companion: Some(crate::plan::PublishedPort {
                host: 5229,
                container: 3002,
            }),
            networks: Vec::new(),
            published: None,
            mounts: Vec::new(),
            depends_on: Vec::new(),
            overrides: crate::plan::PlanOverrides::default(),
        });
        template.validate().expect("the template is valid");

        let install = |ports: &dyn PortProbe| {
            install_template_with(
                &InstallContext::new(
                    &FakeRunner::passing(4),
                    &Ready(true),
                    ports,
                    &CancelToken::new(),
                ),
                &template,
                "Memos",
                &answers_for("My notes"),
                &root,
                7,
            )
        };
        let app = install(&BusyPorts(vec![5229])).expect("install with a second address");
        assert_eq!(app.launch_url, "http://localhost:5230");
        let compose = fs::read_to_string(root.join("memos/compose.yaml")).unwrap();
        assert!(compose.contains("published: \"5231\""), "{compose}");
        assert!(
            compose.contains("SOCKET: \"http://localhost:5231\""),
            "{compose}"
        );
        assert!(compose.contains("PORT: \"5231\""), "{compose}");
        assert_eq!(crate::plan::published_host_port(&compose), Some(5230));

        // Everything is free now, the declared port included; the reinstall
        // still lands where the first install did.
        uninstall_with(&FakeRunner::passing(1), &app, false).unwrap();
        let app = install(&Ports(true)).expect("reinstall over kept data");
        let compose = fs::read_to_string(root.join("memos/compose.yaml")).unwrap();
        assert!(compose.contains("published: \"5231\""), "{compose}");

        // And a second address somebody else took since is reported, not
        // quietly moved.
        uninstall_with(&FakeRunner::passing(1), &app, false).unwrap();
        let error = install(&BusyPorts(vec![5231])).unwrap_err();
        assert_eq!(error.code, ErrorCode::PortInUse);
        assert!(error.message.contains("5231"), "{}", error.message);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_seed_outside_the_data_folder_is_refused() {
        let mut template = sample_template();
        for path in [
            "compose.yaml",
            "data/../compose.yaml",
            "/etc/passwd",
            "data/",
            "data/a b",
        ] {
            template.seeds = vec![crate::setup::SeedFile {
                path: path.into(),
                content: String::new(),
            }];
            assert!(template.validate().is_err(), "seed {path:?} was accepted");
        }
    }

    #[test]
    fn corrupt_retained_secrets_stop_before_docker_and_remain_untouched() {
        let root = scratch_root("corrupt-plan-secrets");
        let project = root.join("memos");
        fs::create_dir_all(&project).unwrap();
        let path = project.join(SECRETS_FILE);
        let content = "{broken-private-secret";
        fs::write(&path, content).unwrap();
        let runner = FakeRunner::passing(0);
        let error = install_template_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("Stored setup credentials are invalid"));
        assert!(!error.to_string().contains(content));
        assert!(runner.calls.lock().unwrap().is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), content);
        assert!(!project.join("compose.yaml").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_installs_refuse_a_lock_for_another_app() {
        let recipe = crate::recipes::recipe("memos").unwrap();
        assert!(begin_install(
            &recipe,
            lock_operation("wrong-recipe-lock").unwrap(),
            &|_| {}
        )
        .is_err());
        assert!(lock_operation("wrong-recipe-lock").is_ok());
        assert!(begin_template_install(
            &sample_template(),
            "Memos",
            &answers_for("Notes"),
            lock_operation("wrong-template-lock").unwrap(),
            &|_| {}
        )
        .is_err());
        assert!(lock_operation("wrong-template-lock").is_ok());
    }

    #[test]
    fn reinstalling_over_preserved_data_keeps_the_same_secret() {
        let root = scratch_root("plan-reinstall");
        install_template_with(
            &context(
                &FakeRunner::passing(4),
                &Ready(true),
                &Ports(true),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap();
        let project = root.join("memos");
        let original = stored_secrets(&project);
        // Data written while the first secret was in force.
        fs::write(project.join("data/app.db"), b"rows").unwrap();

        install_template_with(
            &context(
                &FakeRunner::passing(4),
                &Ready(true),
                &Ports(true),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            9,
        )
        .expect("reinstall should reuse the retained secret");

        // A fresh password here would leave the app unable to open this data.
        assert_eq!(
            stored_secrets(&project)["DB_PASSWORD"],
            original["DB_PASSWORD"]
        );
        assert_eq!(fs::read(project.join("data/app.db")).unwrap(), b"rows");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_secret_the_plan_no_longer_declares_is_dropped_from_disk() {
        let root = scratch_root("plan-secret-drop");
        install_template_with(
            &context(
                &FakeRunner::passing(4),
                &Ready(true),
                &Ports(true),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap();
        let project = root.join("memos");

        // Something left behind by an older version of the plan.
        let mut stored = stored_secrets(&project);
        stored.insert("OLD_TOKEN".into(), "leftover".into());
        fs::write(
            project.join(SECRETS_FILE),
            serde_json::to_string_pretty(&stored).unwrap(),
        )
        .unwrap();

        install_template_with(
            &context(
                &FakeRunner::passing(4),
                &Ready(true),
                &Ports(true),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            9,
        )
        .unwrap();
        let after = stored_secrets(&project);
        assert!(!after.contains_key("OLD_TOKEN"), "a stale secret survived");
        assert!(after.contains_key("DB_PASSWORD"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_first_install_steps_past_a_busy_port_instead_of_failing() {
        let root = scratch_root("plan-port-busy");
        let runner = FakeRunner::passing(4);
        // The plan prefers 5230; something else already answers there.
        let busy = BusyPorts(vec![5230, 5231]);
        let app = install_template_with(
            &context_with_ports(&runner, &Ready(true), &busy, &CancelToken::new()),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .expect("a busy preferred port must not fail a first install");

        assert_eq!(app.launch_url, "http://localhost:5232");
        let compose = fs::read_to_string(root.join("memos/compose.yaml")).unwrap();
        // The container port never moves; only the address this computer answers on.
        assert!(compose.contains("127.0.0.1:5232:5230"), "{compose}");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_app_is_told_the_port_it_actually_got_not_the_one_it_asked_for() {
        let root = scratch_root("plan-platform-address");
        let mut template = sample_template();
        // An upstream definition that tells the app its own address, which is
        // the shape 59 Runtipi and 145 CapRover values take.
        template.plan.services[0]
            .environment
            .push(("SITE_URL".into(), "${LOCAL_STORE_URL}".into()));
        template.plan.services[0]
            .environment
            .push(("SITE_HOST".into(), "${LOCAL_STORE_HOST}".into()));

        // 5230 is the plan's preference; it is taken, so the install moves on.
        let app = install_template_with(
            &context_with_ports(
                &FakeRunner::passing(4),
                &Ready(true),
                &BusyPorts(vec![5230, 5231]),
                &CancelToken::new(),
            ),
            &template,
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap();

        assert_eq!(app.launch_url, "http://localhost:5232");
        let compose = fs::read_to_string(root.join("memos/compose.yaml")).unwrap();
        // Being told 5230 here would leave the app publishing links to an
        // address it does not answer on.
        assert!(
            compose.contains(r#"SITE_URL: "http://localhost:5232""#),
            "{compose}"
        );
        assert!(
            compose.contains(r#"SITE_HOST: "localhost:5232""#),
            "{compose}"
        );
        // The container still listens on 5230; only the address the app is
        // told about, and the host side of the mapping, moved.
        assert!(compose.contains("127.0.0.1:5232:5230"), "{compose}");
        assert!(
            !compose
                .lines()
                .any(|line| line.contains("SITE") && line.contains("5230")),
            "{compose}"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_reinstall_keeps_the_address_the_app_already_had() {
        let root = scratch_root("plan-port-stable");
        // First install lands on 5232 because the preferred port is taken.
        install_template_with(
            &context_with_ports(
                &FakeRunner::passing(4),
                &Ready(true),
                &BusyPorts(vec![5230, 5231]),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap();

        // By the time it is reinstalled the preferred port is free again.
        // Moving back would break every link the person saved.
        let again = install_template_with(
            &context(
                &FakeRunner::passing(4),
                &Ready(true),
                &Ports(true),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            9,
        )
        .unwrap();
        assert_eq!(again.launch_url, "http://localhost:5232");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_reinstall_onto_a_busy_port_reports_it_rather_than_starting_a_second_copy() {
        let root = scratch_root("plan-port-conflict");
        install_template_with(
            &context(
                &FakeRunner::passing(4),
                &Ready(true),
                &Ports(true),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap();

        // The likeliest occupant of 5230 now is this very app, still running.
        // Quietly moving to 5231 would leave two copies of it side by side.
        let runner = FakeRunner::passing(4);
        let error = install_template_with(
            &context_with_ports(
                &runner,
                &Ready(true),
                &BusyPorts(vec![5230]),
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            9,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::PortInUse);
        assert!(error.message.contains("5230"), "{}", error.message);
        // Docker is asked whether it is running before the port is judged,
        // which is fine; what must not happen is a second copy being started.
        assert!(
            !runner
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| call.args.iter().any(|arg| arg == "up")),
            "a second copy of the app was started on the conflicting port"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_install_with_no_free_port_anywhere_near_is_refused() {
        let root = scratch_root("plan-port-exhausted");
        let busy = BusyPorts((5230..=5260).collect());
        let error = install_template_with(
            &context_with_ports(
                &FakeRunner::passing(4),
                &Ready(true),
                &busy,
                &CancelToken::new(),
            ),
            &sample_template(),
            "Memos",
            &answers_for("My notes"),
            &root,
            7,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::PortInUse);
        assert!(
            !root.join("memos").exists(),
            "a refused install wrote files"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_missing_answer_stops_the_install_before_docker_is_touched() {
        let root = scratch_root("plan-missing-answer");
        let runner = FakeRunner::passing(4);
        let error = install_template_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &sample_template(),
            "Memos",
            &std::collections::BTreeMap::new(),
            &root,
            7,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert!(error.message.contains("SITE_NAME"), "{}", error.message);
        assert!(
            !root.join("memos").exists(),
            "a refused install wrote files"
        );
        assert!(runner.calls.lock().unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rollback_after_a_failed_commit_keeps_data_that_predates_the_install() {
        let root =
            std::env::temp_dir().join(format!("local-store-rollback-{}-{}", std::process::id(), 5));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("data/keep.db"), b"keep").unwrap();
        let app = managed(&root);

        // Reinstalling over data the user chose to keep: rollback must stop the
        // containers and leave every byte of that data in place.
        let runner = FakeRunner::passing(1);
        rollback_install_with(&runner, &app, true).unwrap();
        assert_eq!(fs::read(root.join("data/keep.db")).unwrap(), b"keep");
        assert!(runner.calls.lock().unwrap()[0]
            .args
            .ends_with(&["down".to_owned()]));

        // A directory this install created carries no earlier data, so the
        // rollback removes it along with its volumes.
        let runner = FakeRunner::passing(1);
        rollback_install_with(&runner, &app, false).unwrap();
        assert!(!root.exists());
        assert!(runner.calls.lock().unwrap()[0]
            .args
            .ends_with(&["down".to_owned(), "--volumes".to_owned()]));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reinstall_reuses_preserved_data_without_deleting_it_on_failure() {
        let root =
            std::env::temp_dir().join(format!("local-store-reinstall-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let project = root.join("memos");
        fs::create_dir_all(project.join("data")).unwrap();
        fs::write(project.join("data/keep.db"), b"keep").unwrap();
        fs::write(project.join("compose.yaml"), b"previous").unwrap();
        let runner = FakeRunner {
            outputs: Mutex::new(VecDeque::from([
                Ok(ProcessOutput {
                    success: true,
                    stdout: "1".into(),
                    stderr: String::new(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: true,
                    stdout: "1".into(),
                    stderr: String::new(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "invalid".into(),
                    truncated: false,
                }),
                Ok(ProcessOutput {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                }),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        assert!(install_recipe_with(
            &context(&runner, &Ready(true), &Ports(true), &CancelToken::new()),
            &crate::recipes::recipe("memos").unwrap(),
            &root,
            5
        )
        .is_err());
        assert_eq!(fs::read(project.join("data/keep.db")).unwrap(), b"keep");
        assert_eq!(fs::read(project.join("compose.yaml")).unwrap(), b"previous");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn lifecycle_and_status_preserve_process_error_codes() {
        let app = managed(Path::new("unused-project"));
        for code in [
            ProcessErrorCode::TimedOut,
            ProcessErrorCode::Cancelled,
            ProcessErrorCode::ProcessUnavailable,
        ] {
            let expected = AppError::from(ProcessError::new(code, "failure"));
            let runner = FakeRunner {
                outputs: Mutex::new(VecDeque::from([
                    Err(ProcessError::new(code, "failure")),
                    Err(ProcessError::new(code, "failure")),
                ])),
                calls: Mutex::new(Vec::new()),
            };
            assert_eq!(start_with(&runner, &app).unwrap_err(), expected);
            assert_eq!(status_with(&runner, &app).unwrap_err(), expected);
        }
        let error = wait_for_health_with(
            &Ready(false),
            "http://localhost",
            Duration::ZERO,
            &CancelToken::new(),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::TimedOut);
    }
    #[test]
    fn install_progress_reports_real_stages_and_cleanup() {
        for healthy in [true, false] {
            let root = std::env::temp_dir().join(format!(
                "local-store-progress-{}-{healthy}",
                std::process::id()
            ));
            let stages = Mutex::new(Vec::new());
            let progress = |stage| stages.lock().unwrap().push(stage);
            let runner = FakeRunner::passing(5);
            let health = Ready(healthy);
            let ports = Ports(true);
            let cancel = CancelToken::new();
            // Cancelling *at* the health wait keeps the unhealthy path failing
            // without tripping the earlier cancellation checkpoints, which
            // would otherwise stop the install before these stages are
            // reported at all.
            let progress = |stage| {
                progress(stage);
                if !healthy && matches!(stage, InstallStage::WaitingForHealth) {
                    cancel.cancel();
                }
            };
            let mut context = InstallContext::new(&runner, &health, &ports, &cancel);
            context.progress = Some(&progress);
            let result = install_recipe_with(
                &context,
                &crate::recipes::recipe("memos").unwrap(),
                &root,
                1,
            );
            assert_eq!(result.is_ok(), healthy);
            let mut expected = vec![
                InstallStage::CheckingSystem,
                InstallStage::PreparingFiles,
                InstallStage::ValidatingRecipe,
                InstallStage::StartingContainers,
                InstallStage::WaitingForHealth,
            ];
            if !healthy {
                expected.push(InstallStage::RollingBack);
                assert!(!root.join("memos").exists());
            }
            assert_eq!(*stages.lock().unwrap(), expected);
            fs::remove_dir_all(&root).unwrap();
        }
    }
    #[test]
    fn failed_container_cleanup_preserves_setup_files_and_reports_both_failures() {
        let root =
            std::env::temp_dir().join(format!("local-store-cleanup-failed-{}", std::process::id()));
        let runner = FakeRunner::passing(4);
        runner.outputs.lock().unwrap().push_back(Ok(ProcessOutput {
            success: false,
            stderr: "cleanup refused".into(),
            ..Default::default()
        }));
        let cancel = CancelToken::new();
        let health = Ready(false);
        let ports = Ports(true);
        let mut ctx = InstallContext::new(&runner, &health, &ports, &cancel);
        // Cancel once the install reaches the health wait, so the failure under
        // test is the refused cleanup rather than an early cancellation
        // checkpoint.
        let cancel_at_health = |stage: InstallStage| {
            if matches!(stage, InstallStage::WaitingForHealth) {
                cancel.cancel();
            }
        };
        ctx.progress = Some(&cancel_at_health);
        let error = install_recipe_with(&ctx, &crate::recipes::recipe("memos").unwrap(), &root, 1)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::RollbackFailed);
        assert!(error.message.contains("cancelled"));
        assert!(error.message.contains("cleanup refused"));
        assert!(root.join("memos/compose.yaml").exists());
        assert!(root.join("memos/data").exists());
        fs::remove_dir_all(root).unwrap();
    }
}

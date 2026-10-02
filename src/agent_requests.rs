//! Bounded cross-process owner approvals for exact agent mutations. Requests
//! contain reviewed identities and hashes, never answers, credentials or shell
//! commands. An agent may request an action; only the launcher may approve it.
use crate::{
    agent_gateway::AgentGateway,
    agent_policy::AgentAction,
    brand::CONFIG_SLUG,
    error::{AppError, AppResult, ErrorCode},
    model::{InstalledApp, RuntimeSpec},
    recipes::Recipe,
    runtime::{self, engine::EngineBinding, ProcessRunner},
    storage,
};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const MAX_ROWS: usize = 256;
const MAX_ACTIVE: usize = 32;
const MAX_BYTES: u64 = 1024 * 1024;
const APPROVAL_SECONDS: u64 = 600;
const REQUEST_SECONDS: u64 = 24 * 3600;
const RUN_SECONDS: u64 = 15 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    Install,
    UninstallKeepData,
}
impl MutationKind {
    fn action(self) -> AgentAction {
        match self {
            Self::Install => AgentAction::Install,
            Self::UninstallKeepData => AgentAction::Uninstall,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationState {
    Pending,
    Approved,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Denied,
    Expired,
}
impl MutationState {
    fn active(self) -> bool {
        matches!(self, Self::Pending | Self::Approved | Self::Running)
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationFailure {
    RuntimeFailed,
    AccessRevoked,
    IdentityChanged,
    TimedOut,
    ConnectorInterrupted,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationTarget {
    pub kind: MutationKind,
    pub app_id: String,
    pub display_name: String,
    pub fingerprint: String,
    pub engine: EngineBinding,
    pub engine_fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationRequest {
    pub id: String,
    pub client_id: String,
    pub target: MutationTarget,
    pub state: MutationState,
    pub requested_at_unix: u64,
    pub expires_at_unix: u64,
    pub approved_until_unix: Option<u64>,
    pub started_at_unix: Option<u64>,
    pub runtime_operation_id: Option<u64>,
    pub cancel_requested: bool,
    pub stage: Option<String>,
    pub failure: Option<MutationFailure>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    request: MutationRequest,
    credential_generation: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueueFile {
    version: u32,
    rows: Vec<Row>,
}
struct Queue {
    path: PathBuf,
    rows: Vec<Row>,
    _lock: fs::File,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}
fn client_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
fn denied() -> AppError {
    AppError::new(ErrorCode::Forbidden, "Agent mutation access denied.")
}
fn clock() -> AppResult<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::internal("System clock is unavailable."))?
        .as_secs())
}

impl Queue {
    fn open(root: &Path, now: u64) -> AppResult<Self> {
        fs::create_dir_all(root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("agent-requests.lock"))?;
        FileExt::try_lock(&lock).map_err(|_| {
            AppError::new(
                ErrorCode::OperationBusy,
                "Agent requests are being updated. Retry shortly.",
            )
        })?;
        let path = root.join("agent-requests-v1.json");
        let rows = if path.exists() {
            if fs::metadata(&path)?.len() > MAX_BYTES {
                return Err(AppError::new(
                    ErrorCode::StorageCorrupt,
                    "Agent request queue is too large.",
                ));
            }
            let file: QueueFile = serde_json::from_slice(&fs::read(&path)?).map_err(|_| {
                AppError::new(ErrorCode::StorageCorrupt, "Agent request queue is invalid.")
            })?;
            if file.version != 1 || file.rows.len() > MAX_ROWS {
                return Err(AppError::new(
                    ErrorCode::StorageCorrupt,
                    "Agent request queue version or size is unsupported.",
                ));
            }
            let mut ids = BTreeSet::new();
            for row in &file.rows {
                let r = &row.request;
                if !digest_valid(&r.id)
                    || !ids.insert(r.id.clone())
                    || !client_valid(&r.client_id)
                    || !crate::model::is_valid_installed_app_id(&r.target.app_id)
                    || r.target.app_id.len() > 128
                    || !digest_valid(&row.credential_generation)
                    || !digest_valid(&r.target.fingerprint)
                    || !digest_valid(&r.target.engine_fingerprint)
                    || r.target.engine.validate().is_err()
                    || r.target.display_name.is_empty()
                    || r.target.display_name.len() > 256
                    || r.expires_at_unix <= r.requested_at_unix
                    || r.expires_at_unix - r.requested_at_unix > REQUEST_SECONDS
                    || r.approved_until_unix.is_some_and(|t| t > r.expires_at_unix)
                    || (matches!(
                        r.state,
                        MutationState::Approved | MutationState::Running | MutationState::Succeeded
                    ) && r.approved_until_unix.is_none())
                    || (r.state == MutationState::Running
                        && (r.started_at_unix.is_none() || r.runtime_operation_id.is_none()))
                    || r.stage.as_ref().is_some_and(|s| {
                        !matches!(
                            s.as_str(),
                            "checking_system"
                                | "preparing_files"
                                | "validating_recipe"
                                | "starting_containers"
                                | "waiting_for_health"
                                | "saving_app"
                                | "rolling_back"
                        )
                    })
                {
                    return Err(AppError::new(
                        ErrorCode::StorageCorrupt,
                        "Agent request queue contains an invalid record.",
                    ));
                }
            }
            file.rows
        } else {
            Vec::new()
        };
        let mut queue = Self {
            path,
            rows,
            _lock: lock,
        };
        let mut changed = false;
        for row in &mut queue.rows {
            let r = &mut row.request;
            if matches!(r.state, MutationState::Pending | MutationState::Approved)
                && (now >= r.expires_at_unix || r.approved_until_unix.is_some_and(|t| now >= t))
            {
                r.state = MutationState::Expired;
                changed = true;
            }
        }
        if changed {
            queue.save()?;
        }
        Ok(queue)
    }
    fn save(&self) -> AppResult<()> {
        let bytes = serde_json::to_vec_pretty(&QueueFile {
            version: 1,
            rows: self.rows.clone(),
        })
        .map_err(AppError::internal)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(AppError::invalid("Too many agent requests."));
        }
        storage::write_file_atomically(&self.path, &bytes).map_err(AppError::from)
    }
    fn row(&self, id: &str) -> AppResult<&Row> {
        self.rows
            .iter()
            .find(|r| r.request.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Agent request does not exist."))
    }
    fn row_mut(&mut self, id: &str) -> AppResult<&mut Row> {
        self.rows
            .iter_mut()
            .find(|r| r.request.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Agent request does not exist."))
    }
    fn submit(
        &mut self,
        client: &str,
        generation: &str,
        target: MutationTarget,
        now: u64,
    ) -> AppResult<MutationRequest> {
        if self
            .rows
            .iter()
            .filter(|r| r.request.state.active())
            .count()
            >= MAX_ACTIVE
            || self
                .rows
                .iter()
                .filter(|r| r.request.client_id == client && r.request.state.active())
                .count()
                >= 8
        {
            return Err(AppError::new(
                ErrorCode::OperationBusy,
                "Finish or cancel pending requests before adding another.",
            ));
        }
        if self.rows.len() == MAX_ROWS {
            if let Some(index) = self.rows.iter().position(|r| !r.request.state.active()) {
                self.rows.remove(index);
            } else {
                return Err(AppError::new(
                    ErrorCode::OperationBusy,
                    "Agent request queue is full.",
                ));
            }
        }
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| AppError::internal("Could not create agent request."))?;
        let id = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let request = MutationRequest {
            id,
            client_id: client.into(),
            target,
            state: MutationState::Pending,
            requested_at_unix: now,
            expires_at_unix: now
                .checked_add(REQUEST_SECONDS)
                .ok_or_else(|| AppError::invalid("Request expiry is out of range."))?,
            approved_until_unix: None,
            started_at_unix: None,
            runtime_operation_id: None,
            cancel_requested: false,
            stage: None,
            failure: None,
        };
        self.rows.push(Row {
            request: request.clone(),
            credential_generation: generation.into(),
        });
        self.save()?;
        Ok(request)
    }
    fn decide(
        &mut self,
        id: &str,
        generation: &str,
        target: &MutationTarget,
        approve: bool,
        now: u64,
    ) -> AppResult<MutationRequest> {
        let row = self.row_mut(id)?;
        if row.request.state != MutationState::Pending
            || (approve
                && (row.credential_generation != generation || &row.request.target != target))
        {
            return Err(denied());
        }
        row.request.state = if approve {
            MutationState::Approved
        } else {
            MutationState::Denied
        };
        if approve {
            row.request.approved_until_unix = Some(
                now.saturating_add(APPROVAL_SECONDS)
                    .min(row.request.expires_at_unix),
            );
        }
        let result = row.request.clone();
        self.save()?;
        Ok(result)
    }
    fn claim(
        &mut self,
        id: &str,
        client: &str,
        generation: &str,
        target: &MutationTarget,
        operation_id: u64,
        now: u64,
    ) -> AppResult<MutationRequest> {
        let row = self.row_mut(id)?;
        if row.request.client_id != client
            || row.credential_generation != generation
            || &row.request.target != target
            || row.request.state != MutationState::Approved
            || row.request.approved_until_unix.is_none_or(|t| now >= t)
        {
            return Err(denied());
        }
        row.request.state = MutationState::Running;
        row.request.started_at_unix = Some(now);
        row.request.runtime_operation_id = Some(operation_id);
        let result = row.request.clone();
        self.save()?;
        Ok(result)
    }
    fn cancel(&mut self, id: &str, client: &str, generation: &str) -> AppResult<MutationRequest> {
        let row = self.row_mut(id)?;
        if row.request.client_id != client || row.credential_generation != generation {
            return Err(denied());
        }
        if matches!(
            row.request.state,
            MutationState::Pending | MutationState::Approved
        ) {
            row.request.state = MutationState::Cancelled;
        }
        if row.request.state == MutationState::Running
            && row.request.target.kind == MutationKind::Install
        {
            row.request.cancel_requested = true;
        } else if row.request.state == MutationState::Running {
            return Err(AppError::new(
                ErrorCode::UnsupportedOperation,
                "Keep-data uninstall has already started and cannot be cancelled.",
            ));
        }
        let result = row.request.clone();
        self.save()?;
        Ok(result)
    }
    fn finish(&mut self, id: &str, result: &AppResult<()>) -> AppResult<()> {
        let row = self.row_mut(id)?;
        if row.request.state != MutationState::Running {
            return Err(denied());
        }
        match result {
            Ok(()) => row.request.state = MutationState::Succeeded,
            Err(error) if error.code == ErrorCode::Cancelled => {
                row.request.state = MutationState::Cancelled
            }
            Err(_) => {
                row.request.state = MutationState::Failed;
                row.request.failure = Some(MutationFailure::RuntimeFailed);
            }
        }
        self.save()
    }
}

fn engine_fingerprint(binding: &EngineBinding) -> AppResult<String> {
    let spec = binding.command(&runtime::CommandSpec::docker(
        vec!["info".into(), "--format".into(), "{{.ID}}".into()],
        None,
        runtime::DIAGNOSTIC_TIMEOUT,
    ))?;
    let output = runtime::SystemProcessRunner.run(&spec)?;
    if !output.success
        || output.truncated
        || output.stdout.trim().is_empty()
        || output.stdout.len() > 256
    {
        return Err(AppError::invalid(
            "The exact engine identity could not be verified.",
        ));
    }
    let ownership = if binding.is_wsl() {
        let journal = runtime::engine::wsl::bootstrap::load(&storage::managed_engine_state_root())?
            .ok_or_else(|| AppError::invalid("The owned engine record is missing."))?;
        hash(journal.ownership_token.as_bytes())
    } else {
        String::new()
    };
    Ok(hash(
        &serde_json::to_vec(&(binding, output.stdout.trim(), ownership))
            .map_err(AppError::internal)?,
    ))
}
enum PreparedTarget {
    Install(Box<Recipe>),
    Uninstall(Box<InstalledApp>),
}
fn describe(kind: MutationKind, app_id: &str) -> AppResult<(MutationTarget, PreparedTarget)> {
    if app_id.len() > 128 || !crate::model::is_valid_installed_app_id(app_id) {
        return Err(AppError::invalid("Invalid installed app identity."));
    }
    let apps = storage::load_or_migrate_registry()
        .map_err(AppError::from)?
        .apps;
    let (name, fingerprint, engine, prepared) = match kind {
        MutationKind::Install => {
            if apps.iter().any(|a| a.id == app_id) {
                return Err(AppError::new(
                    ErrorCode::AlreadyExists,
                    "That app is already installed.",
                ));
            }
            let recipe = crate::recipes::recipe(app_id).ok_or_else(|| AppError::new(ErrorCode::UnsupportedOperation, "Agent installation currently supports reviewed zero-input recipes only. Install apps requiring setup in the launcher."))?;
            let offering = crate::offerings::offering(app_id)
                .ok_or_else(|| AppError::invalid("Reviewed offering is unavailable."))?;
            let template = offering.plan_template(None)?;
            if !template.fields.is_empty() || !template.secrets.is_empty() {
                return Err(AppError::new(
                    ErrorCode::UnsupportedOperation,
                    "This app requires owner setup in the launcher.",
                ));
            }
            let fingerprint = hash(&serde_json::to_vec(&recipe).map_err(AppError::internal)?);
            let engine = EngineBinding::discover(&runtime::SystemProcessRunner)?;
            (
                recipe.display_name.clone(),
                fingerprint,
                engine,
                PreparedTarget::Install(Box::new(recipe)),
            )
        }
        MutationKind::UninstallKeepData => {
            let app = apps
                .into_iter()
                .find(|a| a.id == app_id)
                .ok_or_else(|| AppError::new(ErrorCode::NotFound, "That app is not installed."))?;
            let RuntimeSpec::Compose {
                project_dir,
                compose_file,
                ..
            } = &app.runtime
            else {
                return Err(AppError::new(
                    ErrorCode::UnsupportedOperation,
                    "Connected apps are not managed installations.",
                ));
            };
            runtime::confined_to_managed_root(project_dir, app_id, &storage::managed_apps_root())?;
            if compose_file.canonicalize()?.parent() != Some(project_dir.canonicalize()?.as_path())
                || fs::metadata(compose_file)?.len() > MAX_BYTES
            {
                return Err(AppError::new(
                    ErrorCode::UnsafePath,
                    "The installed app's Compose identity is invalid.",
                ));
            }
            let engine = runtime::engine::retained(project_dir)?.ok_or_else(|| {
                AppError::invalid(
                    "Adopt the app's saved engine in the launcher before requesting removal.",
                )
            })?;
            let fingerprint = hash(
                &serde_json::to_vec(&(&app, hash(&fs::read(compose_file)?)))
                    .map_err(AppError::internal)?,
            );
            (
                app.display_name.clone(),
                fingerprint,
                engine,
                PreparedTarget::Uninstall(Box::new(app)),
            )
        }
    };
    let identity = engine_fingerprint(&engine)?;
    Ok((
        MutationTarget {
            kind,
            app_id: app_id.into(),
            display_name: name,
            fingerprint,
            engine,
            engine_fingerprint: identity,
        },
        prepared,
    ))
}

#[derive(Clone)]
pub struct AgentRequests {
    root: PathBuf,
    gateway: AgentGateway,
}
impl AgentRequests {
    pub fn open_local() -> AppResult<Self> {
        let base = std::env::var_os("LOCALAPPDATA")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::PrerequisiteUnavailable,
                    "Local app data is unavailable.",
                )
            })?;
        let root = PathBuf::from(base).join(CONFIG_SLUG).join("agent-requests");
        drop(Queue::open(&root, clock()?)?);
        Ok(Self {
            root,
            gateway: AgentGateway::open_local()?,
        })
    }
    pub fn request(
        &self,
        secret: &str,
        kind: MutationKind,
        app_id: &str,
    ) -> AppResult<MutationRequest> {
        let (client, generation) = self.gateway.authenticate_mutation(secret)?;
        if kind == MutationKind::UninstallKeepData {
            self.gateway
                .authorize_status_request(secret, app_id, clock()?)?;
        }
        let (target, _) = describe(kind, app_id)?;
        self.gateway
            .audit_mutation(&client, app_id, kind.action(), false, clock()?)?;
        Queue::open(&self.root, clock()?)?.submit(&client, &generation, target, clock()?)
    }
    pub fn status(&self, secret: &str, id: &str) -> AppResult<MutationRequest> {
        // Progress and revocation checks briefly hold the credential/queue
        // locks. A status read may wait a bounded 250 ms; mutations never retry.
        for attempt in 0..10 {
            match self.status_once(secret, id) {
                Err(error) if error.code == ErrorCode::OperationBusy && attempt < 9 => {
                    thread::sleep(Duration::from_millis(25));
                }
                outcome => return outcome,
            }
        }
        unreachable!("bounded status read always returns on its final attempt")
    }
    fn status_once(&self, secret: &str, id: &str) -> AppResult<MutationRequest> {
        let (client, generation) = self.gateway.authenticate_mutation(secret)?;
        let queue = Queue::open(&self.root, clock()?)?;
        let row = queue.row(id)?;
        if row.request.client_id != client || row.credential_generation != generation {
            return Err(denied());
        }
        Ok(row.request.clone())
    }
    pub fn list_for_owner(&self) -> AppResult<Vec<MutationRequest>> {
        Ok(Queue::open(&self.root, clock()?)?
            .rows
            .iter()
            .rev()
            .map(|r| r.request.clone())
            .collect())
    }
    pub fn decide_for_owner(
        &self,
        id: &str,
        approve: bool,
        consent: bool,
    ) -> AppResult<MutationRequest> {
        if !consent {
            return Err(denied());
        }
        let row = Queue::open(&self.root, clock()?)?.row(id)?.clone();
        let request = row.request;
        let generation = if approve {
            self.gateway.owner_client_generation(&request.client_id)?
        } else {
            // Denying an old request must remain possible after revocation.
            // It never grants authority to a revoked credential generation.
            row.credential_generation
        };
        let target = if approve {
            describe(request.target.kind, &request.target.app_id)?.0
        } else {
            request.target.clone()
        };
        Queue::open(&self.root, clock()?)?.decide(id, &generation, &target, approve, clock()?)
    }
    pub fn cancel(&self, secret: &str, id: &str) -> AppResult<MutationRequest> {
        let (client, generation) = self.gateway.authenticate_mutation(secret)?;
        Queue::open(&self.root, clock()?)?.cancel(id, &client, &generation)
    }
    pub fn execute(&self, secret: &str, id: &str) -> AppResult<MutationRequest> {
        let (client, generation) = self.gateway.authenticate_mutation(secret)?;
        let request = self.status(secret, id)?;
        if request.state != MutationState::Approved {
            return Err(denied());
        }
        if request.target.kind == MutationKind::UninstallKeepData {
            self.gateway
                .authorize_status_request(secret, &request.target.app_id, clock()?)?;
        }
        let lock = runtime::lock_operation(&request.target.app_id)?;
        let (target, prepared) = describe(request.target.kind, &request.target.app_id)?;
        let latest_generation = self.gateway.owner_client_generation(&client)?;
        if generation != latest_generation {
            return Err(denied());
        }
        let mut queue = Queue::open(&self.root, clock()?)?;
        let claimed = queue.claim(id, &client, &generation, &target, lock.id(), clock()?)?;
        if let Err(error) = self.gateway.audit_mutation(
            &client,
            &target.app_id,
            target.kind.action(),
            true,
            clock()?,
        ) {
            // Retain the same queue lock across claim and audit. If audit
            // fails, persist the consumed failure without racing another
            // progress reader or leaving a Running row without a worker.
            queue.finish(id, &Err(error.clone()))?;
            return Err(error);
        }
        drop(queue);
        let service = self.clone();
        let worker_id = id.to_owned();
        let cancel = lock.token();
        let shutdown_cancel = cancel.clone();
        let worker = thread::Builder::new()
            .name("local-store-agent-mutation".into())
            .spawn(move || service.run(worker_id, generation, prepared, target, lock));
        match worker {
            Ok(handle) => {
                workers()?.push(ActiveWorker {
                    handle,
                    cancel: shutdown_cancel,
                });
                Ok(claimed)
            }
            Err(error) => {
                let error = AppError::from(error);
                Queue::open(&self.root, clock()?)?.finish(id, &Err(error.clone()))?;
                Err(error)
            }
        }
    }
    fn run(
        &self,
        id: String,
        generation: String,
        prepared: PreparedTarget,
        target: MutationTarget,
        lock: runtime::OperationLock,
    ) {
        let stop = Arc::new(AtomicBool::new(false));
        let monitor_stop = stop.clone();
        let service = self.clone();
        let monitor_id = id.clone();
        let token = lock.token();
        let monitor = thread::spawn(move || {
            let started = std::time::Instant::now();
            while !monitor_stop.load(Ordering::Acquire) {
                // Queue contention must not extend the overall runtime bound.
                if started.elapsed() >= Duration::from_secs(RUN_SECONDS) {
                    token.cancel();
                    break;
                }
                let result = (|| -> AppResult<bool> {
                    let queue = Queue::open(&service.root, clock()?)?;
                    let row = queue.row(&monitor_id)?;
                    let revoked = service
                        .gateway
                        .owner_client_generation(&row.request.client_id)?
                        != generation;
                    Ok(row.request.cancel_requested || revoked)
                })();
                match result {
                    Ok(false) => {}
                    Err(error) if error.code == ErrorCode::OperationBusy => {}
                    _ => {
                        token.cancel();
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(250));
            }
        });
        let progress = |stage| {
            if let Ok(mut queue) = Queue::open(&self.root, clock().unwrap_or(0)) {
                if let Ok(row) = queue.row_mut(&id) {
                    row.request.stage = serde_json::to_value(stage)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned));
                    let _ = queue.save();
                }
            }
        };
        let result: AppResult<()> = match prepared {
            PreparedTarget::Install(recipe) => {
                runtime::begin_install_on_engine(&recipe, lock, &target.engine, &progress)
                    .and_then(|pending| pending.commit(&progress))
                    .map(|_| ())
            }
            PreparedTarget::Uninstall(app) => {
                // Keep the same app lock through registry removal. This path
                // never passes --volumes and never deletes app data folders.
                let result = runtime::uninstall_with(&runtime::SystemProcessRunner, &app, false)
                    .and_then(|_| {
                        storage::remove_installed_app(&app.id)
                            .map(|_| ())
                            .map_err(AppError::from)
                    });
                drop(lock);
                result
            }
        };
        stop.store(true, Ordering::Release);
        let _ = monitor.join();
        // Short-lived owner updates can hold the sidecar lock; retry only the
        // bounded metadata commit, never the mutation itself.
        for _ in 0..20 {
            match clock()
                .and_then(|now| Queue::open(&self.root, now))
                .and_then(|mut q| q.finish(&id, &result))
            {
                Ok(()) => break,
                Err(error) if error.code == ErrorCode::OperationBusy => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(_) => break,
            }
        }
    }
}
struct ActiveWorker {
    handle: JoinHandle<()>,
    cancel: runtime::CancelToken,
}
fn workers() -> AppResult<std::sync::MutexGuard<'static, Vec<ActiveWorker>>> {
    static WORKERS: OnceLock<Mutex<Vec<ActiveWorker>>> = OnceLock::new();
    let mut handles = WORKERS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .map_err(|_| AppError::internal("Agent mutation workers are unavailable."))?;
    handles.retain(|w| !w.handle.is_finished());
    Ok(handles)
}
/// Closing a transport cancels its workers and lets existing rollback complete
/// before the MCP process exits. Killing the process still needs launcher recovery.
pub fn finish_transport_workers() {
    let handles = workers()
        .map(|mut w| std::mem::take(&mut *w))
        .unwrap_or_default();
    for worker in &handles {
        worker.cancel.cancel();
    }
    for worker in handles {
        let _ = worker.handle.join();
    }
}

#[tauri::command]
pub fn agent_mutation_requests(window: tauri::WebviewWindow) -> AppResult<Vec<MutationRequest>> {
    crate::commands::require_launcher(&window)?;
    AgentRequests::open_local()?.list_for_owner()
}
#[tauri::command]
pub fn agent_mutation_decide(
    window: tauri::WebviewWindow,
    request_id: String,
    approve: bool,
    consent: bool,
) -> AppResult<MutationRequest> {
    crate::commands::require_launcher(&window)?;
    AgentRequests::open_local()?.decide_for_owner(&request_id, approve, consent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "local-store-requests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn target() -> MutationTarget {
        MutationTarget {
            kind: MutationKind::Install,
            app_id: "memos".into(),
            display_name: "Memos".into(),
            fingerprint: hash(b"recipe"),
            engine: EngineBinding::managed_wsl(),
            engine_fingerprint: hash(b"owned engine"),
        }
    }
    #[test]
    fn authenticated_request_lookup_rechecks_revocation_and_reenrollment_generation() {
        let root = root();
        let gateway = AgentGateway::open(&root.join("policy"), &root.join("auth")).unwrap();
        let secret = gateway.enroll_client_for_owner("client").unwrap();
        let other = gateway.enroll_client_for_owner("other").unwrap();
        let (_, generation) = gateway.authenticate_mutation(&secret).unwrap();
        let service = AgentRequests {
            root: root.join("queue"),
            gateway: gateway.clone(),
        };
        let now = clock().unwrap();
        let request = Queue::open(&service.root, now)
            .unwrap()
            .submit("client", &generation, target(), now)
            .unwrap();
        assert_eq!(
            service.status(&secret, &request.id).unwrap().client_id,
            "client"
        );
        assert_eq!(
            service.status(&other, &request.id).unwrap_err().code,
            ErrorCode::Forbidden
        );
        gateway.revoke_client_for_owner("client").unwrap();
        assert_eq!(
            service.status(&secret, &request.id).unwrap_err().code,
            ErrorCode::Forbidden
        );
        let replacement = gateway.enroll_client_for_owner("client").unwrap();
        assert_eq!(
            service.status(&replacement, &request.id).unwrap_err().code,
            ErrorCode::Forbidden
        );
        // Owner denial is still available after rotation and does not depend
        // on any agent-supplied identity, grant or approval token.
        assert_eq!(
            service
                .decide_for_owner(&request.id, false, false)
                .unwrap_err()
                .code,
            ErrorCode::Forbidden
        );
        assert_eq!(
            service
                .decide_for_owner(&request.id, false, true)
                .unwrap()
                .state,
            MutationState::Denied
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn approval_is_exact_one_use_expiring_and_cross_client_denied() {
        let root = root();
        let mut queue = Queue::open(&root, 100).unwrap();
        let generation = hash(b"credential generation");
        let t = target();
        let r = queue.submit("client", &generation, t.clone(), 100).unwrap();
        assert!(queue
            .claim(&r.id, "client", &generation, &t, 1, 101)
            .is_err());
        queue.decide(&r.id, &generation, &t, true, 101).unwrap();
        assert!(queue
            .claim(&r.id, "other", &generation, &t, 1, 102)
            .is_err());
        assert!(queue
            .claim(&r.id, "client", &hash(b"rotated"), &t, 1, 102)
            .is_err());
        let mut changed = t.clone();
        changed.fingerprint = hash(b"changed recipe");
        assert!(queue
            .claim(&r.id, "client", &generation, &changed, 1, 102)
            .is_err());
        changed = t.clone();
        changed.engine_fingerprint = hash(b"replacement engine");
        assert!(queue
            .claim(&r.id, "client", &generation, &changed, 1, 102)
            .is_err());
        queue
            .claim(&r.id, "client", &generation, &t, 1, 102)
            .unwrap();
        assert!(queue
            .claim(&r.id, "client", &generation, &t, 1, 103)
            .is_err());
        let expiring = queue.submit("client", &generation, t.clone(), 103).unwrap();
        queue
            .decide(&expiring.id, &generation, &t, true, 103)
            .unwrap();
        drop(queue);
        let mut queue = Queue::open(&root, 703).unwrap();
        assert_eq!(
            queue.row(&expiring.id).unwrap().request.state,
            MutationState::Expired
        );
        assert!(queue
            .claim(&expiring.id, "client", &generation, &t, 1, 703)
            .is_err());
        drop(queue);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn owner_decision_checks_generation_identity_and_denial() {
        let root = root();
        let mut queue = Queue::open(&root, 100).unwrap();
        let generation = hash(b"credential generation");
        let t = target();
        let r = queue.submit("client", &generation, t.clone(), 100).unwrap();
        assert!(queue
            .decide(&r.id, &hash(b"rotated"), &t, true, 101)
            .is_err());
        let mut changed = t.clone();
        changed.fingerprint = hash(b"new recipe");
        assert!(queue
            .decide(&r.id, &generation, &changed, true, 101)
            .is_err());
        queue.decide(&r.id, &generation, &t, false, 101).unwrap();
        assert!(queue
            .claim(&r.id, "client", &generation, &t, 1, 102)
            .is_err());
        drop(queue);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_scopes_and_terminal_result_are_truthful() {
        let root = root();
        let mut queue = Queue::open(&root, 100).unwrap();
        let generation = hash(b"credential generation");
        let t = target();
        let r = queue.submit("client", &generation, t.clone(), 100).unwrap();
        queue.decide(&r.id, &generation, &t, true, 101).unwrap();
        queue
            .claim(&r.id, "client", &generation, &t, 9, 102)
            .unwrap();
        assert!(queue.cancel(&r.id, "other", &generation).is_err());
        assert!(queue.cancel(&r.id, "client", &hash(b"rotated")).is_err());
        assert!(
            queue
                .cancel(&r.id, "client", &generation)
                .unwrap()
                .cancel_requested
        );
        // Cancellation after the existing install commit cutoff must not turn
        // an already registered app into a false cancelled operation.
        queue.finish(&r.id, &Ok(())).unwrap();
        assert_eq!(
            queue.row(&r.id).unwrap().request.state,
            MutationState::Succeeded
        );
        let other = queue.submit("client", &generation, t, 100).unwrap();
        assert_eq!(
            queue
                .cancel(&other.id, "client", &generation)
                .unwrap()
                .state,
            MutationState::Cancelled
        );
        drop(queue);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn persistence_is_locked_bounded_versioned_and_contains_no_secret() {
        let root = root();
        let generation = hash(b"this bearer is not saved");
        let t = target();
        let mut queue = Queue::open(&root, 100).unwrap();
        assert_eq!(
            Queue::open(&root, 100).err().unwrap().code,
            ErrorCode::OperationBusy
        );
        for _ in 0..8 {
            queue.submit("client", &generation, t.clone(), 100).unwrap();
        }
        assert!(queue.submit("client", &generation, t, 100).is_err());
        drop(queue);
        let text = fs::read_to_string(root.join("agent-requests-v1.json")).unwrap();
        assert!(!text.contains("this bearer is not saved"));
        assert_eq!(Queue::open(&root, 101).unwrap().rows.len(), 8);
        fs::write(
            root.join("agent-requests-v1.json"),
            r#"{"version":99,"rows":[]}"#,
        )
        .unwrap();
        assert_eq!(
            Queue::open(&root, 101).err().unwrap().code,
            ErrorCode::StorageCorrupt
        );
        fs::remove_dir_all(root).unwrap();
    }
}

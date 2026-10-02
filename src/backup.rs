//! Bounded, quiesced app snapshots. This is an owner API, not an agent tool.
//!
//! Shared host folders and setup-bearing templates fail closed. Bind data and
//! reviewed named volumes are captured only after all app services quiesce.
use crate::{
    error::{AppError, AppResult, ErrorCode},
    model::{InstalledApp, RuntimeSpec},
    plan::{DeploymentPlan, PlanMount},
    runtime::{self, engine, CommandSpec, ProcessRunner, DIAGNOSTIC_TIMEOUT},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

mod protection;
mod volumes;

const MAGIC: &[u8] = b"LOCALSTORE-DPAPI-SNAPSHOT-1\n";
const MAX_DATA_BYTES: usize = 256 * 1024 * 1024;
const MAX_ENCODED_BYTES: usize = MAX_DATA_BYTES + 8 * 1024 * 1024;
const MAX_FILES: usize = 4096;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StorageContract {
    pub offering_id: String,
    pub reviewed_plan_sha256: String,
    pub bind_directories: Vec<String>,
    pub named_volumes: Vec<String>,
    pub has_shared_host_folders: bool,
}

impl StorageContract {
    pub fn from_plan(plan: &DeploymentPlan) -> AppResult<Self> {
        plan.validate().map_err(AppError::invalid)?;
        let mut directories = BTreeSet::new();
        let mut shared = false;
        for mount in plan.services.iter().flat_map(|service| &service.mounts) {
            match mount {
                PlanMount::Directory { source, .. } => {
                    if !safe_relative(source) {
                        return Err(AppError::invalid("Invalid snapshot bind path."));
                    }
                    directories.insert(source.clone());
                }
                PlanMount::Host { .. } => shared = true,
                PlanMount::Volume { .. } => {}
            }
        }
        let bind_directories: Vec<_> = directories.into_iter().collect();
        if bind_directories.iter().enumerate().any(|(i, directory)| {
            bind_directories
                .iter()
                .skip(i + 1)
                .any(|other| Path::new(other).starts_with(directory))
        }) {
            return Err(AppError::invalid(
                "Overlapping snapshot bind directories are unsupported.",
            ));
        }
        Ok(Self {
            offering_id: plan.id.clone(),
            reviewed_plan_sha256: hash(plan.to_compose().map_err(AppError::invalid)?.as_bytes()),
            bind_directories,
            named_volumes: plan.named_volumes.clone(),
            has_shared_host_folders: shared,
        })
    }

    pub fn supported(&self) -> AppResult<()> {
        if self.has_shared_host_folders {
            return Err(AppError::new(ErrorCode::UnsupportedOperation,
                "Shared host folders are outside this snapshot. No complete backup can be promised for this app yet."));
        }
        if self.bind_directories.is_empty() && self.named_volumes.is_empty() {
            return Err(AppError::invalid("No reviewed app data to snapshot."));
        }
        Ok(())
    }
}

// Private payload intentionally has no Debug implementation. Neither file
// contents nor stored credentials belong in logs, discovery or receipts.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema_version: u32,
    contract: StorageContract,
    engine: engine::EngineBinding,
    daemon_id_sha256: String,
    files: Vec<Entry>,
    volumes: Vec<VolumeEntry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VolumeEntry {
    name: String,
    sha256: String,
    #[serde(skip)]
    bytes: Vec<u8>,
    #[serde(default)]
    length: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    directory: bool,
    sha256: String,
    #[serde(skip)]
    bytes: Vec<u8>,
    #[serde(default)]
    length: usize,
}

#[derive(Debug, Serialize)]
pub struct SnapshotReceipt {
    pub offering_id: String,
    pub file_count: usize,
    pub data_bytes: usize,
    pub named_volume_count: usize,
    pub protection: &'static str,
    pub app_resumed: bool,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && !path
            .split('/')
            .any(|part| part.is_empty() || part.ends_with([' ', '.']))
        && !path.split('/').any(|part| {
            let stem = part
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || stem
                    .strip_prefix("COM")
                    .or_else(|| stem.strip_prefix("LPT"))
                    .is_some_and(|tail| {
                        tail.len() == 1 && matches!(tail.as_bytes()[0], b'1'..=b'9')
                    })
        })
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn confined_path(root: &Path, relative: &str) -> AppResult<PathBuf> {
    if !safe_relative(relative) {
        return Err(AppError::new(
            ErrorCode::UnsafePath,
            "Snapshot path is not confined.",
        ));
    }
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                reject_link(&path)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(path)
}

fn reject_link(path: &Path) -> AppResult<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(AppError::new(
                ErrorCode::UnsafePath,
                "Snapshot paths cannot be Windows reparse points.",
            ));
        }
    }
    if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
        return Err(AppError::new(
            ErrorCode::UnsafePath,
            "Snapshot paths must be ordinary files or directories.",
        ));
    }
    Ok(metadata)
}

fn admitted(app: &InstalledApp, managed_root: &Path) -> AppResult<(PathBuf, StorageContract)> {
    let RuntimeSpec::Compose {
        project_dir,
        compose_file,
        project_name,
    } = &app.runtime
    else {
        return Err(AppError::invalid(
            "Connected apps do not own Local Store storage.",
        ));
    };
    if !crate::model::is_valid_installed_app_id(&app.id) {
        return Err(AppError::invalid("Invalid snapshot app ID."));
    }
    let id = app.catalog_id.as_deref().unwrap_or(&app.id);
    let offering = crate::offerings::offering(id)
        .ok_or_else(|| AppError::invalid("Snapshot offering is no longer reviewed."))?;
    let template = offering.plan_template(None)?;
    // The first slice cannot safely reconstruct user setup or externally
    // provisioned database/authentication state from a resolved Compose file.
    if !template.fields.is_empty() {
        return Err(AppError::new(
            ErrorCode::UnsupportedOperation,
            "This app needs a separately proven backup adapter.",
        ));
    }
    let contract = StorageContract::from_plan(&template.plan)?;
    contract.supported()?;
    let expected = managed_root.canonicalize()?.join(&app.id);
    reject_link(&expected)?;
    if project_dir.canonicalize()? != expected
        || compose_file.canonicalize()? != expected.join("compose.yaml")
        || project_name != &format!("local-store-{}", app.id)
    {
        return Err(AppError::new(
            ErrorCode::UnsafePath,
            "Snapshot runtime is outside this app's managed project.",
        ));
    }
    reject_link(compose_file)?;
    let mut compose_bytes = Vec::new();
    fs::File::open(compose_file)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut compose_bytes)?;
    if compose_bytes.len() > 1024 * 1024 {
        return Err(AppError::invalid("Snapshot Compose exceeds its bound."));
    }
    let compose = String::from_utf8(compose_bytes)
        .map_err(|_| AppError::invalid("Snapshot Compose is not UTF-8."))?;
    let content = compose
        .strip_prefix(engine::COMPOSE_BINDING_MARKER)
        .ok_or_else(|| AppError::invalid("Snapshot requires a saved managed-engine binding."))?;
    let port = crate::plan::published_host_port(content)
        .ok_or_else(|| AppError::invalid("Snapshot Compose port is missing."))?;
    let mut live = offering.plan_template(Some(port))?;
    live.plan.id = app.id.clone();
    for (service, port) in crate::plan::companion_host_ports(content) {
        live.plan.set_companion_host(&service, port);
    }
    crate::setup::fill_platform_values(&mut live.plan, port);
    let credentials_path = expected.join(runtime::SECRETS_FILE);
    let secrets = if credentials_path.exists() {
        reject_link(&credentials_path)?;
        let mut bytes = Vec::new();
        fs::File::open(&credentials_path)?
            .take(65537)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err(AppError::invalid(
                "Snapshot credentials exceed their bound.",
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::invalid("Snapshot credentials are malformed."))?
    } else {
        Default::default()
    };
    let resolved = live
        .resolve(&Default::default(), &secrets)
        .map_err(AppError::invalid)?;
    if content != resolved.to_compose().map_err(AppError::invalid)? {
        return Err(AppError::invalid(
            "Stored Compose differs from its reviewed recipe; nothing was stopped.",
        ));
    }
    Ok((expected, contract))
}

fn owned_engine(
    runner: &dyn ProcessRunner,
    project: &Path,
) -> AppResult<(engine::EngineBinding, String)> {
    let binding =
        engine::retained(project)?.ok_or_else(|| AppError::invalid("No saved engine binding."))?;
    if !binding.is_wsl() || crate::engine_setup::selected(runner)? != Some(binding.clone()) {
        return Err(AppError::invalid(
            "Snapshot requires the verified, selected Local Store engine.",
        ));
    }
    let command = binding.command(&CommandSpec::new(
        "docker",
        vec!["info".into(), "--format".into(), "{{.ID}}".into()],
        None,
        DIAGNOSTIC_TIMEOUT,
    ))?;
    let out = runner.run(&command)?;
    let id = out.stdout.trim();
    if !out.success
        || out.truncated
        || id.is_empty()
        || id.len() > 256
        || id.chars().any(char::is_control)
    {
        return Err(AppError::invalid(
            "Snapshot engine identity could not be verified.",
        ));
    }
    Ok((binding, hash(id.as_bytes())))
}

pub(crate) fn verify_containers(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
) -> AppResult<crate::recovery::OwnershipStatus> {
    let RuntimeSpec::Compose {
        compose_file,
        project_name,
        project_dir,
    } = &app.runtime
    else {
        return Err(AppError::invalid("App is not managed."));
    };
    let query = |args| -> AppResult<String> {
        let command = engine::project_command(
            project_dir,
            CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT),
        )?;
        let out = runner.run(&command)?;
        if !out.success || out.truncated {
            return Err(AppError::invalid(
                "Snapshot container inventory is incomplete.",
            ));
        }
        Ok(out.stdout)
    };
    let ids = query(vec![
        "container".into(),
        "ls".into(),
        "--all".into(),
        "--quiet".into(),
        "--no-trunc".into(),
        "--filter".into(),
        format!("label=com.docker.compose.project={project_name}"),
    ])?;
    let ids: Vec<_> = ids.split_whitespace().collect();
    if ids.is_empty() {
        return Ok(crate::recovery::OwnershipStatus::NoContainers);
    }
    if ids.len() > 32
        || ids
            .iter()
            .any(|id| id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(AppError::invalid("Snapshot container IDs are invalid."));
    }
    let offering = crate::offerings::offering(app.catalog_id.as_deref().unwrap_or(&app.id))
        .ok_or_else(|| AppError::invalid("Snapshot offering is not reviewed."))?;
    let plan = offering.plan_template(None)?.plan;
    let services: BTreeSet<_> = plan.services.iter().map(|s| s.name.as_str()).collect();
    let mut args = vec![
        "container".into(),
        "inspect".into(),
        "--format".into(),
        "{{json .Config.Labels}}".into(),
    ];
    args.extend(ids.iter().map(|id| (*id).to_owned()));
    let labels = query(args)?;
    let mut seen = BTreeSet::new();
    for row in labels.lines() {
        let labels: std::collections::BTreeMap<String, String> = serde_json::from_str(row)
            .map_err(|_| AppError::invalid("Snapshot ownership labels are malformed."))?;
        let get = |key: &str| labels.get(key).map(String::as_str).unwrap_or_default();
        let service = get("com.docker.compose.service");
        if get("com.docker.compose.project") != project_name
            || !services.contains(service)
            || !seen.insert(service.to_owned())
            || !get("com.docker.compose.oneoff").eq_ignore_ascii_case("false")
            || !engine::ownership_paths_match(
                project_dir,
                compose_file,
                get("com.docker.compose.project.config_files"),
                get("com.docker.compose.project.working_dir"),
            )?
        {
            return Err(AppError::new(
                ErrorCode::Forbidden,
                "Snapshot container ownership does not match this managed project.",
            ));
        }
    }
    if seen.len() != ids.len() || seen.len() != services.len() {
        return Err(AppError::invalid(
            "Snapshot inventory omits a required service.",
        ));
    }
    let mut state_args = vec![
        "container".into(),
        "inspect".into(),
        "--format".into(),
        "{{.State.Status}} {{.State.ExitCode}}".into(),
    ];
    state_args.extend(ids.iter().map(|id| (*id).to_owned()));
    let states = query(state_args)?;
    let states: Vec<_> = states.lines().collect();
    if states.len() != ids.len()
        || !(states.iter().all(|state| *state == "running 0")
            || states
                .iter()
                .all(|state| matches!(*state, "created 0" | "exited 0")))
    {
        return Err(AppError::invalid("Snapshot requires all app services running or cleanly stopped; paused, restarting or mixed states need review."));
    }
    Ok(crate::recovery::OwnershipStatus::Verified)
}

fn collect(
    root: &Path,
    relative: &str,
    entries: &mut Vec<Entry>,
    total: &mut usize,
    depth: usize,
) -> AppResult<()> {
    if depth > MAX_DEPTH || entries.len() >= MAX_FILES || !safe_relative(relative) {
        return Err(AppError::invalid(
            "Snapshot exceeds its path, depth or file limit.",
        ));
    }
    let path = confined_path(root, relative)?;
    let metadata = reject_link(&path)?;
    if metadata.is_dir() {
        entries.push(Entry {
            path: relative.into(),
            directory: true,
            sha256: hash(&[]),
            bytes: Vec::new(),
            length: 0,
        });
        let mut children: Vec<_> = fs::read_dir(&path)?.collect::<Result<_, _>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| AppError::invalid("Snapshot filename must be Unicode."))?;
            collect(
                root,
                &format!("{relative}/{name}"),
                entries,
                total,
                depth + 1,
            )?;
        }
    } else {
        if metadata.len() > (MAX_DATA_BYTES - *total) as u64 {
            return Err(AppError::invalid(
                "Snapshot exceeds the 256 MiB development limit.",
            ));
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)?
            .take((MAX_DATA_BYTES - *total + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_DATA_BYTES - *total {
            return Err(AppError::invalid(
                "Snapshot data changed or exceeds its limit.",
            ));
        }
        *total += bytes.len();
        entries.push(Entry {
            path: relative.into(),
            directory: false,
            sha256: hash(&bytes),
            length: bytes.len(),
            bytes,
        });
    }
    Ok(())
}

/// Stop the owned app, prove no containers remain running, collect every
/// reviewed bind and retained credential, then resume only if it was running.
/// The returned bytes are already encrypted for the current Windows account.
pub fn snapshot_with(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    managed_root: &Path,
    consent: bool,
) -> AppResult<(Vec<u8>, SnapshotReceipt)> {
    if !consent {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "Approve pausing this app and creating an encrypted snapshot first.",
        ));
    }
    let _operation = runtime::lock_operation(&app.id)?;
    let (project, contract) = admitted(app, managed_root)?;
    let (binding, daemon_id_sha256) = owned_engine(runner, &project)?;
    if verify_containers(runner, app)? != crate::recovery::OwnershipStatus::Verified {
        return Err(AppError::invalid(
            "Snapshot requires this installed app's owned containers.",
        ));
    }
    let was_running = runtime::status_with(runner, app)? == runtime::AppStatus::Running;
    if was_running {
        if let Err(error) = runtime::stop_with(runner, app) {
            return match runtime::start_with(runner, app) {
                Ok(()) => Err(error),
                Err(resume) => Err(AppError::rollback(error, resume)),
            };
        }
    }
    let captured = (|| {
        if runtime::status_with(runner, app)? != runtime::AppStatus::Stopped {
            return Err(AppError::invalid(
                "App did not quiesce; no snapshot was created.",
            ));
        }
        verify_containers(runner, app)?;
        let mut files = Vec::new();
        let mut total = 0;
        for relative in &contract.bind_directories {
            collect(&project, relative, &mut files, &mut total, 0)?;
        }
        let credentials = project.join(runtime::SECRETS_FILE);
        if credentials.exists() {
            collect(&project, runtime::SECRETS_FILE, &mut files, &mut total, 0)?;
        }
        let volumes = volumes::capture(runner, app, &project, &contract.named_volumes, &mut total)?;
        let receipt = SnapshotReceipt {
            offering_id: contract.offering_id.clone(),
            file_count: files.iter().filter(|entry| !entry.directory).count(),
            data_bytes: total,
            named_volume_count: volumes.len(),
            protection: "windows_current_user_dpapi",
            app_resumed: was_running,
        };
        let snapshot = Snapshot {
            schema_version: 1,
            contract,
            engine: binding,
            daemon_id_sha256,
            files,
            volumes,
        };
        let mut encoded = encode(&snapshot)?;
        let protected = protection::protect(&encoded);
        encoded.fill(0);
        let protected = protected?;
        let mut output = MAGIC.to_vec();
        output.extend(protected);
        Ok((output, receipt))
    })();
    if was_running {
        if let Err(error) = runtime::start_with(runner, app) {
            return Err(AppError::rollback(
                "Snapshot capture completed or failed",
                error,
            ));
        }
    }
    captured
}

fn decode(bytes: &[u8]) -> AppResult<Snapshot> {
    if bytes.len() > MAX_ENCODED_BYTES || !bytes.starts_with(MAGIC) {
        return Err(AppError::invalid(
            "Invalid or oversized protected snapshot.",
        ));
    }
    let mut plain = protection::unprotect(&bytes[MAGIC.len()..])?;
    if plain.len() > MAX_ENCODED_BYTES {
        return Err(AppError::invalid("Decrypted snapshot exceeds its limit."));
    }
    let snapshot = decode_plain(&plain);
    plain.fill(0);
    let snapshot = snapshot?;
    validate_entries(&snapshot)?;
    Ok(snapshot)
}

fn validate_entries(snapshot: &Snapshot) -> AppResult<()> {
    snapshot.contract.supported()?;
    if snapshot.schema_version != 1 || snapshot.files.len() > MAX_FILES || !snapshot.engine.is_wsl()
    {
        return Err(AppError::invalid("Unsupported snapshot format."));
    }
    let mut paths = BTreeSet::new();
    let mut total = 0usize;
    for entry in &snapshot.files {
        let admitted = entry.path == runtime::SECRETS_FILE
            || snapshot
                .contract
                .bind_directories
                .iter()
                .any(|root| Path::new(&entry.path).starts_with(root));
        total = total
            .checked_add(entry.bytes.len())
            .ok_or_else(|| AppError::invalid("Snapshot size overflow."))?;
        if !safe_relative(&entry.path)
            || !admitted
            || !paths.insert(entry.path.to_lowercase())
            || entry.sha256 != hash(&entry.bytes)
            || (entry.directory && !entry.bytes.is_empty())
            || total > MAX_DATA_BYTES
        {
            return Err(AppError::invalid(
                "Snapshot has an unsafe, duplicate or corrupt entry.",
            ));
        }
    }
    for directory in &snapshot.contract.bind_directories {
        if !snapshot
            .files
            .iter()
            .any(|entry| entry.path == *directory && entry.directory)
        {
            return Err(AppError::invalid(
                "Snapshot omitted a required bind directory.",
            ));
        }
    }
    let mut volumes = BTreeSet::new();
    for volume in &snapshot.volumes {
        total = total
            .checked_add(volume.bytes.len())
            .ok_or_else(|| AppError::invalid("Snapshot size overflow."))?;
        if !snapshot.contract.named_volumes.contains(&volume.name)
            || !volumes.insert(&volume.name)
            || volume.sha256 != hash(&volume.bytes)
            || total > MAX_DATA_BYTES
        {
            return Err(AppError::invalid(
                "Snapshot volume is missing, duplicated or corrupt.",
            ));
        }
        volumes::validate_tar(&volume.bytes)?;
    }
    if volumes.len() != snapshot.contract.named_volumes.len() {
        return Err(AppError::invalid(
            "Snapshot omitted a required named volume.",
        ));
    }
    for entry in &snapshot.files {
        if snapshot.files.iter().any(|other| {
            !other.directory
                && other.path != entry.path
                && Path::new(&entry.path).starts_with(&other.path)
        }) {
            return Err(AppError::invalid(
                "Snapshot has a file where a parent directory is required.",
            ));
        }
    }
    Ok(())
}

/// Restore data only into an existing reviewed project with no containers and
/// empty bind directories. Never overwrite an installed database, credentials,
/// Compose, engine binding or unrelated file. Caller starts/installs separately.
pub fn restore_into_fresh_with(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    managed_root: &Path,
    protected: &[u8],
    consent: bool,
) -> AppResult<SnapshotReceipt> {
    if !consent {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "Approve restoring this snapshot into a fresh isolated app first.",
        ));
    }
    let _operation = runtime::lock_operation(&app.id)?;
    let snapshot = decode(protected)?;
    let (project, contract) = admitted(app, managed_root)?;
    let (binding, daemon) = owned_engine(runner, &project)?;
    if contract != snapshot.contract
        || binding != snapshot.engine
        || daemon != snapshot.daemon_id_sha256
    {
        return Err(AppError::invalid(
            "Snapshot plan or managed engine differs from this fresh install.",
        ));
    }
    if verify_containers(runner, app)? != crate::recovery::OwnershipStatus::NoContainers {
        return Err(AppError::invalid(
            "Restore requires a fresh project with no containers.",
        ));
    }
    volumes::admit_restore(runner, app, &snapshot.volumes)?;
    restore_files(&project, &snapshot)?;
    volumes::restore(runner, app, &project, &snapshot.volumes)?;
    Ok(SnapshotReceipt {
        offering_id: contract.offering_id,
        file_count: snapshot
            .files
            .iter()
            .filter(|entry| !entry.directory)
            .count(),
        data_bytes: snapshot
            .files
            .iter()
            .map(|entry| entry.bytes.len())
            .sum::<usize>()
            + snapshot
                .volumes
                .iter()
                .map(|entry| entry.bytes.len())
                .sum::<usize>(),
        named_volume_count: snapshot.volumes.len(),
        protection: "windows_current_user_dpapi",
        app_resumed: false,
    })
}

fn encode(snapshot: &Snapshot) -> AppResult<Vec<u8>> {
    let metadata = serde_json::to_vec(snapshot).map_err(AppError::internal)?;
    if metadata.len() > 1024 * 1024 {
        return Err(AppError::invalid("Snapshot metadata exceeds its limit."));
    }
    let mut output = (metadata.len() as u32).to_le_bytes().to_vec();
    output.extend(metadata);
    for file in &snapshot.files {
        output.extend(&file.bytes);
    }
    for volume in &snapshot.volumes {
        output.extend(&volume.bytes);
    }
    if output.len() > MAX_ENCODED_BYTES {
        return Err(AppError::invalid("Snapshot encoding exceeds its limit."));
    }
    Ok(output)
}

fn decode_plain(bytes: &[u8]) -> AppResult<Snapshot> {
    if bytes.len() < 4 {
        return Err(AppError::invalid("Snapshot framing is incomplete."));
    }
    let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    if length > 1024 * 1024 || length > bytes.len() - 4 {
        return Err(AppError::invalid("Snapshot metadata framing is invalid."));
    }
    let mut snapshot: Snapshot = serde_json::from_slice(&bytes[4..4 + length])
        .map_err(|_| AppError::invalid("Snapshot metadata is malformed."))?;
    if snapshot.files.len() > MAX_FILES || snapshot.volumes.len() > 16 {
        return Err(AppError::invalid("Snapshot entry count exceeds its bound."));
    }
    let mut offset = 4 + length;
    for file in &mut snapshot.files {
        if file.length > bytes.len() - offset {
            return Err(AppError::invalid("Snapshot file framing is incomplete."));
        }
        file.bytes = bytes[offset..offset + file.length].to_vec();
        offset += file.length;
    }
    for volume in &mut snapshot.volumes {
        if volume.length > bytes.len() - offset {
            return Err(AppError::invalid("Snapshot volume framing is incomplete."));
        }
        volume.bytes = bytes[offset..offset + volume.length].to_vec();
        offset += volume.length;
    }
    if offset != bytes.len() {
        return Err(AppError::invalid(
            "Snapshot contains unexpected trailing bytes.",
        ));
    }
    Ok(snapshot)
}

fn restore_files(project: &Path, snapshot: &Snapshot) -> AppResult<()> {
    validate_entries(snapshot)?;
    // Admission is complete before the first write. An interrupted restore
    // leaves partial *new* data, which makes retries refuse; inspect/recreate
    // the isolated target rather than merge with a potentially partial DB.
    for directory in &snapshot.contract.bind_directories {
        let target = confined_path(project, directory)?;
        if target.exists()
            && (!reject_link(&target)?.is_dir() || fs::read_dir(&target)?.next().is_some())
        {
            return Err(AppError::invalid(
                "Restore target contains data; nothing was overwritten.",
            ));
        }
    }
    if project.join(runtime::SECRETS_FILE).exists() {
        return Err(AppError::invalid(
            "Restore target already contains credentials; nothing was overwritten.",
        ));
    }
    for entry in &snapshot.files {
        confined_path(project, &entry.path)?;
    }
    let mut entries: Vec<_> = snapshot.files.iter().collect();
    entries.sort_by_key(|entry| (entry.path.matches('/').count(), !entry.directory));
    for entry in entries {
        let target = confined_path(project, &entry.path)?;
        if entry.directory {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)?;
            file.write_all(&entry.bytes)?;
            file.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

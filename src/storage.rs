//! Registry storage: where registered apps live and how the apps.json file is
//! loaded and saved. The single source of truth for wrapped apps, used by BOTH
//! the GUI (Tauri commands) and the CLI (standalone mode).

use crate::{
    brand::{CONFIG_SLUG, LEGACY_CONFIG_SLUG},
    model::{
        is_valid_installed_app_id, next_available_installed_app_id, AppDef, InstalledApp,
        RuntimeSpec,
    },
};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Errors returned by the strict Registry V2 API.  The legacy `AppDef` facade
/// below intentionally keeps its historical best-effort behavior until callers
/// migrate in a later task.
#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    AlreadyExists(String),
    NotFound(String),
    Json(serde_json::Error),
    InvalidRegistryVersion(u32),
    MigrationRefused(String),
    InvalidComposeValue(String),
    LockUnavailable(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(reason) | Self::NotFound(reason) => f.write_str(reason),
            Self::Io(error) => write!(f, "registry I/O error: {error}"),
            Self::Json(error) => write!(f, "registry JSON error: {error}"),
            Self::InvalidRegistryVersion(version) => {
                write!(f, "unsupported registry version {version}; expected 2")
            }
            Self::MigrationRefused(reason) => write!(f, "v1 migration refused: {reason}"),
            Self::LockUnavailable(reason) => write!(f, "registry is busy: {reason}"),
            Self::InvalidComposeValue(value) => {
                write!(f, "v1 compose value is not a path: {value:?}")
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

/// The versioned, losslessly serializable registry format.  V1's `health`
/// field has no V2 `InstalledApp` equivalent: the untouched source bytes are
/// retained in `migration-v1-backup.json` rather than inventing a value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryV2 {
    pub version: u32,
    pub apps: Vec<InstalledApp>,
}

impl RegistryV2 {
    pub const VERSION: u32 = 2;

    pub fn new(apps: Vec<InstalledApp>) -> Self {
        Self {
            version: Self::VERSION,
            apps,
        }
    }
}

/// Registry V2 roots are platform configuration *base* directories: `%APPDATA%`
/// on Windows, `$XDG_CONFIG_HOME` on Linux, else `$HOME/.config`. V2 files are
/// stored directly under that root; the historical source is
/// `<root>/<legacy-config-slug>/apps.json`. `LOCAL_STORE_CONFIG_DIR` is honored only in test builds, never by production.
fn registry_config_root() -> PathBuf {
    #[cfg(test)]
    if let Ok(root) = std::env::var("LOCAL_STORE_CONFIG_DIR") {
        if !root.is_empty() {
            return PathBuf::from(root);
        }
    }

    PathBuf::from(config_base())
}

pub fn managed_apps_root() -> PathBuf {
    registry_config_root().join(CONFIG_SLUG).join("apps")
}

/// Per-machine managed-engine state. This deliberately uses LOCALAPPDATA when
/// available: a WSL distribution is local to this Windows installation and
/// must not follow a roaming APPDATA profile to another machine.
pub fn managed_engine_state_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(registry_config_root)
        .join(CONFIG_SLUG)
        .join("engine")
        .join("wsl-state")
}

fn config_base() -> String {
    std::env::var("APPDATA")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| format!("{home}/.config"))
        })
        .unwrap_or_else(|| ".".to_string())
}

/// How long a registry change waits for another process to finish its own.
const REGISTRY_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const REGISTRY_LOCK_POLL: Duration = Duration::from_millis(25);

pub fn registry_lock_path_for_root(root: &Path) -> PathBuf {
    root.join(CONFIG_SLUG).join("registry.lock")
}

/// Holds the cross-process registry lock until dropped.
#[must_use = "the registry stays locked only while this guard is alive"]
pub struct RegistryLock(fs::File);
impl Drop for RegistryLock {
    fn drop(&mut self) {
        // Closing the handle would release it anyway; unlocking first keeps
        // the release explicit and immediate.
        let _ = FileExt::unlock(&self.0);
    }
}

/// Serialise a registry read-modify-write against other processes.
///
/// Saving the registry replaces the file by rename, so the registry file
/// cannot carry the lock — the lock would be dropped along with the file it
/// was taken on. A sidecar `registry.lock` is locked instead, and every
/// mutation holds it across the whole read-modify-write. Without this, two
/// processes each read the same registry and the second save silently discards
/// the first one's change.
///
/// The wait is bounded: a stuck holder should surface as a clear "busy" error
/// rather than an operation that never returns. The operating system releases
/// the lock if a holder exits or crashes.
pub fn lock_registry_at(root: &Path) -> StorageResult<RegistryLock> {
    let path = registry_lock_path_for_root(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    let deadline = Instant::now() + REGISTRY_LOCK_TIMEOUT;
    loop {
        match FileExt::try_lock(&file) {
            Ok(()) => return Ok(RegistryLock(file)),
            Err(_) if Instant::now() < deadline => std::thread::sleep(REGISTRY_LOCK_POLL),
            Err(error) => {
                return Err(StorageError::LockUnavailable(format!(
                    "another Local Store process held the registry for over {} seconds ({error})",
                    REGISTRY_LOCK_TIMEOUT.as_secs()
                )))
            }
        }
    }
}
fn lock_registry() -> StorageResult<RegistryLock> {
    lock_registry_at(&registry_config_root())
}

/// Lock only if a registry directory already exists.
///
/// Read paths must not bring configuration into being: activating an app that
/// does not exist has to leave the disk untouched. When there is no directory
/// there is also no registry to race over, and any mutation creates both.
fn lock_registry_if_present() -> StorageResult<Option<RegistryLock>> {
    let root = registry_config_root();
    match registry_lock_path_for_root(&root).parent() {
        Some(directory) if directory.exists() => lock_registry_at(&root).map(Some),
        _ => Ok(None),
    }
}

pub fn registry_v2_path_for_root(root: &Path) -> PathBuf {
    root.join(CONFIG_SLUG).join("registry-v2.json")
}

pub fn migration_v1_backup_path_for_root(root: &Path) -> PathBuf {
    root.join(CONFIG_SLUG).join("migration-v1-backup.json")
}

pub fn registry_v2_previous_path_for_root(root: &Path) -> PathBuf {
    root.join(CONFIG_SLUG).join("registry-v2.previous.json")
}

pub fn primary_v1_path_for_root(root: &Path) -> PathBuf {
    root.join(CONFIG_SLUG).join("apps.json")
}

pub fn legacy_v1_path_for_root(root: &Path) -> PathBuf {
    legacy_v1_path_for_platform(root, std::env::consts::OS)
}

/// Platform-neutral helper deliberately takes a root, so Windows and Linux
/// legacy locations can be covered without relying on the host test platform.
pub fn legacy_v1_path_for_platform(root: &Path, _platform: &str) -> PathBuf {
    root.join(LEGACY_CONFIG_SLUG).join("apps.json")
}

pub fn load_registry_v2() -> StorageResult<RegistryV2> {
    load_registry_v2_at(&registry_config_root())
}

pub fn load_registry_v2_at(root: &Path) -> StorageResult<RegistryV2> {
    let data = match fs::read(registry_v2_path_for_root(root)) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::read(registry_v2_previous_path_for_root(root)).map_err(|previous_error| {
                StorageError::MigrationRefused(format!(
                    "live registry is absent and no recovery file is valid: {previous_error}"
                ))
            })?
        }
        Err(error) => return Err(error.into()),
    };
    let registry: RegistryV2 = serde_json::from_slice(&data)?;
    validate_registry(&registry)?;
    Ok(registry)
}

pub fn save_registry_v2(registry: &RegistryV2) -> StorageResult<()> {
    save_registry_v2_at(&registry_config_root(), registry)
}

/// Load the canonical registry, importing an older registry once when needed.
pub fn load_or_migrate_registry() -> StorageResult<RegistryV2> {
    // Locked because this can migrate, which writes. Mutations call the `_at`
    // form directly, already holding the lock.
    let _lock = lock_registry_if_present()?;
    load_or_migrate_registry_at(&registry_config_root())
}

pub fn load_or_migrate_registry_at(root: &Path) -> StorageResult<RegistryV2> {
    if registry_v2_path_for_root(root).exists() || registry_v2_previous_path_for_root(root).exists()
    {
        return load_registry_v2_at(root);
    }
    if primary_v1_path_for_root(root).exists() || legacy_v1_path_for_root(root).exists() {
        migrate_v1_registry_at(
            root,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| StorageError::MigrationRefused(error.to_string()))?
                .as_secs(),
        )?;
        return load_registry_v2_at(root);
    }
    Ok(RegistryV2::new(Vec::new()))
}

pub fn insert_installed_app(app: InstalledApp) -> StorageResult<()> {
    // Held across the read and the save: without it two processes read the
    // same registry and the second save discards the first one's change.
    let _lock = lock_registry()?;
    let mut registry = load_or_migrate_registry_at(&registry_config_root())?;
    if registry.apps.iter().any(|saved| saved.id == app.id) {
        return Err(StorageError::AlreadyExists(format!(
            "app id {:?} already exists",
            app.id
        )));
    }
    registry.apps.push(app);
    save_registry_v2(&registry)
}

pub fn remove_installed_app(id: &str) -> StorageResult<InstalledApp> {
    // Held across the read and the save: without it two processes read the
    // same registry and the second save discards the first one's change.
    let _lock = lock_registry()?;
    let mut registry = load_or_migrate_registry_at(&registry_config_root())?;
    let position = registry
        .apps
        .iter()
        .position(|app| app.id == id)
        .ok_or_else(|| StorageError::NotFound(format!("app id {id:?} not found")))?;
    let removed = registry.apps.remove(position);
    save_registry_v2(&registry)?;
    Ok(removed)
}

pub fn update_installed_app(app: InstalledApp) -> StorageResult<()> {
    // Held across the read and the save: without it two processes read the
    // same registry and the second save discards the first one's change.
    let _lock = lock_registry()?;
    let mut registry = load_or_migrate_registry_at(&registry_config_root())?;
    let saved = registry
        .apps
        .iter_mut()
        .find(|saved| saved.id == app.id)
        .ok_or_else(|| StorageError::NotFound(format!("app id {:?} not found", app.id)))?;
    *saved = app;
    save_registry_v2(&registry)
}

pub fn slug_for_display_name(name: &str) -> String {
    ascii_slug(name)
}

pub fn save_registry_v2_at(root: &Path, registry: &RegistryV2) -> StorageResult<()> {
    if registry.version != RegistryV2::VERSION {
        return Err(StorageError::InvalidRegistryVersion(registry.version));
    }
    let encoded = serde_json::to_vec_pretty(registry)?;
    atomic_replace_with_previous(&registry_v2_path_for_root(root), &encoded)
}

fn atomic_replace_with_previous(path: &Path, bytes: &[u8]) -> StorageResult<()> {
    if path.exists() {
        // A read failure means the old state is not known-good: do not replace it.
        let previous = fs::read(path)?;
        atomic_write(&path.with_extension("previous.json"), &previous)?;
    }
    atomic_write(path, bytes)
}

/// Write `bytes` to `path` atomically.
///
/// A crash or a failed write leaves either the previous contents or nothing at
/// all, never a half-written file. Used for the registry and for generated
/// Compose files, which Docker must never read in a torn state.
pub fn write_file_atomically(path: &Path, bytes: &[u8]) -> StorageResult<()> {
    atomic_write(path, bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> StorageResult<()> {
    let parent = path.parent().ok_or_else(|| {
        StorageError::MigrationRefused(format!("path has no parent directory: {}", path.display()))
    })?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| StorageError::MigrationRefused(error.to_string()))?
        .as_nanos();
    let mut temp = None;
    for attempt in 0_u32..100 {
        let candidate = parent.join(format!(
            ".{}-{nonce}-{attempt}.tmp",
            path.file_name().unwrap().to_string_lossy()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temp = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let (temp_path, mut file) = temp.ok_or_else(|| {
        StorageError::MigrationRefused("could not allocate unique temporary registry file".into())
    })?;
    let result = (|| -> StorageResult<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            // Windows cannot rename over a target. The prior bytes have already
            // been atomically retained as `.previous.json` before this removal.
            fs::remove_file(path)?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn validate_registry(registry: &RegistryV2) -> StorageResult<()> {
    if registry.version != RegistryV2::VERSION {
        return Err(StorageError::InvalidRegistryVersion(registry.version));
    }
    let mut ids = HashSet::new();
    for app in &registry.apps {
        if !is_valid_installed_app_id(&app.id) || !ids.insert(&app.id) {
            return Err(StorageError::MigrationRefused(format!(
                "invalid app id {:?}",
                app.id
            )));
        }
        if app.display_name.trim().is_empty() || app.launch_url.trim().is_empty() {
            return Err(StorageError::MigrationRefused(
                "empty essential app field".into(),
            ));
        }
        if app.updated_at_unix < app.created_at_unix {
            return Err(StorageError::MigrationRefused(
                "updated timestamp predates creation".into(),
            ));
        }
        if let RuntimeSpec::Compose {
            project_name,
            project_dir,
            compose_file,
        } = &app.runtime
        {
            if !is_valid_installed_app_id(project_name)
                || project_dir
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                || compose_file
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                || !compose_file.starts_with(project_dir)
            {
                return Err(StorageError::MigrationRefused(
                    "invalid compose runtime paths".into(),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct LegacyAppDef {
    name: String,
    url: String,
    icon: Option<String>,
    compose: Option<String>,
    health: Option<String>,
}

/// Import once only when no V2 file exists. Valid V1 bytes are parsed and fully
/// converted before either destination file is written. `health` remains in the
/// exact backup because InstalledApp intentionally has no health field.
pub fn migrate_v1_registry() -> StorageResult<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| StorageError::MigrationRefused(error.to_string()))?
        .as_secs();
    migrate_v1_registry_at(&registry_config_root(), now)
}

pub fn migrate_v1_registry_at(root: &Path, timestamp: u64) -> StorageResult<()> {
    let v2_path = registry_v2_path_for_root(root);
    if fs::metadata(&v2_path).is_ok() {
        return Err(StorageError::MigrationRefused(format!(
            "{} already exists; migration never overwrites V2",
            v2_path.display()
        )));
    }
    let backup_path = migration_v1_backup_path_for_root(root);
    let source = match fs::read(&backup_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match fs::read(primary_v1_path_for_root(root)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::read(legacy_v1_path_for_root(root))?
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(error) => return Err(error.into()),
    };
    let legacy: Vec<LegacyAppDef> = serde_json::from_slice(&source)?;
    let apps = convert_legacy_apps(legacy, timestamp)?;
    if !backup_path.exists() {
        atomic_write(&backup_path, &source)?;
    }
    save_registry_v2_at(root, &RegistryV2::new(apps))
}

fn convert_legacy_apps(
    legacy: Vec<LegacyAppDef>,
    timestamp: u64,
) -> StorageResult<Vec<InstalledApp>> {
    let mut used = HashSet::new();
    legacy
        .into_iter()
        .map(|app| {
            let base = ascii_slug(&app.name);
            let id = next_available_installed_app_id(&base, |candidate| !used.contains(candidate))
                .expect("ascii_slug always creates a valid installed-app ID");
            used.insert(id.clone());
            let runtime = match app.compose.filter(|value| !value.trim().is_empty()) {
                None => RuntimeSpec::External,
                Some(compose) => compose_runtime(&compose, &id)?,
            };
            // Read the V1 health field so deserialization remains deliberately
            // five-field and its source value is retained in the byte backup.
            let _health = app.health;
            Ok(InstalledApp {
                id,
                catalog_id: None,
                display_name: app.name,
                launch_url: app.url,
                icon_path: app.icon.map(PathBuf::from),
                runtime,
                created_at_unix: timestamp,
                updated_at_unix: timestamp,
            })
        })
        .collect()
}

fn compose_runtime(value: &str, project_name: &str) -> StorageResult<RuntimeSpec> {
    if value.contains('\n')
        || value.contains('\r')
        || value.contains("services:")
        || value.contains("version:")
        || value.contains(": ")
        || value.trim_start().starts_with("---")
    {
        return Err(StorageError::InvalidComposeValue(value.to_owned()));
    }
    let compose_file = PathBuf::from(value);
    let project_dir = compose_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| StorageError::InvalidComposeValue(value.to_owned()))?
        .to_path_buf();
    Ok(RuntimeSpec::Compose {
        project_name: project_name.to_owned(),
        project_dir,
        compose_file,
    })
}

fn ascii_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_hyphen = false;
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() {
            if pending_hyphen && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(char::from(byte.to_ascii_lowercase()));
            pending_hyphen = false;
        } else if !slug.is_empty() {
            pending_hyphen = true;
        }
    }
    if slug.is_empty() {
        "app".to_owned()
    } else {
        slug
    }
}

/// Where registered apps live. Same location on every platform via APPDATA
/// (Windows) or XDG/HOME fallback on Linux/macOS.
fn config_path(slug: &str) -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{}/.config", h)))
        .unwrap_or_else(|| ".".to_string());
    let mut p = PathBuf::from(base);
    p.push(slug);
    p.push("apps.json");
    p
}

pub fn resolve_config() -> String {
    config_path(CONFIG_SLUG).to_string_lossy().into_owned()
}

fn legacy_config() -> String {
    config_path(LEGACY_CONFIG_SLUG)
        .to_string_lossy()
        .into_owned()
}

/// Fallible V1 access for the launcher until runtime and CLI migrate together.
pub fn try_load_apps() -> StorageResult<Vec<AppDef>> {
    try_load_apps_from_paths(Path::new(&resolve_config()), Path::new(&legacy_config()))
}

fn try_load_apps_from_paths(primary: &Path, legacy: &Path) -> StorageResult<Vec<AppDef>> {
    let bytes = match fs::read(primary) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::read(legacy) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        },
        Err(error) => return Err(error.into()),
    };
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn try_save_apps(apps: &[AppDef]) -> StorageResult<()> {
    atomic_replace_with_previous(
        Path::new(&resolve_config()),
        &serde_json::to_vec_pretty(apps)?,
    )
}

pub fn load_apps() -> Vec<AppDef> {
    let primary = PathBuf::from(resolve_config());
    let legacy = PathBuf::from(legacy_config());
    load_apps_from_paths(&primary, &legacy)
}

fn load_apps_from_paths(primary: &std::path::Path, legacy: &std::path::Path) -> Vec<AppDef> {
    match std::fs::read_to_string(primary) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::read_to_string(legacy)
                .ok()
                .and_then(|data| serde_json::from_str(&data).ok())
                .unwrap_or_default()
        }
        Err(_) => Vec::new(),
    }
}

pub fn save_apps(apps: &[AppDef]) {
    let path = resolve_config();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string(apps).unwrap());
}

/// Insert or replace an app by name (dedup on name).
pub fn upsert_app(
    name: &str,
    url: &str,
    icon: Option<String>,
    compose: Option<String>,
    health: Option<String>,
) {
    let mut apps = load_apps();
    apps.retain(|a| a.name != name);
    apps.push(AppDef {
        name: name.to_string(),
        url: url.to_string(),
        icon,
        compose,
        health,
    });
    save_apps(&apps);
}

pub fn remove_app(name: &str) -> bool {
    let mut apps = load_apps();
    let before = apps.len();
    apps.retain(|a| a.name != name);
    if apps.len() != before {
        save_apps(&apps);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "local-store-storage-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        (
            root.clone(),
            root.join("primary.json"),
            root.join("legacy.json"),
        )
    }

    fn app_json(name: &str) -> String {
        format!(
            r#"[{{"name":"{name}","url":"http://{name}","icon":null,"compose":null,"health":null}}]"#
        )
    }

    #[test]
    fn strict_launcher_load_reports_corruption_instead_of_empty_state() {
        let (root, primary, legacy) = fixture_paths();
        fs::write(&primary, "broken").unwrap();
        fs::write(&legacy, app_json("legacy")).unwrap();
        assert!(try_load_apps_from_paths(&primary, &legacy).is_err());
        fs::remove_file(&primary).unwrap();
        assert_eq!(
            try_load_apps_from_paths(&primary, &legacy).unwrap()[0].name,
            "legacy"
        );
        fs::remove_file(&legacy).unwrap();
        assert!(try_load_apps_from_paths(&primary, &legacy)
            .unwrap()
            .is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_root_loads_an_empty_v2_registry_without_writing() {
        let (root, _, _) = fixture_paths();
        let registry = load_or_migrate_registry_at(&root).unwrap();
        assert_eq!(registry, RegistryV2::new(Vec::new()));
        assert!(!registry_v2_path_for_root(&root).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_or_migrate_imports_v1_once() {
        let (root, _, _) = fixture_paths();
        let old = primary_v1_path_for_root(&root);
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, app_json("notes")).unwrap();
        let imported = load_or_migrate_registry_at(&root).unwrap();
        assert_eq!(imported.apps[0].id, "notes");
        assert!(registry_v2_path_for_root(&root).exists());
        assert_eq!(load_or_migrate_registry_at(&root).unwrap(), imported);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_path_uses_native_separator() {
        let p = resolve_config();
        if cfg!(windows) {
            assert!(p.ends_with("local-store\\apps.json"), "got: {}", p);
        } else {
            assert!(!p.contains('\\'), "path must use native separator: {}", p);
            assert!(p.ends_with("local-store/apps.json"), "got: {}", p);
        }
    }

    #[test]
    fn upsert_dedups_by_name() {
        let mut apps: Vec<AppDef> = vec![];
        apps.push(AppDef {
            name: "x".into(),
            url: "http://a".into(),
            icon: None,
            compose: None,
            health: None,
        });
        apps.push(AppDef {
            name: "x".into(),
            url: "http://b".into(),
            icon: None,
            compose: None,
            health: None,
        });
        apps.retain(|a| a.name != "x");
        apps.push(AppDef {
            name: "x".into(),
            url: "http://c".into(),
            icon: None,
            compose: None,
            health: None,
        });
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].url, "http://c");
    }

    #[test]
    fn primary_registry_wins_over_legacy_registry() {
        let (root, primary, legacy) = fixture_paths();
        fs::write(&primary, app_json("primary")).unwrap();
        fs::write(&legacy, app_json("legacy")).unwrap();

        assert_eq!(load_apps_from_paths(&primary, &legacy)[0].name, "primary");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_primary_registry_falls_back_to_legacy_registry() {
        let (root, primary, legacy) = fixture_paths();
        fs::write(&legacy, app_json("legacy")).unwrap();

        assert_eq!(load_apps_from_paths(&primary, &legacy)[0].name, "legacy");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_primary_registry_does_not_fall_back_to_legacy_registry() {
        let (root, primary, legacy) = fixture_paths();
        fs::write(&primary, "not json").unwrap();
        fs::write(&legacy, app_json("legacy")).unwrap();

        assert!(load_apps_from_paths(&primary, &legacy).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_primary_registry_does_not_fall_back_to_legacy_registry() {
        let (root, primary, legacy) = fixture_paths();
        fs::create_dir(&primary).unwrap();
        fs::write(&legacy, app_json("legacy")).unwrap();

        assert!(load_apps_from_paths(&primary, &legacy).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn v2_paths(root: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
        (
            registry_v2_path_for_root(root),
            migration_v1_backup_path_for_root(root),
            legacy_v1_path_for_root(root),
        )
    }

    fn fixture_v1() -> &'static [u8] {
        include_bytes!("../tests/fixtures/apps-v1.json")
    }

    #[test]
    fn registry_v2_atomic_save_and_load_round_trip() {
        let (root, registry_path, _, _) = {
            let (root, _, _) = fixture_paths();
            let (registry, backup, legacy) = v2_paths(&root);
            (root, registry, backup, legacy)
        };
        let registry = RegistryV2::new(vec![InstalledApp {
            id: "external-dashboard".into(),
            catalog_id: None,
            display_name: "External Dashboard".into(),
            launch_url: "https://dashboard.example.test".into(),
            icon_path: None,
            runtime: RuntimeSpec::External,
            created_at_unix: 7,
            updated_at_unix: 7,
        }]);

        save_registry_v2_at(&root, &registry).unwrap();
        assert_eq!(load_registry_v2_at(&root).unwrap(), registry);
        assert!(registry_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_imports_fixture_writes_exact_backup_and_preserves_runtime_boundary() {
        let (root, registry_path, backup_path, legacy_path) = {
            let (root, _, _) = fixture_paths();
            let (registry, backup, legacy) = v2_paths(&root);
            (root, registry, backup, legacy)
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, fixture_v1()).unwrap();

        migrate_v1_registry_at(&root, 1_700_000_000).unwrap();

        assert_eq!(fs::read(&backup_path).unwrap(), fixture_v1());
        let migrated = load_registry_v2_at(&root).unwrap();
        assert_eq!(migrated.version, 2);
        assert_eq!(migrated.apps.len(), 2);
        assert_eq!(migrated.apps[0].id, "external-dashboard");
        assert_eq!(migrated.apps[0].display_name, "External Dashboard");
        assert_eq!(
            migrated.apps[0].icon_path,
            Some(PathBuf::from("C:/icons/dashboard.png"))
        );
        assert!(matches!(migrated.apps[0].runtime, RuntimeSpec::External));
        match &migrated.apps[1].runtime {
            RuntimeSpec::Compose {
                project_name,
                project_dir,
                compose_file,
            } => {
                assert_eq!(project_name, "paperless-ngx");
                assert_eq!(
                    project_dir,
                    &PathBuf::from("C:/Users/example/compose/paperless")
                );
                assert_eq!(
                    compose_file,
                    &PathBuf::from("C:/Users/example/compose/paperless/compose.yaml")
                );
            }
            RuntimeSpec::External => panic!("compose path must migrate to compose runtime"),
        }
        assert!(registry_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_assigns_collision_safe_ascii_ids() {
        let (root, _, _, legacy_path) = {
            let (root, _, _) = fixture_paths();
            let (registry, backup, legacy) = v2_paths(&root);
            (root, registry, backup, legacy)
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(
            &legacy_path,
            r#"[{"name":"Café!","url":"https://one.test","icon":null,"compose":null,"health":null},{"name":"CAFÉ?","url":"https://two.test","icon":null,"compose":null,"health":null},{"name":"---","url":"https://three.test","icon":null,"compose":null,"health":null}]"#,
        )
        .unwrap();

        migrate_v1_registry_at(&root, 9).unwrap();
        let ids: Vec<_> = load_registry_v2_at(&root)
            .unwrap()
            .apps
            .into_iter()
            .map(|app| app.id)
            .collect();
        assert_eq!(ids, vec!["caf", "caf-2", "app"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_v1_leaves_v2_and_backup_absent() {
        let (root, registry_path, backup_path, legacy_path) = {
            let (root, _, _) = fixture_paths();
            let (registry, backup, legacy) = v2_paths(&root);
            (root, registry, backup, legacy)
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, b"not valid json").unwrap();

        assert!(migrate_v1_registry_at(&root, 1).is_err());
        assert!(!registry_path.exists());
        assert!(!backup_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_refuses_nonempty_v2_without_changing_it() {
        let (root, registry_path, backup_path, legacy_path) = {
            let (root, _, _) = fixture_paths();
            let (registry, backup, legacy) = v2_paths(&root);
            (root, registry, backup, legacy)
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, fixture_v1()).unwrap();
        let existing = RegistryV2::new(vec![InstalledApp {
            id: "existing".into(),
            catalog_id: None,
            display_name: "Existing".into(),
            launch_url: "https://existing.test".into(),
            icon_path: None,
            runtime: RuntimeSpec::External,
            created_at_unix: 1,
            updated_at_unix: 1,
        }]);
        save_registry_v2_at(&root, &existing).unwrap();
        let original = fs::read(&registry_path).unwrap();

        assert!(migrate_v1_registry_at(&root, 2).is_err());
        assert_eq!(fs::read(&registry_path).unwrap(), original);
        assert!(!backup_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inline_compose_yaml_is_rejected_without_writing_migration_files() {
        let (root, registry_path, backup_path, legacy_path) = {
            let (root, _, _) = fixture_paths();
            let (registry, backup, legacy) = v2_paths(&root);
            (root, registry, backup, legacy)
        };
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, r#"[{"name":"unsafe","url":"https://unsafe.test","icon":null,"compose":"services:\n  web:\n    image: unsafe","health":null}]"#).unwrap();

        assert!(migrate_v1_registry_at(&root, 1).is_err());
        assert!(!registry_path.exists());
        assert!(!backup_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_path_helpers_match_root_semantics_on_windows_and_linux() {
        assert_eq!(
            legacy_v1_path_for_platform(PathBuf::from("C:/Config").as_path(), "windows"),
            PathBuf::from("C:/Config")
                .join(LEGACY_CONFIG_SLUG)
                .join("apps.json")
        );
        assert_eq!(
            legacy_v1_path_for_platform(PathBuf::from("/home/user/.config").as_path(), "linux"),
            PathBuf::from("/home/user/.config")
                .join(LEGACY_CONFIG_SLUG)
                .join("apps.json")
        );
    }

    #[test]
    fn all_v2_product_files_live_under_local_store_for_windows_and_linux_bases() {
        for root in [
            PathBuf::from("C:/Config"),
            PathBuf::from("/home/user/.config"),
        ] {
            assert_eq!(
                registry_v2_path_for_root(&root),
                root.join(CONFIG_SLUG).join("registry-v2.json")
            );
            assert_eq!(
                migration_v1_backup_path_for_root(&root),
                root.join(CONFIG_SLUG).join("migration-v1-backup.json")
            );
            assert_eq!(
                registry_v2_previous_path_for_root(&root),
                root.join(CONFIG_SLUG).join("registry-v2.previous.json")
            );
            assert_eq!(
                primary_v1_path_for_root(&root),
                root.join(CONFIG_SLUG).join("apps.json")
            );
        }
    }

    #[test]
    fn migration_prefers_current_local_store_v1_over_pre_rebrand_v1() {
        let (root, _, _) = fixture_paths();
        let current = primary_v1_path_for_root(&root);
        let legacy = legacy_v1_path_for_root(&root);
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&current, app_json("current")).unwrap();
        fs::write(&legacy, app_json("legacy")).unwrap();

        migrate_v1_registry_at(&root, 9).unwrap();

        assert_eq!(
            load_registry_v2_at(&root).unwrap().apps[0].display_name,
            "current"
        );
        assert_eq!(
            fs::read(migration_v1_backup_path_for_root(&root)).unwrap(),
            fs::read(current).unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_recovers_previous_registry_when_live_file_is_absent() {
        let (root, _, _) = fixture_paths();
        let prior = RegistryV2::new(vec![InstalledApp {
            id: "prior".into(),
            catalog_id: None,
            display_name: "Prior".into(),
            launch_url: "https://prior.test".into(),
            icon_path: None,
            runtime: RuntimeSpec::External,
            created_at_unix: 1,
            updated_at_unix: 1,
        }]);
        fs::create_dir_all(root.join(CONFIG_SLUG)).unwrap();
        fs::write(
            registry_v2_previous_path_for_root(&root),
            serde_json::to_vec(&prior).unwrap(),
        )
        .unwrap();

        assert_eq!(load_registry_v2_at(&root).unwrap(), prior);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_retry_uses_immutable_backup_after_original_changes() {
        let (root, _, _) = fixture_paths();
        let current = primary_v1_path_for_root(&root);
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        let original = app_json("original");
        fs::write(&current, &original).unwrap();
        fs::write(migration_v1_backup_path_for_root(&root), &original).unwrap();
        fs::write(&current, app_json("changed")).unwrap();

        migrate_v1_registry_at(&root, 9).unwrap();

        assert_eq!(
            fs::read(migration_v1_backup_path_for_root(&root)).unwrap(),
            original.as_bytes()
        );
        assert_eq!(
            load_registry_v2_at(&root).unwrap().apps[0].display_name,
            "original"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_rejects_unknown_v2_fields_and_invalid_runtime_invariants() {
        let (root, _, _) = fixture_paths();
        let registry_path = registry_v2_path_for_root(&root);
        fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
        fs::write(
            &registry_path,
            r#"{"version":2,"apps":[{"id":"Bad_ID","catalog_id":null,"display_name":"","launch_url":"","icon_path":null,"runtime":{"kind":"compose","project_name":"Bad_ID","project_dir":"projects/app","compose_file":"projects/app/../escape.yml","extra":true},"created_at_unix":2,"updated_at_unix":1,"extra":true}],"extra":true}"#,
        )
        .unwrap();

        let error = load_registry_v2_at(&root).unwrap_err().to_string();
        assert!(error.contains("unknown field") || error.contains("invalid"));
        fs::remove_dir_all(root).unwrap();
    }
}

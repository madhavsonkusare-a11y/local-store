//! Durable ownership record for a future WSL import transaction.
//!
//! This module does not call WSL. The caller must first inventory registered
//! distros, then reserve a fresh journal before importing the fixed distro.
use super::DISTRO;
use crate::{
    error::{AppError, AppResult},
    runtime::{CommandSpec, ProcessRunner, DIAGNOSTIC_TIMEOUT},
    storage,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const JOURNAL_FILE: &str = "bootstrap.json";
pub const TOKEN_FILE: &str = "ownership-token";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024;
const MAX_DISTRO_LIST_CHARS: usize = 64 * 1024;
const MAX_ROOTFS_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapState {
    Reserved,
    Imported,
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    RetryImport,
    ResumeVerification,
    Ready,
    ManualReview,
}

/// The only launcher-facing view of an existing bootstrap transaction. It
/// deliberately omits the ownership token, data directory and payload digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BootstrapStatus {
    NotConfigured,
    Recovery {
        phase: BootstrapState,
        disposition: RecoveryDisposition,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapJournal {
    pub schema_version: u32,
    pub distro: String,
    pub install_dir: PathBuf,
    pub rootfs_sha256: String,
    /// Random identifier generated once by the future bootstrap coordinator.
    pub ownership_token: String,
    pub state: BootstrapState,
}

impl BootstrapJournal {
    pub fn new(
        install_dir: PathBuf,
        rootfs_sha256: String,
        ownership_token: String,
    ) -> AppResult<Self> {
        let value = Self {
            schema_version: 1,
            distro: DISTRO.into(),
            install_dir,
            rootfs_sha256,
            ownership_token,
            state: BootstrapState::Reserved,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> AppResult<()> {
        let path = self.install_dir.to_string_lossy();
        let bytes = path.as_bytes();
        let local_absolute = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        let digest = self.rootfs_sha256.len() == 64
            && self
                .rootfs_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
        let token = self.ownership_token.len() >= 32
            && self.ownership_token.len() <= 128
            && self
                .ownership_token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if self.schema_version != 1
            || self.distro != DISTRO
            || !local_absolute
            || path.starts_with(r"\\")
            || path.contains("..")
            || path.chars().any(char::is_control)
            || path.len() > 4096
            || !digest
            || !token
        {
            return Err(AppError::invalid(
                "Invalid managed-engine bootstrap ownership journal.",
            ));
        }
        Ok(())
    }

    pub fn advance(&mut self, next: BootstrapState) -> AppResult<()> {
        let valid = matches!(
            (self.state, next),
            (BootstrapState::Reserved, BootstrapState::Imported)
                | (BootstrapState::Imported, BootstrapState::Verified)
        );
        if !valid {
            return Err(AppError::invalid(
                "Invalid managed-engine bootstrap state transition.",
            ));
        }
        self.state = next;
        Ok(())
    }
}

pub fn load(directory: &Path) -> AppResult<Option<BootstrapJournal>> {
    let path = directory.join(JOURNAL_FILE);
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut data = Vec::new();
    file.by_ref()
        .take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(AppError::invalid(
            "Managed-engine bootstrap journal is too large.",
        ));
    }
    let journal: BootstrapJournal = serde_json::from_slice(&data)
        .map_err(|_| AppError::invalid("Managed-engine bootstrap journal is corrupt."))?;
    journal.validate()?;
    Ok(Some(journal))
}

/// First writer owns the reservation. Existing, corrupt, or mismatched state is
/// never overwritten, which lets a later coordinator recover deliberately.
pub fn reserve(directory: &Path, journal: &BootstrapJournal) -> AppResult<()> {
    journal.validate()?;
    fs::create_dir_all(directory)?;
    if load(directory)?.is_some() {
        return Err(AppError::invalid(
            "A managed-engine bootstrap reservation already exists.",
        ));
    }
    let path = directory.join(JOURNAL_FILE);
    let bytes = serde_json::to_vec_pretty(journal).map_err(AppError::internal)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    use std::io::Write;
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            AppError::invalid("A managed-engine bootstrap reservation already exists.")
        } else {
            error.into()
        }
    })?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        // The caller sees failure and the possibly partial reservation remains;
        // load will reject it. Never erase evidence another process may own.
        return Err(error.into());
    }
    storage::write_file_atomically(
        &directory.join(TOKEN_FILE),
        journal.ownership_token.as_bytes(),
    )?;
    Ok(())
}

/// Decode the bounded `wsl.exe --list --quiet` capture. On Windows its UTF-16LE
/// output reaches the shared text runner as ASCII characters separated by NULs.
/// Ambiguous or lossy output refuses so it cannot conceal a name collision.
pub fn parse_distro_names(output: &str) -> AppResult<Vec<String>> {
    let decoded = decode_wsl_text(output)?;
    let mut names = Vec::new();
    for line in decoded.lines() {
        let name = line.trim().trim_start_matches('*').trim();
        if name.is_empty() {
            continue;
        }
        if name.len() > 256 || name.chars().any(char::is_control) {
            return Err(AppError::invalid("WSL returned an invalid distro name."));
        }
        names.push(name.to_owned());
    }
    Ok(names)
}

fn decode_wsl_text(output: &str) -> AppResult<String> {
    if output.chars().count() > MAX_DISTRO_LIST_CHARS
        || output.contains('\u{fffd}')
        || output
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\0' | '\r' | '\n' | '\t'))
    {
        return Err(AppError::invalid(
            "WSL returned an invalid or incomplete distro inventory.",
        ));
    }
    if !output.contains('\0') {
        Ok(output.to_owned())
    } else {
        let chars: Vec<_> = output.chars().collect();
        if !chars
            .chunks(2)
            .all(|pair| pair.len() == 2 && pair[1] == '\0' && pair[0].is_ascii())
        {
            return Err(AppError::invalid(
                "WSL distro inventory encoding is ambiguous.",
            ));
        }
        Ok(chars.into_iter().step_by(2).collect())
    }
}

fn wsl_command(args: Vec<String>, timeout: std::time::Duration) -> CommandSpec {
    let mut spec = CommandSpec::new("wsl.exe", args, None, timeout);
    spec.remove_env.push("WSLENV".into());
    spec
}

/// Read-only collision gate. It never imports, terminates or unregisters WSL.
pub fn preflight(runner: &dyn ProcessRunner, install_dir: &Path) -> AppResult<()> {
    if install_dir.exists() {
        return Err(AppError::invalid("The managed-engine data directory already exists; inspect or recover it before bootstrap."));
    }
    let spec = wsl_command(vec!["--list".into(), "--quiet".into()], DIAGNOSTIC_TIMEOUT);
    let output = runner.run(&spec)?;
    if !output.success || output.truncated {
        return Err(AppError::invalid(
            "WSL distro inventory failed or was incomplete.",
        ));
    }
    if parse_distro_names(&output.stdout)?
        .iter()
        .any(|name| name.eq_ignore_ascii_case(DISTRO))
    {
        return Err(AppError::invalid("The reserved Local Store WSL distro name already exists; bootstrap will not replace it."));
    }
    Ok(())
}

pub fn verify_rootfs(path: &Path, expected_bytes: u64, expected_sha256: &str) -> AppResult<()> {
    if expected_bytes == 0
        || expected_bytes > MAX_ROOTFS_BYTES
        || expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(AppError::invalid("Invalid expected WSL rootfs identity."));
    }
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(AppError::invalid(
            "WSL rootfs length does not match its locked identity.",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    let mut read = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        read = read
            .checked_add(count as u64)
            .ok_or_else(|| AppError::invalid("WSL rootfs length overflow."))?;
        if read > expected_bytes {
            return Err(AppError::invalid(
                "WSL rootfs changed while it was being verified.",
            ));
        }
        digest.update(&buffer[..count]);
    }
    let actual = format!("{:x}", digest.finalize());
    if read != expected_bytes || actual != expected_sha256 {
        return Err(AppError::invalid(
            "WSL rootfs SHA-256 does not match its locked identity.",
        ));
    }
    Ok(())
}

/// Prepare, but deliberately do not execute, the only import command. The
/// ownership journal lives outside `install_dir`, which must remain absent for
/// WSL to create. A reservation is durable before the command is returned.
pub fn prepare_import(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
    journal: &BootstrapJournal,
    rootfs: &Path,
    rootfs_bytes: u64,
) -> AppResult<CommandSpec> {
    journal.validate()?;
    if state_dir == journal.install_dir
        || state_dir.starts_with(&journal.install_dir)
        || journal.install_dir.starts_with(state_dir)
    {
        return Err(AppError::invalid(
            "Bootstrap ownership state must be outside the WSL data directory.",
        ));
    }
    preflight(runner, &journal.install_dir)?;
    verify_rootfs(rootfs, rootfs_bytes, &journal.rootfs_sha256)?;
    reserve(state_dir, journal)?;
    Ok(import_command(journal, rootfs))
}

fn import_command(journal: &BootstrapJournal, rootfs: &Path) -> CommandSpec {
    wsl_command(
        vec![
            "--import".into(),
            DISTRO.into(),
            journal.install_dir.to_string_lossy().into_owned(),
            rootfs.to_string_lossy().into_owned(),
            "--version".into(),
            "2".into(),
        ],
        crate::runtime::PROVISION_TIMEOUT,
    )
}

fn finish_import(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
    journal: &BootstrapJournal,
    command: &CommandSpec,
) -> AppResult<BootstrapJournal> {
    let output = runner.run(command).map_err(|error| {
        let error = AppError::from(error);
        AppError::new(error.code, format!("WSL import did not complete: {} The ownership reservation was retained for recovery.", error.message))
    })?;
    if !output.success || output.truncated {
        return Err(AppError::invalid("WSL import failed or returned incomplete output. The ownership reservation was retained for recovery."));
    }
    let mut imported = journal.clone();
    imported.advance(BootstrapState::Imported)?;
    save(state_dir, &imported)?;
    Ok(imported)
}

/// Execute the prepared import transaction. Any failure leaves `Reserved`
/// durable because a timeout can mean WSL changed state after our observation.
/// Recovery must inventory it; this function never unregisters or retries.
pub fn import(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
    journal: &BootstrapJournal,
    rootfs: &Path,
    rootfs_bytes: u64,
) -> AppResult<BootstrapJournal> {
    let command = prepare_import(runner, state_dir, journal, rootfs, rootfs_bytes)?;
    finish_import(runner, state_dir, journal, &command)
}

const COMPONENTS: [(&str, &str); 5] = [
    ("containerd.io", "2.3.5-1~ubuntu.24.04~noble"),
    ("docker-buildx-plugin", "0.37.1-1~ubuntu.24.04~noble"),
    ("docker-ce", "5:29.8.0-1~ubuntu.24.04~noble"),
    ("docker-ce-cli", "5:29.8.0-1~ubuntu.24.04~noble"),
    ("docker-compose-plugin", "5.5.1-1~ubuntu.24.04~noble"),
];

fn inside(args: Vec<String>) -> CommandSpec {
    let mut all = vec![
        "--distribution".into(),
        DISTRO.into(),
        "--user".into(),
        "root".into(),
        "--exec".into(),
    ];
    all.extend(args);
    wsl_command(all, DIAGNOSTIC_TIMEOUT)
}

fn successful(runner: &dyn ProcessRunner, spec: &CommandSpec, label: &str) -> AppResult<String> {
    let output = runner.run(spec)?;
    if !output.success || output.truncated {
        return Err(AppError::invalid(format!(
            "Managed-engine {label} failed or returned incomplete output."
        )));
    }
    Ok(output.stdout)
}

fn ownership_token_source(state_dir: &Path) -> AppResult<String> {
    // WSL is a Windows-only product path. Library tests also run on Linux,
    // where a temporary file cannot have a Windows drive identity; command
    // tests use this fixed, non-existent WSL source rather than weakening the
    // production path translator.
    #[cfg(all(test, not(windows)))]
    {
        let _ = state_dir;
        Ok("/mnt/c/local-store-test/ownership-token".into())
    }
    #[cfg(not(all(test, not(windows))))]
    {
        let token = state_dir.join(TOKEN_FILE).canonicalize()?;
        let windows = crate::folders::docker_path(&token);
        super::windows_drive_path(
            windows
                .to_str()
                .ok_or_else(|| AppError::invalid("Ownership-token path must be Unicode."))?,
        )
    }
}

/// Establish the facts required for a Verified journal. It never changes any
/// other distro. Imported remains durable on every failure for explicit repair.
pub fn verify_imported(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
) -> AppResult<BootstrapJournal> {
    let mut journal = load(state_dir)?
        .ok_or_else(|| AppError::invalid("Managed-engine bootstrap journal is missing."))?;
    if journal.state != BootstrapState::Imported {
        return Err(AppError::invalid(
            "Only an imported managed engine can be verified.",
        ));
    }
    let verbose = successful(
        runner,
        &wsl_command(
            vec!["--list".into(), "--verbose".into()],
            DIAGNOSTIC_TIMEOUT,
        ),
        "WSL inventory",
    )?;
    let decoded = decode_wsl_text(&verbose)?;
    let matching: Vec<_> = decoded
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line
                .trim()
                .trim_start_matches('*')
                .split_whitespace()
                .collect();
            (fields
                .first()
                .is_some_and(|name| name.eq_ignore_ascii_case(DISTRO)))
            .then_some(fields)
        })
        .collect();
    if matching.len() != 1 || matching[0].last() != Some(&"2") {
        return Err(AppError::invalid(
            "The imported Local Store distro is missing, duplicated, or not WSL 2.",
        ));
    }
    let source = ownership_token_source(state_dir)?;
    successful(
        runner,
        &inside(vec![
            "/usr/bin/install".into(),
            "--mode=0400".into(),
            source.clone(),
            "/usr/share/local-store/ownership-token".into(),
        ]),
        "ownership installation",
    )?;
    successful(
        runner,
        &inside(vec![
            "/usr/bin/cmp".into(),
            "--silent".into(),
            source,
            "/usr/share/local-store/ownership-token".into(),
        ]),
        "ownership verification",
    )?;
    let mut args = vec![
        "/usr/bin/dpkg-query".into(),
        "-W".into(),
        "-f=${Package}\\t${Version}\\n".into(),
    ];
    args.extend(COMPONENTS.iter().map(|(name, _)| (*name).into()));
    let packages = successful(runner, &inside(args), "component inventory")?;
    let found: std::collections::BTreeMap<_, _> = packages
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect();
    if found.len() != COMPONENTS.len()
        || COMPONENTS
            .iter()
            .any(|(name, version)| found.get(name) != Some(version))
    {
        return Err(AppError::invalid(
            "Managed-engine component versions differ from the locked payload.",
        ));
    }
    let doctor = crate::runtime::doctor_with(&super::DiagnosticRunner { inner: runner });
    if !doctor.ready {
        return Err(AppError::invalid(
            "Managed-engine Docker daemon or Compose plugin is not ready.",
        ));
    }
    journal.advance(BootstrapState::Verified)?;
    save(state_dir, &journal)?;
    Ok(journal)
}

/// Read-only recovery classification. A Reserved transaction with any external
/// WSL footprint is intentionally ambiguous: import may have timed out after
/// mutation, and no in-distro token has yet been proven. Nothing is deleted.
pub fn classify_recovery(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
) -> AppResult<RecoveryDisposition> {
    let journal = load(state_dir)?
        .ok_or_else(|| AppError::invalid("Managed-engine bootstrap journal is missing."))?;
    let output = successful(
        runner,
        &wsl_command(vec!["--list".into(), "--quiet".into()], DIAGNOSTIC_TIMEOUT),
        "WSL recovery inventory",
    )?;
    let count = parse_distro_names(&output)?
        .iter()
        .filter(|name| name.eq_ignore_ascii_case(DISTRO))
        .count();
    if count > 1 {
        return Ok(RecoveryDisposition::ManualReview);
    }
    let distro_exists = count == 1;
    let directory_exists = journal.install_dir.exists();
    Ok(match journal.state {
        BootstrapState::Reserved if !distro_exists && !directory_exists => {
            RecoveryDisposition::RetryImport
        }
        BootstrapState::Reserved => RecoveryDisposition::ManualReview,
        BootstrapState::Imported if distro_exists && directory_exists => {
            RecoveryDisposition::ResumeVerification
        }
        BootstrapState::Verified if distro_exists && directory_exists => RecoveryDisposition::Ready,
        BootstrapState::Imported | BootstrapState::Verified => RecoveryDisposition::ManualReview,
    })
}

/// Inspect an existing transaction without reserving, importing, retrying, or
/// unregistering a distro. A missing journal is the normal first-launch state.
pub fn inspect(runner: &dyn ProcessRunner, state_dir: &Path) -> AppResult<BootstrapStatus> {
    let Some(journal) = load(state_dir)? else {
        return Ok(BootstrapStatus::NotConfigured);
    };
    Ok(BootstrapStatus::Recovery {
        phase: journal.state,
        disposition: classify_recovery(runner, state_dir)?,
    })
}

/// Retry only the one unambiguous recovery case: an existing Reserved journal
/// with neither the fixed distro nor its data directory present. The payload is
/// verified again and the original identity is never replaced.
pub fn retry_import(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
    rootfs: &Path,
    rootfs_bytes: u64,
) -> AppResult<BootstrapJournal> {
    let journal = load(state_dir)?
        .ok_or_else(|| AppError::invalid("Managed-engine bootstrap journal is missing."))?;
    if classify_recovery(runner, state_dir)? != RecoveryDisposition::RetryImport {
        return Err(AppError::invalid(
            "Managed-engine import cannot be retried because its footprint is uncertain.",
        ));
    }
    verify_rootfs(rootfs, rootfs_bytes, &journal.rootfs_sha256)?;
    finish_import(
        runner,
        state_dir,
        &journal,
        &import_command(&journal, rootfs),
    )
}

/// Prepare the destructive WSL unregister command only after the external
/// Verified journal, fixed distro footprint and in-distro token all agree.
/// The caller still owns the explicit user decision and execution.
pub fn authorize_unregistration(
    runner: &dyn ProcessRunner,
    state_dir: &Path,
) -> AppResult<CommandSpec> {
    let journal = load(state_dir)?
        .ok_or_else(|| AppError::invalid("Managed-engine bootstrap journal is missing."))?;
    if journal.state != BootstrapState::Verified
        || classify_recovery(runner, state_dir)? != RecoveryDisposition::Ready
    {
        return Err(AppError::invalid(
            "Managed-engine removal requires a complete verified ownership footprint.",
        ));
    }
    let source = ownership_token_source(state_dir)?;
    successful(
        runner,
        &inside(vec![
            "/usr/bin/cmp".into(),
            "--silent".into(),
            source,
            "/usr/share/local-store/ownership-token".into(),
        ]),
        "removal ownership verification",
    )?;
    Ok(wsl_command(
        vec!["--unregister".into(), DISTRO.into()],
        crate::runtime::PROVISION_TIMEOUT,
    ))
}

pub fn save(directory: &Path, journal: &BootstrapJournal) -> AppResult<()> {
    journal.validate()?;
    let previous = load(directory)?
        .ok_or_else(|| AppError::invalid("Managed-engine bootstrap reservation is missing."))?;
    if previous.distro != journal.distro
        || previous.install_dir != journal.install_dir
        || previous.rootfs_sha256 != journal.rootfs_sha256
        || previous.ownership_token != journal.ownership_token
    {
        return Err(AppError::invalid(
            "Refusing to replace managed-engine ownership identity.",
        ));
    }
    storage::write_file_atomically(
        &directory.join(JOURNAL_FILE),
        &serde_json::to_vec_pretty(journal).map_err(AppError::internal)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{CancelToken, ProcessError, ProcessOutput};
    use std::{collections::VecDeque, sync::Mutex};
    fn temp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "local-store-bootstrap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    /// Journals model a Windows-owned directory. Linux CI needs a valid
    /// Windows-shaped fixture while Windows uses a live temporary directory.
    fn fixture_install_dir(parent: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            parent.join("data")
        }
        #[cfg(not(windows))]
        {
            PathBuf::from(format!(
                r"C:\local-store-bootstrap-{}",
                parent.file_name().unwrap().to_string_lossy()
            ))
        }
    }
    fn journal() -> BootstrapJournal {
        BootstrapJournal::new(
            PathBuf::from(r"C:\ProgramData\Local Store\engine-v1"),
            "a".repeat(64),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap()
    }

    #[test]
    fn reservation_is_exclusive_and_identity_is_immutable() {
        let root = temp();
        let mut value = journal();
        reserve(&root, &value).unwrap();
        assert_eq!(load(&root).unwrap(), Some(value.clone()));
        assert!(reserve(&root, &value).is_err());
        value.advance(BootstrapState::Imported).unwrap();
        save(&root, &value).unwrap();
        value.advance(BootstrapState::Verified).unwrap();
        save(&root, &value).unwrap();
        assert_eq!(
            load(&root).unwrap().unwrap().state,
            BootstrapState::Verified
        );
        let mut changed = value.clone();
        changed.rootfs_sha256 = "b".repeat(64);
        assert!(save(&root, &changed).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_oversized_or_ambiguous_state_is_never_replaced() {
        for path in [
            PathBuf::from(r"\\server\share"),
            PathBuf::from("relative"),
            PathBuf::from(r"C:\a\..\b"),
        ] {
            assert!(BootstrapJournal::new(
                path,
                "a".repeat(64),
                "01234567-89ab-cdef-0123-456789abcdef".into()
            )
            .is_err());
        }
        let mut value = journal();
        assert!(value.advance(BootstrapState::Verified).is_err());
        let root = temp();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(JOURNAL_FILE), "{").unwrap();
        assert!(load(&root).is_err());
        assert!(reserve(&root, &journal()).is_err());
        fs::write(
            root.join(JOURNAL_FILE),
            vec![b'x'; MAX_JOURNAL_BYTES as usize + 1],
        )
        .unwrap();
        assert!(load(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    struct Inventory {
        output: ProcessOutput,
        calls: Mutex<Vec<CommandSpec>>,
    }
    impl ProcessRunner for Inventory {
        fn run_cancellable(
            &self,
            spec: &CommandSpec,
            _: &CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            self.calls.lock().unwrap().push(spec.clone());
            Ok(self.output.clone())
        }
    }
    fn inventory(stdout: &str) -> Inventory {
        Inventory {
            output: ProcessOutput {
                success: true,
                stdout: stdout.into(),
                stderr: String::new(),
                truncated: false,
            },
            calls: Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn utf8_and_utf16le_shaped_inventories_are_bounded_and_equivalent() {
        let utf8 = "docker-desktop\r\nUbuntu\r\n";
        let utf16_shape: String = utf8.chars().flat_map(|c| [c, '\0']).collect();
        assert_eq!(
            parse_distro_names(utf8).unwrap(),
            vec!["docker-desktop", "Ubuntu"]
        );
        assert_eq!(
            parse_distro_names(&utf16_shape).unwrap(),
            vec!["docker-desktop", "Ubuntu"]
        );
        for invalid in [
            "docker\0desktop",
            "docker\u{fffd}desktop",
            "docker\u{0007}desktop",
        ] {
            assert!(parse_distro_names(invalid).is_err());
        }
        assert!(parse_distro_names(&"x".repeat(MAX_DISTRO_LIST_CHARS + 1)).is_err());
    }

    #[test]
    fn preflight_refuses_name_or_directory_collision_without_mutation() {
        let parent = temp();
        let target = parent.join("engine-v1");
        let collision = inventory(&format!("docker-desktop\r\n{}\r\n", DISTRO.to_uppercase()));
        assert!(preflight(&collision, &target).is_err());
        assert!(!target.exists());
        let clear = inventory("docker-desktop\r\n");
        preflight(&clear, &target).unwrap();
        let calls = clear.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].program, "wsl.exe");
        assert_eq!(calls[0].args, ["--list", "--quiet"]);
        assert!(calls[0].remove_env.contains(&"WSLENV".into()));
        drop(calls);
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("foreign.txt"), "keep").unwrap();
        let untouched = inventory("");
        assert!(preflight(&untouched, &target).is_err());
        assert!(target.join("foreign.txt").is_file());
        assert!(untouched.calls.lock().unwrap().is_empty());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn inspection_is_read_only_and_exposes_only_safe_recovery_state() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let state = parent.join("state");
        let missing = inventory("");
        assert_eq!(
            inspect(&missing, &state).unwrap(),
            BootstrapStatus::NotConfigured
        );
        assert!(missing.calls.lock().unwrap().is_empty());

        let journal = BootstrapJournal::new(
            fixture_install_dir(&parent),
            "a".repeat(64),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap();
        reserve(&state, &journal).unwrap();
        assert_eq!(
            inspect(&inventory("docker-desktop\r\n"), &state).unwrap(),
            BootstrapStatus::Recovery {
                phase: BootstrapState::Reserved,
                disposition: RecoveryDisposition::RetryImport,
            }
        );
        assert_eq!(load(&state).unwrap(), Some(journal));
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn payload_is_streamed_and_import_is_prepared_only_after_reservation() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let rootfs = parent.join("rootfs.tar");
        fs::write(&rootfs, b"reviewed payload").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"reviewed payload"));
        let install = fixture_install_dir(&parent);
        let journal = BootstrapJournal::new(
            install.clone(),
            digest.clone(),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap();
        let state = parent.join("bootstrap-state");
        let clear = inventory("docker-desktop\r\n");
        let command = prepare_import(&clear, &state, &journal, &rootfs, 16).unwrap();
        assert_eq!(command.program, "wsl.exe");
        assert_eq!(command.args[0], "--import");
        assert_eq!(command.args[1], DISTRO);
        assert_eq!(command.args[2], install.to_string_lossy());
        assert_eq!(command.args[3], rootfs.to_string_lossy());
        assert_eq!(&command.args[4..], ["--version", "2"]);
        assert_eq!(command.timeout, crate::runtime::PROVISION_TIMEOUT);
        assert_eq!(load(&state).unwrap(), Some(journal));
        assert!(!install.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn payload_mismatch_or_collision_never_reserves_or_returns_import() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let rootfs = parent.join("rootfs.tar");
        fs::write(&rootfs, b"payload").unwrap();
        let state = parent.join("state");
        let install = fixture_install_dir(&parent);
        for (bytes, digest) in [
            (6, "a".repeat(64)),
            (7, "a".repeat(64)),
            (7, format!("{:x}", Sha256::digest(b"different"))),
        ] {
            let journal = BootstrapJournal::new(
                install.clone(),
                digest,
                "01234567-89ab-cdef-0123-456789abcdef".into(),
            )
            .unwrap();
            assert!(prepare_import(&inventory(""), &state, &journal, &rootfs, bytes).is_err());
            assert!(!state.exists() && !install.exists());
        }
        let good = format!("{:x}", Sha256::digest(b"payload"));
        let journal =
            BootstrapJournal::new(install, good, "01234567-89ab-cdef-0123-456789abcdef".into())
                .unwrap();
        assert!(prepare_import(&inventory(DISTRO), &state, &journal, &rootfs, 7).is_err());
        assert!(!state.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    struct Sequence {
        outputs: Mutex<VecDeque<Result<ProcessOutput, ProcessError>>>,
    }
    impl ProcessRunner for Sequence {
        fn run_cancellable(
            &self,
            _: &CommandSpec,
            _: &CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            self.outputs.lock().unwrap().pop_front().unwrap()
        }
    }
    fn ok(stdout: &str) -> Result<ProcessOutput, ProcessError> {
        Ok(ProcessOutput {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
            truncated: false,
        })
    }

    #[test]
    fn import_advances_only_after_a_successful_bounded_command() {
        for outcome in [
            Ok(ProcessOutput {
                success: false,
                stdout: String::new(),
                stderr: "failed".into(),
                truncated: false,
            }),
            Ok(ProcessOutput {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
                truncated: true,
            }),
            Err(ProcessError::new(
                crate::runtime::ProcessErrorCode::TimedOut,
                "uncertain timeout",
            )),
        ] {
            let parent = temp();
            fs::create_dir_all(&parent).unwrap();
            let rootfs = parent.join("rootfs.tar");
            fs::write(&rootfs, b"payload").unwrap();
            let journal = BootstrapJournal::new(
                fixture_install_dir(&parent),
                format!("{:x}", Sha256::digest(b"payload")),
                "01234567-89ab-cdef-0123-456789abcdef".into(),
            )
            .unwrap();
            let runner = Sequence {
                outputs: Mutex::new(VecDeque::from([ok("docker-desktop\r\n"), outcome])),
            };
            assert!(import(&runner, &parent.join("state"), &journal, &rootfs, 7).is_err());
            assert_eq!(
                load(&parent.join("state")).unwrap().unwrap().state,
                BootstrapState::Reserved
            );
            fs::remove_dir_all(parent).unwrap();
        }
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let rootfs = parent.join("rootfs.tar");
        fs::write(&rootfs, b"payload").unwrap();
        let journal = BootstrapJournal::new(
            fixture_install_dir(&parent),
            format!("{:x}", Sha256::digest(b"payload")),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap();
        let runner = Sequence {
            outputs: Mutex::new(VecDeque::from([ok("docker-desktop\r\n"), ok("")])),
        };
        assert_eq!(
            import(&runner, &parent.join("state"), &journal, &rootfs, 7)
                .unwrap()
                .state,
            BootstrapState::Imported
        );
        assert_eq!(
            load(&parent.join("state")).unwrap().unwrap().state,
            BootstrapState::Imported
        );
        fs::remove_dir_all(parent).unwrap();
    }

    fn imported_fixture(parent: &Path) -> (PathBuf, BootstrapJournal) {
        let state = parent.join("state");
        let mut journal = BootstrapJournal::new(
            fixture_install_dir(parent),
            "a".repeat(64),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap();
        reserve(&state, &journal).unwrap();
        journal.advance(BootstrapState::Imported).unwrap();
        save(&state, &journal).unwrap();
        (state, journal)
    }
    fn package_output() -> String {
        COMPONENTS
            .iter()
            .map(|(name, version)| format!("{name}\t{version}\n"))
            .collect()
    }

    #[test]
    fn verification_requires_wsl2_token_components_and_ready_daemon() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let (state, _) = imported_fixture(&parent);
        let outputs = [
            ok(&format!("  NAME STATE VERSION\r\n* {DISTRO} Running 2\r\n")),
            ok(""),
            ok(""),
            ok(&package_output()),
            ok("29.8.0\n"),
            ok("5.5.1\n"),
        ];
        let runner = Sequence {
            outputs: Mutex::new(VecDeque::from(outputs)),
        };
        assert_eq!(
            verify_imported(&runner, &state).unwrap().state,
            BootstrapState::Verified
        );
        assert_eq!(
            load(&state).unwrap().unwrap().state,
            BootstrapState::Verified
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn failed_identity_or_components_never_claims_verified() {
        for outputs in [
            vec![ok(&format!("{DISTRO} Stopped 1\n"))],
            vec![
                ok(&format!("{DISTRO} Running 2\n")),
                Ok(ProcessOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                }),
            ],
            vec![
                ok(&format!("{DISTRO} Running 2\n")),
                ok(""),
                ok(""),
                ok("docker-ce\twrong\n"),
            ],
            vec![
                ok(&format!("{DISTRO} Running 2\n")),
                ok(""),
                ok(""),
                ok(&package_output()),
                ok("29.8.0"),
                Ok(ProcessOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                }),
            ],
        ] {
            let parent = temp();
            fs::create_dir_all(&parent).unwrap();
            let (state, _) = imported_fixture(&parent);
            let runner = Sequence {
                outputs: Mutex::new(outputs.into()),
            };
            assert!(verify_imported(&runner, &state).is_err());
            assert_eq!(
                load(&state).unwrap().unwrap().state,
                BootstrapState::Imported
            );
            fs::remove_dir_all(parent).unwrap();
        }
    }

    #[test]
    fn recovery_classification_never_treats_uncertain_footprints_as_owned() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let state = parent.join("state");
        let mut journal = BootstrapJournal::new(
            fixture_install_dir(&parent),
            "a".repeat(64),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap();
        reserve(&state, &journal).unwrap();
        assert_eq!(
            classify_recovery(&inventory("docker-desktop\n"), &state).unwrap(),
            RecoveryDisposition::RetryImport
        );
        fs::create_dir_all(&journal.install_dir).unwrap();
        assert_eq!(
            classify_recovery(&inventory("docker-desktop\n"), &state).unwrap(),
            RecoveryDisposition::ManualReview
        );
        assert_eq!(
            classify_recovery(&inventory(&format!("{DISTRO}\n")), &state).unwrap(),
            RecoveryDisposition::ManualReview
        );
        journal.advance(BootstrapState::Imported).unwrap();
        save(&state, &journal).unwrap();
        assert_eq!(
            classify_recovery(&inventory(&format!("{DISTRO}\n")), &state).unwrap(),
            RecoveryDisposition::ResumeVerification
        );
        journal.advance(BootstrapState::Verified).unwrap();
        save(&state, &journal).unwrap();
        assert_eq!(
            classify_recovery(&inventory(&format!("{DISTRO}\n")), &state).unwrap(),
            RecoveryDisposition::Ready
        );
        fs::remove_dir_all(&journal.install_dir).unwrap();
        assert_eq!(
            classify_recovery(&inventory(&format!("{DISTRO}\n")), &state).unwrap(),
            RecoveryDisposition::ManualReview
        );
        assert_eq!(
            classify_recovery(&inventory(&format!("{DISTRO}\n{DISTRO}\n")), &state).unwrap(),
            RecoveryDisposition::ManualReview
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn retry_reuses_reservation_and_only_advances_after_success() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let state = parent.join("state");
        let rootfs = parent.join("rootfs.tar");
        fs::write(&rootfs, b"payload").unwrap();
        let journal = BootstrapJournal::new(
            fixture_install_dir(&parent),
            format!("{:x}", Sha256::digest(b"payload")),
            "01234567-89ab-cdef-0123-456789abcdef".into(),
        )
        .unwrap();
        reserve(&state, &journal).unwrap();
        let runner = Sequence {
            outputs: Mutex::new(VecDeque::from([ok("docker-desktop\n"), ok("")])),
        };
        assert_eq!(
            retry_import(&runner, &state, &rootfs, 7).unwrap().state,
            BootstrapState::Imported
        );
        assert_eq!(
            load(&state).unwrap().unwrap().ownership_token,
            journal.ownership_token
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn removal_is_only_prepared_after_verified_token_proof() {
        let parent = temp();
        fs::create_dir_all(&parent).unwrap();
        let (state, mut journal) = imported_fixture(&parent);
        fs::create_dir_all(&journal.install_dir).unwrap();
        journal.advance(BootstrapState::Verified).unwrap();
        save(&state, &journal).unwrap();
        let runner = Sequence {
            outputs: Mutex::new(VecDeque::from([ok(&format!("{DISTRO}\n")), ok("")])),
        };
        let command = authorize_unregistration(&runner, &state).unwrap();
        assert_eq!(command.program, "wsl.exe");
        assert_eq!(command.args, ["--unregister", DISTRO]);

        let denied = Sequence {
            outputs: Mutex::new(VecDeque::from([
                ok(&format!("{DISTRO}\n")),
                Ok(ProcessOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: false,
                }),
            ])),
        };
        assert!(authorize_unregistration(&denied, &state).is_err());
        fs::remove_dir_all(parent).unwrap();
    }
}

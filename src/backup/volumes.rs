//! Named-volume transport reuses qualification's exact owned mountpoint
//! admission. Binary archives go through a short-lived owned staging file,
//! never through the bounded UTF-8 diagnostic stream or an agent-supplied path.
use super::*;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VolumeInspection {
    name: String,
    driver: String,
    scope: String,
    mountpoint: String,
    labels: std::collections::BTreeMap<String, String>,
    options: Option<serde_json::Value>,
}

fn query(runner: &dyn ProcessRunner, app: &InstalledApp, args: Vec<String>) -> AppResult<String> {
    let RuntimeSpec::Compose { project_dir, .. } = &app.runtime else {
        return Err(AppError::invalid("App has no managed project."));
    };
    let command = engine::project_command(
        project_dir,
        CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT),
    )?;
    let output = runner.run(&command)?;
    if !output.success || output.truncated {
        return Err(AppError::invalid(
            "Snapshot volume inspection failed or was incomplete.",
        ));
    }
    Ok(output.stdout)
}

fn project(app: &InstalledApp) -> AppResult<&str> {
    let RuntimeSpec::Compose { project_name, .. } = &app.runtime else {
        return Err(AppError::invalid("App has no managed project."));
    };
    Ok(project_name)
}

fn mountpoint(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    name: &str,
) -> AppResult<Option<String>> {
    if !crate::model::is_valid_installed_app_id(name) {
        return Err(AppError::invalid("Invalid reviewed volume name."));
    }
    let project = project(app)?;
    let full = format!("{project}_{name}");
    let listed = query(
        runner,
        app,
        vec![
            "volume".into(),
            "ls".into(),
            "--format".into(),
            "{{.Name}}".into(),
        ],
    )?;
    if !listed.lines().any(|row| row == full) {
        return Ok(None);
    }
    let inspected = query(
        runner,
        app,
        vec![
            "volume".into(),
            "inspect".into(),
            "--format".into(),
            "{{json .}}".into(),
            full.clone(),
        ],
    )?;
    let volume: VolumeInspection = serde_json::from_str(inspected.trim())
        .map_err(|_| AppError::invalid("Snapshot volume metadata is malformed."))?;
    let expected = format!("/var/lib/docker/volumes/{full}/_data");
    if volume.name != full
        || volume.driver != "local"
        || volume.scope != "local"
        || volume.mountpoint != expected
        || volume
            .labels
            .get("com.docker.compose.project")
            .map(String::as_str)
            != Some(project)
        || volume
            .labels
            .get("com.docker.compose.volume")
            .map(String::as_str)
            != Some(name)
        || volume.options.as_ref().is_some_and(|options| {
            !options.is_null() && options.as_object().is_none_or(|fields| !fields.is_empty())
        })
    {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "Snapshot volume ownership, driver or mountpoint differs from the reviewed app.",
        ));
    }
    let users = query(
        runner,
        app,
        vec![
            "container".into(),
            "ls".into(),
            "--quiet".into(),
            "--filter".into(),
            format!("volume={full}"),
        ],
    )?;
    if !users.trim().is_empty() {
        return Err(AppError::invalid(
            "A container still uses this volume; snapshot or restore cannot proceed.",
        ));
    }
    let canonical = wsl(
        runner,
        "/usr/bin/readlink",
        vec![
            "--canonicalize-existing".into(),
            "--".into(),
            expected.clone(),
        ],
    )?;
    if canonical.trim_end_matches('\n') != expected {
        return Err(AppError::new(
            ErrorCode::UnsafePath,
            "Snapshot volume mountpoint is a symlink or missing.",
        ));
    }
    Ok(Some(expected))
}

fn wsl(runner: &dyn ProcessRunner, executable: &str, args: Vec<String>) -> AppResult<String> {
    let mut command = CommandSpec::new(
        "wsl.exe",
        vec![
            "--distribution".into(),
            engine::wsl::DISTRO.into(),
            "--user".into(),
            "root".into(),
            "--cd".into(),
            "/".into(),
            "--exec".into(),
            executable.into(),
        ],
        None,
        Duration::from_secs(180),
    );
    command.args.extend(args);
    command.remove_env.push("WSLENV".into());
    let output = runner.run(&command)?;
    if !output.success || output.truncated {
        return Err(AppError::invalid(
            "Owned-volume archive operation failed or was incomplete.",
        ));
    }
    Ok(output.stdout)
}

struct Staging {
    root: PathBuf,
    file: PathBuf,
    linux_file: String,
}
impl Staging {
    fn new(project: &Path) -> AppResult<Self> {
        let nonce = crate::setup::generate_secret(32).map_err(AppError::invalid)?;
        let root = project.join(format!(".snapshot-{nonce}"));
        fs::create_dir(&root)?;
        let file = root.join("volume.tar");
        let linux_file = engine::wsl::windows_drive_path(
            crate::folders::docker_path(&file)
                .to_str()
                .ok_or_else(|| AppError::invalid("Snapshot staging path must be Unicode."))?,
        )?;
        Ok(Self {
            root,
            file,
            linux_file,
        })
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        // Only the one staging file and empty directory this operation created.
        // No recursive deletion and no paths recovered from the archive.
        let _ = fs::remove_file(&self.file);
        let _ = fs::remove_dir(&self.root);
    }
}

pub(super) fn capture(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    project: &Path,
    names: &[String],
    total: &mut usize,
) -> AppResult<Vec<VolumeEntry>> {
    if names.len() > 16 {
        return Err(AppError::invalid("Snapshot has too many named volumes."));
    }
    let mut entries = Vec::new();
    for name in names {
        let mount = mountpoint(runner, app, name)?
            .ok_or_else(|| AppError::invalid("Snapshot volume is missing."))?;
        let measured = wsl(
            runner,
            "/usr/bin/du",
            vec![
                "--bytes".into(),
                "--summarize".into(),
                "--one-file-system".into(),
                "--".into(),
                mount.clone(),
            ],
        )?;
        let (size, path) = measured
            .trim_end_matches('\n')
            .split_once('\t')
            .ok_or_else(|| AppError::invalid("Snapshot volume size is malformed."))?;
        let size: u64 = size
            .parse()
            .map_err(|_| AppError::invalid("Snapshot volume size is invalid."))?;
        if path != mount || size > (MAX_DATA_BYTES - *total) as u64 {
            return Err(AppError::invalid(
                "Snapshot exceeds the 256 MiB development bound.",
            ));
        }
        let stage = Staging::new(project)?;
        wsl(
            runner,
            "/usr/bin/tar",
            vec![
                "--create".into(),
                "--format=ustar".into(),
                "--file".into(),
                stage.linux_file.clone(),
                "--directory".into(),
                mount,
                "--one-file-system".into(),
                "--hard-dereference".into(),
                "--numeric-owner".into(),
                ".".into(),
            ],
        )?;
        reject_link(&stage.file)?;
        let mut bytes = Vec::new();
        fs::File::open(&stage.file)?
            .take((MAX_DATA_BYTES - *total + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_DATA_BYTES - *total {
            return Err(AppError::invalid(
                "Snapshot volume archive exceeds its bound.",
            ));
        }
        validate_tar(&bytes)?;
        *total += bytes.len();
        entries.push(VolumeEntry {
            name: name.clone(),
            sha256: hash(&bytes),
            length: bytes.len(),
            bytes,
        });
    }
    Ok(entries)
}

fn empty(runner: &dyn ProcessRunner, mount: &str) -> AppResult<()> {
    let entries = wsl(
        runner,
        "/usr/bin/find",
        vec![
            mount.into(),
            "-mindepth".into(),
            "1".into(),
            "-maxdepth".into(),
            "1".into(),
            "-print".into(),
            "-quit".into(),
        ],
    )?;
    if !entries.trim().is_empty() {
        return Err(AppError::invalid(
            "Restore volume contains data; nothing was overwritten.",
        ));
    }
    Ok(())
}

pub(super) fn admit_restore(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    entries: &[VolumeEntry],
) -> AppResult<()> {
    for entry in entries {
        validate_tar(&entry.bytes)?;
        if let Some(mount) = mountpoint(runner, app, &entry.name)? {
            empty(runner, &mount)?;
        }
    }
    Ok(())
}

pub(super) fn restore(
    runner: &dyn ProcessRunner,
    app: &InstalledApp,
    project_dir: &Path,
    entries: &[VolumeEntry],
) -> AppResult<()> {
    for entry in entries {
        if mountpoint(runner, app, &entry.name)?.is_none() {
            let name = format!("{}_{}", project(app)?, entry.name);
            query(
                runner,
                app,
                vec![
                    "volume".into(),
                    "create".into(),
                    "--driver".into(),
                    "local".into(),
                    "--label".into(),
                    format!("com.docker.compose.project={}", project(app)?),
                    "--label".into(),
                    format!("com.docker.compose.volume={}", entry.name),
                    name,
                ],
            )?;
        }
        let mount = mountpoint(runner, app, &entry.name)?
            .ok_or_else(|| AppError::invalid("Fresh restore volume was not created."))?;
        empty(runner, &mount)?;
        let stage = Staging::new(project_dir)?;
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stage.file)?;
        file.write_all(&entry.bytes)?;
        file.sync_all()?;
        drop(file);
        wsl(
            runner,
            "/usr/bin/tar",
            vec![
                "--extract".into(),
                "--file".into(),
                stage.linux_file.clone(),
                "--directory".into(),
                mount,
                "--same-owner".into(),
                "--same-permissions".into(),
                "--numeric-owner".into(),
            ],
        )?;
    }
    Ok(())
}

fn field(bytes: &[u8]) -> AppResult<String> {
    let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| AppError::invalid("Snapshot tar metadata must be UTF-8."))
}
fn octal(bytes: &[u8]) -> AppResult<usize> {
    let value = field(bytes)?.trim().to_owned();
    if value.is_empty() {
        return Ok(0);
    }
    usize::from_str_radix(&value, 8)
        .map_err(|_| AppError::invalid("Snapshot tar numeric field is invalid."))
}

/// Only POSIX ustar ordinary files/directories; refuse links, device nodes,
/// sockets, PAX/GNU path extensions and traversal before the first restore write.
pub(super) fn validate_tar(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() < 1024 || !bytes.len().is_multiple_of(512) || bytes.len() > MAX_DATA_BYTES {
        return Err(AppError::invalid("Snapshot tar framing is invalid."));
    }
    let mut offset = 0usize;
    let mut paths = BTreeSet::new();
    let mut count = 0;
    while offset + 512 <= bytes.len() {
        let header = &bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            if bytes.len() - offset < 1024 || bytes[offset..].iter().any(|byte| *byte != 0) {
                return Err(AppError::invalid("Snapshot tar terminator is invalid."));
            }
            return Ok(());
        }
        count += 1;
        if count > MAX_FILES || &header[257..263] != b"ustar\0" {
            return Err(AppError::invalid(
                "Snapshot archive is not bounded POSIX ustar.",
            ));
        }
        let checksum = octal(&header[148..156])?;
        let actual: usize = header
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    32
                } else {
                    *byte as usize
                }
            })
            .sum();
        if checksum != actual {
            return Err(AppError::invalid("Snapshot tar checksum is invalid."));
        }
        let name = field(&header[..100])?;
        let prefix = field(&header[345..500])?;
        let full = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let relative = full
            .strip_prefix("./")
            .unwrap_or(&full)
            .trim_end_matches('/');
        let root = relative == "." || relative.is_empty();
        let directory = header[156] == b'5';
        let length = octal(&header[124..136])?;
        // Never permit setuid/setgid/sticky modes in an app recovery archive.
        if (!root && (!safe_relative(relative) || !paths.insert(relative.to_owned())))
            || (root && !directory)
            || !matches!(header[156], 0 | b'0' | b'5')
            || (directory && length != 0)
            || octal(&header[100..108])? & 0o7000 != 0
        {
            return Err(AppError::invalid(
                "Snapshot tar contains an unsafe path, type or permission.",
            ));
        }
        let padded = length
            .checked_add(511)
            .map(|size| size / 512 * 512)
            .ok_or_else(|| AppError::invalid("Snapshot tar size overflow."))?;
        offset = offset
            .checked_add(512)
            .and_then(|value| value.checked_add(padded))
            .ok_or_else(|| AppError::invalid("Snapshot tar framing overflow."))?;
        if offset > bytes.len() {
            return Err(AppError::invalid("Snapshot tar data is truncated."));
        }
    }
    Err(AppError::invalid(
        "Snapshot tar has no complete terminator.",
    ))
}

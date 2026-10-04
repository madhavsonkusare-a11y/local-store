//! Owner-only removal of an empty, strictly product-owned WSL engine.
//! No app data, arbitrary directory, ambient engine or global WSL shutdown.
use crate::{
    error::{AppError, AppResult},
    model::RuntimeSpec,
    runtime::{
        self,
        engine::wsl::{self, bootstrap},
        CancelToken, CommandSpec, ProcessError, ProcessErrorCode, ProcessOutput, ProcessRunner,
        DIAGNOSTIC_TIMEOUT,
    },
    storage,
};
use fs4::FileExt;
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
pub struct RemovalPreview {
    pub distro: &'static str,
    pub removable: bool,
    pub deletes_engine_images_and_virtual_disk: bool,
    pub preserves_external_apps: bool,
}

struct Paths {
    state: PathBuf,
    data: PathBuf,
    apps: PathBuf,
    registry: PathBuf,
}
impl Paths {
    fn native() -> AppResult<Self> {
        let apps = storage::managed_apps_root();
        let registry = apps
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| AppError::invalid("Managed app configuration root is unavailable."))?
            .to_owned();
        Ok(Self {
            state: storage::managed_engine_state_root(),
            data: storage::managed_engine_data_root(),
            apps,
            registry,
        })
    }
}

fn plain(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(AppError::invalid(
                "Engine removal refuses Windows reparse points.",
            ));
        }
    }
    if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(AppError::invalid(
            "Engine removal refuses links and special files.",
        ));
    }
    Ok(())
}

fn checked_tree(path: &Path) -> AppResult<()> {
    // Check existing ancestry as well as the leaf; a linked parent must not
    // redirect this transaction to a foreign journal or disk.
    for ancestor in path.ancestors() {
        if ancestor.exists() {
            plain(ancestor)?;
        }
    }
    Ok(())
}

fn no_apps(paths: &Paths) -> AppResult<()> {
    checked_tree(&paths.apps)?;
    checked_tree(&paths.registry)?;
    for path in [
        storage::registry_v2_path_for_root(&paths.registry),
        storage::registry_v2_previous_path_for_root(&paths.registry),
        storage::primary_v1_path_for_root(&paths.registry),
        storage::legacy_v1_path_for_root(&paths.registry),
    ] {
        if path.exists() {
            plain(&path)?;
        }
    }
    // Never migrate an older registry while holding the removal lock. Its
    // format/ownership must be resolved through normal recovery first.
    if storage::primary_v1_path_for_root(&paths.registry).exists()
        || storage::legacy_v1_path_for_root(&paths.registry).exists()
    {
        return Err(AppError::invalid(
            "A legacy app registry remains. Review/migrate it before engine removal.",
        ));
    }
    let registry = if storage::registry_v2_path_for_root(&paths.registry).exists()
        || storage::registry_v2_previous_path_for_root(&paths.registry).exists()
    {
        storage::load_registry_v2_at(&paths.registry)?
    } else {
        storage::RegistryV2::new(Vec::new())
    };
    for app in &registry.apps {
        if let RuntimeSpec::Compose { project_dir, .. } = &app.runtime {
            // Saved bindings are checked rather than assuming the current
            // selection migrated an older app. All Compose apps are refused.
            runtime::engine::retained(project_dir)?;
            return Err(AppError::invalid(
                "Managed apps remain registered. Uninstall them explicitly before engine removal.",
            ));
        }
    }
    if paths.apps.exists() && fs::read_dir(&paths.apps)?.next().transpose()?.is_some() {
        return Err(AppError::invalid("Retained managed-app data or setup remains. Review each app and explicitly remove its data before engine removal."));
    }
    Ok(())
}

fn singleton(paths: &Paths, name: &str) -> AppResult<File> {
    let path = paths.state.join(name);
    if path.exists() {
        plain(&path)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    FileExt::try_lock(&file).map_err(|_| AppError::invalid("The engine supervisor is running or starting. Use engine stop-supervisor, wait for its exit, then preview removal again."))?;
    Ok(file)
}

fn checked_output(runner: &dyn ProcessRunner, spec: &CommandSpec) -> AppResult<String> {
    let out = runner.run(spec)?;
    if !out.success || out.truncated || !out.stderr.is_empty() {
        return Err(AppError::invalid("Engine removal could not establish a complete successful inventory or operation. Ownership records were preserved."));
    }
    Ok(out.stdout)
}

fn inventory_command(args: &[&str]) -> CommandSpec {
    let mut command = CommandSpec::new(
        "wsl.exe",
        vec![
            "--distribution".into(),
            wsl::DISTRO.into(),
            "--user".into(),
            "root".into(),
            "--exec".into(),
            "/usr/bin/env".into(),
            "-i".into(),
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin".into(),
            "HOME=/root".into(),
            "LANG=C.UTF-8".into(),
            "/usr/bin/docker".into(),
            "--host".into(),
            "unix:///var/run/docker.sock".into(),
        ],
        None,
        DIAGNOSTIC_TIMEOUT,
    );
    command.args.extend(args.iter().map(|s| (*s).into()));
    command.remove_env.push("WSLENV".into());
    command
}

fn empty_engine(runner: &dyn ProcessRunner) -> AppResult<()> {
    for args in [
        &["ps", "--all", "--quiet"][..],
        &["volume", "ls", "--quiet"][..],
    ] {
        let output = checked_output(runner, &inventory_command(args))?;
        if !output.bytes().all(|b| matches!(b, b'\r' | b'\n')) {
            return Err(AppError::invalid("Containers or volumes remain in the owned engine, or its inventory is ambiguous. Nothing was removed."));
        }
    }
    Ok(())
}

fn identity(paths: &Paths) -> AppResult<bootstrap::BootstrapJournal> {
    checked_tree(&paths.state)?;
    checked_tree(&paths.data)?;
    for name in [
        bootstrap::JOURNAL_FILE,
        bootstrap::TOKEN_FILE,
        "selected-engine.json",
        "engine-selection-configured",
        "removal-v1.json",
        "removal-v1.tmp",
    ] {
        let path = paths.state.join(name);
        if path.exists() {
            plain(&path)?;
        }
    }
    let journal = bootstrap::load(&paths.state)?
        .ok_or_else(|| AppError::invalid("Verified engine ownership journal is missing."))?;
    if journal.state != bootstrap::BootstrapState::Verified
        || journal.install_dir.canonicalize()? != paths.data.canonicalize()?
    {
        return Err(AppError::invalid("Removal requires the product-owned native engine directory. Adopted development or foreign engine locations are preserved."));
    }
    if crate::engine_setup::selected_at(&paths.state)?
        != Some(runtime::engine::EngineBinding::managed_wsl())
    {
        return Err(AppError::invalid(
            "Engine removal requires the explicit owned-engine selection.",
        ));
    }
    let mut disks = 0;
    for entry in fs::read_dir(&paths.data)? {
        let entry = entry?;
        plain(&entry.path())?;
        disks += 1;
        if entry.file_name() != "ext4.vhdx" || !entry.file_type()?.is_file() {
            return Err(AppError::invalid("Unexpected host files remain in the engine data directory. Review them separately; nothing was removed."));
        }
    }
    if disks != 1 {
        return Err(AppError::invalid(
            "The native engine virtual disk is missing or ambiguous; removal was refused.",
        ));
    }
    Ok(journal)
}

struct StrictOwnershipRunner<'a>(&'a dyn ProcessRunner);
impl ProcessRunner for StrictOwnershipRunner<'_> {
    fn run_cancellable(
        &self,
        spec: &CommandSpec,
        cancel: &CancelToken,
    ) -> Result<ProcessOutput, ProcessError> {
        let out = self.0.run_cancellable(spec, cancel)?;
        if out.truncated
            || !out.stderr.is_empty()
            || (spec.args.iter().any(|a| a == "/usr/bin/cmp") && !out.stdout.is_empty())
        {
            return Err(ProcessError::new(
                ProcessErrorCode::ProcessFailed,
                "Ownership verification returned incomplete or unexpected output.",
            ));
        }
        Ok(out)
    }
}

fn ready(
    runner: &dyn ProcessRunner,
    paths: &Paths,
) -> AppResult<(bootstrap::BootstrapJournal, CommandSpec)> {
    no_apps(paths)?;
    let journal = identity(paths)?;
    let command =
        bootstrap::authorize_unregistration(&StrictOwnershipRunner(runner), &paths.state)?;
    empty_engine(runner)?;
    Ok((journal, command))
}

fn with_locks<T>(paths: &Paths, action: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    let _engine = runtime::lock_operation("managed-engine")?;
    // Install locks are held before its registry commit; registry alone is not
    // sufficient to exclude a not-yet-registered install on this engine.
    let mut ids: Vec<_> = crate::offerings::offerings()
        .into_iter()
        .map(|o| o.id().to_owned())
        .collect();
    ids.sort();
    ids.dedup();
    let _apps: Vec<_> = ids
        .iter()
        .map(|id| runtime::lock_operation(id))
        .collect::<AppResult<_>>()?;
    let _registry = storage::lock_registry_at(&paths.registry)?;
    checked_tree(&paths.state)?;
    if !paths.state.is_dir() {
        return Err(AppError::invalid("Owned engine state is unavailable."));
    }
    let _spawn = singleton(paths, "supervisor-spawn.lock")?;
    let _worker = singleton(paths, "supervisor.lock")?;
    action()
}

/// A preview is never consent; execute rechecks every fact under the same locks.
pub fn preview(runner: &dyn ProcessRunner) -> AppResult<RemovalPreview> {
    let paths = Paths::native()?;
    with_locks(&paths, || {
        ready(runner, &paths)?;
        Ok(RemovalPreview {
            distro: wsl::DISTRO,
            removable: true,
            deletes_engine_images_and_virtual_disk: true,
            preserves_external_apps: true,
        })
    })
}

fn execute_at(
    runner: &dyn ProcessRunner,
    paths: &Paths,
    confirmation: &str,
    consent: bool,
) -> AppResult<()> {
    if !consent || confirmation != wsl::DISTRO {
        return Err(AppError::invalid("Removing the empty owned engine requires --consent and --confirm local-store-engine-v1. Its cached images and virtual disk are deleted."));
    }
    with_locks(paths, || {
        let (journal, _) = ready(runner, paths)?;
        let (current, command) = ready(runner, paths)?;
        if current != journal {
            return Err(AppError::invalid(
                "Engine identity changed after preview; removal was refused.",
            ));
        }
        let state_files: Vec<_> = [
            bootstrap::JOURNAL_FILE,
            bootstrap::TOKEN_FILE,
            "selected-engine.json",
            "engine-selection-configured",
        ]
        .iter()
        .map(|name| Ok((*name, fs::read(paths.state.join(name))?)))
        .collect::<AppResult<_>>()?;
        storage::write_file_atomically(&paths.state.join("removal-v1.json"), br#"{"schema_version":1,"distro":"local-store-engine-v1","phase":"unregister_requested"}"#)?;
        checked_output(runner, &command)?;
        let mut list = CommandSpec::new(
            "wsl.exe",
            vec!["--list".into(), "--quiet".into()],
            None,
            DIAGNOSTIC_TIMEOUT,
        );
        list.remove_env.push("WSLENV".into());
        let names = bootstrap::parse_distro_names(&checked_output(runner, &list)?)?;
        if names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(wsl::DISTRO))
        {
            return Err(AppError::invalid(
                "The owned distribution is still registered; ownership records were retained.",
            ));
        }
        // No recursive deletion: WSL owns removal of its disk. Foreign host
        // contents and changed identity files always survive a cleanup refusal.
        checked_tree(&paths.data)?;
        if paths.data.exists() && fs::read_dir(&paths.data)?.next().transpose()?.is_some() {
            return Err(AppError::invalid("WSL returned but engine host files remain. Ownership records were retained for manual review."));
        }
        for (name, bytes) in &state_files {
            plain(&paths.state.join(name))?;
            if fs::read(paths.state.join(name))? != *bytes {
                return Err(AppError::invalid(
                    "Engine identity files changed during removal. Remaining state was preserved.",
                ));
            }
        }
        let cleanup: AppResult<()> = (|| {
            for (name, _) in &state_files {
                fs::remove_file(paths.state.join(name))?;
            }
            if paths.data.exists() {
                fs::remove_dir(&paths.data)?;
            }
            storage::write_file_atomically(
                &paths.state.join("removal-v1.json"),
                br#"{"schema_version":1,"distro":"local-store-engine-v1","phase":"removed"}"#,
            )?;
            Ok(())
        })();
        if let Err(error) = cleanup {
            // Preserve/restore every original identity byte if local cleanup
            // fails after WSL removal. The absent distro then forces manual
            // review rather than allowing a blind destructive retry.
            let restore: AppResult<()> = (|| {
                checked_tree(&paths.state)?;
                for (name, bytes) in &state_files {
                    let path = paths.state.join(name);
                    if path.exists() {
                        plain(&path)?;
                        if fs::read(&path)? != *bytes {
                            return Err(AppError::invalid("Refusing to overwrite changed engine state during cleanup recovery."));
                        }
                    } else {
                        storage::write_file_atomically(&path, bytes)?;
                    }
                }
                Ok(())
            })();
            return match restore {
                Ok(()) => Err(error),
                Err(recovery) => Err(AppError::rollback(error, recovery)),
            };
        }
        Ok(())
    })
}

/// Owner CLI only. No IPC, MCP tool, supplied path or arbitrary distro input.
pub fn execute(runner: &dyn ProcessRunner, confirmation: &str, consent: bool) -> AppResult<()> {
    execute_at(runner, &Paths::native()?, confirmation, consent)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{CancelToken, ProcessError, ProcessOutput};
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    };

    struct Fixture {
        root: PathBuf,
        paths: Paths,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let root = std::env::temp_dir().join(format!(
                "local-store-removal-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let paths = Paths {
                state: root.join("engine/wsl-state"),
                data: root.join("engine/wsl-data"),
                apps: root.join("config/local-store/apps"),
                registry: root.join("config"),
            };
            fs::create_dir_all(&paths.state).unwrap();
            fs::create_dir_all(&paths.data).unwrap();
            fs::create_dir_all(&paths.apps).unwrap();
            Self { root, paths }
        }
        #[cfg(windows)]
        fn journal(&self) {
            let mut journal = bootstrap::BootstrapJournal::new(
                self.paths.data.clone(),
                "a".repeat(64),
                "b".repeat(64),
            )
            .unwrap();
            journal.state = bootstrap::BootstrapState::Verified;
            fs::write(
                self.paths.state.join(bootstrap::JOURNAL_FILE),
                serde_json::to_vec(&journal).unwrap(),
            )
            .unwrap();
            fs::write(self.paths.state.join(bootstrap::TOKEN_FILE), "b".repeat(64)).unwrap();
            fs::write(
                self.paths.state.join("selected-engine.json"),
                serde_json::to_vec(&runtime::engine::EngineBinding::managed_wsl()).unwrap(),
            )
            .unwrap();
            fs::write(
                self.paths.state.join("engine-selection-configured"),
                "configured\n",
            )
            .unwrap();
            fs::write(self.paths.data.join("ext4.vhdx"), b"synthetic virtual disk").unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    struct Fake {
        data: PathBuf,
        calls: Mutex<Vec<Vec<String>>>,
        removed: Mutex<bool>,
        containers: bool,
        volumes: bool,
        fail_unregister: bool,
        lose_unregister_ack: bool,
        truncate_inventory: bool,
        malformed_inventory: bool,
        change_on_recheck: bool,
        volume_checks: Mutex<usize>,
    }
    impl Fake {
        fn new(data: PathBuf) -> Self {
            Self {
                data,
                calls: Mutex::new(Vec::new()),
                removed: Mutex::new(false),
                containers: false,
                volumes: false,
                fail_unregister: false,
                lose_unregister_ack: false,
                truncate_inventory: false,
                malformed_inventory: false,
                change_on_recheck: false,
                volume_checks: Mutex::new(0),
            }
        }
        fn unregistered(&self) -> bool {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .any(|a| a.first().is_some_and(|s| s == "--unregister"))
        }
    }
    impl ProcessRunner for Fake {
        fn run_cancellable(
            &self,
            command: &CommandSpec,
            _: &CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            assert_eq!(command.program, "wsl.exe");
            assert!(!command
                .args
                .iter()
                .any(|a| a == "--shutdown" || a == "--terminate"));
            self.calls.lock().unwrap().push(command.args.clone());
            let mut out = ProcessOutput {
                success: true,
                ..Default::default()
            };
            match command.args.first().map(String::as_str) {
                Some("--list") => {
                    out.stdout = if *self.removed.lock().unwrap() {
                        "docker-desktop\nOther-user-distro\n".into()
                    } else {
                        format!("docker-desktop\n{}\nOther-user-distro\n", wsl::DISTRO)
                    };
                }
                Some("--unregister") => {
                    assert_eq!(command.args, ["--unregister", wsl::DISTRO]);
                    out.success = !self.fail_unregister;
                    if out.success {
                        *self.removed.lock().unwrap() = true;
                        fs::remove_file(self.data.join("ext4.vhdx")).unwrap();
                        if self.lose_unregister_ack {
                            out.success = false;
                        }
                    }
                }
                _ if command.args.iter().any(|a| a == "/usr/bin/docker") => {
                    out.truncated = self.truncate_inventory;
                    if self.malformed_inventory {
                        out.stdout = " \t".into();
                    }
                    if command.args.iter().any(|a| a == "ps") && self.containers {
                        out.stdout = "owned-or-foreign-container\n".into();
                    }
                    if command.args.iter().any(|a| a == "volume") {
                        let mut count = self.volume_checks.lock().unwrap();
                        *count += 1;
                        if self.volumes || (self.change_on_recheck && *count > 1) {
                            out.stdout = "retained-volume\n".into();
                        }
                    }
                }
                _ => {}
            }
            Ok(out)
        }
    }

    #[test]
    fn wrong_confirmation_and_missing_consent_do_not_probe_or_mutate() {
        let fixture = Fixture::new();
        let runner = Fake::new(fixture.paths.data.clone());
        assert!(execute_at(&runner, &fixture.paths, "foreign-distro", true).is_err());
        assert!(execute_at(&runner, &fixture.paths, wsl::DISTRO, false).is_err());
        assert!(runner.calls.lock().unwrap().is_empty());
        assert!(!fixture.paths.state.join("removal-v1.json").exists());
    }

    #[test]
    fn retained_data_legacy_and_malformed_registries_are_refused() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.paths.apps.join("unregistered-private-data")).unwrap();
        assert!(no_apps(&fixture.paths).is_err());
        fs::remove_dir(fixture.paths.apps.join("unregistered-private-data")).unwrap();
        let legacy = storage::primary_v1_path_for_root(&fixture.paths.registry);
        fs::write(&legacy, "[]").unwrap();
        assert!(no_apps(&fixture.paths).is_err());
        fs::remove_file(legacy).unwrap();
        fs::write(
            storage::registry_v2_path_for_root(&fixture.paths.registry),
            "{",
        )
        .unwrap();
        assert!(no_apps(&fixture.paths).is_err());
    }

    #[test]
    fn saved_managed_bindings_block_removal_and_external_registry_is_preserved() {
        let fixture = Fixture::new();
        let app = crate::model::InstalledApp {
            id: "linked".into(),
            catalog_id: None,
            display_name: "Linked app".into(),
            launch_url: "http://127.0.0.1:12345".into(),
            icon_path: None,
            runtime: RuntimeSpec::External,
            created_at_unix: 1,
            updated_at_unix: 1,
        };
        storage::save_registry_v2_at(
            &fixture.paths.registry,
            &storage::RegistryV2::new(vec![app.clone()]),
        )
        .unwrap();
        let path = storage::registry_v2_path_for_root(&fixture.paths.registry);
        let original = fs::read(&path).unwrap();
        no_apps(&fixture.paths).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        let mut managed = app;
        managed.runtime = RuntimeSpec::Compose {
            project_name: "local-store-linked".into(),
            project_dir: fixture.paths.data.clone(),
            compose_file: fixture.paths.data.join("compose.yaml"),
        };
        storage::save_registry_v2_at(
            &fixture.paths.registry,
            &storage::RegistryV2::new(vec![managed]),
        )
        .unwrap();
        assert!(no_apps(&fixture.paths).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn foreign_selection_adopted_directory_and_reparse_ancestry_are_refused() {
        let fixture = Fixture::new();
        fixture.journal();
        fs::write(fixture.paths.state.join("selected-engine.json"), br#"{"schema_version":1,"program":"docker","endpoint":"npipe:////./pipe/docker_engine"}"#).unwrap();
        assert!(identity(&fixture.paths).is_err());
        fixture.journal();
        let journal_path = fixture.paths.state.join(bootstrap::JOURNAL_FILE);
        let mut journal = bootstrap::load(&fixture.paths.state).unwrap().unwrap();
        journal.install_dir = fixture.paths.apps.clone();
        fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
        assert!(identity(&fixture.paths).is_err());
        let link = fixture.root.join("linked-folder");
        let result = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference = 'Stop'\nNew-Item -ItemType Junction -Path $env:LOCAL_STORE_FIXTURE_LINK -Target $env:LOCAL_STORE_FIXTURE_TARGET | Out-Null"])
            .env("LOCAL_STORE_FIXTURE_LINK", &link)
            .env("LOCAL_STORE_FIXTURE_TARGET", &fixture.paths.data)
            .output().unwrap();
        assert!(
            result.status.success(),
            "Could not create the isolated NTFS junction fixture: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(plain(&link).is_err());
        assert!(checked_tree(&link.join("ext4.vhdx")).is_err());
        fs::remove_dir(&link).unwrap();
    }

    #[test]
    fn incomplete_or_nonempty_engine_inventory_is_never_empty() {
        for case in 0..4 {
            let fixture = Fixture::new();
            let mut runner = Fake::new(fixture.paths.data.clone());
            match case {
                0 => runner.containers = true,
                1 => runner.volumes = true,
                2 => runner.truncate_inventory = true,
                _ => runner.malformed_inventory = true,
            }
            assert!(empty_engine(&runner).is_err());
            assert!(!runner.unregistered());
        }
    }

    #[test]
    fn worker_lock_blocks_removal_even_without_fresh_status() {
        let fixture = Fixture::new();
        let held = singleton(&fixture.paths, "supervisor.lock").unwrap();
        assert!(singleton(&fixture.paths, "supervisor.lock").is_err());
        drop(held);
        assert!(singleton(&fixture.paths, "supervisor.lock").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn transaction_rechecks_preserves_failures_and_removes_only_empty_owned_engine() {
        // These are injected process/temporary-filesystem transactions. No WSL
        // command is executed, including in the successful unregister case.
        for case in 0..8 {
            let fixture = Fixture::new();
            fixture.journal();
            let mut runner = Fake::new(fixture.paths.data.clone());
            let original = fs::read(fixture.paths.state.join(bootstrap::JOURNAL_FILE)).unwrap();
            match case {
                0 => runner.containers = true,
                1 => runner.volumes = true,
                2 => runner.change_on_recheck = true,
                3 => runner.fail_unregister = true,
                4 => {
                    fs::write(fixture.paths.data.join("owner-file.txt"), "preserve me").unwrap();
                }
                6 => runner.lose_unregister_ack = true,
                _ => {}
            }
            #[cfg(windows)]
            let _held_token = if case == 7 {
                use std::os::windows::fs::OpenOptionsExt;
                Some(
                    OpenOptions::new()
                        .read(true)
                        .share_mode(1)
                        .open(fixture.paths.state.join(bootstrap::TOKEN_FILE))
                        .unwrap(),
                )
            } else {
                None
            };
            let result = execute_at(&runner, &fixture.paths, wsl::DISTRO, true);
            if case == 5 {
                result.unwrap();
                assert!(runner.unregistered());
                assert!(!fixture.paths.data.exists());
                assert!(!fixture.paths.state.join(bootstrap::JOURNAL_FILE).exists());
                assert!(!fixture.paths.state.join(bootstrap::TOKEN_FILE).exists());
                assert!(!fixture.paths.state.join("selected-engine.json").exists());
                assert!(fixture.paths.apps.exists());
                assert!(
                    fs::read_to_string(fixture.paths.state.join("removal-v1.json"))
                        .unwrap()
                        .contains("removed")
                );
            } else {
                assert!(result.is_err());
                assert_eq!(
                    fs::read(fixture.paths.state.join(bootstrap::JOURNAL_FILE)).unwrap(),
                    original
                );
                assert_eq!(
                    fixture.paths.data.join("ext4.vhdx").exists(),
                    case != 6 && case != 7
                );
                assert_eq!(runner.unregistered(), case == 3 || case == 6 || case == 7);
            }
        }
    }
}
#[cfg(all(test, windows))]
#[path = "engine_removal/real_test.rs"]
mod real_test;

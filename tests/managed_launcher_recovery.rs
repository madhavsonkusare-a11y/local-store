//! Real owner-recovery parity for launcher IPC's underlying functions.
//! Run serially on the owned WSL engine, never against an existing Memos app.
//! This proves backend actions/projections, not a native Windows WebView flow.
#![cfg(windows)]

use local_store::{
    launcher_projection::{self, OnboardingStep, OnboardingStore, RecoveryStage},
    model::InstalledApp,
    qualification::{Bystanders, FirstUse, Isolation, Phase, ScriptProbe, StepResult},
    recovery::{self, OwnershipStatus, RecoveryCandidate},
    runtime::{
        self,
        engine::{self, wsl::bootstrap, EngineBinding, EngineRunner},
        CommandSpec, ProcessRunner, SystemProcessRunner, DIAGNOSTIC_TIMEOUT,
    },
    storage,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const PROJECT: &str = "local-store-memos";

struct RestoreConfig {
    appdata: Option<OsString>,
    xdg: Option<OsString>,
}
impl RestoreConfig {
    fn capture() -> Self {
        Self {
            appdata: std::env::var_os("APPDATA"),
            xdg: std::env::var_os("XDG_CONFIG_HOME"),
        }
    }
}
impl Drop for RestoreConfig {
    fn drop(&mut self) {
        for (key, value) in [("APPDATA", &self.appdata), ("XDG_CONFIG_HOME", &self.xdg)] {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

/// Cleanup uses the exact recovery ownership/path check. A label-only cleanup
/// guard is inappropriate here because production recovery requires offering ID
/// `memos`, and therefore its normal Compose project name.
struct FixtureCleanup {
    root: PathBuf,
    active: bool,
}
impl FixtureCleanup {
    fn cleanup(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let expected = self.root.join("local-store/apps/memos");
        if storage::managed_apps_root().join("memos") != expected {
            return Err("fixture configuration root changed before cleanup".into());
        }
        let registry = storage::load_or_migrate_registry().map_err(|error| error.to_string())?;
        if registry.apps.iter().any(|app| app.id == "memos") {
            storage::remove_installed_app("memos").map_err(|error| error.to_string())?;
        }
        if expected.join("compose.yaml").is_file() {
            recovery::discard("memos", true).map_err(|error| error.message)?;
        }
        self.active = false;
        Ok(())
    }
}
impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!(
                "Fixture recovery cleanup needs review: {}",
                runtime::redact(&error)
            );
        }
    }
}

fn record<T>(
    steps: &mut Vec<StepResult>,
    label: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let result = work();
    steps.push(StepResult {
        step: label.into(),
        passed: result.is_ok(),
        detail: result.as_ref().err().map(|error| runtime::redact(error)),
    });
    result
}

fn query(runner: &dyn ProcessRunner, args: Vec<String>) -> Result<String, String> {
    let output = runner
        .run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT))
        .map_err(|error| error.message)?;
    if !output.success || output.truncated {
        return Err("owned engine returned failed or incomplete resource inspection".into());
    }
    Ok(output.stdout)
}

fn refuse_collision(runner: &dyn ProcessRunner) -> Result<(), String> {
    for (kind, list) in [("container", "-aq"), ("network", "-q"), ("volume", "-q")] {
        if !query(
            runner,
            vec![
                kind.into(),
                "ls".into(),
                list.into(),
                "--filter".into(),
                format!("label=com.docker.compose.project={PROJECT}"),
            ],
        )?
        .trim()
        .is_empty()
        {
            return Err(
                "Memos already owns resources on this engine; recovery proof will not touch them"
                    .into(),
            );
        }
    }
    if !query(
        runner,
        vec![
            "container".into(),
            "ls".into(),
            "-aq".into(),
            "--filter".into(),
            format!("name={PROJECT}"),
        ],
    )?
    .trim()
    .is_empty()
    {
        return Err("the Memos container name is already in use; nothing was installed".into());
    }
    Ok(())
}

fn container_states(runner: &dyn ProcessRunner) -> Result<BTreeMap<String, String>, String> {
    let text = query(
        runner,
        vec![
            "container".into(),
            "ls".into(),
            "--all".into(),
            "--no-trunc".into(),
            "--format".into(),
            "{{.ID}}\t{{.State}}".into(),
        ],
    )?;
    let mut states = BTreeMap::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let (id, state) = line
            .split_once('\t')
            .ok_or("container state inventory was malformed")?;
        if id.len() != 64
            || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || state.is_empty()
            || states.insert(id.into(), state.into()).is_some()
        {
            return Err("container state inventory was invalid or duplicated".into());
        }
    }
    Ok(states)
}

fn candidate(expected: OwnershipStatus) -> Result<RecoveryCandidate, String> {
    let mut candidates = recovery::inspect().map_err(|error| error.message)?;
    if candidates.len() != 1 || candidates[0].recipe_id != "memos" {
        return Err("the isolated launcher did not report exactly one retained Memos setup".into());
    }
    let mut candidate = candidates.remove(0);
    recovery::verify_with(&mut candidate, &SystemProcessRunner).map_err(|error| error.message)?;
    if candidate.ownership_status != expected {
        return Err("the launcher recovery ownership state did not match the real engine".into());
    }
    Ok(candidate)
}

fn installed() -> Result<InstalledApp, String> {
    let registry = storage::load_or_migrate_registry().map_err(|error| error.to_string())?;
    if registry.apps.len() != 1 || registry.apps[0].id != "memos" {
        return Err("recovery did not commit exactly one isolated Memos app".into());
    }
    Ok(registry.apps[0].clone())
}

fn data_hashes(data: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, String>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if metadata.is_symlink() {
                return Err("fixture data unexpectedly contains a link".into());
            }
            if metadata.is_dir() {
                walk(root, &path, files)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .to_path_buf();
                files.insert(
                    relative,
                    format!(
                        "{:x}",
                        Sha256::digest(fs::read(&path).map_err(|error| error.to_string())?)
                    ),
                );
            } else {
                return Err("fixture data contains a non-file object".into());
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(data, data, &mut files)?;
    if files.is_empty() {
        return Err("Memos produced no persisted files to preserve".into());
    }
    Ok(files)
}

#[test]
#[ignore = "real owner recovery; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1 and select the owned engine"]
fn managed_launcher_recovers_lost_registry_and_preserves_private_memo() {
    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_MANAGED_QUALIFICATION").as_deref(),
        Ok("1")
    );
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = repo.join(".cache/managed-qualification");
    fs::create_dir_all(&scratch).unwrap();
    // Same cross-process proof slot as the qualification harness. Refuse busy
    // rather than running beside another app proof on the shared daemon.
    let slot = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(scratch.join("qualification.lock"))
        .unwrap();
    fs4::FileExt::try_lock(&slot)
        .expect("another qualification is active; retry after it finishes");
    let state_dir = repo.join(".cache/engine/real-wsl-proof/state");
    let binding = EngineBinding::managed_wsl();
    let runner = EngineRunner {
        inner: &SystemProcessRunner,
        binding: Some(binding.clone()),
    };
    let mut steps = Vec::new();
    let status = record(
        &mut steps,
        "verified owned engine and explicit native selection",
        || {
            let status = bootstrap::status(
                &SystemProcessRunner,
                &state_dir,
                &storage::managed_engine_data_root(),
            )
            .map_err(|error| error.message)?;
            if status.daemon != bootstrap::ManagedDaemonState::Responsive
                || EngineBinding::discover(&SystemProcessRunner).map_err(|error| error.message)?
                    != binding
            {
                return Err("the owned engine is not explicitly selected and responsive".into());
            }
            Ok(status)
        },
    )
    .expect("owned engine preflight");
    record(
        &mut steps,
        "refuse existing Memos resources before fixture setup",
        || refuse_collision(&runner),
    )
    .expect("Memos project must be unused");
    let before = container_states(&runner).unwrap();
    let bystanders = Bystanders::note(&runner).unwrap();
    let isolation = Isolation::new("launcher-recovery", &scratch).unwrap();
    let _restore = RestoreConfig::capture();
    isolation.take_over_config_root();
    let mut cleanup = FixtureCleanup {
        root: isolation.root.clone(),
        active: true,
    };
    let project = storage::managed_apps_root().join("memos");
    let probe = ScriptProbe::new(
        repo.join("scripts/memos-content-probe.mjs"),
        "exact private memo survives owner recovery",
    )
    .with_args(vec![isolation
        .root
        .join("private-memo-state.json")
        .to_string_lossy()
        .into_owned()]);
    let result = (|| -> Result<(), String> {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        drop(listener);
        let recipe = local_store::recipes::recipe("memos")
            .ok_or("Memos recipe missing")?
            .with_host_port(port)?;
        let app = record(
            &mut steps,
            "install isolated Memos on the owned engine",
            || {
                let lock = runtime::lock_operation("memos").map_err(|error| error.message)?;
                runtime::begin_install_on_engine(&recipe, lock, &binding, &|_| {})
                    .and_then(|pending| pending.commit(&|_| {}))
                    .map_err(|error| error.message)
            },
        )?;
        let address = app.launch_url.clone();
        record(
            &mut steps,
            "create first admin and exact private memo",
            || probe.exercise(Phase::FirstInstall, &address),
        )?;
        let onboarding_root = isolation.root.join("local-store");
        record(
            &mut steps,
            "persist viewed onboarding steps without granting engine permissions",
            || {
                let mut onboarding =
                    OnboardingStore::open(&onboarding_root).map_err(|error| error.message)?;
                onboarding
                    .mark_viewed(OnboardingStep::WelcomeViewed)
                    .map_err(|error| error.message)?;
                onboarding
                    .mark_viewed(OnboardingStep::EngineExplanationViewed)
                    .map_err(|error| error.message)?;
                Ok(())
            },
        )?;
        record(
            &mut steps,
            "simulate lost registry only inside the isolated root",
            || {
                let _lock = storage::lock_registry_at(&isolation.root)
                    .map_err(|error| error.to_string())?;
                for (path, name) in [
                    (
                        storage::registry_v2_path_for_root(&isolation.root),
                        "lost-registry-v2.json",
                    ),
                    (
                        storage::registry_v2_previous_path_for_root(&isolation.root),
                        "lost-registry-previous.json",
                    ),
                ] {
                    if path.is_file() {
                        fs::rename(path, isolation.root.join(name))
                            .map_err(|error| error.to_string())?;
                    }
                }
                if !storage::load_or_migrate_registry_at(&isolation.root)
                    .map_err(|error| error.to_string())?
                    .apps
                    .is_empty()
                {
                    return Err("lost registry still listed an installed app".into());
                }
                Ok(())
            },
        )?;
        record(
            &mut steps,
            "launcher projects verified retained setup with persisted viewed steps",
            || {
                let retained = candidate(OwnershipStatus::Verified)?;
                let progress =
                    OnboardingStore::read(&onboarding_root).map_err(|error| error.message)?;
                let projection =
                    launcher_projection::launcher_state(&status, &[], &[retained], &progress);
                if projection.onboarding.retained_setups != 1
                    || projection.recovery[0].stage != RecoveryStage::OwnershipVerified
                    || projection.progress.viewed.len() != 2
                {
                    return Err(
                        "launcher recovery projection lost actual ownership or onboarding facts"
                            .into(),
                    );
                }
                Ok(())
            },
        )?;
        record(
            &mut steps,
            "owner recovery adoption restores the exact address and engine binding",
            || {
                let adopted = recovery::adopt("memos").map_err(|error| error.message)?;
                if adopted.launch_url != address
                    || adopted.containers != 1
                    || engine::retained(&project).map_err(|error| error.message)?
                        != Some(binding.clone())
                {
                    return Err("recovery adoption changed the fixture address or engine".into());
                }
                installed()?;
                Ok(())
            },
        )?;
        record(
            &mut steps,
            "read the exact private memo after lost-registry recovery",
            || probe.exercise(Phase::AfterRestart, &address),
        )?;
        record(
            &mut steps,
            "owner uninstall keeps fixture data and removes its registry entry",
            || runtime::uninstall_and_remove(&installed()?, false).map_err(|error| error.message),
        )?;
        let hashes = record(
            &mut steps,
            "snapshot actual persisted Memos files while stopped",
            || data_hashes(&project.join("data")),
        )?;
        record(
            &mut steps,
            "launcher projects retained setup with no containers",
            || {
                let projection =
                    launcher_projection::recovery(&candidate(OwnershipStatus::NoContainers)?);
                if projection.stage != RecoveryStage::NoContainers {
                    return Err(
                        "launcher did not report the real no-container recovery state".into(),
                    );
                }
                Ok(())
            },
        )?;
        record(
            &mut steps,
            "owner discard preserves setup, engine binding and every data file",
            || {
                let discarded = recovery::discard("memos", false).map_err(|error| error.message)?;
                if discarded.data_deleted
                    || discarded.containers_removed != 0
                    || !project.join("compose.yaml").is_file()
                    || engine::retained(&project).map_err(|error| error.message)?
                        != Some(binding.clone())
                    || data_hashes(&project.join("data"))? != hashes
                {
                    return Err(
                        "keep-data recovery discard changed retained setup or persisted data"
                            .into(),
                    );
                }
                candidate(OwnershipStatus::NoContainers)?;
                Ok(())
            },
        )?;
        record(
            &mut steps,
            "re-adopt retained setup through the locked owner recovery action",
            || {
                recovery::adopt("memos")
                    .map(|_| ())
                    .map_err(|error| error.message)
            },
        )?;
        record(
            &mut steps,
            "read the exact private memo after keep-data discard and re-adoption",
            || probe.exercise(Phase::AfterReinstall, &address),
        )?;
        Ok(())
    })();
    record(
        &mut steps,
        "remove only the verified fixture and its test data",
        || cleanup.cleanup(),
    )
    .ok();
    record(
        &mut steps,
        "existing containers retain their exact IDs and states",
        || {
            bystanders.survived(&runner)?;
            if container_states(&runner)? != before {
                return Err("container IDs or states changed outside the removed fixture".into());
            }
            Ok(())
        },
    )
    .ok();
    let passed = result.is_ok() && steps.iter().all(|step| step.passed);
    let proof = serde_json::json!({
        "schema_version": 1, "recorded_at_unix": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "passed": passed, "scope": "Windows launcher owner recovery functions and projections on owned WSL engine; not native WebView or clean-machine proof",
        "app": "memos", "engine": binding,
        "test_sha256": format!("{:x}", Sha256::digest(include_bytes!("managed_launcher_recovery.rs"))),
        "content_probe_sha256": format!("{:x}", Sha256::digest(include_bytes!("../scripts/memos-content-probe.mjs"))),
        "steps": steps,
    });
    fs::write(
        repo.join("docs/evidence/windows-launcher-recovery-2026-10-01.json"),
        format!("{}\n", serde_json::to_string_pretty(&proof).unwrap()),
    )
    .unwrap();
    assert!(
        passed,
        "managed launcher recovery failed: {:?}",
        proof["steps"]
    );
}

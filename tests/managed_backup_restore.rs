//! Opt-in proof: encrypted consistent snapshots restore into different app
//! projects and named volumes, then existing real-content probes read them.
#![cfg(windows)]
use local_store::{
    backup,
    model::{InstalledApp, RuntimeSpec},
    qualification::{Bystanders, FirstUse, Isolation, Phase, ScriptProbe},
    runtime::{
        self,
        engine::{self, EngineBinding, EngineRunner},
        SystemProcessRunner,
    },
    storage,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Environment {
    appdata: Option<OsString>,
    xdg: Option<OsString>,
}
impl Drop for Environment {
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
struct Cleanup {
    apps: Vec<InstalledApp>,
    state: PathBuf,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        for app in &self.apps {
            if !app.id.contains("-qualify-") {
                continue;
            }
            if let RuntimeSpec::Compose { project_dir, .. } = &app.runtime {
                if project_dir.starts_with(storage::managed_apps_root())
                    && project_dir.join("compose.yaml").is_file()
                {
                    let _ = runtime::uninstall_with(&SystemProcessRunner, app, true);
                    let _ = storage::remove_installed_app(&app.id);
                }
            }
        }
        let _ = fs::remove_file(&self.state);
    }
}

fn prove(id: &str) -> Result<serde_json::Value, String> {
    let map = |error: local_store::error::AppError| error.message;
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let _environment = Environment {
        appdata: std::env::var_os("APPDATA"),
        xdg: std::env::var_os("XDG_CONFIG_HOME"),
    };
    let isolation = Isolation::new(id, &repo.join(".cache/backup-proof")).map_err(map)?;
    isolation.take_over_config_root();
    let binding = EngineBinding::managed_wsl();
    let runner = EngineRunner {
        inner: &SystemProcessRunner,
        binding: Some(binding.clone()),
    };
    let bystanders = Bystanders::note(&runner).map_err(map)?;
    let state = isolation.root.join("private-probe-state.json");
    let script = match id {
        "memos" => "memos-content-probe.mjs",
        "gitea" => "gitea-repository-probe.mjs",
        _ => return Err("unsupported proof app".into()),
    };
    let probe = ScriptProbe::new(
        repo.join("scripts").join(script),
        "private content survives a consistent encrypted snapshot and fresh isolated restore",
    )
    .with_args(vec![state.to_string_lossy().into_owned()]);
    let mut cleanup = Cleanup {
        apps: vec![],
        state: state.clone(),
    };
    let mut template = local_store::offerings::offering(id)
        .ok_or("reviewed offering missing")?
        .plan_template(None)
        .map_err(map)?;
    template.plan.id = isolation.run_id.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    drop(listener);
    template.plan.set_published_host(port);
    let mut source =
        runtime::install_template(&template, "Isolated backup source", &BTreeMap::new())
            .map_err(map)?;
    source.catalog_id = Some(id.into());
    cleanup.apps.push(source.clone());
    runtime::wait_for_health(&source.launch_url, Duration::from_secs(180)).map_err(map)?;
    probe.exercise(Phase::FirstInstall, &source.launch_url)?;
    let root = storage::managed_apps_root();
    let source_project = match &source.runtime {
        RuntimeSpec::Compose { project_dir, .. } => project_dir.clone(),
        _ => unreachable!(),
    };
    let credential = serde_json::to_vec(&serde_json::json!({"backup-fixture":local_store::setup::generate_secret(64).map_err(|error| error.to_string())?})).unwrap();
    fs::write(source_project.join(runtime::SECRETS_FILE), &credential)
        .map_err(|error| error.to_string())?;
    let (encrypted, receipt) =
        backup::snapshot_with(&SystemProcessRunner, &source, &root, true).map_err(map)?;
    let private_marker = b"Local Store Gitea";
    if encrypted
        .windows(private_marker.len())
        .any(|part| part == private_marker)
    {
        return Err("snapshot leaked private probe content".into());
    }
    // The source still answers after capture; then remove only its containers,
    // retaining its original data for a direct preservation check.
    runtime::wait_for_health(&source.launch_url, Duration::from_secs(180)).map_err(map)?;
    probe.exercise(Phase::AfterRestart, &source.launch_url)?;
    runtime::uninstall_with(&SystemProcessRunner, &source, false).map_err(map)?;
    storage::remove_installed_app(&source.id).map_err(|error| error.to_string())?;
    let target_id = format!("{}-restore", isolation.run_id);
    let target_dir = root.join(&target_id);
    fs::create_dir(&target_dir).map_err(|error| error.to_string())?;
    let mut target_template = template.clone();
    target_template.plan.id = target_id.clone();
    // Keep the isolated source's chosen addresses so Gitea's restored ROOT_URL
    // remains valid. The original containers are down; projects/volumes differ.
    let original_compose = fs::read_to_string(source_project.join("compose.yaml"))
        .map_err(|error| error.to_string())?;
    let original_compose = original_compose
        .strip_prefix(engine::COMPOSE_BINDING_MARKER)
        .ok_or("source binding marker missing")?;
    for (service, port) in local_store::plan::companion_host_ports(original_compose) {
        target_template.plan.set_companion_host(&service, port);
    }
    local_store::setup::fill_platform_values(&mut target_template.plan, port);
    let plan = target_template
        .resolve(&BTreeMap::new(), &BTreeMap::new())
        .map_err(|error| error.to_string())?;
    // Save the binding before its mandatory marker. A marker without its
    // binding is deliberately treated as interrupted/corrupt state.
    engine::save(&target_dir, &binding).map_err(map)?;
    fs::write(
        target_dir.join("compose.yaml"),
        format!(
            "{}{}",
            engine::COMPOSE_BINDING_MARKER,
            plan.to_compose().map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    let projected = engine::wsl::project_plan(
        &plan,
        target_dir.to_str().ok_or("project is not Unicode")?,
        &[],
    )
    .map_err(map)?;
    fs::write(
        target_dir.join(engine::wsl::COMPOSE_FILE),
        projected.compose,
    )
    .map_err(|error| error.to_string())?;
    let mut target = source.clone();
    target.id = target_id.clone();
    target.runtime = RuntimeSpec::Compose {
        project_name: format!("local-store-{target_id}"),
        project_dir: target_dir.clone(),
        compose_file: target_dir.join("compose.yaml"),
    };
    cleanup.apps.push(target.clone());
    let restored =
        backup::restore_into_fresh_with(&SystemProcessRunner, &target, &root, &encrypted, true)
            .map_err(map)?;
    if fs::read(target_dir.join(runtime::SECRETS_FILE)).map_err(|error| error.to_string())?
        != credential
    {
        return Err("restored generated credential bytes differ from the captured original".into());
    }
    runtime::start_with(&SystemProcessRunner, &target).map_err(map)?;
    runtime::wait_for_health(&target.launch_url, Duration::from_secs(180)).map_err(map)?;
    probe.exercise(Phase::AfterReinstall, &target.launch_url)?;
    if backup::restore_into_fresh_with(&SystemProcessRunner, &target, &root, &encrypted, true)
        .is_ok()
    {
        return Err("restore overwrote a running populated target".into());
    }
    runtime::uninstall_with(&SystemProcessRunner, &target, true).map_err(map)?;
    runtime::uninstall_with(&SystemProcessRunner, &source, true).map_err(map)?;
    bystanders.survived(&runner)?;
    isolation.registry_is_empty()?;
    use sha2::{Digest, Sha256};
    let encrypted_sha256 = format!("{:x}", Sha256::digest(&encrypted));
    Ok(
        serde_json::json!({"app": id, "passed": true, "private_content_verified_in_different_project": true, "source_resumed_and_content_preserved": true, "populated_restore_refused": true, "bystanders_preserved": true, "generated_credentials_preserved":true, "capture": receipt, "restore": restored, "fresh_project": true, "named_volumes": plan.named_volumes.len(), "encrypted_snapshot_sha256": encrypted_sha256}),
    )
}

#[test]
#[ignore = "real managed-engine backup proof; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn consistent_memos_and_gitea_snapshots_restore_into_fresh_projects() {
    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_MANAGED_QUALIFICATION").as_deref(),
        Ok("1")
    );
    use fs4::FileExt;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repo.join(".cache/managed-qualification");
    fs::create_dir_all(&directory).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("qualification.lock"))
        .unwrap();
    FileExt::try_lock(&lock).expect("another engine proof is active; do not run concurrently");
    let mut results = vec![];
    for id in ["memos", "gitea"] {
        match prove(id) {
            Ok(evidence) => results.push(evidence),
            Err(error) => {
                results.push(
                    serde_json::json!({"app":id,"passed":false,"detail":runtime::redact(&error)}),
                );
                break;
            }
        }
    }
    let passed = results.len() == 2 && results.iter().all(|row| row["passed"] == true);
    let evidence = serde_json::json!({"schema_version":1,"recorded_at_unix":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),"host_os":"windows","engine":EngineBinding::managed_wsl(),"passed":passed,"apps":results,"limits":{"current_windows_account_only":true,"max_snapshot_data_bytes":268435456,"shared_host_folders_excluded":true,"app_version_upgrades_unproven":true,"external_actions_not_reversible":true}});
    fs::write(
        repo.join("docs/evidence/windows-backup-restore-2026-10-01.json"),
        format!("{}\n", serde_json::to_string_pretty(&evidence).unwrap()),
    )
    .unwrap();
    assert!(passed, "backup proof failed: {evidence}");
}

//! The one mutation in recovery, and every refusal that guards it.
//!
//! `discard` removes what an interrupted install left behind. Each check below
//! is a way it could act on something it should not: a container somebody else
//! owns, an app that has since been installed properly, a directory outside
//! the managed root, or an app another operation is holding. A recovery action
//! that got any of these wrong would be worse than no recovery action.
use local_store::runtime::{CommandSpec, ProcessError, ProcessOutput, ProcessRunner};
use local_store::{error::ErrorCode, recovery};
use std::path::PathBuf;
use std::sync::Mutex;

/// Answers a scripted sequence of Docker calls and records what was asked.
struct Docker {
    replies: Mutex<Vec<String>>,
    calls: Mutex<Vec<Vec<String>>>,
    fail_on: Option<String>,
}

impl Docker {
    fn new(replies: Vec<String>) -> Self {
        Self {
            replies: Mutex::new(replies),
            calls: Mutex::new(Vec::new()),
            fail_on: None,
        }
    }
    fn failing(replies: Vec<String>, on: &str) -> Self {
        Self {
            replies: Mutex::new(replies),
            calls: Mutex::new(Vec::new()),
            fail_on: Some(on.to_owned()),
        }
    }
    fn asked(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

impl ProcessRunner for Docker {
    fn run_cancellable(
        &self,
        spec: &CommandSpec,
        _: &local_store::runtime::CancelToken,
    ) -> Result<ProcessOutput, ProcessError> {
        self.run(spec)
    }
    fn run(&self, spec: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
        assert_eq!(spec.program, "docker");
        self.calls.lock().unwrap().push(spec.args.clone());
        let failed = self
            .fail_on
            .as_ref()
            .is_some_and(|needle| spec.args.iter().any(|arg| arg == needle));
        let mut replies = self.replies.lock().unwrap();
        let stdout = if replies.is_empty() {
            String::new()
        } else {
            replies.remove(0)
        };
        Ok(ProcessOutput {
            success: !failed,
            stdout,
            stderr: String::new(),
            truncated: false,
        })
    }
}

/// The operation lock and the config-root environment are both process-wide,
/// so these tests take turns. Poisoning is ignored deliberately: one failing
/// test should report its own assertion, not turn every later one into a lock
/// error.
static SERIAL: Mutex<()> = Mutex::new(());

fn serially() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "discard-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// Lay down what an interrupted install leaves: a project directory with a
/// Compose file, and no registry entry naming it.
fn retained(name: &str) -> (PathBuf, PathBuf) {
    let root = root(name);
    let project = root.join("local-store/apps/memos");
    std::fs::create_dir_all(project.join("data")).unwrap();
    std::fs::write(project.join("compose.yaml"), b"services: {}\n").unwrap();
    std::fs::write(project.join("data/notes.db"), b"a person's notes").unwrap();
    std::env::set_var("APPDATA", &root);
    std::env::set_var("XDG_CONFIG_HOME", &root);
    (root, project)
}

/// The label reply that makes ownership verify against this project.
fn owning_labels(project: &PathBuf) -> String {
    serde_json::json!({
        "com.docker.compose.project": "local-store-memos",
        "com.docker.compose.service": "memos",
        "com.docker.compose.oneoff": "False",
        "com.docker.compose.project.config_files": project.join("compose.yaml"),
        "com.docker.compose.project.working_dir": project,
    })
    .to_string()
}

#[test]
fn a_discard_removes_the_containers_and_keeps_the_data_unless_asked() {
    let _serial = serially();
    let (_root, project) = retained("keeps-data");
    let id = "a".repeat(64);
    let docker = Docker::new(vec![
        id.clone(),              // verify: containers under the project label
        owning_labels(&project), // verify: their labels
        id.clone(),              // count what the project owns before removal
        String::new(),           // compose down
        String::new(),           // recount: nothing left
    ]);
    let done = recovery::discard_with(&docker, "memos", false).expect("discard should succeed");
    assert_eq!(done.containers_removed, 1);
    assert!(!done.data_deleted);

    // The data a person had is still there, and so are the setup files.
    assert!(project.join("data/notes.db").is_file());
    assert!(project.join("compose.yaml").is_file());

    // Only the retained Compose file, only this project, and no --volumes.
    let down = docker
        .asked()
        .into_iter()
        .find(|args| args.contains(&"down".to_owned()))
        .expect("compose down was never run");
    assert!(down.contains(&"-p".to_owned()) && down.contains(&"local-store-memos".to_owned()));
    // The Compose file is passed canonicalized, which is what stops a symlink
    // or a relative segment redirecting the removal somewhere else.
    let canonical = project
        .join("compose.yaml")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(down.contains(&canonical), "{down:?}");
    assert!(
        !down.contains(&"--volumes".to_owned()),
        "a keep-data discard asked Docker to delete volumes"
    );
}

#[test]
fn deleting_data_is_explicit_and_takes_the_directory_with_it() {
    let _serial = serially();
    let (_root, project) = retained("deletes-data");
    let docker = Docker::new(vec![
        String::new(), // verify: no containers at all
        String::new(), // compose down
        String::new(), // recount
    ]);
    let done = recovery::discard_with(&docker, "memos", true).expect("discard should succeed");
    assert_eq!(done.containers_removed, 0);
    assert!(done.data_deleted);
    assert!(!project.exists(), "the retained directory survived");

    let down = docker
        .asked()
        .into_iter()
        .find(|args| args.contains(&"down".to_owned()))
        .unwrap();
    assert!(down.contains(&"--volumes".to_owned()));
}

#[test]
fn final_cleanup_inspection_keeps_the_binding_after_its_file_is_deleted() {
    let _serial = serially();
    let (_root, project) = retained("bound-deletes-data");
    let binding = local_store::runtime::engine::EngineBinding {
        schema_version: 1,
        program: "docker".into(),
        endpoint: "unix:///owned-test.sock".into(),
    };
    local_store::runtime::engine::save(&project, &binding).unwrap();
    let docker = Docker::new(vec![String::new(), String::new(), String::new()]);
    recovery::discard_with(&docker, "memos", true).unwrap();
    assert!(!project.exists());
    let calls = docker.asked();
    assert_eq!(calls.len(), 3);
    assert!(calls
        .iter()
        .all(|args| args[..2] == ["--host", "unix:///owned-test.sock"]));
}

#[test]
fn corrupt_binding_refuses_recovery_before_contacting_docker() {
    let _serial = serially();
    let (_root, project) = retained("corrupt-engine");
    std::fs::write(project.join(local_store::runtime::engine::ENGINE_FILE), "{").unwrap();
    let docker = Docker::new(vec![]);
    assert!(recovery::discard_with(&docker, "memos", true).is_err());
    assert!(docker.asked().is_empty());
    assert!(project.exists());
}

#[test]
#[cfg(windows)]
fn wsl_recovery_keeps_its_engine_after_deleting_the_project() {
    use local_store::runtime::engine::{self, wsl};
    let _serial = serially();
    let (_root, project) = retained("wsl-delete-data");
    engine::save(&project, &engine::EngineBinding::managed_wsl()).unwrap();
    std::fs::write(project.join(wsl::COMPOSE_FILE), "services: {}\n").unwrap();
    let windows = local_store::folders::docker_path(&project.canonicalize().unwrap());
    let linux = wsl::windows_drive_path(windows.to_str().unwrap()).unwrap();
    let labels = serde_json::json!({
        "com.docker.compose.project": "local-store-memos",
        "com.docker.compose.service": "memos",
        "com.docker.compose.oneoff": "False",
        "com.docker.compose.project.config_files": format!("{linux}/compose.wsl.yaml"),
        "com.docker.compose.project.working_dir": linux,
    })
    .to_string();
    struct Wsl(Docker);
    impl ProcessRunner for Wsl {
        fn run_cancellable(
            &self,
            spec: &CommandSpec,
            _: &local_store::runtime::CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            assert_eq!(spec.program, "wsl.exe");
            assert!(spec.args.iter().any(|arg| arg == wsl::DISTRO));
            let at = spec
                .args
                .iter()
                .position(|arg| arg == "/usr/bin/docker")
                .unwrap();
            let mut docker = spec.clone();
            docker.program = "docker".into();
            docker.args = spec.args[at + 3..].to_vec();
            if docker.args.first().is_some_and(|arg| arg == "compose") {
                assert!(docker.args[2].ends_with("/compose.wsl.yaml"));
            }
            self.0.run(&docker)
        }
    }
    let runner = Wsl(Docker::new(vec![
        "a".repeat(64),
        labels,
        "a".repeat(64),
        String::new(),
        String::new(),
    ]));
    let done = recovery::discard_with(&runner, "memos", true).unwrap();
    assert_eq!(done.containers_removed, 1);
    assert!(done.data_deleted && !project.exists());
    assert_eq!(runner.0.asked().len(), 5);
}

#[test]
fn containers_that_do_not_match_the_retained_files_are_left_alone() {
    let _serial = serially();
    let (_root, project) = retained("mismatch");
    let elsewhere = serde_json::json!({
        "com.docker.compose.project": "local-store-memos",
        "com.docker.compose.service": "memos",
        "com.docker.compose.oneoff": "False",
        // Same project name, a different Compose file: somebody else's.
        "com.docker.compose.project.config_files": project.join("other.yaml"),
        "com.docker.compose.project.working_dir": project,
    })
    .to_string();
    let docker = Docker::new(vec!["b".repeat(64), elsewhere]);
    let error = recovery::discard_with(&docker, "memos", true).unwrap_err();
    assert_eq!(error.code, ErrorCode::UnsafePath);

    // Nothing was stopped and nothing was deleted.
    assert!(project.join("data/notes.db").is_file());
    assert!(
        !docker
            .asked()
            .iter()
            .any(|args| args.contains(&"down".to_owned())),
        "Docker was asked to remove containers it does not own"
    );
}

#[test]
fn an_app_that_is_properly_installed_is_not_a_recovery_candidate() {
    let _serial = serially();
    let (root, project) = retained("installed");
    // The same app, but now in the registry: this is an install, not wreckage.
    let registry = local_store::storage::RegistryV2::new(vec![local_store::model::InstalledApp {
        id: "memos".into(),
        catalog_id: None,
        display_name: "Memos".into(),
        launch_url: "http://localhost:5230".into(),
        icon_path: None,
        runtime: local_store::model::RuntimeSpec::Compose {
            project_name: "local-store-memos".into(),
            project_dir: project.clone(),
            compose_file: project.join("compose.yaml"),
        },
        created_at_unix: 1,
        updated_at_unix: 1,
    }]);
    local_store::storage::save_registry_v2_at(&root, &registry).unwrap();

    let docker = Docker::new(vec![]);
    let error = recovery::discard_with(&docker, "memos", true).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotFound);
    assert!(
        error.message.contains("uninstall it instead"),
        "{}",
        error.message
    );
    assert!(project.join("data/notes.db").is_file());
    assert!(docker.asked().is_empty(), "Docker was touched");
}

#[test]
fn a_failed_removal_leaves_the_files_where_they_were() {
    let _serial = serially();
    let (_root, project) = retained("down-fails");
    let docker = Docker::failing(vec![String::new(), String::new(), String::new()], "down");
    let error = recovery::discard_with(&docker, "memos", true).unwrap_err();
    assert_eq!(error.code, ErrorCode::ProcessFailed);
    // Containers first, files second: a failure at the container step must not
    // have taken the data with it.
    assert!(project.join("data/notes.db").is_file());
    assert!(project.join("compose.yaml").is_file());
}

#[test]
fn a_removal_docker_did_not_complete_is_reported_rather_than_claimed() {
    let _serial = serially();
    let (_root, project) = retained("still-there");
    let id = "c".repeat(64);
    let docker = Docker::new(vec![
        id.clone(),
        owning_labels(&project),
        id.clone(),
        String::new(), // compose down exits zero
        // ...but a container is still there afterwards.
        id,
    ]);
    let error = recovery::discard_with(&docker, "memos", false).unwrap_err();
    assert_eq!(error.code, ErrorCode::ProcessFailed);
    assert!(error.message.contains("still present"), "{}", error.message);
}

#[test]
fn a_busy_app_is_refused_before_anything_is_read() {
    let _serial = serially();
    let (_root, _project) = retained("busy");
    // Something else already holds this app's operation lock.
    let held = local_store::runtime::lock_operation("memos").expect("first lock");
    let docker = Docker::new(vec![]);
    let error = recovery::discard_with(&docker, "memos", true).unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert!(docker.asked().is_empty(), "Docker was touched while busy");
    drop(held);
}

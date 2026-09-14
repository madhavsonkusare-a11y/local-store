//! Explicitly bind a registered legacy project after read-only ownership checks.
use super::{retained, save, EngineBinding, COMPOSE_BINDING_MARKER};
use crate::{
    error::{AppError, AppResult},
    model::{InstalledApp, RuntimeSpec},
    runtime::{CommandSpec, ProcessRunner, SystemProcessRunner, DIAGNOSTIC_TIMEOUT},
    storage,
};
use std::{collections::BTreeSet, fs, io::Read, path::Path};

/// This adopts the current local endpoint; it never moves containers or data.
pub fn adopt_current_engine(app_id: &str) -> AppResult<EngineBinding> {
    let _lock = crate::runtime::lock_operation(app_id)?;
    let app = storage::load_or_migrate_registry()?
        .apps
        .into_iter()
        .find(|app| app.id == app_id)
        .ok_or_else(|| AppError::invalid("The app is no longer installed."))?;
    adopt_with(&app, &storage::managed_apps_root(), &SystemProcessRunner)
}

fn adopt_with(
    app: &InstalledApp,
    root: &Path,
    runner: &dyn ProcessRunner,
) -> AppResult<EngineBinding> {
    let RuntimeSpec::Compose {
        project_name,
        project_dir,
        compose_file,
    } = &app.runtime
    else {
        return Err(AppError::invalid(
            "Connected apps have no container engine to adopt.",
        ));
    };
    crate::runtime::confined_to_managed_root(project_dir, &app.id, root)?;
    let expected = project_dir.join("compose.yaml");
    if compose_file != &expected
        || expected.canonicalize()?.parent() != Some(project_dir.canonicalize()?.as_path())
    {
        return Err(AppError::invalid(
            "The Compose file is outside the managed project.",
        ));
    }
    let mut compose = Vec::new();
    fs::File::open(&expected)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut compose)?;
    if compose.len() > 1024 * 1024 {
        return Err(AppError::invalid("The retained Compose file is too large."));
    }
    // A saved binding wins even if the ambient default has changed. This also
    // permits retrying the marker write after a partial filesystem failure.
    let binding = match retained(project_dir)? {
        Some(binding) => binding,
        None => {
            let binding = runner
                .engine_binding()?
                .ok_or_else(|| AppError::invalid("No local engine was selected."))?;
            verify_ownership(runner, &binding, project_name, &expected)?;
            save(project_dir, &binding)?;
            binding
        }
    };
    if !compose.starts_with(COMPOSE_BINDING_MARKER.as_bytes()) {
        let mut marked = COMPOSE_BINDING_MARKER.as_bytes().to_vec();
        marked.extend(compose);
        storage::write_file_atomically(&expected, &marked).map_err(|_| AppError::invalid(
            "Engine binding saved, but its Compose marker could not be written. Retry bind-engine to finish."
        ))?;
    }
    Ok(binding)
}

fn verify_ownership(
    runner: &dyn ProcessRunner,
    binding: &EngineBinding,
    project: &str,
    compose: &Path,
) -> AppResult<()> {
    let query = |args| -> AppResult<String> {
        let out = runner.run(&binding.command(&CommandSpec::new(
            "docker",
            args,
            None,
            DIAGNOSTIC_TIMEOUT,
        ))?)?;
        if !out.success || out.truncated {
            return Err(AppError::invalid(
                "Engine adoption requires a complete Docker ownership inspection.",
            ));
        }
        Ok(out.stdout)
    };
    let inventory = query(vec![
        "container".into(),
        "ls".into(),
        "--all".into(),
        "--quiet".into(),
        "--no-trunc".into(),
        "--filter".into(),
        format!("label=com.docker.compose.project={project}"),
    ])?;
    let ids: Vec<_> = inventory.split_whitespace().collect();
    if ids.is_empty()
        || ids.len() > 32
        || ids
            .iter()
            .any(|id| id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()))
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(AppError::invalid("No unambiguous existing containers were found on this engine. Select the original local engine and retry; adoption does not migrate apps."));
    }
    let mut args = vec![
        "container".into(),
        "inspect".into(),
        "--format".into(),
        "{{json .Config.Labels}}".into(),
    ];
    args.extend(ids.iter().map(|id| (*id).to_owned()));
    let output = query(args)?;
    let lines: Vec<_> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let expected = compose.canonicalize()?;
    let valid = lines.len() == ids.len()
        && lines.iter().all(|line| {
            let Ok(labels) =
                serde_json::from_str::<std::collections::BTreeMap<String, String>>(line)
            else {
                return false;
            };
            let get = |key: &str| labels.get(key).map(String::as_str).unwrap_or_default();
            let config = Path::new(get("com.docker.compose.project.config_files"));
            let working = Path::new(get("com.docker.compose.project.working_dir"));
            get("com.docker.compose.project") == project
                && !get("com.docker.compose.service").is_empty()
                && get("com.docker.compose.oneoff").eq_ignore_ascii_case("false")
                && config.is_absolute()
                && working.is_absolute()
                && config.canonicalize().ok().as_ref() == Some(&expected)
                && working.canonicalize().ok().as_deref() == expected.parent()
        });
    if !valid {
        return Err(AppError::invalid("Container ownership does not match this app's retained Compose project; no binding was saved."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{CancelToken, ProcessError, ProcessOutput};
    use std::sync::Mutex;

    struct Fixture {
        root: std::path::PathBuf,
        app: InstalledApp,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "engine-adoption-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let project = root.join("memos");
            fs::create_dir_all(&project).unwrap();
            let compose = project.join("compose.yaml");
            fs::write(&compose, "services: {}\n").unwrap();
            let app = InstalledApp {
                id: "memos".into(),
                catalog_id: None,
                display_name: "Memos".into(),
                launch_url: "http://localhost:5230".into(),
                icon_path: None,
                runtime: RuntimeSpec::Compose {
                    project_name: "local-store-memos".into(),
                    project_dir: project,
                    compose_file: compose,
                },
                created_at_unix: 0,
                updated_at_unix: 0,
            };
            Self { root, app }
        }
        fn labels(&self, service: &str) -> String {
            serde_json::json!({
                "com.docker.compose.project": "local-store-memos",
                "com.docker.compose.service": service,
                "com.docker.compose.oneoff": "False",
                "com.docker.compose.project.config_files": self.root.join("memos/compose.yaml"),
                "com.docker.compose.project.working_dir": self.root.join("memos"),
            })
            .to_string()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    struct Docker {
        replies: Mutex<Vec<String>>,
        truncated: bool,
        calls: Mutex<usize>,
    }
    fn binding() -> EngineBinding {
        EngineBinding {
            schema_version: 1,
            program: "alternate-docker".into(),
            endpoint: "unix:///original.sock".into(),
        }
    }
    impl ProcessRunner for Docker {
        fn engine_binding(&self) -> AppResult<Option<EngineBinding>> {
            Ok(Some(binding()))
        }
        fn run_cancellable(
            &self,
            spec: &CommandSpec,
            _: &CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            assert_eq!(spec.program, "alternate-docker");
            assert_eq!(&spec.args[..2], ["--host", "unix:///original.sock"]);
            assert!(spec.remove_env.iter().any(|key| key == "DOCKER_CONTEXT"));
            assert_eq!(spec.args[2], "container");
            assert!(["ls", "inspect"].contains(&spec.args[3].as_str()));
            *self.calls.lock().unwrap() += 1;
            Ok(ProcessOutput {
                success: true,
                stdout: self.replies.lock().unwrap().remove(0),
                stderr: String::new(),
                truncated: self.truncated,
            })
        }
    }
    fn docker(replies: Vec<String>) -> Docker {
        Docker {
            replies: Mutex::new(replies),
            truncated: false,
            calls: Mutex::new(0),
        }
    }

    #[test]
    fn adopts_multiservice_project_without_executing_compose_and_retry_is_idempotent() {
        let f = Fixture::new();
        let runner = docker(vec![
            format!("{}\n{}", "a".repeat(64), "b".repeat(64)),
            format!("{}\n{}", f.labels("web"), f.labels("database")),
        ]);
        assert_eq!(adopt_with(&f.app, &f.root, &runner).unwrap(), binding());
        let project = f.root.join("memos");
        assert_eq!(retained(&project).unwrap(), Some(binding()));
        assert_eq!(
            fs::read_to_string(project.join("compose.yaml")).unwrap(),
            format!("{COMPOSE_BINDING_MARKER}services: {{}}\n")
        );
        adopt_with(&f.app, &f.root, &runner).unwrap();
        assert_eq!(*runner.calls.lock().unwrap(), 2);
        fs::remove_file(project.join(super::super::ENGINE_FILE)).unwrap();
        assert!(adopt_with(&f.app, &f.root, &runner).is_err());
    }

    #[test]
    fn uncertain_or_foreign_ownership_leaves_legacy_files_untouched() {
        let f = Fixture::new();
        for replies in [
            vec![String::new()],
            vec!["invalid-id".into()],
            vec![format!("{}\n{}", "a".repeat(64), "a".repeat(64))],
            vec!["a".repeat(64), "{}".into()],
            vec![
                "a".repeat(64),
                f.labels("web")
                    .replace("local-store-memos", "foreign-project"),
            ],
            vec!["a".repeat(64), f.labels("web").replace("False", "True")],
            vec![
                "a".repeat(64),
                format!("{}\n{}", f.labels("web"), f.labels("worker")),
            ],
        ] {
            assert!(adopt_with(&f.app, &f.root, &docker(replies)).is_err());
            assert!(!f
                .root
                .join("memos")
                .join(super::super::ENGINE_FILE)
                .exists());
            assert_eq!(
                fs::read_to_string(f.root.join("memos/compose.yaml")).unwrap(),
                "services: {}\n"
            );
        }
        let mut runner = docker(vec!["a".repeat(64)]);
        runner.truncated = true;
        assert!(adopt_with(&f.app, &f.root, &runner).is_err());
    }
}

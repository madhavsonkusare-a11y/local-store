//! Persisted local engine selection. Discovery is not migration.
use super::{
    CancelToken, CommandSpec, ProcessError, ProcessErrorCode, ProcessOutput, ProcessRunner,
    DIAGNOSTIC_TIMEOUT,
};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

pub const ENGINE_FILE: &str = "local-store-engine.json";
pub const COMPOSE_BINDING_MARKER: &str = "# local-store-engine-binding: 1\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineBinding {
    pub schema_version: u32,
    pub program: String,
    pub endpoint: String,
}

impl EngineBinding {
    pub fn validate(&self) -> AppResult<()> {
        let local = self
            .endpoint
            .strip_prefix("unix:///")
            .is_some_and(|p| !p.is_empty())
            || self
                .endpoint
                .strip_prefix("npipe:////./pipe/")
                .is_some_and(|p| !p.is_empty() && !p.contains('/'));
        if self.schema_version != 1
            || self.program.is_empty()
            || self.program.len() > 4096
            || self.program.chars().any(char::is_control)
            || !local
            || self.endpoint.len() > 4096
            || self.endpoint.chars().any(char::is_control)
        {
            return Err(AppError::invalid("Invalid or unsupported stored engine binding. Restore it before operating on this app."));
        }
        Ok(())
    }

    /// Capture the effective local endpoint once. Never store mutable context
    /// names as the identity used by subsequent lifecycle commands.
    pub fn discover(runner: &dyn ProcessRunner) -> AppResult<Self> {
        let explicit_context = std::env::var("DOCKER_CONTEXT")
            .ok()
            .filter(|s| !s.is_empty());
        let endpoint = if explicit_context.is_none() {
            std::env::var("DOCKER_HOST").ok().filter(|s| !s.is_empty())
        } else {
            None
        };
        let endpoint = match endpoint {
            Some(endpoint) => endpoint,
            None => {
                let mut args = vec!["context".into(), "inspect".into()];
                if let Some(context) = explicit_context {
                    args.push(context);
                }
                args.extend(["--format".into(), "{{json .Endpoints.docker.Host}}".into()]);
                let out =
                    runner.run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT))?;
                if !out.success || out.truncated {
                    return Err(AppError::invalid(
                        "Could not determine the selected Docker endpoint.",
                    ));
                }
                serde_json::from_str::<String>(out.stdout.trim())
                    .map_err(|_| AppError::invalid("Docker returned an invalid endpoint."))?
            }
        };
        let binding = Self {
            schema_version: 1,
            program: "docker".into(),
            endpoint,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn command(&self, spec: &CommandSpec) -> AppResult<CommandSpec> {
        self.validate()?;
        let mut command = spec.clone();
        command.program = self.program.clone();
        if command.args.first().is_some_and(|a| a == "--host") {
            if command.args.get(1) != Some(&self.endpoint) {
                return Err(AppError::invalid(
                    "The operation targets a different engine than its saved binding.",
                ));
            }
        } else {
            command
                .args
                .splice(0..0, ["--host".into(), self.endpoint.clone()]);
        }
        for name in [
            "DOCKER_HOST",
            "DOCKER_CONTEXT",
            "DOCKER_TLS_VERIFY",
            "DOCKER_CERT_PATH",
        ] {
            if !command.remove_env.iter().any(|key| key == name) {
                command.remove_env.push(name.into());
            }
        }
        Ok(command)
    }
}

pub fn retained(project: &Path) -> AppResult<Option<EngineBinding>> {
    let path = project.join(ENGINE_FILE);
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut header = Vec::new();
            if let Ok(file) = fs::File::open(project.join("compose.yaml")) {
                file.take(128).read_to_end(&mut header).ok();
            }
            if header.starts_with(b"# local-store-engine-binding:") {
                return Err(AppError::invalid(
                    "This app's engine binding is missing. Restore it before operating on the app.",
                ));
            }
            return Ok(None);
        }
        Err(_) => {
            return Err(AppError::invalid(
                "Stored engine binding cannot be read; no fallback engine was selected.",
            ))
        }
    };
    let mut data = Vec::new();
    file.take(16385).read_to_end(&mut data)?;
    if data.len() > 16384 {
        return Err(AppError::invalid("Stored engine binding is too large."));
    }
    let binding: EngineBinding = serde_json::from_slice(&data).map_err(|_| {
        AppError::invalid("Stored engine binding is corrupt; no fallback engine was selected.")
    })?;
    binding.validate()?;
    Ok(Some(binding))
}

pub fn save(project: &Path, binding: &EngineBinding) -> AppResult<()> {
    binding.validate()?;
    if let Some(previous) = retained(project)? {
        if previous != *binding {
            return Err(AppError::invalid(
                "Refusing to replace this app's engine binding.",
            ));
        }
        return Ok(());
    }
    crate::storage::write_file_atomically(
        &project.join(ENGINE_FILE),
        &serde_json::to_vec_pretty(binding).map_err(AppError::internal)?,
    )?;
    Ok(())
}

pub(crate) fn project_command(project: &Path, spec: CommandSpec) -> AppResult<CommandSpec> {
    match retained(project)? {
        Some(binding) => binding.command(&spec),
        None => Ok(spec), // Historical installs require a separate explicit migration.
    }
}

/// Route every Docker command, including qualification's inventory and cleanup,
/// through one captured binding. Other tools (e.g. Node probes) stay unchanged.
pub struct EngineRunner<'a> {
    pub inner: &'a dyn ProcessRunner,
    pub binding: Option<EngineBinding>,
}
impl ProcessRunner for EngineRunner<'_> {
    fn engine_binding(&self) -> AppResult<Option<EngineBinding>> {
        Ok(self.binding.clone())
    }
    fn run_cancellable(
        &self,
        spec: &CommandSpec,
        cancel: &CancelToken,
    ) -> Result<ProcessOutput, ProcessError> {
        let routed = match (&self.binding, spec.program.as_str()) {
            (Some(binding), "docker") => binding.command(spec).map_err(|error| {
                ProcessError::new(ProcessErrorCode::ProcessFailed, error.message)
            })?,
            _ => spec.clone(),
        };
        self.inner.run_cancellable(&routed, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn binding() -> EngineBinding {
        EngineBinding {
            schema_version: 1,
            program: "alternate-docker".into(),
            endpoint: "unix:///owned.sock".into(),
        }
    }

    #[test]
    fn selection_preserves_arguments_and_deadlines_but_removes_context_overrides() {
        let selected = binding();
        let spec = CommandSpec::new(
            "docker",
            vec![
                "compose".into(),
                "-f".into(),
                "a folder/compose.yaml".into(),
                "up".into(),
            ],
            Some("a folder".into()),
            Duration::from_secs(19),
        );
        let command = selected.command(&spec).unwrap();
        assert_eq!(command.program, "alternate-docker");
        assert_eq!(&command.args[2..], spec.args);
        assert_eq!(command.cwd, spec.cwd);
        assert_eq!(command.timeout, spec.timeout);
        assert!(command.remove_env.iter().any(|key| key == "DOCKER_CONTEXT"));
        assert_eq!(selected.command(&command).unwrap(), command);
        let mut other = selected.clone();
        other.endpoint = "unix:///other.sock".into();
        assert!(other.command(&command).is_err());
    }

    #[test]
    fn unsupported_bindings_are_not_used_as_remote_fallbacks() {
        for endpoint in [
            "tcp://remote:2375",
            "ssh://user@host",
            "unix:///",
            "npipe:////remote/pipe/docker",
            "unix:///x\n",
        ] {
            let mut selected = binding();
            selected.endpoint = endpoint.into();
            assert!(selected.validate().is_err());
        }
        let mut selected = binding();
        selected.schema_version = 2;
        assert!(selected.validate().is_err());
    }

    #[test]
    fn marked_projects_refuse_a_missing_or_corrupt_binding() {
        let root = std::env::temp_dir().join(format!(
            "engine-binding-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        assert_eq!(retained(&root).unwrap(), None);
        save(&root, &binding()).unwrap();
        assert_eq!(retained(&root).unwrap(), Some(binding()));
        let mut other = binding();
        other.endpoint = "unix:///other.sock".into();
        assert!(save(&root, &other).is_err());
        fs::write(root.join("compose.yaml"), COMPOSE_BINDING_MARKER).unwrap();
        fs::remove_file(root.join(ENGINE_FILE)).unwrap();
        assert!(retained(&root).is_err());
        fs::write(root.join(ENGINE_FILE), "{").unwrap();
        assert!(retained(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "real Docker; supply LOCAL_STORE_ENGINE_TEST_ENDPOINT and an invalid DOCKER_CONTEXT"]
    fn explicit_endpoint_survives_an_invalid_ambient_context() {
        assert_eq!(
            std::env::var("DOCKER_CONTEXT").as_deref(),
            Ok("local-store-invalid-proof")
        );
        let selected = EngineBinding {
            schema_version: 1,
            program: "docker".into(),
            endpoint: std::env::var("LOCAL_STORE_ENGINE_TEST_ENDPOINT")
                .expect("explicit test endpoint"),
        };
        let runner = EngineRunner {
            inner: &crate::runtime::SystemProcessRunner,
            binding: Some(selected),
        };
        let report = crate::runtime::doctor_with(&runner);
        assert!(report.ready, "{report:?}");
    }
}

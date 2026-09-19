//! Command transport for the future owned WSL distro. Not a selectable backend
//! until Compose projection, bootstrap ownership and lifecycle proof exist.
use crate::{
    error::{AppError, AppResult},
    runtime::{
        CancelToken, CommandSpec, ProcessError, ProcessErrorCode, ProcessOutput, ProcessRunner,
    },
};

pub const DISTRO: &str = "local-store-engine-v1";
pub mod bootstrap;
mod projection;
pub use projection::{project_plan, PathPair, ProjectedPlan, ProjectedSeed, COMPOSE_FILE};

/// Lexical mapping for the owned distro's required /mnt drive automount layout.
/// This does not establish existence, ownership, mount availability or access.
/// UNC, device namespaces and ambiguous relative/traversal paths are refused.
pub fn windows_drive_path(path: &str) -> AppResult<String> {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let bytes = path.as_bytes();
    if bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'/' | b'\\')
        || path.chars().any(char::is_control)
        || path.len() > 32768
    {
        return Err(AppError::invalid(
            "WSL requires an absolute local Windows drive path.",
        ));
    }
    let tail = path[3..].replace('\\', "/");
    let parts: Vec<_> = tail.trim_end_matches('/').split('/').collect();
    if !tail.is_empty()
        && parts.iter().any(|part| {
            part.is_empty()
                || *part == "."
                || *part == ".."
                || part.ends_with([' ', '.'])
                || part.contains([':', '"', '<', '>', '|', '?', '*'])
        })
    {
        return Err(AppError::invalid(
            "The Windows path cannot be translated unambiguously for WSL.",
        ));
    }
    Ok(format!(
        "/mnt/{}/{}",
        (bytes[0] as char).to_ascii_lowercase(),
        parts.join("/")
    ))
}

/// Build a read-only engine diagnostic without a shell or implicit distro/user.
/// Only the doctor commands are admitted until Compose projection is available.
pub fn diagnostic_command(spec: &CommandSpec) -> AppResult<CommandSpec> {
    let args: Vec<_> = spec.args.iter().map(String::as_str).collect();
    let supported = matches!(
        args.as_slice(),
        ["version", "--format", "{{.Server.Version}}"] | ["compose", "version", "--short"]
    );
    if spec.program != "docker" || spec.cwd.is_some() || !supported {
        return Err(AppError::invalid("WSL transport currently supports engine diagnostics only; app operations require Compose and bind-path projection."));
    }
    Ok(wrap(spec, "/", spec.args.clone()))
}

fn wrap(spec: &CommandSpec, cwd: &str, args: Vec<String>) -> CommandSpec {
    let mut command = CommandSpec::new(
        "wsl.exe",
        vec![
            "--distribution".into(),
            DISTRO.into(),
            "--user".into(),
            "root".into(),
            "--cd".into(),
            cwd.into(),
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
        spec.timeout,
    );
    command.args.extend(args);
    command.remove_env = spec.remove_env.clone();
    for key in [
        "WSLENV",
        "DOCKER_HOST",
        "DOCKER_CONTEXT",
        "DOCKER_TLS_VERIFY",
        "DOCKER_CERT_PATH",
    ] {
        if !command.remove_env.iter().any(|value| value == key) {
            command.remove_env.push(key.into());
        }
    }
    command
}

/// Route existing runtime commands through a saved WSL binding. Only the
/// generated companion Compose file can be used; arbitrary -f paths refuse.
pub(crate) fn runtime_command(spec: &CommandSpec) -> AppResult<CommandSpec> {
    if diagnostic_command(spec).is_ok() {
        return diagnostic_command(spec);
    }
    if spec.program != "docker" {
        return Err(AppError::invalid("WSL binding requires a Docker command."));
    }
    if spec.args.first().is_some_and(|a| a == "compose") {
        let project = spec.cwd.as_ref().ok_or_else(|| {
            AppError::invalid("WSL Compose requires its managed project directory.")
        })?;
        let args = &spec.args;
        if args.len() < 6
            || args[1] != "-f"
            || std::path::Path::new(&args[2]).canonicalize()?
                != project.join("compose.yaml").canonicalize()?
            || args[3] != "-p"
            || !args[4].starts_with("local-store-")
        {
            return Err(AppError::invalid("Unsupported WSL Compose invocation."));
        }
        let projected = project.join(COMPOSE_FILE);
        if !projected.is_file()
            || projected.canonicalize()?.parent() != Some(project.canonicalize()?.as_path())
        {
            return Err(AppError::invalid("The WSL Compose artifact is missing or outside its project; restore it before operating on this app."));
        }
        let canonical_project = crate::folders::docker_path(&project.canonicalize()?);
        let linux = windows_drive_path(
            canonical_project
                .to_str()
                .ok_or_else(|| AppError::invalid("Project path must be Unicode."))?,
        )?;
        let mut translated = args.clone();
        translated[2] = format!("{linux}/{COMPOSE_FILE}");
        return Ok(wrap(spec, &linux, translated));
    }
    if spec.cwd.is_some()
        || !spec.args.first().is_some_and(|a| {
            [
                "pull",
                "inspect",
                "image",
                "container",
                "network",
                "volume",
                "stats",
            ]
            .contains(&a.as_str())
        })
    {
        return Err(AppError::invalid("Unsupported WSL engine command."));
    }
    Ok(wrap(spec, "/", spec.args.clone()))
}

/// Reuses the bounded process runner; construction does not launch WSL. Calling
/// doctor_with on this runner may start the named distro, so bootstrap must first
/// establish ownership. It is intentionally not wired to engine discovery.
pub struct DiagnosticRunner<'a> {
    pub inner: &'a dyn ProcessRunner,
}
impl ProcessRunner for DiagnosticRunner<'_> {
    fn run_cancellable(
        &self,
        spec: &CommandSpec,
        cancel: &CancelToken,
    ) -> Result<ProcessOutput, ProcessError> {
        let command = diagnostic_command(spec)
            .map_err(|error| ProcessError::new(ProcessErrorCode::ProcessFailed, error.message))?;
        self.inner.run_cancellable(&command, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::DIAGNOSTIC_TIMEOUT;

    #[test]
    fn paths_preserve_spaces_unicode_and_literal_shell_characters() {
        for (input, expected) in [
            (
                r"D:\My Vault\应用\a$(touch nope);b",
                "/mnt/d/My Vault/应用/a$(touch nope);b",
            ),
            (r"\\?\C:\Data\app", "/mnt/c/Data/app"),
            ("C:/Data/app/", "/mnt/c/Data/app"),
            ("C:\\", "/mnt/c/"),
        ] {
            assert_eq!(windows_drive_path(input).unwrap(), expected);
        }
        for path in [
            r"\\server\share",
            r"\\?\UNC\server\share",
            r"\\.\pipe\docker",
            "C:relative",
            "/tmp/app",
            "C:/a/../b",
            "C:/a/./b",
            "C:/a//b",
            "C:/a:stream",
            "C:/a.",
            "C:/a\n",
        ] {
            assert!(windows_drive_path(path).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn doctor_routes_both_commands_and_forwards_cancellation_without_a_shell() {
        struct Fake;
        impl ProcessRunner for Fake {
            fn run_cancellable(
                &self,
                spec: &CommandSpec,
                token: &CancelToken,
            ) -> Result<ProcessOutput, ProcessError> {
                assert_eq!(spec.program, "wsl.exe");
                assert_eq!(spec.timeout, DIAGNOSTIC_TIMEOUT);
                assert_eq!(
                    &spec.args[..8],
                    [
                        "--distribution",
                        DISTRO,
                        "--user",
                        "root",
                        "--cd",
                        "/",
                        "--exec",
                        "/usr/bin/env"
                    ]
                );
                assert!(spec.remove_env.iter().any(|key| key == "WSLENV"));
                assert!(token.is_cancelled());
                Ok(ProcessOutput {
                    success: true,
                    stdout: "29.8.0".into(),
                    stderr: String::new(),
                    truncated: false,
                })
            }
        }
        let runner = DiagnosticRunner { inner: &Fake };
        let cancel = CancelToken::new();
        cancel.cancel();
        for args in [
            vec!["version", "--format", "{{.Server.Version}}"],
            vec!["compose", "version", "--short"],
        ] {
            runner
                .run_cancellable(
                    &CommandSpec::new(
                        "docker",
                        args.into_iter().map(String::from).collect(),
                        None,
                        DIAGNOSTIC_TIMEOUT,
                    ),
                    &cancel,
                )
                .unwrap();
        }
    }

    #[test]
    fn app_commands_and_untranslated_working_directories_refuse_before_execution() {
        for args in [
            vec!["compose", "up", "-d"],
            vec!["container", "rm", "foreign"],
            vec!["--host", "tcp://remote", "version"],
        ] {
            assert!(diagnostic_command(&CommandSpec::new(
                "docker",
                args.into_iter().map(String::from).collect(),
                None,
                DIAGNOSTIC_TIMEOUT
            ))
            .is_err());
        }
        assert!(diagnostic_command(&CommandSpec::new(
            "docker",
            vec!["compose".into(), "version".into(), "--short".into()],
            Some("C:/apps".into()),
            DIAGNOSTIC_TIMEOUT
        ))
        .is_err());
    }

    #[test]
    fn project_scoped_stats_use_the_owned_engine_without_a_shell() {
        let command = runtime_command(&CommandSpec::new(
            "docker",
            vec!["stats".into(), "--no-stream".into(), "abc123".into()],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .unwrap();
        assert_eq!(command.program, "wsl.exe");
        assert!(command.args.iter().any(|arg| arg == "stats"));
        assert!(command.args.iter().any(|arg| arg == "abc123"));
        assert!(!command.args.iter().any(|arg| arg == "sh"));
    }
}

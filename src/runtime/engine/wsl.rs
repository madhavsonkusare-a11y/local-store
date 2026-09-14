//! Command transport for the future owned WSL distro. Not a selectable backend
//! until Compose projection, bootstrap ownership and lifecycle proof exist.
use crate::{
    error::{AppError, AppResult},
    runtime::{
        CancelToken, CommandSpec, ProcessError, ProcessErrorCode, ProcessOutput, ProcessRunner,
    },
};

pub const DISTRO: &str = "local-store-engine-v1";

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
    let mut command = CommandSpec::new(
        "wsl.exe",
        vec![
            "--distribution".into(),
            DISTRO.into(),
            "--user".into(),
            "root".into(),
            "--cd".into(),
            "/".into(),
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
    command.args.extend(spec.args.clone());
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
    Ok(command)
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
}

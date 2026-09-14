//! Standalone command-line interface for Local Store.
use local_store::{
    brand::{CLI_NAME, PRODUCT_NAME},
    error::{AppError, AppResult, ErrorCode},
    model::{next_available_installed_app_id, InstalledApp, RuntimeSpec},
    offerings, platform, runtime, storage, windowing,
};
use std::time::{SystemTime, UNIX_EPOCH};
#[path = "cli_queries.rs"]
mod queries;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Give a release build somewhere to write.
///
/// The shipped Windows binary is a GUI-subsystem image, so it starts without a
/// console. Attaching to the parent's console makes interactive output visible
/// — but it also replaces the process's standard handles. When a shell has
/// already supplied them by redirecting or piping, attaching would send the
/// output to the console window instead of where it was asked to go, which
/// silently loses it. So attach only when there is nothing to write to.
#[cfg(windows)]
pub(crate) fn ensure_console() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetConsoleWindow, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };
    unsafe {
        let output = GetStdHandle(STD_OUTPUT_HANDLE);
        let inherited = !output.is_null() && output != INVALID_HANDLE_VALUE;
        if !inherited && GetConsoleWindow().is_null() {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
#[cfg(not(windows))]
pub(crate) fn ensure_console() {}

fn usage() -> String {
    format!("Usage:\n  {CLI_NAME} add <name> --url <url>\n  {CLI_NAME} list\n  {CLI_NAME} open <id-or-name> [--browser]\n  {CLI_NAME} shortcut <id-or-name>\n  {CLI_NAME} remove <id-or-name>\n  {CLI_NAME} doctor [--json]\n  {CLI_NAME} recovery [--json] [--docker]\n  {CLI_NAME} recover <recipe-id> [--delete-data]\n  {CLI_NAME} adopt <recipe-id>\n  {CLI_NAME} bind-engine <id-or-name>\n  {CLI_NAME} install <recipe-id>\n  {CLI_NAME} recipes\n  {CLI_NAME} start|stop|status|logs <id-or-name>\n  {CLI_NAME} uninstall <id-or-name> [--delete-data]\n  {CLI_NAME} catalog [search words] [options] (see catalog --help)\n  {CLI_NAME} version")
}
fn get_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1).cloned())
}
fn positional(args: &[String], index: usize, line: &str) -> Result<String, i32> {
    args.get(index)
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| {
            eprintln!("{line}");
            1
        })
}
fn registry() -> AppResult<Vec<InstalledApp>> {
    storage::load_or_migrate_registry()
        .map(|registry| registry.apps)
        .map_err(AppError::from)
}
fn find_app(value: &str) -> AppResult<InstalledApp> {
    registry()?
        .into_iter()
        .find(|app| app.id == value || app.display_name.eq_ignore_ascii_case(value))
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::NotFound,
                format!("No app with ID or name \"{value}\" found."),
            )
        })
}
fn now() -> AppResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(AppError::internal)
}
fn report(result: AppResult<()>, success: &str) -> i32 {
    match result {
        Ok(()) => {
            println!("{success}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
/// Write a command's output without panicking when the reader has gone away.
///
/// `print!` panics on a write error. A closed pipe is not a fault of the
/// command: it happens whenever a reader stops early, and on Windows whenever a
/// shell declines to wait for this GUI-subsystem image. Report it as success,
/// the way a well-behaved CLI treats `| head`.
fn emit(text: &str) -> Result<(), i32> {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Err(0),
        Err(error) => {
            eprintln!("Could not write output: {error}");
            Err(1)
        }
    }
}
fn query_result(result: Result<queries::Output, String>) -> i32 {
    match result {
        Ok(output) => match emit(&output.text) {
            Ok(()) => output.code,
            Err(code) => code,
        },
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

pub fn run_cli() -> i32 {
    ensure_console();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match normalize_action_args(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let Some(command) = args.first().map(String::as_str) else {
        eprintln!("{}", usage());
        return 1;
    };
    match command {
        "add" => {
            let name = match positional(
                &args,
                1,
                &format!("Usage: {CLI_NAME} add <name> --url <url>"),
            ) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let Some(url) = get_flag(&args, "--url") else {
                eprintln!("Usage: {CLI_NAME} add <name> --url <url>");
                return 1;
            };
            let url = match windowing::validated_external_url(&url) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            let apps = match registry() {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            let base = storage::slug_for_display_name(&name);
            let Some(id) = next_available_installed_app_id(&base, |candidate| {
                !apps.iter().any(|app| app.id == candidate)
            }) else {
                eprintln!("Could not create a safe app ID.");
                return 1;
            };
            let timestamp = match now() {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            let app = InstalledApp {
                id: id.clone(),
                catalog_id: None,
                display_name: name,
                launch_url: url,
                icon_path: None,
                runtime: RuntimeSpec::External,
                created_at_unix: timestamp,
                updated_at_unix: timestamp,
            };
            report(
                storage::insert_installed_app(app).map_err(AppError::from),
                &format!("Connected app as \"{id}\"."),
            )
        }
        "list" => match registry() {
            Ok(apps) => {
                println!("{}", serde_json::to_string_pretty(&apps).unwrap());
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        },
        "doctor" => query_result(queries::doctor(&args[1..], &runtime::SystemProcessRunner)),
        "recovery" => query_result(queries::recovery(&args[1..])),
        // The one recovery action. `recovery` stays read-only: an inventory
        // command that could also delete things is too easy to run by accident.
        "recover" => {
            let id = match positional(
                &args,
                1,
                &format!("Usage: {CLI_NAME} recover <recipe-id> [--delete-data]"),
            ) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let delete_data = args.iter().any(|arg| arg == "--delete-data");
            match local_store::recovery::discard(&id, delete_data) {
                Ok(done) => {
                    println!(
                        "Removed {} container(s) left by an interrupted setup of {}. {}",
                        done.containers_removed,
                        done.recipe_id,
                        if done.data_deleted {
                            "Its setup files and data were deleted."
                        } else {
                            "Its setup files and data were kept."
                        }
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            }
        }
        "bind-engine" => {
            let app = match find_app(&args[1]) {
                Ok(app) => app,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            match runtime::engine::adopt_current_engine(&app.id) {
                Ok(binding) => {
                    println!("App bound to {}.", binding.endpoint);
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            }
        }
        "adopt" => {
            let id = match positional(&args, 1, &format!("Usage: {CLI_NAME} adopt <recipe-id>")) {
                Ok(value) => value,
                Err(code) => return code,
            };
            match local_store::recovery::adopt(&id) {
                Ok(done) => {
                    println!(
                        "Finished the interrupted setup of {}. It is answering at {} and now appears in My Apps ({} container(s)).",
                        done.recipe_id, done.launch_url, done.containers
                    );
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            }
        }
        "install" => {
            let id = match positional(&args, 1, &format!("Usage: {CLI_NAME} install <recipe-id>")) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let Some(offering) = offerings::offering(&id) else {
                eprintln!("No reviewed install recipe named \"{id}\".");
                return 1;
            };
            match offering.recipe(None) {
                // `install_recipe` commits to the registry inside the same
                // operation lock, so there is no separate insert here.
                Ok(Some(recipe)) => report(
                    runtime::install_recipe(&recipe).map(|_| ()),
                    "Installed and started.",
                ),
                // An imported app has no Compose file of its own and is
                // rendered from its reviewed plan.
                Ok(None) => {
                    let answers = std::collections::BTreeMap::new();
                    match offering.plan_template(None) {
                        Ok(template) => {
                            // This command has no way to ask a question, so an
                            // app that needs an answer is sent to the window
                            // that can, rather than installed with a guess.
                            if let Err(errors) = template.accept_answers(&answers) {
                                for error in errors {
                                    eprintln!("{}: {}", error.key, error.message);
                                }
                                eprintln!(
                                    "This app needs setup answers. Install it from the {} window.",
                                    PRODUCT_NAME
                                );
                                return 1;
                            }
                            report(
                                runtime::install_template(
                                    &template,
                                    offering.display_name(),
                                    &answers,
                                )
                                .map(|_| ()),
                                "Installed and started.",
                            )
                        }
                        Err(error) => {
                            eprintln!("{}", error.message);
                            1
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{}", error.message);
                    1
                }
            }
        }
        "recipes" => {
            // Everything installable, from either reviewed source, so the CLI
            // and the launcher cannot disagree about what is on offer.
            for offering in offerings::offerings() {
                let summary = match offering.summary(None) {
                    Ok(summary) => summary,
                    Err(error) => {
                        eprintln!("{}: {}", offering.id(), error.message);
                        continue;
                    }
                };
                println!(
                    "{:<14} {:<12} {}",
                    summary.id, summary.version, summary.image
                );
            }
            0
        }
        "open" => {
            let value = match positional(
                &args,
                1,
                &format!("Usage: {CLI_NAME} open <id-or-name> [--browser]"),
            ) {
                Ok(value) => value,
                Err(code) => return code,
            };
            if !args.iter().any(|arg| arg == "--browser") {
                eprintln!("Use open <id-or-name> for a desktop window, or add --browser for the default browser.");
                return 1;
            }
            match find_app(&value) {
                Ok(app) => {
                    if app.is_managed() {
                        if let Err(error) = runtime::start(&app) {
                            eprintln!("{error}");
                            return 1;
                        }
                    }
                    // A browser that will not open is a failure, not a
                    // successful open: report it and say so in the exit code.
                    if let Err(error) = windowing::launch_browser(&app.launch_url) {
                        eprintln!("{error}");
                        return 1;
                    }
                    println!("Opened {}.", app.display_name);
                    0
                }
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            }
        }
        "shortcut" => {
            let value = match positional(
                &args,
                1,
                &format!("Usage: {CLI_NAME} shortcut <id-or-name>"),
            ) {
                Ok(value) => value,
                Err(code) => return code,
            };
            match find_app(&value) {
                Ok(app) => match std::env::current_exe()
                    .map_err(AppError::from)
                    .and_then(|bin| {
                        platform::create_shortcut_for(
                            &app.id,
                            &app.display_name,
                            &bin.to_string_lossy(),
                            app.icon_path.as_ref().and_then(|path| path.to_str()),
                        )
                        .map_err(|e| AppError::new(ErrorCode::ShortcutFailed, e))
                    }) {
                    Ok(path) => {
                        println!("Shortcut created: {path}");
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                },
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            }
        }
        "start" | "stop" | "status" | "logs" | "remove" | "uninstall" => {
            let value = match positional(
                &args,
                1,
                &format!("Usage: {CLI_NAME} {command} <id-or-name>"),
            ) {
                Ok(value) => value,
                Err(code) => return code,
            };
            let app = match find_app(&value) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            };
            match command {
                "start" => report(runtime::start(&app), "Started."),
                "stop" => report(runtime::stop(&app), "Stopped."),
                "status" => match runtime::status(&app) {
                    Ok(status) => {
                        println!("{status:?}");
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                },
                "logs" => match runtime::logs(&app) {
                    Ok(logs) => {
                        print!("{logs}");
                        0
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                },
                "remove" if app.is_managed() => {
                    eprintln!("Managed apps must be removed with uninstall.");
                    1
                }
                "remove" => report(
                    storage::remove_installed_app(&app.id)
                        .map(|_| ())
                        .map_err(AppError::from),
                    "Connection removed.",
                ),
                "uninstall" if !app.is_managed() => {
                    eprintln!("Connected apps must be removed with remove.");
                    1
                }
                "uninstall" => {
                    let delete_data = args.iter().any(|arg| arg == "--delete-data");
                    report(
                        runtime::uninstall_and_remove(&app, delete_data),
                        if delete_data {
                            "App and data removed."
                        } else {
                            "App removed; data preserved."
                        },
                    )
                }
                _ => unreachable!(),
            }
        }
        "catalog" => query_result(queries::catalog(&args[1..])),
        "--help" | "-h" => {
            println!("{}", usage());
            0
        }
        "version" | "--version" | "-V" => {
            println!("{CLI_NAME} {VERSION}");
            0
        }
        _ => {
            eprintln!("Unknown command \"{command}\".\n{}", usage());
            1
        }
    }
}

// Parse all action arguments before any registry access or Docker operation.
// Canonical ordering lets existing handlers retain their data-safety checks.
fn normalize_action_args(args: Vec<String>) -> Result<Vec<String>, String> {
    let Some(command) = args.first() else {
        return Ok(args);
    };
    if ![
        "bind-engine",
        "add",
        "install",
        "open",
        "shortcut",
        "start",
        "stop",
        "status",
        "logs",
        "remove",
        "uninstall",
        "list",
        "recipes",
        "version",
        "--version",
        "-V",
    ]
    .contains(&command.as_str())
    {
        return Ok(args);
    }
    let mut parser = pico_args::Arguments::from_vec(args[1..].iter().map(Into::into).collect());
    if parser.contains(["-h", "--help"]) {
        return Ok(vec!["--help".into()]);
    }
    let url: Option<String> = if command == "add" {
        Some(
            parser
                .value_from_str("--url")
                .map_err(|e| format!("add --url: {e}"))?,
        )
    } else {
        None
    };
    let flag = match command.as_str() {
        "open" if parser.contains("--browser") => Some("--browser"),
        "uninstall" if parser.contains("--delete-data") => Some("--delete-data"),
        _ => None,
    };
    let remaining = parser.finish();
    let required = !["list", "recipes", "version", "--version", "-V"].contains(&command.as_str());
    if remaining.len() != usize::from(required) {
        return Err(format!(
            "Invalid arguments for {command}; use {CLI_NAME} --help."
        ));
    }
    let mut normalized = vec![command.clone()];
    for arg in remaining {
        let arg = arg.into_string().map_err(|_| "Arguments must be UTF-8")?;
        if arg.trim().is_empty() || arg.starts_with('-') {
            return Err(format!("Invalid name or option for {command}: {arg}"));
        }
        normalized.push(arg);
    }
    if let Some(url) = url {
        // Validate before loading or migrating the registry.
        normalized.extend(["--url".into(), windowing::validated_external_url(&url)?]);
    }
    if let Some(flag) = flag {
        normalized.push(flag.into());
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_parser_preserves_names_and_explicit_destructive_flags() {
        let args = |values: &[&str]| values.iter().map(|v| (*v).to_owned()).collect();
        assert_eq!(
            normalize_action_args(args(&["uninstall", "--delete-data", "memos"])).unwrap(),
            args(&["uninstall", "memos", "--delete-data"])
        );
        assert_eq!(
            normalize_action_args(args(&["add", "--url=https://example.com", "My notes"])).unwrap(),
            args(&["add", "My notes", "--url", "https://example.com"])
        );
        for values in [
            vec!["bind-engine"],
            vec!["bind-engine", "memos", "--force"],
            vec!["uninstall", "memos", "--delete-dtaa"],
            vec!["remove", "memos", "--delete-data"],
            vec!["stop", "one", "two"],
            vec!["list", "extra"],
            vec!["uninstall", "memos", "--delete-data", "--delete-data"],
            vec!["add", "notes", "--url", "javascript:alert(1)"],
            vec![
                "add",
                "notes",
                "--url",
                "https://example.com",
                "--url",
                "https://other.com",
            ],
        ] {
            assert!(
                normalize_action_args(args(&values)).is_err(),
                "accepted {values:?}"
            );
        }
    }
    #[test]
    fn get_flag_returns_value() {
        assert_eq!(
            get_flag(
                &["add".into(), "x".into(), "--url".into(), "http://x".into()],
                "--url"
            ),
            Some("http://x".into())
        );
    }
    #[test]
    fn get_flag_absent_is_none() {
        assert_eq!(get_flag(&["add".into()], "--url"), None);
    }
    #[test]
    fn positional_rejects_flag() {
        assert!(positional(&["open".into(), "--browser".into()], 1, "usage").is_err());
    }
    #[test]
    fn usage_mentions_managed_commands() {
        let value = usage();
        for command in [
            "doctor",
            "install",
            "recipes",
            "start",
            "stop",
            "logs",
            "uninstall",
            "catalog",
        ] {
            assert!(value.contains(command));
        }
    }
    #[test]
    fn catalog_has_reviewed_memos() {
        assert_eq!(
            local_store::catalog::search_catalog("Memos", "", 0, 10).entries[0]
                .recipe_id
                .as_deref(),
            Some("memos")
        );
    }
}

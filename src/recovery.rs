//! What an interrupted install leaves behind, and how to clear it.
//!
//! The inventory is read-only: retained files are candidates, never proof of a
//! crash or permission to stop a container. Optional Docker label verification
//! is a snapshot, so `discard` — the one mutation here — rechecks all of it
//! under the app's operation lock rather than trusting what a person was
//! shown.
use crate::{
    error::{AppError, AppResult, ErrorCode},
    runtime::{CommandSpec, ProcessRunner, DIAGNOSTIC_TIMEOUT},
    storage,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct RecoveryCandidate {
    pub recipe_id: String,
    pub display_name: String,
    pub compose_file: PathBuf,
    pub project_name: String,
    pub docker_ownership_verified: bool,
    pub ownership_status: OwnershipStatus,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipStatus {
    NotChecked,
    NoContainers,
    Verified,
    Mismatch,
}

/// Inspect only IDs and Compose labels; never read container environment or run
/// a retained Compose file. This snapshot must be rechecked under lock before
/// any future recovery action, since Docker can change immediately afterward.
pub fn verify_with(candidate: &mut RecoveryCandidate, runner: &dyn ProcessRunner) -> AppResult<()> {
    candidate.docker_ownership_verified = false;
    candidate.ownership_status = OwnershipStatus::NotChecked;
    let project_dir = candidate
        .compose_file
        .parent()
        .ok_or_else(|| AppError::invalid("Recovery files have no project directory."))?;
    let query = |args: Vec<String>| -> AppResult<String> {
        let output = runner
            .run(&crate::runtime::engine::project_command(
                project_dir,
                CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT),
            )?)
            .map_err(AppError::from)?;
        if !output.success || output.truncated {
            return Err(AppError::invalid(
                "Docker ownership inspection failed or returned incomplete output.",
            ));
        }
        Ok(output.stdout)
    };
    let output = query(vec![
        "container".into(),
        "ls".into(),
        "--all".into(),
        "--quiet".into(),
        "--no-trunc".into(),
        "--filter".into(),
        format!(
            "label=com.docker.compose.project={}",
            candidate.project_name
        ),
    ])?;
    let ids: Vec<&str> = output.split_whitespace().collect();
    if ids.is_empty() {
        candidate.ownership_status = OwnershipStatus::NoContainers;
        return Ok(());
    }
    if ids.len() > 32
        || ids
            .iter()
            .any(|id| id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(AppError::invalid(
            "Docker returned an invalid ownership inventory.",
        ));
    }
    let mut args = vec![
        "container".into(),
        "inspect".into(),
        "--format".into(),
        "{{json .Config.Labels}}".into(),
    ];
    args.extend(ids.iter().map(|id| (*id).to_owned()));
    let labels = query(args)?;
    let lines: Vec<&str> = labels
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let expected = candidate.compose_file.canonicalize()?;
    let matches = lines.len() == ids.len()
        && lines.iter().all(|line| {
            let Ok(labels) =
                serde_json::from_str::<std::collections::BTreeMap<String, String>>(line)
            else {
                return false;
            };
            let get = |key: &str| labels.get(key).map(String::as_str).unwrap_or_default();
            let config = get("com.docker.compose.project.config_files");
            let working = get("com.docker.compose.project.working_dir");
            get("com.docker.compose.project") == candidate.project_name
                && get("com.docker.compose.service") == candidate.recipe_id
                && get("com.docker.compose.oneoff").eq_ignore_ascii_case("false")
                && Path::new(config).is_absolute()
                && Path::new(working).is_absolute()
                && Path::new(config).canonicalize().ok().as_ref() == Some(&expected)
                && Path::new(working).canonicalize().ok().as_deref() == expected.parent()
        });
    candidate.docker_ownership_verified = matches;
    candidate.ownership_status = if matches {
        OwnershipStatus::Verified
    } else {
        OwnershipStatus::Mismatch
    };
    Ok(())
}

pub fn inspect_at(config: &Path) -> AppResult<Vec<RecoveryCandidate>> {
    let live = storage::registry_v2_path_for_root(config);
    let previous = storage::registry_v2_previous_path_for_root(config);
    let installed = if live.exists() || previous.exists() {
        storage::load_registry_v2_at(config)?.apps
    } else {
        if storage::primary_v1_path_for_root(config).exists()
            || storage::legacy_v1_path_for_root(config).exists()
        {
            return Err(AppError::invalid("A legacy registry needs migration before recovery inspection. Open Local Store first."));
        }
        Vec::new()
    };
    let root = config.join(crate::brand::CONFIG_SLUG).join("apps");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let resolved_root = root.canonicalize()?;
    let mut candidates = Vec::new();
    // Every app a person could have started installing, not only the recipes.
    // Once imported apps became installable, an interrupted PrivateBin or
    // flatnotes install left files that recovery could not see, so the only
    // way out was to find them by hand.
    for offering in crate::offerings::offerings() {
        let id = offering.id().to_owned();
        if installed.iter().any(|app| app.id == id) {
            continue;
        }
        let project = root.join(&id);
        let compose = project.join("compose.yaml");
        if !compose.exists() {
            continue;
        }
        let resolved_project = project.canonicalize()?;
        let resolved_compose = compose.canonicalize()?;
        if resolved_project.parent() != Some(resolved_root.as_path())
            || resolved_project.file_name() != Some(std::ffi::OsStr::new(&id))
            || resolved_compose.parent() != Some(resolved_project.as_path())
            || !resolved_compose.is_file()
        {
            return Err(AppError::invalid(
                "Retained setup files resolve outside their managed app directory.",
            ));
        }
        candidates.push(RecoveryCandidate {
            project_name: format!("local-store-{id}"),
            recipe_id: id,
            display_name: offering.display_name().to_owned(),
            compose_file: resolved_compose,
            docker_ownership_verified: false,
            ownership_status: OwnershipStatus::NotChecked,
        });
    }
    Ok(candidates)
}

pub fn inspect() -> AppResult<Vec<RecoveryCandidate>> {
    let apps = storage::managed_apps_root();
    let config = apps
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| AppError::invalid("Invalid managed configuration root."))?;
    inspect_at(config)
}

/// What a discard actually did, so a caller can say so rather than guess.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Discarded {
    pub recipe_id: String,
    /// Containers the retained project owned when the lock was taken.
    pub containers_removed: usize,
    /// Whether the retained directory was deleted. False means the setup files
    /// and any data are still there.
    pub data_deleted: bool,
}

/// Clear the leftovers of an install that never finished.
///
/// This is the only mutation in this module, and everything the inspection
/// side is careful about applies here twice over. The candidate list a person
/// was shown is a snapshot: by the time they click, the app may have been
/// installed by another window, the containers may have gone, or something
/// else may have taken the project name. So none of that snapshot is trusted —
/// the whole check runs again under the app's operation lock, and the lock is
/// held until the removal is done.
///
/// Discard, not adopt. These files are from a transaction that never
/// committed: no health was confirmed and no registry entry was written. The
/// honest exit is to clear them so an ordinary install can proceed, rather than
/// to register an app whose install nobody finished.
pub fn discard(recipe_id: &str, delete_data: bool) -> AppResult<Discarded> {
    discard_with(&crate::runtime::SystemProcessRunner, recipe_id, delete_data)
}

pub fn discard_with(
    runner: &dyn ProcessRunner,
    recipe_id: &str,
    delete_data: bool,
) -> AppResult<Discarded> {
    // Taken before anything is read, and held until everything is done. An
    // install of this app running right now would otherwise have its files
    // removed from under it.
    let _lock = crate::runtime::lock_operation(recipe_id)?;

    let apps = storage::managed_apps_root();
    let config = apps
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| AppError::invalid("Invalid managed configuration root."))?
        .to_path_buf();

    // Re-derived under the lock rather than taken from the caller. This is
    // also what refuses an app that has since been installed: `inspect_at`
    // skips anything the registry lists, so a real app is simply not a
    // candidate and cannot be discarded by this path.
    let mut candidate = inspect_at(&config)?
        .into_iter()
        .find(|candidate| candidate.recipe_id == recipe_id)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::NotFound,
                "There are no retained setup files for this app. If it is installed, uninstall it instead.",
            )
        })?;

    let selected = crate::runtime::engine::EngineRunner {
        inner: runner,
        binding: crate::runtime::engine::retained(&apps.join(recipe_id))?,
    };
    let runner: &dyn ProcessRunner = &selected;
    // Retain this binding even after delete-data removes its file: the final
    // ownership check must still inspect the engine used for cleanup.
    // A snapshot taken before the lock proves nothing about now.
    verify_with(&mut candidate, runner)?;
    let containers = match candidate.ownership_status {
        OwnershipStatus::Verified => count_owned(runner, &candidate.project_name)?,
        OwnershipStatus::NoContainers => 0,
        // Something is running under this project name that this Compose file
        // does not account for. Stopping it would be acting on somebody else's
        // container.
        OwnershipStatus::Mismatch => {
            return Err(AppError::new(
                ErrorCode::UnsafePath,
                "Containers using this project name do not match the retained setup files. Review them in Docker before recovering.",
            ))
        }
        OwnershipStatus::NotChecked => {
            return Err(AppError::invalid(
                "Docker ownership could not be checked, so nothing was removed.",
            ))
        }
    };

    // Built from the managed root rather than from the canonicalized Compose
    // path: `confined_to_managed_root` compares against exactly this shape,
    // and on Windows a canonical path carries a `\?\` prefix that would
    // never match it. `inspect_at` has already proven the two resolve to the
    // same directory.
    let project_dir = apps.join(recipe_id);

    // Only the retained Compose file, only this project name, and only from
    // inside its own directory.
    let mut args = vec![
        "compose".to_owned(),
        "-f".to_owned(),
        candidate.compose_file.to_string_lossy().into_owned(),
        "-p".to_owned(),
        candidate.project_name.clone(),
        "down".to_owned(),
    ];
    if delete_data {
        args.push("--volumes".to_owned());
    }
    let output = runner
        .run(&CommandSpec::docker(
            args,
            Some(project_dir.clone()),
            crate::runtime::LIFECYCLE_TIMEOUT,
        ))
        .map_err(AppError::from)?;
    if !output.success {
        return Err(AppError::new(
            ErrorCode::ProcessFailed,
            "Docker could not remove the retained containers. Nothing was deleted.",
        ));
    }

    // Containers first, files second: a failure above leaves the files, which
    // is the recoverable order.
    if delete_data {
        crate::runtime::confined_to_managed_root(&project_dir, recipe_id, &apps)?;
        std::fs::remove_dir_all(&project_dir).map_err(AppError::from)?;
    }

    // The removal is only done when Docker agrees it is.
    if count_owned(runner, &candidate.project_name)? != 0 {
        return Err(AppError::new(
            ErrorCode::ProcessFailed,
            "Containers for this app are still present after recovery. Review them in Docker.",
        ));
    }
    Ok(Discarded {
        recipe_id: recipe_id.to_owned(),
        containers_removed: containers,
        data_deleted: delete_data,
    })
}

/// How many containers carry this Compose project label right now.
fn count_owned(runner: &dyn ProcessRunner, project_name: &str) -> AppResult<usize> {
    let output = runner
        .run(&CommandSpec::new(
            "docker",
            vec![
                "container".into(),
                "ls".into(),
                "--all".into(),
                "--quiet".into(),
                "--filter".into(),
                format!("label=com.docker.compose.project={project_name}"),
            ],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .map_err(AppError::from)?;
    if !output.success || output.truncated {
        return Err(AppError::invalid(
            "Docker ownership inspection failed or returned incomplete output.",
        ));
    }
    Ok(output.stdout.split_whitespace().count())
}

/// What an adoption actually did, so a caller can say so rather than guess.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Adopted {
    pub recipe_id: String,
    pub launch_url: String,
    /// Containers the retained project owned once it was running.
    pub containers: usize,
}

/// Finish an install that was interrupted, instead of clearing it.
///
/// `discard` was the honest first answer: files from a transaction that never
/// committed have no proven health and no registry entry, so clearing them and
/// letting an ordinary install proceed is always safe. But it throws away a
/// download and a database that may be minutes old, and for anyone on a slow
/// connection that is the difference between a recoverable hiccup and starting
/// over.
///
/// Adoption is the other answer, and the reason it took a separate change is
/// the health story. An install commits a registry entry only after the app
/// answers on its own address; adopting must hold that same line, or it would
/// put a broken app in My Apps and call it installed. So this brings the
/// retained project up and waits for the app to actually answer. If it never
/// does, nothing is registered and the files are left exactly where they were,
/// which keeps `discard` available.
pub fn adopt(recipe_id: &str) -> AppResult<Adopted> {
    adopt_with(
        &crate::runtime::SystemProcessRunner,
        &crate::runtime::HttpHealthProbe,
        recipe_id,
        // The same sixty seconds an install allows itself.
        std::time::Duration::from_secs(60),
    )
}

pub fn adopt_with(
    runner: &dyn ProcessRunner,
    probe: &dyn crate::runtime::HealthProbe,
    recipe_id: &str,
    health_timeout: std::time::Duration,
) -> AppResult<Adopted> {
    // Taken before anything is read and held until the entry is written, so an
    // install of this app running right now cannot have its files adopted out
    // from under it.
    let _lock = crate::runtime::lock_operation(recipe_id)?;

    let apps = storage::managed_apps_root();
    let config = apps
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| AppError::invalid("Invalid managed configuration root."))?
        .to_path_buf();

    // Re-derived under the lock. This also refuses an app that has since been
    // installed properly: `inspect_at` skips anything the registry lists.
    let mut candidate = inspect_at(&config)?
        .into_iter()
        .find(|candidate| candidate.recipe_id == recipe_id)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::NotFound,
                "There are no retained setup files for this app. If it is installed, there is nothing to adopt.",
            )
        })?;

    let selected = crate::runtime::engine::EngineRunner {
        inner: runner,
        binding: crate::runtime::engine::retained(&apps.join(recipe_id))?,
    };
    let runner: &dyn ProcessRunner = &selected;
    // The offering is what says who this app is. Adopting an id nothing offers
    // would put an app in My Apps that no review stands behind.
    let offering = crate::offerings::offering(recipe_id).ok_or_else(|| {
        AppError::new(
            ErrorCode::NotFound,
            "This app is not one this version offers, so it cannot be adopted.",
        )
    })?;

    // A snapshot taken before the lock proves nothing about now.
    verify_with(&mut candidate, runner)?;
    match candidate.ownership_status {
        OwnershipStatus::Verified | OwnershipStatus::NoContainers => {}
        OwnershipStatus::Mismatch => {
            return Err(AppError::new(
                ErrorCode::UnsafePath,
                "Containers using this project name do not match the retained setup files. Review them in Docker before recovering.",
            ))
        }
        OwnershipStatus::NotChecked => {
            return Err(AppError::invalid(
                "Docker ownership could not be checked, so nothing was adopted.",
            ))
        }
    }

    let project_dir = crate::folders::docker_path(
        candidate
            .compose_file
            .parent()
            .ok_or_else(|| AppError::invalid("Retained setup files have no project directory."))?,
    );
    // Everything handed to Docker, and everything written into the registry,
    // uses this form: an entry recorded with an extended-length path would
    // start today and fail every time the app was started afterwards.
    let compose_file = crate::folders::docker_path(&candidate.compose_file);

    // The address comes from the file that is actually there, not from what
    // the app would prefer today: an interrupted install may have taken a
    // different port, and adopting it under the wrong address would register a
    // link that goes nowhere.
    let compose = std::fs::read_to_string(&candidate.compose_file).map_err(AppError::from)?;
    let host_port = crate::plan::published_host_port(&compose)
        .ok_or_else(|| AppError::invalid("The retained setup files publish no address to open."))?;
    let launch_url = format!("http://localhost:{host_port}");

    // Bring up whatever the interruption left down. This is the same command
    // the install itself would have run, against the same file, under the same
    // project name.
    let output = runner
        .run(&CommandSpec::docker(
            vec![
                "compose".to_owned(),
                "-f".to_owned(),
                compose_file.to_string_lossy().into_owned(),
                "-p".to_owned(),
                candidate.project_name.clone(),
                "up".to_owned(),
                "-d".to_owned(),
            ],
            Some(project_dir.clone()),
            crate::runtime::LIFECYCLE_TIMEOUT,
        ))
        .map_err(AppError::from)?;
    if !output.success {
        // Say what Docker said. "Could not start" alone leaves a person with
        // nothing to act on, and this is the step most likely to fail for a
        // reason they can fix — a port taken since, an image since removed.
        let detail = crate::runtime::redact(
            output
                .stderr
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>()
                .join(
                    "
",
                )
                .trim(),
        );
        let message = if detail.is_empty() {
            "Docker could not start the retained containers, so nothing was adopted.".to_owned()
        } else {
            format!(
                "Docker could not start the retained containers, so nothing was adopted: {detail}"
            )
        };
        return Err(AppError::new(ErrorCode::ProcessFailed, message));
    }

    // The line an install holds, held here too. Without this, adoption would
    // be a way to register an app nobody has seen work.
    crate::runtime::wait_for_health_with(
        probe,
        &launch_url,
        health_timeout,
        &crate::runtime::CancelToken::new(),
    )
        .map_err(|_| {
            AppError::new(
                ErrorCode::TimedOut,
                "The app did not answer on its address, so it was not added. The setup files are untouched, and you can recover them instead.",
            )
        })?;

    let containers = count_owned(runner, &candidate.project_name)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    storage::insert_installed_app(crate::model::InstalledApp {
        id: recipe_id.to_owned(),
        catalog_id: crate::catalog::catalog_id(offering.catalog_name()),
        display_name: offering.display_name().to_owned(),
        launch_url: launch_url.clone(),
        icon_path: None,
        runtime: crate::model::RuntimeSpec::Compose {
            project_name: candidate.project_name.clone(),
            project_dir,
            compose_file,
        },
        created_at_unix: now,
        updated_at_unix: now,
    })
    .map_err(AppError::from)?;

    Ok(Adopted {
        recipe_id: recipe_id.to_owned(),
        launch_url,
        containers,
    })
}

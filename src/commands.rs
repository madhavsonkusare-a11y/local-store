//! Typed launcher commands. Remote app windows have no permission to invoke these.
use crate::{
    catalog,
    error::{AppError, AppResult, ErrorCode},
    model::{InstalledApp, RuntimeSpec},
    runtime::{self, AppStatus, DoctorReport},
    storage, windowing,
};
use serde::Serialize;
use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
static REGISTRY_WRITE: Mutex<()> = Mutex::new(());

#[tauri::command]
pub async fn inspect_recovery(
    window: tauri::WebviewWindow,
) -> AppResult<Vec<crate::recovery::RecoveryCandidate>> {
    require_launcher(&window)?;
    tauri::async_runtime::spawn_blocking(|| {
        let mut candidates = crate::recovery::inspect()?;
        for candidate in &mut candidates {
            crate::recovery::verify_with(candidate, &runtime::SystemProcessRunner)?;
        }
        Ok(candidates)
    })
    .await
    .map_err(AppError::internal)?
}

#[derive(Serialize)]
pub struct AppView {
    #[serde(flatten)]
    pub app: InstalledApp,
    pub status: AppStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error: Option<AppError>,
}
pub fn require_launcher(window: &tauri::WebviewWindow) -> AppResult<()> {
    if window.label() == "launcher" {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::Forbidden,
            "Only the launcher can perform this action.",
        ))
    }
}
fn now() -> AppResult<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(AppError::internal)?
        .as_secs())
}
fn find_app(id: &str) -> AppResult<InstalledApp> {
    storage::load_or_migrate_registry()
        .map_err(AppError::from)?
        .apps
        .into_iter()
        .find(|app| app.id == id)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::NotFound,
                "This app no longer exists. Refresh My Apps.",
            )
        })
}

#[tauri::command]
pub async fn list_apps(window: tauri::WebviewWindow) -> AppResult<Vec<AppView>> {
    require_launcher(&window)?;
    tauri::async_runtime::spawn_blocking(|| {
        let registry = storage::load_or_migrate_registry().map_err(AppError::from)?;
        Ok(registry
            .apps
            .into_iter()
            .map(|app| {
                let (status, status_error) = match runtime::status(&app) {
                    Ok(status) => (status, None),
                    Err(error) => (AppStatus::Error, Some(error)),
                };
                AppView {
                    app,
                    status,
                    status_error,
                }
            })
            .collect())
    })
    .await
    .map_err(AppError::internal)?
}

pub fn validate_connection(name: &str, url: &str) -> AppResult<(String, String)> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "Use an app name between 1 and 80 characters without control characters.",
        ));
    }
    Ok((
        name.into(),
        windowing::validated_external_url(url.trim()).map_err(AppError::invalid)?,
    ))
}
#[tauri::command]
pub fn add_app(
    window: tauri::WebviewWindow,
    name: String,
    url: String,
    catalog_id: Option<String>,
) -> AppResult<()> {
    require_launcher(&window)?;
    let (name, url) = validate_connection(&name, &url)?;
    let _lock = REGISTRY_WRITE.lock().map_err(AppError::internal)?;
    let registry = storage::load_or_migrate_registry().map_err(AppError::from)?;
    if registry
        .apps
        .iter()
        .any(|app| app.display_name.eq_ignore_ascii_case(&name))
    {
        return Err(AppError::new(
            ErrorCode::AlreadyExists,
            "That name is already in My Apps. Choose a different name.",
        ));
    }
    let base = storage::slug_for_display_name(&name);
    let id = crate::model::next_available_installed_app_id(&base, |candidate| {
        !registry.apps.iter().any(|app| app.id == candidate)
    })
    .ok_or_else(|| AppError::internal("Could not create a safe app ID."))?;
    let timestamp = now()?;
    storage::insert_installed_app(InstalledApp {
        id,
        catalog_id: catalog_id.as_deref().and_then(catalog::catalog_id),
        display_name: name,
        launch_url: url,
        icon_path: None,
        runtime: RuntimeSpec::External,
        created_at_unix: timestamp,
        updated_at_unix: timestamp,
    })
    .map_err(AppError::from)
}
#[tauri::command]
pub fn remove_app_cmd(window: tauri::WebviewWindow, id: String) -> AppResult<()> {
    require_launcher(&window)?;
    let _lock = REGISTRY_WRITE.lock().map_err(AppError::internal)?;
    let app = find_app(&id)?;
    if app.is_managed() {
        return Err(AppError::new(
            ErrorCode::UnsupportedOperation,
            "Use Uninstall for an app managed by Local Store.",
        ));
    }
    storage::remove_installed_app(&id)
        .map(|_| ())
        .map_err(AppError::from)
}
#[tauri::command]
pub fn search_catalog(
    window: tauri::WebviewWindow,
    query: String,
    category: String,
    offset: usize,
    limit: usize,
    filters: Option<catalog::Filters>,
) -> AppResult<catalog::CatalogPage> {
    require_launcher(&window)?;
    Ok(catalog::search_filtered(
        &query,
        &category,
        offset,
        limit,
        &filters.unwrap_or_default(),
    ))
}
#[tauri::command]
pub fn open_project(window: tauri::WebviewWindow, url: String) -> AppResult<()> {
    require_launcher(&window)?;
    open::that_detached(windowing::validated_external_url(&url).map_err(AppError::invalid)?)
        .map_err(|e| AppError::new(ErrorCode::BrowserOpenFailed, e))
}
#[tauri::command]
pub async fn doctor(window: tauri::WebviewWindow) -> AppResult<DoctorReport> {
    require_launcher(&window)?;
    tauri::async_runtime::spawn_blocking(runtime::doctor)
        .await
        .map_err(AppError::internal)
}

/// Read-only managed-engine setup state for the launcher. Bootstrap actions
/// remain unavailable until the product's explicit consent/elevation flow is
/// implemented.
#[tauri::command]
pub async fn managed_engine_status(
    window: tauri::WebviewWindow,
) -> AppResult<runtime::engine::wsl::bootstrap::BootstrapStatus> {
    require_launcher(&window)?;
    tauri::async_runtime::spawn_blocking(|| {
        runtime::engine::wsl::bootstrap::inspect(
            &runtime::SystemProcessRunner,
            &storage::managed_engine_state_root(),
        )
    })
    .await
    .map_err(AppError::internal)?
}
/// The template a reviewed recipe corresponds to.
///
/// One function so the setup a person reviews and the setup an install
/// enforces are the same thing. Reviewed recipes declare no typed fields yet;
/// when one does, both sides gain it together.
fn offering_for(id: &str) -> AppResult<crate::offerings::Offering> {
    crate::offerings::offering(id).ok_or_else(|| {
        AppError::new(
            ErrorCode::NotFound,
            "This recipe is not verified for installation.",
        )
    })
}

#[derive(serde::Serialize)]
pub struct RecipeReview {
    /// Flattened so the launcher reads the same field names whether this app
    /// came from a reviewed recipe or an approved imported template.
    #[serde(flatten)]
    pub recipe: crate::offerings::OfferingSummary,
    pub setup_review: crate::setup::SetupReview,
}

#[tauri::command]
pub fn recipe_details(window: tauri::WebviewWindow, id: String) -> AppResult<RecipeReview> {
    require_launcher(&window)?;
    let offering = offering_for(&id)?;
    let setup_review = offering
        .plan_template(None)?
        .setup_review()
        .map_err(AppError::invalid)?;
    Ok(RecipeReview {
        recipe: offering.summary(None)?,
        setup_review,
    })
}
/// Ask a running install to stop.
///
/// The request names both the app and the operation, so a click that arrives
/// after an install has finished cannot cancel whatever replaced it. Returns
/// whether a matching operation was still running; a `false` is normal and not
/// an error. Cancellation is honoured at checkpoints only, and never after the
/// registry commit begins.
/// Whether an app is answering on its address right now.
///
/// Separate from `list_apps` on purpose: a probe per app would make listing as
/// slow as the least reachable app. The launcher asks for each app after the
/// list is on screen, so rows fill in as answers arrive.
#[tauri::command]
pub async fn app_readiness(
    window: tauri::WebviewWindow,
    id: String,
) -> AppResult<runtime::Readiness> {
    require_launcher(&window)?;
    let app = find_app(&id)?;
    tauri::async_runtime::spawn_blocking(move || runtime::readiness(&app))
        .await
        .map_err(AppError::internal)
}

/// Whether an address answers, before it is saved as a connection.
///
/// Advisory only. An app the user has simply not started yet is a perfectly
/// good connection to save, so this never blocks saving — it only tells them
/// what was found, so a typo does not become a silently dead entry.
#[tauri::command]
pub async fn check_address(
    window: tauri::WebviewWindow,
    url: String,
) -> AppResult<runtime::Readiness> {
    require_launcher(&window)?;
    let url = windowing::validated_external_url(url.trim()).map_err(AppError::invalid)?;
    tauri::async_runtime::spawn_blocking(move || runtime::address_readiness(&url))
        .await
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn cancel_app_setup(
    window: tauri::WebviewWindow,
    id: String,
    operation_id: u64,
) -> AppResult<bool> {
    require_launcher(&window)?;
    Ok(runtime::cancel_operation(&id, operation_id))
}

#[tauri::command]
pub async fn install_app(
    window: tauri::WebviewWindow,
    recipe_id: String,
    host_port: Option<u16>,
    answers: Option<std::collections::BTreeMap<String, String>>,
) -> AppResult<()> {
    require_launcher(&window)?;
    // The same lookup `recipe_details` used, so the install cannot resolve to
    // something other than what the person was shown — and a withheld template
    // is not resolvable at all.
    let offering = offering_for(&recipe_id)?;
    // A chosen port republishes the pinned recipe; the recipe itself is a
    // reviewed constant and is never mutated. The remap re-validates, so an
    // address left behind by the rewrite fails here rather than at the daemon.
    // An imported template has no Compose text to rewrite, so its single
    // published endpoint moves instead.
    let recipe = offering.recipe(host_port)?;
    // Answers are checked against what this app actually declares, before
    // anything is locked or written. `accept_answers` refuses a key no field
    // asked for, so a client cannot supply an environment value the review
    // never showed anyone.
    let answers = answers.unwrap_or_default();
    let template = offering.plan_template(host_port)?;
    let app_id = offering.id().to_owned();
    let display_name = offering.display_name().to_owned();
    template.accept_answers(&answers).map_err(|errors| {
        let mut listed: Vec<String> = errors
            .iter()
            .map(|error| format!("{}: {}", error.key, error.message))
            .collect();
        listed.sort();
        AppError::new(ErrorCode::InvalidInput, listed.join("; "))
    })?;

    if storage::load_or_migrate_registry()
        .map_err(AppError::from)?
        .apps
        .iter()
        .any(|app| app.id == app_id)
    {
        return Err(AppError::new(
            ErrorCode::AlreadyExists,
            format!("{display_name} is already in My Apps."),
        ));
    }
    // Claimed before any worker starts, so a busy app is reported at once and
    // the operation id can travel with the very first event.
    let lock = runtime::lock_operation(&app_id)?;
    let cancel_id = lock.id();
    crate::operations::track_install(
        &window,
        &recipe_id,
        Some(cancel_id),
        |progress| async move {
            // The install holds the app's operation lock open until the commit
            // lands, so nothing can act on the app in between. The commit is the
            // cancellation cutoff and handles its own rollback, preserving any
            // data that predates this install.
            let worker_progress = progress.clone();
            let commit_progress = progress.clone();
            tauri::async_runtime::spawn_blocking(move || {
                // A recipe with no typed setup installs from its own reviewed
                // Compose file, byte for byte. Rendering it from a plan would
                // put a second author between the review and what ships. An
                // imported template has no such file and is always rendered.
                let pending = match &recipe {
                    Some(recipe) if template.fields.is_empty() && template.secrets.is_empty() => {
                        runtime::begin_install(recipe, lock, worker_progress.as_ref())?
                    }
                    _ => runtime::begin_template_install(
                        &template,
                        &display_name,
                        &answers,
                        lock,
                        worker_progress.as_ref(),
                    )?,
                };
                pending.commit(commit_progress.as_ref())
            })
            .await
            .map_err(AppError::internal)??;
            Ok(())
        },
    )
    .await
}
#[tauri::command]
pub async fn start_app(window: tauri::WebviewWindow, id: String) -> AppResult<()> {
    require_launcher(&window)?;
    crate::operations::track(
        &window,
        &id,
        crate::operations::OperationKind::Start,
        async {
            let app = find_app(&id)?;
            tauri::async_runtime::spawn_blocking(move || runtime::start(&app))
                .await
                .map_err(AppError::internal)?
        },
    )
    .await
}
#[tauri::command]
pub async fn stop_app(window: tauri::WebviewWindow, id: String) -> AppResult<()> {
    require_launcher(&window)?;
    crate::operations::track(
        &window,
        &id,
        crate::operations::OperationKind::Stop,
        async {
            let app = find_app(&id)?;
            tauri::async_runtime::spawn_blocking(move || runtime::stop(&app))
                .await
                .map_err(AppError::internal)?
        },
    )
    .await
}
#[tauri::command]
pub async fn app_logs(window: tauri::WebviewWindow, id: String) -> AppResult<String> {
    require_launcher(&window)?;
    let app = find_app(&id)?;
    tauri::async_runtime::spawn_blocking(move || runtime::logs(&app))
        .await
        .map_err(AppError::internal)?
}
#[tauri::command]
pub async fn uninstall_app(
    window: tauri::WebviewWindow,
    id: String,
    delete_data: bool,
) -> AppResult<()> {
    require_launcher(&window)?;
    crate::operations::track(
        &window,
        &id,
        crate::operations::OperationKind::Uninstall,
        async {
            let app = find_app(&id)?;
            tauri::async_runtime::spawn_blocking(move || {
                runtime::uninstall_and_remove(&app, delete_data)
            })
            .await
            .map_err(AppError::internal)?
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The form a person fills in and the answers an install accepts have to
    /// come from the same template, or the review shows one thing and the
    /// install enforces another. Both sides now go through `offering_for`, so
    /// this covers imported apps as well as recipes.
    #[test]
    fn a_reviewed_recipe_offers_no_setup_fields_and_accepts_no_answers() {
        for recipe in crate::recipes::reviewed_recipes() {
            let template = offering_for(&recipe.id)
                .expect("a reviewed recipe is offered")
                .plan_template(None)
                .expect("recipe should convert");
            let review = template.setup_review().expect("template should project");
            assert!(
                review.fields.is_empty(),
                "{} declares setup fields the install path has never been run with",
                recipe.id
            );
            assert_eq!(review.service_count, template.plan.services.len());

            // An answer nobody asked for is refused rather than dropped: a
            // client cannot supply an environment value the review never
            // showed anyone.
            let smuggled = [("ADMIN_TOKEN".to_owned(), "let-me-in".to_owned())]
                .into_iter()
                .collect();
            let errors = template
                .accept_answers(&smuggled)
                .expect_err("an undeclared answer must be refused");
            assert_eq!(errors[0].key, "ADMIN_TOKEN");
            assert!(
                !errors[0].message.contains("let-me-in"),
                "the refusal echoed the value it was given"
            );
        }
    }

    #[test]
    fn connection_validation_accepts_lan_and_rejects_unsafe_or_empty_values() {
        assert_eq!(
            validate_connection(" My app ", " http://192.168.1.2:3000 ").unwrap(),
            ("My app".into(), "http://192.168.1.2:3000".into())
        );
        for (name, url) in [
            ("", "https://example.com"),
            ("app", "file:///tmp/a"),
            ("app", "https://user:pass@example.com"),
            ("app", "http://bad host"),
            ("app", "https://"),
        ] {
            assert!(validate_connection(name, url).is_err());
        }
    }
}

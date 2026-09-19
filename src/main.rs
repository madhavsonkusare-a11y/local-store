#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use local_store::{
    brand::PRODUCT_NAME,
    commands,
    error::{AppError, AppResult, ErrorCode},
    platform, runtime, storage, windowing,
};

use tauri_plugin_deep_link::DeepLinkExt;

mod cli;

#[tauri::command]
fn create_shortcut(window: tauri::WebviewWindow, id: String) -> AppResult<String> {
    commands::require_launcher(&window)?;
    let app = storage::load_or_migrate_registry()
        .map_err(AppError::from)?
        .apps
        .into_iter()
        .find(|app| app.id == id)
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "This app no longer exists."))?;
    let bin = std::env::current_exe()
        .map_err(AppError::from)?
        .to_string_lossy()
        .into_owned();
    platform::create_shortcut_for(
        &app.id,
        &app.display_name,
        &bin,
        app.icon_path.as_ref().and_then(|path| path.to_str()),
    )
    .map_err(|e| AppError::new(ErrorCode::ShortcutFailed, e))
}

#[tauri::command]
async fn open_app(
    window: tauri::WebviewWindow,
    app_handle: tauri::AppHandle,
    id: String,
) -> AppResult<()> {
    commands::require_launcher(&window)?;
    local_store::operations::track(
        &window,
        &id,
        local_store::operations::OperationKind::Open,
        async {
            let app = storage::load_or_migrate_registry()
                .map_err(AppError::from)?
                .apps
                .into_iter()
                .find(|app| app.id == id)
                .ok_or_else(|| AppError::new(ErrorCode::NotFound, "This app no longer exists."))?;
            if app.is_managed() {
                let pending = app.clone();
                tauri::async_runtime::spawn_blocking(move || runtime::start(&pending))
                    .await
                    .map_err(AppError::internal)??;
            }
            windowing::build_window(
                &app_handle,
                &app.id,
                &app.display_name,
                &app.launch_url,
                app.icon_path.as_ref().and_then(|path| path.to_str()),
            )
            .map_err(|e| AppError::new(ErrorCode::WindowOpenFailed, e))
        },
    )
    .await
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "open") {
        cli::ensure_console();
    }
    let activation = match local_store::activation::request_from_args(&args[1..]) {
        Ok(request) => request,
        Err(error) => {
            cli::ensure_console();
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let initial_app = activation
        .map(|request| {
            let registry = storage::load_or_migrate_registry().map_err(AppError::from)?;
            let app = request.resolve(&registry.apps)?.clone();
            Ok::<_, AppError>(app)
        })
        .transpose()
        .unwrap_or_else(|error| {
            cli::ensure_console();
            eprintln!("{error}");
            std::process::exit(1);
        });

    // If invoked with CLI subcommands (e.g. `local-store add ...`), run as CLI and exit.
    if initial_app.is_none() && args.len() > 1 {
        let code = cli::run_cli();
        // `process::exit` runs no destructors and flushes no buffered
        // writer. Without this the CLI's output is lost whenever stdout
        // is a file or pipe handed over by a shell.
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(code);
    }

    tauri::Builder::default()
        .manage(local_store::native::NativeState::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            match local_store::native::secondary_request(&args) {
                Ok(Some(request)) => local_store::native::dispatch(app, request),
                Ok(None) if args.len() == 1 => local_store::native::focus_launcher(app),
                Ok(None) => {}
                Err(error) => local_store::native::report_failure(app, error),
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .setup(move |app| {
            storage::load_or_migrate_registry().map_err(std::io::Error::other)?;
            let win = tauri::WebviewWindowBuilder::new(
                app,
                "launcher",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title(PRODUCT_NAME)
            .theme(Some(tauri::Theme::Dark))
            .inner_size(1180.0, 760.0)
            .min_inner_size(800.0, 600.0)
            .resizable(true)
            .build()?;
            let _ = win.set_focus();
            let deep_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    match local_store::activation::parse_deep_link(url.as_str()) {
                        Ok(request) => local_store::native::dispatch(&deep_handle, request),
                        Err(error) => local_store::native::report_failure(&deep_handle, error),
                    }
                }
            });
            if let Err(error) = platform::register_protocol(app.handle()) {
                local_store::native::report_failure(app.handle(), error);
            }
            if let Some(initial) = &initial_app {
                local_store::native::dispatch(
                    app.handle(),
                    local_store::activation::OpenRequest {
                        target: initial.id.clone(),
                        allow_display_name: false,
                    },
                );
            } else if let Some(urls) = app.deep_link().get_current()? {
                for url in urls {
                    match local_store::activation::parse_deep_link(url.as_str()) {
                        Ok(request) => local_store::native::dispatch(app.handle(), request),
                        Err(error) => local_store::native::report_failure(app.handle(), error),
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_apps,
            local_store::native::take_activation_errors,
            commands::add_app,
            open_app,
            create_shortcut,
            commands::remove_app_cmd,
            commands::search_catalog,
            commands::open_project,
            commands::doctor,
            commands::managed_engine_status,
            commands::inspect_recovery,
            commands::recipe_details,
            commands::install_app,
            commands::cancel_app_setup,
            commands::check_address,
            commands::app_readiness,
            commands::start_app,
            commands::stop_app,
            commands::app_logs,
            commands::uninstall_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running Local Store");
}

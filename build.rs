#[path = "src/catalog_schema.rs"]
mod catalog_schema;

fn main() {
    println!("cargo:rerun-if-changed=src/generated/catalog.json");
    println!("cargo:rerun-if-changed=src/assets/catalog");
    let catalog: catalog_schema::Catalog = serde_json::from_str(
        &std::fs::read_to_string("src/generated/catalog.json").expect("read generated catalog"),
    )
    .expect("parse catalog schema");
    catalog
        .validate(std::path::Path::new("src"))
        .expect("validate generated catalog");
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_apps",
            "take_activation_errors",
            "add_app",
            "open_app",
            "create_shortcut",
            "remove_app_cmd",
            "search_catalog",
            "resolve_github_source",
            "open_project",
            "doctor",
            "managed_engine_status",
            "inspect_recovery",
            "recipe_details",
            "install_app",
            "start_app",
            "stop_app",
            "app_logs",
            "uninstall_app",
            "cancel_app_setup",
            "app_readiness",
            "check_address",
        ]),
    ))
    .expect("build Tauri application");
}

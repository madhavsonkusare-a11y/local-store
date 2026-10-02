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
            "launch_readiness",
            "launch_readiness_batch",
            "engine_setup_preview",
            "engine_setup_action",
            "agent_connections",
            "agent_content_connections",
            "agent_content_connect",
            "agent_content_disconnect",
            "agent_content_grant",
            "agent_content_requests",
            "agent_content_decide",
            "agent_data_grant",
            "agent_mutation_requests",
            "agent_mutation_decide",
            "agent_enrollment_begin",
            "agent_enrollment_export",
            "agent_enrollment_cancel",
            "agent_client_revoke",
            "agent_grant_set",
            "agent_grant_revoke",
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
            "launcher_state",
            "onboarding_progress",
            "mark_onboarding_viewed",
            "repair_managed_engine",
            "discard_retained_setup",
            "adopt_retained_setup",
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

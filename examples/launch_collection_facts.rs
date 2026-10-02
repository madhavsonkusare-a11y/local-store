//! Read-only display facts for the selected launch cohort; no engine commands.
fn main() {
    let curated: serde_json::Value =
        serde_json::from_slice(include_bytes!("../catalog/v1-qualified-apps.json"))
            .expect("checked launch cohort");
    let ids: Vec<String> = curated["apps"]
        .as_array()
        .expect("selected apps")
        .iter()
        .map(|app| app["id"].as_str().unwrap().to_owned())
        .collect();
    let facts = local_store::launch_readiness::for_offerings(&ids);
    let summary = serde_json::json!({
        "selected": ids.len(),
        "zero_input": facts.iter().filter(|fact| fact.install_mode == "zero_input").count(),
        "setup_assisted": facts.iter().filter(|fact| fact.install_mode == "setup_assisted").count(),
        "current_verified_tasks": facts.iter().filter(|fact| fact.task_verified && fact.current_evidence).count(),
        "apps": facts
    });
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}

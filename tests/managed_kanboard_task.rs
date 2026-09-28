//! Opt-in managed-engine proof of a Kanboard project/task API workflow.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn kanboard_project_task_move_survives_reinstall() {
    use local_store::{
        qualification::{qualify_on_engine_at, FirstUse, ScriptProbe},
        runtime::engine::EngineBinding,
    };
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_MANAGED_QUALIFICATION").as_deref(),
        Ok("1")
    );
    const TASK: &str = "a project and exact task are created, the task moves to a second column, and both survive restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let state = root.join(format!(
        ".cache/managed-qualification/kanboard-task-{nonce}.json"
    ));
    let probe = ScriptProbe::new(root.join("scripts/kanboard-task-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let answers = BTreeMap::from([("PLUGIN_INSTALLER".to_owned(), "false".to_owned())]);
    let result = qualify_on_engine_at(
        "kanboard",
        &answers,
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/kanboard-managed-task-2026-09-28.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "Kanboard task failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}

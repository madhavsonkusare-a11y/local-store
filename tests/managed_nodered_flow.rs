//! Opt-in managed-engine proof that a deployed Node-RED flow stays usable.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn nodered_flow_survives_reinstall() {
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
    const TASK: &str = "a deployed function flow returns its computed value, the editor stays connected, and both survive restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let state = root.join(format!(
        ".cache/managed-qualification/nodered-flow-{nonce}.json"
    ));
    let probe = ScriptProbe::new(root.join("scripts/nodered-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let result = qualify_on_engine_at(
        "nodered",
        &BTreeMap::new(),
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/nodered-managed-flow-2026-09-28.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "Node-RED task failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}

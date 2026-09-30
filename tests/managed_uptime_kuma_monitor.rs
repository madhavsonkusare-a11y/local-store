//! Opt-in managed-engine proof of Uptime Kuma's actual monitoring task.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn uptime_kuma_self_monitor_survives_reinstall() {
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
    const TASK: &str = "a local admin creates a unique HTTP monitor of the app's own page, it records Up, and that exact monitor remains Up after restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let state = root.join(format!(
        ".cache/managed-qualification/uptime-kuma-monitor-{nonce}.json"
    ));
    let probe = ScriptProbe::new(root.join("scripts/uptime-kuma-monitor-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let result = qualify_on_engine_at(
        "uptime-kuma",
        &BTreeMap::new(),
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/uptime-kuma-managed-monitor-2026-09-30.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "Uptime Kuma monitor failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}

//! Explicit opt-in proof on the owned WSL engine. This does not assert a
//! meaningful in-app task; Q04 must supply that separately.

#[cfg(windows)]
#[test]
#[ignore = "real managed WSL lifecycle; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn memos_lifecycle_and_resource_sampling_on_managed_engine() {
    qualify_one("memos");
}

#[cfg(windows)]
#[test]
#[ignore = "real managed WSL lifecycle; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn n8n_named_volume_sampling_on_managed_engine() {
    qualify_one("n8n");
}

#[cfg(windows)]
fn qualify_one(app: &str) {
    use local_store::{
        qualification::{qualify_on_engine_at, AnswersOnly, FirstUse, ScriptProbe},
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let probe_state = root.join(format!(
        ".cache/managed-qualification/memos-proof-{nonce}.json"
    ));
    let probe: Box<dyn FirstUse> = if app == "memos" {
        Box::new(ScriptProbe::new(
            root.join("scripts/memos-content-probe.mjs"),
            "an admin signs in, creates a private memo, and reads the same content after restart and reinstall",
        ).with_args(vec![probe_state.to_string_lossy().into_owned()]))
    } else {
        Box::new(AnswersOnly)
    };
    let result = qualify_on_engine_at(
        app,
        &BTreeMap::new(),
        probe.as_ref(),
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&probe_state);
    let evidence = result.expect("managed qualification could run");
    let output = root.join(format!(
        "docs/evidence/{app}-managed-resource-2026-09-23.json"
    ));
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "managed {app} failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
    assert_eq!(
        evidence.identity.unwrap().engine,
        EngineBinding::managed_wsl()
    );
}

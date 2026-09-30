//! Opt-in managed-engine proof of an exact WordPress post.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn wordpress_post_survives_reinstall() {
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
    const TASK: &str = "an administrator installs WordPress, publishes an exact unique post, and reads the same published content after restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let state = root.join(format!(
        ".cache/managed-qualification/wordpress-post-{nonce}.json"
    ));
    let probe = ScriptProbe::new(root.join("scripts/wordpress-post-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let result = qualify_on_engine_at(
        "wordpress",
        &BTreeMap::new(),
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/wordpress-managed-post-2026-09-30.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "WordPress post failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}

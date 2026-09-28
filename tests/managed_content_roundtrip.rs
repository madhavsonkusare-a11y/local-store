//! Opt-in pilot of the shared content driver. These are new proofs, not a
//! replacement for the existing Memos evidence until a live run passes.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn shared_content_driver_on_flatnotes() {
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let state = root.join(format!(
        ".cache/managed-qualification/flatnotes-content-{nonce}.json"
    ));
    let user = "local-store-probe";
    let password = "a-password-somebody-chose-9271";
    let answers = BTreeMap::from([
        ("FLATNOTES_AUTH_TYPE".to_owned(), "password".to_owned()),
        ("FLATNOTES_USERNAME".to_owned(), user.to_owned()),
        ("FLATNOTES_PASSWORD".to_owned(), password.to_owned()),
    ]);
    const TASK: &str = "the chosen password signs in, a wrong password fails, and an exact unique note survives restart and keep-data reinstall";
    let probe =
        ScriptProbe::new(root.join("scripts/content-roundtrip-probe.mjs"), TASK).with_args(vec![
            state.to_string_lossy().into_owned(),
            "flatnotes".into(),
            user.into(),
            password.into(),
        ]);
    let result = qualify_on_engine_at(
        "flatnotes",
        &answers,
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/flatnotes-managed-content-2026-09-28.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "flatnotes content failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}

//! Opt-in managed-engine proof of Immich photo upload, search and original download.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn immich_photo_survives_reinstall() {
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
    const TASK: &str = "an administrator uploads an owned photo, finds it by filename, and downloads identical original bytes after restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let media = root.join(format!(".cache/managed-qualification/immich-media-{nonce}"));
    std::fs::create_dir_all(&media).expect("owned media folder created");
    let state = root.join(format!(
        ".cache/managed-qualification/immich-photo-{nonce}.json"
    ));
    let probe = ScriptProbe::new(root.join("scripts/immich-photo-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let fields = BTreeMap::from([(
        "FOLDER_IMAGES_IMMICH".to_owned(),
        media.to_string_lossy().into_owned(),
    )]);
    let result = qualify_on_engine_at(
        "immich",
        &fields,
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    assert!(media.starts_with(root.join(".cache/managed-qualification")));
    let _ = std::fs::remove_dir_all(&media);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/immich-managed-photo-2026-09-30.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "Immich photo failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}

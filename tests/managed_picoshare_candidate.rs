//! A withheld frozen-roster candidate is qualified without catalog promotion.
#[cfg(windows)]
#[test]
#[ignore = "serialized real managed-engine candidate run; explicit opt-in required"]
fn withheld_picoshare_file_sharing_survives_reinstall() {
    use local_store::{
        qualification::{qualify_template_on_engine_at, ScriptProbe, Subject},
        runtime::engine::EngineBinding,
        templates::ReviewedTemplate,
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
    const TASK: &str = "an isolated admin passphrase signs in while a wrong passphrase fails; an exact synthetic file, sharing expiration and anonymous byte-identical download survive restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proposal = root.join("catalog/promotion-proposals/picoshare-withheld.json");
    let reviewed: ReviewedTemplate =
        serde_json::from_slice(&std::fs::read(&proposal).unwrap()).unwrap();
    assert_eq!(reviewed.id, "picoshare");
    assert!(!reviewed.offerable());
    assert!(local_store::offerings::offering("picoshare").is_none());
    let template = reviewed
        .plan_template()
        .expect("exact reviewed candidate mapping");
    let field = template
        .fields
        .iter()
        .find(|field| field.key == "CAP_PS_SHARED_SECRET")
        .unwrap();
    assert!(field.required && field.sensitive);
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).unwrap();
    let password: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = root.join(".cache/managed-qualification");
    std::fs::create_dir_all(&scratch).unwrap();
    let state = scratch.join(format!("picoshare-{nonce}.json"));
    std::fs::write(
        &state,
        serde_json::to_vec(&serde_json::json!({"password":password})).unwrap(),
    )
    .unwrap();
    struct RemoveState(PathBuf);
    impl Drop for RemoveState {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _remove = RemoveState(state.clone());
    let probe = ScriptProbe::new(root.join("scripts/picoshare-file-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let answers = BTreeMap::from([("CAP_PS_SHARED_SECRET".to_owned(), password)]);
    let observed = reviewed
        .requirements
        .images
        .iter()
        .map(|image| image.checked_at.as_str())
        .min()
        .unwrap()
        .to_owned();
    let about = Subject {
        app: reviewed.id.clone(),
        kind: "withheld candidate mapping",
        promotion: "withheld".into(),
        source_revision: reviewed.origin.revision.clone(),
        source_adapter: reviewed.origin.importer.clone(),
        source_locator: format!("{}#{}", reviewed.origin.repository, reviewed.origin.path),
        source_observed_on: reviewed.verified_at.clone(),
        images_observed_on: observed,
    };
    let result = qualify_template_on_engine_at(
        &about,
        template,
        &answers,
        &probe,
        &scratch,
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    )
    .expect("candidate run completed");
    std::fs::write(
        root.join("docs/evidence/picoshare-managed-candidate-2026-10-01.json"),
        format!("{}\n", result.to_json()),
    )
    .unwrap();
    assert!(
        result.passed,
        "PicoShare candidate failed: {:?}",
        result.failure()
    );
    assert!(local_store::offerings::offering("picoshare").is_none());
}

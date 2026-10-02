//! A withheld frozen-roster candidate is qualified without catalog promotion.
#[cfg(windows)]
#[test]
#[ignore = "serialized real managed-engine candidate run; explicit opt-in required"]
fn withheld_linkding_private_bookmark_survives_reinstall() {
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
    const TASK: &str = "an isolated required administrator account signs in while wrong credentials and anonymous access fail; an exact private bookmark URL, title, description, notes and two tags survive restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proposal = root.join("catalog/promotion-proposals/linkding-withheld.json");
    let reviewed: ReviewedTemplate =
        serde_json::from_slice(&std::fs::read(&proposal).unwrap()).unwrap();
    assert_eq!(reviewed.id, "linkding");
    assert!(!reviewed.offerable());
    assert!(local_store::offerings::offering("linkding").is_none());
    let template = reviewed
        .plan_template()
        .expect("exact reviewed candidate mapping");
    let field = template
        .fields
        .iter()
        .find(|field| field.key == "CAP_LD_SUPERUSER_PASSWORD")
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
    let state = scratch.join(format!("linkding-{nonce}.json"));
    std::fs::write(
        &state,
        serde_json::to_vec(
            &serde_json::json!({"password":password,"username":"local-store-proof"}),
        )
        .unwrap(),
    )
    .unwrap();
    struct RemoveState(PathBuf);
    impl Drop for RemoveState {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _remove = RemoveState(state.clone());
    let probe = ScriptProbe::new(root.join("scripts/linkding-bookmark-probe.mjs"), TASK)
        .with_args(vec![state.to_string_lossy().into_owned()]);
    let answers = BTreeMap::from([
        ("CAP_LD_SUPERUSER_PASSWORD".to_owned(), password),
        (
            "CAP_LD_SUPERUSER_NAME".to_owned(),
            "local-store-proof".to_owned(),
        ),
    ]);
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
        root.join("docs/evidence/linkding-managed-candidate-2026-10-02.json"),
        format!("{}\n", result.to_json()),
    )
    .unwrap();
    assert!(
        result.passed,
        "Linkding candidate failed: {:?}",
        result.failure()
    );
    assert!(local_store::offerings::offering("linkding").is_none());
}

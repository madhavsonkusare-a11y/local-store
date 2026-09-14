//! Compose parser proof only: no daemon, image pulls, mounts or WSL launch.
use local_store::{
    plan::{plan_for_recipe, PlanMount},
    runtime::{
        engine::wsl::project_plan, CommandSpec, ProcessRunner, SystemProcessRunner,
        DIAGNOSTIC_TIMEOUT,
    },
    setup::SeedFile,
};

#[test]
fn compose_preserves_projected_mounts_and_all_other_service_settings() {
    let runner = SystemProcessRunner;
    if runner
        .run(&CommandSpec::new(
            "docker",
            vec!["compose".into(), "version".into(), "--short".into()],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .is_err()
    {
        eprintln!("skipped: Docker Compose CLI unavailable");
        return;
    }
    let mut plan = plan_for_recipe(&local_store::recipes::recipe("memos").unwrap()).unwrap();
    let mut database = plan.services[0].clone();
    database.name = "database".into();
    database.published = None;
    database.mounts = vec![PlanMount::Volume {
        name: "database-data".into(),
        target: "/var/lib/db".into(),
        read_only: false,
    }];
    plan.services.push(database);
    plan.named_volumes.push("database-data".into());
    plan.services[0].depends_on.push("database".into());
    plan.services[0].mounts.push(PlanMount::Host {
        source: r"E:\Photos # ${HOME}".into(),
        target: "/photos".into(),
        read_only: true,
    });
    plan.services[0].mounts.push(PlanMount::Directory {
        source: "data/config.json".into(),
        target: "/etc/app.json".into(),
        read_only: true,
    });
    let projection = project_plan(
        &plan,
        r"D:\My Vault\应用\memos",
        &[SeedFile {
            path: "data/config.json".into(),
            content: "{}".into(),
        }],
    )
    .unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("wsl-projection-{}.yaml", std::process::id()));
    std::fs::write(&path, &projection.compose).unwrap();
    let output = runner
        .run(&CommandSpec::new(
            "docker",
            vec![
                "compose".into(),
                "-p".into(),
                "local-store-projection-proof".into(),
                "-f".into(),
                path.to_str().unwrap().into(),
                "config".into(),
                "--format".into(),
                "json".into(),
            ],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        output.success && !output.truncated,
        "Compose rejected projection: {}",
        output.stderr
    );
    let parsed: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    let service = &parsed["services"]["memos"];
    assert_eq!(
        service["volumes"][0]["source"],
        "/mnt/d/My Vault/应用/memos/data"
    );
    // `config` re-escapes dollars when exporting its already-interpolated model.
    // docker/compose cmd/compose/config.go runConfig performs this deliberately.
    assert_eq!(service["volumes"][1]["source"], "/mnt/e/Photos # $${HOME}");
    assert_eq!(service["volumes"][1]["read_only"], true);
    assert_eq!(
        service["volumes"][2]["source"],
        "/mnt/d/My Vault/应用/memos/data/config.json"
    );
    assert_eq!(service["volumes"][2]["target"], "/etc/app.json");
    // Compose may omit false fields in its normalized JSON; omission is false.
    assert_ne!(service["volumes"][2]["bind"]["create_host_path"], true);
    assert_eq!(
        parsed["services"]["database"]["volumes"][0]["type"],
        "volume"
    );
    assert_eq!(
        parsed["services"]["database"]["volumes"][0]["source"],
        "database-data"
    );
    assert_eq!(
        service["depends_on"]["database"]["condition"],
        "service_started"
    );
    assert_eq!(service["ports"][0]["host_ip"], "127.0.0.1");
    assert_eq!(parsed["services"].as_object().unwrap().len(), 2);
}

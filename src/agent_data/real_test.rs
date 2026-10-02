//! Real read-only note-file proof; never uses owner app credentials.
use super::*;
use crate::{
    qualification::{Bystanders, FirstUse, Isolation, Phase, ScriptProbe},
    runtime::{engine::EngineRunner, SystemProcessRunner},
};
use std::{collections::BTreeMap, ffi::OsString};
struct Restore(Vec<(&'static str, Option<OsString>)>);
impl Drop for Restore {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
struct Cleanup;
impl Cleanup {
    fn run(&self) -> AppResult<()> {
        if storage::managed_apps_root()
            .join("flatnotes/compose.yaml")
            .is_file()
        {
            storage::remove_installed_app("flatnotes")?;
            crate::recovery::discard("flatnotes", true)?;
        }
        Ok(())
    }
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.run();
    }
}
#[test]
#[ignore = "real file provider; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn flatnotes_exact_scoped_file_read_and_revocation() {
    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_MANAGED_QUALIFICATION").as_deref(),
        Ok("1")
    );
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = repo.join(".cache/managed-qualification");
    fs::create_dir_all(&scratch).unwrap();
    let slot = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(scratch.join("qualification.lock"))
        .unwrap();
    fs4::FileExt::try_lock(&slot).expect("another proof is active");
    let binding = EngineBinding::discover(&SystemProcessRunner).expect("selected owned engine");
    assert!(binding.is_wsl());
    let runner = EngineRunner {
        inner: &SystemProcessRunner,
        binding: Some(binding.clone()),
    };
    let bystanders = Bystanders::note(&runner).unwrap();
    for (kind, list) in [("container", "-aq"), ("network", "-q"), ("volume", "-q")] {
        let out = runner
            .run(&runtime::CommandSpec::new(
                "docker",
                vec![
                    kind.into(),
                    "ls".into(),
                    list.into(),
                    "--filter".into(),
                    "label=com.docker.compose.project=local-store-flatnotes".into(),
                ],
                None,
                runtime::DIAGNOSTIC_TIMEOUT,
            ))
            .unwrap();
        assert!(
            out.success && !out.truncated && out.stdout.trim().is_empty(),
            "existing Flatnotes resources refused"
        );
    }
    let native = storage::managed_engine_state_root();
    let records = ["bootstrap.json", "ownership-token", "selected-engine.json"]
        .map(|name| (name, fs::read(native.join(name)).unwrap()));
    let isolation = Isolation::new("flatnotes-agent-data", &scratch).unwrap();
    let _restore = Restore(
        ["APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    isolation.take_over_config_root();
    std::env::set_var("LOCALAPPDATA", &isolation.root);
    fs::create_dir_all(storage::managed_engine_state_root()).unwrap();
    for (name, bytes) in &records {
        fs::write(storage::managed_engine_state_root().join(name), bytes).unwrap();
    }
    let cleanup = Cleanup;
    let mut steps = Vec::new();
    let result = (|| -> AppResult<()> {
        let listen = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listen.local_addr()?.port();
        drop(listen);
        let template = crate::offerings::offering("flatnotes")
            .unwrap()
            .plan_template(Some(port))?;
        let password = format!("fixture-note-{}", isolation.run_id);
        let answers = BTreeMap::from([
            ("FLATNOTES_AUTH_TYPE".into(), "password".into()),
            ("FLATNOTES_USERNAME".into(), "fixture-notes".into()),
            ("FLATNOTES_PASSWORD".into(), password.clone()),
        ]);
        let app = runtime::install_template_on_engine(&template, "flatnotes", &answers, &binding)?;
        steps.push("isolated reviewed Flatnotes installed with typed synthetic credentials");
        let state = isolation.root.join("private-note-proof.json");
        let probe = ScriptProbe::new(
            repo.join("scripts/content-roundtrip-probe.mjs"),
            "private API note for exact file access",
        )
        .with_args(vec![
            state.to_string_lossy().into(),
            "flatnotes".into(),
            "fixture-notes".into(),
            password,
        ]);
        probe
            .exercise(Phase::FirstInstall, &app.launch_url)
            .map_err(AppError::invalid)?;
        let private: Value =
            serde_json::from_slice(&fs::read(&state)?).map_err(AppError::internal)?;
        let name = format!("{}.md", private["item"].as_str().ok_or_else(denied)?);
        let args = json!({"app_id":"flatnotes","name":name});
        let gateway = AgentGateway::open_local()?;
        let bearer = gateway.enroll_client_for_owner("file-fixture")?;
        let other = gateway.enroll_client_for_owner("other-file-fixture")?;
        assert!(call(&bearer, "local_store_flatnotes_read_file", &args).is_err());
        assert!(grant_for_owner("file-fixture", "flatnotes", 1, false).is_err());
        grant_for_owner("file-fixture", "flatnotes", 1, true)?;
        let note = call(&bearer, "local_store_flatnotes_read_file", &args)?;
        assert_eq!(note["content"], private["content"]);
        assert_eq!(note["untrusted_content"], true);
        steps.push("no implicit access; explicit exact read grant returns same private app note");
        let listing = call(
            &bearer,
            "local_store_flatnotes_files",
            &json!({"app_id":"flatnotes"}),
        )?;
        assert!(listing["notes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item.as_str().is_some_and(file_name)));
        assert!(listing.to_string().contains(&name));
        for bad in [
            "../credentials.json",
            ".flatnotes_secret_key",
            ".flatnotes_totp_key",
            "CON.md",
        ] {
            assert!(call(
                &bearer,
                "local_store_flatnotes_read_file",
                &json!({"app_id":"flatnotes","name":bad})
            )
            .is_err());
        }
        assert!(call(&other, "local_store_flatnotes_read_file", &args).is_err());
        assert!(call(&bearer, "local_store_flatnotes_write_file", &args).is_err());
        assert!(call(
            &bearer,
            "local_store_flatnotes_read_file",
            &json!({"app_id":"memos","name":name})
        )
        .is_err());
        steps.push("other clients, app IDs, credentials, traversal, devices and all writes denied");
        runtime::stop(&app)?;
        runtime::start(&app)?;
        assert_eq!(
            call(&bearer, "local_store_flatnotes_read_file", &args)?["content"],
            private["content"]
        );
        steps.push("exact private note remains readable after managed restart");
        gateway.revoke_status_for_owner("file-fixture", "flatnotes")?;
        assert!(call(&bearer, "local_store_flatnotes_read_file", &args).is_err());
        let audit = serde_json::to_string(&gateway.snapshot_for_owner()?.audit)
            .map_err(AppError::internal)?;
        assert!(!audit.contains(&bearer) && !audit.contains(private["content"].as_str().unwrap()));
        steps.push("live revocation denies reads and durable audit contains no note or credential");
        fs::remove_file(state)?;
        Ok(())
    })();
    let clean = cleanup.run();
    let unchanged = bystanders.survived(&runner);
    let native_unchanged = records
        .iter()
        .all(|(name, bytes)| fs::read(native.join(name)).is_ok_and(|current| current == *bytes));
    let passed = result.is_ok() && clean.is_ok() && unchanged.is_ok() && native_unchanged;
    let receipt = json!({"schema_version":1,"proof":"flatnotes-scoped-markdown-files","passed":passed,"recorded_at_unix":now().unwrap(),"test_sha256":hash(include_bytes!("real_test.rs")),"provider_sha256":hash(include_bytes!("../agent_data.rs")),"steps":steps,"owned_cleanup":clean.is_ok(),"bystanders_preserved":unchanged.is_ok(),"native_engine_files_unchanged":native_unchanged,"failure":result.err().map(|e|e.code),"limits":["read-only ordinary Markdown notes; no credentials, DB, other folders or writes","gateway grants do not sandbox same-Windows-user shell access","does not certify native WebView or clean-machine setup"]});
    fs::write(
        repo.join("docs/evidence/flatnotes-agent-files-2026-10-02.json"),
        format!("{}\n", serde_json::to_string_pretty(&receipt).unwrap()),
    )
    .unwrap();
    assert!(
        passed,
        "real file access proof failed; inspect redacted receipt"
    );
}

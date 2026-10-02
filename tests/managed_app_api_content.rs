//! Opt-in serial proofs for typed read APIs; synthetic owner accounts only.
//! Each run uses its own app ID, configuration, media and protected token store.
#![cfg(windows)]
use local_store::{
    agent_content::{self, AgentContent},
    agent_gateway::AgentGateway,
    model::{InstalledApp, RuntimeSpec},
    qualification::{Bystanders, FirstUse, Isolation, Phase, ScriptProbe},
    runtime::{
        self,
        engine::{EngineBinding, EngineRunner},
        SystemProcessRunner,
    },
    storage,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Environment {
    appdata: Option<OsString>,
    xdg: Option<OsString>,
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, value) in [("APPDATA", &self.appdata), ("XDG_CONFIG_HOME", &self.xdg)] {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
struct Cleanup {
    root: PathBuf,
    app: Option<InstalledApp>,
    done: bool,
    attempted: bool,
}
impl Cleanup {
    fn run(&mut self) -> Result<(), String> {
        if self.done {
            return Ok(());
        }
        if self.attempted {
            return Err("uncertain fixture cleanup retained for explicit recovery".into());
        }
        self.attempted = true;
        if let Some(app) = &self.app {
            let RuntimeSpec::Compose { project_dir, .. } = &app.runtime else {
                return Err("fixture is not a managed app".into());
            };
            if !app.id.contains("-qualify-")
                || project_dir != &storage::managed_apps_root().join(&app.id)
                || !project_dir.starts_with(&self.root)
            {
                return Err("fixture ownership root changed".into());
            }
            runtime::uninstall_and_remove(app, true).map_err(|e| e.message)?;
        } else if self.root.join("local-store/apps").is_dir() {
            return Err("uncommitted fixture setup retained for explicit engine inventory".into());
        }
        // Resolve the final target before recursive Windows fixture deletion.
        // It must remain inside the exact qualification workspace and match
        // the originally created non-symlink private directory.
        let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".cache/managed-qualification")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let resolved = self.root.canonicalize().map_err(|e| e.to_string())?;
        if !resolved.starts_with(&scratch)
            || resolved == scratch
            || self.root.file_name() != resolved.file_name()
        {
            return Err("fixture deletion target changed".into());
        }
        fs::remove_dir_all(&resolved).map_err(|e| e.to_string())?;
        self.done = true;
        Ok(())
    }
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.run();
    }
}
fn tone(path: &Path) -> Result<(), String> {
    let samples = (0..8000)
        .map(|i| (((i as f64 * std::f64::consts::TAU * 440.0 / 8000.0).sin()) * 8000.0) as i16)
        .collect::<Vec<_>>();
    let length = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + length as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8000u32.to_le_bytes());
    bytes.extend_from_slice(&16000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&length.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).map_err(|e| e.to_string())
}
fn prove(id: &str, tool: &str) {
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
    fs4::FileExt::try_lock(&slot)
        .expect("another proof is active; run these tests with --test-threads=1");
    let binding = EngineBinding::managed_wsl();
    assert_eq!(
        EngineBinding::discover(&SystemProcessRunner).unwrap(),
        binding
    );
    let runner = EngineRunner {
        inner: &SystemProcessRunner,
        binding: Some(binding.clone()),
    };
    let bystanders = Bystanders::note(&runner).unwrap();
    let isolation = Isolation::new(&format!("agent-api-{id}"), &scratch).unwrap();
    let _environment = Environment {
        appdata: std::env::var_os("APPDATA"),
        xdg: std::env::var_os("XDG_CONFIG_HOME"),
    };
    isolation.take_over_config_root();
    let mut cleanup = Cleanup {
        root: isolation.root.clone(),
        app: None,
        done: false,
        attempted: false,
    };
    let mut steps = Vec::new();
    let result = (|| -> Result<(), String> {
        storage::save_registry_v2(&storage::RegistryV2::new(Vec::new()))
            .map_err(|e| e.to_string())?;
        isolation.registry_is_empty()?;
        let media = isolation.root.join("owned-media");
        fs::create_dir_all(&media).map_err(|e| e.to_string())?;
        let mut answers = BTreeMap::new();
        if id == "immich" {
            answers.insert(
                "FOLDER_IMAGES_IMMICH".into(),
                media.to_string_lossy().into_owned(),
            );
        }
        if id == "jellyfin" {
            answers.insert("FOLDER_MEDIA".into(), media.to_string_lossy().into_owned());
            answers.insert("TZ".into(), "Asia/Kolkata".into());
            tone(&media.join("Local Store Proof Tone.wav"))?;
        }
        let mut template = local_store::offerings::offering(id)
            .ok_or("reviewed offering unavailable")?
            .plan_template(None)
            .map_err(|e| e.message)?;
        template.plan.id = isolation.run_id.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);
        template.plan.set_published_host(port);
        let mut app = runtime::install_template(&template, "Synthetic API content proof", &answers)
            .map_err(|e| e.message)?;
        cleanup.app = Some(app.clone());
        app.catalog_id = Some(id.into());
        storage::update_installed_app(app.clone()).map_err(|e| e.to_string())?;
        cleanup.app = Some(app.clone());
        runtime::wait_for_health(&app.launch_url, Duration::from_secs(180))
            .map_err(|e| e.message)?;
        steps.push("reviewed pinned app installed under a unique private project and app ID on the selected owned engine");
        let state_path = isolation.root.join("private-first-use.json");
        let token_path = isolation.root.join("private-api-token.json");
        let script = match id {
            "gitea" => "gitea-repository-probe.mjs",
            "wordpress" => "wordpress-post-probe.mjs",
            "kanboard" => "kanboard-task-probe.mjs",
            "immich" => "immich-photo-probe.mjs",
            "jellyfin" => "jellyfin-media-probe.mjs",
            "uptime-kuma" => "uptime-kuma-monitor-probe.mjs",
            _ => unreachable!(),
        };
        let mut arguments = vec![state_path.to_string_lossy().into_owned()];
        if id == "jellyfin" {
            arguments.push(
                media
                    .join("Local Store Proof Tone.wav")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        ScriptProbe::new(
            repo.join("scripts").join(script),
            "synthetic owner content for scoped read proof",
        )
        .with_args(arguments)
        .exercise(Phase::FirstInstall, &app.launch_url)?;
        let created = std::process::Command::new("node")
            .arg(repo.join("scripts/app-api-read-fixture.mjs"))
            .arg(id)
            .arg(&app.launch_url)
            .arg(&state_path)
            .arg(&token_path)
            .output()
            .map_err(|_| "synthetic API fixture could not start")?;
        if !created.status.success() {
            // Preserve useful fixed-stage diagnostics before cleanup without
            // copying arbitrary upstream text, source excerpts or credentials.
            let error = String::from_utf8_lossy(&created.stderr);
            for line in error
                .lines()
                .filter_map(|line| line.strip_prefix("Error: "))
            {
                if let Some(status) = line
                    .strip_prefix(&format!("{id} fixture endpoint refused request (HTTP "))
                    .and_then(|rest| rest.strip_suffix(')'))
                    .and_then(|value| value.parse::<u16>().ok())
                    .filter(|value| (100..=599).contains(value))
                {
                    return Err(format!("synthetic {id} fixture endpoint refused request (HTTP {status}); private output withheld"));
                }
                if [
                    "synthetic private WordPress post was refused",
                    "synthetic Kanboard RPC was refused",
                    "synthetic Kanboard user creation failed",
                    "synthetic Kanboard private content creation failed",
                    "Gitea did not issue the scoped fixture token",
                    "Immich did not issue the scoped fixture key",
                    "synthetic Immich login failed",
                    "synthetic Jellyfin login failed",
                ]
                .contains(&line)
                {
                    return Err(format!("{line}; private output withheld"));
                }
            }
            return Err("synthetic API credential/content preparation failed; private subprocess output withheld".into());
        }
        let state: Value =
            serde_json::from_slice(&fs::read(&token_path).map_err(|e| e.to_string())?)
                .map_err(|_| "invalid private API fixture state")?;
        let token = state["token"].as_str().ok_or("fixture token missing")?;
        let gateway = AgentGateway::open(
            &isolation.root.join("agent-policy"),
            &isolation.root.join("agent-auth"),
        )
        .map_err(|e| e.message)?;
        let content =
            AgentContent::open_at(&isolation.root.join("protected-content"), gateway.clone());
        let bearer = gateway
            .enroll_client_for_owner("fixture-agent")
            .map_err(|e| e.message)?;
        let args = json!({"app_id":app.id});
        assert!(content.call_reviewed(&bearer, tool, &args).is_err());
        assert!(content.connect_for_owner(&app.id, token, false).is_err());
        content
            .connect_for_owner(&app.id, token, true)
            .map_err(|e| e.message)?;
        let protected = fs::read(isolation.root.join("protected-content/content-v1.dpapi"))
            .map_err(|e| e.to_string())?;
        assert!(!protected
            .windows(token.len())
            .any(|window| window == token.as_bytes()));
        assert!(content.call_reviewed(&bearer, tool, &args).is_err());
        assert!(content
            .grant_for_owner("fixture-agent", &app.id, 1, true, true)
            .is_err());
        content
            .grant_for_owner("fixture-agent", &app.id, 1, false, true)
            .map_err(|e| e.message)?;
        let value = content
            .call_reviewed(&bearer, tool, &args)
            .map_err(|e| e.message)?;
        let marker = state["marker"].as_str().ok_or("fixture marker missing")?;
        assert!(
            value.to_string().contains(marker),
            "exact synthetic private content missing from typed read"
        );
        if id == "gitea" {
            assert!(value["repositories"]
                .as_array()
                .unwrap()
                .iter()
                .any(|repo| repo["name"] == state["marker"] && repo["private"] == true));
        }
        if id == "wordpress" {
            assert!(value["posts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|post| post["status"] == "private"
                    && post["content"] == state["private_content"]));
        }
        if id == "uptime-kuma" {
            assert!(value["monitors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|monitor| monitor["name"] == state["marker"] && monitor["status"] == "up"));
            let projected = value.to_string();
            assert!(
                !projected.contains("127.0.0.1")
                    && !projected.contains("monitor_url")
                    && !projected.contains("monitor_hostname")
                    && !projected.contains("monitor_port")
            );
        }
        let other = gateway
            .enroll_client_for_owner("other-agent")
            .map_err(|e| e.message)?;
        assert!(content.call_reviewed(&other, tool, &args).is_err());
        assert!(content
            .call_reviewed(
                &bearer,
                tool,
                &json!({"app_id":app.id,"url":"http://evil/","method":"DELETE"})
            )
            .is_err());
        assert!(content
            .call_reviewed(&bearer, tool, &json!({"app_id":"foreign-app"}))
            .is_err());
        assert!(content
            .call_reviewed(
                &bearer,
                "local_store_agent_grant",
                &json!({"app_id":app.id,"client_id":"fixture-agent","write":true})
            )
            .is_err());
        if id == "wordpress" {
            steps.push("private malicious instructions remain exact returned text; they cannot reveal another app credential, create a grant, request writes or select a foreign URL");
        }
        steps.push("explicit protected credential consent; no implicit grants; exact private summary read; cross-client, arbitrary URL/method and write-grant requests denied");
        gateway
            .revoke_status_for_owner("fixture-agent", &app.id)
            .map_err(|e| e.message)?;
        assert!(content.call_reviewed(&bearer, tool, &args).is_err());
        content
            .grant_for_owner("fixture-agent", &app.id, 1, false, true)
            .map_err(|e| e.message)?;
        content
            .connect_for_owner(&app.id, token, true)
            .map_err(|e| e.message)?;
        assert!(
            content.call_reviewed(&bearer, tool, &args).is_err(),
            "replaced credential must require owner re-grant"
        );
        content
            .grant_for_owner("fixture-agent", &app.id, 1, false, true)
            .map_err(|e| e.message)?;
        assert!(content
            .call_reviewed(&bearer, tool, &args)
            .map_err(|e| e.message)?
            .to_string()
            .contains(marker));
        let audit =
            serde_json::to_string(&gateway.snapshot_for_owner().map_err(|e| e.message)?.audit)
                .unwrap();
        assert!(!audit.contains(token) && !audit.contains(&bearer) && !audit.contains(marker));
        content
            .disconnect_for_owner(&app.id, true)
            .map_err(|e| e.message)?;
        assert!(content.call_reviewed(&bearer, tool, &args).is_err());
        content
            .connect_for_owner(&app.id, token, true)
            .map_err(|e| e.message)?;
        assert!(
            content.call_reviewed(&bearer, tool, &args).is_err(),
            "disconnect and reconnect must not revive an earlier app grant"
        );
        steps.push("live grant revocation, credential replacement requiring a new grant, owner disconnect/reconnect without grant revival and secret-free audit verified");
        Ok(())
    })();
    let cleanup_result = cleanup.run();
    let bystander_result = bystanders.survived(&runner);
    let passed = result.is_ok() && cleanup_result.is_ok() && bystander_result.is_ok();
    if cleanup_result.is_ok() && bystander_result.is_ok() {
        steps.push(
            "exact fixture cleaned including generated credentials; existing containers unchanged",
        );
    }
    let proof = json!({"schema_version":1,"recorded_at_unix":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),"passed":passed,"app":id,"provider":agent_content::app_api::provider(id),"engine":binding,"scope":"real reviewed read-only API summary path; no app writes, generic APIs, native WebView or full app functionality claim","test_sha256":format!("{:x}",Sha256::digest(include_bytes!("managed_app_api_content.rs"))),"provider_sha256":format!("{:x}",Sha256::digest(include_bytes!("../src/agent_content/app_api.rs"))),"content_boundary_sha256":format!("{:x}",Sha256::digest(include_bytes!("../src/agent_content.rs"))),"fixture_sha256":format!("{:x}",Sha256::digest(include_bytes!("../scripts/app-api-read-fixture.mjs"))),"steps":steps,"failure":result.as_ref().err(),"cleanup_passed":cleanup_result.is_ok(),"bystanders_unchanged":bystander_result.is_ok()});
    fs::write(
        repo.join(format!(
            "docs/evidence/{id}-agent-api-content-2026-10-02.json"
        )),
        format!("{}\n", serde_json::to_string_pretty(&proof).unwrap()),
    )
    .unwrap();
    assert!(
        passed,
        "{id} API content proof failed: {:?}; cleanup: {:?}; bystanders: {:?}",
        result, cleanup_result, bystander_result
    );
}
#[test]
#[ignore = "real reviewed app API; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serial"]
fn gitea_scoped_api_reads() {
    prove("gitea", "local_store_gitea_repositories");
}
#[test]
#[ignore = "real reviewed app API; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serial"]
fn wordpress_scoped_api_reads() {
    prove("wordpress", "local_store_wordpress_posts");
}
#[test]
#[ignore = "real reviewed app API; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serial"]
fn kanboard_scoped_api_reads() {
    prove("kanboard", "local_store_kanboard_dashboard");
}
#[test]
#[ignore = "real reviewed app API; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serial"]
fn immich_scoped_api_reads() {
    prove("immich", "local_store_immich_assets");
}
#[test]
#[ignore = "real reviewed app API; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serial"]
fn jellyfin_scoped_api_reads() {
    prove("jellyfin", "local_store_jellyfin_library");
}
#[test]
#[ignore = "real reviewed app API; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serial"]
fn uptime_kuma_scoped_api_reads() {
    prove("uptime-kuma", "local_store_uptime_kuma_monitors");
}

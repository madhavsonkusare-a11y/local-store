//! Opt-in end-to-end agent lifecycle smoke on the owned managed WSL engine.
//! This uses a qualification-private registry, policy, credential store, and app.

#[cfg(all(windows, feature = "mcp-sidecar"))]
#[test]
#[ignore = "real managed WSL lifecycle; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn owner_granted_mcp_stop_start_and_revocation_on_memos() {
    use local_store::{
        qualification::{qualify_on_engine_at, FirstUse, Phase},
        runtime::engine::EngineBinding,
    };
    use std::{collections::BTreeMap, path::PathBuf};

    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_MANAGED_QUALIFICATION").as_deref(),
        Ok("1")
    );
    struct AgentLifecycle;
    impl FirstUse for AgentLifecycle {
        fn describes(&self) -> &str {
            "an enrolled MCP client with a short owner grant can stop and start its installed app, while a revoked grant cannot"
        }
        fn fingerprint_material(&self) -> Vec<u8> {
            include_bytes!("managed_agent_lifecycle.rs").to_vec()
        }
        fn exercise(&self, phase: Phase, address: &str) -> Result<(), String> {
            if phase == Phase::FirstInstall {
                exercise_agent_lifecycle(address)?;
            }
            Ok(())
        }
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let evidence = qualify_on_engine_at(
        "memos",
        &BTreeMap::new(),
        &AgentLifecycle as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    )
    .expect("managed lifecycle smoke could run");
    let output = root.join("docs/evidence/memos-agent-lifecycle-2026-09-30.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "agent lifecycle failed: {:?}",
        evidence.failure()
    );
}

#[cfg(all(windows, feature = "mcp-sidecar"))]
fn exercise_agent_lifecycle(address: &str) -> Result<(), String> {
    use local_store::{agent_gateway::AgentGateway, agent_policy::AgentPolicyStore, storage};
    use std::{path::PathBuf, time::Duration};

    let registry = storage::load_or_migrate_registry().map_err(|error| error.to_string())?;
    let app = registry
        .apps
        .iter()
        .find(|app| app.launch_url == address)
        .ok_or("qualification app is not installed")?;
    let app_id = app.id.as_str();
    let private_root = PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is absent")?);
    let local_root = private_root.join("local-store");
    let policy_root = local_root.join("agent-policy");
    let credentials_root = local_root.join("agent-auth");
    let gateway =
        AgentGateway::open(&policy_root, &credentials_root).map_err(|error| error.message)?;
    let secret = gateway
        .enroll_client_for_owner("qualification-client")
        .map_err(|error| error.message)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    gateway
        .grant_lifecycle_for_owner("qualification-client", app_id, 1, now)
        .map_err(|error| error.message)?;

    let call = |tool| mcp_call(&private_root, &secret, tool, app_id);
    assert_status(&call("local_store_get_status")?, "running")?;
    assert_success(&call("local_store_stop_app")?)?;
    assert_status(&call("local_store_get_status")?, "stopped")?;
    // Starting is attempted even when an earlier assertion fails: a stopped
    // qualification app must not be left behind for the outer cleanup to fight.
    let started = call("local_store_start_app")?;
    assert_success(&started)?;
    local_store::qualification::answered(
        &local_store::runtime::HttpHealthProbe,
        address,
        Duration::from_secs(120),
    )?;
    gateway
        .revoke_status_for_owner("qualification-client", app_id)
        .map_err(|error| error.message)?;
    if call("local_store_stop_app")?["result"]["isError"] != true {
        return Err("revoked lifecycle grant still stopped the app".into());
    }
    let audit = AgentPolicyStore::open(&policy_root).map_err(|error| error.message)?;
    if audit.audit().len() != 5 || audit.audit().iter().filter(|event| event.allowed).count() != 4 {
        return Err("lifecycle audit did not record four grants and one denial".into());
    }
    if serde_json::to_string(audit.audit())
        .map_err(|error| error.to_string())?
        .contains(&secret)
    {
        return Err("agent secret leaked into audit".into());
    }
    Ok(())
}

#[cfg(all(windows, feature = "mcp-sidecar"))]
fn mcp_call(
    root: &std::path::Path,
    secret: &str,
    tool: &str,
    app_id: &str,
) -> Result<serde_json::Value, String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_local-store-mcp"))
        .env("LOCALAPPDATA", root)
        .env("LOCAL_STORE_AGENT_BEARER", secret)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let init = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
    let call = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name":tool,"arguments":{"app_id":app_id}}});
    {
        let stdin = child.stdin.as_mut().ok_or("MCP stdin unavailable")?;
        writeln!(stdin, "{init}").map_err(|error| error.to_string())?;
        writeln!(stdin, "{call}").map_err(|error| error.to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "MCP sidecar exited unsuccessfully: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let lines = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 2 {
        return Err(format!(
            "MCP sidecar returned {} responses, expected 2",
            lines.len()
        ));
    }
    serde_json::from_slice(lines[1]).map_err(|error| error.to_string())
}

#[cfg(all(windows, feature = "mcp-sidecar"))]
fn assert_success(response: &serde_json::Value) -> Result<(), String> {
    if response["result"]["isError"] == false {
        Ok(())
    } else {
        Err(format!(
            "MCP tool failed: {}",
            response["result"]["content"]
        ))
    }
}

#[cfg(all(windows, feature = "mcp-sidecar"))]
fn assert_status(response: &serde_json::Value, expected: &str) -> Result<(), String> {
    assert_success(response)?;
    if response["result"]["structuredContent"]["status"] == expected {
        Ok(())
    } else {
        Err(format!(
            "expected {expected} app status; received {}",
            response["result"]["structuredContent"]
        ))
    }
}

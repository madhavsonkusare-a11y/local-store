//! Opt-in proof of real approved MCP mutations on the selected owned engine.
//! Never run beside another qualification; all app, auth and queue files use
//! one private profile. Native engine ownership records are copied, not changed.
#![cfg(all(windows, feature = "mcp-sidecar"))]

use local_store::{
    agent_gateway::AgentGateway,
    agent_policy::{AgentAction, AgentPolicyStore},
    agent_requests::{AgentRequests, MutationRequest, MutationState},
    qualification::{FirstUse, Isolation, Phase, ScriptProbe, StepResult},
    recovery,
    runtime::{
        self,
        engine::{EngineBinding, EngineRunner},
        CommandSpec, ProcessRunner, SystemProcessRunner, DIAGNOSTIC_TIMEOUT,
    },
    storage,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PROJECT: &str = "local-store-memos";
const CLIENT: &str = "mutation-qualification-client";
const ENGINE_FILES: [&str; 4] = [
    "bootstrap.json",
    "ownership-token",
    "selected-engine.json",
    "engine-selection-configured",
];

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
struct RestoreConfig(Vec<(&'static str, Option<OsString>)>);
impl RestoreConfig {
    fn capture() -> Self {
        Self(
            ["APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME"]
                .into_iter()
                .map(|k| (k, std::env::var_os(k)))
                .collect(),
        )
    }
}
impl Drop for RestoreConfig {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

fn query(runner: &dyn ProcessRunner, args: Vec<String>) -> Result<String, String> {
    let output = runner
        .run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT))
        .map_err(|_| "engine inspection could not run".to_owned())?;
    if !output.success || output.truncated {
        return Err("engine inspection failed or was incomplete".into());
    }
    Ok(output.stdout)
}
fn refuse_collision(runner: &dyn ProcessRunner) -> Result<(), String> {
    for (kind, list) in [("container", "-aq"), ("network", "-q"), ("volume", "-q")] {
        let found = query(
            runner,
            vec![
                kind.into(),
                "ls".into(),
                list.into(),
                "--filter".into(),
                format!("label=com.docker.compose.project={PROJECT}"),
            ],
        )?;
        if !found.trim().is_empty() {
            return Err("existing Memos resources must not be touched by this proof".into());
        }
    }
    if !query(
        runner,
        vec![
            "container".into(),
            "ls".into(),
            "-aq".into(),
            "--filter".into(),
            format!("name={PROJECT}"),
        ],
    )?
    .trim()
    .is_empty()
    {
        return Err("Memos container name already exists; proof refused".into());
    }
    Ok(())
}
fn container_states(runner: &dyn ProcessRunner) -> Result<BTreeMap<String, String>, String> {
    let text = query(
        runner,
        vec![
            "container".into(),
            "ls".into(),
            "--all".into(),
            "--no-trunc".into(),
            "--format".into(),
            "{{.ID}}\t{{.State}}".into(),
        ],
    )?;
    let mut states = BTreeMap::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let (id, state) = line.split_once('\t').ok_or("invalid container inventory")?;
        if id.len() != 64
            || !id.bytes().all(|b| b.is_ascii_hexdigit())
            || state.is_empty()
            || states.insert(id.into(), state.into()).is_some()
        {
            return Err("ambiguous container inventory".into());
        }
    }
    Ok(states)
}
fn data_hashes(root: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, String>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|_| "fixture data is unavailable")? {
            let path = entry.map_err(|_| "fixture data could not be read")?.path();
            let meta =
                fs::symlink_metadata(&path).map_err(|_| "fixture data metadata is unavailable")?;
            if meta.is_symlink() {
                return Err("fixture data contains a link".into());
            }
            if meta.is_dir() {
                walk(root, &path, out)?;
            } else if meta.is_file() {
                out.insert(
                    path.strip_prefix(root)
                        .map_err(|_| "data escaped fixture")?
                        .to_path_buf(),
                    digest(&fs::read(path).map_err(|_| "fixture file could not be read")?),
                );
            } else {
                return Err("fixture data contains a non-file object".into());
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    if out.is_empty() {
        return Err("Memos produced no retained data".into());
    }
    Ok(out)
}
fn record<T>(
    steps: &mut Vec<StepResult>,
    label: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let result = work();
    steps.push(StepResult {
        step: label.into(),
        passed: result.is_ok(),
        detail: result.as_ref().err().map(|e| runtime::redact(e)),
    });
    result
}
fn installed() -> Result<local_store::model::InstalledApp, String> {
    let apps = storage::load_or_migrate_registry()
        .map_err(|_| "fixture registry could not be read")?
        .apps;
    if apps.len() != 1 || apps[0].id != "memos" {
        return Err("expected exactly one fixture Memos app".into());
    }
    Ok(apps[0].clone())
}

struct McpSession {
    child: Child,
    stdin: Option<ChildStdin>,
    responses: Receiver<Result<Value, String>>,
    id: u64,
}
impl McpSession {
    fn open(secret: &str) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_local-store-mcp"))
            .env("LOCAL_STORE_AGENT_BEARER", secret)
            .env("LOCAL_STORE_ENGINE_SUPERVISOR_PROCESS_ONLY", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "MCP connector could not start")?;
        let stdin = child.stdin.take().ok_or("MCP input is unavailable")?;
        let stdout = child.stdout.take().ok_or("MCP output is unavailable")?;
        let (send, responses) = mpsc::sync_channel(4);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if line.len() <= 64 * 1024 => {
                        let result = serde_json::from_str(&line)
                            .map_err(|_| "MCP response was invalid".into());
                        if send.send(result).is_err() {
                            break;
                        }
                    }
                    _ => {
                        let _ = send.send(Err(
                            "MCP response exceeded its bound or could not be read".into(),
                        ));
                        break;
                    }
                }
            }
        });
        let mut session = Self {
            child,
            stdin: Some(stdin),
            responses,
            id: 0,
        };
        let init = session.call("initialize", json!({}))?;
        if init.get("result").is_none() {
            return Err("MCP initialization failed".into());
        }
        Ok(session)
    }
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.id += 1;
        let id = self.id;
        let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        let stdin = self.stdin.as_mut().ok_or("MCP input was closed")?;
        writeln!(stdin, "{request}").map_err(|_| "MCP request could not be sent")?;
        stdin
            .flush()
            .map_err(|_| "MCP request could not be flushed")?;
        let response = self
            .responses
            .recv_timeout(Duration::from_secs(180))
            .map_err(|_| "MCP response timed out or connector exited")??;
        if response["id"] != id {
            return Err("MCP response identity did not match".into());
        }
        Ok(response)
    }
    fn tool(&mut self, name: &str, key: &str, value: &str) -> Result<Value, String> {
        self.call("tools/call", json!({"name":name,"arguments":{key:value}}))
    }
    fn request(&mut self, name: &str, key: &str, value: &str) -> Result<MutationRequest, String> {
        let started = Instant::now();
        let response = loop {
            let response = self.tool(name, key, value)?;
            if response["result"]["isError"] == true
                && response["result"]["structuredContent"]["error"]["retry_safe"] == true
                && started.elapsed() < Duration::from_secs(2)
            {
                // The server explicitly identifies only pre-mutation lock
                // contention as retry-safe. Ambiguous writes are never retried.
                thread::sleep(Duration::from_millis(25));
                continue;
            }
            break response;
        };
        if response["result"]["isError"] != false {
            // Only fixed local refusal messages enter the public receipt.
            // Never copy arbitrary transport or application response text.
            let text = response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("");
            let reason = [
                "Agent mutation access denied.",
                "Agent requests are being updated. Retry shortly.",
                "Agent credentials are in use by another process.",
                "Agent policy is in use by another process.",
                "Agent request queue contains an invalid record.",
                "The exact engine identity could not be verified.",
            ]
            .into_iter()
            .find(|reason| text == *reason)
            .unwrap_or("MCP mutation call was denied or failed");
            return Err(reason.into());
        }
        serde_json::from_value(response["result"]["structuredContent"].clone())
            .map_err(|_| "MCP mutation status was invalid".into())
    }
    fn expect_denied(&mut self, name: &str, key: &str, value: &str) -> Result<(), String> {
        if self.tool(name, key, value)?["result"]["isError"] == true {
            Ok(())
        } else {
            Err("MCP unexpectedly allowed a forbidden mutation".into())
        }
    }
    fn terminal(&mut self, id: &str) -> Result<MutationRequest, String> {
        let start = Instant::now();
        loop {
            if start.elapsed() > Duration::from_secs(20 * 60) {
                return Err("mutation did not finish within its bound".into());
            }
            let request = match self.request("local_store_request_status", "request_id", id) {
                Ok(request) => request,
                // The asynchronous worker briefly owns the same queue while
                // updating progress. Retry this read, never the mutation.
                Err(error)
                    if matches!(
                        error.as_str(),
                        "Agent requests are being updated. Retry shortly."
                            | "Agent credentials are in use by another process."
                            | "Agent policy is in use by another process."
                    ) =>
                {
                    thread::sleep(Duration::from_millis(250));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if request.state != MutationState::Running {
                return Ok(request);
            }
            thread::sleep(Duration::from_millis(250));
        }
    }
}
impl Drop for McpSession {
    fn drop(&mut self) {
        self.stdin.take(); // EOF cancels unfinished installs and awaits rollback.
        let start = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Err(_) => break,
                Ok(None) => {}
            }
            if start.elapsed() > Duration::from_secs(240) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct FixtureCleanup {
    root: PathBuf,
    active: bool,
}
impl FixtureCleanup {
    fn cleanup(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let expected = self.root.join("local-store/apps/memos");
        if storage::managed_apps_root().join("memos") != expected {
            return Err("fixture profile changed before cleanup".into());
        }
        if storage::load_or_migrate_registry()
            .map_err(|_| "fixture registry unavailable for cleanup")?
            .apps
            .iter()
            .any(|a| a.id == "memos")
        {
            storage::remove_installed_app("memos")
                .map_err(|_| "fixture registry cleanup failed")?;
        }
        if expected.join("compose.yaml").is_file() {
            recovery::discard("memos", true)
                .map_err(|_| "verified fixture recovery cleanup failed")?;
        }
        self.active = false;
        Ok(())
    }
}
impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            eprintln!("Agent mutation fixture cleanup needs owner review.");
        }
    }
}

#[test]
#[ignore = "real owned Windows engine; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1; run serially"]
fn approved_mcp_install_keep_data_uninstall_and_running_cancel() {
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
    fs4::FileExt::try_lock(&slot).expect("another qualification is active; retry once finished");
    let binding = EngineBinding::discover(&SystemProcessRunner)
        .expect("the native owned engine must already be explicitly selected and ready");
    assert!(
        binding.is_wsl(),
        "this proof never selects or uses Docker Desktop"
    );
    let runner = EngineRunner {
        inner: &SystemProcessRunner,
        binding: Some(binding.clone()),
    };
    refuse_collision(&runner).expect("fixed Memos fixture project must be unused");
    let before = container_states(&runner).expect("bystander inventory before mutation");
    let native_state = storage::managed_engine_state_root();
    let engine_records = ENGINE_FILES
        .into_iter()
        .map(|name| {
            let path = native_state.join(name);
            let meta = fs::metadata(&path).expect("native selection/ownership record exists");
            assert!(
                meta.is_file() && meta.len() <= 16 * 1024,
                "native engine record must be bounded"
            );
            (name, fs::read(path).expect("native engine record readable"))
        })
        .collect::<BTreeMap<_, _>>();
    let isolation = Isolation::new("agent-mutation", &scratch).unwrap();
    let _restore = RestoreConfig::capture();
    isolation.take_over_config_root();
    std::env::set_var("LOCALAPPDATA", &isolation.root);
    let fixture_state = storage::managed_engine_state_root();
    fs::create_dir_all(&fixture_state).unwrap();
    for (name, bytes) in &engine_records {
        fs::write(fixture_state.join(name), bytes).unwrap();
    }
    assert_eq!(
        EngineBinding::discover(&SystemProcessRunner).unwrap(),
        binding,
        "private profile must reference the same freshly verified owned engine"
    );
    let mut cleanup = FixtureCleanup {
        root: isolation.root.clone(),
        active: true,
    };
    let service = AgentRequests::open_local().unwrap();
    let gateway = AgentGateway::open_local().unwrap();
    let secret = gateway.enroll_client_for_owner(CLIENT).unwrap();
    let probe = ScriptProbe::new(
        repo.join("scripts/memos-content-probe.mjs"),
        "exact private memo survives approved agent reinstall",
    )
    .with_args(vec![isolation
        .root
        .join("private-memo-state.json")
        .to_string_lossy()
        .into_owned()]);
    let mut steps = Vec::new();
    let mut cancellation_stage = None;
    let result = (|| -> Result<(), String> {
        let mut mcp = McpSession::open(&secret)?;
        let request = record(
            &mut steps,
            "authenticated MCP creates pending exact Memos install",
            || mcp.request("local_store_request_install", "app_id", "memos"),
        )?;
        if request.state != MutationState::Pending {
            return Err("install was not pending owner approval".into());
        }
        record(
            &mut steps,
            "agent execution denied before owner approval",
            || mcp.expect_denied("local_store_execute_request", "request_id", &request.id),
        )?;
        record(
            &mut steps,
            "owner explicitly approves exact request",
            || {
                service
                    .decide_for_owner(&request.id, true, true)
                    .map(|_| ())
                    .map_err(|_| "owner approval failed".into())
            },
        )?;
        let running = mcp.request("local_store_execute_request", "request_id", &request.id)?;
        if running.state != MutationState::Running {
            return Err("approved install did not expose a running operation".into());
        }
        record(
            &mut steps,
            "approved MCP install finishes and commits app",
            || {
                if mcp.terminal(&request.id)?.state != MutationState::Succeeded {
                    return Err("approved installation failed".into());
                }
                installed().map(|_| ())
            },
        )?;
        record(
            &mut steps,
            "consumed install approval cannot be replayed",
            || mcp.expect_denied("local_store_execute_request", "request_id", &request.id),
        )?;
        gateway
            .grant_status_for_owner(CLIENT, "memos", 1, now())
            .map_err(|_| "owner status grant failed")?;
        record(
            &mut steps,
            "owner-granted MCP status observes actual running app",
            || {
                let response = mcp.tool("local_store_get_status", "app_id", "memos")?;
                if response["result"]["isError"] != false
                    || response["result"]["structuredContent"]["status"] != "running"
                {
                    return Err("granted MCP status was not running".into());
                }
                Ok(())
            },
        )?;
        let app = installed()?;
        record(
            &mut steps,
            "create exact private memo in real Memos",
            || probe.exercise(Phase::FirstInstall, &app.launch_url),
        )?;
        runtime::stop(&app)
            .map_err(|_| "owner could not quiesce fixture before retained-data check")?;
        let data = storage::managed_apps_root().join("memos/data");
        let saved = data_hashes(&data)?;
        let uninstall = mcp.request("local_store_request_uninstall", "app_id", "memos")?;
        mcp.expect_denied("local_store_execute_request", "request_id", &uninstall.id)?;
        service
            .decide_for_owner(&uninstall.id, true, true)
            .map_err(|_| "owner keep-data uninstall approval failed")?;
        mcp.request("local_store_execute_request", "request_id", &uninstall.id)?;
        record(
            &mut steps,
            "approved keep-data uninstall removes app and retains exact quiesced files",
            || {
                if mcp.terminal(&uninstall.id)?.state != MutationState::Succeeded {
                    return Err("approved keep-data uninstall failed".into());
                }
                if !storage::load_or_migrate_registry()
                    .map_err(|_| "fixture registry unavailable")?
                    .apps
                    .is_empty()
                    || data_hashes(&data)? != saved
                {
                    return Err(
                        "keep-data uninstall changed retained data or kept registry entry".into(),
                    );
                }
                Ok(())
            },
        )?;
        let second = mcp.request("local_store_request_install", "app_id", "memos")?;
        service
            .decide_for_owner(&second.id, true, true)
            .map_err(|_| "owner second install approval failed")?;
        let second = mcp.request("local_store_execute_request", "request_id", &second.id)?;
        if second.state != MutationState::Running {
            return Err("second approved install was not running".into());
        }
        // Inspect once without introducing a delay that could race the commit.
        // Null explicitly means engine verification precedes recipe progress.
        cancellation_stage = mcp
            .request("local_store_request_status", "request_id", &second.id)?
            .stage;
        let cancelled = mcp.request("local_store_cancel_request", "request_id", &second.id)?;
        if !cancelled.cancel_requested {
            return Err("second install crossed commit cutoff before cancellation; no cancellation proof claimed".into());
        }
        record(
            &mut steps,
            "authenticated cancellation stops actual running install and preserves prior data",
            || {
                if mcp.terminal(&second.id)?.state != MutationState::Cancelled {
                    return Err("running install did not finish cancelled".into());
                }
                if !storage::load_or_migrate_registry()
                    .map_err(|_| "fixture registry unavailable")?
                    .apps
                    .is_empty()
                {
                    return Err("cancelled reinstall committed an app".into());
                }
                // SQLite can legitimately change journal bytes during a
                // partially started reinstall. Retention is checked here;
                // the exact private memo is verified after fresh reinstall.
                data_hashes(&data)?;
                refuse_collision(&runner)
            },
        )?;
        record(
            &mut steps,
            "cancelled consumed approval cannot be replayed",
            || mcp.expect_denied("local_store_execute_request", "request_id", &second.id),
        )?;
        let third = mcp.request("local_store_request_install", "app_id", "memos")?;
        service
            .decide_for_owner(&third.id, true, true)
            .map_err(|_| "owner fresh reinstall approval failed")?;
        mcp.request("local_store_execute_request", "request_id", &third.id)?;
        if mcp.terminal(&third.id)?.state != MutationState::Succeeded {
            return Err("fresh approved reinstall failed".into());
        }
        record(
            &mut steps,
            "fresh approved reinstall reads the same private memo",
            || probe.exercise(Phase::AfterReinstall, &installed()?.launch_url),
        )?;
        gateway
            .revoke_status_for_owner(CLIENT, "memos")
            .map_err(|_| "owner grant revocation failed")?;
        record(
            &mut steps,
            "revoked app grant denies live MCP status and uninstall request",
            || {
                mcp.expect_denied("local_store_get_status", "app_id", "memos")?;
                mcp.expect_denied("local_store_request_uninstall", "app_id", "memos")
            },
        )?;
        gateway
            .revoke_client_for_owner(CLIENT)
            .map_err(|_| "owner client revocation failed")?;
        record(
            &mut steps,
            "revoked credential denies operation lookup in existing MCP session",
            || mcp.expect_denied("local_store_request_status", "request_id", &third.id),
        )?;
        record(
            &mut steps,
            "durable audit records allowed mutation dispatch and contains no bearer",
            || {
                let audit =
                    AgentPolicyStore::open(&isolation.root.join("local-store/agent-policy"))
                        .map_err(|_| "fixture audit could not be read")?;
                if !audit
                    .audit()
                    .iter()
                    .any(|e| e.allowed && e.action == AgentAction::Install)
                    || !audit
                        .audit()
                        .iter()
                        .any(|e| e.allowed && e.action == AgentAction::Uninstall)
                {
                    return Err("mutation dispatch audit missing".into());
                }
                if serde_json::to_string(audit.audit())
                    .map_err(|_| "audit encoding failed")?
                    .contains(&secret)
                {
                    return Err("bearer leaked into audit".into());
                }
                Ok(())
            },
        )?;
        Ok(())
    })();
    let clean = record(
        &mut steps,
        "cleanup only verified private Memos fixture",
        || cleanup.cleanup(),
    );
    let unchanged = record(
        &mut steps,
        "all pre-existing engine containers and states remain unchanged",
        || {
            if container_states(&runner)? != before {
                return Err("bystander container inventory changed".into());
            }
            refuse_collision(&runner)
        },
    );
    let native_unchanged = record(
        &mut steps,
        "native engine ownership and selection files unchanged",
        || {
            for (name, bytes) in &engine_records {
                if fs::read(native_state.join(name))
                    .map_err(|_| "native engine record unavailable after proof")?
                    != *bytes
                {
                    return Err("native engine settings changed during proof".into());
                }
            }
            Ok(())
        },
    );
    let passed = result.is_ok() && clean.is_ok() && unchanged.is_ok() && native_unchanged.is_ok();
    let evidence = json!({"schema_version":1,"proof":"approved-mcp-mutations","app":"memos","engine":"owned-wsl","passed":passed,
        "run_id":isolation.run_id,"checked_at_unix":now(),"test_sha256":digest(include_bytes!("managed_agent_mutations.rs")),
        "implementation_sha256":{
            "agent_requests":digest(include_bytes!("../src/agent_requests.rs")),
            "agent_gateway":digest(include_bytes!("../src/agent_gateway.rs")),
            "agent_mcp":digest(include_bytes!("../src/agent_mcp.rs"))},
        "cancellation_observed_stage":cancellation_stage,"bystander_count":before.len(),"steps":steps,
        "failure":result.as_ref().err().map(|error|runtime::redact(error)),
        "limits":["Windows backend and real stdio proof; does not prove native WebView UI","Running cancellation can occur before container creation; observed stage is recorded","External effects and permanent data deletion are not exercised"]});
    let encoded = serde_json::to_string_pretty(&evidence).unwrap();
    assert!(
        !encoded.contains(&secret),
        "bearer must not appear in evidence"
    );
    fs::write(
        repo.join("docs/evidence/memos-agent-mutations-2026-10-01.json"),
        format!("{encoded}\n"),
    )
    .unwrap();
    assert!(
        passed,
        "real approved mutation proof failed; inspect redacted evidence and private fixture"
    );
}

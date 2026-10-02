//! PrivateBin DOM provider in a non-root sandboxed browser with no network.
//! Rust brokers only the exact reviewed app's bounded HTTP resources. Paste
//! keys travel by stdin, stay DPAPI protected, and are never returned to agents.
use super::*;
use std::collections::BTreeMap;
pub const PROVIDER: &str = "privatebin-isolated-browser-v1@2.0.6";
const IMAGE:&str="mcr.microsoft.com/playwright:v1.62.1-noble@sha256:dcc5531e97840b9b5e794f2814476b21571c5124a3fca2267d73041f56e7580e";
const PACKAGE_SHA: &str = "6e4e424e7d4651b64e250707f41548e71ef0620e61252b39f1a91d2354e0ece0";
const RUNNER: &[u8] = include_bytes!("../../providers/privatebin/runner.mjs");
const PROFILE: &[u8] = include_bytes!("../../providers/privatebin/seccomp_profile.json");
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pastes {
    version: u32,
    items: Vec<Paste>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Paste {
    id: String,
    fragment: String,
}
fn paste_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn fragment(key: &str) -> bool {
    (32..=64).contains(&key.len())
        && key
            .bytes()
            .all(|b| b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&b))
}
fn pastes(connection: &Connection) -> AppResult<Pastes> {
    let state: Pastes = serde_json::from_str(&connection.token).map_err(|_| denied())?;
    if state.version != 1
        || state.items.len() > 32
        || state
            .items
            .iter()
            .any(|paste| !paste_id(&paste.id) || !fragment(&paste.fragment))
    {
        return Err(denied());
    }
    Ok(state)
}
fn package() -> AppResult<PathBuf> {
    let exe = std::env::current_exe()?;
    let mut base = exe.parent().ok_or_else(denied)?.to_owned();
    if base.file_name().is_some_and(|name| name == "deps") {
        base = base.parent().ok_or_else(denied)?.to_owned();
    }
    let root = base.join("browser/playwright-core");
    if !root.is_dir() {
        return Err(AppError::new(ErrorCode::PrerequisiteUnavailable,"The reviewed browser package is missing. Build the browser provider payload before connecting this app."));
    }
    fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) -> AppResult<()> {
        for item in fs::read_dir(dir)? {
            let item = item?;
            let meta = fs::symlink_metadata(item.path())?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err(denied());
                }
            }
            if meta.file_type().is_symlink() {
                return Err(denied());
            }
            if meta.is_dir() {
                collect(root, &item.path(), files)?;
            } else if meta.is_file() && meta.len() <= 8 * 1024 * 1024 {
                files.push((
                    item.path()
                        .strip_prefix(root)
                        .map_err(|_| denied())?
                        .to_string_lossy()
                        .replace('\\', "/"),
                    item.path(),
                ));
            } else {
                return Err(denied());
            }
            if files.len() > 4096 {
                return Err(denied());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(&root, &root, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut digest = Sha256::new();
    let mut total = 0;
    for (name, path) in files {
        let bytes = fs::read(path)?;
        total += bytes.len();
        if total > 64 * 1024 * 1024 {
            return Err(denied());
        }
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(Sha256::digest(&bytes));
    }
    if format!("{:x}", digest.finalize()) != PACKAGE_SHA {
        return Err(AppError::invalid(
            "The browser package does not match its reviewed integrity lock.",
        ));
    }
    Ok(root)
}
fn docker(args: Vec<String>, timeout: Duration) -> AppResult<runtime::ProcessOutput> {
    let spec =
        EngineBinding::managed_wsl().command(&runtime::CommandSpec::docker(args, None, timeout))?;
    runtime::SystemProcessRunner.run(&spec).map_err(|_| {
        AppError::new(
            ErrorCode::PrerequisiteUnavailable,
            "The isolated app browser could not complete its engine operation.",
        )
    })
}
fn checked(output: runtime::ProcessOutput) -> AppResult<String> {
    if !output.success || output.truncated {
        return Err(AppError::new(
            ErrorCode::PrerequisiteUnavailable,
            "The isolated app browser refused this operation.",
        ));
    }
    Ok(output.stdout)
}
pub(super) fn connect(content: &AgentContent, app_id: &str) -> AppResult<()> {
    let (_, _, fingerprint) = target_for_template(app_id, "privatebin", PROVIDER)?;
    package()?;
    let inspect = docker(
        vec![
            "image".into(),
            "inspect".into(),
            IMAGE.into(),
            "--format".into(),
            "{{.Id}}".into(),
        ],
        runtime::DIAGNOSTIC_TIMEOUT,
    )?;
    if !inspect.success {
        checked(docker(
            vec!["pull".into(), IMAGE.into()],
            runtime::PROVISION_TIMEOUT,
        )?)?;
    }
    checked(docker(
        vec![
            "image".into(),
            "inspect".into(),
            IMAGE.into(),
            "--format".into(),
            "{{.Id}}".into(),
        ],
        runtime::DIAGNOSTIC_TIMEOUT,
    )?)?;
    let mut store = Store::open(&content.root)?;
    content.revoke_app_scopes_for_owner(app_id)?;
    store
        .state
        .proposals
        .retain(|proposal| proposal.app_id != app_id);
    // Reconnecting does not discard existing protected fragment keys when the
    // exact installed target is unchanged. Pending approvals are invalidated.
    let previous = store
        .state
        .connections
        .iter()
        .find(|c| c.app_id == app_id && c.fingerprint == fingerprint)
        .map(|c| c.token.clone());
    store.state.connections.retain(|c| c.app_id != app_id);
    let token = previous.unwrap_or(
        serde_json::to_string(&Pastes {
            version: 1,
            ..Pastes::default()
        })
        .map_err(AppError::internal)?,
    );
    store.state.connections.push(Connection {
        app_id: app_id.into(),
        fingerprint,
        token,
        generation: nonce()?,
    });
    store.save()
}
fn get(
    client: &reqwest::blocking::Client,
    url: url::Url,
    json_response: bool,
) -> AppResult<Vec<u8>> {
    let mut request = client.get(url);
    if json_response {
        request = request.header("X-Requested-With", "JSONHttpRequest");
    }
    let response = request.send().map_err(|_| {
        AppError::new(
            ErrorCode::PrerequisiteUnavailable,
            "The reviewed app did not answer.",
        )
    })?;
    if !response.status().is_success() {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| denied())?;
    if bytes.len() > 1024 * 1024 {
        return Err(denied());
    }
    Ok(bytes)
}
fn client() -> AppResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| denied())
}
fn browser(content: &AgentContent, address: &url::Url, mut input: Value) -> AppResult<Value> {
    let payload = package()?;
    let http = client()?;
    let html = String::from_utf8(get(&http, address.clone(), false)?).map_err(|_| denied())?;
    let pattern =
        regex::Regex::new(r#"(?:src|href)\s*=\s*["']([^"']+)["']"#).map_err(AppError::internal)?;
    let mut assets = BTreeMap::new();
    let mut total = html.len();
    for capture in pattern.captures_iter(&html) {
        let value = capture.get(1).ok_or_else(denied)?.as_str();
        let path = address.join(value).map_err(|_| denied())?;
        if path.origin() != address.origin()
            || !matches!(path.path().split('/').nth(1), Some("js" | "css"))
            || !(path.path().ends_with(".js") || path.path().ends_with(".css"))
            || path.fragment().is_some()
        {
            continue;
        }
        let key = format!(
            "{}{}",
            path.path(),
            path.query().map(|q| format!("?{q}")).unwrap_or_default()
        );
        if assets.contains_key(&key) {
            continue;
        }
        if assets.len() >= 32 {
            return Err(denied());
        }
        let bytes = get(&http, path.clone(), false)?;
        total += bytes.len();
        if total > 8 * 1024 * 1024 {
            return Err(denied());
        }
        let body = String::from_utf8(bytes).map_err(|_| denied())?;
        assets.insert(key,json!({"type":if path.path().ends_with(".js"){"application/javascript"}else{"text/css"},"body":body}));
    }
    input["address"] = json!(address.as_str());
    input["html"] = json!(html);
    input["assets"] = json!(assets);
    let public = content.root.join("browser-runtime");
    fs::create_dir_all(&public)?;
    let runner = public.join("runner.mjs");
    let profile = public.join("seccomp_profile.json");
    storage::write_file_atomically(&runner, RUNNER)?;
    storage::write_file_atomically(&profile, PROFILE)?;
    fn mount(path: &Path) -> AppResult<String> {
        let value = path.to_str().ok_or_else(denied)?;
        if value.contains(',') {
            return Err(denied());
        }
        engine::wsl::windows_drive_path(value)
    }
    let name = format!("local-store-browser-{}", nonce()?);
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = docker(
                vec![
                    "container".into(),
                    "rm".into(),
                    "--force".into(),
                    self.0.clone(),
                ],
                runtime::DIAGNOSTIC_TIMEOUT,
            );
        }
    }
    let _cleanup = Cleanup(name.clone());
    let args = vec![
        "container".into(),
        "run".into(),
        "--rm".into(),
        "--pull=never".into(),
        "--init".into(),
        "--interactive".into(),
        "--name".into(),
        name,
        "--label".into(),
        "local-store.owner=privatebin-browser-v1".into(),
        "--network=none".into(),
        "--user=pwuser".into(),
        "--cap-drop=ALL".into(),
        "--security-opt=no-new-privileges".into(),
        format!("--security-opt=seccomp={}", mount(&profile)?),
        "--read-only".into(),
        "--tmpfs=/tmp:rw,nosuid,nodev,size=268435456".into(),
        "--tmpfs=/home/pwuser:rw,nosuid,nodev,size=67108864,uid=1001,gid=1001".into(),
        "--shm-size=256m".into(),
        "--memory=1g".into(),
        "--cpus=2".into(),
        "--pids-limit=256".into(),
        "--mount".into(),
        format!(
            "type=bind,src={},dst=/provider/playwright-core,readonly",
            mount(&payload)?
        ),
        "--mount".into(),
        format!(
            "type=bind,src={},dst=/provider/runner.mjs,readonly",
            mount(&runner)?
        ),
        IMAGE.into(),
        "node".into(),
        "/provider/runner.mjs".into(),
    ];
    let spec = EngineBinding::managed_wsl().command(&runtime::CommandSpec::docker(
        args,
        None,
        Duration::from_secs(90),
    ))?;
    let output = runtime::SystemProcessRunner
        .run_with_private_input(
            &spec,
            &serde_json::to_vec(&input).map_err(AppError::internal)?,
        )
        .map_err(|_| denied())?;
    let text = checked(output)?;
    serde_json::from_str(&text).map_err(|_| denied())
}
pub fn tools() -> Vec<Value> {
    [
    ("local_store_privatebin_read","Read only an encrypted paste previously created through this exact owner-connected app. Arbitrary URLs, paste keys, burning pastes, deletion and browser code are unavailable.",vec!["app_id","paste_id"]),
    ("local_store_privatebin_request_create","Request exact owner approval for one encrypted, non-burning PrivateBin paste with one-day expiry and no discussion. Does not create a paste.",vec!["app_id","content"]),
    ("local_store_privatebin_execute_create","Consume one exact approved encrypted-paste write once. The fragment key stays protected internally and is never returned to the agent.",vec!["request_id"]),
].into_iter().map(|(name,description,keys)|{let properties=keys.iter().map(|key|(key.to_string(),json!({"type":"string"}))).collect::<serde_json::Map<_,_>>();json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":keys,"additionalProperties":false},"annotations":{"readOnlyHint":name=="local_store_privatebin_read","destructiveHint":false}})}).collect()
}
pub fn arguments_valid(name: &str, args: &Value) -> bool {
    let keys: &[&str] = match name {
        "local_store_privatebin_read" => &["app_id", "paste_id"],
        "local_store_privatebin_request_create" => &["app_id", "content"],
        "local_store_privatebin_execute_create" => &["request_id"],
        _ => return false,
    };
    args.as_object().is_some_and(|obj| {
        obj.len() == keys.len()
            && keys.iter().all(|key| {
                obj.get(*key).and_then(Value::as_str).is_some_and(|v| {
                    !v.is_empty() && v.len() <= if *key == "content" { MAX_CONTENT } else { 128 }
                })
            })
    })
}
pub(super) fn call(
    content: &AgentContent,
    bearer: &str,
    name: &str,
    args: &Value,
) -> AppResult<Value> {
    if !arguments_valid(name, args) {
        return Err(denied());
    }
    let app_id = args["app_id"].as_str().unwrap_or_default();
    if name == "local_store_privatebin_read" {
        content
            .gateway
            .authorize_content(bearer, app_id, false, now()?)?;
        let (_, address, fingerprint) = target_for_template(app_id, "privatebin", PROVIDER)?;
        let store = Store::open(&content.root)?;
        let state = pastes(store.connection(app_id, &fingerprint)?)?;
        let paste = state
            .items
            .iter()
            .find(|paste| Some(paste.id.as_str()) == args["paste_id"].as_str())
            .ok_or_else(denied)?;
        content
            .gateway
            .authorize_content(bearer, app_id, false, now()?)?;
        let url = address
            .join(&format!("?pasteid={}", paste.id))
            .map_err(|_| denied())?;
        let encrypted: Value =
            serde_json::from_slice(&get(&client()?, url, true)?).map_err(|_| denied())?;
        let result = browser(
            content,
            &address,
            json!({"operation":"read","paste_id":paste.id,"fragment":paste.fragment,"encrypted":encrypted}),
        )?;
        content
            .gateway
            .authorize_content(bearer, app_id, false, now()?)?;
        let text = result["content"]
            .as_str()
            .filter(|s| s.len() <= MAX_CONTENT)
            .ok_or_else(denied)?;
        return Ok(json!({"paste_id":paste.id,"content":text,"untrusted_content":true}));
    }
    if name == "local_store_privatebin_request_create" {
        let text = args["content"].as_str().ok_or_else(denied)?;
        valid_content(text)?;
        let clock = now()?;
        content
            .gateway
            .check_content_write_request(bearer, app_id, clock)?;
        let (client_id, client_generation) = content.gateway.authenticate_mutation(bearer)?;
        let (_, _, fingerprint) = target_for_template(app_id, "privatebin", PROVIDER)?;
        let mut store = Store::open(&content.root)?;
        let connection = store.connection(app_id, &fingerprint)?;
        if pastes(connection)?.items.len() >= 32 {
            return Err(AppError::invalid(
                "This protected app connection has reached its 32-paste limit.",
            ));
        }
        let connection_generation = connection.generation.clone();
        store
            .state
            .proposals
            .retain(|p| !p.consumed && p.expires_at_unix > clock);
        if store.state.proposals.len() >= 32 {
            return Err(denied());
        }
        let id = nonce()?;
        store.state.proposals.push(Proposal {
            id: id.clone(),
            client_id,
            client_generation,
            app_id: app_id.into(),
            fingerprint,
            connection_generation,
            arguments_hash: ContentOperation::PrivateBinPaste.hash(text),
            content: text.into(),
            operation: ContentOperation::PrivateBinPaste,
            expires_at_unix: clock + 600,
            approved_until_unix: None,
            consumed: false,
        });
        store.save()?;
        return Ok(json!({"request_id":id,"state":"pending_owner_approval"}));
    }
    let (row, address, _) = content.claim(
        bearer,
        args["request_id"].as_str().ok_or_else(denied)?,
        ContentOperation::PrivateBinPaste,
    )?;
    let result = browser(
        content,
        &address,
        json!({"operation":"create","content":row.content}),
    )?;
    let fragment = result["fragment"]
        .as_str()
        .filter(|key| fragment(key))
        .ok_or_else(denied)?;
    let payload = &result["payload"];
    if payload["v"] != 2
        || payload["meta"]["expire"] != "1day"
        || payload["adata"]
            .as_array()
            .is_none_or(|a| a.len() != 4 || a[2] != 0 || a[3] != 0)
        || payload["ct"].as_str().is_none_or(|ct| ct.len() > 32768)
        || payload.as_object().is_none_or(|obj| {
            obj.len() != 4
                || !["v", "meta", "adata", "ct"]
                    .iter()
                    .all(|key| obj.contains_key(*key))
        })
    {
        return Err(denied());
    }
    // Recheck after browser work immediately before the single upstream write.
    content
        .gateway
        .authorize_content(bearer, &row.app_id, true, now()?)?;
    let (_, _, fingerprint) = target_for_template(&row.app_id, "privatebin", PROVIDER)?;
    if fingerprint != row.fingerprint {
        return Err(denied());
    }
    let mut store = Store::open(&content.root)?;
    let connection = store
        .state
        .connections
        .iter_mut()
        .find(|c| {
            c.app_id == row.app_id
                && c.fingerprint == row.fingerprint
                && c.generation == row.connection_generation
        })
        .ok_or_else(denied)?;
    let mut state = pastes(connection)?;
    if state.items.len() >= 32 {
        return Err(denied());
    }
    let response = client()?
        .post(address)
        .header("X-Requested-With", "JSONHttpRequest")
        .header("content-type", "application/json")
        .body(serde_json::to_vec(payload).map_err(AppError::internal)?)
        .send()
        .map_err(|_| {
            AppError::new(
                ErrorCode::PrerequisiteUnavailable,
                "Encrypted paste write could not be confirmed. No retry was made.",
            )
        })?;
    if !response.status().is_success() {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    response
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| denied())?;
    if bytes.len() > 8192 {
        return Err(denied());
    }
    let reply: Value = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    let id = reply["id"]
        .as_str()
        .filter(|id| paste_id(id))
        .ok_or_else(denied)?;
    if reply["status"] != 0 {
        return Err(denied());
    }
    state.items.push(Paste {
        id: id.into(),
        fragment: fragment.into(),
    });
    connection.token = serde_json::to_string(&state).map_err(AppError::internal)?;
    store.save()?;
    Ok(
        json!({"paste_id":id,"expires_after":"1day","burn_after_reading":false,"discussion":false,"fragment_key_returned":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_arbitrary_links_keys_code_deletion_and_self_approval() {
        assert!(!arguments_valid(
            "delete_paste",
            &json!({"app_id":"privatebin"})
        ));
        assert!(!arguments_valid(
            "local_store_privatebin_read",
            &json!({"app_id":"privatebin","paste_id":"x","url":"http://evil"})
        ));
        assert!(!arguments_valid(
            "local_store_privatebin_request_create",
            &json!({"app_id":"privatebin","content":"x","approved":true})
        ));
        assert!(!paste_id("../../secret"));
        assert!(!fragment("token with spaces"));
    }
}

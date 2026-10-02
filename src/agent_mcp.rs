//! Read-only MCP stdio boundary for the existing authenticated agent gateway.
//! The owner configures an enrolled bearer secret in the subprocess environment.

use crate::{agent_access::ToolResult, agent_gateway::AgentGateway};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LINE: usize = 64 * 1024;
const PROTOCOL_VERSION: &str = "2025-06-18";
const TOOL_NAME: &str = "local_store_get_status";
const LIST_TOOL_NAME: &str = "local_store_list_granted_apps";
const START_TOOL_NAME: &str = "local_store_start_app";
const STOP_TOOL_NAME: &str = "local_store_stop_app";
const REQUEST_INSTALL: &str = "local_store_request_install";
const REQUEST_UNINSTALL: &str = "local_store_request_uninstall";
const EXECUTE_REQUEST: &str = "local_store_execute_request";
const REQUEST_STATUS: &str = "local_store_request_status";
const CANCEL_REQUEST: &str = "local_store_cancel_request";

pub fn serve_stdio() -> Result<(), String> {
    let secret = std::env::var("LOCAL_STORE_AGENT_BEARER")
        .map_err(|_| "LOCAL_STORE_AGENT_BEARER is required".to_owned())?;
    if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("LOCAL_STORE_AGENT_BEARER is invalid".to_owned());
    }
    let gateway = AgentGateway::open_local().map_err(|error| error.message)?;
    let outcome = serve_with_mutations(
        io::stdin().lock(),
        io::stdout().lock(),
        |app_id| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "System clock is unavailable".to_owned())?
                .as_secs();
            gateway
                .get_status(&secret, None, app_id, now)
                .map_err(|error| error.message)
        },
        || {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "System clock is unavailable".to_owned())?
                .as_secs();
            gateway
                .list_granted_apps(&secret, now)
                .map_err(|error| error.message)
        },
        |app_id, start| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "System clock is unavailable".to_owned())?
                .as_secs();
            gateway
                .lifecycle(&secret, app_id, start, now)
                .map_err(|error| error.message)
        },
        |name, identity| {
            use crate::agent_requests::{AgentRequests, MutationKind};
            let requests = AgentRequests::open_local().map_err(|error| error.message)?;
            let result = match name {
                REQUEST_INSTALL => requests.request(&secret, MutationKind::Install, identity),
                REQUEST_UNINSTALL => {
                    requests.request(&secret, MutationKind::UninstallKeepData, identity)
                }
                EXECUTE_REQUEST => requests.execute(&secret, identity),
                REQUEST_STATUS => requests.status(&secret, identity),
                CANCEL_REQUEST => requests.cancel(&secret, identity),
                _ => return Err("Unknown mutation tool".into()),
            }
            .map_err(|error| error.message)?;
            serde_json::to_value(result).map_err(|_| "Could not encode operation status".to_owned())
        },
    )
    .map_err(|error| error.to_string());
    crate::agent_requests::finish_transport_workers();
    outcome
}

fn serve_with_mutations<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    mut status: impl FnMut(&str) -> Result<ToolResult, String>,
    mut list: impl FnMut() -> Result<Vec<String>, String>,
    mut lifecycle: impl FnMut(&str, bool) -> Result<ToolResult, String>,
    mut mutations: impl FnMut(&str, &str) -> Result<Value, String>,
) -> io::Result<()> {
    let mut initialized = false;
    loop {
        let mut line = Vec::new();
        let read =
            std::io::Read::take(&mut input, (MAX_LINE + 1) as u64).read_until(b'\n', &mut line)?;
        if read == 0 {
            return Ok(());
        }
        if line.len() > MAX_LINE || !line.ends_with(b"\n") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP input line is too long",
            ));
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(request) => request,
            Err(_) => {
                write_json(&mut output, &rpc_error(Value::Null, -32700, "Parse error"))?;
                continue;
            }
        };
        if let Some(response) = handle_with_mutations(
            &request,
            &mut initialized,
            &mut status,
            &mut list,
            &mut lifecycle,
            &mut mutations,
        ) {
            write_json(&mut output, &response)?;
        }
    }
}

fn write_json(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn handle_with_mutations(
    request: &Value,
    initialized: &mut bool,
    status: &mut impl FnMut(&str) -> Result<ToolResult, String>,
    list: &mut impl FnMut() -> Result<Vec<String>, String>,
    lifecycle: &mut impl FnMut(&str, bool) -> Result<ToolResult, String>,
    mutations: &mut impl FnMut(&str, &str) -> Result<Value, String>,
) -> Option<Value> {
    let id = request.get("id").cloned();
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !request.is_object()
        || id
            .as_ref()
            .is_some_and(|v| !v.is_string() && !v.is_number())
    {
        return Some(rpc_error(Value::Null, -32600, "Invalid Request"));
    }
    let method = match request.get("method").and_then(Value::as_str) {
        Some(method) => method,
        None => {
            return Some(rpc_error(
                id.unwrap_or(Value::Null),
                -32600,
                "Invalid Request",
            ))
        }
    };
    // Notifications cannot invoke tools. Client identity metadata is ignored.
    let id = id?;
    let result = match method {
        "initialize" => {
            if *initialized {
                return Some(rpc_error(id, -32600, "Already initialized"));
            }
            *initialized = true;
            json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {"tools": {}},
                "serverInfo": {"name": "local-store", "version": env!("CARGO_PKG_VERSION")}})
        }
        "ping" if *initialized => json!({}),
        "tools/list" if *initialized => {
            let mut result = json!({"tools": [{
            "name": TOOL_NAME,
            "description": "Read the status of one installed Local Store app when the owner granted this client access.",
            "inputSchema": {"type": "object", "properties": {"app_id": {"type": "string"}},
                "required": ["app_id"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false}
        }, {
            "name": LIST_TOOL_NAME,
            "description": "List installed Local Store app IDs with a live status grant for this client.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false}
        }, {
            "name": START_TOOL_NAME,
            "description": "Start one installed app with a live owner-granted lifecycle scope.",
            "inputSchema": {"type": "object", "properties": {"app_id": {"type": "string"}},
                "required": ["app_id"], "additionalProperties": false},
            "annotations": {"readOnlyHint": false, "destructiveHint": false}
        }, {
            "name": STOP_TOOL_NAME,
            "description": "Stop one installed app with a live owner-granted lifecycle scope.",
            "inputSchema": {"type": "object", "properties": {"app_id": {"type": "string"}},
                "required": ["app_id"], "additionalProperties": false},
            "annotations": {"readOnlyHint": false, "destructiveHint": false}
        }, mutation_tool(REQUEST_INSTALL, "Request owner approval to install a reviewed zero-input recipe; this never installs without approval.", "app_id", false),
           mutation_tool(REQUEST_UNINSTALL, "Request owner approval to uninstall an installed managed app while keeping all app data.", "app_id", false),
           mutation_tool(EXECUTE_REQUEST, "Execute one unexpired owner-approved request once. Returns a running operation to poll; never grants or approves itself.", "request_id", false),
           mutation_tool(REQUEST_STATUS, "Read this client's exact mutation request and operation status.", "request_id", true),
           mutation_tool(CANCEL_REQUEST, "Cancel this client's pending request or running install before its registry commit cutoff.", "request_id", false)]});
            if let Some(tools) = result["tools"].as_array_mut() {
                tools.extend(crate::agent_content::tools());
            }
            result
        }
        "tools/call" if *initialized => {
            let params = request.get("params");
            let name = params.and_then(|p| p.get("name")).and_then(Value::as_str);
            if name.is_some_and(|name| {
                crate::agent_content::tools()
                    .iter()
                    .any(|tool| tool["name"] == name)
            }) {
                let name = name.unwrap();
                let arguments = params
                    .and_then(|p| p.get("arguments"))
                    .unwrap_or(&Value::Null);
                if !crate::agent_content::tool_arguments_valid(name, arguments) {
                    return Some(rpc_error(id, -32602, "Invalid content arguments"));
                }
                let outcome = std::env::var("LOCAL_STORE_AGENT_BEARER")
                    .map_err(|_| "Agent connection is unavailable".to_owned())
                    .and_then(|bearer| {
                        crate::agent_content::call_tool(&bearer, name, arguments)
                            .map_err(|e| e.message)
                    });
                let result = match outcome {
                    Ok(value) => {
                        json!({"content":[{"type":"text","text":serde_json::to_string(&value).unwrap_or_default()}],"structuredContent":value,"isError":false})
                    }
                    Err(error) => json!({"content":[{"type":"text","text":error}],"isError":true}),
                };
                return Some(json!({"jsonrpc":"2.0","id":id,"result":result}));
            }
            if matches!(
                name,
                Some(
                    REQUEST_INSTALL
                        | REQUEST_UNINSTALL
                        | EXECUTE_REQUEST
                        | REQUEST_STATUS
                        | CANCEL_REQUEST
                )
            ) {
                let name = name.unwrap();
                let key = if matches!(name, REQUEST_INSTALL | REQUEST_UNINSTALL) {
                    "app_id"
                } else {
                    "request_id"
                };
                let arguments = params.and_then(|p| p.get("arguments"));
                let identity = arguments.and_then(|a| a.get(key)).and_then(Value::as_str);
                if !arguments.is_some_and(|a| a.as_object().is_some_and(|obj| obj.len() == 1))
                    || identity.is_none_or(|v| v.is_empty() || v.len() > 128)
                {
                    return Some(rpc_error(id, -32602, "Invalid mutation identity"));
                }
                let result = match mutations(name, identity.unwrap()) {
                    Ok(value) => {
                        json!({"content": [{"type":"text", "text": serde_json::to_string(&value).unwrap_or_default()}], "structuredContent": value, "isError": false})
                    }
                    Err(error) => {
                        let retry_safe = matches!(
                            error.as_str(),
                            "Agent requests are being updated. Retry shortly."
                                | "Agent credentials are in use by another process."
                        );
                        json!({"content": [{"type":"text", "text": error}], "structuredContent":{"error":{"code":if retry_safe {"operation_busy"} else {"request_failed"},"retry_safe":retry_safe}}, "isError": true})
                    }
                };
                return Some(json!({"jsonrpc":"2.0", "id":id, "result":result}));
            }
            if name == Some(LIST_TOOL_NAME) {
                let args = params.and_then(|p| p.get("arguments"));
                if !args.is_none_or(|a| a.as_object().is_some_and(|obj| obj.is_empty())) {
                    return Some(rpc_error(id, -32602, "Invalid arguments"));
                }
                let result = match list() {
                    Ok(app_ids) => json!({"content": [{"type": "text",
                        "text": serde_json::to_string(&app_ids).unwrap_or_default()}],
                        "structuredContent": {"app_ids": app_ids}, "isError": false}),
                    Err(error) => {
                        json!({"content": [{"type": "text", "text": error}], "isError": true})
                    }
                };
                return Some(json!({"jsonrpc": "2.0", "id": id, "result": result}));
            }
            if !matches!(name, Some(TOOL_NAME | START_TOOL_NAME | STOP_TOOL_NAME)) {
                return Some(rpc_error(id, -32602, "Unknown tool"));
            }
            let arguments = params.and_then(|p| p.get("arguments"));
            let app_id = arguments
                .and_then(|a| a.get("app_id"))
                .and_then(Value::as_str);
            if !arguments.is_some_and(|a| a.as_object().is_some_and(|obj| obj.len() == 1))
                || app_id.is_none_or(|v| v.is_empty() || v.len() > 128)
            {
                return Some(rpc_error(id, -32602, "Invalid app_id"));
            }
            let app_id = app_id.unwrap();
            let outcome = if name == Some(TOOL_NAME) {
                status(app_id)
            } else {
                lifecycle(app_id, name == Some(START_TOOL_NAME))
            };
            match outcome {
                Ok(value) => json!({"content": [{"type": "text",
                    "text": serde_json::to_string(&value).unwrap_or_default()}],
                    "structuredContent": value, "isError": false}),
                Err(error) => {
                    json!({"content": [{"type": "text", "text": error}], "isError": true})
                }
            }
        }
        _ if !*initialized => return Some(rpc_error(id, -32000, "Server not initialized")),
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn mutation_tool(name: &str, description: &str, key: &str, read_only: bool) -> Value {
    json!({"name": name, "description": description,
        "inputSchema": {"type":"object", "properties": {key: {"type":"string"}}, "required":[key], "additionalProperties":false},
        "annotations": {"readOnlyHint":read_only, "destructiveHint": name == EXECUTE_REQUEST}})
}

#[cfg(test)]
fn handle(
    request: &Value,
    initialized: &mut bool,
    status: &mut impl FnMut(&str) -> Result<ToolResult, String>,
    list: &mut impl FnMut() -> Result<Vec<String>, String>,
    lifecycle: &mut impl FnMut(&str, bool) -> Result<ToolResult, String>,
) -> Option<Value> {
    handle_with_mutations(
        request,
        initialized,
        status,
        list,
        lifecycle,
        &mut |_, _| Err("Owner approval required".into()),
    )
}

#[cfg(test)]
fn serve<R: BufRead, W: Write>(
    input: R,
    output: W,
    status: impl FnMut(&str) -> Result<ToolResult, String>,
    list: impl FnMut() -> Result<Vec<String>, String>,
    lifecycle: impl FnMut(&str, bool) -> Result<ToolResult, String>,
) -> io::Result<()> {
    serve_with_mutations(input, output, status, list, lifecycle, |_, _| {
        Err("Owner approval required".into())
    })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutation_tools_require_initialization_exact_arguments_and_never_approve() {
        let mut initialized = false;
        let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":REQUEST_INSTALL,"arguments":{"app_id":"memos"}}});
        let response = handle_with_mutations(
            &request,
            &mut initialized,
            &mut |_| panic!("status dispatched"),
            &mut || panic!("list dispatched"),
            &mut |_, _| panic!("lifecycle dispatched"),
            &mut |_, _| panic!("uninitialized mutation dispatched"),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], -32000);
        initialized = true;
        for (name, key) in [
            (REQUEST_INSTALL, "app_id"),
            (REQUEST_UNINSTALL, "app_id"),
            (EXECUTE_REQUEST, "request_id"),
            (REQUEST_STATUS, "request_id"),
            (CANCEL_REQUEST, "request_id"),
        ] {
            let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":name,"arguments":{key:"memos"}}});
            let response = handle_with_mutations(
                &request,
                &mut initialized,
                &mut |_| panic!("status dispatched"),
                &mut || panic!("list dispatched"),
                &mut |_, _| panic!("lifecycle dispatched"),
                &mut |actual_name, identity| {
                    assert_eq!(actual_name, name);
                    assert_eq!(identity, "memos");
                    Ok(json!({"state":"pending"}))
                },
            )
            .unwrap();
            assert_eq!(response["result"]["structuredContent"]["state"], "pending");
            assert_eq!(
                mutation_tool(name, "reviewed operation", key, false)["inputSchema"]["properties"]
                    [key]["type"],
                "string"
            );
            let invalid = json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":name,"arguments":{key:"memos","approve":true}}});
            let response = handle_with_mutations(
                &invalid,
                &mut initialized,
                &mut |_| panic!("status dispatched"),
                &mut || panic!("list dispatched"),
                &mut |_, _| panic!("lifecycle dispatched"),
                &mut |_, _| panic!("client approval argument dispatched"),
            )
            .unwrap();
            assert_eq!(response["error"]["code"], -32602);
        }
    }

    #[test]
    fn call_requires_initialization_and_uses_gateway_result() {
        let mut initialized = false;
        let mut calls = 0;
        let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":TOOL_NAME,"arguments":{"app_id":"memos"}},
            "clientInfo":{"name":"owner"}});
        let denied = handle(
            &request,
            &mut initialized,
            &mut |_| {
                calls += 1;
                Err("denied".into())
            },
            &mut || Err("denied".into()),
            &mut |_, _| Err("denied".into()),
        )
        .unwrap();
        assert_eq!(denied["error"]["code"], -32000);
        assert_eq!(calls, 0);
        let init = json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{}});
        assert!(handle(
            &init,
            &mut initialized,
            &mut |_| Err("denied".into()),
            &mut || Err("denied".into()),
            &mut |_, _| Err("denied".into())
        )
        .unwrap()
        .get("result")
        .is_some());
        let denied = handle(
            &request,
            &mut initialized,
            &mut |_| {
                calls += 1;
                Err("denied".into())
            },
            &mut || Err("denied".into()),
            &mut |_, _| Err("denied".into()),
        )
        .unwrap();
        assert_eq!(denied["result"]["isError"], true);
        assert_eq!(calls, 1);
    }

    #[test]
    fn list_tool_dispatches_without_client_supplied_scope() {
        let mut initialized = true;
        let request = json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":LIST_TOOL_NAME,"arguments":{}}});
        let response = handle(
            &request,
            &mut initialized,
            &mut |_| panic!("status called"),
            &mut || Ok(vec!["memos".into()]),
            &mut |_, _| panic!("lifecycle called"),
        )
        .unwrap();
        assert_eq!(
            response["result"]["structuredContent"]["app_ids"],
            json!(["memos"])
        );
        let invalid = json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":LIST_TOOL_NAME,"arguments":{"client_id":"other"}}});
        assert_eq!(
            handle(
                &invalid,
                &mut initialized,
                &mut |_| panic!("status called"),
                &mut || panic!("list called"),
                &mut |_, _| panic!("lifecycle called")
            )
            .unwrap()["error"]["code"],
            -32602
        );
    }

    #[test]
    fn stdio_roundtrip_is_newline_delimited() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n";
        let mut output = Vec::new();
        serve(
            &input[..],
            &mut output,
            |_| Err("denied".into()),
            || Err("denied".into()),
            |_, _| Err("denied".into()),
        )
        .unwrap();
        let lines: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(lines.len(), 2);
        let tools: Value = serde_json::from_slice(lines[1]).unwrap();
        assert_eq!(tools["result"]["tools"][0]["name"], TOOL_NAME);
    }

    #[test]
    fn lifecycle_tools_validate_id_and_use_server_side_scope() {
        let mut initialized = true;
        let mut operations = Vec::new();
        for (name, start) in [(START_TOOL_NAME, true), (STOP_TOOL_NAME, false)] {
            let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                "params":{"name":name,"arguments":{"app_id":"memos"}},
                "clientInfo":{"name":"pretend-owner"}});
            let result = handle(
                &request,
                &mut initialized,
                &mut |_| panic!("status called"),
                &mut || panic!("list called"),
                &mut |app_id, operation| {
                    operations.push((app_id.to_owned(), operation));
                    Err("owner grant required".into())
                },
            )
            .unwrap();
            assert_eq!(result["result"]["isError"], true);
            assert_eq!(operations.last(), Some(&("memos".into(), start)));
        }
        let bad = json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":START_TOOL_NAME,
                "arguments":{"app_id":"memos","client_id":"owner"}}});
        assert_eq!(
            handle(
                &bad,
                &mut initialized,
                &mut |_| panic!("status called"),
                &mut || panic!("list called"),
                &mut |_, _| panic!("lifecycle called")
            )
            .unwrap()["error"]["code"],
            -32602
        );
    }
}

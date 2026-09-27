//! Read-only MCP stdio boundary for the existing authenticated agent gateway.
//! The owner configures an enrolled bearer secret in the subprocess environment.

use crate::{agent_access::ToolResult, agent_gateway::AgentGateway};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LINE: usize = 64 * 1024;
const PROTOCOL_VERSION: &str = "2025-06-18";
const TOOL_NAME: &str = "local_store_get_status";

pub fn serve_stdio() -> Result<(), String> {
    let secret = std::env::var("LOCAL_STORE_AGENT_BEARER")
        .map_err(|_| "LOCAL_STORE_AGENT_BEARER is required".to_owned())?;
    if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("LOCAL_STORE_AGENT_BEARER is invalid".to_owned());
    }
    let gateway = AgentGateway::open_local().map_err(|error| error.message)?;
    serve(io::stdin().lock(), io::stdout().lock(), |app_id| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock is unavailable".to_owned())?
            .as_secs();
        gateway
            .get_status(&secret, None, app_id, now)
            .map_err(|error| error.message)
    })
    .map_err(|error| error.to_string())
}

fn serve<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    mut status: impl FnMut(&str) -> Result<ToolResult, String>,
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
        if let Some(response) = handle(&request, &mut initialized, &mut status) {
            write_json(&mut output, &response)?;
        }
    }
}

fn write_json(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn handle(
    request: &Value,
    initialized: &mut bool,
    status: &mut impl FnMut(&str) -> Result<ToolResult, String>,
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
        "tools/list" if *initialized => json!({"tools": [{
            "name": TOOL_NAME,
            "description": "Read the status of one installed Local Store app when the owner granted this client access.",
            "inputSchema": {"type": "object", "properties": {"app_id": {"type": "string"}},
                "required": ["app_id"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false}
        }]}),
        "tools/call" if *initialized => {
            let params = request.get("params");
            if params.and_then(|p| p.get("name")).and_then(Value::as_str) != Some(TOOL_NAME) {
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
            match status(app_id.unwrap()) {
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

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn call_requires_initialization_and_uses_gateway_result() {
        let mut initialized = false;
        let mut calls = 0;
        let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":TOOL_NAME,"arguments":{"app_id":"memos"}},
            "clientInfo":{"name":"owner"}});
        let denied = handle(&request, &mut initialized, &mut |_| {
            calls += 1;
            Err("denied".into())
        })
        .unwrap();
        assert_eq!(denied["error"]["code"], -32000);
        assert_eq!(calls, 0);
        let init = json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{}});
        assert!(
            handle(&init, &mut initialized, &mut |_| Err("denied".into()))
                .unwrap()
                .get("result")
                .is_some()
        );
        let denied = handle(&request, &mut initialized, &mut |_| {
            calls += 1;
            Err("denied".into())
        })
        .unwrap();
        assert_eq!(denied["result"]["isError"], true);
        assert_eq!(calls, 1);
    }

    #[test]
    fn stdio_roundtrip_is_newline_delimited() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n";
        let mut output = Vec::new();
        serve(&input[..], &mut output, |_| Err("denied".into())).unwrap();
        let lines: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(lines.len(), 2);
        let tools: Value = serde_json::from_slice(lines[1]).unwrap();
        assert_eq!(tools["result"]["tools"][0]["name"], TOOL_NAME);
    }
}

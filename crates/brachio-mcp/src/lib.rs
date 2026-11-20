//! Brachio's read-only MCP server over newline-delimited JSON-RPC on stdio.

use std::collections::HashSet;
use std::io::{self, BufRead, Read, Write};

use serde::Deserialize;
use serde_json::{json, Value};

const MAX_LINE: usize = 64 * 1024;
const MAX_REQUEST_IDS: usize = 65_536;
const TOOL: &str = "list_child_tasks";
const VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Initialize {
    protocol_version: String,
    capabilities: serde_json::Map<String, Value>,
    client_info: ClientInfo,
}

#[derive(Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolArguments {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCall {
    name: String,
    #[serde(default)]
    arguments: ToolArguments,
    #[serde(default, rename = "_meta")]
    _meta: Option<serde_json::Map<String, Value>>,
}

#[derive(Default)]
struct Session {
    version: Option<String>,
    ready: bool,
    ids: HashSet<Value>,
}

fn error(id: &Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

impl Session {
    fn respond(
        &mut self,
        request: &Value,
        list: &mut impl FnMut() -> Result<Value, String>,
    ) -> Option<Value> {
        let id = request.get("id");
        let method = request.get("method").and_then(Value::as_str);
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() {
            return Some(error(&Value::Null, -32600, "Invalid request"));
        }
        let method = method?;
        let Some(id) = id else {
            if method == "notifications/initialized" && self.version.is_some() {
                self.ready = true;
            }
            return None;
        };
        if id.as_str().is_none_or(|text| text.len() > 256) && !id.is_i64() && !id.is_u64() {
            return Some(error(&Value::Null, -32600, "Invalid request ID"));
        }
        if !self.ids.insert(id.clone()) {
            return Some(error(id, -32600, "Duplicate request ID"));
        }
        let params = request.get("params").unwrap_or(&Value::Null);
        let result = match method {
            "ping" => json!({}),
            "initialize" if self.version.is_none() => {
                let Ok(init) = serde_json::from_value::<Initialize>(params.clone()) else {
                    return Some(error(id, -32602, "Invalid initialize parameters"));
                };
                if init.client_info.name.is_empty() || init.client_info.version.is_empty() {
                    return Some(error(id, -32602, "Invalid client info"));
                }
                let _client_capabilities = init.capabilities;
                let version = if VERSIONS.contains(&init.protocol_version.as_str()) {
                    init.protocol_version
                } else {
                    VERSIONS[0].to_owned()
                };
                self.version = Some(version.clone());
                json!({
                    "protocolVersion":version, "capabilities":{"tools":{}},
                    "serverInfo":{"name":"brachio","version":env!("CARGO_PKG_VERSION")}
                })
            }
            "initialize" => return Some(error(id, -32600, "Already initialized")),
            _ if !self.ready => return Some(error(id, -32002, "Server not initialized")),
            "tools/list" => json!({"tools":[{
                "name":TOOL, "description":"List the child tasks and their report status for this terminal.",
                "inputSchema":{"type":"object","properties":{},"additionalProperties":false},
                "annotations":{"readOnlyHint":true,"openWorldHint":false}
            }]}),
            "tools/call" => {
                let Ok(call) = serde_json::from_value::<ToolCall>(params.clone()) else {
                    return Some(error(id, -32602, "Invalid tool arguments"));
                };
                if call.name != TOOL {
                    return Some(error(id, -32602, "Unknown tool"));
                }
                let ToolArguments {} = call.arguments;
                match list() {
                    Ok(children) => {
                        json!({"content":[{"type":"text","text":children.to_string()}]})
                    }
                    Err(message) => {
                        json!({"isError":true,"content":[{"type":"text","text":message}]})
                    }
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

/// Serve MCP requests until EOF, reading child state through the caller's existing identity.
/// Stdout is reserved for protocol messages.
///
/// # Errors
/// Returns an I/O error if stdin or stdout fails.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    mut list: impl FnMut() -> Result<Value, String>,
) -> io::Result<()> {
    let mut session = Session::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        if (&mut input)
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() > MAX_LINE || session.ids.len() >= MAX_REQUEST_IDS {
            write_response(
                &mut output,
                &error(&Value::Null, -32600, "Session limit exceeded"),
            )?;
            return Ok(());
        }
        let reply = match serde_json::from_slice(&line) {
            Ok(request) => session.respond(&request, &mut list),
            Err(_) => Some(error(&Value::Null, -32700, "Parse error")),
        };
        if let Some(reply) = reply {
            write_response(&mut output, &reply)?;
        }
    }
}

fn write_response(output: &mut impl Write, reply: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, reply)?;
    output.write_all(b"\n")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_the_read_only_tool_and_rejects_invalid_calls() {
        let input = concat!(
            "invalid\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_child_tasks","arguments":{"key":"other"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"spawn_task"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_child_tasks"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_child_tasks"}}"#,
            "\n",
        );
        let mut output = Vec::new();
        let mut calls = 0;
        serve(input.as_bytes(), &mut output, || {
            calls += 1;
            Ok(json!([{"key":"child:agent","reported":true}]))
        })
        .expect("serve");
        assert_eq!(calls, 1);
        let replies: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(&line.expect("read reply")).expect("JSON reply"))
            .collect();
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[1]["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(
            replies[2]["result"]["tools"]
                .as_array()
                .expect("tools")
                .len(),
            1
        );
        assert_eq!(replies[3]["error"]["code"], -32602);
        assert_eq!(replies[4]["error"]["code"], -32602);
        assert_eq!(
            replies[5]["result"]["content"][0]["text"],
            r#"[{"key":"child:agent","reported":true}]"#
        );
        assert_eq!(replies[6]["error"]["code"], -32600);
    }

    #[test]
    fn oversized_input_is_rejected_without_dispatch() {
        let input = vec![b'x'; MAX_LINE + 1];
        let mut output = Vec::new();
        serve(input.as_slice(), &mut output, || {
            panic!("oversized frame reached tool")
        })
        .expect("serve");
        let reply: Value = serde_json::from_slice(&output).expect("JSON reply");
        assert_eq!(reply["error"]["code"], -32600);
    }
}

//! Brachio's MCP server over newline-delimited JSON-RPC on stdio.
//!
//! Every tool goes through the [`Backend`], which the CLI wires to the same hook socket, identity
//! and approval flow as the `brachio` commands. Nothing here can create a task without the user's
//! approval; the app still enforces the child and depth limits.

use std::collections::HashSet;
use std::io::{self, BufRead, Read, Write};

use serde::Deserialize;
use serde_json::{json, Value};

const MAX_LINE: usize = 64 * 1024;
const MAX_REQUEST_IDS: usize = 65_536;
const VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const BAD_ARGUMENTS: &str = "Invalid tool arguments";

/// Same limits the app enforces for `brachio task create` and `brachio send`.
pub const MAX_TITLE_CHARS: usize = 200;
/// Longest prompt or body, in bytes.
pub const MAX_TEXT_BYTES: usize = 8 * 1024;
/// Longest review subject, in characters.
pub const MAX_SUBJECT_CHARS: usize = 200;
const MAX_BASE_BYTES: usize = 256;
/// Longest `get_task_result` wait, below what MCP clients commonly allow a tool call (60 s).
pub const MAX_WAIT_SECS: u64 = 30;

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

/// What a tool call reaches. An `Err` becomes an MCP tool error the agent can read.
pub trait Backend {
    /// The caller's child tasks.
    ///
    /// # Errors
    /// A message for the agent when the app cannot be reached or refuses.
    fn list_children(&mut self) -> Result<Value, String>;

    /// Ask the user to create a child task.
    ///
    /// # Errors
    /// A message for the agent, including when the user refuses.
    fn spawn_task(&mut self, request: SpawnTask) -> Result<Value, String>;

    /// The children, after waiting up to `wait_secs` (0 means do not wait) for one to report.
    ///
    /// # Errors
    /// A message for the agent when the app cannot be reached or refuses.
    fn task_result(&mut self, wait_secs: u64) -> Result<Value, String>;

    /// Send a `merge_ready` message to the lead task.
    ///
    /// # Errors
    /// A message for the agent when the app cannot be reached or refuses.
    fn request_review(&mut self, request: ReviewRequest) -> Result<Value, String>;

    /// Send a `decision_gate` to the lead task; the user answers it, and the answer comes back
    /// as a reply in this task's mailbox.
    ///
    /// # Errors
    /// A message for the agent when the app cannot be reached or refuses.
    fn raise_decision_gate(&mut self, request: DecisionGate) -> Result<Value, String>;
}

/// Validated input of `raise_decision_gate`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionGate {
    /// The question, on one line.
    pub subject: String,
    /// Details.
    #[serde(default)]
    pub body: String,
    /// The choices the user may pick, 2 to 6.
    pub options: Vec<String>,
}

/// Validated input of `spawn_task`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnTask {
    /// Task title.
    pub title: String,
    /// What the child agent starts with.
    pub prompt: String,
    /// `claude` or `codex`.
    #[serde(default)]
    pub agent: Option<String>,
    /// Commit-ish to branch from.
    #[serde(default)]
    pub base: Option<String>,
}

/// Validated input of `request_review`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    /// One line saying what this is about.
    pub subject: String,
    /// Details.
    #[serde(default)]
    pub body: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultArguments {
    #[serde(default)]
    wait_secs: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCall {
    name: String,
    #[serde(default)]
    arguments: Value,
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

fn tools() -> Value {
    json!([
        {
            "name":"list_child_tasks",
            "description":"List the child tasks and their report status for this terminal.",
            "inputSchema":{"type":"object","properties":{},"additionalProperties":false},
            "annotations":{"readOnlyHint":true,"openWorldHint":false}
        },
        {
            "name":"spawn_task",
            "description":"Ask the user to start a child task with its own worktree and branch. The user must approve first; this call waits for their answer (up to 5 minutes) and fails if they refuse. At most 4 open children, one level deep.",
            "inputSchema":{
                "type":"object",
                "properties":{
                    "title":{"type":"string","minLength":1,"maxLength":MAX_TITLE_CHARS,"description":"Task title; the branch name derives from it."},
                    "prompt":{"type":"string","description":"What the child's agent should do, at most 8 KiB."},
                    "agent":{"type":"string","enum":["claude","codex"]},
                    "base":{"type":"string","minLength":1,"maxLength":MAX_BASE_BYTES,"description":"Commit-ish to branch from; defaults to this task's branch."}
                },
                "required":["title","prompt"],
                "additionalProperties":false
            },
            "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}
        },
        {
            "name":"get_task_result",
            "description":"Show the child tasks and which have reported done. With wait_secs, first waits that long (at most 30) for a child to report; it never blocks longer.",
            "inputSchema":{
                "type":"object",
                "properties":{"wait_secs":{"type":"integer","minimum":0,"maximum":MAX_WAIT_SECS}},
                "additionalProperties":false
            },
            "annotations":{"readOnlyHint":true,"openWorldHint":false}
        },
        {
            "name":"request_review",
            "description":"Tell the lead task this branch is ready for review. Sends a merge_ready message to the lead.",
            "inputSchema":{
                "type":"object",
                "properties":{
                    "subject":{"type":"string","minLength":1,"maxLength":MAX_SUBJECT_CHARS},
                    "body":{"type":"string","description":"Details, at most 8 KiB; put long text in a file and send its path."}
                },
                "required":["subject"],
                "additionalProperties":false
            },
            "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}
        },
        {
            "name":"raise_decision_gate",
            "description":"Ask the user to choose before work that depends on this task goes on. Sends a decision_gate to the lead; the user's choice comes back as a reply in your mailbox. Nothing is decided for you.",
            "inputSchema":{
                "type":"object",
                "properties":{
                    "subject":{"type":"string","minLength":1,"maxLength":MAX_SUBJECT_CHARS},
                    "body":{"type":"string","description":"Details, at most 8 KiB."},
                    "options":{"type":"array","minItems":2,"maxItems":6,"items":{"type":"string","minLength":1,"maxLength":80}}
                },
                "required":["subject","options"],
                "additionalProperties":false
            },
            "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}
        }
    ])
}

/// `Err` is a protocol error (unknown tool or invalid arguments); `Ok(Err)` is a tool error.
fn call_tool(
    name: &str,
    arguments: Value,
    backend: &mut impl Backend,
) -> Result<Result<Value, String>, &'static str> {
    let arguments = if arguments.is_null() {
        json!({})
    } else {
        arguments
    };
    Ok(match name {
        "list_child_tasks" => {
            if arguments.as_object().is_none_or(|map| !map.is_empty()) {
                return Err(BAD_ARGUMENTS);
            }
            backend.list_children()
        }
        "spawn_task" => {
            let request: SpawnTask =
                serde_json::from_value(arguments).map_err(|_| BAD_ARGUMENTS)?;
            let agent_ok = request
                .agent
                .as_deref()
                .is_none_or(|agent| agent == "claude" || agent == "codex");
            let base_ok = request
                .base
                .as_deref()
                .is_none_or(|base| !base.is_empty() && base.len() <= MAX_BASE_BYTES);
            if request.title.trim().is_empty()
                || request.title.chars().count() > MAX_TITLE_CHARS
                || request.prompt.len() > MAX_TEXT_BYTES
                || !agent_ok
                || !base_ok
            {
                return Err(BAD_ARGUMENTS);
            }
            backend.spawn_task(request)
        }
        "get_task_result" => {
            let ResultArguments { wait_secs } =
                serde_json::from_value(arguments).map_err(|_| BAD_ARGUMENTS)?;
            if wait_secs > MAX_WAIT_SECS {
                return Err(BAD_ARGUMENTS);
            }
            backend.task_result(wait_secs)
        }
        "request_review" => {
            let request: ReviewRequest =
                serde_json::from_value(arguments).map_err(|_| BAD_ARGUMENTS)?;
            if request.subject.trim().is_empty()
                || request.subject.chars().count() > MAX_SUBJECT_CHARS
                || request.body.len() > MAX_TEXT_BYTES
            {
                return Err(BAD_ARGUMENTS);
            }
            backend.request_review(request)
        }
        "raise_decision_gate" => {
            let request: DecisionGate =
                serde_json::from_value(arguments).map_err(|_| BAD_ARGUMENTS)?;
            if request.subject.trim().is_empty()
                || request.subject.chars().count() > MAX_SUBJECT_CHARS
                || request.body.len() > MAX_TEXT_BYTES
                || !(2..=6).contains(&request.options.len())
            {
                return Err(BAD_ARGUMENTS);
            }
            backend.raise_decision_gate(request)
        }
        _ => return Err("Unknown tool"),
    })
}

impl Session {
    fn respond(&mut self, request: &Value, backend: &mut impl Backend) -> Option<Value> {
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
            "tools/list" => json!({"tools":tools()}),
            "tools/call" => {
                let Ok(call) = serde_json::from_value::<ToolCall>(params.clone()) else {
                    return Some(error(id, -32602, BAD_ARGUMENTS));
                };
                match call_tool(&call.name, call.arguments, backend) {
                    Ok(Ok(value)) => json!({"content":[{"type":"text","text":value.to_string()}]}),
                    Ok(Err(message)) => {
                        json!({"isError":true,"content":[{"type":"text","text":message}]})
                    }
                    Err(message) => return Some(error(id, -32602, message)),
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

/// Serve MCP requests until EOF, calling tools through the caller's existing identity and
/// approval flow. Stdout is reserved for protocol messages.
///
/// # Errors
/// Returns an I/O error if stdin or stdout fails.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    mut backend: impl Backend,
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
            Ok(request) => session.respond(&request, &mut backend),
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

    #[derive(Default)]
    struct Recorder {
        calls: Vec<String>,
    }

    impl Backend for Recorder {
        fn list_children(&mut self) -> Result<Value, String> {
            self.calls.push("list".into());
            Ok(json!([{"key":"child:agent","reported":true}]))
        }
        fn spawn_task(&mut self, request: SpawnTask) -> Result<Value, String> {
            self.calls.push(format!("spawn {}", request.title));
            Err("The user refused.".into())
        }
        fn task_result(&mut self, wait_secs: u64) -> Result<Value, String> {
            self.calls.push(format!("result {wait_secs}"));
            Ok(json!({}))
        }
        fn request_review(&mut self, request: ReviewRequest) -> Result<Value, String> {
            self.calls.push(format!("review {}", request.subject));
            Ok(json!({}))
        }
        fn raise_decision_gate(&mut self, request: DecisionGate) -> Result<Value, String> {
            self.calls.push(format!("gate {}", request.options.len()));
            Ok(json!({}))
        }
    }

    const HANDSHAKE: &str = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
    );

    fn run(requests: &str) -> (Vec<Value>, Recorder) {
        let input = format!("{HANDSHAKE}{requests}");
        let mut output = Vec::new();
        let mut recorder = Recorder::default();
        serve(input.as_bytes(), &mut output, &mut recorder).expect("serve");
        let replies = output
            .lines()
            .map(|line| serde_json::from_str(&line.expect("read reply")).expect("JSON reply"))
            .collect::<Vec<Value>>();
        (replies[1..].to_vec(), recorder)
    }

    impl Backend for &mut Recorder {
        fn list_children(&mut self) -> Result<Value, String> {
            (**self).list_children()
        }
        fn spawn_task(&mut self, request: SpawnTask) -> Result<Value, String> {
            (**self).spawn_task(request)
        }
        fn task_result(&mut self, wait_secs: u64) -> Result<Value, String> {
            (**self).task_result(wait_secs)
        }
        fn request_review(&mut self, request: ReviewRequest) -> Result<Value, String> {
            (**self).request_review(request)
        }
        fn raise_decision_gate(&mut self, request: DecisionGate) -> Result<Value, String> {
            (**self).raise_decision_gate(request)
        }
    }

    fn call(id: u32, name: &str, arguments: &Value) -> String {
        format!(
            "{}\n",
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})
        )
    }

    #[test]
    fn lists_exactly_the_served_tools_without_run_qa() {
        let (replies, _) = run(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let names: Vec<&str> = replies[0]["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name"))
            .collect();
        assert_eq!(
            names,
            [
                "list_child_tasks",
                "spawn_task",
                "get_task_result",
                "request_review",
                "raise_decision_gate"
            ]
        );
    }

    #[test]
    fn invalid_or_oversize_input_never_reaches_the_backend() {
        let long_title = "t".repeat(201);
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        let requests = [
            call(2, "list_child_tasks", &json!({"key":"other"})),
            call(3, "run_qa", &json!({})),
            call(4, "spawn_task", &json!({"title":long_title,"prompt":"p"})),
            call(5, "spawn_task", &json!({"title":"t","prompt":big})),
            call(
                6,
                "spawn_task",
                &json!({"title":"t","prompt":"p","agent":"gpt"}),
            ),
            call(
                7,
                "spawn_task",
                &json!({"title":"t","prompt":"p","extra":1}),
            ),
            call(8, "get_task_result", &json!({"wait_secs":31})),
            call(9, "request_review", &json!({"subject":"s","body":big})),
            call(10, "request_review", &json!({"subject":" "})),
        ]
        .concat();
        let (replies, recorder) = run(&requests);
        assert_eq!(replies.len(), 9);
        assert!(replies.iter().all(|reply| reply["error"]["code"] == -32602));
        assert!(recorder.calls.is_empty());
    }

    #[test]
    fn tools_reach_the_backend_and_a_refusal_is_a_tool_error() {
        let requests = [
            call(2, "list_child_tasks", &json!({})),
            call(3, "spawn_task", &json!({"title":"Docs","prompt":"Write"})),
            call(4, "get_task_result", &json!({"wait_secs":5})),
            call(5, "request_review", &json!({"subject":"Ready"})),
        ]
        .concat();
        let (replies, recorder) = run(&requests);
        assert_eq!(
            recorder.calls,
            ["list", "spawn Docs", "result 5", "review Ready"]
        );
        assert_eq!(replies[1]["result"]["isError"], true);
        assert_eq!(
            replies[1]["result"]["content"][0]["text"],
            "The user refused."
        );
        assert!(replies[2]["result"].get("isError").is_none());
    }

    #[test]
    fn oversized_input_is_rejected_without_dispatch() {
        let input = vec![b'x'; MAX_LINE + 1];
        let mut output = Vec::new();
        let mut recorder = Recorder::default();
        serve(input.as_slice(), &mut output, &mut recorder).expect("serve");
        let reply: Value = serde_json::from_slice(&output).expect("JSON reply");
        assert_eq!(reply["error"]["code"], -32600);
        assert!(recorder.calls.is_empty());
    }
}

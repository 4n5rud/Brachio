//! Brachio's MCP server over newline-delimited JSON-RPC on stdio.
//!
//! Every tool goes through the [`Backend`], which the CLI wires to the same hook socket, identity
//! and approval flow as the `brachio` commands. Nothing here can create a task without the user's
//! approval; the app still enforces the child and depth limits.

use std::collections::HashSet;
use std::io::{self, BufRead, Read, Write};
use std::sync::{mpsc, Mutex, MutexGuard, PoisonError};
use std::thread;

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
            "description":"Ask the user to start a child task with its own worktree and branch. The user must approve first; this call waits for their answer (up to 5 minutes) and fails if they refuse. Cancelling the call does not withdraw a request the user already approved: the child may still be created, and list_child_tasks shows it. At most 4 open children, one level deep.",
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

/// A tool call whose arguments were checked, ready for the backend.
enum Tool {
    ListChildren,
    Spawn(SpawnTask),
    Result(u64),
    Review(ReviewRequest),
    Gate(DecisionGate),
}

/// Checks a tool call without reaching the backend. `Err` is a protocol error (unknown tool or
/// invalid arguments).
fn parse_tool(name: &str, arguments: Value) -> Result<Tool, &'static str> {
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
            Tool::ListChildren
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
            Tool::Spawn(request)
        }
        "get_task_result" => {
            let ResultArguments { wait_secs } =
                serde_json::from_value(arguments).map_err(|_| BAD_ARGUMENTS)?;
            if wait_secs > MAX_WAIT_SECS {
                return Err(BAD_ARGUMENTS);
            }
            Tool::Result(wait_secs)
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
            Tool::Review(request)
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
            Tool::Gate(request)
        }
        _ => return Err("Unknown tool"),
    })
}

impl Tool {
    /// Runs the call; `Err` is a tool error the agent can read.
    fn run(self, backend: &mut impl Backend) -> Result<Value, String> {
        match self {
            Self::ListChildren => backend.list_children(),
            Self::Spawn(request) => backend.spawn_task(request),
            Self::Result(wait_secs) => backend.task_result(wait_secs),
            Self::Review(request) => backend.request_review(request),
            Self::Gate(request) => backend.raise_decision_gate(request),
        }
    }
}

fn tool_reply(id: &Value, outcome: Result<Value, String>) -> Value {
    let result = match outcome {
        Ok(value) => json!({"content":[{"type":"text","text":value.to_string()}]}),
        Err(message) => json!({"isError":true,"content":[{"type":"text","text":message}]}),
    };
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

/// What one message asks of the server.
enum Step {
    /// Answer now.
    Reply(Value),
    /// Run a tool; it may take minutes, so it runs apart from reading.
    Call(Value, Tool),
    /// `notifications/cancelled` for this request id.
    Cancel(Value),
    /// A notification that needs no answer.
    Nothing,
}

impl Session {
    fn respond(&mut self, request: &Value) -> Step {
        let id = request.get("id");
        let method = request.get("method").and_then(Value::as_str);
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Step::Reply(error(&Value::Null, -32600, "Invalid request"));
        }
        let Some(method) = method else {
            return Step::Reply(error(&Value::Null, -32600, "Invalid request"));
        };
        let params = request.get("params").unwrap_or(&Value::Null);
        let Some(id) = id else {
            match method {
                "notifications/initialized" if self.version.is_some() => self.ready = true,
                "notifications/cancelled" => {
                    if let Some(cancelled) = params.get("requestId") {
                        return Step::Cancel(cancelled.clone());
                    }
                }
                _ => {}
            }
            return Step::Nothing;
        };
        if id.as_str().is_none_or(|text| text.len() > 256) && !id.is_i64() && !id.is_u64() {
            return Step::Reply(error(&Value::Null, -32600, "Invalid request ID"));
        }
        if !self.ids.insert(id.clone()) {
            return Step::Reply(error(id, -32600, "Duplicate request ID"));
        }
        let result = match method {
            "ping" => json!({}),
            "initialize" if self.version.is_none() => {
                let Ok(init) = serde_json::from_value::<Initialize>(params.clone()) else {
                    return Step::Reply(error(id, -32602, "Invalid initialize parameters"));
                };
                if init.client_info.name.is_empty() || init.client_info.version.is_empty() {
                    return Step::Reply(error(id, -32602, "Invalid client info"));
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
            "initialize" => return Step::Reply(error(id, -32600, "Already initialized")),
            _ if !self.ready => return Step::Reply(error(id, -32002, "Server not initialized")),
            "tools/list" => json!({"tools":tools()}),
            "tools/call" => {
                let Ok(call) = serde_json::from_value::<ToolCall>(params.clone()) else {
                    return Step::Reply(error(id, -32602, BAD_ARGUMENTS));
                };
                return match parse_tool(&call.name, call.arguments) {
                    Ok(tool) => Step::Call(id.clone(), tool),
                    Err(message) => Step::Reply(error(id, -32602, message)),
                };
            }
            _ => return Step::Reply(error(id, -32601, "Method not found")),
        };
        Step::Reply(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

/// Tool calls handed to the backend and not answered yet, and which of them were cancelled.
#[derive(Default)]
struct InFlight {
    waiting: HashSet<Value>,
    cancelled: HashSet<Value>,
    /// Input ended or replies can no longer be written: no further call may start.
    closed: bool,
}

fn lock(in_flight: &Mutex<InFlight>) -> MutexGuard<'_, InFlight> {
    in_flight.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Serve MCP requests until EOF, calling tools through the caller's existing identity and
/// approval flow. Stdout is reserved for protocol messages.
///
/// # Errors
/// Returns an I/O error if stdin or stdout fails.
pub fn serve(
    input: impl BufRead,
    output: impl Write + Send,
    backend: impl Backend + Send,
) -> io::Result<()> {
    serve_with(input, move || output, backend)
}

/// [`serve`], with the output made on the thread that writes the replies, so it may hold a lock
/// such as [`io::StdoutLock`] for as long as the server runs.
///
/// Reading and answering control messages (`ping`, `initialize`, `tools/list`, cancellation)
/// never waits for a tool call: tool calls run one at a time on their own thread, in the order
/// they arrived. `notifications/cancelled` for a call that has not started removes it, so the
/// backend is never asked. A call that is already running is not interrupted, because the
/// backend may be waiting for the user's approval and an approved effect cannot be undone; its
/// result is dropped instead of sent, as MCP asks.
///
/// When input ends, or a reply cannot be written, calls that have not started are dropped, so
/// no further approval is asked for when nobody can read the result. Only the call already
/// running is waited for.
///
/// # Errors
/// Returns an I/O error if stdin or stdout fails.
pub fn serve_with<W: Write>(
    input: impl BufRead,
    make_output: impl FnOnce() -> W + Send,
    mut backend: impl Backend + Send,
) -> io::Result<()> {
    let in_flight = Mutex::new(InFlight::default());
    let (replies, outbox) = mpsc::channel::<Value>();
    let (jobs, queue) = mpsc::channel::<(Value, Tool)>();
    thread::scope(|scope| {
        let mut input = input;
        let in_flight = &in_flight;
        let writer = scope.spawn(move || write_replies(make_output(), &outbox, in_flight));
        let tool_replies = replies.clone();
        scope.spawn(move || {
            for (id, tool) in queue {
                {
                    let mut state = lock(in_flight);
                    let cancelled = state.cancelled.remove(&id);
                    if state.closed || cancelled {
                        state.waiting.remove(&id);
                        continue;
                    }
                }
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    tool.run(&mut backend)
                }))
                .unwrap_or_else(|_| Err("The tool failed unexpectedly.".to_owned()));
                let dropped = {
                    let mut state = lock(in_flight);
                    state.waiting.remove(&id);
                    state.cancelled.remove(&id)
                };
                if !dropped && tool_replies.send(tool_reply(&id, outcome)).is_err() {
                    return;
                }
            }
        });
        let read = read_requests(&mut input, &replies, &jobs, in_flight);
        lock(in_flight).closed = true;
        // Nothing more is read; let go of the input now rather than after the running call.
        drop(input);
        drop(jobs);
        drop(replies);
        let written = writer
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("the MCP reply writer stopped")));
        read.and(written)
    })
}

/// Reads messages until EOF or until replies can no longer be written.
fn read_requests(
    input: &mut impl BufRead,
    replies: &mpsc::Sender<Value>,
    jobs: &mpsc::Sender<(Value, Tool)>,
    in_flight: &Mutex<InFlight>,
) -> io::Result<()> {
    let mut session = Session::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        if input
            .by_ref()
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() > MAX_LINE || session.ids.len() >= MAX_REQUEST_IDS {
            let _ = replies.send(error(&Value::Null, -32600, "Session limit exceeded"));
            return Ok(());
        }
        let step = match serde_json::from_slice(&line) {
            Ok(request) => session.respond(&request),
            Err(_) => Step::Reply(error(&Value::Null, -32700, "Parse error")),
        };
        let open = match step {
            Step::Reply(reply) => replies.send(reply).is_ok(),
            Step::Call(id, tool) => {
                let mut state = lock(in_flight);
                if state.closed {
                    return Ok(());
                }
                state.waiting.insert(id.clone());
                jobs.send((id, tool)).is_ok()
            }
            Step::Cancel(id) => {
                let mut state = lock(in_flight);
                if state.waiting.contains(&id) {
                    state.cancelled.insert(id);
                }
                true
            }
            Step::Nothing => true,
        };
        if !open {
            return Ok(());
        }
    }
}

/// Writes replies until every sender is gone. On a write failure, no further call may start.
fn write_replies(
    mut output: impl Write,
    outbox: &mpsc::Receiver<Value>,
    in_flight: &Mutex<InFlight>,
) -> io::Result<()> {
    for reply in outbox {
        if let Err(failure) = write_response(&mut output, &reply) {
            lock(in_flight).closed = true;
            return Err(failure);
        }
    }
    Ok(())
}

fn write_response(output: &mut impl Write, reply: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, reply)?;
    output.write_all(b"\n")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

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
            assert_ne!(request.subject, "panic", "the backend failed");
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

    /// Sends the handshake and `requests`, keeps the input open until `expected` replies after
    /// the handshake arrived, then ends it. Calls not started at EOF are dropped, as a client
    /// that closed its input cannot read them.
    fn run(requests: &str, expected: usize) -> (Vec<Value>, Recorder) {
        let mut recorder = Recorder::default();
        let (send_line, lines) = mpsc::channel();
        let (sink, replies) = mpsc::channel();
        let mut received = Vec::new();
        let backend = &mut recorder;
        std::thread::scope(|scope| {
            let server = scope.spawn(move || serve(Pipe::new(lines), Replies::new(sink), backend));
            send_line
                .send(format!("{HANDSHAKE}{requests}").into_bytes())
                .expect("send");
            for _ in 0..=expected {
                received.push(replies.recv_timeout(PATIENCE).expect("reply"));
            }
            drop(send_line);
            server.join().expect("server thread").expect("serve");
        });
        received.extend(replies.try_iter());
        (received[1..].to_vec(), recorder)
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
        let (replies, _) = run(
            concat!(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, "\n"),
            1,
        );
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
        let (replies, recorder) = run(&requests, 9);
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
        let (replies, recorder) = run(&requests, 4);
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

    /// Input the test writes line by line while `serve` runs.
    struct Pipe {
        lines: mpsc::Receiver<Vec<u8>>,
        buffer: Vec<u8>,
        at: usize,
        /// Told when the server lets go of its input.
        closed: Option<mpsc::Sender<()>>,
    }

    impl Pipe {
        fn new(lines: mpsc::Receiver<Vec<u8>>) -> Self {
            Self {
                lines,
                buffer: Vec::new(),
                at: 0,
                closed: None,
            }
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            if let Some(closed) = &self.closed {
                let _ = closed.send(());
            }
        }
    }

    impl Read for Pipe {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let available = self.fill_buf()?;
            let count = available.len().min(out.len());
            out[..count].copy_from_slice(&available[..count]);
            self.consume(count);
            Ok(count)
        }
    }

    impl BufRead for Pipe {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if self.at == self.buffer.len() {
                match self.lines.recv() {
                    Ok(line) => {
                        self.buffer = line;
                        self.at = 0;
                    }
                    Err(_) => return Ok(&[]),
                }
            }
            Ok(&self.buffer[self.at..])
        }
        fn consume(&mut self, amount: usize) {
            self.at += amount;
        }
    }

    /// Output that hands each reply to the test as soon as it is written.
    struct Replies {
        sink: mpsc::Sender<Value>,
        pending: Vec<u8>,
        /// Replies written before every further write fails, as a closed pipe does.
        working_replies: usize,
        /// Told when the server lets go of its output.
        closed: Option<mpsc::Sender<()>>,
    }

    impl Replies {
        fn new(sink: mpsc::Sender<Value>) -> Self {
            Self {
                sink,
                pending: Vec::new(),
                working_replies: usize::MAX,
                closed: None,
            }
        }
    }

    impl Drop for Replies {
        fn drop(&mut self) {
            if let Some(closed) = &self.closed {
                let _ = closed.send(());
            }
        }
    }

    impl Write for Replies {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.working_replies == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.pending.extend_from_slice(bytes);
            while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.pending.drain(..=end).collect();
                let reply = serde_json::from_slice(&line).map_err(io::Error::other)?;
                let _ = self.sink.send(reply);
                self.working_replies -= 1;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A backend whose `spawn_task` waits until the test releases it, as a pending approval does.
    struct Blocking {
        started: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Blocking {
        fn record(&self, call: &str) {
            self.calls.lock().expect("calls").push(call.to_owned());
        }
    }

    impl Backend for Blocking {
        fn list_children(&mut self) -> Result<Value, String> {
            self.record("list");
            Ok(json!([]))
        }
        fn spawn_task(&mut self, request: SpawnTask) -> Result<Value, String> {
            self.record(&format!("spawn {}", request.title));
            let _ = self.started.send(());
            let _ = self.release.recv();
            Ok(json!({"key":"child"}))
        }
        fn task_result(&mut self, _wait_secs: u64) -> Result<Value, String> {
            self.record("result");
            Ok(json!({}))
        }
        fn request_review(&mut self, _request: ReviewRequest) -> Result<Value, String> {
            self.record("review");
            Ok(json!({}))
        }
        fn raise_decision_gate(&mut self, _request: DecisionGate) -> Result<Value, String> {
            self.record("gate");
            Ok(json!({}))
        }
    }

    const PATIENCE: Duration = Duration::from_secs(10);

    fn reply_to(replies: &mpsc::Receiver<Value>, id: u32) -> Value {
        let reply = replies
            .recv_timeout(PATIENCE)
            .unwrap_or_else(|_| panic!("no reply to request {id} while a tool call is pending"));
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    #[test]
    fn a_pending_tool_call_still_answers_ping_and_honors_cancellation() {
        let (send_line, lines) = mpsc::channel::<Vec<u8>>();
        let (sink, replies) = mpsc::channel();
        let (started, backend_started) = mpsc::channel();
        let (release_backend, release) = mpsc::channel();
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let backend = Blocking {
            started,
            release,
            calls: calls.clone(),
        };
        let server =
            std::thread::spawn(move || serve(Pipe::new(lines), Replies::new(sink), backend));
        let send = |text: String| send_line.send(text.into_bytes()).expect("send line");
        let ping = |id: u32| format!("{}\n", json!({"jsonrpc":"2.0","id":id,"method":"ping"}));
        let cancel = |id: u32| {
            format!(
                "{}\n",
                json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id,"reason":"test"}})
            )
        };

        send(HANDSHAKE.to_owned());
        reply_to(&replies, 1);
        send(call(
            2,
            "spawn_task",
            &json!({"title":"Docs","prompt":"Write"}),
        ));
        backend_started
            .recv_timeout(PATIENCE)
            .expect("the tool call reached the backend");

        send(ping(3));
        assert_eq!(reply_to(&replies, 3)["result"], json!({}));

        // A call queued behind the pending one, cancelled before it starts, never runs.
        send(call(4, "list_child_tasks", &json!({})));
        send(cancel(4));
        // The running call cannot be withdrawn; its result is dropped instead of sent.
        send(cancel(2));
        send(ping(5));
        reply_to(&replies, 5);

        release_backend.send(()).expect("release");
        send(call(6, "get_task_result", &json!({})));
        reply_to(&replies, 6);
        drop(send_line);
        server.join().expect("server thread").expect("serve");

        assert!(
            replies.try_recv().is_err(),
            "a cancelled request was answered"
        );
        assert_eq!(*calls.lock().expect("calls"), ["spawn Docs", "result"]);
    }

    #[test]
    fn cancelling_an_unknown_request_changes_nothing() {
        let requests = [
            call(2, "list_child_tasks", &json!({})),
            format!(
                "{}\n",
                json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":99}})
            ),
            call(3, "list_child_tasks", &json!({})),
        ]
        .concat();
        let (replies, recorder) = run(&requests, 2);
        let ids: Vec<&Value> = replies.iter().map(|reply| &reply["id"]).collect();
        assert_eq!(ids, [&json!(2), &json!(3)]);
        assert_eq!(recorder.calls, ["list", "list"]);
    }

    struct Harness {
        send_line: mpsc::Sender<Vec<u8>>,
        replies: mpsc::Receiver<Value>,
        backend_started: mpsc::Receiver<()>,
        release_backend: mpsc::Sender<()>,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        server: std::thread::JoinHandle<io::Result<()>>,
    }

    /// Serves a blocking backend; `input_closed` and `output_closed` hear when the server lets
    /// go of its input and output, and `working_replies` replies are written before output fails.
    fn harness(
        working_replies: usize,
        input_closed: mpsc::Sender<()>,
        output_closed: mpsc::Sender<()>,
    ) -> Harness {
        let (send_line, lines) = mpsc::channel::<Vec<u8>>();
        let (sink, replies) = mpsc::channel();
        let (started, backend_started) = mpsc::channel();
        let (release_backend, release) = mpsc::channel();
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let backend = Blocking {
            started,
            release,
            calls: calls.clone(),
        };
        let server = std::thread::spawn(move || {
            let mut input = Pipe::new(lines);
            input.closed = Some(input_closed);
            let mut output = Replies::new(sink);
            output.working_replies = working_replies;
            output.closed = Some(output_closed);
            serve(input, output, backend)
        });
        Harness {
            send_line,
            replies,
            backend_started,
            release_backend,
            calls,
            server,
        }
    }

    impl Harness {
        fn send(&self, text: &str) {
            self.send_line
                .send(text.as_bytes().to_vec())
                .expect("send line");
        }
    }

    fn spawn_call(id: u32) -> String {
        call(id, "spawn_task", &json!({"title":"Docs","prompt":"Write"}))
    }

    #[test]
    fn calls_not_started_when_input_ends_are_dropped_and_the_running_one_finishes() {
        let (input_closed, input_gone) = mpsc::channel();
        let (output_closed, _output_gone) = mpsc::channel();
        let test = harness(usize::MAX, input_closed, output_closed);
        test.send(HANDSHAKE);
        reply_to(&test.replies, 1);
        test.send(&spawn_call(2));
        test.backend_started
            .recv_timeout(PATIENCE)
            .expect("the tool call reached the backend");
        test.send(&spawn_call(3));
        test.send(&call(4, "list_child_tasks", &json!({})));
        test.send(&format!(
            "{}\n",
            json!({"jsonrpc":"2.0","id":5,"method":"ping"})
        ));
        reply_to(&test.replies, 5);

        let Harness {
            send_line,
            replies,
            release_backend,
            calls,
            server,
            ..
        } = test;
        drop(send_line);
        input_gone
            .recv_timeout(PATIENCE)
            .expect("the server let go of its input");
        drop(release_backend);
        server.join().expect("server thread").expect("serve");

        assert_eq!(
            reply_to(&replies, 2)["result"]["content"][0]["text"],
            r#"{"key":"child"}"#
        );
        assert!(replies.try_recv().is_err(), "a dropped call was answered");
        assert_eq!(*calls.lock().expect("calls"), ["spawn Docs"]);
    }

    #[test]
    fn no_call_starts_after_a_reply_cannot_be_written() {
        let (input_closed, _input_gone) = mpsc::channel();
        let (output_closed, output_gone) = mpsc::channel();
        // The initialize reply is written; the next write fails, as when stdout was closed.
        let Harness {
            send_line,
            replies,
            release_backend,
            calls,
            server,
            ..
        } = harness(1, input_closed, output_closed);
        drop(release_backend);
        send_line.send(HANDSHAKE.as_bytes().to_vec()).expect("send");
        reply_to(&replies, 1);
        send_line
            .send(format!("{}\n", json!({"jsonrpc":"2.0","id":2,"method":"ping"})).into_bytes())
            .expect("send");
        output_gone
            .recv_timeout(PATIENCE)
            .expect("the failed output was let go");

        // Nobody can read the answer, so the call must not ask the user for approval.
        let _ = send_line.send(spawn_call(3).into_bytes());
        let _ = send_line.send(call(4, "list_child_tasks", &json!({})).into_bytes());
        drop(send_line);
        let served = server.join().expect("server thread");
        assert!(served.is_err(), "a failed write is an error");
        assert!(calls.lock().expect("calls").is_empty());
    }

    #[test]
    fn a_backend_that_panics_answers_with_a_tool_error_and_the_server_goes_on() {
        let requests = [
            call(2, "request_review", &json!({"subject":"panic"})),
            call(3, "list_child_tasks", &json!({})),
        ]
        .concat();
        let (replies, recorder) = run(&requests, 2);
        assert_eq!(replies[0]["id"], 2);
        assert_eq!(replies[0]["result"]["isError"], true);
        assert_eq!(replies[1]["id"], 3);
        assert_eq!(recorder.calls, ["review panic", "list"]);
    }
}

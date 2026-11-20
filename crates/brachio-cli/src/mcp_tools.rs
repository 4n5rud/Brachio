//! The MCP tools, answered through the same socket, identity and user approval as the commands.

use std::time::Duration;

use brachio_mcp::{Backend, ReviewRequest, SpawnTask};
use brachio_runtime::mailbox::{
    MailRequest, MailResponse, SendMail, CALL_TIMEOUT, CREATE_TASK_TIMEOUT,
};
use serde_json::{json, Value};

use crate::mail;

/// How much longer than the wait the call waits for the app's answer.
const ANSWER_SLACK: Duration = Duration::from_secs(10);

pub struct McpTools;

fn call(request: &MailRequest, timeout: Duration) -> Result<MailResponse, String> {
    let (endpoint, _) = mail::endpoint_and_key().map_err(|error| error.to_string())?;
    mail::call_within(&endpoint, request, timeout).map_err(|error| error.to_string())
}

fn own_key() -> Result<String, String> {
    mail::endpoint_and_key()
        .map(|(_, key)| key)
        .map_err(|error| error.to_string())
}

fn unexpected() -> String {
    "Brachio answered with something unexpected.".to_owned()
}

fn child_list(key: String) -> Result<Value, String> {
    let MailResponse::TaskList { children } = call(&MailRequest::ListTasks { key }, CALL_TIMEOUT)?
    else {
        return Err(unexpected());
    };
    serde_json::to_value(children).map_err(|error| error.to_string())
}

impl Backend for McpTools {
    fn list_children(&mut self) -> Result<Value, String> {
        child_list(own_key()?)
    }

    fn spawn_task(&mut self, request: SpawnTask) -> Result<Value, String> {
        let request = MailRequest::CreateTask {
            key: own_key()?,
            title: request.title,
            prompt: request.prompt,
            agent: request.agent,
            base: request.base,
        };
        let MailResponse::TaskCreated { key, branch, .. } = call(&request, CREATE_TASK_TIMEOUT)?
        else {
            return Err(unexpected());
        };
        Ok(json!({"key":key,"branch":branch}))
    }

    fn task_result(&mut self, wait_secs: u64) -> Result<Value, String> {
        let key = own_key()?;
        let wait = if wait_secs == 0 {
            Value::Null
        } else {
            let request = MailRequest::WaitTasks {
                key: key.clone(),
                all: false,
                timeout_secs: wait_secs,
            };
            let MailResponse::TasksReported {
                reported,
                pending,
                timed_out,
            } = call(&request, Duration::from_secs(wait_secs) + ANSWER_SLACK)?
            else {
                return Err(unexpected());
            };
            json!({"reported":reported,"pending":pending,"timed_out":timed_out})
        };
        Ok(json!({"children":child_list(key)?,"wait":wait}))
    }

    fn request_review(&mut self, request: ReviewRequest) -> Result<Value, String> {
        let request = MailRequest::Send(SendMail {
            from: own_key()?,
            to: "parent".to_owned(),
            kind: "merge_ready".to_owned(),
            priority: None,
            subject: request.subject,
            body: request.body,
            payload: None,
            reply_to: None,
            request_id: None,
        });
        let MailResponse::Sent { id, to, .. } = call(&request, CALL_TIMEOUT)? else {
            return Err(unexpected());
        };
        Ok(json!({"id":id,"to":to}))
    }
}

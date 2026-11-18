//! Client the app uses to reach a running terminal runtime.

use std::io;

use interprocess::local_socket::tokio::Stream;
use thiserror::Error;
use tokio::io::BufReader;

use crate::endpoint::{self, Liveness};
use crate::protocol;
use crate::transport;
use crate::wire::{
    read_message, write_message, Action, Reply, Request, ResponseResult, IO_TIMEOUT, MAX_MESSAGE,
    MAX_RESPONSE,
};

/// Why a request to the runtime did not produce a reply.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The endpoint is missing or refused the connection, so no runtime owns it.
    #[error("terminal runtime is not running")]
    NotRunning,
    /// The connection failed in a way that does not show whether a runtime exists.
    #[error("cannot tell whether the terminal runtime is running: {0}")]
    Unreachable(io::Error),
    /// The runtime understood the request and refused it.
    #[error("terminal runtime refused the request: {0}")]
    Refused(String),
    /// The runtime answered with something this client cannot use.
    #[error("terminal runtime sent an unexpected reply: {0}")]
    Unexpected(String),
    /// The connection broke after it was made.
    #[error("connection to the terminal runtime failed: {0}")]
    Connection(String),
}

/// A handle to the runtime that owns one endpoint. Each call opens its own connection.
#[derive(Clone, Debug)]
pub struct Client {
    endpoint: String,
}

/// Replies that keep arriving on one connection after the first.
pub struct Following {
    reader: BufReader<Stream>,
}

impl Client {
    /// Create a client for the runtime serving `endpoint`.
    #[must_use]
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_owned(),
        }
    }

    /// Send one action and wait for its reply.
    ///
    /// # Errors
    /// Fails when no runtime answers, the runtime refuses the action, or the reply is unusable.
    pub async fn call(&self, action: Action) -> Result<Reply, ClientError> {
        let (reply, _) = self.open(action).await?;
        Ok(reply)
    }

    /// Send a follow action and return its first reply together with the later ones.
    ///
    /// # Errors
    /// Fails like [`Client::call`].
    pub async fn follow(&self, action: Action) -> Result<(Reply, Following), ClientError> {
        self.open(action).await
    }

    async fn open(&self, action: Action) -> Result<(Reply, Following), ClientError> {
        let connected = transport::connect(&self.endpoint).await;
        let mut stream = match (endpoint::classify(&connected), connected) {
            (_, Ok(stream)) => stream,
            (Liveness::Dead, Err(_)) => return Err(ClientError::NotRunning),
            (_, Err(error)) => return Err(ClientError::Unreachable(error)),
        };
        let request = Request {
            version: protocol::CURRENT,
            action,
        };
        timed(write_message(&mut stream, &request, MAX_MESSAGE)).await?;
        let mut reader = BufReader::new(stream);
        let reply = read_reply(&mut reader).await?.ok_or_else(|| {
            ClientError::Connection("runtime closed before responding".to_owned())
        })?;
        Ok((reply, Following { reader }))
    }
}

impl Following {
    /// Wait for the next reply, or `None` when the runtime closed the connection.
    ///
    /// # Errors
    /// Fails when the connection breaks or the runtime refuses.
    pub async fn next(&mut self) -> Result<Option<Reply>, ClientError> {
        let message = read_message(&mut self.reader, MAX_RESPONSE)
            .await
            .map_err(|error| ClientError::Connection(format!("{error:#}")))?;
        message.map(|message| decode(&message)).transpose()
    }
}

async fn read_reply(reader: &mut BufReader<Stream>) -> Result<Option<Reply>, ClientError> {
    let message = timed(read_message(reader, MAX_RESPONSE)).await?;
    message.map(|message| decode(&message)).transpose()
}

fn decode(message: &[u8]) -> Result<Reply, ClientError> {
    let reply: Reply = serde_json::from_slice(message)
        .map_err(|error| ClientError::Unexpected(error.to_string()))?;
    if let ResponseResult::Error { error } = reply.result {
        return Err(ClientError::Refused(error));
    }
    Ok(reply)
}

async fn timed<T>(
    operation: impl std::future::Future<Output = anyhow::Result<T>>,
) -> Result<T, ClientError> {
    match tokio::time::timeout(IO_TIMEOUT, operation).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(ClientError::Connection(format!("{error:#}"))),
        Err(_) => Err(ClientError::Connection("timed out".to_owned())),
    }
}

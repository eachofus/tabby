// === IMPORTS ===
use anyhow::Result;
use axum::{extract::Request, http::HeaderName};
use axum_extra::headers::Header;
use serde::{Deserialize, Serialize};
use tabby_common::api::event::{EventLogger, LogEntry};
use tokio_tungstenite::connect_async;

use crate::axum::websocket::WebSocketTransport;

// === CONSTANTS ===
/// Header name for client request metadata
pub static CLIENT_REQUEST_HEADER: HeaderName = HeaderName::from_static("x-tabby-client-request");

// === STRUCTS ===
/// Request structure for connecting to the hub
#[derive(Serialize, Deserialize)]
pub struct ConnectHubRequest;

/// Client wrapper for hub communication
#[derive(Clone)]
pub struct WorkerClient(HubClient);

// === TRAITS ===
/// RPC service trait for hub communication
#[tarpc::service]
pub trait Hub {
    /// Write a log entry to the hub
    async fn write_log(x: LogEntry);
}

// === IMPLEMENTATIONS ===
impl Header for ConnectHubRequest {
    fn name() -> &'static axum::http::HeaderName {
        &CLIENT_REQUEST_HEADER
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, axum_extra::headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i axum::http::HeaderValue>,
    {
        let mut x: Vec<_> = values
            .map(|x| serde_json::from_slice(x.as_bytes()))
            .collect();
        if let Some(x) = x.pop() {
            x.map_err(|_| axum_extra::headers::Error::invalid())
        } else {
            Err(axum_extra::headers::Error::invalid())
        }
    }

    fn encode<E: Extend<axum::http::HeaderValue>>(&self, _values: &mut E) {
        todo!()
    }
}

impl EventLogger for WorkerClient {
    fn write(&self, x: LogEntry) {
        let context = tarpc::context::current();
        let client = self.0.clone();
        tokio::spawn(async move { client.write_log(context, x).await });
    }
}

// === FREE FUNCTIONS ===
/// Create a new worker client connected to the specified hub
pub async fn create_worker_client(addr: &str, token: &str) -> WorkerClient {
    let request = build_client_request(addr, token, ConnectHubRequest);
    let (socket, _) = connect_async(request).await.unwrap();
    WorkerClient(HubClient::new(Default::default(), WebSocketTransport::from(socket)).spawn())
}

/// Build HTTP request for WebSocket connection to hub
fn build_client_request(addr: &str, token: &str, request: ConnectHubRequest) -> Request<()> {
    Request::builder()
        .uri(format!("ws://{addr}/hub"))
        .header("Host", addr)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "unused")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header(
            CLIENT_REQUEST_HEADER.as_str(),
            serde_json::to_string(&request).unwrap(),
        )
        .body(())
        .unwrap()
}

// TODO: Remove unused function or integrate into tracing system
// Currently unused - dead code warning
#[allow(dead_code)]
fn tracing_context() -> tarpc::context::Context {
    tarpc::context::current()
}

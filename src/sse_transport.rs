//! Legacy SSE transport for backwards compatibility with MCP clients.
//!
//! Implements the deprecated HTTP+SSE transport protocol:
//! - GET `/sse` → opens SSE stream, sends `endpoint` event with POST URL
//! - POST `/message?sessionId=...` → receives JSON-RPC messages from client

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use rmcp::{
    model::ClientJsonRpcMessage, service::TxJsonRpcMessage,
    transport::sink_stream::TransportAdapterSinkStream, transport::IntoTransport, RoleServer,
    ServiceExt,
};
use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::PollSender;

/// Shared state for SSE transport.
type TxStore =
    Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::mpsc::Sender<ClientJsonRpcMessage>>>>;

/// Query parameters for the POST endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostEventQuery {
    session_id: String,
}

/// Application state for the SSE transport.
#[derive(Clone)]
struct SseApp {
    txs: TxStore,
    new_session_tx: tokio::sync::mpsc::UnboundedSender<SseSession>,
    post_path: Arc<str>,
}

/// An active SSE session with a client.
pub struct SseSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Stream of incoming client JSON-RPC messages.
    pub client_rx: ReceiverStream<ClientJsonRpcMessage>,
    /// Sink for outgoing server JSON-RPC messages.
    pub server_tx: PollSender<TxJsonRpcMessage<RoleServer>>,
}

/// Creates the SSE transport router and a channel to receive new sessions.
///
/// Returns `(Router, tokio::sync::mpsc::UnboundedReceiver<SseSession>)`.
/// The receiver yields new sessions as clients connect via GET `/sse`.
pub fn create_sse_router(
    sse_path: &str,
    post_path: &str,
) -> (Router, tokio::sync::mpsc::UnboundedReceiver<SseSession>) {
    let (new_session_tx, new_session_rx) = tokio::sync::mpsc::unbounded_channel();

    let app = SseApp {
        txs: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        new_session_tx,
        post_path: post_path.into(),
    };

    let router = Router::new()
        .route(sse_path, get(sse_handler))
        .route(post_path, post(post_event_handler))
        .with_state(app);

    (router, new_session_rx)
}

/// GET `/sse` handler — opens an SSE stream for a new client session.
async fn sse_handler(
    State(app): State<SseApp>,
    parts: axum::http::request::Parts,
) -> Result<Response, Response> {
    let session_id = uuid::Uuid::new_v4().to_string();
    tracing::info!(%session_id, ?parts, "new SSE connection");

    // Channels for bidirectional communication
    let (from_client_tx, from_client_rx) = tokio::sync::mpsc::channel(64);
    let (to_client_tx, to_client_rx) = tokio::sync::mpsc::channel(64);

    // Register session
    app.txs
        .write()
        .await
        .insert(session_id.clone(), from_client_tx);

    // Clone before moving into PollSender
    let to_client_tx_clone = to_client_tx.clone();

    let session = SseSession {
        session_id: session_id.clone(),
        client_rx: ReceiverStream::new(from_client_rx),
        server_tx: PollSender::new(to_client_tx),
    };

    if app.new_session_tx.send(session).is_err() {
        tracing::warn!("Failed to send session — server is shut down");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Server is shutting down").into_response());
    }

    // Build the SSE stream: first send the endpoint event, then stream messages
    let post_path = app.post_path.as_ref().to_string();
    let endpoint_event = Event::default()
        .event("endpoint")
        .data(format!("{}?sessionId={}", post_path, session_id));

    let stream = futures::stream::once(futures::future::ok(endpoint_event)).chain(
        ReceiverStream::new(to_client_rx).map(|message| match serde_json::to_string(&message) {
            Ok(bytes) => Ok(Event::default().event("message").data(&bytes)),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        }),
    );

    // Clean up on disconnect
    let session_id_clone = session_id.clone();
    let txs = app.txs.clone();
    tokio::spawn(async move {
        to_client_tx_clone.closed().await;
        txs.write().await.remove(&session_id_clone);
        tracing::debug!(%session_id_clone, "SSE session closed, cleaned up");
    });

    let response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();

    Ok(response)
}

/// POST `/message?sessionId=...` handler — receives JSON-RPC messages from client.
async fn post_event_handler(
    State(app): State<SseApp>,
    Query(PostEventQuery { session_id }): Query<PostEventQuery>,
    parts: axum::http::request::Parts,
    Json(mut message): Json<ClientJsonRpcMessage>,
) -> Response {
    tracing::debug!(%session_id, ?parts, ?message, "received client message via POST");

    let tx = {
        let rg = app.txs.read().await;
        match rg.get(session_id.as_str()) {
            Some(tx) => tx.clone(),
            None => {
                tracing::warn!(%session_id, "session not found");
                return StatusCode::NOT_FOUND.into_response();
            }
        }
    };

    message.insert_extension(parts);

    match tx.send(message).await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(_) => {
            tracing::error!(%session_id, "failed to send message — channel closed");
            StatusCode::GONE.into_response()
        }
    }
}

/// Serves an MCP handler for each incoming SSE session.
pub async fn serve_sse_sessions<S, F>(
    mut session_rx: tokio::sync::mpsc::UnboundedReceiver<SseSession>,
    handler_factory: F,
) where
    S: rmcp::Service<RoleServer> + Send + 'static,
    F: Fn() -> S + Send + 'static,
{
    while let Some(session) = session_rx.recv().await {
        let handler = handler_factory();
        let session_id = session.session_id.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_session(handler, session).await {
                tracing::error!(%session_id, "session error: {}", e);
            }
        });
    }
}

async fn serve_session<S>(service: S, session: SseSession) -> std::io::Result<()>
where
    S: rmcp::Service<RoleServer>,
{
    let SseSession {
        session_id,
        client_rx,
        server_tx,
    } = session;

    // Use the SinkStream transport adapter from rmcp.
    // PollSender's error type is PollSendError, which implements Error+Send+Sync.
    let transport = IntoTransport::<RoleServer, _, TransportAdapterSinkStream>::into_transport((
        server_tx, client_rx,
    ));

    let server = service
        .serve(transport)
        .await
        .map_err(std::io::Error::other)?;

    tracing::debug!(%session_id, "SSE session initialized");

    server.waiting().await.map_err(std::io::Error::other)?;

    tracing::debug!(%session_id, "SSE session ended");
    Ok(())
}

//! Redis Pub/Sub fanout for HTTP and WebSocket consumers.
//!
//! This module owns a single long-lived Redis Pub/Sub connection and fans incoming
//! messages out to in-process `broadcast` channels keyed by Redis channel name.
//! HTTP `/SUBSCRIBE/*channel` and WebSocket subscribers then attach to those
//! broadcast channels.
//!
//! The HTTP endpoint supports:
//! - SSE (default, modern clients)
//! - Chunked JSON stream (legacy-friendly Comet mode when JSON is negotiated)
//! - Chunked JSONP stream (legacy Comet mode when `jsonp`/`callback` is present)

use crate::format::select_jsonp_callback;
use crate::handler::AppState;
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive},
    response::{IntoResponse, Response, Sse},
};
use futures::stream::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info};

#[derive(Clone)]
/// Coordinates Redis channel subscriptions and local fanout receivers.
///
/// `PubSubManager` keeps an in-memory map from channel name to `broadcast::Sender`.
/// The first local subscriber to a channel enqueues a `Subscribe` command to the
/// background Redis Pub/Sub loop. Additional local subscribers reuse the same sender.
pub struct PubSubManager {
    cmd_tx: mpsc::Sender<Command>,
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,
}

enum Command {
    Subscribe(String),
    // Unsubscribe(String), // TODO: Implement unsubscribe cleanup
}

impl PubSubManager {
    /// Creates a new manager and spawns the Redis Pub/Sub background task.
    ///
    /// The task keeps reconnecting on failure. A dedicated Redis Pub/Sub connection
    /// is required because normal multiplexed Redis connections cannot run the
    /// blocking subscription message loop.
    pub fn new(client: redis::Client) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(100);
        let channels: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let channels_clone = channels.clone();

        tokio::spawn(async move {
            loop {
                info!("Starting Pub/Sub background task...");
                // Use get_async_pubsub() to get a dedicated connection for subscriptions.
                // Standard async connections in redis-rs are multiplexed and cannot be used
                // for blocking subscription loops.
                let mut pubsub = match client.get_async_pubsub().await {
                    Ok(pubsub) => pubsub,
                    Err(e) => {
                        error!("Failed to get Redis Pub/Sub connection: {}", e);
                        // Retry with backoff or delay to avoid tight loop on failure
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                };

                loop {
                    // Check for commands first
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            Command::Subscribe(channel) => {
                                if let Err(e) = pubsub.subscribe(&channel).await {
                                    error!("Failed to subscribe to {}: {}", channel, e);
                                } else {
                                    info!("Subscribed to {}", channel);
                                }
                            }
                        }
                    }

                    // Listen for messages with a timeout to allow checking commands periodically
                    // We create a new stream scope here so we can drop it to process commands
                    {
                        let mut stream = pubsub.on_message();
                        match tokio::time::timeout(
                            std::time::Duration::from_millis(100),
                            stream.next(),
                        )
                        .await
                        {
                            Ok(Some(msg)) => {
                                let channel_name = msg.get_channel_name().to_string();
                                let payload: String = match msg.get_payload() {
                                    Ok(p) => p,
                                    Err(e) => {
                                        error!("Failed to get payload: {}", e);
                                        continue;
                                    }
                                };

                                let map = channels_clone.read().await;
                                if let Some(sender) = map.get(&channel_name) {
                                    let _ = sender.send(payload);
                                }
                            }
                            Ok(None) => {
                                error!("Pub/Sub stream ended, reconnecting...");
                                break;
                            }
                            Err(_) => {
                                // Timeout, loop back to check commands
                                continue;
                            }
                        }
                    }
                }
            }
        });

        Self { cmd_tx, channels }
    }

    /// Subscribes to a Redis channel and returns a local message receiver.
    ///
    /// If this is the first subscriber for `channel`, a Redis `SUBSCRIBE` command
    /// is sent to the background task. Receivers can observe lag if they fall behind
    /// the broadcast buffer; callers must handle `RecvError::Lagged`.
    pub async fn subscribe(&self, channel: String) -> broadcast::Receiver<String> {
        let mut map = self.channels.write().await;
        if let Some(sender) = map.get(&channel) {
            sender.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(100);
            map.insert(channel.clone(), tx);
            let _ = self.cmd_tx.send(Command::Subscribe(channel)).await;
            rx
        }
    }
}

/// Handles HTTP Pub/Sub subscriptions on `/SUBSCRIBE/{*channel}`.
///
/// Mode selection:
/// - If `jsonp` or `callback` query parameter is present: stream chunked JSONP
///   chunks as `<callback>(<json>);\n`.
/// - Else if request `Accept` explicitly negotiates JSON and does not request SSE:
///   stream chunked JSON chunks (newline-delimited JSON documents).
/// - Else: default to SSE for compatibility with existing clients.
///
/// All modes keep the connection open and emit messages as they arrive.
pub async fn handle_subscribe(
    Path(channel): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Response {
    let mut rx = state.pubsub.subscribe(channel.clone()).await;
    let jsonp_callback = select_jsonp_callback(&params);

    if let Some(callback) = jsonp_callback {
        let channel_name = channel.clone();
        let callback_name = callback.to_string();
        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let payload = json!({
                            "SUBSCRIBE": ["message", channel_name.as_str(), msg]
                        });
                        let chunk = format!("{callback_name}({payload});\n");
                        yield Ok::<Bytes, Infallible>(Bytes::from(chunk));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let payload = json!({
                            "SUBSCRIBE": ["error", channel_name.as_str(), "lagged"]
                        });
                        let chunk = format!("{callback_name}({payload});\n");
                        yield Ok(Bytes::from(chunk));
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            )
            .body(Body::from_stream(stream))
            .unwrap();
        return with_cors(response);
    }

    if wants_chunked_json(&headers) {
        let channel_name = channel.clone();
        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let payload = json!({
                            "SUBSCRIBE": ["message", channel_name.as_str(), msg]
                        });
                        let chunk = format!("{payload}\n");
                        yield Ok::<Bytes, Infallible>(Bytes::from(chunk));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let payload = json!({
                            "SUBSCRIBE": ["error", channel_name.as_str(), "lagged"]
                        });
                        let chunk = format!("{payload}\n");
                        yield Ok(Bytes::from(chunk));
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from_stream(stream))
            .unwrap();
        return with_cors(response);
    }

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => yield Ok::<_, Infallible>(Event::default().data(msg)),
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    yield Ok(Event::default().event("error").data("lagged"))
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    with_cors(
        Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response(),
    )
}

/// Returns true when the request explicitly negotiates JSON streaming.
///
/// `Accept: text/event-stream` always wins and keeps SSE behavior.
/// We require an explicit JSON media range; wildcard-only accepts continue
/// to use SSE by default for backward compatibility.
fn wants_chunked_json(headers: &HeaderMap) -> bool {
    let Some(accept) = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()) else {
        return false;
    };

    let lowered = accept.to_ascii_lowercase();
    if lowered.contains("text/event-stream") {
        return false;
    }

    lowered.contains("application/json") || lowered.contains("application/*+json")
}

fn with_cors(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    response
}

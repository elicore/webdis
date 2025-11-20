use crate::handler::AppState;
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive},
    response::{IntoResponse, Sse},
};
use futures::stream::StreamExt; // Added this line
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info};

#[derive(Clone)]
pub struct PubSubManager {
    cmd_tx: mpsc::Sender<Command>,
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,
}

enum Command {
    Subscribe(String),
    // Unsubscribe(String), // TODO: Implement unsubscribe cleanup
}

impl PubSubManager {
    pub fn new(client: deadpool_redis::redis::Client) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(100);
        let channels: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let channels_clone = channels.clone();

        tokio::spawn(async move {
            loop {
                info!("Starting Pub/Sub background task...");
                let conn = match client.get_async_connection().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        error!("Failed to get Redis connection for Pub/Sub: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                };

                let mut pubsub = conn.into_pubsub();

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

pub async fn handle_subscribe(
    Path(channel): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut rx = state.pubsub.subscribe(channel).await;

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => yield Ok::<_, std::convert::Infallible>(Event::default().data(msg)),
                Err(broadcast::error::RecvError::Lagged(_)) => yield Ok(Event::default().event("error").data("lagged")),
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

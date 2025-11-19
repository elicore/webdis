use crate::handler::AppState;
use axum::response::sse::{Event, KeepAlive};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Sse},
};
use futures::stream;
use std::sync::Arc;
use tokio_stream::StreamExt; // Use tokio_stream::StreamExt for throttle and map

pub async fn handle_subscribe(
    Path(channel): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // This is a simplified SSE implementation for Pub/Sub
    // In a real implementation, we would need a dedicated connection for Pub/Sub
    // and a way to stream messages to the client.
    // Since deadpool-redis pools connections, we need to be careful not to block a connection forever.
    // Ideally, we should use a separate Pub/Sub manager.

    // For now, let's just return a stream that emits a message every second as a placeholder
    // to demonstrate the structure. Real Pub/Sub requires a dedicated async task.

    let stream = stream::repeat_with(|| Event::default().data("Hello, world!"))
        .map(|e| Ok::<_, std::convert::Infallible>(e))
        .throttle(std::time::Duration::from_secs(1));

    Sse::new(stream).keep_alive(KeepAlive::default())
}

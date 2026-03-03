use crate::handler::AppState;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::{SinkExt, StreamExt};
use redis::aio::MultiplexedConnection;
use redis::Value;
use redis_web_core::config::{CompatHiredisConfig, Config};
use redis_web_core::resp;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone)]
pub struct CompatSessionSettings {
    pub enabled: bool,
    pub path_prefix: String,
    pub session_ttl: Duration,
    pub max_sessions: usize,
    pub max_pipeline_commands: usize,
}

impl CompatSessionSettings {
    pub fn from_config(config: &Config) -> Self {
        let cfg = config
            .compat_hiredis
            .clone()
            .unwrap_or_else(CompatHiredisConfig::default);
        let path_prefix = normalize_prefix(&cfg.path_prefix);
        Self {
            enabled: cfg.enabled,
            path_prefix,
            session_ttl: Duration::from_secs(cfg.session_ttl_sec),
            max_sessions: cfg.max_sessions,
            max_pipeline_commands: cfg.max_pipeline_commands,
        }
    }
}

fn normalize_prefix(prefix: &str) -> String {
    let mut v = prefix.trim().to_string();
    if !v.starts_with('/') {
        v = format!("/{v}");
    }
    while v.ends_with('/') && v.len() > 1 {
        v.pop();
    }
    if v.is_empty() {
        "/__compat".to_string()
    } else {
        v
    }
}

pub struct CompatSessionManager {
    settings: CompatSessionSettings,
    command_client: redis::Client,
    pubsub_client: redis::Client,
    sessions: RwLock<HashMap<String, Arc<CompatSession>>>,
}

impl CompatSessionManager {
    pub fn new(config: &Config) -> Result<Self, redis::RedisError> {
        let settings = CompatSessionSettings::from_config(config);
        let command_client = crate::redis::create_client(config)?;
        let pubsub_client = crate::redis::create_pubsub_client(config)?;

        Ok(Self {
            settings,
            command_client,
            pubsub_client,
            sessions: RwLock::new(HashMap::new()),
        })
    }

    pub fn settings(&self) -> &CompatSessionSettings {
        &self.settings
    }

    pub async fn create_session(&self) -> Result<Arc<CompatSession>, String> {
        self.sweep_expired().await;

        {
            let sessions = self.sessions.read().await;
            if sessions.len() >= self.settings.max_sessions {
                return Err("compat session limit reached".to_string());
            }
        }

        let conn = self
            .command_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("failed to create compat command connection: {e}"))?;

        let id = Uuid::new_v4().simple().to_string();
        let session = Arc::new(CompatSession::new(
            id.clone(),
            conn,
            self.pubsub_client.clone(),
        ));

        let mut sessions = self.sessions.write().await;
        sessions.insert(id, session.clone());
        Ok(session)
    }

    pub async fn get_session(&self, id: &str) -> Option<Arc<CompatSession>> {
        self.sweep_expired().await;

        let session = {
            let sessions = self.sessions.read().await;
            sessions.get(id).cloned()
        };

        if let Some(session) = session.as_ref() {
            session.touch().await;
        }

        session
    }

    pub async fn remove_session(&self, id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id).is_some()
    }

    async fn sweep_expired(&self) {
        let mut expired = Vec::new();
        {
            let sessions = self.sessions.read().await;
            for (id, session) in sessions.iter() {
                if session.idle_for().await > self.settings.session_ttl {
                    expired.push(id.clone());
                }
            }
        }

        if expired.is_empty() {
            return;
        }

        let mut sessions = self.sessions.write().await;
        for id in expired {
            sessions.remove(&id);
        }
    }
}

pub struct CompatSession {
    id: String,
    created_at: Instant,
    last_access: Mutex<Instant>,
    command_conn: Mutex<MultiplexedConnection>,
    push_tx: broadcast::Sender<Vec<u8>>,
    pubsub_cmd_tx: Mutex<Option<mpsc::Sender<SessionPubSubCommand>>>,
    pubsub_client: redis::Client,
    http_pubsub_warning_emitted: AtomicBool,
    has_pubsub: AtomicBool,
}

impl CompatSession {
    fn new(id: String, conn: MultiplexedConnection, pubsub_client: redis::Client) -> Self {
        let (push_tx, _) = broadcast::channel(1024);
        Self {
            id,
            created_at: Instant::now(),
            last_access: Mutex::new(Instant::now()),
            command_conn: Mutex::new(conn),
            push_tx,
            pubsub_cmd_tx: Mutex::new(None),
            pubsub_client,
            http_pubsub_warning_emitted: AtomicBool::new(false),
            has_pubsub: AtomicBool::new(false),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    async fn touch(&self) {
        let mut last = self.last_access.lock().await;
        *last = Instant::now();
    }

    async fn idle_for(&self) -> Duration {
        let last = self.last_access.lock().await;
        Instant::now().saturating_duration_since(*last)
    }

    fn subscribe_receiver(&self) -> broadcast::Receiver<Vec<u8>> {
        self.push_tx.subscribe()
    }

    fn maybe_emit_http_pubsub_warning(&self) {
        if !self.has_pubsub.load(Ordering::Relaxed) {
            return;
        }

        if std::env::var("REDIS_WEB_COMPAT_MUTE_HTTP_PUBSUB_WARNING")
            .ok()
            .as_deref()
            == Some("1")
        {
            return;
        }

        if self
            .http_pubsub_warning_emitted
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            warn!(
                session_id = %self.id,
                "compat pub/sub stream is using HTTP fallback; set REDIS_WEB_COMPAT_MUTE_HTTP_PUBSUB_WARNING=1 to mute"
            );
        }
    }

    async fn execute_command(&self, args: Vec<Vec<u8>>) -> Vec<u8> {
        self.touch().await;

        if args.is_empty() {
            return b"-ERR Empty command\r\n".to_vec();
        }

        let command_name = String::from_utf8_lossy(&args[0]).to_ascii_uppercase();
        match command_name.as_str() {
            "SUBSCRIBE" | "PSUBSCRIBE" | "UNSUBSCRIBE" | "PUNSUBSCRIBE" => {
                self.execute_pubsub_control(command_name, args[1..].to_vec())
                    .await
            }
            _ => {
                let mut redis_cmd = redis::cmd(&command_name);
                for arg in args.iter().skip(1) {
                    redis_cmd.arg(arg);
                }

                let mut conn = self.command_conn.lock().await;
                let result: Result<Value, _> = redis_cmd.query_async(&mut *conn).await;
                match result {
                    Ok(value) => resp::value_to_resp(&value),
                    Err(error) => format!("-ERR {}\r\n", error).into_bytes(),
                }
            }
        }
    }

    async fn execute_pubsub_control(&self, command: String, args: Vec<Vec<u8>>) -> Vec<u8> {
        self.has_pubsub.store(true, Ordering::Relaxed);

        let tx = match self.ensure_pubsub_task().await {
            Ok(tx) => tx,
            Err(error) => return format!("-ERR {}\r\n", error).into_bytes(),
        };

        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        if tx
            .send(SessionPubSubCommand {
                op: command,
                args,
                responder: resp_tx,
            })
            .await
            .is_err()
        {
            return b"-ERR pubsub session is unavailable\r\n".to_vec();
        }

        match resp_rx.await {
            Ok(Ok(frames)) => frames.concat(),
            Ok(Err(error)) => format!("-ERR {}\r\n", error).into_bytes(),
            Err(_) => b"-ERR pubsub response channel closed\r\n".to_vec(),
        }
    }

    async fn ensure_pubsub_task(&self) -> Result<mpsc::Sender<SessionPubSubCommand>, String> {
        {
            let guard = self.pubsub_cmd_tx.lock().await;
            if let Some(tx) = guard.as_ref() {
                return Ok(tx.clone());
            }
        }

        let (tx, mut rx) = mpsc::channel::<SessionPubSubCommand>(64);
        let push_tx = self.push_tx.clone();
        let client = self.pubsub_client.clone();

        tokio::spawn(async move {
            let mut pubsub = match client.get_async_pubsub().await {
                Ok(pubsub) => pubsub,
                Err(error) => {
                    let _ = push_tx
                        .send(format!("-ERR pubsub init failed: {}\r\n", error).into_bytes());
                    return;
                }
            };

            let mut channels = HashSet::<Vec<u8>>::new();
            let mut patterns = HashSet::<Vec<u8>>::new();

            loop {
                while let Ok(cmd) = rx.try_recv() {
                    let result = handle_pubsub_command(
                        &mut pubsub,
                        &mut channels,
                        &mut patterns,
                        cmd.op,
                        cmd.args,
                    )
                    .await;
                    let _ = cmd.responder.send(result);
                }

                {
                    let mut stream = pubsub.on_message();
                    match tokio::time::timeout(Duration::from_millis(100), stream.next()).await {
                        Ok(Some(msg)) => {
                            let frame = if msg.from_pattern() {
                                let pattern: Vec<u8> = msg.get_pattern().unwrap_or_default();
                                let channel = msg.get_channel_name().as_bytes().to_vec();
                                let payload = msg.get_payload_bytes().to_vec();
                                resp::value_to_resp(&Value::Array(vec![
                                    Value::BulkString(b"pmessage".to_vec()),
                                    Value::BulkString(pattern),
                                    Value::BulkString(channel),
                                    Value::BulkString(payload),
                                ]))
                            } else {
                                let channel = msg.get_channel_name().as_bytes().to_vec();
                                let payload = msg.get_payload_bytes().to_vec();
                                resp::value_to_resp(&Value::Array(vec![
                                    Value::BulkString(b"message".to_vec()),
                                    Value::BulkString(channel),
                                    Value::BulkString(payload),
                                ]))
                            };
                            let _ = push_tx.send(frame);
                        }
                        Ok(None) => {
                            let _ = push_tx.send(b"-ERR pubsub stream ended\r\n".to_vec());
                            return;
                        }
                        Err(_) => {
                            if rx.is_closed() {
                                return;
                            }
                        }
                    }
                }
            }
        });

        let mut guard = self.pubsub_cmd_tx.lock().await;
        *guard = Some(tx.clone());
        Ok(tx)
    }
}

struct SessionPubSubCommand {
    op: String,
    args: Vec<Vec<u8>>,
    responder: tokio::sync::oneshot::Sender<Result<Vec<Vec<u8>>, String>>,
}

async fn handle_pubsub_command(
    pubsub: &mut redis::aio::PubSub,
    channels: &mut HashSet<Vec<u8>>,
    patterns: &mut HashSet<Vec<u8>>,
    op: String,
    args: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, String> {
    let mut replies = Vec::new();

    match op.as_str() {
        "SUBSCRIBE" => {
            if args.is_empty() {
                return Err("SUBSCRIBE requires at least one channel".to_string());
            }
            for channel in args {
                pubsub
                    .subscribe(channel.clone())
                    .await
                    .map_err(|e| e.to_string())?;
                channels.insert(channel.clone());
                let count = (channels.len() + patterns.len()) as i64;
                replies.push(resp::value_to_resp(&Value::Array(vec![
                    Value::BulkString(b"subscribe".to_vec()),
                    Value::BulkString(channel),
                    Value::Int(count),
                ])));
            }
        }
        "PSUBSCRIBE" => {
            if args.is_empty() {
                return Err("PSUBSCRIBE requires at least one pattern".to_string());
            }
            for pattern in args {
                pubsub
                    .psubscribe(pattern.clone())
                    .await
                    .map_err(|e| e.to_string())?;
                patterns.insert(pattern.clone());
                let count = (channels.len() + patterns.len()) as i64;
                replies.push(resp::value_to_resp(&Value::Array(vec![
                    Value::BulkString(b"psubscribe".to_vec()),
                    Value::BulkString(pattern),
                    Value::Int(count),
                ])));
            }
        }
        "UNSUBSCRIBE" => {
            let targets = if args.is_empty() {
                channels.iter().cloned().collect::<Vec<_>>()
            } else {
                args
            };

            if targets.is_empty() {
                replies.push(resp::value_to_resp(&Value::Array(vec![
                    Value::BulkString(b"unsubscribe".to_vec()),
                    Value::BulkString(Vec::new()),
                    Value::Int((channels.len() + patterns.len()) as i64),
                ])));
            } else {
                for channel in targets {
                    pubsub
                        .unsubscribe(channel.clone())
                        .await
                        .map_err(|e| e.to_string())?;
                    channels.remove(&channel);
                    let count = (channels.len() + patterns.len()) as i64;
                    replies.push(resp::value_to_resp(&Value::Array(vec![
                        Value::BulkString(b"unsubscribe".to_vec()),
                        Value::BulkString(channel),
                        Value::Int(count),
                    ])));
                }
            }
        }
        "PUNSUBSCRIBE" => {
            let targets = if args.is_empty() {
                patterns.iter().cloned().collect::<Vec<_>>()
            } else {
                args
            };

            if targets.is_empty() {
                replies.push(resp::value_to_resp(&Value::Array(vec![
                    Value::BulkString(b"punsubscribe".to_vec()),
                    Value::BulkString(Vec::new()),
                    Value::Int((channels.len() + patterns.len()) as i64),
                ])));
            } else {
                for pattern in targets {
                    pubsub
                        .punsubscribe(pattern.clone())
                        .await
                        .map_err(|e| e.to_string())?;
                    patterns.remove(&pattern);
                    let count = (channels.len() + patterns.len()) as i64;
                    replies.push(resp::value_to_resp(&Value::Array(vec![
                        Value::BulkString(b"punsubscribe".to_vec()),
                        Value::BulkString(pattern),
                        Value::Int(count),
                    ])));
                }
            }
        }
        _ => return Err(format!("unsupported pubsub command: {}", op)),
    }

    Ok(replies)
}

pub async fn create_session(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(manager) = state.compat_hiredis.clone() else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("compat disabled")).into_response());
    };

    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1");

    match manager.create_session().await {
        Ok(session) => {
            let prefix = manager.settings().path_prefix.clone();
            let id = session.id().to_string();
            let body = json!({
                "session_id": id,
                "created_at_ms": session.created_at().elapsed().as_millis(),
                "ws_url": format!("ws://{host}{prefix}/ws/{}", session.id()),
                "cmd_url": format!("http://{host}{prefix}/cmd/{}.raw", session.id()),
                "stream_url": format!("http://{host}{prefix}/stream/{}.raw", session.id()),
                "session_ttl_sec": manager.settings().session_ttl.as_secs()
            });
            with_cors((StatusCode::CREATED, axum::Json(body)).into_response())
        }
        Err(error) => with_cors(
            (
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(json!({"error": error})),
            )
                .into_response(),
        ),
    }
}

pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Response {
    let Some(manager) = state.compat_hiredis.clone() else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("compat disabled")).into_response());
    };

    if manager.remove_session(&session_id).await {
        with_cors(StatusCode::NO_CONTENT.into_response())
    } else {
        with_cors((StatusCode::NOT_FOUND, Body::from("session not found")).into_response())
    }
}

pub async fn command_raw(
    State(state): State<Arc<AppState>>,
    Path(session_path): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(manager) = state.compat_hiredis.clone() else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("compat disabled")).into_response());
    };

    let session_id = normalize_raw_session_path(session_path);
    let Some(session) = manager.get_session(&session_id).await else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("session not found")).into_response());
    };

    let auth_header = extract_auth_header(&headers);

    let mut buffer = body.to_vec();
    if buffer.is_empty() {
        return with_cors(resp_plain(
            StatusCode::BAD_REQUEST,
            b"-ERR Empty command body\r\n".to_vec(),
        ));
    }

    let mut out = Vec::new();
    let mut command_count = 0usize;
    loop {
        if buffer.is_empty() {
            break;
        }
        match parse_next_command(&mut buffer) {
            ParseResult::Command(args) => {
                command_count += 1;
                if command_count > manager.settings().max_pipeline_commands {
                    return with_cors(resp_plain(
                        StatusCode::BAD_REQUEST,
                        b"-ERR Pipelined command limit exceeded\r\n".to_vec(),
                    ));
                }

                if !is_command_allowed(&state, addr, auth_header.as_deref(), &args) {
                    out.extend_from_slice(b"-ERR forbidden\r\n");
                    continue;
                }

                let frame = session.execute_command(args).await;
                out.extend_from_slice(&frame);
            }
            ParseResult::NeedMore => {
                return with_cors(resp_plain(
                    StatusCode::BAD_REQUEST,
                    b"-ERR Incomplete RESP command\r\n".to_vec(),
                ));
            }
            ParseResult::Invalid => {
                return with_cors(resp_plain(
                    StatusCode::BAD_REQUEST,
                    b"-ERR Invalid RESP command\r\n".to_vec(),
                ));
            }
        }
    }

    with_cors(resp_plain(StatusCode::OK, out))
}

pub async fn stream_raw(
    State(state): State<Arc<AppState>>,
    Path(session_path): Path<String>,
) -> Response {
    let Some(manager) = state.compat_hiredis.clone() else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("compat disabled")).into_response());
    };

    let session_id = normalize_raw_session_path(session_path);
    let Some(session) = manager.get_session(&session_id).await else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("session not found")).into_response());
    };

    session.maybe_emit_http_pubsub_warning();

    let mut rx = session.subscribe_receiver();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(frame) => yield Ok::<Bytes, Infallible>(Bytes::from(frame)),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from_stream(stream))
        .unwrap();

    with_cors(response)
}

pub async fn ws_raw(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let Some(manager) = state.compat_hiredis.clone() else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("compat disabled")).into_response());
    };

    let Some(session) = manager.get_session(&session_id).await else {
        return with_cors((StatusCode::NOT_FOUND, Body::from("session not found")).into_response());
    };

    let auth_header = extract_auth_header(&headers);

    ws.on_upgrade(move |socket| handle_ws(socket, state, session, addr, auth_header))
}

async fn handle_ws(
    socket: WebSocket,
    state: Arc<AppState>,
    session: Arc<CompatSession>,
    addr: SocketAddr,
    auth_header: Option<String>,
) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(128);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut push_rx = session.subscribe_receiver();
    let tx_push = tx.clone();
    tokio::spawn(async move {
        loop {
            match push_rx.recv().await {
                Ok(frame) => {
                    if tx_push.send(Message::Binary(frame.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let mut buffer = Vec::new();
    while let Some(next) = receiver.next().await {
        let Ok(message) = next else {
            return;
        };

        match message {
            Message::Binary(data) => buffer.extend_from_slice(&data),
            Message::Text(text) => buffer.extend_from_slice(text.as_bytes()),
            Message::Close(_) => return,
            Message::Ping(payload) => {
                if tx.send(Message::Pong(payload)).await.is_err() {
                    return;
                }
                continue;
            }
            _ => continue,
        }

        loop {
            match parse_next_command(&mut buffer) {
                ParseResult::Command(args) => {
                    if !is_command_allowed(&state, addr, auth_header.as_deref(), &args) {
                        if tx
                            .send(Message::Binary(b"-ERR forbidden\r\n".to_vec().into()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                        continue;
                    }

                    let frame = session.execute_command(args).await;
                    if tx.send(Message::Binary(frame.into())).await.is_err() {
                        return;
                    }
                }
                ParseResult::NeedMore => break,
                ParseResult::Invalid => {
                    let _ = tx
                        .send(Message::Binary(b"-ERR Invalid RESP\r\n".to_vec().into()))
                        .await;
                    buffer.clear();
                    break;
                }
            }
        }
    }
}

fn is_command_allowed(
    state: &Arc<AppState>,
    addr: SocketAddr,
    auth_header: Option<&str>,
    args: &[Vec<u8>],
) -> bool {
    if args.is_empty() {
        return true;
    }

    let command_name = String::from_utf8_lossy(&args[0]);
    state
        .acl
        .check(addr.ip(), command_name.as_ref(), auth_header)
}

fn extract_auth_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
}

enum ParseResult {
    Command(Vec<Vec<u8>>),
    NeedMore,
    Invalid,
}

fn parse_next_command(buffer: &mut Vec<u8>) -> ParseResult {
    match resp::parse_command(buffer.as_slice()) {
        Ok(Some((args, consumed))) => {
            buffer.drain(..consumed);
            ParseResult::Command(args)
        }
        Ok(None) => ParseResult::NeedMore,
        Err(_) => ParseResult::Invalid,
    }
}

fn normalize_raw_session_path(raw: String) -> String {
    raw.strip_suffix(".raw").unwrap_or(raw.as_str()).to_string()
}

fn resp_plain(status: StatusCode, payload: Vec<u8>) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(payload))
        .unwrap()
}

fn with_cors(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    response
}

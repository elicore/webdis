//! Benchmark suite execution.

use crate::model::{
    CommandSuiteConfig, MetricSummary, StreamingSuiteConfig, SuiteResult, SuiteStatus,
    VariantBenchmarkResult, VariantRunContext, WebSocketSuiteConfig, WorkloadResult,
};
use crate::process::LaunchedServer;
use crate::summary::redis_endpoint_summary;
use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use redis::aio::MultiplexedConnection;
use redis_web_core::config::{Config, TransportMode};
use redis_web_runtime::grpc::proto::{
    redis_gateway_client::RedisGatewayClient, CommandRequest, SubscribeRequest,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tonic::Request;

const STREAM_PUBLISH_YIELD_INTERVAL: u64 = 16;
const STREAM_MAX_IN_FLIGHT_MESSAGES: usize = 32;
const STREAM_READY_SENTINEL: &str = "__redis_web_bench_ready__";
const STREAM_READY_ATTEMPTS: usize = 50;
const STREAM_READY_TIMEOUT: Duration = Duration::from_millis(200);
const STREAM_DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn benchmark_variant(
    context: VariantRunContext,
    common_commands: &CommandSuiteConfig,
    websocket_commands: &WebSocketSuiteConfig,
    streaming: &StreamingSuiteConfig,
    workspace_root: &std::path::Path,
) -> Result<VariantBenchmarkResult> {
    let server = LaunchedServer::start(&context.config, workspace_root)
        .await
        .with_context(|| format!("failed to start redis-web for variant `{}`", context.name))?;

    let suites = vec![
        run_common_commands_suite(&context.name, &context.config, &server, common_commands).await?,
        run_websocket_commands_suite(&context.name, &context.config, &server, websocket_commands)
            .await?,
        run_streaming_suite(&context.name, &context.config, &server, streaming).await?,
    ];

    Ok(VariantBenchmarkResult {
        name: context.name,
        redis_endpoint: redis_endpoint_summary(&context.config),
        transport_mode: transport_mode_name(context.config.transport_mode).to_string(),
        config_diff: context.diff,
        suites,
    })
}

fn transport_mode_name(mode: TransportMode) -> &'static str {
    match mode {
        TransportMode::Rest => "rest",
        TransportMode::Grpc => "grpc",
    }
}

async fn run_common_commands_suite(
    variant_name: &str,
    config: &Config,
    server: &LaunchedServer,
    suite: &CommandSuiteConfig,
) -> Result<SuiteResult> {
    let workloads = match config.transport_mode {
        TransportMode::Rest => {
            run_http_common_commands(variant_name, server.http_base_url(), suite).await?
        }
        TransportMode::Grpc => {
            run_grpc_common_commands(variant_name, server.grpc_endpoint(), suite).await?
        }
    };

    Ok(SuiteResult {
        suite: "common_commands".to_string(),
        status: SuiteStatus::Completed,
        workloads,
    })
}

async fn run_http_common_commands(
    variant_name: &str,
    base_url: String,
    suite: &CommandSuiteConfig,
) -> Result<Vec<WorkloadResult>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    for _ in 0..suite.warmup_ops {
        client
            .get(format!("{base_url}/PING.raw"))
            .send()
            .await?
            .error_for_status()?;
    }

    let ping_metrics = measure_http_parallel(suite, {
        let client = client.clone();
        let base_url = base_url.clone();
        move |_worker, _idx| {
            let client = client.clone();
            let url = format!("{base_url}/PING.raw");
            async move {
                client.get(url).send().await?.error_for_status()?;
                Ok(())
            }
        }
    })
    .await?;

    let small_payload = "small-value".to_string();
    for idx in 0..suite.warmup_ops {
        let key = format!("redis-web-bench:{variant_name}:http:small:warmup:{idx}");
        http_set_get_once(&client, &base_url, &key, &small_payload).await?;
    }
    let small_metrics = measure_http_parallel(suite, {
        let client = client.clone();
        let base_url = base_url.clone();
        let variant_name = variant_name.to_string();
        let payload = small_payload.clone();
        move |worker, idx| {
            let client = client.clone();
            let base_url = base_url.clone();
            let variant_name = variant_name.clone();
            let payload = payload.clone();
            async move {
                let key = format!("redis-web-bench:{variant_name}:http:small:{worker}:{idx}");
                http_set_get_once(&client, &base_url, &key, &payload).await
            }
        }
    })
    .await?;

    let medium_payload = "m".repeat(4096);
    for idx in 0..suite.warmup_ops {
        let key = format!("redis-web-bench:{variant_name}:http:medium:warmup:{idx}");
        http_set_get_once(&client, &base_url, &key, &medium_payload).await?;
    }
    let medium_metrics = measure_http_parallel(suite, {
        let client = client.clone();
        let base_url = base_url.clone();
        let variant_name = variant_name.to_string();
        let payload = medium_payload.clone();
        move |worker, idx| {
            let client = client.clone();
            let base_url = base_url.clone();
            let variant_name = variant_name.clone();
            let payload = payload.clone();
            async move {
                let key = format!("redis-web-bench:{variant_name}:http:medium:{worker}:{idx}");
                http_set_get_once(&client, &base_url, &key, &payload).await
            }
        }
    })
    .await?;

    Ok(vec![
        completed_workload("ping", ping_metrics),
        completed_workload("small_set_get", small_metrics),
        completed_workload("medium_set_get", medium_metrics),
    ])
}

async fn measure_http_parallel<F, Fut>(suite: &CommandSuiteConfig, op: F) -> Result<MetricSummary>
where
    F: Fn(usize, u64) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let op = Arc::new(op);
    let counter = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(
        suite.measured_ops as usize,
    )));
    let error_count = Arc::new(AtomicU64::new(0));
    let mut join_set = JoinSet::new();
    let start = Instant::now();

    for worker in 0..suite.concurrency {
        let op = op.clone();
        let counter = counter.clone();
        let latencies = latencies.clone();
        let error_count = error_count.clone();
        let measured_ops = suite.measured_ops;
        join_set.spawn(async move {
            loop {
                let idx = counter.fetch_add(1, Ordering::Relaxed);
                if idx >= measured_ops {
                    break;
                }
                let op_start = Instant::now();
                match op(worker, idx).await {
                    Ok(()) => latencies.lock().await.push(op_start.elapsed()),
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.context("HTTP worker task panicked")?;
    }

    let latencies = Arc::into_inner(latencies)
        .expect("latencies still referenced")
        .into_inner();
    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.measured_ops,
        error_count.load(Ordering::Relaxed),
    ))
}

async fn http_set_get_once(
    client: &reqwest::Client,
    base_url: &str,
    key: &str,
    payload: &str,
) -> Result<()> {
    client
        .put(format!("{base_url}/SET/{key}"))
        .body(payload.to_string())
        .send()
        .await?
        .error_for_status()?;
    client
        .get(format!("{base_url}/GET/{key}.raw"))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn run_grpc_common_commands(
    variant_name: &str,
    endpoint: String,
    suite: &CommandSuiteConfig,
) -> Result<Vec<WorkloadResult>> {
    {
        let mut client = RedisGatewayClient::connect(endpoint.clone()).await?;
        for _ in 0..suite.warmup_ops {
            grpc_ping_once(&mut client).await?;
        }
    }
    let ping_metrics = measure_grpc_ping_parallel(suite, endpoint.clone()).await?;

    {
        let mut client = RedisGatewayClient::connect(endpoint.clone()).await?;
        for idx in 0..suite.warmup_ops {
            let key = format!("redis-web-bench:{variant_name}:grpc:small:warmup:{idx}");
            grpc_set_get_once(&mut client, &key, b"small-value".to_vec()).await?;
        }
    }
    let small_metrics = measure_grpc_set_get_parallel(
        suite,
        endpoint.clone(),
        variant_name.to_string(),
        "small",
        b"small-value".to_vec(),
    )
    .await?;

    {
        let mut client = RedisGatewayClient::connect(endpoint.clone()).await?;
        let payload = vec![b'm'; 4096];
        for idx in 0..suite.warmup_ops {
            let key = format!("redis-web-bench:{variant_name}:grpc:medium:warmup:{idx}");
            grpc_set_get_once(&mut client, &key, payload.clone()).await?;
        }
    }
    let medium_metrics = measure_grpc_set_get_parallel(
        suite,
        endpoint,
        variant_name.to_string(),
        "medium",
        vec![b'm'; 4096],
    )
    .await?;

    Ok(vec![
        completed_workload("ping", ping_metrics),
        completed_workload("small_set_get", small_metrics),
        completed_workload("medium_set_get", medium_metrics),
    ])
}

async fn measure_grpc_ping_parallel(
    suite: &CommandSuiteConfig,
    endpoint: String,
) -> Result<MetricSummary> {
    let counter = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(
        suite.measured_ops as usize,
    )));
    let error_count = Arc::new(AtomicU64::new(0));
    let mut join_set = JoinSet::new();
    let start = Instant::now();

    for _worker in 0..suite.concurrency {
        let endpoint = endpoint.clone();
        let counter = counter.clone();
        let latencies = latencies.clone();
        let error_count = error_count.clone();
        let measured_ops = suite.measured_ops;
        join_set.spawn(async move {
            let mut client = RedisGatewayClient::connect(endpoint).await?;
            loop {
                let idx = counter.fetch_add(1, Ordering::Relaxed);
                if idx >= measured_ops {
                    break;
                }
                let op_start = Instant::now();
                match grpc_ping_once(&mut client).await {
                    Ok(()) => latencies.lock().await.push(op_start.elapsed()),
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Result::<()>::Ok(())
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.context("gRPC worker task panicked")??;
    }

    let latencies = Arc::into_inner(latencies)
        .expect("latencies still referenced")
        .into_inner();
    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.measured_ops,
        error_count.load(Ordering::Relaxed),
    ))
}

async fn measure_grpc_set_get_parallel(
    suite: &CommandSuiteConfig,
    endpoint: String,
    variant_name: String,
    label: &'static str,
    payload: Vec<u8>,
) -> Result<MetricSummary> {
    let counter = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(
        suite.measured_ops as usize,
    )));
    let error_count = Arc::new(AtomicU64::new(0));
    let mut join_set = JoinSet::new();
    let start = Instant::now();

    for worker in 0..suite.concurrency {
        let endpoint = endpoint.clone();
        let variant_name = variant_name.clone();
        let payload = payload.clone();
        let counter = counter.clone();
        let latencies = latencies.clone();
        let error_count = error_count.clone();
        let measured_ops = suite.measured_ops;
        join_set.spawn(async move {
            let mut client = RedisGatewayClient::connect(endpoint).await?;
            loop {
                let idx = counter.fetch_add(1, Ordering::Relaxed);
                if idx >= measured_ops {
                    break;
                }
                let key = format!("redis-web-bench:{variant_name}:grpc:{label}:{worker}:{idx}");
                let op_start = Instant::now();
                match grpc_set_get_once(&mut client, &key, payload.clone()).await {
                    Ok(()) => latencies.lock().await.push(op_start.elapsed()),
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Result::<()>::Ok(())
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.context("gRPC worker task panicked")??;
    }

    let latencies = Arc::into_inner(latencies)
        .expect("latencies still referenced")
        .into_inner();
    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.measured_ops,
        error_count.load(Ordering::Relaxed),
    ))
}

async fn grpc_ping_once(client: &mut RedisGatewayClient<tonic::transport::Channel>) -> Result<()> {
    client
        .execute(CommandRequest {
            command: "PING".to_string(),
            database: None,
            args: Vec::new(),
        })
        .await?;
    Ok(())
}

async fn grpc_set_get_once(
    client: &mut RedisGatewayClient<tonic::transport::Channel>,
    key: &str,
    payload: Vec<u8>,
) -> Result<()> {
    client
        .execute(CommandRequest {
            command: "SET".to_string(),
            database: None,
            args: vec![key.as_bytes().to_vec(), payload],
        })
        .await?;
    client
        .execute(CommandRequest {
            command: "GET".to_string(),
            database: None,
            args: vec![key.as_bytes().to_vec()],
        })
        .await?;
    Ok(())
}

async fn run_websocket_commands_suite(
    variant_name: &str,
    config: &Config,
    server: &LaunchedServer,
    suite: &WebSocketSuiteConfig,
) -> Result<SuiteResult> {
    if config.transport_mode != TransportMode::Rest || !config.websockets {
        return Ok(skipped_suite(
            "websocket_commands",
            "requires `transport_mode = rest` and `websockets = true`",
        ));
    }

    let ws_url = server.websocket_url();
    let ping_metrics =
        websocket_roundtrip_workload(&ws_url, suite, variant_name, "ping", WebSocketPayload::Ping)
            .await?;
    let small_metrics = websocket_roundtrip_workload(
        &ws_url,
        suite,
        variant_name,
        "small_set_get",
        WebSocketPayload::SetGet {
            value: "small-value".to_string(),
        },
    )
    .await?;
    let medium_metrics = websocket_roundtrip_workload(
        &ws_url,
        suite,
        variant_name,
        "medium_set_get",
        WebSocketPayload::SetGet {
            value: "m".repeat(4096),
        },
    )
    .await?;

    Ok(SuiteResult {
        suite: "websocket_commands".to_string(),
        status: SuiteStatus::Completed,
        workloads: vec![
            completed_workload("ping", ping_metrics),
            completed_workload("small_set_get", small_metrics),
            completed_workload("medium_set_get", medium_metrics),
        ],
    })
}

#[derive(Clone)]
enum WebSocketPayload {
    Ping,
    SetGet { value: String },
}

async fn websocket_roundtrip_workload(
    ws_url: &str,
    suite: &WebSocketSuiteConfig,
    variant_name: &str,
    label: &str,
    payload: WebSocketPayload,
) -> Result<MetricSummary> {
    for warmup_idx in 0..suite.warmup_ops {
        let (mut socket, _) = connect_async(ws_url).await?;
        run_websocket_operation(
            &mut socket,
            variant_name,
            label,
            0,
            warmup_idx,
            payload.clone(),
        )
        .await?;
    }

    let counter = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(
        suite.measured_ops as usize,
    )));
    let error_count = Arc::new(AtomicU64::new(0));
    let mut join_set = JoinSet::new();
    let start = Instant::now();

    for worker in 0..suite.persistent_connections {
        let ws_url = ws_url.to_string();
        let variant_name = variant_name.to_string();
        let label = label.to_string();
        let payload = payload.clone();
        let counter = counter.clone();
        let latencies = latencies.clone();
        let error_count = error_count.clone();
        let measured_ops = suite.measured_ops;
        join_set.spawn(async move {
            let (mut socket, _) = connect_async(ws_url).await?;
            loop {
                let idx = counter.fetch_add(1, Ordering::Relaxed);
                if idx >= measured_ops {
                    break;
                }
                let op_start = Instant::now();
                match run_websocket_operation(
                    &mut socket,
                    &variant_name,
                    &label,
                    worker,
                    idx,
                    payload.clone(),
                )
                .await
                {
                    Ok(()) => latencies.lock().await.push(op_start.elapsed()),
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Result::<()>::Ok(())
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.context("websocket worker panicked")??;
    }

    let latencies = Arc::into_inner(latencies)
        .expect("latencies still referenced")
        .into_inner();
    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.measured_ops,
        error_count.load(Ordering::Relaxed),
    ))
}

async fn run_websocket_operation(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    variant_name: &str,
    label: &str,
    worker: usize,
    idx: u64,
    payload: WebSocketPayload,
) -> Result<()> {
    match payload {
        WebSocketPayload::Ping => {
            socket
                .send(Message::Text(serde_json::to_string(&vec!["PING"])?.into()))
                .await?;
            let _ = next_websocket_text(socket).await?;
        }
        WebSocketPayload::SetGet { value } => {
            let key = format!("redis-web-bench:{variant_name}:ws:{label}:{worker}:{idx}");
            socket
                .send(Message::Text(
                    serde_json::to_string(&vec!["SET", key.as_str(), value.as_str()])?.into(),
                ))
                .await?;
            let _ = next_websocket_text(socket).await?;
            socket
                .send(Message::Text(
                    serde_json::to_string(&vec!["GET", key.as_str()])?.into(),
                ))
                .await?;
            let _ = next_websocket_text(socket).await?;
        }
    }
    Ok(())
}

async fn next_websocket_text(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<String> {
    let message = timeout(Duration::from_secs(5), socket.next())
        .await
        .context("timed out waiting for websocket response")?
        .ok_or_else(|| anyhow!("websocket stream ended unexpectedly"))??;
    match message {
        Message::Text(text) => Ok(text.to_string()),
        Message::Binary(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        other => Err(anyhow!("unexpected websocket message: {other:?}")),
    }
}

async fn run_streaming_suite(
    variant_name: &str,
    config: &Config,
    server: &LaunchedServer,
    suite: &StreamingSuiteConfig,
) -> Result<SuiteResult> {
    let workloads = match config.transport_mode {
        TransportMode::Rest => {
            let base_url = server.http_base_url();
            vec![
                completed_workload(
                    "subscribe_startup",
                    sse_startup_workload(variant_name, &base_url, suite, config).await?,
                ),
                completed_workload(
                    "delivery_stream",
                    sse_delivery_workload(variant_name, &base_url, suite, config).await?,
                ),
            ]
        }
        TransportMode::Grpc => {
            let endpoint = server.grpc_endpoint();
            vec![
                completed_workload(
                    "subscribe_startup",
                    grpc_startup_workload(variant_name, &endpoint, suite, config).await?,
                ),
                completed_workload(
                    "delivery_stream",
                    grpc_delivery_workload(variant_name, &endpoint, suite, config).await?,
                ),
            ]
        }
    };

    Ok(SuiteResult {
        suite: "streaming_pubsub".to_string(),
        status: SuiteStatus::Completed,
        workloads,
    })
}

async fn sse_startup_workload(
    variant_name: &str,
    base_url: &str,
    suite: &StreamingSuiteConfig,
    config: &Config,
) -> Result<MetricSummary> {
    let client = reqwest::Client::builder().build()?;
    let mut publisher = redis_connection(config).await?;

    for idx in 0..suite.startup_warmup_ops {
        let channel = format!("redis-web-bench:{variant_name}:sse-startup:warmup:{idx}");
        measure_sse_startup_once(
            &client,
            base_url,
            &mut publisher,
            &channel,
            &format!("warmup-{idx}"),
        )
        .await?;
    }

    let mut latencies = Vec::with_capacity(suite.startup_measured_ops as usize);
    let start = Instant::now();
    let mut errors = 0;
    for idx in 0..suite.startup_measured_ops {
        let channel = format!("redis-web-bench:{variant_name}:sse-startup:{idx}");
        match measure_sse_startup_once(
            &client,
            base_url,
            &mut publisher,
            &channel,
            &format!("measured-{idx}"),
        )
        .await
        {
            Ok(latency) => latencies.push(latency),
            Err(_) => errors += 1,
        }
    }

    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.startup_measured_ops,
        errors,
    ))
}

async fn measure_sse_startup_once(
    client: &reqwest::Client,
    base_url: &str,
    publisher: &mut MultiplexedConnection,
    channel: &str,
    payload: &str,
) -> Result<Duration> {
    let started = Instant::now();
    let mut response = client
        .get(format!("{base_url}/SUBSCRIBE/{channel}"))
        .send()
        .await?
        .error_for_status()?;
    wait_for_sse_subscription_ready(&mut response, publisher, channel).await?;

    for _ in 0..STREAM_READY_ATTEMPTS {
        publish_message(publisher, channel, payload).await?;
        match read_next_sse_payload_with_timeout(&mut response, STREAM_READY_TIMEOUT).await {
            Ok(received) if received == payload => return Ok(started.elapsed()),
            Ok(_) => continue,
            Err(_) => continue,
        }
    }

    bail!("timed out waiting for SSE startup payload `{payload}`")
}

async fn sse_delivery_workload(
    variant_name: &str,
    base_url: &str,
    suite: &StreamingSuiteConfig,
    config: &Config,
) -> Result<MetricSummary> {
    let client = reqwest::Client::builder().build()?;
    let mut warmup_publisher = redis_connection(config).await?;
    let channel = format!("redis-web-bench:{variant_name}:sse-stream");
    let mut response = client
        .get(format!("{base_url}/SUBSCRIBE/{channel}"))
        .send()
        .await?
        .error_for_status()?;
    wait_for_sse_subscription_ready(&mut response, &mut warmup_publisher, &channel).await?;

    for idx in 0..suite.warmup_messages {
        let payload = format!("warmup:{idx}");
        publish_message(&mut warmup_publisher, &channel, &payload).await?;
        let _ = read_next_sse_payload_with_timeout(&mut response, STREAM_DELIVERY_TIMEOUT).await?;
    }

    let send_times = Arc::new(Mutex::new(HashMap::<u64, Instant>::with_capacity(
        suite.measured_messages as usize,
    )));
    let mut measured_publisher = redis_connection(config).await?;
    let channel_for_publish = channel.clone();
    let send_times_for_publish = send_times.clone();
    let measured_messages = suite.measured_messages;
    let publish_task = tokio::spawn(async move {
        for idx in 0..measured_messages {
            loop {
                if send_times_for_publish.lock().unwrap().len() < STREAM_MAX_IN_FLIGHT_MESSAGES {
                    break;
                }
                tokio::task::yield_now().await;
            }
            send_times_for_publish
                .lock()
                .unwrap()
                .insert(idx, Instant::now());
            let payload = format!("{idx}");
            publish_message(&mut measured_publisher, &channel_for_publish, &payload).await?;
            if idx % STREAM_PUBLISH_YIELD_INTERVAL == STREAM_PUBLISH_YIELD_INTERVAL - 1 {
                tokio::task::yield_now().await;
            }
        }
        Result::<()>::Ok(())
    });

    let start = Instant::now();
    let mut latencies = Vec::with_capacity(suite.measured_messages as usize);
    let mut errors = 0;
    for _ in 0..suite.measured_messages {
        match read_next_sse_payload_with_timeout(&mut response, STREAM_DELIVERY_TIMEOUT).await {
            Ok(payload) => {
                let idx = payload
                    .parse::<u64>()
                    .context("invalid SSE payload index")?;
                let sent_at = send_times
                    .lock()
                    .unwrap()
                    .remove(&idx)
                    .ok_or_else(|| anyhow!("missing send timestamp for payload `{idx}`"))?;
                latencies.push(sent_at.elapsed());
            }
            Err(_) => errors += 1,
        }
    }
    publish_task
        .await
        .context("SSE publisher task panicked")??;

    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.measured_messages,
        errors,
    ))
}

async fn wait_for_sse_subscription_ready(
    response: &mut reqwest::Response,
    publisher: &mut MultiplexedConnection,
    channel: &str,
) -> Result<()> {
    for _ in 0..STREAM_READY_ATTEMPTS {
        publish_message(publisher, channel, STREAM_READY_SENTINEL).await?;
        match read_next_sse_payload_with_timeout(response, STREAM_READY_TIMEOUT).await {
            Ok(payload) if payload == STREAM_READY_SENTINEL => return Ok(()),
            Ok(_) => continue,
            Err(_) => continue,
        }
    }

    bail!("timed out waiting for SSE subscription readiness on channel `{channel}`")
}

async fn read_next_sse_payload_with_timeout(
    response: &mut reqwest::Response,
    timeout_duration: Duration,
) -> Result<String> {
    let mut buffer = String::new();
    loop {
        let chunk = timeout(timeout_duration, response.chunk())
            .await
            .context("timed out waiting for SSE chunk")??
            .ok_or_else(|| anyhow!("SSE stream ended unexpectedly"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buffer.find('\n') {
            let mut line = buffer[..idx].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            buffer = buffer[idx + 1..].to_string();
            if let Some(payload) = line.strip_prefix("data:") {
                let payload = payload.trim_start().to_string();
                if !payload.is_empty() {
                    return Ok(payload);
                }
            }
        }
    }
}

async fn grpc_startup_workload(
    variant_name: &str,
    endpoint: &str,
    suite: &StreamingSuiteConfig,
    config: &Config,
) -> Result<MetricSummary> {
    let mut publisher = redis_connection(config).await?;

    for idx in 0..suite.startup_warmup_ops {
        let channel = format!("redis-web-bench:{variant_name}:grpc-startup:warmup:{idx}");
        measure_grpc_startup_once(endpoint, &mut publisher, &channel, &format!("warmup-{idx}"))
            .await?;
    }

    let mut latencies = Vec::with_capacity(suite.startup_measured_ops as usize);
    let start = Instant::now();
    let mut errors = 0;
    for idx in 0..suite.startup_measured_ops {
        let channel = format!("redis-web-bench:{variant_name}:grpc-startup:{idx}");
        match measure_grpc_startup_once(
            endpoint,
            &mut publisher,
            &channel,
            &format!("measured-{idx}"),
        )
        .await
        {
            Ok(latency) => latencies.push(latency),
            Err(_) => errors += 1,
        }
    }

    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.startup_measured_ops,
        errors,
    ))
}

async fn measure_grpc_startup_once(
    endpoint: &str,
    publisher: &mut MultiplexedConnection,
    channel: &str,
    payload: &str,
) -> Result<Duration> {
    let started = Instant::now();
    let mut client = RedisGatewayClient::connect(endpoint.to_string()).await?;
    let mut stream = client
        .subscribe(Request::new(SubscribeRequest {
            channel: channel.to_string(),
        }))
        .await?
        .into_inner();
    wait_for_grpc_subscription_ready(&mut stream, publisher, channel).await?;

    for _ in 0..STREAM_READY_ATTEMPTS {
        publish_message(publisher, channel, payload).await?;
        match next_grpc_payload(&mut stream, STREAM_READY_TIMEOUT).await {
            Ok(received) if received == payload => return Ok(started.elapsed()),
            Ok(_) => continue,
            Err(_) => continue,
        }
    }

    bail!("timed out waiting for gRPC startup payload `{payload}`")
}

async fn grpc_delivery_workload(
    variant_name: &str,
    endpoint: &str,
    suite: &StreamingSuiteConfig,
    config: &Config,
) -> Result<MetricSummary> {
    let mut publisher = redis_connection(config).await?;
    let mut client = RedisGatewayClient::connect(endpoint.to_string()).await?;
    let channel = format!("redis-web-bench:{variant_name}:grpc-stream");
    let mut stream = client
        .subscribe(Request::new(SubscribeRequest {
            channel: channel.clone(),
        }))
        .await?
        .into_inner();
    wait_for_grpc_subscription_ready(&mut stream, &mut publisher, &channel).await?;

    for idx in 0..suite.warmup_messages {
        let payload = format!("warmup:{idx}");
        publish_message(&mut publisher, &channel, &payload).await?;
        let _ = next_grpc_payload(&mut stream, STREAM_DELIVERY_TIMEOUT).await?;
    }

    let send_times = Arc::new(Mutex::new(HashMap::<u64, Instant>::with_capacity(
        suite.measured_messages as usize,
    )));
    let mut measured_publisher = redis_connection(config).await?;
    let channel_for_publish = channel.clone();
    let send_times_for_publish = send_times.clone();
    let measured_messages = suite.measured_messages;
    let publish_task = tokio::spawn(async move {
        for idx in 0..measured_messages {
            loop {
                if send_times_for_publish.lock().unwrap().len() < STREAM_MAX_IN_FLIGHT_MESSAGES {
                    break;
                }
                tokio::task::yield_now().await;
            }
            send_times_for_publish
                .lock()
                .unwrap()
                .insert(idx, Instant::now());
            let payload = format!("{idx}");
            publish_message(&mut measured_publisher, &channel_for_publish, &payload).await?;
            if idx % STREAM_PUBLISH_YIELD_INTERVAL == STREAM_PUBLISH_YIELD_INTERVAL - 1 {
                tokio::task::yield_now().await;
            }
        }
        Result::<()>::Ok(())
    });

    let start = Instant::now();
    let mut latencies = Vec::with_capacity(suite.measured_messages as usize);
    let mut errors = 0;
    for _ in 0..suite.measured_messages {
        match next_grpc_payload(&mut stream, STREAM_DELIVERY_TIMEOUT).await {
            Ok(payload) => {
                let idx = payload
                    .parse::<u64>()
                    .context("invalid gRPC payload index")?;
                let sent_at = send_times
                    .lock()
                    .unwrap()
                    .remove(&idx)
                    .ok_or_else(|| anyhow!("missing send timestamp for payload `{idx}`"))?;
                latencies.push(sent_at.elapsed());
            }
            _ => errors += 1,
        }
    }
    publish_task
        .await
        .context("gRPC publisher task panicked")??;

    Ok(summarize_metrics(
        latencies,
        start.elapsed(),
        suite.measured_messages,
        errors,
    ))
}

async fn wait_for_grpc_subscription_ready(
    stream: &mut tonic::Streaming<redis_web_runtime::grpc::proto::SubscribeEvent>,
    publisher: &mut MultiplexedConnection,
    channel: &str,
) -> Result<()> {
    for _ in 0..STREAM_READY_ATTEMPTS {
        publish_message(publisher, channel, STREAM_READY_SENTINEL).await?;
        match next_grpc_payload(stream, STREAM_READY_TIMEOUT).await {
            Ok(payload) if payload == STREAM_READY_SENTINEL => return Ok(()),
            Ok(_) => continue,
            Err(_) => continue,
        }
    }

    bail!("timed out waiting for gRPC subscription readiness on channel `{channel}`")
}

async fn next_grpc_payload(
    stream: &mut tonic::Streaming<redis_web_runtime::grpc::proto::SubscribeEvent>,
    timeout_duration: Duration,
) -> Result<String> {
    let event = timeout(timeout_duration, stream.next())
        .await
        .context("timed out waiting for gRPC subscribe event")?
        .ok_or_else(|| anyhow!("gRPC subscribe stream ended unexpectedly"))??;
    String::from_utf8(event.payload).context("gRPC payload was not UTF-8")
}

async fn redis_connection(config: &Config) -> Result<MultiplexedConnection> {
    let client =
        redis::Client::open(config.get_redis_url()).context("failed to create Redis client")?;
    client
        .get_multiplexed_async_connection()
        .await
        .context("failed to connect to Redis")
}

async fn publish_message(
    conn: &mut MultiplexedConnection,
    channel: &str,
    payload: &str,
) -> Result<()> {
    redis::cmd("PUBLISH")
        .arg(channel)
        .arg(payload)
        .query_async::<i64>(conn)
        .await
        .context("failed to publish Redis message")?;
    Ok(())
}

fn completed_workload(name: &str, metrics: MetricSummary) -> WorkloadResult {
    WorkloadResult {
        name: name.to_string(),
        metrics: Some(metrics),
        notes: None,
    }
}

fn skipped_suite(name: &str, reason: &str) -> SuiteResult {
    SuiteResult {
        suite: name.to_string(),
        status: SuiteStatus::Skipped {
            reason: reason.to_string(),
        },
        workloads: Vec::new(),
    }
}

fn summarize_metrics(
    mut latencies: Vec<Duration>,
    elapsed: Duration,
    attempted_ops: u64,
    error_count: u64,
) -> MetricSummary {
    latencies.sort_unstable();
    let success_count = latencies.len() as u64;
    MetricSummary {
        attempted_ops,
        success_count,
        error_count,
        p50_ms: percentile_ms(&latencies, 0.50),
        p95_ms: percentile_ms(&latencies, 0.95),
        p99_ms: percentile_ms(&latencies, 0.99),
        throughput_per_sec: if elapsed.is_zero() {
            0.0
        } else {
            success_count as f64 / elapsed.as_secs_f64()
        },
    }
}

fn percentile_ms(latencies: &[Duration], percentile: f64) -> Option<f64> {
    if latencies.is_empty() {
        return None;
    }
    let idx = ((latencies.len() as f64 - 1.0) * percentile).round() as usize;
    Some(latencies[idx].as_secs_f64() * 1000.0)
}

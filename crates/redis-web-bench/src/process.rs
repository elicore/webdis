//! redis-web process management for benchmark runs.

use anyhow::{anyhow, bail, Context, Result};
use redis_web_core::config::{Config, TransportMode};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist")
        .to_path_buf()
}

pub(crate) struct LaunchedServer {
    process: Child,
    _tempdir: TempDir,
    port: u16,
}

impl LaunchedServer {
    pub(crate) async fn start(config: &Config, workspace_root: &Path) -> Result<Self> {
        ensure_binary_for_mode(workspace_root, config.transport_mode)?;
        let binary = binary_path_for_mode(workspace_root, config.transport_mode)?;
        let tempdir = tempfile::tempdir().context("failed to create tempdir for benchmark run")?;
        let port = pick_unused_local_port()?;
        let mut json =
            serde_json::to_value(config).context("failed to serialize config for benchmark run")?;
        prepare_runtime_config(&mut json, config.transport_mode, port)?;

        let config_path = tempdir.path().join("redis-web.bench.json");
        let stdout_path = tempdir.path().join("stdout.log");
        let stderr_path = tempdir.path().join("stderr.log");
        fs::write(
            &config_path,
            format!("{}\n", serde_json::to_string_pretty(&json)?),
        )?;
        let stdout = fs::File::create(&stdout_path)?;
        let stderr = fs::File::create(&stderr_path)?;

        let mut process = Command::new(binary)
            .arg(&config_path)
            .current_dir(workspace_root)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .context("failed to spawn redis-web benchmark target")?;

        let mut ready = false;
        for _ in 0..80 {
            if let Ok(Ok(_)) = timeout(
                Duration::from_millis(100),
                TcpStream::connect(("127.0.0.1", port)),
            )
            .await
            {
                ready = true;
                break;
            }
            if let Some(status) = process.try_wait()? {
                let stderr_text = fs::read_to_string(&stderr_path).unwrap_or_default();
                bail!(
                    "redis-web exited before becoming ready (status: {status}). stderr:\n{}",
                    stderr_text
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        if !ready {
            let stderr_text = fs::read_to_string(&stderr_path).unwrap_or_default();
            let _ = process.kill();
            bail!("redis-web did not become ready on port {port}. stderr:\n{stderr_text}");
        }

        Ok(Self {
            process,
            _tempdir: tempdir,
            port,
        })
    }

    pub(crate) fn http_base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub(crate) fn grpc_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub(crate) fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}/.json", self.port)
    }
}

impl Drop for LaunchedServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

fn prepare_runtime_config(json: &mut Value, mode: TransportMode, port: u16) -> Result<()> {
    let object = json
        .as_object_mut()
        .ok_or_else(|| anyhow!("serialized config should be an object"))?;
    object.insert("verbosity".to_string(), Value::from(0));

    match mode {
        TransportMode::Rest => {
            object.insert(
                "http_host".to_string(),
                Value::String("127.0.0.1".to_string()),
            );
            object.insert("http_port".to_string(), Value::from(port));
        }
        TransportMode::Grpc => {
            let grpc = object
                .entry("grpc".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            let grpc_object = grpc
                .as_object_mut()
                .ok_or_else(|| anyhow!("grpc config should be an object"))?;
            grpc_object.insert("host".to_string(), Value::String("127.0.0.1".to_string()));
            grpc_object.insert("port".to_string(), Value::from(port));
        }
    }

    Ok(())
}

fn ensure_binary_for_mode(workspace_root: &Path, mode: TransportMode) -> Result<()> {
    static REST_BUILD_ONCE: OnceLock<()> = OnceLock::new();
    static GRPC_BUILD_ONCE: OnceLock<()> = OnceLock::new();

    let (bin_name, build_once) = match mode {
        TransportMode::Rest => ("redis-web", &REST_BUILD_ONCE),
        TransportMode::Grpc => ("redis-web-grpc", &GRPC_BUILD_ONCE),
    };

    if build_once.get().is_some() {
        return Ok(());
    }

    let status = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("redis-web")
        .arg("--bin")
        .arg(bin_name)
        .arg("--release")
        .current_dir(workspace_root)
        .status()
        .with_context(|| format!("failed to build {bin_name} benchmark target"))?;
    if !status.success() {
        bail!("cargo build -p redis-web --bin {bin_name} --release failed");
    }

    let _ = build_once.set(());
    Ok(())
}

fn binary_path_for_mode(workspace_root: &Path, mode: TransportMode) -> Result<PathBuf> {
    let bin_name = match mode {
        TransportMode::Rest => "redis-web",
        TransportMode::Grpc => "redis-web-grpc",
    };
    let path = workspace_root.join(format!("target/release/{bin_name}"));
    if path.exists() {
        Ok(path)
    } else {
        bail!("expected redis-web binary at {}", path.display())
    }
}

fn pick_unused_local_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .context("failed to bind temporary local port")?;
    Ok(listener.local_addr()?.port())
}

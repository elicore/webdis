use redis_web_bench::{run_compare_with_registry, SuiteRegistry};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

struct ManagedRedis {
    port: u16,
    process: ManagedRedisProcess,
}

enum ManagedRedisProcess {
    Native(Child),
    Docker(String),
}

impl ManagedRedis {
    async fn start() -> Option<Self> {
        let port = pick_unused_local_port();

        if command_available("redis-server", &["--version"]) {
            let child = Command::new("redis-server")
                .arg("--port")
                .arg(port.to_string())
                .arg("--save")
                .arg("")
                .arg("--appendonly")
                .arg("no")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            wait_for_port(port).await.ok()?;
            return Some(Self {
                port,
                process: ManagedRedisProcess::Native(child),
            });
        }

        if command_available("docker", &["version"]) {
            let output = Command::new("docker")
                .arg("run")
                .arg("--rm")
                .arg("-d")
                .arg("-p")
                .arg(format!("{port}:6379"))
                .arg("redis:8.2-alpine")
                .arg("redis-server")
                .arg("--save")
                .arg("")
                .arg("--appendonly")
                .arg("no")
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            wait_for_port(port).await.ok()?;
            return Some(Self {
                port,
                process: ManagedRedisProcess::Docker(container_id),
            });
        }

        None
    }
}

impl Drop for ManagedRedis {
    fn drop(&mut self) {
        match &mut self.process {
            ManagedRedisProcess::Native(child) => {
                let _ = child.kill();
            }
            ManagedRedisProcess::Docker(container_id) => {
                let _ = Command::new("docker")
                    .arg("stop")
                    .arg(container_id)
                    .output();
            }
        }
    }
}

fn command_available(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn pick_unused_local_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

async fn wait_for_port(port: u16) -> anyhow::Result<()> {
    for _ in 0..80 {
        if let Ok(Ok(_)) = timeout(
            Duration::from_millis(100),
            TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        {
            return Ok(());
        }
        sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("timed out waiting for Redis on port {port}")
}

fn write_base_config(tempdir: &TempDir, redis_port: u16) -> PathBuf {
    let path = tempdir.path().join("redis-web.base.json");
    fs::write(
        &path,
        format!(
            "{{\n  \"redis_host\": \"127.0.0.1\",\n  \"redis_port\": {redis_port},\n  \"http_host\": \"127.0.0.1\",\n  \"http_port\": 7379,\n  \"verbosity\": 0\n}}\n"
        ),
    )
    .unwrap();
    path
}

#[tokio::test]
async fn compare_smoke_writes_json_and_markdown_artifacts() {
    let Some(redis) = ManagedRedis::start().await else {
        eprintln!("Skipping smoke benchmark test: neither redis-server nor docker is available");
        return;
    };

    let tempdir = tempfile::tempdir().unwrap();
    let base_config = write_base_config(&tempdir, redis.port);
    let output_dir = tempdir.path().join("artifacts");
    let spec_path = tempdir.path().join("compare.yaml");

    fs::write(
        &spec_path,
        format!(
            "base_config: {}\noutput_dir: {}\nvariants:\n  - name: rest-ws\n    overrides:\n      websockets: true\n  - name: grpc-transport\n    overrides:\n      transport_mode: grpc\n      runtime_worker_threads: 2\n",
            base_config.display(),
            output_dir.display()
        ),
    )
    .unwrap();

    let artifact_dir = run_compare_with_registry(&spec_path, SuiteRegistry::smoke())
        .await
        .unwrap();
    let results_path = artifact_dir.join("results.json");
    let report_path = artifact_dir.join("report.md");
    assert!(results_path.exists());
    assert!(report_path.exists());

    let results: Value = serde_json::from_str(&fs::read_to_string(results_path).unwrap()).unwrap();
    let runs = results["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3);

    let rest_ws = runs.iter().find(|run| run["name"] == "rest-ws").unwrap();
    let grpc = runs
        .iter()
        .find(|run| run["name"] == "grpc-transport")
        .unwrap();

    let rest_ws_suite = rest_ws["suites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|suite| suite["suite"] == "websocket_commands")
        .unwrap();
    assert_eq!(rest_ws_suite["status"]["kind"], "completed");

    let grpc_suite = grpc["suites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|suite| suite["suite"] == "streaming_pubsub")
        .unwrap();
    assert_eq!(grpc_suite["status"]["kind"], "completed");

    let baseline = runs.iter().find(|run| run["name"] == "baseline").unwrap();
    let read_heavy_suite = baseline["suites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|suite| suite["suite"] == "read_heavy_cache")
        .unwrap();
    assert_eq!(read_heavy_suite["status"]["kind"], "completed");

    let report = fs::read_to_string(report_path).unwrap();
    assert!(report.contains("read_heavy_cache"));
    assert!(report.contains("websocket_commands"));
    assert!(report.contains("streaming_pubsub"));
    assert!(report.contains("runtime_worker_threads"));
}

#[tokio::test]
async fn compare_rejects_invalid_overrides_before_running() {
    let tempdir = tempfile::tempdir().unwrap();
    let base_config = write_base_config(&tempdir, 6379);
    let spec_path = tempdir.path().join("invalid.yaml");

    fs::write(
        &spec_path,
        format!(
            "base_config: {}\nvariants:\n  - name: broken\n    overrides:\n      runtime_worker_threads: 0\n",
            base_config.display(),
        ),
    )
    .unwrap();

    let error = run_compare_with_registry(&spec_path, SuiteRegistry::smoke())
        .await
        .expect_err("invalid runtime_worker_threads should fail");
    let message = error.to_string();
    assert!(
        !message.is_empty(),
        "invalid override should yield a non-empty validation error"
    );
}

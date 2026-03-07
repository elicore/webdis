//! Small helpers for run metadata and human-readable endpoint summaries.

use crate::model::EnvironmentSummary;
use redis_web_core::config::Config;
use std::path::Path;
use std::process::Command;

pub(crate) fn environment_summary() -> EnvironmentSummary {
    EnvironmentSummary {
        hostname: std::env::var("HOSTNAME")
            .ok()
            .or_else(|| std::env::var("COMPUTERNAME").ok()),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_parallelism: std::thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(1),
    }
}

pub(crate) fn redis_endpoint_summary(config: &Config) -> String {
    if let Some(socket) = config.redis_socket.as_deref() {
        format!("unix://{} (db={})", socket, config.database)
    } else {
        format!(
            "{}:{} (db={}, tls={})",
            config.redis_host,
            config.redis_port,
            config.database,
            config.ssl.as_ref().is_some_and(|ssl| ssl.enabled)
        )
    }
}

pub(crate) fn git_sha(workspace_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(workspace_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8(output.stdout).ok()?;
    Some(sha.trim().to_string())
}

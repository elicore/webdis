//! Core types shared across the benchmark runner.
//!
//! This module intentionally keeps data definitions separate from execution logic so
//! the spec loader, benchmark suites, process launcher, and report renderer can stay
//! focused on one job each.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub const BENCHMARK_SUITE_VERSION: &str = "v1";

/// Top-level YAML/JSON benchmark spec.
#[derive(Clone, Debug, Deserialize)]
pub struct CompareSpec {
    pub base_config: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub variants: Vec<VariantSpec>,
}

/// Named variant applied as a recursive JSON merge over the base config.
#[derive(Clone, Debug, Deserialize)]
pub struct VariantSpec {
    pub name: String,
    pub overrides: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedSpec {
    pub(crate) base_config: PathBuf,
    pub(crate) output_root: PathBuf,
    pub(crate) variants: Vec<VariantSpec>,
}

/// Static v1 benchmark suite registry.
#[derive(Clone, Debug)]
pub struct SuiteRegistry {
    pub(crate) common_commands: CommandSuiteConfig,
    pub(crate) websocket_commands: WebSocketSuiteConfig,
    pub(crate) streaming: StreamingSuiteConfig,
}

#[derive(Clone, Debug)]
pub(crate) struct CommandSuiteConfig {
    pub(crate) warmup_ops: u64,
    pub(crate) measured_ops: u64,
    pub(crate) concurrency: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct WebSocketSuiteConfig {
    pub(crate) warmup_ops: u64,
    pub(crate) measured_ops: u64,
    pub(crate) persistent_connections: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct StreamingSuiteConfig {
    pub(crate) startup_warmup_ops: u64,
    pub(crate) startup_measured_ops: u64,
    pub(crate) warmup_messages: u64,
    pub(crate) measured_messages: u64,
}

impl SuiteRegistry {
    pub fn default_v1() -> Self {
        Self {
            common_commands: CommandSuiteConfig {
                warmup_ops: 500,
                measured_ops: 10_000,
                concurrency: 32,
            },
            websocket_commands: WebSocketSuiteConfig {
                warmup_ops: 500,
                measured_ops: 10_000,
                persistent_connections: 8,
            },
            streaming: StreamingSuiteConfig {
                startup_warmup_ops: 25,
                startup_measured_ops: 100,
                warmup_messages: 100,
                measured_messages: 5_000,
            },
        }
    }

    pub fn smoke() -> Self {
        Self {
            common_commands: CommandSuiteConfig {
                warmup_ops: 5,
                measured_ops: 20,
                concurrency: 4,
            },
            websocket_commands: WebSocketSuiteConfig {
                warmup_ops: 3,
                measured_ops: 12,
                persistent_connections: 2,
            },
            streaming: StreamingSuiteConfig {
                startup_warmup_ops: 1,
                startup_measured_ops: 3,
                warmup_messages: 2,
                measured_messages: 8,
            },
        }
    }
}

/// Machine-readable benchmark artifact.
#[derive(Debug, Serialize)]
pub struct BenchmarkResults {
    pub generated_at: String,
    pub suite_version: String,
    pub git_sha: Option<String>,
    pub environment: EnvironmentSummary,
    pub baseline_name: String,
    pub runs: Vec<VariantBenchmarkResult>,
}

#[derive(Debug, Serialize)]
pub struct EnvironmentSummary {
    pub hostname: Option<String>,
    pub os: String,
    pub arch: String,
    pub cpu_parallelism: usize,
}

#[derive(Debug, Serialize)]
pub struct VariantBenchmarkResult {
    pub name: String,
    pub redis_endpoint: String,
    pub transport_mode: String,
    pub config_diff: Vec<ConfigDiff>,
    pub suites: Vec<SuiteResult>,
}

#[derive(Debug, Serialize)]
pub struct ConfigDiff {
    pub key: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Serialize)]
pub struct SuiteResult {
    pub suite: String,
    pub status: SuiteStatus,
    pub workloads: Vec<WorkloadResult>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuiteStatus {
    Completed,
    Skipped { reason: String },
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkloadResult {
    pub name: String,
    pub metrics: Option<MetricSummary>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MetricSummary {
    pub attempted_ops: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub throughput_per_sec: f64,
}

#[derive(Debug)]
pub(crate) struct VariantRunContext {
    pub(crate) name: String,
    pub(crate) config: redis_web_core::config::Config,
    pub(crate) diff: Vec<ConfigDiff>,
}

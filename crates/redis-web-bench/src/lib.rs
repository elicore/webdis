//! `redis-web-bench` compares a base redis-web config against named variants.
//!
//! The crate is organized by responsibility:
//! - [`model`] defines the shared data structures and static suite presets.
//! - [`spec`] resolves YAML/JSON specs, merges overrides, and computes config diffs.
//! - [`process`] launches isolated `redis-web` processes for each variant.
//! - [`suites`] runs the actual benchmark workloads.
//! - [`report`] renders the Markdown summary artifact.

mod model;
mod process;
mod report;
mod spec;
mod suites;
mod summary;

pub use model::{
    BenchmarkResults, CompareSpec, ConfigDiff, EnvironmentSummary, MetricSummary, SuiteRegistry,
    SuiteResult, SuiteStatus, VariantBenchmarkResult, VariantSpec, WorkloadResult,
    BENCHMARK_SUITE_VERSION,
};

use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::process::workspace_root;
use crate::report::render_markdown_report;
use crate::spec::{build_variant_context, load_json_value, resolve_spec};
use crate::suites::benchmark_variant;
use crate::summary::{environment_summary, git_sha};

pub async fn run_compare(spec_path: &Path) -> Result<PathBuf> {
    run_compare_with_registry(spec_path, SuiteRegistry::default_v1()).await
}

pub async fn run_compare_with_registry(
    spec_path: &Path,
    registry: SuiteRegistry,
) -> Result<PathBuf> {
    let workspace_root = workspace_root();
    let resolved_spec = resolve_spec(spec_path, &workspace_root)?;

    let base_raw = load_json_value(&resolved_spec.base_config)?;
    let (baseline_context, baseline_value) = build_variant_context(
        "baseline",
        &base_raw,
        &serde_json::Value::Object(Default::default()),
        None,
    )?;

    let mut variant_contexts = Vec::with_capacity(resolved_spec.variants.len() + 1);
    variant_contexts.push(baseline_context);
    for variant in &resolved_spec.variants {
        let (context, _) = build_variant_context(
            &variant.name,
            &base_raw,
            &variant.overrides,
            Some(&baseline_value),
        )?;
        variant_contexts.push(context);
    }

    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let artifact_dir = resolved_spec.output_root.join(&timestamp);
    fs::create_dir_all(&artifact_dir).with_context(|| {
        format!(
            "failed to create artifact directory {}",
            artifact_dir.display()
        )
    })?;

    let mut runs = Vec::with_capacity(variant_contexts.len());
    for context in variant_contexts {
        runs.push(
            benchmark_variant(
                context,
                &registry.common_commands,
                &registry.websocket_commands,
                &registry.streaming,
                &workspace_root,
            )
            .await?,
        );
    }

    let results = BenchmarkResults {
        generated_at: Utc::now().to_rfc3339(),
        suite_version: BENCHMARK_SUITE_VERSION.to_string(),
        git_sha: git_sha(&workspace_root),
        environment: environment_summary(),
        baseline_name: "baseline".to_string(),
        runs,
    };

    let results_path = artifact_dir.join("results.json");
    let report_path = artifact_dir.join("report.md");
    fs::write(
        &results_path,
        format!("{}\n", serde_json::to_string_pretty(&results)?),
    )
    .with_context(|| format!("failed to write {}", results_path.display()))?;
    fs::write(&report_path, render_markdown_report(&results))
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    Ok(artifact_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_serialize_to_json() {
        let results = BenchmarkResults {
            generated_at: "2026-03-07T00:00:00Z".to_string(),
            suite_version: BENCHMARK_SUITE_VERSION.to_string(),
            git_sha: Some("abc123".to_string()),
            environment: environment_summary(),
            baseline_name: "baseline".to_string(),
            runs: vec![VariantBenchmarkResult {
                name: "baseline".to_string(),
                redis_endpoint: "127.0.0.1:6379 (db=0, tls=false)".to_string(),
                transport_mode: "rest".to_string(),
                config_diff: Vec::new(),
                suites: vec![SuiteResult {
                    suite: "common_commands".to_string(),
                    status: SuiteStatus::Completed,
                    workloads: vec![WorkloadResult {
                        name: "ping".to_string(),
                        metrics: Some(MetricSummary {
                            attempted_ops: 1,
                            success_count: 1,
                            error_count: 0,
                            p50_ms: Some(1.0),
                            p95_ms: Some(1.0),
                            p99_ms: Some(1.0),
                            throughput_per_sec: 1000.0,
                        }),
                        notes: None,
                    }],
                }],
            }],
        };

        let json = serde_json::to_string(&results).unwrap();
        assert!(json.contains("common_commands"));
        assert!(json.contains("throughput_per_sec"));
    }
}

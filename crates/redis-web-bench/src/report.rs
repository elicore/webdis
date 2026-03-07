//! Markdown report rendering.

use crate::model::{
    BenchmarkResults, MetricSummary, SuiteResult, SuiteStatus, VariantBenchmarkResult,
    WorkloadResult,
};
use std::collections::BTreeSet;

pub(crate) fn render_markdown_report(results: &BenchmarkResults) -> String {
    let mut out = String::new();
    out.push_str("# Configuration Benchmark Report\n\n");
    out.push_str(&format!("- Generated at: `{}`\n", results.generated_at));
    out.push_str(&format!("- Suite version: `{}`\n", results.suite_version));
    if let Some(git_sha) = &results.git_sha {
        out.push_str(&format!("- Git SHA: `{}`\n", git_sha));
    }
    out.push_str(&format!(
        "- Environment: `{}/{}` with `{}` logical CPUs\n\n",
        results.environment.os, results.environment.arch, results.environment.cpu_parallelism
    ));

    out.push_str("## Config Changes\n\n");
    for run in &results.runs {
        out.push_str(&format!("### {}\n\n", run.name));
        out.push_str(&format!("- Transport: `{}`\n", run.transport_mode));
        out.push_str(&format!("- Redis endpoint: `{}`\n", run.redis_endpoint));
        if run.config_diff.is_empty() {
            out.push_str("- Changed config: baseline\n\n");
        } else {
            out.push_str("- Changed config:\n");
            for diff in &run.config_diff {
                out.push_str(&format!(
                    "  - `{}`: `{}` -> `{}`\n",
                    diff.key, diff.before, diff.after
                ));
            }
            out.push('\n');
        }
    }

    let baseline = results
        .runs
        .iter()
        .find(|run| run.name == results.baseline_name)
        .or_else(|| results.runs.first());

    let suite_names: BTreeSet<_> = results
        .runs
        .iter()
        .flat_map(|run| run.suites.iter().map(|suite| suite.suite.clone()))
        .collect();

    for suite_name in suite_names {
        out.push_str(&format!("## {}\n\n", suite_name));
        out.push_str("| Variant | Workload | p50 ms | Δ p50 | p95 ms | Δ p95 | p99 ms | Δ p99 | Throughput/s | Δ throughput | Errors | Notes |\n");
        out.push_str(
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n",
        );
        for run in &results.runs {
            let suite = run
                .suites
                .iter()
                .find(|candidate| candidate.suite == suite_name);
            match suite {
                Some(SuiteResult {
                    status: SuiteStatus::Completed,
                    workloads,
                    ..
                }) => {
                    for workload in workloads {
                        let baseline_metrics = baseline
                            .and_then(|baseline_run| {
                                baseline_run
                                    .suites
                                    .iter()
                                    .find(|candidate| candidate.suite == suite_name)
                            })
                            .and_then(|baseline_suite| {
                                baseline_suite
                                    .workloads
                                    .iter()
                                    .find(|candidate| candidate.name == workload.name)
                            })
                            .and_then(|baseline_workload| baseline_workload.metrics.as_ref());
                        out.push_str(&render_workload_row(run, workload, baseline_metrics));
                    }
                }
                Some(SuiteResult {
                    status: SuiteStatus::Skipped { reason },
                    ..
                }) => {
                    out.push_str(&format!(
                        "| {} | - | - | - | - | - | - | - | - | - | - | {} |\n",
                        run.name, reason
                    ));
                }
                None => {
                    out.push_str(&format!(
                        "| {} | - | - | - | - | - | - | - | - | - | - | suite missing |\n",
                        run.name
                    ));
                }
            }
        }
        out.push('\n');
    }

    out
}

fn render_workload_row(
    run: &VariantBenchmarkResult,
    workload: &WorkloadResult,
    baseline_metrics: Option<&MetricSummary>,
) -> String {
    let Some(metrics) = &workload.metrics else {
        return format!(
            "| {} | {} | - | - | - | - | - | - | - | - | - | {} |\n",
            run.name,
            workload.name,
            workload.notes.as_deref().unwrap_or("n/a")
        );
    };

    format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        run.name,
        workload.name,
        fmt_opt(metrics.p50_ms),
        fmt_delta(
            metrics.p50_ms,
            baseline_metrics.and_then(|baseline| baseline.p50_ms)
        ),
        fmt_opt(metrics.p95_ms),
        fmt_delta(
            metrics.p95_ms,
            baseline_metrics.and_then(|baseline| baseline.p95_ms)
        ),
        fmt_opt(metrics.p99_ms),
        fmt_delta(
            metrics.p99_ms,
            baseline_metrics.and_then(|baseline| baseline.p99_ms)
        ),
        fmt_float(metrics.throughput_per_sec),
        fmt_delta(
            Some(metrics.throughput_per_sec),
            baseline_metrics.map(|baseline| baseline.throughput_per_sec),
        ),
        metrics.error_count,
        workload.notes.as_deref().unwrap_or(""),
    )
}

fn fmt_opt(value: Option<f64>) -> String {
    value.map(fmt_float).unwrap_or_else(|| "-".to_string())
}

fn fmt_float(value: f64) -> String {
    format!("{value:.2}")
}

fn fmt_delta(value: Option<f64>, baseline: Option<f64>) -> String {
    match (value, baseline) {
        (Some(value), Some(baseline)) => format!("{:+.2}", value - baseline),
        _ => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        BenchmarkResults, ConfigDiff, EnvironmentSummary, MetricSummary, SuiteResult, SuiteStatus,
        VariantBenchmarkResult, WorkloadResult,
    };

    #[test]
    fn markdown_report_highlights_deltas() {
        let baseline = VariantBenchmarkResult {
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
                        p50_ms: Some(2.0),
                        p95_ms: Some(2.0),
                        p99_ms: Some(2.0),
                        throughput_per_sec: 100.0,
                    }),
                    notes: None,
                }],
            }],
        };
        let variant = VariantBenchmarkResult {
            name: "grpc-8-workers".to_string(),
            redis_endpoint: "127.0.0.1:6379 (db=0, tls=false)".to_string(),
            transport_mode: "grpc".to_string(),
            config_diff: vec![ConfigDiff {
                key: "runtime_worker_threads".to_string(),
                before: "<unset>".to_string(),
                after: "8".to_string(),
            }],
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
                        throughput_per_sec: 120.0,
                    }),
                    notes: None,
                }],
            }],
        };

        let report = render_markdown_report(&BenchmarkResults {
            generated_at: "2026-03-07T00:00:00Z".to_string(),
            suite_version: "v1".to_string(),
            git_sha: None,
            environment: EnvironmentSummary {
                hostname: None,
                os: "macos".to_string(),
                arch: "aarch64".to_string(),
                cpu_parallelism: 8,
            },
            baseline_name: "baseline".to_string(),
            runs: vec![baseline, variant],
        });

        assert!(report.contains("runtime_worker_threads"));
        assert!(report.contains("+20.00"));
        assert!(report.contains("-1.00"));
    }
}

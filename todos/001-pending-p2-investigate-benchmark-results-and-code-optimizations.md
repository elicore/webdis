---
status: pending
priority: p2
issue_id: "001"
tags: [benchmark, performance, redis-web, optimization]
dependencies: []
---

# Problem Statement

The new configuration benchmark framework is producing realistic comparative data,
but the results now need focused analysis. We need to explain why some variants
improve throughput or latency while others regress, then convert that analysis
into code-level optimization proposals for `redis-web` and the benchmark runner.

# Findings

- The March 8, 2026 use-case matrix (`target/perf/20260308T230054Z`) completed
  cleanly across baseline plus 10 named variants.
- The March 8, 2026 rerun with the new `read_heavy_cache` suite
  (`target/perf/20260309T022215Z`) also completed cleanly across all variants.
- `edge-rest-8-workers` improved the REST baseline more consistently than
  `edge-rest-16-workers`, suggesting the current machine or runtime path stops
  benefiting from additional worker count beyond a moderate level.
- `grpc-mesh-4-workers` produced the strongest `ping` throughput in that run,
  which suggests the gRPC unary path may have lower overhead than the REST path
  for tiny requests on this host.
- `sidecar-loopback` reduced footprint but regressed small request/response
  workloads, which may indicate under-provisioned runtime workers or Redis pool
  pressure.
- `grpc-debuggable` did not show a large penalty from reflection being enabled,
  which is useful operationally but should still be validated under profiling.
- In `read_heavy_cache`, the gRPC variants materially outperformed the REST
  variants across `GET`, `MGET`, and `HMGET`, which makes the read path a clear
  target for transport-path analysis and possible REST-side optimization.

# Proposed Solutions

## Option 1: Analyze Existing Artifacts and Recommend Code Changes

Pros:
- Fastest path to concrete optimization ideas
- Uses the benchmark framework immediately
- Low implementation risk

Cons:
- Limited by the data already collected
- May miss runtime behavior that only appears under profiling

Effort: Medium
Risk: Low

## Option 2: Add Profiling-Guided Investigation

Pros:
- Produces stronger evidence for optimization decisions
- Can separate transport overhead from Redis client overhead and runtime scheduling

Cons:
- Requires extra tooling and reproducible benchmark runs
- Slower turnaround than artifact-only analysis

Effort: Medium to High
Risk: Low

## Option 3: Extend Benchmarks First, Then Investigate

Pros:
- Better workload coverage before drawing conclusions
- Lets investigation include read-heavy cache and future write-heavy cases

Cons:
- Defers immediate optimization work
- Adds more implementation before analysis

Effort: High
Risk: Medium

# Recommended Action

Pending triage. The likely best path is a combined pass:
1. rerun the use-case matrix including `read_heavy_cache`
2. analyze per-suite regressions and wins by transport and worker count
3. profile the strongest and weakest variants
4. propose code optimizations with evidence

# Acceptance Criteria

- [ ] Review the latest benchmark artifacts, including a run with `read_heavy_cache`
- [ ] Summarize the most meaningful regressions and wins by suite and variant
- [ ] Identify likely bottlenecks in `redis-web` request handling, runtime sizing, or Redis access
- [ ] Propose a prioritized list of code optimizations with rationale
- [ ] Define how to validate each proposed optimization with the benchmark framework

# Work Log

### 2026-03-08 - Create investigation task

**By:** Codex

**Actions:**
- Added a tracked todo for follow-up performance analysis and optimization work.
- Captured the key observations from the March 8, 2026 use-case benchmark run.
- Scoped the task to include the newly added `read_heavy_cache` suite in the next analysis pass.

**Learnings:**
- The benchmark framework is now mature enough to drive optimization work rather than just transport comparisons.
- The next useful step is evidence-based analysis, not more speculative tuning.

### 2026-03-08 - Update task with read-heavy cache artifact

**By:** Codex

**Actions:**
- Added the new `read_heavy_cache` artifact path from `target/perf/20260309T022215Z`.
- Recorded that all variants completed the new suite without benchmark errors.
- Captured the most important new observation: gRPC leads the current REST path across the read-heavy cache workloads.

**Learnings:**
- The benchmark framework now has enough coverage to support a real optimization investigation for both command-heavy and read-heavy traffic.

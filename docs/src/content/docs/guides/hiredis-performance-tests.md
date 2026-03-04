---
title: Hiredis Compat Performance Harness
description: Comparison methodology for direct hiredis TCP vs redis-web compatibility transports.
---

## Goal

Track performance over time for optimization work. This harness is informational
and does not currently enforce CI pass/fail thresholds.

## Comparison Modes

Measure at minimum:
- direct hiredis -> Redis TCP baseline
- compat over redis-web WS raw path
- compat over redis-web HTTP fallback path

## Workload Categories

Include representative workloads:
- small GET/SET command loops
- mixed read/write sequences
- pipelined bursts
- Pub/Sub message fanout scenarios

## Reporting

Capture per run:
- p50/p95/p99 latency
- throughput (ops/sec)
- command mix and pipeline depth
- transport mode and host environment

Store benchmark results as artifacts so maintainers can compare trendlines
between commits.

Current harness script:
- `crates/redis-web/tests/bench-hiredis-compat.sh`

Run:

```bash
make bench_hiredis_compat
```

With environment overrides:

```bash
HOST=127.0.0.1 PORT=7379 ITERATIONS=500 make bench_hiredis_compat
```

The harness prints timing summary per run, for example:

```text
[compat-bench] Creating compat session on http://127.0.0.1:7379/__compat/session
[compat-bench] Running 500 SET/GET roundtrips
[compat-bench] elapsed_sec=12 iterations=500
[compat-bench] Cleaning up session
```

## Caveats

Do not compare runs collected on materially different hardware or background
load conditions without clear labeling.

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

## Caveats

Do not compare runs collected on materially different hardware or background
load conditions without clear labeling.

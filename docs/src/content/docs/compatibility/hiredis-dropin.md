---
title: Hiredis Drop-In Compatibility
description: Compatibility scope, status, and integration guidance for the redis-web hiredis shim.
---

## Scope

The hiredis compatibility track aims to let existing hiredis-based clients relink
against a redis-web-backed library with minimal code changes.

Target ABI:
- hiredis 1.3.x (sync API first)

Platform scope:
- Linux
- macOS

Artifact scope:
- shared library
- static library
- hiredis-style headers

## Current Status

Implemented:
- Workspace crate: `crates/redis-web-hiredis-compat`
- `cdylib` + `staticlib` artifact configuration
- Exported hiredis-compatible symbol scaffold
- Header scaffold at `include/hiredis/hiredis.h`
- pkg-config files for both naming modes

In progress:
- Redis command transport bridge over redis-web (WS-first with HTTP fallback)
- Full sync-command execution parity
- Pub/Sub parity over WS and HTTP fallback

Stubbed by design in v1:
- Async/SSL extras return deterministic unsupported behavior until fully implemented

## Naming Modes

The plan supports two naming modes:
- `libhiredis*` compatibility naming for drop-in relink scenarios
- `libredisweb_hiredis*` canonical naming for explicit integrations

## Environment Controls (Planned)

The compatibility layer will expose env-based controls for:
- transport mode (`auto`, `ws`, `http`, `direct`)
- compat endpoint prefix
- auth settings
- timeout tuning
- HTTP-fallback Pub/Sub warning mute

## Operational Note

HTTP fallback for Pub/Sub emits a warning by default. Use
`REDIS_WEB_COMPAT_MUTE_HTTP_PUBSUB_WARNING=1` to mute.

---
title: Overview
description: What redis-web is, when to use it, and how it relates to webdis.
---

`redis-web` is a Redis HTTP and WebSocket gateway. It exposes simple request
paths for Redis commands, and it supports WebSocket connections when you want a
long-lived channel.

Use `redis-web` when:

- You want HTTP or WebSocket access to Redis without writing a custom proxy.
- You need a small, auditable surface that can sit next to an existing app.
- You are migrating from `webdis` and want compatibility coverage.

## Compatibility scope

This repository started as a strict `webdis` compatibility effort. It now
tracks compatibility where it matters for migration safety, while using
canonical `redis-web` naming for binaries, crates, images, and docs.

If you need exact compatibility guarantees and migration steps, see
[Webdis Compatibility & Migration](/compatibility/webdis-compatibility/).

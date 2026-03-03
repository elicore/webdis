---
title: Run and First Requests
description: Start redis-web locally and send your first HTTP or WebSocket commands.
---

## Run locally

```bash
cargo run -p redis-web --bin redis-web -- redis-web.json
```

If no config path is passed, `redis-web` loads `redis-web.json` by default, then
falls back to `webdis.json` for compatibility.

## Write a default config

```bash
redis-web --write-default-config
```

This writes `redis-web.json` with `$schema` set to `./redis-web.schema.json`.

## First HTTP requests

```bash
curl http://127.0.0.1:7379/SET/hello/world
curl http://127.0.0.1:7379/GET/hello
```

Each path segment becomes a Redis argument. If you see `400`, double-check your
command spelling and URL encoding.

## JSON WebSocket

```javascript
const ws = new WebSocket('ws://127.0.0.1:7379/.json');
ws.onopen = () => ws.send(JSON.stringify(['SET', 'hello', 'world']));
ws.onmessage = (msg) => console.log(msg.data);
```

The JSON socket replies with JSON-encoded Redis responses and supports multiple
commands on a single connection.

## Raw RESP WebSocket

```javascript
const ws = new WebSocket('ws://127.0.0.1:7379/.raw');
ws.onopen = () => ws.send('*1\r\n$4\r\nPING\r\n');
```

Use the raw socket when you need RESP fidelity or binary-safe payloads.

---
title: HTTP and WebSocket API
description: Command encoding, response formats, and WebSocket endpoints.
---

## Surface selection

`redis-web` exposes one public surface at a time:

- `transport_mode = "rest"`: HTTP, WebSocket, and optional compat routes
- `transport_mode = "grpc"`: gRPC only

The sections below describe both transport families.

## HTTP command patterns

- `GET /COMMAND/arg0/.../argN[.ext]`
- `GET /<db>/COMMAND/...` for per-request DB selection
- `POST /` with command path in request body
- `PUT /COMMAND/...` with final argument in request body

Each path segment becomes one Redis argument. Use standard URL encoding if your
arguments include spaces, slashes, or binary data.

Example:

```bash
curl http://127.0.0.1:7379/7/GET/key
```

If you need to send a large value, prefer `PUT` (value in the body) or `POST`
(the whole command line in the body). This avoids long URLs and keeps proxies
happy.

## Response formats

Format selection via extension suffix:

- `.json` (default)
- `.msg` / `.msgpack`
- `.raw`

JSON is the most interoperable choice, while MessagePack offers smaller
payloads and faster decoding for high-throughput clients. Use `.raw` when you
want unmodified RESP replies.

Passthrough MIME mappings include `.txt`, `.html`, `.xml`, `.png`, `.jpg`.

Header override without body format change:

```bash
curl "http://127.0.0.1:7379/GET/hello?type=application/pdf"
```

The `type=` parameter only changes the `Content-Type` header. It does not
change the response body format.

## Status mapping

- `200` success
- `400` malformed command
- `403` ACL denial
- `500` execution/runtime error
- `503` Redis unavailable

## gRPC API

When `transport_mode` is `grpc`, redis-web exposes the `redis_web.v1.RedisGateway`
service with three RPCs:

- `Execute(CommandRequest) returns (CommandReply)`
- `ExecuteStream(stream StreamCommandRequest) returns (stream StreamCommandReply)`
- `Subscribe(SubscribeRequest) returns (stream SubscribeEvent)`

`CommandRequest` carries a command name plus binary-safe arguments:

```json
{
  "command": "SET",
  "args": ["aGVsbG8=", "d29ybGQ="]
}
```

Behavior notes:

- Unary RPC failures map to gRPC status codes (`INVALID_ARGUMENT`, `PERMISSION_DENIED`, `UNAVAILABLE`, `INTERNAL`).
- `ExecuteStream` keeps command-level failures in the streamed payload so the stream can continue.
- `Subscribe` is a single-channel server stream intended to cover the current public Pub/Sub surface.
- gRPC replies use a typed `RedisValue` tree, not JSON and not raw RESP frames.

For a local development workflow with `grpcurl`, reflection, and a Rust client
example, see [gRPC Development](/guides/grpc-development/).

## WebSocket endpoints

WebSocket endpoints are enabled when `"websockets": true` in the config. They
are useful when you want a long-lived connection for many commands, or when
your client already speaks WebSocket and you want to avoid per-request HTTP
overhead.

- `/.json`: JSON array commands and JSON responses
- `/.raw`: raw RESP frames in/out

With `/.json`, send a JSON array where the first element is the command and the
rest are arguments. The server responds with a JSON-encoded Redis reply.

JSON example:

```json
["SET", "hello", "world"]
```

With `/.raw`, you are responsible for framing the request in RESP. This is the
right choice when you need full fidelity (binary keys/values, streaming, or
existing RESP tooling).

Raw RESP example:

```text
*2\r\n$4\r\nPING\r\n$4\r\nPONG\r\n
```

Connections stay open until the client closes them. You can send multiple
commands over the same socket.

## Hiredis Compat Endpoints

When `compat_hiredis.enabled` is true, redis-web also exposes session-oriented
compat routes (default prefix: `/__compat`):

- `POST /__compat/session`
- `DELETE /__compat/session/{session_id}`
- `POST /__compat/cmd/{session_id}.raw`
- `GET /__compat/stream/{session_id}.raw`
- `GET /__compat/ws/{session_id}`

These endpoints are intended for hiredis compatibility shims that need
connection-scoped behavior while tunneling over HTTP/WebSocket.

### Compatibility bridge examples

Create a session and capture the session id:

```bash
BASE=http://127.0.0.1:7379
SESSION_ID=$(curl -sS -X POST "$BASE/__compat/session" | \
  sed -n 's/.*"session_id":"\([^"]*\)".*/\1/p')
```

Issue command traffic to one session with RESP frames:

```bash
curl -sS -X POST "$BASE/__compat/cmd/$SESSION_ID.raw" \
  --data-binary $'*3\r\n$3\r\nSET\r\n$6\r\nmy:key\r\n$7\r\nhello\r\n'

curl -sS -X POST "$BASE/__compat/cmd/$SESSION_ID.raw" \
  --data-binary $'*2\r\n$3\r\nGET\r\n$6\r\nmy:key\r\n'
```

Open the stream feed for pub/sub traffic:

```bash
curl -sS -X POST "$BASE/__compat/cmd/$SESSION_ID.raw" \
  --data-binary $'*2\r\n$9\r\nSUBSCRIBE\r\n$6\r\nalpha\r\n' >/dev/null

curl -N "$BASE/__compat/stream/$SESSION_ID.raw"
```

Remove a session after use:

```bash
curl -sS -X DELETE "$BASE/__compat/session/$SESSION_ID"
```

If HTTP-stream pub/sub is used, redis-web emits a one-time warning by default.
Set `REDIS_WEB_COMPAT_MUTE_HTTP_PUBSUB_WARNING=1` to suppress it.

## Practical compat route runbook

### Route prefix overrides

The compatibility routes are mounted under `compat_hiredis.path_prefix` and
normalized before registration.

```json
{
  "compat_hiredis": {
    "path_prefix": "/compat"
  }
}
```

mounts to `/compat/session` instead of `/__compat/session`.

```bash
BASE=http://127.0.0.1:7379
curl -sS -X POST "$BASE/compat/session"
```

### Command and session checks

For every request on `/.../cmd/{session}.raw`, redis-web parses all concatenated RESP
frames in one request and executes them in order.

```bash
SESSION_ID=...

# command + pipelined command in one request
curl -sS -X POST "$BASE/compat/cmd/$SESSION_ID.raw" \
  --data-binary $'*2\r\n$3\r\nGET\r\n$7\r\nhealthz\r\n*2\r\n$3\r\nPING\r\n$4\r\nping\r\n'
```

Behavior notes:

- Empty body returns `-ERR Empty command body` with HTTP `400`.
- Unsupported parse shape returns `-ERR Invalid RESP command` with HTTP `400`.
- `-ERR forbidden` is emitted per disallowed command while allowing other commands in
  the same pipeline to execute.

### Limits and lifecycle

- `max_pipeline_commands` controls how many RESP frames can be sent per request.
  Exceeding this returns HTTP `400` with `-ERR Pipelined command limit exceeded`.
- `max_sessions` caps total concurrent active sessions. Requests returning `429` include:
  `{"error":"compat session limit reached"}`.
- Idle sessions are evicted on request-driven access checks once
  `session_ttl_sec` elapsed since last command interaction.
- `DELETE /__compat/session/{session_id}` returns `204` for successful cleanup and
  `404` for unknown sessions.

### Transport mode and startup check

Compat endpoints are mounted only when:

- `transport_mode` is `rest`
- `compat_hiredis.enabled` is true

Use the command below to confirm mount behavior in config-only runs:

```bash
redis-web --write-default-config --config /tmp/default.json
```

### Troubleshooting

- If `POST /__compat/session` returns `503`, Redis backend startup is not yet ready.
- If responses are all `-ERR forbidden`, verify ACL configuration and auth headers.
  The compat endpoints share the same ACL policy as normal request handlers.
- If you still receive `404` for a valid session ID, confirm that your client uses
  exactly the session ID returned in `POST /__compat/session`.

### Test ownership map

- API examples and runtime flow assertions:
  `crates/redis-web/tests/integration_hiredis_compat_test.rs`
- Limits, path normalization, and auth/forbidden behavior are tracked in the same test
  suite and should be expanded before publishing endpoint runbook claims.
- `compat_hiredis` decoding and defaults: `crates/redis-web/tests/config_test.rs`

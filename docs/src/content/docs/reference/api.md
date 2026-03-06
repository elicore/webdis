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

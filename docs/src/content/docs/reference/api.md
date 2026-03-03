---
title: HTTP and WebSocket API
description: Command encoding, response formats, and WebSocket endpoints.
---

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

# Webdis (Rust)

This repository contains a Rust rewrite of Webdis: a lightweight HTTP/WebSocket gateway for Redis. The goal of this implementation is to keep the original feature set while providing modern tooling, structured configuration, and a comprehensive automated test suite.

Key highlights:

- Axum-based HTTP server with async Redis connection pooling via `deadpool-redis`
- Optional WebSocket endpoint for Pub/Sub streaming
- Drop-in compatibility with the original `webdis.json` (captured as `webdis.legacy.json`)
- JSON schema-backed configuration with sensible defaults and editor validation

## Quick start

```sh
cargo run --release -- webdis.json
```

If you omit the config path, the binary looks for `webdis.json` in the current directory. To scaffold a fresh configuration that references the JSON schema:

```sh
webdis --write-default-config --config /path/to/webdis.json
```

The command refuses to overwrite existing files.

## Using Webdis

### Basic HTTP examples

```sh
# SET key/value
curl http://127.0.0.1:7379/SET/hello/world
# -> {"SET":"OK"}

# GET key
curl http://127.0.0.1:7379/GET/hello
# -> {"GET":"world"}

# POST body: command in the request body
curl -XPOST -d 'GET/hello' http://127.0.0.1:7379/
```

### Command format

Requests encode Redis commands in the URI:

- `GET /COMMAND/arg0/.../argN[.ext]`
- `POST /` with `COMMAND/arg0/.../argN` in the HTTP body
- `PUT /COMMAND/arg0/.../argN-1` with `argN` in the HTTP body

The optional `.ext` suffix selects the output format (see below). Arguments should be URL-encoded as usual.

### HTTP status codes

- `200 OK` – command executed successfully.
- `400 Bad Request` – malformed or empty command.
- `403 Forbidden` – command rejected by ACL.
- `500 Internal Server Error` – unexpected Redis or server error.
- `503 Service Unavailable` – Redis connection pool unavailable.

### JSON and other output formats

JSON is the default:

```sh
# string
curl http://127.0.0.1:7379/GET/y
# -> {"GET":"41"}

# number
curl http://127.0.0.1:7379/INCR/y
# -> {"INCR":42}
```

Other formats:

- `.raw` or `?type=raw` – plain-text representation (useful for CLI/debugging).
- `.msg` / `.msgpack` or `?type=msg` / `?type=msgpack` – MessagePack (`application/x-msgpack`).

### File upload

`PUT` uses the HTTP body as the last argument, which is handy for sending JSON or other UTF‑8 payloads:

```sh
echo '{"a":1,"b":"c"}' > doc.json
curl --upload-file doc.json http://127.0.0.1:7379/SET/json_key
curl http://127.0.0.1:7379/GET/json_key
```

### WebSockets

When `"websockets": true` is set in the config, Webdis enables a JSON WebSocket endpoint at `/.json`:

- Connect to `ws://host:port/.json`.
- Send commands as JSON arrays, e.g. `["SET", "hello", "world"]`.
- Receive responses as JSON objects, e.g. `{"SET":"OK"}`.

Example (browser console):

```javascript
const socket = new WebSocket("ws://127.0.0.1:7379/.json");
socket.onmessage = (e) => console.log("WS:", e.data);
socket.onopen = () => {
  socket.send(JSON.stringify(["SET", "hello", "world"]));
  socket.send(JSON.stringify(["GET", "hello"]));
};
```

### Pub/Sub

Pub/Sub is available both over HTTP and WebSockets:

- HTTP: `GET /SUBSCRIBE/channel` streams messages as Server-Sent Events (SSE).
- WebSocket: send `["SUBSCRIBE", "channel"]` over `/.json` and receive messages as `{"message": "payload"}`.

## Configuration

- All configuration is expressed as JSON and validated by `webdis.schema.json`.
- `webdis.json` is the canonical sample for the Rust server.
- `webdis.legacy.json` mirrors the historical C configuration and remains fully supported.
- Every field below matches the descriptions in the schema; optional values fall back to the listed defaults.

| Key | Description | Optional | Type | Default |
| --- | --- | --- | --- | --- |
| `$schema` | Path or URL to this schema file so editors can enable validation. | Yes | string | `./webdis.schema.json` |
| `redis_host` | Hostname or IP address of the target Redis server. | Yes | string | `127.0.0.1` |
| `redis_port` | TCP port of the target Redis server. | Yes | integer | `6379` |
| `redis_auth` | Authentication parameters passed to Redis (string password or `[username, password]`). | Yes | string / array | _unset_ |
| `http_host` | Interface Webdis binds to for HTTP traffic. | Yes | string | `0.0.0.0` |
| `http_port` | Port Webdis listens on. | Yes | integer | `7379` |
| `http_threads` | Number of Tokio worker threads dedicated to HTTP handling. | Yes | integer | `4` |
| `threads` | Legacy alias for `http_threads`; prefer `http_threads` when possible. | Yes | integer | _deprecated_ |
| `pool_size_per_thread` | Number of Redis connections allocated per HTTP worker thread. | Yes | integer | `10` |
| `pool_size` | Legacy alias for `pool_size_per_thread`; prefer the canonical name. | Yes | integer | _deprecated_ |
| `database` | Redis logical database index selected after connecting. | Yes | integer | `0` |
| `daemonize` | Whether to run Webdis in the background as a daemon. | Yes | boolean | `false` |
| `pidfile` | Override path to the PID file when running as a daemon. | Yes | string | `webdis.pid` (when daemonized) |
| `websockets` | Enable WebSocket endpoint (/.json) for Pub/Sub and command execution. | Yes | boolean | `false` |
| `ssl` | Configuration for TLS connections to Redis (see below). | Yes | object | _unset_ |
| `acl` | Ordered list of ACL rules evaluated from top to bottom. | Yes | array | _unset_ |
| `http_max_request_size` | Maximum accepted HTTP request size in bytes. | Yes | integer | `134217728` (128 MiB) |
| `user` | Drop privileges to this Unix user before serving requests. | Yes | string | _unset_ |
| `group` | Drop privileges to this Unix group before serving requests. | Yes | string | _unset_ |
| `default_root` | Redis command executed when `/` is requested, e.g. `/GET/index.html`. | Yes | string | _unset_ |
| `verbosity` | Logging verbosity level (`0 = error`, `4 = debug`, `>=5 = trace`). | Yes | integer | `4` |
| `logfile` | Path to the log file; stdout/stderr are used when unset. | Yes | string | _unset_ |
| `log_fsync` | Controls how aggressively Webdis fsyncs its logs (`auto`, `all`, or milliseconds). | Yes | string / integer | _unset_ |
| `hiredis` | Legacy Hiredis keep-alive settings kept for compatibility (ignored by the Rust server). | Yes | object | _unset_ |

### Nested structures

- **`ssl`**
  - `enabled` (bool, required when present): Set to `true` for TLS.
  - `ca_cert_bundle` (string, required): CA bundle path.
  - `path_to_certs` (string, optional): Directory of trusted certificates.
  - `client_cert` / `client_key` (strings, required): Client credentials for mutual TLS.
  - `redis_sni` (string, optional): Override SNI hostname.

- **`acl` elements**
  - `disabled` / `enabled`: Arrays of command names.
  - `http_basic_auth`: `username:password` string for routing decisions.
  - `ip`: IP or CIDR filter.

## Compatibility

- `threads` → `http_threads` and `pool_size` → `pool_size_per_thread` remain accepted; when both the legacy and canonical keys are present, the canonical key wins.
- `webdis.legacy.json` and `webdis.prod.json` continue to load without modification so you can upgrade deployments incrementally.
- Legacy-only sections such as `hiredis` are parsed but ignored by the new server so they do not trigger validation errors.

## Testing

| Command | Purpose |
| --- | --- |
| `cargo test --test config_test` | Validates configuration parsing, defaults, schema-backed helpers, and legacy alias precedence (fast, no Redis needed). |
| `cargo test --test integration_test` | Spins up the server, exercises HTTP/WebSocket/ACL behavior against a real Redis instance, and enforces request-size limits. Requires permission to bind ephemeral ports and connect to Redis on `127.0.0.1:6379`. |

Integration tests spawn a temporary configuration per case, so they can run in parallel. If your environment restricts binding to random localhost ports, run the integration suite in a sandbox that allows it.

## Development workflow

1. Edit configuration or code.
2. Run the fast config tests (`cargo test --test config_test`).
3. Run the full suite (including integration) when you can bind local ports.
4. Use `webdis --write-default-config` whenever you introduce new options to keep the sample config and schema aligned.

That’s it—Webdis remains tiny, easy to configure, and now benefits from Rust’s safety guarantees plus structured documentation.

# Webdis (Rust)

This repository contains a Rust rewrite of Webdis: a lightweight HTTP/WebSocket gateway for Redis. The goal of this implementation is to keep the original feature set while providing modern tooling, structured configuration, and a comprehensive automated test suite.

Key highlights:

- Axum-based HTTP server with async Redis connection pooling via `deadpool`
- Optional WebSocket endpoint for Pub/Sub streaming
- Drop-in compatibility with the original `webdis.json` (captured as `webdis.legacy.json`)
- JSON schema-backed configuration with sensible defaults and editor validation

## Features

Webdis (Rust) implements a robust set of features for interacting with Redis over HTTP:

- **Command Support**: Execute any Redis command via RESTful URIs.
- **Output Formats**: Supports JSON (default), Raw RESP (`.raw`), and MessagePack (`.msgpack`).
- **Pub/Sub**: Stream message via Server-Sent Events (SSE) or WebSockets.
- **WebSockets**: Bi-directional command execution and Pub/Sub over a single connection.
- **Caching (ETag)**: Automatic `ETag` generation for `GET` requests with support for `If-None-Match` to return `304 Not Modified`.
- **ACL & Security**: Network-based and basic-auth-based Access Control Lists.
- **Connection Pooling**: High-performance async connection pooling.
- **Daemonization**: Run as a system daemon with PID file support and privilege dropping.

## Quick start

```sh
cargo run --release -- webdis.json
```

If you omit the config path, the binary looks for `webdis.json` in the current directory. To scaffold a fresh configuration that references the JSON schema:

```sh
webdis --write-default-config --config /path/to/webdis.json
```

The command refuses to overwrite existing files.

## Docker & deployments

This repository includes Docker & Compose examples for development and production scenarios under `docs/docker/`. Important notes:

- The `Dockerfile` in this repository builds a Rust `webdis` binary only; it does not include an embedded `redis-server`. Some other images (that have historically bundled Redis) may still exist; this repository’s images and examples avoid referencing those upstream variants and instead use the repo-built `elicore/webdis` image or local builds.
- Development compose (`docker-compose.dev.yml`) builds a local image and runs Redis as a sidecar for quick testing.
- Production examples (`docker-compose.prod.yml`) favor pinned images, named volumes, secrets, and external reverse proxies.
- TLS and RDB import examples are also provided under `docs/docker/` and require additional setup described in those docs.

Open `docs/docker/README.md` for more information and a walkthrough of the recommended files and scripts.

Quick demo (local):

````bash
# Build & run Webdis with local Redis via Compose
docker compose -f docker-compose.dev.yml up --build

# Stop and remove the running containers/volumes after testing
docker compose -f docker-compose.dev.yml down -v

Makefile targets (shortcuts):

```bash
# build local dev image (webdis:dev)
make docker-build-dev

# build & tag the org image (elicore/webdis:latest)
make docker-build

# push the org image (needs Docker credentials)
make docker-push

# start dev compose stack
make compose-up-dev

# stop & remove volumes
make compose-down-dev
````

````



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
````

### Command format

Requests encode Redis commands in the URI:

- `GET /COMMAND/arg0/.../argN[.ext]`
- `GET /<db>/COMMAND/arg0/.../argN[.ext]` to target a specific Redis logical database for that request
- `POST /` with `COMMAND/arg0/.../argN` in the HTTP body
- `PUT /COMMAND/arg0/.../argN-1` with `argN` in the HTTP body

The optional `.ext` suffix selects the output format (see below). Arguments should be URL-encoded as usual.

#### Per-request database selection

When the first path segment is a decimal integer and the second is a command, Webdis routes that
request to the specified Redis logical database:

- `/7/GET/key` runs `GET key` against database `7`.
- `/GET/key` (no prefix) runs against the configured default `database`.

This implementation uses **lazy per-database connection pools**:

- The default DB pool is created at startup.
- A non-default DB pool is created only when first requested.
- Pools are then reused per DB, which avoids per-request `SELECT` overhead and prevents DB bleed
  across pooled connections.

If the DB prefix is numeric but out of range (supported range `0..=255`), Webdis returns
`400 Bad Request`.

#### URL percent-encoding semantics (`%2f`, `%2e`)

Webdis splits command arguments on **literal** `/` characters in the request path. To support keys/arguments
that contain `/` or `.`, Webdis applies percent-decoding **per path segment** after splitting:

- `%2F` / `%2f` decodes to `/` *inside* an argument, never as a separator.
- `%2E` / `%2e` decodes to `.` *inside* an argument without triggering output-format suffix parsing.
- Output-format selection via `.ext` is based only on the **unescaped** representation (a literal `.` in the URL).
- Invalid percent-encodings are left unchanged.

Examples:

- Key containing `/`:
  - `GET /SET/a%2Fb/value` sets key `a/b`
  - `GET /GET/a%2Fb` reads key `a/b`
- Key containing `.` without selecting a format:
  - `GET /SET/a%2Eb/world` sets key `a.b`
  - `GET /GET/a%2Eb%2Eraw` reads key `a.b.raw` **as JSON** (no `.raw` suffix was present in the URL)
- Explicit suffix still works:
  - `GET /GET/a%2Eb.raw` reads key `a.b` in raw RESP mode

### HTTP status codes

- `200 OK` – command executed successfully.
- `400 Bad Request` – malformed or empty command.
- `403 Forbidden` – command rejected by ACL.
- `500 Internal Server Error` – unexpected Redis or server error.
- `503 Service Unavailable` – Redis connection pool unavailable.

### Output Formats

JSON is the default:

```sh
# string
curl http://127.0.0.1:7379/GET/y
# -> {"GET":"41"}

# number
curl http://127.0.0.1:7379/INCR/y
# -> {"INCR":42}
```

JSONP is supported for HTTP JSON responses via the `jsonp` (preferred) or `callback` query parameters:

```sh
curl "http://127.0.0.1:7379/GET/y?jsonp=myFn"
# -> myFn({"GET":"41"})
```

- If both `jsonp` and `callback` are present, `jsonp` takes precedence.
- Callback function names are passed through unchanged (minimal validation).
- JSONP responses use `Content-Type: application/javascript; charset=utf-8`.
- JSONP is ignored for non-JSON formats (`.raw`, `.msg`/`.msgpack`, `.txt`, `.html`, etc.).
- Error responses preserve their HTTP status code, but the JSON error payload is still wrapped when JSONP is requested.

The `INFO` command output is automatically parsed into a structured JSON object for easier programmatic inspection, rather than returning the raw multi-line string. This behavior also applies to `CLUSTER INFO`.

Other formats:

- Default (no suffix) or `.json` – JSON envelope (`application/json`).
- `.msg` / `.msgpack` – MessagePack envelope (`application/x-msgpack`).
- `.raw` – raw Redis Protocol (RESP) output (useful for debugging or RESP clients).

Passthrough content types (for Redis string replies):

- `.txt` – `text/plain`
- `.html` – `text/html`
- `.xhtml` – `application/xhtml+xml`
- `.xml` – `text/xml`
- `.png` – `image/png`
- `.jpg` / `.jpeg` – `image/jpeg`

`?type=<mime>` overrides the HTTP `Content-Type` header **without changing the body format**.

Example (JSON body, overridden header):

```sh
curl "http://127.0.0.1:7379/GET/hello?type=application/pdf"
# -> {"GET":"world"}   (but Content-Type: application/pdf)
```

Example (binary passthrough with an image extension):

```sh
curl --upload-file logo.png http://127.0.0.1:7379/SET/logo
curl http://127.0.0.1:7379/GET/logo.png > downloaded.png
```

### File upload

`PUT` uses the HTTP body as the last argument, which is handy for sending JSON, HTML, or binary payloads:

```sh
echo '{"a":1,"b":"c"}' > doc.json
curl --upload-file doc.json http://127.0.0.1:7379/SET/json_key
curl http://127.0.0.1:7379/GET/json_key
```

### WebSockets

When `"websockets": true` is set in the config, Webdis enables two bi-directional endpoints:

#### 1. JSON Endpoint (`/.json`)

- Send commands as JSON arrays, e.g. `["SET", "key", "val"]`.
- Receive responses as JSON objects, e.g. `{"SET":"OK"}`.
- Pub/Sub messages are received as `{"message": "payload"}`.

Example:

```javascript
const socket = new WebSocket("ws://127.0.0.1:7379/.json");
socket.onmessage = (e) => console.log("JSON WS:", e.data);
socket.onopen = () => {
  socket.send(JSON.stringify(["SET", "hello", "world"]));
};
```

#### 2. Raw RESP Endpoint (`/.raw`)

- Send and receive raw Redis Serialization Protocol (RESP) frames.
- This allows full protocol transparency for clients that prefer RESP over JSON.

Example:

```javascript
const socket = new WebSocket("ws://127.0.0.1:7379/.raw");
socket.onmessage = async (e) => {
  const data = await e.data.arrayBuffer();
  console.log("RESP WS (raw bytes):", new Uint8Array(data));
};
socket.onopen = () => {
  // Send "SET hello world" in RESP: *3\r\n$3\r\nSET\r\n$5\r\nhello\r\n$5\r\nworld\r\n
  const encoder = new TextEncoder();
  socket.send(
    encoder.encode("*3\r\n$3\r\nSET\r\n$5\r\nhello\r\n$5\r\nworld\r\n"),
  );
};
```

### Pub/Sub

Pub/Sub is available both over HTTP and WebSockets:

- HTTP: `GET /SUBSCRIBE/channel` supports SSE (default), chunked JSON stream, and JSONP Comet mode.
- WebSocket: send `["SUBSCRIBE", "channel"]` over `/.json` and receive messages as `{"message": "payload"}`.

HTTP mode selection for `/SUBSCRIBE/channel`:

- **SSE (default):** no JSONP query param and no explicit JSON `Accept` negotiation.
- **Chunked JSON stream:** set `Accept: application/json`.
- **Chunked JSONP Comet:** pass `?jsonp=myFn` (or `?callback=myFn`).

Chunked JSON line format:

```json
{"SUBSCRIBE":["message","channel","payload"]}
```

Chunked JSONP line format:

```js
myFn({"SUBSCRIBE":["message","channel","payload"]});
```

Examples:

```sh
# SSE (default)
curl -N http://127.0.0.1:7379/SUBSCRIBE/news

# Chunked JSON stream
curl -N -H "Accept: application/json" http://127.0.0.1:7379/SUBSCRIBE/news

# JSONP Comet stream
curl -N "http://127.0.0.1:7379/SUBSCRIBE/news?jsonp=myFn"
```

## Configuration

- All configuration is expressed as JSON and validated by `webdis.schema.json`.
- `webdis.json` is the canonical sample for the Rust server.
- `webdis.legacy.json` mirrors the historical C configuration and remains fully supported.
- Every field below matches the descriptions in the schema; optional values fall back to the listed defaults.

### Environment variables

For compatibility with the original Webdis, configuration supports environment variable expansion:

- Any JSON **string** value that is exactly of the form `$VARNAME` is expanded from the process environment.
- `VARNAME` must be non-empty and contain only `A–Z`, `0–9`, and `_` (uppercase only).
- If the environment variable is missing, config loading fails with an error that names the missing var and the config key.

Example:

```json
{
  "redis_host": "$REDIS_HOST",
  "redis_port": "$REDIS_PORT"
}
```

| Key                     | Description                                                                             | Optional | Type             | Default                        |
| ----------------------- | --------------------------------------------------------------------------------------- | -------- | ---------------- | ------------------------------ |
| `$schema`               | Path or URL to this schema file so editors can enable validation.                       | Yes      | string           | `./webdis.schema.json`         |
| `redis_host`            | Hostname or IP address of the target Redis server.                                      | Yes      | string           | `127.0.0.1`                    |
| `redis_port`            | TCP port of the target Redis server.                                                    | Yes      | integer          | `6379`                         |
| `redis_socket`          | Filesystem path to a Redis UNIX socket. When set, takes precedence over host/port. TLS is not applicable. | Yes | string | _unset_ |
| `redis_auth`            | Authentication parameters passed to Redis (string password or `[username, password]`).  | Yes      | string / array   | _unset_                        |
| `http_host`             | Interface Webdis binds to for HTTP traffic.                                             | Yes      | string           | `0.0.0.0`                      |
| `http_port`             | Port Webdis listens on.                                                                 | Yes      | integer          | `7379`                         |
| `http_threads`          | Number of Tokio worker threads dedicated to HTTP handling.                              | Yes      | integer          | `4`                            |
| `threads`               | Legacy alias for `http_threads`; prefer `http_threads` when possible.                   | Yes      | integer          | _deprecated_                   |
| `pool_size_per_thread`  | Number of Redis connections allocated per HTTP worker thread.                           | Yes      | integer          | `10`                           |
| `pool_size`             | Legacy alias for `pool_size_per_thread`; prefer the canonical name.                     | Yes      | integer          | _deprecated_                   |
| `database`              | Redis logical database index selected after connecting.                                 | Yes      | integer          | `0`                            |
| `daemonize`             | Whether to run Webdis in the background as a daemon.                                    | Yes      | boolean          | `false`                        |
| `pidfile`               | Override path to the PID file when running as a daemon.                                 | Yes      | string           | `webdis.pid` (when daemonized) |
| `websockets`            | Enable WebSocket endpoint (/.json) for Pub/Sub and command execution.                   | Yes      | boolean          | `false`                        |
| `ssl`                   | Configuration for TLS connections to Redis (see below).                                 | Yes      | object           | _unset_                        |
| `acl`                   | Ordered list of ACL rules evaluated from top to bottom.                                 | Yes      | array            | _unset_                        |
| `http_max_request_size` | Maximum accepted HTTP request size in bytes.                                            | Yes      | integer          | `134217728` (128 MiB)          |
| `user`                  | Drop privileges to this Unix user before serving requests.                              | Yes      | string           | _unset_                        |
| `group`                 | Drop privileges to this Unix group before serving requests.                             | Yes      | string           | _unset_                        |
| `default_root`          | Redis command executed when `/` is requested, e.g. `/GET/index.html`.                   | Yes      | string           | _unset_                        |
| `verbosity`             | Logging verbosity level (`0 = error`, `4 = debug`, `>=5 = trace`).                      | Yes      | integer          | `4`                            |
| `logfile`               | Path to the log file; stdout/stderr are used when unset.                                | Yes      | string           | _unset_                        |
| `log_fsync`             | Controls log fsync behavior: `auto` = no explicit fsync, `all` = fsync after each log write (expensive), or an integer `N` = fsync at most once per `N` ms. | Yes | string / integer | _unset_ |
| `hiredis`               | Legacy Hiredis keep-alive settings kept for compatibility. `keep_alive_sec` tunes TCP keep-alive for Redis TCP/TLS connections (not UNIX sockets). | Yes | object | _unset_ |

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
- Legacy-only sections such as `hiredis` are parsed for compatibility. `hiredis.keep_alive_sec` is honored for Redis TCP/TLS connections and ignored for UNIX sockets (`redis_socket`).

## Testing

| Tier | Command | Purpose |
| --- | --- | --- |
| `unit` | `cargo test --lib` | Fast pure logic checks in `src/` with no network/Redis dependency. |
| `functional` | `cargo test --test config_test --test handler_test --test logging_fsync_test --test functional_interface_mapping_test --test functional_http_contract_test --test functional_ws_contract_test` | Non-Redis HTTP/WS/interface contract checks using injected test executors. |
| `integration` | `cargo test --test integration_process_boot_test --test integration_redis_http_test --test integration_redis_pubsub_test --test integration_redis_socket_test --test websocket_raw_test` | Real Redis/process/socket behavior and end-to-end runtime interactions. |
| `compile-guard` | `cargo test --tests --no-run` | Compiles all external test crates to catch harness drift early. |

CI policy:
- Pull requests run `unit + functional`.
- Pushes to `main` and scheduled builds run the full suite (`unit + functional + integration`).

Local GitHub Actions validation:
- Run Linux matrix entries locally before push with `make ci_local`.
- Requires Docker plus `act` installed on your machine.

## Development workflow

1. Edit configuration or code.
2. Run the fast default gate (`cargo test --lib` plus functional tier).
3. Run the integration tier when you can bind local ports and reach Redis.
4. Use `webdis --write-default-config` whenever you introduce new options to keep the sample config and schema aligned.

## Library embedding

Webdis can be embedded as a library in another Axum/Tokio process.

- Use `webdis::server::build_router(&config)` for default parser + Redis executor wiring.
- Use `webdis::server::build_router_with_dependencies(...)` to inject custom parser/executor implementations.
- Keep CLI-only concerns (daemonize, privilege drop, process exit) in the binary entrypoint.

See `docs/library-embedding.md` for the overview and `docs/embedding/README.md` for separate implementation pages (interfaces, sidecar mount, policy executor, tenant parser, and test stubs).

That’s it—Webdis remains tiny, easy to configure, and now benefits from Rust’s safety guarantees plus structured documentation.

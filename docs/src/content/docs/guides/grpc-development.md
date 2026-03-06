---
title: gRPC Development
description: Run redis-web in gRPC mode and exercise the RedisGateway service locally.
---

Use this guide when you want to develop against the gRPC transport instead of
the REST and WebSocket surface.

## Example config

The repository includes a sample config at
`docs/examples/config/redis-web.grpc.json`:

```json
{
  "$schema": "https://raw.githubusercontent.com/elicore/redis-web/main/redis-web.schema.json",
  "redis_host": "127.0.0.1",
  "redis_port": 6379,
  "transport_mode": "grpc",
  "grpc": {
    "host": "127.0.0.1",
    "port": 7379,
    "enable_health_service": true,
    "enable_reflection": true,
    "max_decoding_message_size": 16777216,
    "max_encoding_message_size": 16777216
  },
  "verbosity": 4,
  "logfile": "redis-web.log"
}
```

This keeps the listener local-only and enables reflection so `grpcurl` can
discover the service without a separate proto step.

## Start the server

```bash
cargo run -p redis-web --bin redis-web -- docs/examples/config/redis-web.grpc.json
```

In this mode, redis-web serves only gRPC. The HTTP, WebSocket, and compat
surfaces are not exposed.

## Inspect the service with grpcurl

List the available services:

```bash
grpcurl -plaintext 127.0.0.1:7379 list
```

Describe the Redis gateway schema:

```bash
grpcurl -plaintext 127.0.0.1:7379 describe redis_web.v1.RedisGateway
```

Check server health:

```bash
grpcurl -plaintext \
  -d '{"service":"redis_web.v1.RedisGateway"}' \
  127.0.0.1:7379 \
  grpc.health.v1.Health/Check
```

## Execute commands

Unary `Execute` accepts a Redis command name and binary-safe arguments. `grpcurl`
expects `bytes` fields as base64 in JSON input.

Set `hello = world`:

```bash
grpcurl -plaintext \
  -d '{"command":"SET","args":["aGVsbG8=","d29ybGQ="]}' \
  127.0.0.1:7379 \
  redis_web.v1.RedisGateway/Execute
```

Read the value back:

```bash
grpcurl -plaintext \
  -d '{"command":"GET","args":["aGVsbG8="]}' \
  127.0.0.1:7379 \
  redis_web.v1.RedisGateway/Execute
```

Target a non-default database:

```bash
grpcurl -plaintext \
  -d '{"database":7,"command":"PING"}' \
  127.0.0.1:7379 \
  redis_web.v1.RedisGateway/Execute
```

## Subscribe to Pub/Sub traffic

Start a subscription stream:

```bash
grpcurl -plaintext \
  -d '{"channel":"updates"}' \
  127.0.0.1:7379 \
  redis_web.v1.RedisGateway/Subscribe
```

In another terminal, publish a message through Redis:

```bash
redis-cli PUBLISH updates hello
```

The streamed event carries `channel` and `payload` as `bytes`, so `grpcurl`
prints them as base64-encoded JSON fields.

## Rust client example

For application code, generate a client from
`crates/redis-web-runtime/proto/redis_web/v1/gateway.proto` with `tonic-build`
or your language-specific protobuf toolchain. A minimal tonic call looks like
this after code generation:

```rust
use redis_web::v1::{redis_gateway_client::RedisGatewayClient, CommandRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RedisGatewayClient::connect("http://127.0.0.1:7379").await?;

    let reply = client
        .execute(CommandRequest {
            command: "SET".into(),
            database: None,
            args: vec![b"hello".to_vec(), b"world".to_vec()],
        })
        .await?
        .into_inner();

    println!("{reply:?}");
    Ok(())
}
```

If you need ad hoc inspection only, `grpcurl` plus reflection is usually the
fastest development loop.

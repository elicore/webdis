---
title: Hiredis Compat Functional Tests
description: Functional test matrix for hiredis-compatible redis-web behavior.
---

## Test Matrix

Functional coverage should include:
- basic command roundtrips
- binary-safe payloads
- pipelined commands
- transaction flows
- reconnect/error-path behavior
- DB selection and connection-scoped state
- Pub/Sub subscribe/pattern/unsubscribe flows

Transport matrix should include:
- forced WS mode
- forced HTTP mode
- auto mode with fallback
- direct bypass mode

## Expected Output Shape

For raw compat command paths, responses should be RESP-framed bytes.

Examples:
- success: `+OK\r\n`
- bulk: `$5\r\nvalue\r\n`
- integer: `:1\r\n`
- error: `-ERR ...\r\n`

## Failure Interpretation

Common failure classes:
- ACL/auth denial
- invalid RESP framing
- session expiry due to TTL
- compat pipeline command-limit exceeded
- upstream Redis unavailability

## Where to Run

Use the repository test targets plus dedicated compat suites as they are added.

Useful commands:

```bash
cargo test -p redis-web --test integration_hiredis_compat_test
make test_hiredis_compat_fixture
```

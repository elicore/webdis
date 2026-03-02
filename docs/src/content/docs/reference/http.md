---
title: HTTP
description: Command encoding, DB prefix semantics, and status mapping.
---

Command patterns:

- `GET /COMMAND/arg0/.../argN[.ext]`
- `GET /<db>/COMMAND/...` for per-request DB selection
- `POST /` with command path in request body
- `PUT /COMMAND/...` with final argument in request body

Example:

```bash
curl http://127.0.0.1:7379/7/GET/key
```

Status mapping:

- `200` success
- `400` malformed command
- `403` ACL denial
- `500` execution/runtime error
- `503` Redis unavailable

---
title: WebSocket
description: JSON and raw RESP socket endpoints.
---

Enabled when `"websockets": true`.

- `/.json`: JSON array commands and JSON responses
- `/.raw`: raw RESP frames in/out

JSON example:

```json
["SET", "hello", "world"]
```

Raw RESP example:

```text
*2\r\n$4\r\nPING\r\n$4\r\nPONG\r\n
```

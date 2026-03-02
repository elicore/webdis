---
title: Response Formats
description: JSON, MessagePack, raw RESP, and passthrough content types.
---

Format selection via extension suffix:

- `.json` (default)
- `.msg` / `.msgpack`
- `.raw`

Passthrough MIME mappings include `.txt`, `.html`, `.xml`, `.png`, `.jpg`.

Header override without body format change:

```bash
curl "http://127.0.0.1:7379/GET/hello?type=application/pdf"
```

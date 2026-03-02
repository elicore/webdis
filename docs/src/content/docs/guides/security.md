---
title: Security
description: ACL, auth, and TLS posture.
---

Key controls:

- ACL command allow/deny rules (`acl`)
- Optional HTTP basic auth routing in ACL rules
- Redis TLS client settings (`ssl`)
- Non-root runtime container user in Dockerfile

Example ACL section:

```json
"acl": [
  { "disabled": ["DEBUG"] },
  { "http_basic_auth": "user:password", "enabled": ["DEBUG"] }
]
```

---
title: Deployment (Docker and Security)
description: Local and production Docker workflows plus core security controls.
---

## Docker development

Use the repository compose file when you want a repeatable local stack. It
starts Redis plus `redis-web`, exposes port `7379`, and mounts the local
`redis-web.json` so edits take effect on restart.

```bash
docker compose -f docker/docker-compose.dev.yml up --build
curl http://127.0.0.1:7379/PING
docker compose -f docker/docker-compose.dev.yml down -v
```

The dev compose file builds `redis-web:dev` locally and mounts `redis-web.json`.

## Docker production

For production, prefer pinned image tags and explicitly managed configuration
files or secrets. This keeps upgrades predictable and avoids surprise changes.

```yaml
services:
  redis-web:
    image: ghcr.io/elicore/redis-web:1.0.0
```

```bash
docker compose -f docker/docker-compose.prod.yml up -d
docker compose -f docker/docker-compose.prod.yml logs -f redis-web
```

After startup, validate with a simple request and wire the health check or
monitoring you already use for Redis-adjacent services.

## Security posture

Core controls:

- ACL command allow/deny rules (`acl`)
- Optional HTTP basic auth routing in ACL rules
- Redis TLS client settings (`ssl`)
- Non-root runtime container user in the Dockerfile

Example ACL section:

```json
"acl": [
  { "disabled": ["DEBUG"] },
  { "http_basic_auth": "user:password", "enabled": ["DEBUG"] }
]
```

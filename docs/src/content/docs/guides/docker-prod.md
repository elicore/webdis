---
title: Docker Production
description: Production-oriented compose and image references.
---

Use pinned image tags in production:

```yaml
services:
  redis-web:
    image: ghcr.io/elicore/redis-web:1.0.0
```

Base production sample:

```bash
docker compose -f docker-compose.prod.yml up -d
docker compose -f docker-compose.prod.yml logs -f redis-web
```

Also see [Release and Signing](/maintainers/release/).

---
title: Docker Dev
description: Local development stack with Redis sidecar.
---

Use the repository compose file:

```bash
docker compose -f docker/docker-compose.dev.yml up --build
curl http://127.0.0.1:7379/PING
docker compose -f docker/docker-compose.dev.yml down -v
```

The dev compose file builds `redis-web:dev` locally and mounts `redis-web.json`.

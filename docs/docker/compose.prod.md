# Production Docker Compose

This file documents `docker-compose.prod.yml`, a production-style docker-compose example. It shows how you might run Webdis in a single-host, resilient setup with named volumes, health checks, and Docker secrets.

Key points:
- Use a specific, pinned image tag for `webdis`, for example `yourorg/webdis:1.0.0`.
- Configure `redis` with a persistent named volume for data durability.
- Use Docker secrets (or a secrets manager) to store sensitive information like Redis passwords.
- Use a reverse proxy (Traefik / NGINX) to front Webdis in production and handle TLS to clients.

Usage:

1. Create the secret file `./secrets/redis_password` (or use Docker Secrets in swarm/Kubernetes):

```bash
mkdir -p ./secrets && printf "$(openssl rand -hex 16)" > ./secrets/redis_password
```

2. Start the stack:

```bash
docker compose -f docker-compose.prod.yml up -d
```

3. Monitor logs and health checks:

```bash
docker compose -f docker-compose.prod.yml ps
docker compose -f docker-compose.prod.yml logs -f webdis
```

Notes:

This compose file intentionally uses an upstream `elicore/webdis:latest` image by default; set `image` to your org image or to a local image built via a CI pipeline.
- For high availability and distributed workloads, consider orchestrating with Kubernetes and using a StatefulSet for Redis.
- Always use a pinned tag for production images and verify the image signature (see `docs/docker/content-trust.md`).

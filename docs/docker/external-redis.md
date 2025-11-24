# Running Webdis in front of an external Redis

This file documents `docker-compose.external-redis.yml` and how to connect the Webdis container to an external Redis service (managed Redis, or a DB hosted elsewhere).

Example usage:

```bash
export REDIS_HOST=redis.example.com
export REDIS_PORT=6379
export REDIS_AUTH=yourpassword
docker compose -f docker-compose.external-redis.yml up -d
```

Notes:

- The external Redis `REDIS_HOST` may be a DNS name or an IP. Ensure network connectivity from the host running Docker to the Redis endpoint.
- If Redis requires TLS, enable the TLS configuration in `webdis.json` and provide the CA certificate to the container (via volume or secrets).
- If your Webdis image is the local Rust-built image (from this repo's Dockerfile), you may need to pass in `REDIS_AUTH` using environment variables or a secrets file.

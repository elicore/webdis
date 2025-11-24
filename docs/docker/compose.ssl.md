# TLS / SSL Docker Compose

Use `docker-compose.ssl.yml` when you need TLS between Redis and Webdis (TLS for the Redis port and mutual TLS verification).

Generating certs:

```bash
./scripts/generate-certs.sh --outdir ./certs --cn redis
```

This will create `ca.crt`, `redis.crt`, `redis.key`, `client.crt`, and `client.key` in `./certs`.

Redis conf:

Use `redis.conf` mounted to the Redis container and ensure TLS is enabled (example `redis.conf` snippets):

```
tls-port 6380
port 0
tls-cert-file /certs/redis.crt
tls-key-file /certs/redis.key
tls-ca-cert-file /certs/ca.crt
tls-auth-clients no
```

Webdis config:

Ensure Webdis is configured to connect to Redis via TLS and knows the CA to trust. You can either set environment variables or set the TLS options in `webdis.json`.

Start the stack:

```bash
docker compose -f docker-compose.ssl.yml up --build
```

Notes:

- For production, do not mount key files into containers via bind mounts. Use Docker Secrets or a secrets manager instead.
- Use a dedicated SAN with the service DNS name `redis` and your IPs when creating the certs for Redis.

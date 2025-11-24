# Serving an RDB snapshot with Docker

Use `docker-compose.rdb.yml` to run Redis with a `dump.rdb` that you provide locally. This is useful to serve a static snapshot without needing to re-sync from a persistent dataset.

1. Prepare `dump.rdb` and copy it into the `redis-data` volume (host path):

```bash
./scripts/import-rdb.sh /path/to/dump.rdb
```

2. The import script copies the RDB into `./redis-data/dump.rdb` and starts the compose stack.

Notes:

- If you're using an RDB snapshot that contains sensitive production data, ensure appropriate permissions on the host volume and avoid exposing Redis to untrusted networks.
- The `import-rdb.sh` script is idempotent and safe for local usage but not a robust ingestion tool for database migrations.
- To export a snapshot from a running Redis instance: run `BGSAVE` and copy out `dump.rdb` from the Redis container (or use `redis-cli --rdb` remote option if available).

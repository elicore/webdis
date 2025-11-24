# Migration map from legacy docs

This file documents the mapping from earlier docs in `docs/` to the new `docs/docker/` structure.

| Old file | New file | Notes |
| --- | --- | --- |
| `docs/webdis-redis-docker-compose.md` | `docs/docker/compose.dev.md` and `docs/docker/compose.prod.md` | Dev vs prod compose examples split for clarity.
| `docs/webdis-redis-docker-compose-ssl.md` | `docs/docker/compose.ssl.md` | Moved TLS-specific content into a reusable compose + cert generation script.
| `docs/webdis-docker-external-redis.md` | `docs/docker/external-redis.md` | Minor rework to clarify differences between local repo-built images and the upstream images that bundle redis-server.
| `docs/webdis-docker-serve-rdb-file.md` | `docs/docker/serve-rdb.md` | References `docker-compose.rdb.yml` and `scripts/import-rdb.sh`.
| `docs/webdis-docker-content-trust.md` | `docs/docker/content-trust.md` | Updated to recommend Cosign and GitHub Actions examples alongside DCT.

These changes aim to make the Docker documentation more modular and up-to-date. If you depended on the old docs verbatim, they are still present under `/docs` for reference.

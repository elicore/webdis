# Docker & Deployments for Webdis

This folder contains examples and helper scripts to run Webdis with Docker and Docker Compose for development and small-scale production usage.

Files in this folder:

- `docker-compose.dev.yml` — build-from-source dev composition (local build, Redis sidecar).
- `docker-compose.prod.yml` — production-like example with named volumes and secrets.
- `docker-compose.external-redis.yml` — run only a Webdis service pointing to an external Redis instance.
- `docker-compose.ssl.yml` — example that configures TLS between Redis and Webdis.
- `docker-compose.rdb.yml` — example that mounts a Redis RDB snapshot (dump.rdb) in a named volume and starts the stack.
- scripts/ — helper scripts for cert generation, image validation, starting webdis, and importing RDB files.

Read the `compose.*.md` files for more detailed instructions and usage examples.

Notes:
- All examples in this folder reference `elicore/webdis` for production images and local builds (`webdis:dev`) for development. We avoid upstream images that embed Redis alongside Webdis (historical upstream builds that bundled Redis) in new examples.
- See `docs/docker/ci-github-actions-sample.yml` for a sample GitHub Actions workflow that builds, pushes, and signs the `elicore/webdis` image with `cosign`.

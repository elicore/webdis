# Development Docker Compose

This file documents `docker-compose.dev.yml`, a useful setup for local development that will:

- Build `webdis` from the local repo with `docker compose -f docker-compose.dev.yml up --build`.
- Run Redis in a sidecar container and mount config/JSON files into the container.
- Keep logs local and expose the Webdis port on loopback (127.0.0.1) by default.

Usage:

1. Ensure Docker and Docker Compose plugin are installed (use `docker compose` CLI).
2. Build and start the stack:

```bash
docker compose -f docker-compose.dev.yml up --build
```

3. Visit Webdis at `http://127.0.0.1:7379` and confirm using `curl http://127.0.0.1:7379/PING`.

Notes:

- This example builds the `webdis` image locally using the repository's Dockerfile. If you want to use a prebuilt image, modify the `webdis` service to use an org image (e.g. `elicore/webdis:latest`) or a local image tag such as `webdis:dev`.
- For new config options, edit `webdis.json` and restart the compose stack.
- You may want to run `docker compose -f docker-compose.dev.yml down -v` to remove volumes after testing.

# AGENTS.md

This file defines practical workflows and command shortcuts for contributors and coding agents working in `/Users/eli.cohen/dev/my/webdis`.

## Workflows

### 1) Local development loop
1. Build and check compile health.
2. Run fast tests first.
3. Run integration tests when local Redis and ephemeral ports are available.

Commands:
```bash
cargo build --release
cargo test --test config_test
cargo test --test integration_test
```

### 2) Run Webdis locally
Use this when iterating on server behavior with a local config.

```bash
cargo run --release -- webdis.json
```

Optional: scaffold a default config file (non-overwriting).
```bash
cargo run -- --write-default-config --config ./webdis.generated.json
```

### 3) Docker-based smoke workflow
Use this when validating runtime behavior in the compose stack.

```bash
./scripts/compose-smoke.sh
```

Manual alternative:
```bash
docker compose -f docker-compose.dev.yml up --build -d
curl -sS http://127.0.0.1:7379/GET/health
docker compose -f docker-compose.dev.yml down -v
```

### 4) Performance and full test pass
Run before larger merges or release prep.

```bash
make test
make perftest
```

## Command Reference

### Build and cleanup
```bash
make build
make clean
```

### Test targets
```bash
make test
make test_all
cargo test --test config_test
cargo test --test integration_test
```

### Helpful scripts
```bash
./scripts/start-webdis.sh --mode dev
./scripts/start-webdis.sh --mode run --tag webdis:dev --config webdis.json
./scripts/generate-certs.sh
./scripts/import-rdb.sh --rdb /absolute/path/to/dump.rdb
./scripts/validate-image.sh --image elicore/webdis:latest --method cosign
```

## Contribution Notes

- Prefer fast feedback: `config_test` before `integration_test`.
- Keep `webdis.schema.json` and sample configs aligned when introducing config keys.
- Avoid committing local runtime artifacts such as `webdis.log`.

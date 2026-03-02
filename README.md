# redis-web

## Intent

`redis-web` is an HTTP/WebSocket gateway for Redis, evolved from the historical `webdis` implementation.

Migration note: `webdis` naming is still supported during a compatibility transition window (`webdis` binary alias and legacy config filenames), but canonical naming is now `redis-web`.

Full documentation lives in the Starlight docs site under `docs/`.

## Run

```bash
cargo run -p redis-web --bin redis-web -- redis-web.json
```

Compatibility alias:

```bash
cargo run -p redis-web --bin webdis -- webdis.json
```

## Example

```bash
curl http://127.0.0.1:7379/SET/hello/world
curl http://127.0.0.1:7379/GET/hello
```

## Development

```bash
cargo build --workspace
make test
make test_integration
```

For deep references (CLI, config schema, compatibility guarantees, embedding, Docker, maintainers), use the docs site content in `docs/src/content/docs`.

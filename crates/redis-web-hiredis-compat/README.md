# redis-web-hiredis-compat

`redis-web-hiredis-compat` provides a C ABI compatibility surface for clients
that expect `hiredis` symbols.

Current status:
- Staged artifacts provide full upstream hiredis runtime behavior by building
  upstream hiredis C sources (core + async) in the harness pipeline.
- Exported symbol parity and public header API name parity are validated against
  pinned upstream hiredis in `make compat_redispy_audit`.
- redis-py/hiredis-py runtime behavior is validated in
  `make compat_redispy_test` and runtime matrix targets.

Headers:
- `include/hiredis/hiredis.h`
- `include/hiredis/read.h`
- `include/hiredis/alloc.h`
- `include/hiredis/sds.h`
- `include/hiredis/async.h`
- `include/hiredis/net.h`

pkg-config:
- `pkgconfig/hiredis.pc`
- `pkgconfig/redisweb-hiredis.pc`


Integration guide:
- `subprojects/redispy-hiredis-compat/USAGE.md`
  - redis-py integration flow
  - architecture diagrams
  - generic hiredis consumer integration steps

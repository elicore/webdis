# redis-web-hiredis-compat

`redis-web-hiredis-compat` provides a C ABI compatibility surface for clients
that expect `hiredis` symbols.

Current status:
- Shared/static library artifact scaffolding is in place.
- Core sync and async symbols are exported.
- Command execution is currently a compatibility scaffold and returns explicit
  unsupported errors while the transport bridge is implemented.

Headers:
- `include/hiredis/hiredis.h`

pkg-config:
- `pkgconfig/hiredis.pc`
- `pkgconfig/redisweb-hiredis.pc`

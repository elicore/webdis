#!/usr/bin/env python3
import argparse
import sys

import redis
from redis.connection import Connection
from redis.utils import HIREDIS_AVAILABLE


def fail(message: str) -> int:
    print(message, file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=6379)
    parser.add_argument("--db", type=int, default=0)
    args = parser.parse_args()

    if not HIREDIS_AVAILABLE:
        return fail("redis-py reports HIREDIS_AVAILABLE=False")

    try:
        import hiredis
        hiredis_version = getattr(hiredis, "__version__", "unknown")
    except Exception as exc:
        return fail(f"failed to import hiredis: {exc}")

    conn = Connection(host=args.host, port=args.port, db=args.db)
    conn.connect()
    parser_name = conn._parser.__class__.__name__
    conn.disconnect()

    if "hiredis" not in parser_name.lower():
        return fail(f"unexpected parser in use: {parser_name}")

    client = redis.Redis(host=args.host, port=args.port, db=args.db)

    if client.ping() is not True:
        return fail("PING failed")

    if client.set("compat:runtime:key", "value") is not True:
        return fail("SET failed")

    value = client.get("compat:runtime:key")
    if value != b"value":
        return fail(f"GET mismatch: {value!r}")

    pipe = client.pipeline(transaction=False)
    pipe.incr("compat:runtime:ctr")
    pipe.incr("compat:runtime:ctr")
    pipe_results = pipe.execute()
    if len(pipe_results) != 2:
        return fail(f"pipeline result mismatch: {pipe_results!r}")

    print(f"hiredis import ok (version={hiredis_version})")
    print(f"redis parser class: {parser_name}")
    print("runtime operations: ping/set/get/pipeline ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

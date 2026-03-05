#!/usr/bin/env python3
import argparse
import sys

import redis
from redis.connection import Connection
from redis.utils import HIREDIS_AVAILABLE


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=6379)
    parser.add_argument("--db", type=int, default=0)
    args = parser.parse_args()

    if not HIREDIS_AVAILABLE:
        print("redis-py reports HIREDIS_AVAILABLE=False", file=sys.stderr)
        return 1

    try:
        import hiredis

        hiredis_version = getattr(hiredis, "__version__", "unknown")
    except Exception as exc:
        print(f"failed to import hiredis: {exc}", file=sys.stderr)
        return 1

    conn = Connection(host=args.host, port=args.port, db=args.db)
    conn.connect()
    parser_name = conn._parser.__class__.__name__
    conn.disconnect()

    if "hiredis" not in parser_name.lower():
        print(f"unexpected parser in use: {parser_name}", file=sys.stderr)
        return 1

    client = redis.Redis(host=args.host, port=args.port, db=args.db)
    try:
        pong = client.ping()
    except Exception as exc:
        print(f"redis ping failed: {exc}", file=sys.stderr)
        return 1

    print(f"hiredis import ok (version={hiredis_version})")
    print(f"redis parser class: {parser_name}")
    print(f"redis ping: {pong}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

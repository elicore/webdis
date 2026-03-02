---
title: Release and Signing
description: Image naming, compatibility tags, and signing behavior.
---

Canonical image namespace:

- `ghcr.io/elicore/redis-web`

Transition compatibility tags are also published under:

- `ghcr.io/elicore/webdis`

Build and push workflow signs images when cosign secrets are configured.

Verification example:

```bash
./scripts/validate-image.sh --image ghcr.io/elicore/redis-web:latest --method cosign
```

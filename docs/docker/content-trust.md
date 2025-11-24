# Image verification and content trust

This document provides recommended ways to verify container images for Webdis before running them in production.

Options:

1) Cosign (recommended) — verify digital signatures from the image publisher with `cosign verify`.

```bash
cosign verify elicore/webdis:latest
```

2) Docker Content Trust — older approach using Notary (DCT). Use `DOCKER_CONTENT_TRUST=1 docker pull ...` and `docker trust inspect` to inspect signatures.

3) Registry digest comparison — compare the manifest or digest from a registry with your expected digest.

Example GitHub Actions snippet (Cosign):

```yaml
name: Verify Image
on: [pull_request]
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - name: Verify image
        run: |
          cosign verify --key ${{ secrets.COSIGN_PUB_KEY }} elicore/webdis:latest
```

Notes:

For a complete CI example that builds, pushes, and signs the `elicore/webdis` image, see `docs/docker/ci-github-actions-sample.yml`.

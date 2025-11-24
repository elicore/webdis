#!/usr/bin/env bash
set -euo pipefail

# Validate docker image integrity and signatures using cosign or Docker Content Trust as fallback
# Usage: ./scripts/validate-image.sh --image elicore/webdis:latest --method cosign

IMAGE=${IMAGE:-elicore/webdis:latest}
METHOD=${METHOD:-cosign}

while [[ "$#" -gt 0 ]]; do
  case $1 in
    --image) IMAGE=$2; shift 2;;
    --method) METHOD=$2; shift 2;;
    -h|--help) echo "usage: $0 --image org/image:tag [--method cosign|dct]"; exit 0;;
    *) echo "Unknown arg $1"; exit 1;;
  esac
done

if [[ -z "$IMAGE" ]]; then
  echo "--image is required"; exit 1
fi

if [[ "$METHOD" == "cosign" ]]; then
  if ! command -v cosign >/dev/null 2>&1; then
    echo "cosign not installed; try 'go install github.com/sigstore/cosign/cmd/cosign@latest' or use --method dct"; exit 1
  fi
  echo "Verifying $IMAGE with cosign..."
  cosign verify "$IMAGE" || { echo "cosign verify failed"; exit 1; }
  echo "Image verified via cosign"
elif [[ "$METHOD" == "dct" ]]; then
  if ! docker trust inspect --pretty "$IMAGE" >/dev/null 2>&1; then
    echo "docker trust inspect failed; ensure image is signed or use cosign"; exit 1
  fi
  echo "Image has Docker Content Trust metadata (user-visible verification)"
else
  echo "Unknown method $METHOD"; exit 1
fi

#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-butt-linux-test}"
PLATFORM="${PLATFORM:-linux/amd64}"
DOCKERFILE="${DOCKERFILE:-Dockerfile.linux-test}"
CARGO_REGISTRY_VOLUME="${CARGO_REGISTRY_VOLUME:-butt-cargo-registry-cache}"
CARGO_GIT_VOLUME="${CARGO_GIT_VOLUME:-butt-cargo-git-cache}"
CARGO_TARGET_VOLUME="${CARGO_TARGET_VOLUME:-butt-cargo-target-cache}"

echo "[docker-test] Building image '${IMAGE_NAME}' using ${DOCKERFILE} (${PLATFORM})..."
docker build --platform "${PLATFORM}" -f "${DOCKERFILE}" -t "${IMAGE_NAME}" .

echo "[docker-test] Running tests in container..."
docker run --rm \
  --platform "${PLATFORM}" \
  -v "${CARGO_REGISTRY_VOLUME}:/usr/local/cargo/registry" \
  -v "${CARGO_GIT_VOLUME}:/usr/local/cargo/git" \
  -v "${CARGO_TARGET_VOLUME}:/app/target" \
  "${IMAGE_NAME}"

echo "[docker-test] Done."

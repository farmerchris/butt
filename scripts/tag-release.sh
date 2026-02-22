#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/tag-release.sh <version> [--push]

Examples:
  ./scripts/tag-release.sh 0.1.3
  ./scripts/tag-release.sh v0.1.3 --push

This script tags origin/main (not your local branch) so it works with PR-protected main.
EOF
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
  usage
  exit 2
fi

VERSION_RAW="$1"
PUSH="${2:-}"

if [[ -n "${PUSH}" && "${PUSH}" != "--push" ]]; then
  echo "Invalid argument: ${PUSH}" >&2
  usage
  exit 2
fi

VERSION="${VERSION_RAW#v}"
if [[ ! "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid version '${VERSION_RAW}'. Use semver like 0.1.3 or v0.1.3." >&2
  exit 2
fi

TAG="v${VERSION}"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Working tree is not clean. Commit/stash changes first." >&2
  exit 1
fi

git fetch --tags origin main
MAIN_REF="$(git rev-parse --verify FETCH_HEAD)"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "Local tag ${TAG} already exists." >&2
  exit 1
fi
if git ls-remote --tags origin | grep -q "refs/tags/${TAG}$"; then
  echo "Remote tag ${TAG} already exists." >&2
  exit 1
fi

MAIN_VERSION="$(
  git show "${MAIN_REF}:Cargo.toml" | sed -n 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*\"\\(.*\\)\"[[:space:]]*$/\\1/p' | head -n1
)"
if [[ -z "${MAIN_VERSION}" ]]; then
  echo "Could not parse version from ${MAIN_REF}:Cargo.toml" >&2
  exit 1
fi
if [[ "${MAIN_VERSION}" != "${VERSION}" ]]; then
  echo "Version mismatch: ${MAIN_REF} Cargo.toml has ${MAIN_VERSION}, requested ${VERSION}" >&2
  exit 1
fi

echo "[release-tag] Creating local tag ${TAG} at ${MAIN_REF}"
git tag -a "${TAG}" -m "${TAG}" "${MAIN_REF}"

if [[ "${PUSH}" == "--push" ]]; then
  echo "[release-tag] Pushing tag ${TAG}"
  git push origin "${TAG}"
else
  echo "[release-tag] Tag created locally only."
  echo "To publish:"
  echo "  git push origin ${TAG}"
fi

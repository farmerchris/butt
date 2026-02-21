#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/release.sh <version> [--push]

Examples:
  ./scripts/release.sh 0.1.3
  ./scripts/release.sh v0.1.3 --push

This script:
  1) updates Cargo.toml version
  2) runs prechecks
  3) commits the version bump
  4) creates annotated tag v<version>
  5) optionally pushes commit + tag with --push
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

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "Tag ${TAG} already exists." >&2
  exit 1
fi

echo "[release] Setting Cargo.toml version -> ${VERSION}"
sed -i.bak -E "s/^version = \".*\"$/version = \"${VERSION}\"/" Cargo.toml
rm -f Cargo.toml.bak

echo "[release] Running prechecks"
env -u RUSTC_WRAPPER ./scripts/precheck.sh

echo "[release] Creating commit"
git add Cargo.toml Cargo.lock
if git diff --cached --quiet; then
  echo "No version changes staged. Did Cargo.toml already have ${VERSION}?" >&2
  exit 1
fi
git commit -m "release: ${TAG}"

echo "[release] Creating tag ${TAG}"
git tag -a "${TAG}" -m "${TAG}"

if [[ "${PUSH}" == "--push" ]]; then
  echo "[release] Pushing commit and tag"
  git push origin HEAD
  git push origin "${TAG}"
else
  echo "[release] Dry run complete (local commit + tag created, not pushed)."
  echo "To publish:"
  echo "  git push origin HEAD"
  echo "  git push origin ${TAG}"
fi

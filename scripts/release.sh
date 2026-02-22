#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/release.sh <version> [--push-branch]

Examples:
  ./scripts/release.sh 0.1.3
  ./scripts/release.sh v0.1.3 --push-branch

This script:
  1) updates Cargo.toml version
  2) runs prechecks
  3) commits the version bump
  4) optionally pushes the current branch with --push-branch

After PR merge, create/push the release tag from origin/main with:
  ./scripts/tag-release.sh <version>
EOF
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
  usage
  exit 2
fi

VERSION_RAW="$1"
PUSH_BRANCH="${2:-}"

if [[ -n "${PUSH_BRANCH}" && "${PUSH_BRANCH}" != "--push-branch" ]]; then
  echo "Invalid argument: ${PUSH_BRANCH}" >&2
  usage
  exit 2
fi

VERSION="${VERSION_RAW#v}"
if [[ ! "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid version '${VERSION_RAW}'. Use semver like 0.1.3 or v0.1.3." >&2
  exit 2
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Working tree is not clean. Commit/stash changes first." >&2
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
git commit -m "release: v${VERSION}"

if [[ "${PUSH_BRANCH}" == "--push-branch" ]]; then
  echo "[release] Pushing current branch"
  git push origin HEAD
else
  echo "[release] Done (local commit created, branch not pushed)."
  echo "To open PR:"
  echo "  git push origin HEAD"
fi

echo "[release] After PR merge, tag from main with:"
echo "  ./scripts/tag-release.sh ${VERSION}"

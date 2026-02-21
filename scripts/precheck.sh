#!/usr/bin/env bash
set -euo pipefail

echo "[precheck] cargo fmt --all"
cargo fmt --all

echo "[precheck] cargo check --all-targets --all-features"
cargo check --all-targets --all-features

echo "[precheck] cargo clippy --all-targets --all-features -- -D warnings"
cargo clippy --all-targets --all-features -- -D warnings

echo "[precheck] cargo build"
cargo build
cargo build -r

echo "[precheck] cargo test"
cargo test --all-targets --all-features

echo "[precheck] All checks passed."

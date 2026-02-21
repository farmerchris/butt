# Contributing

## Setup

- Install stable Rust.
- Run `cargo build`.

## Before opening a PR

- Run all required local checks:
  - `./scripts/precheck.sh`
- Run Linux tests via helper script (if you don't develop on Linux):
  - `./scripts/run-linux-docker-test.sh`

## PR expectations

- Keep changes focused.
- Update docs for behavior or flag changes.
- Add/adjust tests for behavior changes.

## One-step release

- Create version bump + commit + tag in one command:
  - `./scripts/release.sh 0.1.3`
- Push immediately:
  - `./scripts/release.sh 0.1.3 --push`

This avoids mismatched `Cargo.toml` version and git tag.

## Coverage report

- Generate HTML coverage report:
  - `cargo llvm-cov --all-targets --html`
- Open report:
  - `open target/llvm-cov/html/index.html` (macOS)

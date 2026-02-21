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

## Coverage report

- Generate HTML coverage report:
  - `cargo llvm-cov --all-targets --html`
- Open report:
  - `open target/llvm-cov/html/index.html` (macOS)

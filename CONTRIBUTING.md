# Contributing

## Setup

- Install stable Rust.
- Run `cargo build`.

## Before opening a PR

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo check --all-targets --all-features`
- `cargo test --all-targets --all-features`

## PR expectations

- Keep changes focused.
- Update docs for behavior or flag changes.
- Add/adjust tests for behavior changes.

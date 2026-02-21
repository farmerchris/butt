# AGENTS

- Binary name: `butt`
- Main behavior:
  - Throttle normal output with `--line-seconds`
  - Print regex matches immediately
  - Print `[no output for N seconds]` on idle
- Run tests with:
  - `env -u RUSTC_WRAPPER cargo test`
- Required before finishing any change:
  - Double check docs (`README.md`, `AGENTS.md`) match current behavior/flags
  - Run `cargo fmt`
  - Run `env -u RUSTC_WRAPPER cargo clippy --all-targets --all-features -- -D warnings` and fix all warnings
  - Run `env -u RUSTC_WRAPPER cargo check --all-targets --all-features` and fix all warnings/errors
  - Run `env -u RUSTC_WRAPPER cargo test` and fix failing/flaky tests
  - Make sure README.md Usage section matches `--help` output   

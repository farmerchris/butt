# butt

A throttled tail/stdin tool.

- Reads from a file (`butt /path/to/log`) or stdin (`cmd | butt`)
- Prints at most one normal line every `--line-seconds` (default `5`)
- Prints matching `--regex` lines immediately (with optional color)
- Prints `[no output for N seconds]` every `--idle-seconds` when idle (disabled unless provided)

## Usage

```bash
cargo run -- --line-seconds 5 --idle-seconds 10
cargo run -- /path/to/log --regex ERROR --color yellow
```

## Test

```bash
env -u RUSTC_WRAPPER cargo test
```

# butt

A throttled tail/stdin tool. Make sure your processes are not hung, and show imporant messages immediately.

- Reads from a file (`butt /path/to/log`) or stdin (`cmd | butt`)
- Prints at most one normal line every `--line-seconds` (default `5`)
- Prints matching `--regex` lines immediately (with optional color)
- Prints `[no output for N seconds]` every `--idle-seconds` when idle (disabled unless provided)
- `--line-seconds` and `--idle-seconds` (if provided) must be `>= 1`
- Optional safety flags:
  - `--no-follow-symlinks`
  - `--allowed-root /path/to/root`
- Backpressure/limits:
  - `--max-buffer-bytes` (default `1048576`)
  - `--max-line-bytes` (default `65536`)

## Usage

```bash
cargo run -- --line-seconds 5 --idle-seconds 10
cargo run -- /path/to/log --regex ERROR --color yellow
```

## Dev workflow

See `CONTRIBUTING.md` for precheck, Linux Docker test, and coverage commands.

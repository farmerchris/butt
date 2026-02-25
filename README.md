# butt

A throttled tail/stdin tool. Make sure your processes are not hung, and show imporant messages immediately.

- Reads from a file (`butt /path/to/log`) or stdin (`cmd | butt`)
- Prints at most one normal line every `--line-seconds` (default `5`)
- Prints matching `--regex` lines immediately (with optional color)
- Optional case-insensitive regex matching with `-I` / `--case-insensitive`
- Prints `[no output for N seconds]` every `--idle-seconds` when idle (disabled unless provided)
- `--line-seconds` and `--idle-seconds` (if provided) must be `>= 1`
- Optional safety flags:
  - `--no-follow-symlinks`
  - `--allowed-root /path/to/root`
- Backpressure/limits:
  - `--max-buffer-bytes` (default `1048576`)
  - `--max-line-bytes` (default `65536`)

## Installation

```bash
cargo install --path .
```

## Usage

```
Throttle stream output and follow files

Usage: butt [OPTIONS] [PATH]

Arguments:
  [PATH]  File to follow. If omitted, reads from stdin

Options:
  -n, --line-seconds <LINE_SECONDS>
          Print at most one input line per N seconds [default: 5]
  -i, --idle-seconds <IDLE_SECONDS>
          No-output notice period in seconds
  -r, --regex <REGEX>
          Regex pattern to highlight
  -I, --case-insensitive
          Make --regex matching case-insensitive
  -c, --color <COLOR>
          Highlight color for regex matches [default: yellow] [possible values: red, green, yellow, blue, magenta, cyan]
      --poll-millis <POLL_MILLIS>
          Poll interval in milliseconds [default: 200]
      --max-buffer-bytes <MAX_BUFFER_BYTES>
          Maximum pending in-memory bytes while assembling lines [default: 1048576]
      --max-line-bytes <MAX_LINE_BYTES>
          Maximum bytes per line before truncation/drop [default: 65536]
      --no-follow-symlinks
          Refuse following files when PATH is a symlink
      --allowed-root <ALLOWED_ROOT>
          Restrict followed file to this root directory (after canonicalization)
  -h, --help
          Print help
  -V, --version
          Print version
```

## Examples

```bash
/path/to/process | butt --line-seconds 10 --idle-seconds 30
butt /path/to/log --regex ERROR --color yellow
butt /path/to/log --regex error --case-insensitive
```

## Dev workflow

See `CONTRIBUTING.md` for precheck, Linux Docker test, and coverage commands.

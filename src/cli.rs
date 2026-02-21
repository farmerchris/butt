use clap::{Parser, ValueEnum, value_parser};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum HighlightColor {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
}

impl HighlightColor {
    pub(crate) fn ansi_code(&self) -> &'static str {
        match self {
            Self::Red => "31",
            Self::Green => "32",
            Self::Yellow => "33",
            Self::Blue => "34",
            Self::Magenta => "35",
            Self::Cyan => "36",
        }
    }

    pub(crate) fn paint(&self, input: &str) -> String {
        format!("\x1b[{}m{}\x1b[0m", self.ansi_code(), input)
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "butt",
    version,
    about = "Throttle stream output and follow files"
)]
pub(crate) struct Args {
    /// File to follow. If omitted, reads from stdin.
    pub(crate) path: Option<PathBuf>,

    /// Print at most one input line per N seconds.
    #[arg(
        short = 'n',
        long = "line-seconds",
        default_value_t = 5,
        value_parser = value_parser!(u64).range(1..)
    )]
    pub(crate) line_seconds: u64,

    /// No-output notice period in seconds.
    #[arg(
        short = 'i',
        long = "idle-seconds",
        value_parser = value_parser!(u64).range(1..)
    )]
    pub(crate) idle_seconds: Option<u64>,

    /// Regex pattern to highlight.
    #[arg(short, long)]
    pub(crate) regex: Option<String>,

    /// Highlight color for regex matches.
    #[arg(short, long, value_enum, default_value = "yellow")]
    pub(crate) color: HighlightColor,

    /// Poll interval in milliseconds.
    #[arg(long = "poll-millis", default_value_t = 200)]
    pub(crate) poll_millis: u64,

    /// Maximum pending in-memory bytes while assembling lines.
    #[arg(
        long = "max-buffer-bytes",
        default_value_t = 1_048_576,
        value_parser = parse_positive_usize
    )]
    pub(crate) max_buffer_bytes: usize,

    /// Maximum bytes per line before truncation/drop.
    #[arg(
        long = "max-line-bytes",
        default_value_t = 65_536,
        value_parser = parse_positive_usize
    )]
    pub(crate) max_line_bytes: usize,

    /// Refuse following files when PATH is a symlink.
    #[arg(long = "no-follow-symlinks", default_value_t = false)]
    pub(crate) no_follow_symlinks: bool,

    /// Restrict followed file to this root directory (after canonicalization).
    #[arg(long = "allowed-root")]
    pub(crate) allowed_root: Option<PathBuf>,
}

pub(crate) fn parse_positive_usize(input: &str) -> Result<usize, String> {
    let parsed: usize = input
        .parse()
        .map_err(|_| format!("invalid integer '{}'", input))?;
    if parsed == 0 {
        return Err("value must be >= 1".to_string());
    }
    Ok(parsed)
}

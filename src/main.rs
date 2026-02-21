mod cli;
mod follow;
mod limits;
mod output;

use clap::Parser;
use regex::Regex;
use std::fs;

use crate::cli::Args;
use crate::follow::{follow_file, follow_stdin};
use crate::output::should_use_color;

#[cfg(test)]
use crate::output::{decorate_line, highlight_matches};

fn main() {
    let args = Args::parse();
    let colors_enabled = should_use_color();
    let allowed_root = match &args.allowed_root {
        Some(root) => match fs::canonicalize(root) {
            Ok(canonical) => Some(canonical),
            Err(err) => {
                eprintln!("[butt] invalid --allowed-root '{}': {err}", root.display());
                std::process::exit(2);
            }
        },
        None => None,
    };

    let regex = match &args.regex {
        Some(pattern) => match Regex::new(pattern) {
            Ok(re) => Some(re),
            Err(err) => {
                eprintln!("[butt] invalid regex '{pattern}': {err}");
                std::process::exit(2);
            }
        },
        None => None,
    };

    let result = match &args.path {
        Some(path) => follow_file(
            &args,
            path,
            regex.as_ref(),
            colors_enabled,
            allowed_root.as_deref(),
        ),
        None => follow_stdin(&args, regex.as_ref(), colors_enabled),
    };

    if let Err(err) = result {
        eprintln!("[butt] error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::HighlightColor;
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn highlights_all_matches() {
        let re = Regex::new("ERR").expect("regex should compile");
        let out = highlight_matches("x ERR y ERR z", &re, &HighlightColor::Red);
        assert!(out.contains("\x1b[31mERR\x1b[0m"));
        assert_eq!(out.matches("\x1b[31mERR\x1b[0m").count(), 2);
    }

    #[test]
    fn decorates_plain_when_no_regex() {
        let out = decorate_line("plain text", None, &HighlightColor::Yellow, true);
        assert_eq!(out, "plain text");
    }

    #[test]
    fn parses_minimal_args_with_optional_path() {
        let with_path = Args::parse_from(["butt", "./sample.log"]);
        assert_eq!(with_path.path, Some(PathBuf::from("./sample.log")));
        assert_eq!(with_path.line_seconds, 5);
        assert_eq!(with_path.idle_seconds, None);
        assert_eq!(with_path.max_buffer_bytes, 1_048_576);
        assert_eq!(with_path.max_line_bytes, 65_536);

        let without_path = Args::parse_from(["butt"]);
        assert_eq!(without_path.path, None);
    }

    #[test]
    fn rejects_zero_line_seconds() {
        let parsed = Args::try_parse_from(["butt", "--line-seconds", "0"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_zero_idle_seconds() {
        let parsed = Args::try_parse_from(["butt", "--idle-seconds", "0"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_zero_max_buffer_bytes() {
        let parsed = Args::try_parse_from(["butt", "--max-buffer-bytes", "0"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_zero_max_line_bytes() {
        let parsed = Args::try_parse_from(["butt", "--max-line-bytes", "0"]);
        assert!(parsed.is_err());
    }
}

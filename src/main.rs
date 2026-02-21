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

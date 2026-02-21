use clap::{Parser, ValueEnum};
use regex::Regex;
use std::fs::{self, File};
use std::io::{self, BufRead, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Clone, ValueEnum)]
enum HighlightColor {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
}

impl HighlightColor {
    fn ansi_code(&self) -> &'static str {
        match self {
            Self::Red => "31",
            Self::Green => "32",
            Self::Yellow => "33",
            Self::Blue => "34",
            Self::Magenta => "35",
            Self::Cyan => "36",
        }
    }

    fn paint(&self, input: &str) -> String {
        format!("\x1b[{}m{}\x1b[0m", self.ansi_code(), input)
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "butt",
    version,
    about = "Throttle stream output and follow files"
)]
struct Args {
    /// File to follow. If omitted, reads from stdin.
    path: Option<PathBuf>,

    /// Print at most one input line per N seconds.
    #[arg(short = 'n', long = "line-seconds", default_value_t = 5)]
    line_seconds: u64,

    /// No-output notice period in seconds.
    #[arg(short = 'i', long = "idle-seconds")]
    idle_seconds: Option<u64>,

    /// Regex pattern to highlight.
    #[arg(short, long)]
    regex: Option<String>,

    /// Highlight color for regex matches.
    #[arg(short, long, value_enum, default_value = "yellow")]
    color: HighlightColor,

    /// Poll interval in milliseconds.
    #[arg(long = "poll-millis", default_value_t = 200)]
    poll_millis: u64,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    FileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    }
}

struct EmitState {
    next_line_emit: Instant,
    next_idle_emit: Option<Instant>,
    last_output: Instant,
    latest_line: Option<String>,
}

impl EmitState {
    fn new(args: &Args) -> Self {
        let now = Instant::now();
        Self {
            next_line_emit: now + Duration::from_secs(args.line_seconds),
            next_idle_emit: args
                .idle_seconds
                .map(|idle| now + Duration::from_secs(idle)),
            last_output: now,
            latest_line: None,
        }
    }

    fn mark_output_emitted(&mut self, now: Instant, args: &Args) {
        self.last_output = now;
        self.next_idle_emit = args
            .idle_seconds
            .map(|idle| now + Duration::from_secs(idle));
    }

    fn observe_input(
        &mut self,
        line: String,
        args: &Args,
        regex: Option<&Regex>,
        color: &HighlightColor,
        colors_enabled: bool,
    ) {
        let now = Instant::now();

        if let Some(rgx) = regex
            && rgx.is_match(&line)
        {
            println!("{}", decorate_line(&line, regex, color, colors_enabled));
            let _ = io::stdout().flush();
            self.mark_output_emitted(now, args);
            self.latest_line = None;
            self.next_line_emit = now + Duration::from_secs(args.line_seconds);
            return;
        }

        self.latest_line = Some(line);
    }

    fn maybe_emit(&mut self, args: &Args, regex: Option<&Regex>, colors_enabled: bool) {
        let now = Instant::now();
        let line_interval = Duration::from_secs(args.line_seconds);
        if now >= self.next_line_emit {
            if let Some(line) = self.latest_line.take() {
                println!(
                    "{}",
                    decorate_line(&line, regex, &args.color, colors_enabled)
                );
                let _ = io::stdout().flush();
                self.mark_output_emitted(now, args);
            }
            self.next_line_emit = now + line_interval;
        }

        if let Some(idle_seconds) = args.idle_seconds {
            let idle_interval = Duration::from_secs(idle_seconds);
            if now.duration_since(self.last_output) >= idle_interval
                && self.next_idle_emit.is_some_and(|next| now >= next)
            {
                println!("[no output for {} seconds]", idle_seconds);
                let _ = io::stdout().flush();
                self.next_idle_emit = Some(now + idle_interval);
            }
        }
    }
}

fn open_at_end(path: &Path) -> io::Result<File> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::End(0))?;
    Ok(file)
}

fn open_from_start(path: &Path) -> io::Result<File> {
    File::open(path)
}

fn decorate_line(
    line: &str,
    regex: Option<&Regex>,
    color: &HighlightColor,
    colors_enabled: bool,
) -> String {
    if let Some(rgx) = regex {
        if colors_enabled {
            highlight_matches(line, rgx, color)
        } else {
            line.to_string()
        }
    } else {
        line.to_string()
    }
}

fn highlight_matches(line: &str, regex: &Regex, color: &HighlightColor) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last = 0;
    for mat in regex.find_iter(line) {
        out.push_str(&line[last..mat.start()]);
        out.push_str(&color.paint(mat.as_str()));
        last = mat.end();
    }
    out.push_str(&line[last..]);
    out
}

fn collect_complete_lines(pending: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
        let mut line = pending.drain(..=pos).collect::<Vec<u8>>();
        if line.ends_with(b"\n") {
            line.pop();
        }
        if line.ends_with(b"\r") {
            line.pop();
        }
        lines.push(String::from_utf8_lossy(&line).to_string());
    }
    lines
}

fn should_use_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    if let Some(force) = std::env::var_os("CLICOLOR_FORCE")
        && force.to_string_lossy() != "0"
    {
        return true;
    }

    if let Some(clicolor) = std::env::var_os("CLICOLOR")
        && clicolor.to_string_lossy() == "0"
    {
        return false;
    }

    if std::env::var("TERM").is_ok_and(|term| term == "dumb") {
        return false;
    }

    io::stdout().is_terminal()
}

fn follow_file(
    args: &Args,
    path: &Path,
    regex: Option<&Regex>,
    colors_enabled: bool,
) -> io::Result<()> {
    let poll = Duration::from_millis(args.poll_millis);
    let mut emit = EmitState::new(args);

    let mut file = loop {
        match open_at_end(path) {
            Ok(f) => break f,
            Err(err) => {
                eprintln!("[butt] waiting for file '{}' ({err})", path.display());
                thread::sleep(poll);
            }
        }
    };

    #[cfg(unix)]
    let mut opened_id = fs::metadata(path).ok().map(|m| file_identity(&m));

    let mut pending = Vec::new();
    let mut scratch = Vec::new();

    loop {
        emit.maybe_emit(args, regex, colors_enabled);

        match file.read_to_end(&mut scratch) {
            Ok(0) => {}
            Ok(_) => {
                pending.extend_from_slice(&scratch);
                scratch.clear();
                for line in collect_complete_lines(&mut pending) {
                    emit.observe_input(line, args, regex, &args.color, colors_enabled);
                }
            }
            Err(err) => {
                eprintln!("[butt] read error: {err}");
                let _ = io::stderr().flush();
                thread::sleep(poll);
            }
        }

        let pos = file.stream_position()?;
        let len = file.metadata()?.len();
        if len < pos {
            file.seek(SeekFrom::Start(0))?;
            pending.clear();
        }

        match fs::metadata(path) {
            Ok(meta) => {
                #[cfg(unix)]
                {
                    let current_id = file_identity(&meta);
                    if opened_id != Some(current_id) {
                        match open_from_start(path) {
                            Ok(new_file) => {
                                file = new_file;
                                pending.clear();
                                opened_id = Some(current_id);
                                eprintln!(
                                    "[butt] reopened '{}' after rotation/replacement",
                                    path.display()
                                );
                                let _ = io::stderr().flush();
                            }
                            Err(err) => {
                                eprintln!("[butt] reopen failed: {err}");
                                let _ = io::stderr().flush();
                            }
                        }
                    }
                }
            }
            Err(_) => {
                thread::sleep(poll);
                continue;
            }
        }

        thread::sleep(poll);
    }
}

fn follow_stdin(args: &Args, regex: Option<&Regex>, colors_enabled: bool) -> io::Result<()> {
    let poll = Duration::from_millis(args.poll_millis);
    let (tx, rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let stdin = io::stdin();
        let mut locked = stdin.lock();
        loop {
            let mut line = String::new();
            match locked.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut emit = EmitState::new(args);

    loop {
        emit.maybe_emit(args, regex, colors_enabled);

        match rx.recv_timeout(poll) {
            Ok(line) => {
                let line = line.trim_end_matches(['\n', '\r']).to_string();
                emit.observe_input(line, args, regex, &args.color, colors_enabled);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn main() {
    let args = Args::parse();
    let colors_enabled = should_use_color();

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
        Some(path) => follow_file(&args, path, regex.as_ref(), colors_enabled),
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
    use clap::Parser;

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

        let without_path = Args::parse_from(["butt"]);
        assert_eq!(without_path.path, None);
    }
}

use crate::cli::{Args, HighlightColor};
use crate::limits::{append_with_buffer_cap, collect_complete_lines, start_stdin_reader};
use crate::output::decorate_line;
use regex::Regex;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

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

fn validate_follow_target(
    path: &Path,
    no_follow_symlinks: bool,
    allowed_root: Option<&Path>,
) -> io::Result<()> {
    if no_follow_symlinks {
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "symlink targets are not allowed by --no-follow-symlinks",
            ));
        }
    }

    if let Some(root) = allowed_root {
        let canonical_target = fs::canonicalize(path)?;
        if !canonical_target.starts_with(root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "path '{}' is outside allowed root '{}'",
                    canonical_target.display(),
                    root.display()
                ),
            ));
        }
    }

    Ok(())
}

pub(crate) fn follow_file(
    args: &Args,
    path: &Path,
    regex: Option<&Regex>,
    colors_enabled: bool,
    allowed_root: Option<&Path>,
) -> io::Result<()> {
    let poll = Duration::from_millis(args.poll_millis);
    let mut emit = EmitState::new(args);

    let mut file = loop {
        if let Err(err) = validate_follow_target(path, args.no_follow_symlinks, allowed_root) {
            eprintln!("[butt] waiting for file '{}' ({err})", path.display());
            thread::sleep(poll);
            continue;
        }
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

    loop {
        emit.maybe_emit(args, regex, colors_enabled);

        let mut chunk = [0_u8; 8192];
        match file.read(&mut chunk) {
            Ok(0) => {}
            Ok(n) => {
                if append_with_buffer_cap(&mut pending, &chunk[..n], args.max_buffer_bytes) {
                    eprintln!(
                        "[butt] buffer exceeded --max-buffer-bytes={}, dropping buffered data",
                        args.max_buffer_bytes
                    );
                    let _ = io::stderr().flush();
                }

                let (lines, dropped_or_truncated) =
                    collect_complete_lines(&mut pending, args.max_line_bytes);
                if dropped_or_truncated > 0 {
                    eprintln!(
                        "[butt] truncated/dropped {} oversized line fragment(s) (max-line-bytes={})",
                        dropped_or_truncated, args.max_line_bytes
                    );
                    let _ = io::stderr().flush();
                }

                for line in lines {
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
                        if let Err(err) =
                            validate_follow_target(path, args.no_follow_symlinks, allowed_root)
                        {
                            eprintln!("[butt] reopen blocked: {err}");
                            let _ = io::stderr().flush();
                            thread::sleep(poll);
                            continue;
                        }
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

pub(crate) fn follow_stdin(
    args: &Args,
    regex: Option<&Regex>,
    colors_enabled: bool,
) -> io::Result<()> {
    let poll = Duration::from_millis(args.poll_millis);
    let (tx, rx): (SyncSender<String>, Receiver<String>) = mpsc::sync_channel(1024);

    let _reader_handle = start_stdin_reader(tx, args.max_line_bytes, args.max_buffer_bytes);

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[cfg(unix)]
    #[test]
    fn validate_follow_target_rejects_symlink_when_configured() {
        let tmp = tempdir().expect("tempdir");
        let real = tmp.path().join("real.log");
        let link = tmp.path().join("link.log");
        File::create(&real).expect("create real file");
        symlink(&real, &link).expect("create symlink");

        let result = validate_follow_target(&link, true, None);
        assert!(result.is_err());
    }

    #[test]
    fn validate_follow_target_rejects_outside_allowed_root() {
        let root = tempdir().expect("root tempdir");
        let outside = tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("outside.log");
        File::create(&outside_file).expect("create outside file");

        let result = validate_follow_target(&outside_file, false, Some(root.path()));
        assert!(result.is_err());
    }
}

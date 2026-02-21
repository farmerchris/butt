use std::io::{self, BufRead, Write};
use std::sync::mpsc::SyncSender;
use std::thread;

pub(crate) fn collect_complete_lines(
    pending: &mut Vec<u8>,
    max_line_bytes: usize,
) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut dropped_or_truncated = 0;
    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
        let mut line = pending.drain(..=pos).collect::<Vec<u8>>();
        if line.ends_with(b"\n") {
            line.pop();
        }
        if line.ends_with(b"\r") {
            line.pop();
        }
        if line.len() > max_line_bytes {
            line.truncate(max_line_bytes);
            dropped_or_truncated += 1;
        }
        lines.push(String::from_utf8_lossy(&line).to_string());
    }

    if pending.len() > max_line_bytes {
        pending.clear();
        dropped_or_truncated += 1;
    }

    (lines, dropped_or_truncated)
}

pub(crate) fn append_with_buffer_cap(
    pending: &mut Vec<u8>,
    incoming: &[u8],
    max_buffer_bytes: usize,
) -> bool {
    if incoming.len() >= max_buffer_bytes {
        pending.clear();
        pending.extend_from_slice(&incoming[incoming.len() - max_buffer_bytes..]);
        return true;
    }

    let mut dropped = false;
    if pending.len() + incoming.len() > max_buffer_bytes {
        pending.clear();
        dropped = true;
    }

    pending.extend_from_slice(incoming);
    dropped
}

fn truncate_utf8_to_bytes(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }

    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    s.truncate(idx);
}

pub(crate) fn start_stdin_reader(
    tx: SyncSender<String>,
    max_line_bytes: usize,
    max_buffer_bytes: usize,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut locked = stdin.lock();
        loop {
            let mut line = String::new();
            match locked.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if line.len() > max_buffer_bytes {
                        eprintln!(
                            "[butt] stdin chunk exceeded --max-buffer-bytes={}, truncating",
                            max_buffer_bytes
                        );
                        let _ = io::stderr().flush();
                        truncate_utf8_to_bytes(&mut line, max_buffer_bytes);
                    }
                    if line.len() > max_line_bytes {
                        eprintln!(
                            "[butt] line exceeded --max-line-bytes={}, truncating",
                            max_line_bytes
                        );
                        let _ = io::stderr().flush();
                        truncate_utf8_to_bytes(&mut line, max_line_bytes);
                    }
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_complete_lines_truncates_oversized_lines() {
        let mut pending = b"abcdef\n".to_vec();
        let (lines, dropped) = collect_complete_lines(&mut pending, 3);
        assert_eq!(lines, vec!["abc".to_string()]);
        assert_eq!(dropped, 1);
        assert!(pending.is_empty());
    }

    #[test]
    fn collect_complete_lines_drops_oversized_unterminated_fragment() {
        let mut pending = b"abcdef".to_vec();
        let (lines, dropped) = collect_complete_lines(&mut pending, 3);
        assert!(lines.is_empty());
        assert_eq!(dropped, 1);
        assert!(pending.is_empty());
    }

    #[test]
    fn append_with_buffer_cap_keeps_recent_bytes_when_incoming_is_huge() {
        let mut pending = b"old".to_vec();
        let dropped = append_with_buffer_cap(&mut pending, b"abcdef", 4);
        assert!(dropped);
        assert_eq!(pending, b"cdef".to_vec());
    }
}

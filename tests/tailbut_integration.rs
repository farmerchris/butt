use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn spawn_capture_thread<R: Read + Send + 'static>(
    mut reader: R,
) -> (Arc<Mutex<String>>, thread::JoinHandle<()>) {
    let buf = Arc::new(Mutex::new(String::new()));
    let buf_clone = Arc::clone(&buf);
    let handle = thread::spawn(move || {
        let mut chunk = [0_u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&chunk[..n]);
                    buf_clone.lock().expect("lock poisoned").push_str(&text);
                }
                Err(_) => break,
            }
        }
    });
    (buf, handle)
}

fn wait_for_contains(buf: &Arc<Mutex<String>>, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if buf.lock().expect("lock poisoned").contains(needle) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn prints_idle_message_after_configured_interval() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log = tmp.path().join("app.log");
    File::create(&log).expect("create log file");

    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            log.to_str().expect("utf8 path"),
            "--line-seconds",
            "60",
            "--idle-seconds",
            "1",
            "--poll-millis",
            "50",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (_stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    let found = wait_for_contains(
        &stdout_buf,
        "[no output for 1 seconds]",
        Duration::from_secs(4),
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    assert!(found, "idle message did not appear in stdout");
}

#[test]
fn throttles_non_matching_stdin_to_one_line_per_interval() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            "--line-seconds",
            "1",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "20",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (_stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    let mut stdin = child.stdin.take().expect("stdin pipe");
    for i in 0..10 {
        writeln!(stdin, "msg-{i}").expect("write line");
    }
    stdin.flush().expect("flush stdin");

    thread::sleep(Duration::from_millis(1400));

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let out = stdout_buf.lock().expect("lock poisoned").clone();
    let count = out.lines().filter(|line| line.starts_with("msg-")).count();
    assert_eq!(
        count, 1,
        "expected exactly one throttled line, got output: {out}"
    );
}

#[test]
fn regex_matches_print_immediately_and_reset_throttle_window() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            "--line-seconds",
            "2",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "20",
            "--regex",
            "ERR",
            "--color",
            "green",
        ])
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (_stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    let mut stdin = child.stdin.take().expect("stdin pipe");
    writeln!(stdin, "regular message").expect("write non-match");
    stdin.flush().expect("flush non-match");
    thread::sleep(Duration::from_millis(150));
    writeln!(stdin, "ERR first").expect("write first match");
    writeln!(stdin, "ERR second").expect("write second match");
    stdin.flush().expect("flush matches");

    let saw_first = wait_for_contains(
        &stdout_buf,
        "\x1b[32mERR\x1b[0m first",
        Duration::from_secs(2),
    );
    let saw_second = wait_for_contains(
        &stdout_buf,
        "\x1b[32mERR\x1b[0m second",
        Duration::from_secs(2),
    );

    thread::sleep(Duration::from_millis(600));

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let out = stdout_buf.lock().expect("lock poisoned").clone();
    assert!(saw_first, "first regex match was not printed");
    assert!(saw_second, "second regex match was not printed");
    assert!(
        !out.contains("regular message"),
        "non-matching line should remain throttled after regex reset, output: {out}"
    );
}

#[test]
fn follows_rotated_file_and_prints_matching_line() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log = tmp.path().join("app.log");
    File::create(&log).expect("create log file");

    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            log.to_str().expect("utf8 path"),
            "--line-seconds",
            "60",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "25",
            "--regex",
            "rotate",
        ])
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    thread::sleep(Duration::from_millis(300));

    let rotated = tmp.path().join("app.log.1");
    fs::rename(&log, &rotated).expect("rename current log");

    {
        let mut new_file = File::create(&log).expect("create rotated replacement");
        writeln!(new_file, "line after rotate").expect("write rotated line");
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_reopen = false;
    let mut saw_line = false;
    while Instant::now() < deadline {
        if !saw_reopen {
            saw_reopen = wait_for_contains(&stderr_buf, "reopened", Duration::from_millis(200));
        }
        if !saw_line {
            saw_line = wait_for_contains(
                &stdout_buf,
                "\x1b[33mrotate\x1b[0m",
                Duration::from_millis(200),
            );
        }
        if saw_reopen && saw_line {
            break;
        }

        let mut new_file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .expect("open rotated replacement for append");
        writeln!(new_file, "line after rotate").expect("append rotated line");
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let _ = saw_reopen;
    assert!(
        saw_line,
        "did not observe matching line from rotated file after rotation"
    );
}

#[test]
fn no_color_disables_highlight_sequences() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            "--line-seconds",
            "2",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "20",
            "--regex",
            "ERR",
            "--color",
            "green",
        ])
        .env("NO_COLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (_stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    let mut stdin = child.stdin.take().expect("stdin pipe");
    writeln!(stdin, "ERR without color").expect("write line");
    stdin.flush().expect("flush line");

    let saw_plain = wait_for_contains(&stdout_buf, "ERR without color", Duration::from_secs(2));
    let saw_ansi = wait_for_contains(
        &stdout_buf,
        "\x1b[32mERR\x1b[0m",
        Duration::from_millis(300),
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    assert!(saw_plain, "expected plain matched line");
    assert!(!saw_ansi, "did not expect ANSI color when NO_COLOR is set");
}

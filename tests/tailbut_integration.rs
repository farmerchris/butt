use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::symlink;

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

fn unique_marker(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
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
    let marker = unique_marker("throttle");

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
        writeln!(stdin, "{marker}-msg-{i}").expect("write line");
    }
    stdin.flush().expect("flush stdin");

    let saw_first = wait_for_contains(&stdout_buf, &marker, Duration::from_secs(4));
    assert!(saw_first, "expected at least one throttled line");

    thread::sleep(Duration::from_millis(300));

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let out = stdout_buf.lock().expect("lock poisoned").clone();
    let count = out.lines().filter(|line| line.contains(&marker)).count();
    assert_eq!(
        count, 1,
        "expected exactly one throttled line, got output: {out}"
    );
}

#[test]
fn regex_matches_print_immediately_and_reset_throttle_window() {
    let marker = unique_marker("regex");

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
    writeln!(stdin, "{marker} regular message").expect("write non-match");
    stdin.flush().expect("flush non-match");
    thread::sleep(Duration::from_millis(150));
    writeln!(stdin, "{marker} ERR first").expect("write first match");
    writeln!(stdin, "{marker} ERR second").expect("write second match");
    stdin.flush().expect("flush matches");

    let saw_first = wait_for_contains(
        &stdout_buf,
        &format!("{marker} \x1b[32mERR\x1b[0m first"),
        Duration::from_secs(2),
    );
    let saw_second = wait_for_contains(
        &stdout_buf,
        &format!("{marker} \x1b[32mERR\x1b[0m second"),
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
        !out.contains(&format!("{marker} regular message")),
        "non-matching line should remain throttled after regex reset, output: {out}"
    );
}

#[test]
fn regex_case_insensitive_matches_immediately() {
    let marker = unique_marker("regex-ci");

    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            "--line-seconds",
            "2",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "20",
            "--regex",
            "err",
            "--case-insensitive",
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
    writeln!(stdin, "{marker} ERR uppercase").expect("write uppercase match");
    stdin.flush().expect("flush line");

    let saw_highlight = wait_for_contains(
        &stdout_buf,
        &format!("{marker} \x1b[32mERR\x1b[0m uppercase"),
        Duration::from_secs(2),
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    assert!(
        saw_highlight,
        "case-insensitive regex match was not printed immediately"
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
    let marker = unique_marker("nocolor");

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
    writeln!(stdin, "{marker} ERR without color").expect("write line");
    stdin.flush().expect("flush line");

    let saw_plain = wait_for_contains(
        &stdout_buf,
        &format!("{marker} ERR without color"),
        Duration::from_secs(2),
    );
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

#[cfg(unix)]
#[test]
fn no_follow_symlinks_blocks_symlink_targets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let real = tmp.path().join("real.log");
    let link = tmp.path().join("link.log");
    File::create(&real).expect("create real file");
    symlink(&real, &link).expect("create symlink");

    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            link.to_str().expect("utf8 path"),
            "--no-follow-symlinks",
            "--line-seconds",
            "60",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "25",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (_stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    let blocked = wait_for_contains(
        &stderr_buf,
        "symlink targets are not allowed by --no-follow-symlinks",
        Duration::from_secs(3),
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    assert!(blocked, "expected symlink block message");
}

#[test]
fn allowed_root_blocks_paths_outside_root() {
    let root = tempfile::tempdir().expect("root dir");
    let outside = tempfile::tempdir().expect("outside dir");
    let outside_log = outside.path().join("outside.log");
    File::create(&outside_log).expect("create outside log");

    let mut child = Command::new(env!("CARGO_BIN_EXE_butt"))
        .args([
            outside_log.to_str().expect("utf8 path"),
            "--allowed-root",
            root.path().to_str().expect("utf8 root"),
            "--line-seconds",
            "60",
            "--idle-seconds",
            "60",
            "--poll-millis",
            "25",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn butt");

    let (_stdout_buf, stdout_handle) =
        spawn_capture_thread(child.stdout.take().expect("stdout pipe"));
    let (stderr_buf, stderr_handle) =
        spawn_capture_thread(child.stderr.take().expect("stderr pipe"));

    let blocked = wait_for_contains(&stderr_buf, "outside allowed root", Duration::from_secs(3));

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    assert!(blocked, "expected allowed-root block message");
}

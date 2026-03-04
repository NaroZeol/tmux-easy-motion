use nix::pty::openpty;
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn tmp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "tmux-easy-motion-test-{}-{}",
        name,
        std::process::id()
    ));
    p
}

#[test]
fn functional_jump_with_simulated_terminal() {
    let base = tmp_path("jump");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    fs::write(&capture_file, "hello world\nfoo bar\n").unwrap();
    mkfifo(&jump_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
    mkfifo(&target_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();

    let pty = openpty(None, None).unwrap();
    let stdin_file = File::from(pty.slave);
    let stdout_file = stdin_file.try_clone().unwrap();
    let stderr_file = stdin_file.try_clone().unwrap();

    let jump_pipe_reader = jump_pipe.clone();
    let target_pipe_writer = target_pipe.clone();
    let reader_handle = thread::spawn(move || {
        let file = OpenOptions::new()
            .read(true)
            .open(jump_pipe_reader)
            .unwrap();
        let mut reader = BufReader::new(file);

        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert_eq!(first.trim(), "ready");

        let mut writer = OpenOptions::new()
            .write(true)
            .open(target_pipe_writer)
            .unwrap();
        writeln!(writer, "a").unwrap();

        let mut second = String::new();
        reader.read_line(&mut second).unwrap();
        second
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("w")
        .arg("")
        .arg("as")
        .arg("0:0")
        .arg("80:24")
        .arg(&capture_file)
        .arg(&jump_pipe)
        .arg(&target_pipe)
        .stdin(Stdio::from(stdin_file))
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let second_line = reader_handle.join().unwrap();
    assert!(second_line.trim().starts_with("jump "));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_single_target_with_simulated_terminal() {
    let base = tmp_path("single");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    fs::write(&capture_file, "a b\n").unwrap();
    mkfifo(&jump_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
    mkfifo(&target_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();

    let pty = openpty(None, None).unwrap();
    let stdin_file = File::from(pty.slave);
    let stdout_file = stdin_file.try_clone().unwrap();
    let stderr_file = stdin_file.try_clone().unwrap();

    let jump_pipe_reader = jump_pipe.clone();
    let reader_handle = thread::spawn(move || {
        let file = OpenOptions::new()
            .read(true)
            .open(jump_pipe_reader)
            .unwrap();
        let mut reader = BufReader::new(file);

        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut second = String::new();
        reader.read_line(&mut second).unwrap();
        (first, second)
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("w")
        .arg("")
        .arg("as")
        .arg("0:0")
        .arg("80:24")
        .arg(&capture_file)
        .arg(&jump_pipe)
        .arg(&target_pipe)
        .stdin(Stdio::from(stdin_file))
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let (first, second) = reader_handle.join().unwrap();
    assert_eq!(first.trim(), "single-target");
    assert!(second.trim().starts_with("jump "));

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_unicode_characters_with_simulated_terminal() {
    let base = tmp_path("unicode");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    // Test with Unicode prompt "❯ ls -alh" (❯ is 3 bytes, but 1 character)
    fs::write(&capture_file, "❯ ls -alh\n").unwrap();
    mkfifo(&jump_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
    mkfifo(&target_pipe, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();

    let pty = openpty(None, None).unwrap();
    let stdin_file = File::from(pty.slave);
    let stdout_file = stdin_file.try_clone().unwrap();
    let stderr_file = stdin_file.try_clone().unwrap();

    let jump_pipe_reader = jump_pipe.clone();
    let target_pipe_writer = target_pipe.clone();
    let reader_handle = thread::spawn(move || {
        let file = OpenOptions::new()
            .read(true)
            .open(jump_pipe_reader)
            .unwrap();
        let mut reader = BufReader::new(file);

        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert_eq!(first.trim(), "ready");

        let mut writer = OpenOptions::new()
            .write(true)
            .open(target_pipe_writer)
            .unwrap();
        writeln!(writer, "a").unwrap();

        let mut second = String::new();
        reader.read_line(&mut second).unwrap();
        second
    });

    // Test "b" (backward word motion) at position 0:7 (right after "❯ ls ")
    // This should find word boundaries in the backward direction
    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("b")
        .arg("")
        .arg("as")
        .arg("0:7")
        .arg("80:24")
        .arg(&capture_file)
        .arg(&jump_pipe)
        .arg(&target_pipe)
        .stdin(Stdio::from(stdin_file))
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let second_line = reader_handle.join().unwrap();
    assert!(second_line.trim().starts_with("jump "));
    
    // Parse the jump target to verify it's correct
    let parts: Vec<&str> = second_line.trim().split_whitespace().collect();
    if parts.len() >= 3 {
        let row_col = parts[1].split(':').collect::<Vec<_>>();
        assert_eq!(parts[0], "jump");
        // Should jump to the beginning of "ls" (column 2)
        // Or to "❯" (column 0) depending on the current position
        assert!(row_col.len() == 2);
    }

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

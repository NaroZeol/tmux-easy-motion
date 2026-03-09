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
    let reader_handle = thread::spawn(move || {
        let file = OpenOptions::new()
            .read(true)
            .open(jump_pipe_reader)
            .unwrap();
        let mut reader = BufReader::new(file);

        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        first
    });

    // Test "b" (backward word motion) at position 0:5 (after "❯ ls ")
    // Note: using character position not display position for test simplicity
    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("b")
        .arg("")
        .arg("as")
        .arg("0:5")
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

    let first_line = reader_handle.join().unwrap();
    // Should get either "single-target" or "ready" depending on how many targets were found
    assert!(first_line.trim() == "ready" || first_line.trim() == "single-target");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_emoji_character_with_simulated_terminal() {
    let base = tmp_path("emoji");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    // Test with emoji character "hello 🖊 world"
    fs::write(&capture_file, "hello 🖊 world\n").unwrap();
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
        first
    });

    // Test "w" motion when cursor is at the beginning (column 0)
    // This should work even with emoji in the text
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

    let first_output = reader_handle.join().unwrap();
    // Should get either "single-target" or "ready" depending on how many targets were found
    assert!(first_output.trim() == "single-target" || first_output.trim() == "ready");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_emoji_second_column_with_simulated_terminal() {
    let base = tmp_path("emoji_col2");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    // Test with emoji character "hello 🖊 world"
    // emoji 🖊 occupies 2 display columns (e.g., columns 6 and 7)
    fs::write(&capture_file, "hello 🖊 world\n").unwrap();
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
        first
    });

    // Test "b" motion when cursor is at column 7 (second column of emoji 🖊)
    // This should correctly map to the emoji's start position
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

    let first_output = reader_handle.join().unwrap();
    // Should get either "single-target" or "ready" depending on how many targets were found
    assert!(first_output.trim() == "single-target" || first_output.trim() == "ready");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_emoji_first_column_with_simulated_terminal() {
    let base = tmp_path("emoji_col1");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    // Test with emoji character "hello 🖊 world"
    // emoji 🖊 occupies 2 display columns (e.g., columns 6 and 7)
    fs::write(&capture_file, "hello 🖊 world\n").unwrap();
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
        first
    });

    // Test "b" motion when cursor is at column 6 (first column of emoji 🖊)
    // This should correctly map to the emoji's start position
    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("b")
        .arg("")
        .arg("as")
        .arg("0:6")
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

    let first_output = reader_handle.join().unwrap();
    // Should get either "single-target" or "ready" depending on how many targets were found
    assert!(first_output.trim() == "single-target" || first_output.trim() == "ready");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_j_motion_with_emoji_with_simulated_terminal() {
    let base = tmp_path("j_motion_emoji");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    // Test j motion with emoji on first line
    // Cursor is on line 0 column 6 (first column of emoji)
    fs::write(&capture_file, "hello 🖊 world\nline 2\nline 3\n").unwrap();
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
        first
    });

    // Test "j" motion (forward line motion) from cursor at emoji position
    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("j")
        .arg("")
        .arg("as")
        .arg("0:6")
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

    let first_output = reader_handle.join().unwrap();
    assert!(first_output.trim() == "single-target" || first_output.trim() == "ready");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_k_motion_with_emoji_with_simulated_terminal() {
    let base = tmp_path("k_motion_emoji");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    // Test k motion with emoji on second line
    // Cursor is on line 1 column 6 (first column of emoji)
    fs::write(&capture_file, "line 1\nhello 🖊 world\nline 3\n").unwrap();
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
        first
    });

    // Test "k" motion (backward line motion) from cursor at emoji position
    let mut child = Command::new(env!("CARGO_BIN_EXE_tmux-easy-motion"))
        .arg("fg=colour242")
        .arg("fg=colour196,bold")
        .arg("fg=brightyellow,bold")
        .arg("fg=yellow,bold")
        .arg("k")
        .arg("")
        .arg("as")
        .arg("1:6")
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

    let first_output = reader_handle.join().unwrap();
    assert!(first_output.trim() == "single-target" || first_output.trim() == "ready");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_j_motion_with_prompt_like_text() {
    let base = tmp_path("j_prompt");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    fs::write(&capture_file, "alpha\nbeta\ntmux-easy-motion on  main 🖊\n❯ ").unwrap();
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
        .arg("j")
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
    assert_eq!(second_line.trim(), "jump 1:0");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn functional_k_motion_with_prompt_like_text() {
    let base = tmp_path("k_prompt");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let capture_file = base.join("capture.out");
    let jump_pipe = base.join("jump.pipe");
    let target_pipe = base.join("target.pipe");

    fs::write(&capture_file, "build ok\ntmux-easy-motion on  main 🖊\n❯ ").unwrap();
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
        .arg("k")
        .arg("")
        .arg("as")
        .arg("2:0")
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
    assert_eq!(second_line.trim(), "jump 1:0");

    thread::sleep(Duration::from_millis(50));
    let _ = fs::remove_dir_all(base);
}

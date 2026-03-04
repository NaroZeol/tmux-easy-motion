use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{}-{}-{}", prefix, std::process::id(), nanos)
}

fn tmp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(unique_name(name));
    p
}

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_tmux(socket_name: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("tmux")
        .arg("-L")
        .arg(socket_name)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run tmux {:?}: {}", args, e))?;

    if !output.status.success() {
        return Err(format!(
            "tmux {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn start_isolated_tmux(socket_name: &str, session_name: &str, pane_cmd: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .arg("-L")
        .arg(socket_name)
        .arg("-f")
        .arg("/dev/null")
        .args(["new-session", "-d", "-s", session_name, pane_cmd])
        .status()
        .map_err(|e| format!("failed to start isolated tmux: {}", e))?;

    if !status.success() {
        return Err("failed to start isolated tmux server".to_string());
    }

    Ok(())
}

fn cleanup_tmux(socket_name: &str) {
    let _ = Command::new("tmux")
        .arg("-L")
        .arg(socket_name)
        .args(["kill-server"])
        .status();
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace("'", "'\"'\"'"))
}

fn wait_child_with_timeout(child: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("try_wait failed: {}", e))? {
            return Ok(status);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("child timed out after {:?}", timeout));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn locate_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ensure_release_binary_exists(repo_root: &Path) {
    let bin = repo_root.join("target/release/tmux-easy-motion");
    assert!(
        bin.exists(),
        "missing release binary at {} ; run `cargo build --release` first",
        bin.display()
    );
}

fn create_tmux_wrapper(base: &Path, socket_name: &str) -> PathBuf {
    let wrapper_dir = base.join("bin");
    fs::create_dir_all(&wrapper_dir).unwrap();

    let wrapper = wrapper_dir.join("tmux");
    let real_tmux = Command::new("sh")
        .arg("-lc")
        .arg("command -v tmux")
        .output()
        .expect("failed to locate tmux");
    let tmux_path = String::from_utf8_lossy(&real_tmux.stdout).trim().to_string();

    let script = format!(
        "#!/usr/bin/env bash\nexec {} -L {} \"$@\"\n",
        shell_single_quote(&tmux_path),
        shell_single_quote(socket_name)
    );
    fs::write(&wrapper, script).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&wrapper).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper, perms).unwrap();
    }

    wrapper_dir
}

fn tmux_display(socket_name: &str, target: &str, fmt: &str) -> String {
    run_tmux(
        socket_name,
        &["display-message", "-p", "-t", target, fmt],
    )
    .unwrap()
    .trim()
    .to_string()
}

fn move_copy_cursor(socket_name: &str, pane_id: &str, row: usize, col: usize) {
    // Enter copy-mode and normalize position at top-left first.
    run_tmux(socket_name, &["copy-mode", "-t", pane_id]).unwrap();
    run_tmux(
        socket_name,
        &["send-keys", "-t", pane_id, "-X", "-N", "200", "cursor-up"],
    )
    .unwrap();
    run_tmux(socket_name, &["send-keys", "-t", pane_id, "-X", "start-of-line"]).unwrap();

    if row > 0 {
        run_tmux(
            socket_name,
            &[
                "send-keys",
                "-t",
                pane_id,
                "-X",
                "-N",
                &row.to_string(),
                "cursor-down",
            ],
        )
        .unwrap();
    }

    if col > 0 {
        run_tmux(
            socket_name,
            &[
                "send-keys",
                "-t",
                pane_id,
                "-X",
                "-N",
                &col.to_string(),
                "cursor-right",
            ],
        )
        .unwrap();
    }
}

fn run_easy_motion_sh(
    repo_root: &Path,
    wrapper_dir: &Path,
    server_pid: &str,
    session_id: &str,
    window_id: &str,
    pane_id: &str,
    motion: &str,
    motion_argument: &str,
) {
    run_easy_motion_sh_with_injected_key(
        repo_root,
        wrapper_dir,
        server_pid,
        session_id,
        window_id,
        pane_id,
        motion,
        motion_argument,
        None,
    );
}

fn run_easy_motion_sh_with_injected_key(
    repo_root: &Path,
    wrapper_dir: &Path,
    server_pid: &str,
    session_id: &str,
    window_id: &str,
    pane_id: &str,
    motion: &str,
    motion_argument: &str,
    injected_key: Option<&str>,
) {
    let script = repo_root.join("scripts/easy_motion.sh");
    let original_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{}", wrapper_dir.display(), original_path);

    let mut child = Command::new(script)
        .arg(server_pid)
        .arg(session_id)
        .arg(window_id)
        .arg(pane_id)
        .arg(motion)
        .arg(motion_argument)
        .env("PATH", &test_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(key) = injected_key {
        drive_key_injection_until_exit(
            &mut child,
            server_pid,
            session_id,
            key,
            Duration::from_secs(6),
        );
    }

    let status = wait_child_with_timeout(&mut child, Duration::from_secs(15))
        .expect("easy_motion.sh timed out");
    assert!(status.success(), "easy_motion.sh exited with non-zero status");
}

fn sanitize_session_id(session_id: &str) -> String {
    session_id.strip_prefix('$').unwrap_or(session_id).to_string()
}

fn target_key_pipe_path(server_pid: &str, session_id: &str) -> PathBuf {
    let tmpdir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    let user = Command::new("id")
        .arg("-un")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()));
    let sid = sanitize_session_id(session_id);
    PathBuf::from(format!(
        "{}/tmux-easy-motion-target-key-pipe_{}_{}/{}/target_key.pipe",
        tmpdir, user, server_pid, sid
    ))
}

fn inject_target_key_with_retry(
    server_pid: &str,
    session_id: &str,
    key: &str,
    timeout: Duration,
) -> bool {
    let pipe = target_key_pipe_path(server_pid, session_id);
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if pipe.exists() {
            // Open FIFO as read+write to avoid blocking if reader side is not yet attached.
            if let Ok(mut f) = std::fs::OpenOptions::new().read(true).write(true).open(&pipe) {
                if writeln!(f, "{}", key).is_ok() {
                    return true;
                }
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    false
}

fn drive_key_injection_until_exit(
    child: &mut Child,
    server_pid: &str,
    session_id: &str,
    key: &str,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(Some(_status)) = child.try_wait() {
            // Child already exited; no further injection needed.
            return;
        }
        let _ = inject_target_key_with_retry(server_pid, session_id, key, Duration::from_millis(120));
        thread::sleep(Duration::from_millis(40));
    }
}

fn prepare_isolated_pane(socket_name: &str, session_name: &str, content: &str) -> (String, String, String, String) {
    let base = tmp_path("e2e-tmux-fixture");
    fs::create_dir_all(&base).unwrap();
    let fixture = base.join("pane-content.txt");
    fs::write(&fixture, content).unwrap();

    let pane_cmd = format!(
        "sh -lc 'cat {}; exec tail -f /dev/null'",
        shell_single_quote(fixture.to_str().unwrap())
    );
    start_isolated_tmux(socket_name, session_name, &pane_cmd).unwrap();

    // Let pane command flush output.
    thread::sleep(Duration::from_millis(150));

    let pane_id = tmux_display(socket_name, &format!("{}:0.0", session_name), "#{pane_id}");
    let session_id = tmux_display(socket_name, &pane_id, "#{session_id}");
    let window_id = tmux_display(socket_name, &pane_id, "#{window_id}");
    let server_pid = run_tmux(socket_name, &["display-message", "-p", "#{pid}"])
        .unwrap()
        .trim()
        .to_string();

    (pane_id, session_id, window_id, server_pid)
}

fn assert_copy_cursor(socket_name: &str, pane_id: &str, row: usize, col: usize) {
    let pos = tmux_display(socket_name, pane_id, "#{copy_cursor_y}:#{copy_cursor_x}");
    assert_eq!(pos, format!("{}:{}", row, col));
}

#[test]
fn e2e_easy_motion_sh_j_single_target() {
    if !tmux_available() {
        eprintln!("Skipping e2e tmux test because tmux is not available");
        return;
    }

    let repo_root = locate_repo_root();
    ensure_release_binary_exists(&repo_root);

    let socket_name = unique_name("tmux-e2e-sock");
    let session_name = unique_name("tmux-e2e-session");
    let (pane_id, session_id, window_id, server_pid) =
        prepare_isolated_pane(&socket_name, &session_name, "top\\nmid\\nbot\\n");

    let base = tmp_path("e2e-wrapper");
    let wrapper_dir = create_tmux_wrapper(&base, &socket_name);

    move_copy_cursor(&socket_name, &pane_id, 1, 0);
    run_easy_motion_sh(
        &repo_root,
        &wrapper_dir,
        &server_pid,
        &session_id,
        &window_id,
        &pane_id,
        "j",
        "",
    );

    let in_mode = tmux_display(&socket_name, &pane_id, "#{pane_in_mode}");
    assert_eq!(in_mode, "1");

    cleanup_tmux(&socket_name);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn e2e_easy_motion_sh_k_single_target() {
    if !tmux_available() {
        eprintln!("Skipping e2e tmux test because tmux is not available");
        return;
    }

    let repo_root = locate_repo_root();
    ensure_release_binary_exists(&repo_root);

    let socket_name = unique_name("tmux-e2e-sock");
    let session_name = unique_name("tmux-e2e-session");
    let (pane_id, session_id, window_id, server_pid) =
        prepare_isolated_pane(&socket_name, &session_name, "first\\nsecond\\n");

    let base = tmp_path("e2e-wrapper");
    let wrapper_dir = create_tmux_wrapper(&base, &socket_name);

    move_copy_cursor(&socket_name, &pane_id, 1, 0);
    run_easy_motion_sh(
        &repo_root,
        &wrapper_dir,
        &server_pid,
        &session_id,
        &window_id,
        &pane_id,
        "k",
        "",
    );

    assert_copy_cursor(&socket_name, &pane_id, 0, 0);

    cleanup_tmux(&socket_name);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn e2e_easy_motion_sh_unicode_emoji_finds_emoji() {
    if !tmux_available() {
        eprintln!("Skipping e2e tmux test because tmux is not available");
        return;
    }

    let repo_root = locate_repo_root();
    ensure_release_binary_exists(&repo_root);

    let socket_name = unique_name("tmux-e2e-sock");
    let session_name = unique_name("tmux-e2e-session");
    let (pane_id, session_id, window_id, server_pid) = prepare_isolated_pane(
        &socket_name,
        &session_name,
        "prompt ❯ hello 🖊 world\\n",
    );

    let base = tmp_path("e2e-wrapper");
    let wrapper_dir = create_tmux_wrapper(&base, &socket_name);

    move_copy_cursor(&socket_name, &pane_id, 0, 0);
    run_easy_motion_sh(
        &repo_root,
        &wrapper_dir,
        &server_pid,
        &session_id,
        &window_id,
        &pane_id,
        "f",
        "🖊",
    );

    // "prompt ❯ hello " occupies 15 display columns, then emoji starts at col 15.
    assert_copy_cursor(&socket_name, &pane_id, 0, 15);

    cleanup_tmux(&socket_name);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn e2e_easy_motion_sh_multi_target_select_via_pipe_key() {
    if !tmux_available() {
        eprintln!("Skipping e2e tmux test because tmux is not available");
        return;
    }

    let repo_root = locate_repo_root();
    ensure_release_binary_exists(&repo_root);

    let socket_name = unique_name("tmux-e2e-sock");
    let session_name = unique_name("tmux-e2e-session");
    let (pane_id, session_id, window_id, server_pid) =
        prepare_isolated_pane(&socket_name, &session_name, "alpha beta gamma\n");

    let base = tmp_path("e2e-wrapper");
    let wrapper_dir = create_tmux_wrapper(&base, &socket_name);

    move_copy_cursor(&socket_name, &pane_id, 0, 0);
    run_easy_motion_sh_with_injected_key(
        &repo_root,
        &wrapper_dir,
        &server_pid,
        &session_id,
        &window_id,
        &pane_id,
        "w",
        "",
        Some("a"),
    );

    // Targets are "beta" and "gamma"; default first target key is "a" => jump to beta.
    assert_copy_cursor(&socket_name, &pane_id, 0, 6);

    cleanup_tmux(&socket_name);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn e2e_easy_motion_sh_auto_begin_selection_enabled() {
    if !tmux_available() {
        eprintln!("Skipping e2e tmux test because tmux is not available");
        return;
    }

    let repo_root = locate_repo_root();
    ensure_release_binary_exists(&repo_root);

    let socket_name = unique_name("tmux-e2e-sock");
    let session_name = unique_name("tmux-e2e-session");
    let (pane_id, session_id, window_id, server_pid) =
        prepare_isolated_pane(&socket_name, &session_name, "foo x\n");

    // Verify easy_motion.sh honors @easy-motion-auto-begin-selection in tmux options.
    run_tmux(
        &socket_name,
        &["set-option", "-g", "@easy-motion-auto-begin-selection", "1"],
    )
    .unwrap();

    let base = tmp_path("e2e-wrapper");
    let wrapper_dir = create_tmux_wrapper(&base, &socket_name);

    move_copy_cursor(&socket_name, &pane_id, 0, 0);
    run_easy_motion_sh(
        &repo_root,
        &wrapper_dir,
        &server_pid,
        &session_id,
        &window_id,
        &pane_id,
        "f",
        "x",
    );

    assert_copy_cursor(&socket_name, &pane_id, 0, 4);
    // begin-selection sets the mark at current cursor; make it non-empty
    // so `selection_present` becomes observable in a stable way.
    run_tmux(
        &socket_name,
        &["send-keys", "-t", &pane_id, "-X", "-N", "1", "cursor-right"],
    )
    .unwrap();
    let selection = tmux_display(&socket_name, &pane_id, "#{selection_present}");
    assert_eq!(selection, "1");

    cleanup_tmux(&socket_name);
    let _ = fs::remove_dir_all(base);
}

# tmux-easy-motion: Knowledge Base

## Project Overview

**tmux-easy-motion** is a Rust port of the original tmux-easy-motion plugin, providing Vim-style motion navigation for tmux copy-mode with pre-computed jump targets.

### Goals
- Replicate upstream Python plugin behavior using Rust core engine
- Support TPM (tmux Plugin Manager) installation
- Provide zero-Rust-dependency user experience via pre-compiled binaries
- Maintain multi-platform support (Linux/macOS × x86_64/aarch64)

---

## Critical Technical Insights

### 1. Copy-Mode Cursor Positioning

**Challenge**: After jumping to a target position in copy-mode, the cursor appeared to stay at prompt line instead of target.

**Root Cause**: `#{cursor_y}:#{cursor_x}` reports normal-mode cursor position, not copy-mode cursor. When a pane is in copy-mode, must use `#{copy_cursor_y}:#{copy_cursor_x}`.

**Solution**: Implemented smart read function that detects copy-mode state via `#{pane_in_mode}` and reads appropriate cursor:

```bash
if [[ "$(tmux display-message -p -t "${pane_id}" "#{pane_in_mode}")" == "1" ]]; then
    # In copy-mode, use copy cursor
    tmux display-message -p -t "${pane_id}" "#{copy_cursor_y}:#{copy_cursor_x}"
else
    # In normal mode, use normal cursor
    tmux display-message -p -t "${pane_id}" "#{cursor_y}:#{cursor_x}"
fi
```

### 2. Terminal State Preservation Using Swap Panes

**Challenge**: Running Rust binary directly in original pane polluted terminal buffer and selection state persisted.

**Root Cause**: Two issues:
1. Output from Rust program overwrote pane buffer
2. Exiting copy-mode incorrectly or at wrong time left selection highlight active

**Solution**: Adopted upstream's **swap-pane architecture**:
1. Capture original pane content before entering copy-mode
2. Create new temporary swap pane
3. Swap positions so selection UI displays where user is
4. Run Rust binary in swap pane (its TTY receives output)
5. After user selection, swap back to original pane (still in copy-mode, buffer intact)
6. Move cursor in original pane and **keep copy-mode active**
7. User presses ESC naturally to exit copy-mode

**Key insight**: Don't exit copy-mode via `cancel` command. Let user exit naturally. Upstream design was right.

### 3. ENOTTY Graceful Handling

**Challenge**: When stdin has no controlling TTY (e.g., tmux `run-shell` context), `tcgetattr()` fails with `ENOTTY`.

**Solution**: Return `Option<TerminalGuard>` instead of failing:

```rust
pub(crate) fn setup(fd: i32) -> Result<Option<Self>, String> {
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    let original = match termios::tcgetattr(borrowed) {
        Ok(settings) => settings,
        Err(Errno::ENOTTY) => return Ok(None),  // Not a TTY, proceed without terminal setup
        Err(e) => return Err(e.to_string()),
    };
    // ... rest of setup
}
```

---

## Architecture

### Module Structure (Rust)

```
src/
├── main.rs          → Thin entrypoint (wires modules, handles exit code/sleep)
├── app.rs           → Main application flow (parse → resolve → interact)
├── config.rs        → CLI argument parsing + style/color parsing
├── motion.rs        → Motion regex templates, text position conversions
├── grouping.rs      → Multi-level target grouping algorithm
├── render.rs        → Terminal rendering and pipe message output
├── terminal.rs      → termios-based raw mode guard
└── types.rs         → Shared types (Config, GroupedIndices, JumpTargetType)
```

### Script Integration

```
scripts/
├── easy_motion.sh           → Runtime orchestrator (capture, swap, execute, position)
├── helpers.sh               → tmux helpers (cursor read/write, pane operations)
├── options.sh               → Option loading with defaults
├── pipe_target_key.sh       → Target key pipe writer
├── common_variables.sh      → Shared constants
```

### Plugin Entry Points

- **`tmux-easy-motion.tmux`**: TPM-standard entry (symlink to main script)
- **`easy_motion.tmux`**: Main binding setup (key tables, motion registration)

### Execution Flow

1. User activates motion binding (e.g., `<prefix>w`)
2. **easy_motion.tmux** → `scripts/easy_motion.sh w`
3. **easy_motion.sh**:
   - Captures pane text
   - Enters copy-mode on original pane
   - Creates swap pane (temp window)
   - Swaps so UI displays to user
   - Runs Rust binary (input from capture file, output to swap pane TTY)
4. **Rust binary**:
   - Parses args (styles, motion, cursor, pane size)
   - Computes motion matches via regex
   - Groups indices for multi-key targets
   - Renders highlighted text
   - Reads user key presses from pipe
   - Outputs `jump row:col` command
5. **easy_motion.sh** resumes:
   - Swaps pane back (original pane still in copy-mode)
   - Calls `set_cursor_position` to move copy-cursor
   - *Stays in copy-mode* for user to exit naturally
   - Cleans up swap window

---

## Binary Packaging Strategy

### Download Priority

1. **GitHub Release (preferred)**
   - Auto-detected platform: Linux/macOS × x86_64/aarch64
   - Requires: `curl`
   - Asset naming: `tmux-easy-motion-{platform}`

2. **Local build (fallback)**
   - Requires: `cargo` + Rust toolchain
   - Command: `cargo build --release -q`

### Configuration

In `scripts/easy_motion.sh`:

```bash
GITHUB_REPO="NaroZeol/tmux-easy-motion"
GITHUB_RELEASE_API="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
```

Users can fork and update `GITHUB_REPO` to point to their own builds.

### Automated Release Process (GitHub Actions)

File: `.github/workflows/release.yml`

**Trigger**: `git tag v*.*.*`

**Outputs**: Multi-platform binaries attached to GitHub release:
- `tmux-easy-motion-linux-x86_64`
- `tmux-easy-motion-linux-aarch64`
- `tmux-easy-motion-macos-x86_64`
- `tmux-easy-motion-macos-aarch64`

**Release steps**:
1. Update version in `Cargo.toml`
2. `git tag v0.x.x`
3. `git push origin v0.x.x`
4. Actions auto-build and release

---

## TPM Integration

### Installation

User adds to `~/.tmux.conf`:

```tmux
set -g @plugin 'tmux-plugins/tpm'
set -g @plugin 'YOUR_USERNAME/tmux-easy-motion'

run '~/.tmux/plugins/tpm/tpm'
```

Then: `prefix + I` (capital) to install.

### How It Works

1. TPM clones plugin repo to `~/.tmux/plugins/tmux-easy-motion/`
2. On first tmux load, `tmux-easy-motion.tmux` is sourced
3. Plugin initializes (loads bindings, sets up pipes)
4. On first motion invocation:
   - `easy_motion.sh` checks for binary
   - Downloads from GitHub release (or builds locally)
   - Plugin is ready to use

---

## Cursor Motion in Copy-Mode

### Key Behavior Details

**Linewise vs. character motions**:
- Linewise (`j/J/k/K/bd-j`): Match on beginning of content per line
- Character-based: Match within line

**Regex patterns**:
- Forward `w`: `\b(\w)` (word start)
- Forward `e`: `(\w)\b` (word end)
- Backward `b`: `\b(\w)` (bidirectional adjusted)
- Line `j`: `^(?:\s*)(\S)` (first non-space)

**Bidirectional motions** (reachable from both directions):
- Computed as: forward_results + reverse(backward_results)
- Merges results in proximity order (closest first)

### Copy-Mode Cursor Position Edge Cases

**Issue**: Moving cursor in copy-mode can change column due to line wrapping or line-end constraints.

**Handling**: After vertical movement, re-query cursor position before applying horizontal delta:

```bash
set_cursor_position() {
    # Move rows
    if (( rel_row < 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( -rel_row ))" cursor-up
    elif (( rel_row > 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( rel_row ))" cursor-down
    fi
    
    # Re-query position (column may have changed)
    IFS=':' read -r current_row current_col <<< "$(read_cursor_position "${pane_id}")"
    rel_col="$(( col - current_col ))"
    
    # Move columns relative to new position
    if (( rel_col < 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( -rel_col ))" cursor-left
    elif (( rel_col > 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( rel_col ))" cursor-right
    fi
}
```

---

## Known Limitations & Workarounds

1. **Binary download requires `curl`**: If curl unavailable, falls back to local build.
2. **Non-vi copy-mode users**: Plugin binds to `copy-mode-vi` key table. Non-vi mode not supported (would require separate bindings).
3. **Very dense text**: Multi-level grouping caps at `len(target_keys)` per level. For 26+ match density, requires 3+ key presses.
4. **Terminal color support**: Relies on tmux style codes; respects user's terminal palette (256-color or truecolor).

---

## Testing Strategy

### Functional Tests

File: `tests/functional_sim_terminal.rs`

**Approach**: PTY (pseudo-terminal) + FIFO (named pipe) simulation

Tests verify:
- Motion regex matching
- Target grouping algorithm
- Jump command emission
- Multi-key selection flow

**Run**: `cargo test`

### Manual Testing

1. **Basic motion**: `<prefix>w` in copy-mode, select target
2. **Multi-key**: `<prefix>b`, press first key, then second
3. **Edge cases**: Empty lines, EOL wrapping, bidirectional motion
4. **Cross-platform**: Test on Linux + macOS, x86_64 + ARM64

---

## Code Quality Principles

1. **Modular Rust**: Eight focused modules, each with single responsibility
2. **Safe TTY handling**: `TerminalGuard` RAII pattern ensures cleanup
3. **Shell robustness**: Quote all expansions, use `|| return 1` for error propagation
4. **Minimal dependencies**: Only `regex` + `nix` crates (termios + errno)
5. **Platform detection**: Automatic in binary download script
6. **Idempotent plugin initialization**: Safe to reload tmux config

---

## Common Debugging Steps

### Binary not found
```bash
# Check download logic
curl -s https://api.github.com/repos/NaroZeol/tmux-easy-motion/releases/latest | grep browser_download_url

# Fallback: manual build
cd /path/to/plugin && cargo build --release
```

### Copy-mode stuck after jump
- Ensure `set_cursor_position` is called (moves copy-cursor, not normal cursor)
- Check `#{pane_in_mode}` returns `1` when expected
- Verify `cancel` command NOT called (let user ESC)

### Motion matches not showing
- Verify pane text was captured: `cat "${capture_file}"`
- Check regex pattern for motion: `motion_regex_template "${motion}"`
- Test Rust binary directly:
  ```bash
  ./target/release/tmux-easy-motion \
    "fg=colour242" \        # dim style
    "fg=colour196,bold" \   # highlight
    "..." \                 # ... (8+ args)
  ```

---

## References

- **Original Python**: https://github.com/IngoMeyer441/tmux-easy-motion
- **TPM**: https://github.com/tmux-plugins/tpm
- **tmux copy-mode-vi**: `man tmux` → `COPY MODE BINDINGS (vi)`
- **nix crate**: termios, errno handling (Rust POSIX bindings)

---

## Maintenance Checklist

- [ ] Update `Cargo.toml` version before release
- [ ] Test on Linux and macOS (local or CI)
- [ ] Verify GitHub Actions workflow runs on `git tag`
- [ ] Check binary names match `detect_platform()` output
- [ ] Update `GITHUB_REPO` in downstream forks

---

**Last Updated**: March 5, 2026  
**Status**: Production-ready, TPM-compatible, multi-platform binary distribution

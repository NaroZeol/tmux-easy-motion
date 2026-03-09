# tmux-easy-motion: Repository Knowledge Base

## Overview

`tmux-easy-motion` is a tmux plugin with a Rust core and shell integration layer. It implements Vim-style motions for tmux panes, mainly targeting copy-mode navigation. The project is structured around a tmux script entrypoint that captures pane state, launches a Rust renderer in an isolated swap pane, collects target-key input through FIFOs, and then moves the original copy-mode cursor to the selected location.

The current package metadata is in `Cargo.toml`:

- crate name: `tmux-easy-motion`
- version: `0.1.0`
- edition: `2021`
- dependencies: `regex`, `nix`, `unicode-width`

## Current Runtime Architecture

The runtime is split into three layers:

### 1. tmux Entry Layer

Files:

- `tmux-easy-motion.tmux`
- `easy_motion.tmux`

`tmux-easy-motion.tmux` is a thin TPM-style wrapper that invokes `easy_motion.tmux`.

`easy_motion.tmux` is the real plugin entrypoint. It:

- loads helpers and tmux options
- binds prefix tables `easy-motion`, `easy-motion-g`, and `easy-motion-target`
- registers motion bindings like `b`, `w`, `j`, `k`, `f`, `t`, `c`
- uses `command-prompt` for motions requiring 1 or 2 characters
- writes selected target keys into a FIFO through `scripts/pipe_target_key.sh`

Important behavior:

- the normal activation key is `@easy-motion-prefix`
- copy-mode activation key is `@easy-motion-copy-mode-prefix`
- target-key presses are handled in tmux key table `easy-motion-target`

### 2. Shell Orchestration Layer

Primary file:

- `scripts/easy_motion.sh`

Supporting files:

- `scripts/helpers.sh`
- `scripts/options.sh`
- `scripts/common_variables.sh`
- `scripts/pipe_target_key.sh`

This layer does the operational work:

1. ensures a release binary exists, optionally downloading it from GitHub Releases or building locally
2. reads plugin options from tmux globals
3. captures the current pane viewport into a temporary file
4. forces copy-mode, preserving any existing copy cursor location
5. clamps the cursor to the pane width to keep captured text and cursor coordinates aligned
6. creates a temporary swap window/pane using `tail -f /dev/null`
7. swaps the temporary pane into the user's visible position
8. respawns that pane with a generated shell runner script that launches the Rust binary
9. waits on a command FIFO for `ready`, `single-target`, and `jump row:col`
10. swaps back to the original pane, explicitly re-enters copy-mode, and applies the cursor jump
11. optionally begins a copy selection if `@easy-motion-auto-begin-selection` is enabled
12. kills the swap window

Key implementation details:

- the Rust UI no longer writes directly to `#{pane_tty}`
- the swap pane is driven via `tmux respawn-pane -k`
- the generated runner script ends with `exec tail -f /dev/null` so the temporary pane stays alive until the shell script performs cleanup
- stderr from the Rust runner is captured into a temp file and surfaced if the runner exits early

This shell layer is the main place where tmux lifecycle bugs show up.

### 3. Rust Core Layer

Files:

- `src/main.rs`
- `src/app.rs`
- `src/config.rs`
- `src/motion.rs`
- `src/grouping.rs`
- `src/render.rs`
- `src/terminal.rs`
- `src/types.rs`

`src/main.rs` is intentionally thin. It delegates to `app::run_with_tmux_error_display()` and then sleeps for one second before exiting. That sleep is currently part of the binary behavior and should be treated as intentional unless proven otherwise.

`src/app.rs` owns the core flow:

- parse CLI args into a `Config`
- load the capture buffer from disk
- convert tmux row/column into internal text position
- compute motion target indices
- build grouped targets
- render and/or emit command-pipe signals
- consume target key selections from the target-key FIFO
- emit final `jump row:col`

It also handles shell-visible error display by invoking `tmux display-message` when possible.

## Core Data Flow

### Inputs to the Rust binary

The runner script passes these arguments in order:

1. dim style
2. highlight style
3. first grouped highlight style
4. second grouped highlight style
5. motion name
6. motion argument or empty string
7. target key set
8. cursor position as `row:col`
9. pane size as `width:height`
10. capture file path
11. command pipe path
12. target-key pipe path

### Named-pipe protocol

The command pipe carries control output from Rust to shell:

- `ready`
- `single-target`
- `jump row:col`

The target-key pipe carries a single line for each key press from tmux back into the Rust process:

- one target character
- `esc`

The shell script treats early EOF as either cancellation or failure, depending on whether the runner left stderr content behind.

## Key Source File Responsibilities

### `src/config.rs`

Responsibilities:

- validates the motion name
- decides whether a motion argument is required
- parses cursor position and pane size pairs
- parses tmux style strings into ANSI escape sequences

Supported style syntax includes:

- `fg=colour242`
- `bg=colour17`
- named colors like `brightyellow`
- attributes like `bold`, `dim`, `underscore`, `reverse`, `italics`
- truecolor hex values like `#rrggbb`

### `src/motion.rs`

This is the most sensitive file for correctness.

Responsibilities:

- defines valid motions and regex templates
- converts tmux display coordinates to text byte offsets
- converts text positions back to tmux-style display coordinates
- handles forward, backward, and bidirectional motions
- differentiates linewise motions from charwise motions
- handles UTF-8 and display-width-aware cursor mapping

Important invariants:

- tmux reports cursor columns in display cells, not bytes
- multi-byte and width-2 characters must map to safe UTF-8 boundaries
- `j` and `k` use multiline regex mode and match the first non-whitespace character on target lines
- bidirectional motions interleave forward and backward results

Prompt-related regression coverage now exists here for:

- `j_motion_handles_two_line_prompt_text`
- `k_motion_handles_two_line_prompt_text`

### `src/grouping.rs`

Responsibilities:

- recursively groups indices into a tree of `GroupedIndices`
- computes slot sizes when there are more matches than target keys
- generates renderable jump-target markers with ranking:
    - `Direct`
    - `Group`
    - `Preview`

This file controls how multi-level easy-motion labels are assigned.

### `src/render.rs`

Responsibilities:

- sorts jump targets by position and rank
- renders dimmed original text plus highlighted replacement target keys
- emits ANSI screen control sequences
- disables autowrap during the temporary UI render
- restores ANSI state

Critical implementation note:

- rendering must never slice strings on raw byte offsets unless those offsets are verified as UTF-8 character boundaries

The current implementation contains explicit helpers:

- `previous_char_boundary()`
- `next_char_boundary()`

These were added to fix prompt-related crashes involving characters like `❯`, ``, and `🖊`.

### `src/terminal.rs`

Responsibilities:

- switches stdin to non-canonical, no-echo mode when stdin is a TTY
- returns `Ok(None)` when stdin is not a terminal (`ENOTTY`)
- restores the original terminal settings in `Drop`

This is intentionally tolerant because tmux `run-shell` contexts often do not provide a normal controlling TTY.

### `scripts/helpers.sh`

Responsibilities:

- manage per-session FIFO directories
- create/reset the target-key FIFO
- discover tmux server PID from `$TMUX`
- create the temporary swap window
- read the correct cursor source depending on copy-mode state
- move the copy cursor using tmux copy-mode commands

Critical rule:

- when `#{pane_in_mode}` is `1`, always use `#{copy_cursor_y}:#{copy_cursor_x}`
- otherwise use `#{cursor_y}:#{cursor_x}`

## Supported Motions

Current valid motions are:

- `b`, `B`, `ge`, `gE`, `e`, `E`, `w`, `W`
- `j`, `J`, `k`, `K`
- `f`, `F`, `t`, `T`
- `bd-w`, `bd-W`, `bd-e`, `bd-E`, `bd-j`, `bd-J`, `bd-f`, `bd-f2`, `bd-t`, `bd-T`
- `c`

Motions requiring arguments are:

- `f`, `F`, `t`, `T`
- `bd-f`, `bd-f2`, `bd-t`, `bd-T`

## tmux Options and Defaults

Defined in `scripts/options.sh`:

- `@easy-motion-prefix`: default `Space`
- `@easy-motion-copy-mode-prefix`: default `Space`
- `@easy-motion-dim-style`: default `fg=colour242`
- `@easy-motion-highlight-style`: default `fg=colour196,bold`
- `@easy-motion-highlight-2-first-style`: default `fg=brightyellow,bold`
- `@easy-motion-highlight-2-second-style`: default `fg=yellow,bold`
- `@easy-motion-target-keys`: default `asdghklqwertyuiopzxcvbnmfj;`
- `@easy-motion-verbose`: default `0`
- `@easy-motion-auto-begin-selection`: default `0`

At present, `@easy-motion-verbose` is read but not used for runtime logging.

## Testing Layout

### `tests/functional_sim_terminal.rs`

This is the PTY-level functional test suite. It launches the Rust binary directly with:

- a pseudo-terminal from `nix::pty::openpty`
- capture files
- named pipes for command and target-key I/O

Current functional coverage includes:

- basic jumps
- single-target flow
- Unicode prompt text
- emoji in regular motions
- emoji when cursor is on either display column of a width-2 glyph
- `j` and `k` with emoji-containing lines
- `j` and `k` with prompt-like multi-line text

### `tests/e2e_tmux.rs`

This is the tmux integration suite. It spins up isolated tmux servers using `tmux -L <socket> -f /dev/null` and drives the plugin shell script as tmux would.

Current e2e coverage includes:

- `j` single-target jump
- `k` single-target jump
- Unicode emoji target finding with `f`
- multi-target selection by injected key
- auto-begin-selection option
- single-quote motion argument handling
- `k` from a prompt-like last line
- `j` over prompt-like multi-line content

Important operational fact:

- e2e tests should be run serially with `--test-threads=1`

Parallel execution can cause flakiness because tests rely on tmux, PTYs, FIFOs, and process timing.

## Known Runtime Constraints

### Display Width vs Byte Offset

Any work involving cursor positions must keep these separate:

- tmux gives display columns
- Rust string slicing uses byte offsets
- width-2 glyphs and multi-byte UTF-8 must be normalized before slicing or matching

### Prompt-Like Last Lines

The project now explicitly handles prompts like:

```text
tmux-easy-motion on  main 🖊
❯ 
```

This matters because:

- `j` and `k` can target prompt lines
- the prompt contains width-1 and width-2 visible glyphs plus multi-byte UTF-8 sequences
- render-time slicing previously panicked if a target landed inside a multi-byte glyph boundary

### Copy-Mode Persistence

Some tmux and terminal combinations may drop copy-mode during swap-pane transitions. The shell script now re-enters copy-mode after swapping back before applying the jump.

### Temporary Pane Lifecycle

The respawned swap pane stays alive via `tail -f /dev/null` after the Rust process returns. This prevents single-target motions from tearing down the visible temporary pane before the shell script finishes cleanup.

## Build and Development Workflow

Common local commands:

```bash
cargo build --release
cargo test -- --test-threads=1
bash -n scripts/*.sh
```

Release binary path:

```text
target/release/tmux-easy-motion
```

Binary acquisition order in the plugin:

1. existing local release binary
2. GitHub Release download
3. optional local cargo build when `EASY_MOTION_ALLOW_BUILD=1`

## CI and Release Workflow

### `.github/workflows/ci.yml`

Current CI behavior:

- runs on every push and pull request
- installs tmux on Ubuntu
- installs stable Rust
- builds the release binary
- runs only the tmux e2e suite, serially

Current CI does not run the functional PTY suite separately.

### `.github/workflows/release.yml`

Current release workflow:

- triggers on tags matching `v*`
- builds these targets:
    - `x86_64-unknown-linux-gnu`
    - `aarch64-unknown-linux-gnu`
    - `aarch64-apple-darwin`
- uses `cross` for Linux aarch64
- publishes binaries through `softprops/action-gh-release`

Important accuracy note:

- the workflow currently does not build `x86_64-apple-darwin`
- the README claims support for both macOS x86_64 and aarch64
- if documentation and release assets must match, this discrepancy should be fixed deliberately rather than assumed away

## Notable Debugging Patterns

### If motions fail only on prompt-heavy panes

Check:

- UTF-8 boundary handling in `src/render.rs`
- display-width mapping in `src/motion.rs`
- whether the release binary was rebuilt after Rust-side changes

### If copy-mode cursor jumps to the wrong place

Check:

- whether the shell is reading `copy_cursor_*` rather than `cursor_*`
- whether pane width clamping changed the cursor column before capture
- whether copy-mode was re-entered after swap-back

### If the script exits with no visible jump

Check:

- runner stderr temp file created by `write_swap_runner_script()`
- whether Rust emitted `ready` / `single-target` / `jump`
- whether the target-key FIFO was reset and recreated correctly

### If a direct binary run behaves differently from tmux

Remember:

- functional tests use test binaries in `target/debug/deps/...`
- the plugin script runs the release binary in `target/release/tmux-easy-motion`
- rebuilding `cargo build --release` is required after Rust fixes when validating the real plugin path

## Practical Maintenance Checklist

- update this file when runtime flow, CI targets, or test coverage changes
- keep README claims aligned with actual workflow targets and supported release assets
- when touching motion or render code, add regression tests in both:
    - `tests/functional_sim_terminal.rs`
    - `tests/e2e_tmux.rs` if tmux orchestration is involved
- prefer serial test execution for tmux-related verification
- rebuild the release binary before validating plugin behavior through tmux

## Status Snapshot

As of the current repository state:

- swap-pane UI is launched through `respawn-pane`, not direct pane TTY redirection
- prompt-safe `j` and `k` handling has explicit regression coverage
- Unicode render slicing is guarded by character-boundary helpers
- shell error handling can surface runner stderr instead of silently treating every early exit as cancellation

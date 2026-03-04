# tmux-easy-motion (Rust)

A Rust reimplementation of tmux easy-motion behavior, based on the original plugin design.

## Features

- Vim-style motions (`b/B/ge/gE/e/E/w/W/j/J/k/K/f/F/t/T/bd-*/c`)
- Multi-level target key grouping for dense jump sets
- Named-pipe protocol compatible flow (`ready` / `single-target` / `jump row:col`)
- Copy-mode cursor movement flow designed to match upstream behavior

## Build

```bash
cargo build --release
```

Binary output:

```text
target/release/tmux-easy-motion
```

## Installation

### TPM (recommended)

Add this to your `~/.tmux.conf`:

```tmux
set -g @plugin 'tmux-plugins/tpm'
set -g @plugin 'YOUR_GITHUB_USERNAME/tmux-easy-motion'

run '~/.tmux/plugins/tpm/tpm'
```

Then install plugins with `prefix + I` (capital i).

This plugin entrypoint follows TPM conventions and is provided as:

```text
tmux-easy-motion.tmux
```

On first invocation, if the Rust binary is missing, the plugin automatically:

1. **Download** pre-compiled binary from GitHub release (default: `NaroZeol/tmux-easy-motion`)
2. **Fallback** to local `cargo build --release` if download fails or curl is unavailable

Supported platforms:
- Linux x86_64 and aarch64 (ARM64)
- macOS x86_64 and aarch64 (Apple Silicon)

**For your own fork**, update the `GITHUB_REPO` variable in `scripts/easy_motion.sh`:

```bash
GITHUB_REPO="YOUR_GITHUB_USERNAME/tmux-easy-motion"
```

### Manual setup

If you are not using TPM, add:

```tmux
run-shell /path/to/tmux-easy-motion/easy_motion.tmux
```

Reload tmux config:

```bash
tmux source-file ~/.tmux.conf
```

## Configuration

Supported tmux options:

- `@easy-motion-prefix`
- `@easy-motion-copy-mode-prefix`
- `@easy-motion-dim-style`
- `@easy-motion-highlight-style`
- `@easy-motion-highlight-2-first-style`
- `@easy-motion-highlight-2-second-style`
- `@easy-motion-target-keys`
- `@easy-motion-verbose`
- `@easy-motion-auto-begin-selection`

## Testing

Run tests:

```bash
cargo test
```

The integration-style terminal simulation tests are in `tests/functional_sim_terminal.rs`.

## Project Layout

```text
.
├── Cargo.toml
├── README.md
├── easy_motion.tmux
├── scripts/
│   ├── common_variables.sh
│   ├── easy_motion.sh
│   ├── helpers.sh
│   ├── options.sh
│   └── pipe_target_key.sh
├── src/
│   ├── app.rs
│   ├── config.rs
│   ├── grouping.rs
│   ├── main.rs
│   ├── motion.rs
│   ├── render.rs
│   ├── terminal.rs
│   └── types.rs
└── tests/
	└── functional_sim_terminal.rs
```

### Rust Modules

- `src/main.rs`: thin binary entrypoint and process exit handling
- `src/app.rs`: application flow (argument parsing → motion resolution → key interaction)
- `src/config.rs`: CLI argument parsing and style/color parsing
- `src/motion.rs`: motion regex mapping and text-position conversions
- `src/grouping.rs`: target grouping and jump target generation
- `src/render.rs`: terminal rendering and command-pipe output helpers
- `src/terminal.rs`: raw terminal mode guard (`termios` setup/restore)
- `src/types.rs`: shared data types (`Config`, `GroupedIndices`, `JumpTargetType`)

### tmux Integration Scripts

- `easy_motion.tmux`: plugin entrypoint and key-table bindings
- `scripts/easy_motion.sh`: runtime orchestrator for pane capture, swap pane flow, and jump execution
- `scripts/helpers.sh`: tmux helper functions (cursor read/write, pane ops, pipe paths)
- `scripts/options.sh`: option loading and defaults
- `scripts/pipe_target_key.sh`: target-key pipe writer
- `scripts/common_variables.sh`: shared script constants

## Architecture

1. tmux binding triggers `scripts/easy_motion.sh`.
2. Script captures pane text, prepares FIFOs, and runs the Rust binary.
3. Rust computes jump candidates and paints targets in the temporary view.
4. User key input is streamed through target-key pipe.
5. Rust emits `jump row:col`; script restores original pane and moves the copy-mode cursor.

## Development

Build release binary:

```bash
cargo build --release
```

Run all tests:

```bash
cargo test
```

Validate shell scripts:

```bash
bash -n scripts/*.sh
```

## Publishing a Release

This project uses GitHub Actions to automatically build and publish pre-compiled binaries.

### Steps to release a new version

1. Update version in `Cargo.toml`:

```toml
[package]
version = "0.2.0"
```

2. Create and push a git tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

3. GitHub Actions will automatically:
   - Build binaries for Linux (x86_64, aarch64) and macOS (x86_64, aarch64)
   - Upload them to the GitHub release page

Users will then download these pre-compiled binaries on first plugin invocation.

## Notes

- This repository aims for behavior parity with the original workflow while using a Rust executable for the core logic.
- If you find behavior differences, open an issue with tmux version, reproduction steps, and expected output.

use std::fs::File;
use std::io::{self, Write};

use crate::grouping::generate_jump_targets;
use crate::types::{GroupedIndices, JumpTargetType};

const CLEAR_SCREEN: &str = "\x1b[H\x1b[J";
const STYLE_RESET: &str = "\x1b[0m";
const AUTOWRAP_OFF: &str = "\x1b[?7l";
const AUTOWRAP_ON: &str = "\x1b[?7h";
const CURSOR_HIDE: &str = "\x1b[?25l";

fn previous_char_boundary(text: &str, index: usize) -> usize {
    let mut boundary = index.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn next_char_boundary(text: &str, index: usize) -> Option<usize> {
    let start = index.min(text.len());
    let mut boundary = start;
    while boundary < text.len() && !text.is_char_boundary(boundary) {
        boundary += 1;
    }
    if boundary >= text.len() {
        return None;
    }
    let ch = text[boundary..].chars().next()?;
    Some(boundary + ch.len_utf8())
}

pub(crate) fn print_text_with_targets(
    capture_buffer: &str,
    grouped_indices: &GroupedIndices,
    dim_style: &str,
    highlight_style: &str,
    highlight_2_first_style: &str,
    highlight_2_second_style: &str,
    target_keys: &[char],
    terminal_width: usize,
) -> io::Result<()> {
    let mut jump_targets = generate_jump_targets(grouped_indices, target_keys);
    jump_targets.sort_by_key(|(target_type, pos, _)| {
        let rank = match target_type {
            JumpTargetType::Direct => 0,
            JumpTargetType::Group => 1,
            JumpTargetType::Preview => 2,
        };
        (*pos, rank)
    });

    let mut out = String::new();
    let mut previous_text_end = 0;
    for (target_type, raw_text_pos, target_key) in jump_targets {
        let text_pos = previous_char_boundary(capture_buffer, raw_text_pos);
        if text_pos >= capture_buffer.len() {
            continue;
        }
        let mut append_to_buffer = false;
        let mut append_extra_newline = false;
        if capture_buffer.as_bytes()[text_pos] != b'\n' {
            append_to_buffer = true;
        } else {
            append_extra_newline = true;
            let window_start = previous_char_boundary(capture_buffer, text_pos.saturating_sub(terminal_width + 1));
            let slice = &capture_buffer[window_start..text_pos];
            if slice.rfind('\n').is_some() {
                append_to_buffer = true;
            }
        }

        if append_to_buffer {
            let slice_start = previous_char_boundary(capture_buffer, previous_text_end);
            let slice_end = previous_char_boundary(capture_buffer, text_pos);
            if slice_end > slice_start {
                out.push_str(dim_style);
                out.push_str(&capture_buffer[slice_start..slice_end]);
                out.push_str(STYLE_RESET);
            }
            if slice_end >= slice_start {
                let color = match target_type {
                    JumpTargetType::Direct => highlight_style,
                    JumpTargetType::Group => highlight_2_first_style,
                    JumpTargetType::Preview => highlight_2_second_style,
                };
                out.push_str(color);
                out.push(target_key);
                out.push_str(STYLE_RESET);
            }
        }
        if append_extra_newline {
            out.push('\n');
        }
        previous_text_end = next_char_boundary(capture_buffer, text_pos).unwrap_or(capture_buffer.len());
    }

    let rest = capture_buffer[previous_text_end..].trim_end();
    if !rest.is_empty() {
        out.push_str(dim_style);
        out.push_str(rest);
        out.push_str(STYLE_RESET);
    }

    // Disable autowrap during rendering to prevent double newlines in non-full-width panes,
    // then re-enable it afterward so normal terminal behavior is preserved.
    let mut stdout = io::stdout();
    write!(
        stdout,
        "{}{}{}{}{}",
        CURSOR_HIDE,
        AUTOWRAP_OFF,
        CLEAR_SCREEN,
        out.trim_end(),
        AUTOWRAP_ON
    )?;
    stdout.flush()
}

pub(crate) fn print_ready(command_pipe: &mut File) -> io::Result<()> {
    writeln!(command_pipe, "ready")?;
    command_pipe.flush()
}

pub(crate) fn print_single_target(command_pipe: &mut File) -> io::Result<()> {
    writeln!(command_pipe, "single-target")?;
    command_pipe.flush()
}

pub(crate) fn print_jump_target(row: usize, col: usize, command_pipe: &mut File) -> io::Result<()> {
    writeln!(command_pipe, "jump {}:{}", row, col)?;
    command_pipe.flush()
}

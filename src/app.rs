use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::process::Command;

use crate::config::parse_arguments;
use crate::grouping::group_indices;
use crate::motion::{convert_row_col_to_text_pos, convert_text_pos_to_row_col, motion_to_indices};
use crate::render::{position_cursor, print_jump_target, print_ready, print_single_target, print_text_with_targets};
use crate::terminal::TerminalGuard;
use crate::types::{Config, GroupedIndices};

fn read_capture_buffer(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

fn descend_group<'a>(
    grouped: &'a GroupedIndices,
    key_index: usize,
) -> Result<&'a GroupedIndices, String> {
    match grouped {
        GroupedIndices::Leaf(_) => Err("The key is no valid target.".to_string()),
        GroupedIndices::Group(groups) => groups
            .get(key_index)
            .ok_or_else(|| "The key is no valid target.".to_string()),
    }
}

fn handle_user_input(config: &Config) -> Result<(), String> {
    let fd = 0;
    let _guard = TerminalGuard::setup(fd)?;

    let capture_buffer = read_capture_buffer(&config.capture_buffer_filepath)?;
    let (row, col) = config.cursor_position;
    let (pane_width, _) = config.pane_size;
    let cursor_position = convert_row_col_to_text_pos(row, col, &capture_buffer);
    let target_keys: Vec<char> = config.target_keys.chars().collect();

    let mut command_pipe = OpenOptions::new()
        .write(true)
        .open(&config.command_pipe_filepath)
        .map_err(|e| e.to_string())?;

    let mut current_group = group_indices(
        &motion_to_indices(
            cursor_position,
            &capture_buffer,
            &config.motion,
            config.motion_argument.as_deref(),
        )?,
        target_keys.len(),
    );

    let mut first_highlight = true;
    loop {
        let grouped = match current_group.clone() {
            Some(grouped) => grouped,
            None => break,
        };

        match grouped {
            GroupedIndices::Leaf(found_index) => {
                if first_highlight {
                    print_single_target(&mut command_pipe).map_err(|e| e.to_string())?;
                }
                let (target_row, target_col) = convert_text_pos_to_row_col(found_index, &capture_buffer)?;
                print_jump_target(target_row, target_col, &mut command_pipe)
                    .map_err(|e| e.to_string())?;
                break;
            }
            GroupedIndices::Group(_) => {
                print_text_with_targets(
                    &capture_buffer,
                    &grouped,
                    &config.dim_style_code,
                    &config.highlight_style_code,
                    &config.highlight_2_first_style_code,
                    &config.highlight_2_second_style_code,
                    &target_keys,
                    pane_width,
                )
                .map_err(|e| e.to_string())?;
                position_cursor(row, col).map_err(|e| e.to_string())?;
                if first_highlight {
                    print_ready(&mut command_pipe).map_err(|e| e.to_string())?;
                    first_highlight = false;
                }
            }
        }

        let reader = File::open(&config.target_key_pipe_filepath).map_err(|e| e.to_string())?;
        let mut line = String::new();
        let mut buffered = BufReader::new(reader);
        buffered.read_line(&mut line).map_err(|e| e.to_string())?;
        let next_key = line.trim_end_matches(['\n', '\r']);
        if next_key == "esc" {
            break;
        }
        if next_key.chars().count() != 1 {
            return Err(format!("The key \"{}\" is no valid target.", next_key));
        }
        let key_char = next_key.chars().next().ok_or("invalid key")?;
        let target_index = target_keys
            .iter()
            .position(|c| *c == key_char)
            .ok_or_else(|| format!("The key \"{}\" is no valid target.", key_char))?;

        let group_ref = current_group.as_ref().ok_or("No targets")?;
        current_group = Some(descend_group(group_ref, target_index)?.clone());
    }

    Ok(())
}

fn display_tmux_message(message: &str) {
    let status = Command::new("tmux")
        .arg("display-message")
        .arg(message)
        .status();
    if status.is_err() {
        let _ = writeln!(io::stderr(), "Error: {}", message);
    }
}

pub(crate) fn run() -> Result<(), String> {
    let config = parse_arguments()?;
    handle_user_input(&config)
}

pub(crate) fn run_with_tmux_error_display() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(error_message) => {
            display_tmux_message(&format!("Error: {}", error_message));
            1
        }
    }
}

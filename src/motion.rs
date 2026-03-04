use regex::Regex;
use std::collections::HashSet;

fn forward_motions() -> HashSet<&'static str> {
    [
        "e", "E", "w", "W", "j", "J", "f", "t", "bd-w", "bd-W", "bd-e", "bd-E", "bd-j", "bd-J",
        "bd-f", "bd-f2", "bd-t", "bd-T", "c",
    ]
    .into_iter()
    .collect()
}

fn backward_motions() -> HashSet<&'static str> {
    [
        "b", "B", "ge", "gE", "k", "K", "F", "T", "bd-w", "bd-W", "bd-e", "bd-E", "bd-j", "bd-J",
        "bd-f", "bd-f2", "bd-t", "bd-T", "c",
    ]
    .into_iter()
    .collect()
}

fn linewise_motions() -> HashSet<&'static str> {
    ["j", "J", "k", "K", "bd-j", "bd-J"].into_iter().collect()
}

fn motion_regex_template(motion: &str) -> Option<&'static str> {
    match motion {
        "b" => Some(r"\b(\w)"),
        "B" => Some(r"(?:^|\s)(\S)"),
        "ge" => Some(r"(\w)\b"),
        "gE" => Some(r"(\S)(?:\s|$)"),
        "e" => Some(r"(\w)\b"),
        "E" => Some(r"(\S)(?:\s|$)"),
        "w" => Some(r"\b(\w)"),
        "W" => Some(r"(?:^|\s)(\S)"),
        "j" => Some(r"^(?:\s*)(\S)"),
        "J" => Some(r"(\S)(?:\s*)$"),
        "k" => Some(r"^(?:\s*)(\S)"),
        "K" => Some(r"(\S)(?:\s*)$"),
        "f" => Some(r"({})"),
        "F" => Some(r"({})"),
        "t" => Some(r"(.){}"),
        "T" => Some(r"{}(.)"),
        "bd-w" => Some(r"\b(\w)"),
        "bd-W" => Some(r"(?:^|\s)(\S)"),
        "bd-e" => Some(r"(\w)\b"),
        "bd-E" => Some(r"(\S)(?:\s|$)"),
        "bd-j" => Some(r"^(?:\s*)(\S)"),
        "bd-J" => Some(r"(\S)(?:\s*)$"),
        "bd-f" => Some(r"({})"),
        "bd-f2" => Some(r"({})"),
        "bd-t" => Some(r"(.){}"),
        "bd-T" => Some(r"{}(.)"),
        "c" => Some(r"(?:_(\w))|(?:[a-z]([A-Z]))"),
        _ => None,
    }
}

pub(crate) fn convert_row_col_to_text_pos(row: usize, col: usize, text: &str) -> usize {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.is_empty() {
        return 0;
    }
    let row = row.min(lines.len() - 1);
    let row_line = lines[row];
    
    // Convert character column to byte offset
    let char_col = col.min(row_line.chars().count().saturating_sub(1));
    let byte_col = row_line
        .char_indices()
        .nth(char_col)
        .map(|(i, _)| i)
        .unwrap_or(row_line.len());
    
    // Calculate byte position: sum of all previous lines (including newlines) + column offset
    let mut pos = 0;
    for line in lines.iter().take(row) {
        pos += line.len() + 1; // +1 for newline
    }
    pos + byte_col
}

pub(crate) fn convert_text_pos_to_row_col(text_pos: usize, text: &str) -> Result<(usize, usize), String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut current = 0;
    for (row, line) in lines.iter().enumerate() {
        if current + line.len() >= text_pos {
            let byte_col = text_pos - current;
            // Convert byte offset back to character column
            let char_col = line[..byte_col]
                .chars()
                .count();
            return Ok((row, char_col));
        }
        current += line.len() + 1; // +1 for newline
    }
    Err(format!(
        "The text position \"{}\" is out of range.",
        text_pos
    ))
}

fn find_first_line_end(cursor_pos: usize, text: &str) -> usize {
    text[cursor_pos..]
        .find('\n')
        .unwrap_or(text.len() - cursor_pos)
}

fn find_latest_line_start(cursor_pos: usize, text: &str) -> usize {
    text[..=cursor_pos]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn adjust_text(
    cursor_pos: usize,
    text: &str,
    is_forward_motion: bool,
    motion: &str,
) -> (String, isize) {
    let mut offset: isize = 0;
    let linewise = linewise_motions().contains(motion);
    let adjusted = if is_forward_motion {
        if linewise {
            let first_end = find_first_line_end(cursor_pos, text);
            offset = (cursor_pos + first_end) as isize;
            text[cursor_pos + first_end..].to_string()
        } else {
            offset = cursor_pos as isize;
            format!("{} ", &text[cursor_pos..])
        }
    } else if linewise {
        let latest_start = find_latest_line_start(cursor_pos, text);
        text[..latest_start].to_string()
    } else {
        offset = -1;
        format!(" {}", &text[..=cursor_pos])
    };
    (adjusted, offset)
}

pub(crate) fn motion_to_indices(
    cursor_pos: usize,
    text: &str,
    motion: &str,
    motion_argument: Option<&str>,
) -> Result<Vec<usize>, String> {
    let forward = forward_motions();
    let backward = backward_motions();

    if forward.contains(motion) && backward.contains(motion) {
        let mut result = Vec::new();
        let forward_indices =
            motion_to_indices(cursor_pos, text, &format!("{}>", motion), motion_argument)?;
        let backward_indices =
            motion_to_indices(cursor_pos, text, &format!("{}<", motion), motion_argument)?;
        let mut fi = 0;
        let mut bi = 0;
        while fi < forward_indices.len() || bi < backward_indices.len() {
            if fi < forward_indices.len() {
                result.push(forward_indices[fi]);
                fi += 1;
            }
            if bi < backward_indices.len() {
                result.push(backward_indices[bi]);
                bi += 1;
            }
        }
        return Ok(result);
    }

    let mut base_motion = motion.to_string();
    let is_forward = if motion.ends_with('>') {
        base_motion.pop();
        true
    } else if motion.ends_with('<') {
        base_motion.pop();
        false
    } else {
        forward.contains(motion)
    };

    let (adjusted, offset) = adjust_text(cursor_pos, text, is_forward, &base_motion);
    let template =
        motion_regex_template(&base_motion).ok_or_else(|| "invalid motion".to_string())?;
    let pattern = if let Some(argument) = motion_argument {
        template.replacen("{}", &regex::escape(argument), 1)
    } else {
        template.to_string()
    };
    let regex = Regex::new(&pattern).map_err(|e| e.to_string())?;
    let mut caps: Vec<_> = regex.captures_iter(&adjusted).collect();
    if !is_forward {
        caps.reverse();
    }

    let linewise = linewise_motions().contains(base_motion.as_str());
    let mut result = Vec::new();
    for capture in caps {
        for group in 1..capture.len() {
            if let Some(m) = capture.get(group) {
                let start = m.start();
                if linewise || (start > 0 && start < adjusted.len().saturating_sub(1)) {
                    let adjusted_index = (start as isize + offset) as usize;
                    result.push(adjusted_index);
                }
            }
        }
    }
    Ok(result)
}

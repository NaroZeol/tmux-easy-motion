use regex::{Regex, RegexBuilder};
use std::collections::HashSet;
use unicode_width::UnicodeWidthChar;

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
    
    // Convert display column to byte offset
    // tmux reports cursor position in display columns (considering character width)
    let byte_col = if row_line.is_empty() {
        0
    } else {
        // Find the character at the given display column
        let mut display_col = 0;
        let mut byte_offset = 0;
        let mut found = false;
        
        for (byte_idx, ch) in row_line.char_indices() {
            // Get the display width of this character (1 for most, 2 for CJK/emoji, 0 for combining)
            let char_width = ch.width().unwrap_or(1);
            
            // Check if target column falls within this character's display range
            // For example, if emoji at display_col 6 has width 2, it occupies columns 6 and 7
            if col >= display_col && col < display_col + char_width {
                byte_offset = byte_idx;
                found = true;
                break;
            }
            
            display_col += char_width;
        }
        
        // If we've processed all chars and haven't found the column, clamp to end of line
        if !found {
            row_line.len()
        } else {
            byte_offset
        }
    };
    
    // Calculate byte position: sum of all previous lines (including newlines) + column offset
    let mut pos = 0;
    for line in lines.iter().take(row) {
        pos += line.len() + 1; // +1 for newline
    }
    let result_pos = pos + byte_col;
    
    // Ensure the result is at a valid UTF-8 character boundary
    let valid_pos = result_pos.min(text.len());
    if text.is_char_boundary(valid_pos) {
        valid_pos
    } else {
        // This should rarely happen now, but keep as safety net
        let mut adj_pos = valid_pos;
        while adj_pos > 0 && !text.is_char_boundary(adj_pos) {
            adj_pos -= 1;
        }
        adj_pos
    }
}

pub(crate) fn convert_text_pos_to_row_col(text_pos: usize, text: &str) -> Result<(usize, usize), String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut current = 0;
    for (row, line) in lines.iter().enumerate() {
        if current + line.len() >= text_pos {
            let byte_col = text_pos - current;
            
            // Find the nearest valid UTF-8 boundary if byte_col is in the middle of a character
            let mut valid_byte_col = byte_col;
            while valid_byte_col > 0 && !is_char_boundary(line, valid_byte_col) {
                valid_byte_col -= 1;
            }
            
            // Convert byte offset to display column (accounting for character width)
            let mut display_col = 0;
            for ch in line[..valid_byte_col].chars() {
                display_col += ch.width().unwrap_or(1);
            }
            return Ok((row, display_col));
        }
        current += line.len() + 1; // +1 for newline
    }
    Err(format!(
        "The text position \"{}\" is out of range.",
        text_pos
    ))
}

fn is_char_boundary(s: &str, index: usize) -> bool {
    if index > s.len() {
        return false;
    }
    if index == 0 || index == s.len() {
        return true;
    }
    s.is_char_boundary(index)
}

fn find_first_line_end(cursor_pos: usize, text: &str) -> usize {
    // cursor_pos is guaranteed to be at a valid boundary from convert_row_col_to_text_pos
    if cursor_pos >= text.len() {
        return 0;
    }
    
    text[cursor_pos..]
        .find('\n')
        .unwrap_or(text.len() - cursor_pos)
}

fn find_latest_line_start(cursor_pos: usize, text: &str) -> usize {
    // cursor_pos is guaranteed to be at a valid boundary from convert_row_col_to_text_pos
    if cursor_pos >= text.len() {
        return text.rfind('\n').map(|i| i + 1).unwrap_or(0);
    }
    
    // Find the next character boundary to avoid splitting multi-byte chars
    let mut end_pos = cursor_pos + 1;
    while end_pos < text.len() && !text.is_char_boundary(end_pos) {
        end_pos += 1;
    }
    
    text[..end_pos]
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
    // cursor_pos is guaranteed to be at a valid UTF-8 boundary from convert_row_col_to_text_pos
    let safe_pos = cursor_pos.min(text.len());
    
    let mut offset: isize = 0;
    let linewise = linewise_motions().contains(motion);
    let adjusted = if is_forward_motion {
        if linewise {
            let first_end = find_first_line_end(safe_pos, text);
            offset = (safe_pos + first_end) as isize;
            text[safe_pos + first_end..].to_string()
        } else {
            offset = safe_pos as isize;
            if safe_pos < text.len() {
                format!("{} ", &text[safe_pos..])
            } else {
                " ".to_string()
            }
        }
    } else if linewise {
        let latest_start = find_latest_line_start(safe_pos, text);
        text[..latest_start].to_string()
    } else {
        offset = -1;
        if safe_pos < text.len() {
            // For backward motion, we need to include the character at cursor position
            // Find the next character boundary after safe_pos to avoid splitting multi-byte chars
            let mut end_pos = safe_pos + 1;
            while end_pos < text.len() && !text.is_char_boundary(end_pos) {
                end_pos += 1;
            }
            format!(" {}", &text[..end_pos])
        } else if !text.is_empty() {
            format!(" {}", text)
        } else {
            " ".to_string()
        }
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
    
    // Enable multi_line mode for linewise motions (j, k, J, K, bd-j, bd-J)
    // so that ^ and $ match line boundaries instead of just string boundaries
    let regex = if linewise_motions().contains(base_motion.as_str()) {
        RegexBuilder::new(&pattern)
            .multi_line(true)
            .build()
            .map_err(|e| e.to_string())?
    } else {
        Regex::new(&pattern).map_err(|e| e.to_string())?
    };
    
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

use std::collections::HashSet;
use std::env;

use crate::types::Config;

fn valid_motions() -> HashSet<&'static str> {
    [
        "b", "B", "ge", "gE", "e", "E", "w", "W", "j", "J", "k", "K", "f", "F", "t", "T", "bd-w",
        "bd-W", "bd-e", "bd-E", "bd-j", "bd-J", "bd-f", "bd-f2", "bd-t", "bd-T", "c",
    ]
    .into_iter()
    .collect()
}

fn motions_with_argument() -> HashSet<&'static str> {
    ["f", "F", "t", "T", "bd-f", "bd-f2", "bd-t", "bd-T"]
        .into_iter()
        .collect()
}

pub(crate) fn parse_arguments() -> Result<Config, String> {
    let mut argv: Vec<String> = env::args().collect();
    argv.remove(0);

    let dim_style = argv.get(0).ok_or("No dim style given.")?.clone();
    let dim_style_code =
        parse_style(&dim_style).map_err(|_| format!("\"{}\" is not a valid style.", dim_style))?;
    argv.remove(0);

    let highlight_style = argv.get(0).ok_or("No highlight style given.")?.clone();
    let highlight_style_code = parse_style(&highlight_style)
        .map_err(|_| format!("\"{}\" is not a valid style.", highlight_style))?;
    argv.remove(0);

    let h2_first_style = argv
        .get(0)
        .ok_or("No highlight 2 first style given.")?
        .clone();
    let highlight_2_first_style_code = parse_style(&h2_first_style)
        .map_err(|_| format!("\"{}\" is not a valid style.", h2_first_style))?;
    argv.remove(0);

    let h2_second_style = argv
        .get(0)
        .ok_or("No highlight 2 second style given.")?
        .clone();
    let highlight_2_second_style_code = parse_style(&h2_second_style)
        .map_err(|_| format!("\"{}\" is not a valid style.", h2_second_style))?;
    argv.remove(0);

    let motion = argv.get(0).ok_or("No motion given.")?.clone();
    if !valid_motions().contains(motion.as_str()) {
        return Err(format!(
            "The string \"{}\" is not in a valid motion.",
            motion
        ));
    }
    argv.remove(0);

    let motion_arg_raw = argv.get(0).ok_or("No motion argument given.")?.clone();
    let motion_argument = if motions_with_argument().contains(motion.as_str()) {
        Some(motion_arg_raw)
    } else {
        None
    };
    argv.remove(0);

    let target_keys = argv.get(0).ok_or("No target keys given.")?.clone();
    if target_keys.chars().count() < 2 {
        return Err("At least two target keys are needed.".to_string());
    }
    argv.remove(0);

    let cursor_position_raw = argv.get(0).ok_or("No cursor position given.")?.clone();
    let cursor_position = parse_pair(&cursor_position_raw, "cursor position", "<row>:<col>")?;
    argv.remove(0);

    let pane_size_raw = argv.get(0).ok_or("No pane size given.")?.clone();
    let pane_size = parse_pair(&pane_size_raw, "pane size", "<width>:<height>")?;
    argv.remove(0);

    let capture_buffer_filepath = argv
        .get(0)
        .ok_or("No tmux capture buffer filepath given.")?
        .clone();
    argv.remove(0);

    let command_pipe_filepath = argv
        .get(0)
        .ok_or("No jump command pipe filepath given.")?
        .clone();
    argv.remove(0);

    let target_key_pipe_filepath = argv
        .get(0)
        .ok_or("No target key pipe filepath given.")?
        .clone();

    Ok(Config {
        dim_style_code,
        highlight_style_code,
        highlight_2_first_style_code,
        highlight_2_second_style_code,
        motion,
        motion_argument,
        target_keys,
        cursor_position,
        pane_size,
        capture_buffer_filepath,
        command_pipe_filepath,
        target_key_pipe_filepath,
    })
}

fn parse_pair(raw: &str, label: &str, format: &str) -> Result<(usize, usize), String> {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 2 {
        return Err(format!(
            "The {} \"{}\" is not in the format \"{}\".",
            label, raw, format
        ));
    }
    let left = parts[0].trim().parse::<usize>().map_err(|_| {
        format!(
            "The {} \"{}\" is not in the format \"{}\".",
            label, raw, format
        )
    })?;
    let right = parts[1].trim().parse::<usize>().map_err(|_| {
        format!(
            "The {} \"{}\" is not in the format \"{}\".",
            label, raw, format
        )
    })?;
    Ok((left, right))
}

fn parse_style(style: &str) -> Result<String, String> {
    let mut codes = String::new();
    for part in style
        .to_lowercase()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        let code = if let Some(value) = part.strip_prefix("fg=") {
            color_to_code(value, false)?
        } else if let Some(value) = part.strip_prefix("bg=") {
            color_to_code(value, true)?
        } else {
            match part {
                "none" => "\x1b[0m".to_string(),
                "bold" | "bright" => "\x1b[1m".to_string(),
                "dim" => "\x1b[2m".to_string(),
                "italics" => "\x1b[3m".to_string(),
                "underscore" => "\x1b[4m".to_string(),
                "blink" => "\x1b[5m".to_string(),
                "reverse" => "\x1b[7m".to_string(),
                "hidden" => "\x1b[8m".to_string(),
                "overline" => "\x1b[53m".to_string(),
                "double-underscore" => "\x1b[4:2m".to_string(),
                "curly-underscore" => "\x1b[4:3m".to_string(),
                "dotted-underscore" => "\x1b[4:4m".to_string(),
                "dashed-underscore" => "\x1b[4:5m".to_string(),
                _ => return Err(format!("\"{}\" is not a valid style.", style)),
            }
        };
        codes.push_str(&code);
    }
    Ok(codes)
}

fn color_to_code(color: &str, bg: bool) -> Result<String, String> {
    let color = color.trim().to_lowercase();
    if let Some(index) = color
        .strip_prefix("colour")
        .or_else(|| color.strip_prefix("color"))
    {
        let index = index.parse::<u16>().map_err(|_| "invalid color")?;
        if index > 255 {
            return Err("invalid color".to_string());
        }
        return Ok(format!("\x1b[{};5;{}m", if bg { 48 } else { 38 }, index));
    }

    let hex_match = regex::Regex::new(r"^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$").map_err(|_| "regex")?;
    if let Some(caps) = hex_match.captures(&color) {
        let r = u8::from_str_radix(&caps[1], 16).map_err(|_| "hex")?;
        let g = u8::from_str_radix(&caps[2], 16).map_err(|_| "hex")?;
        let b = u8::from_str_radix(&caps[3], 16).map_err(|_| "hex")?;
        return Ok(format!(
            "\x1b[{};2;{};{};{}m",
            if bg { 48 } else { 38 },
            r,
            g,
            b
        ));
    }

    let name_to_index = [
        ("black", 0_u16),
        ("red", 1),
        ("green", 2),
        ("yellow", 3),
        ("blue", 4),
        ("magenta", 5),
        ("cyan", 6),
        ("white", 7),
        ("brightblack", 8),
        ("brightred", 9),
        ("brightgreen", 10),
        ("brightyellow", 11),
        ("brightblue", 12),
        ("brightmagenta", 13),
        ("brightcyan", 14),
        ("brightwhite", 15),
    ];
    let (_, idx) = name_to_index
        .iter()
        .find(|(name, _)| *name == color)
        .ok_or_else(|| "invalid color".to_string())?;

    let mut ansi = 30 + *idx as i32;
    if *idx > 7 {
        ansi += 52;
    }
    if bg {
        ansi += 10;
    }
    Ok(format!("\x1b[{}m", ansi))
}

#[cfg(test)]
mod tests {
    use super::parse_pair;

    #[test]
    fn parse_pair_accepts_whitespace_around_numbers() {
        assert_eq!(parse_pair("150:           35", "pane size", "<width>:<height>"), Ok((150, 35)));
        assert_eq!(parse_pair(" 3 : 7 ", "cursor position", "<row>:<col>"), Ok((3, 7)));
    }
}

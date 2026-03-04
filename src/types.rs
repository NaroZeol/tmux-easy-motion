#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JumpTargetType {
    Direct,
    Group,
    Preview,
}

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) dim_style_code: String,
    pub(crate) highlight_style_code: String,
    pub(crate) highlight_2_first_style_code: String,
    pub(crate) highlight_2_second_style_code: String,
    pub(crate) motion: String,
    pub(crate) motion_argument: Option<String>,
    pub(crate) target_keys: String,
    pub(crate) cursor_position: (usize, usize),
    pub(crate) pane_size: (usize, usize),
    pub(crate) capture_buffer_filepath: String,
    pub(crate) command_pipe_filepath: String,
    pub(crate) target_key_pipe_filepath: String,
}

#[derive(Clone, Debug)]
pub(crate) enum GroupedIndices {
    Leaf(usize),
    Group(Vec<GroupedIndices>),
}

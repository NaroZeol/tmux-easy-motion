#!/usr/bin/env bash

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="${CURRENT_DIR}"

# shellcheck disable=SC1091
source "${SCRIPTS_DIR}/common_variables.sh"

get_target_key_pipe_parent_directory() {
    local server_pid
    server_pid="$1"
    echo "${TMPDIR}/tmux-easy-motion-target-key-pipe_$(id -un)_${server_pid}"
}

get_target_key_pipe_tmp_directory() {
    local server_pid session_id create parent_dir target_key_pipe_tmp_directory
    server_pid="$1"
    session_id="$2"
    if [[ "${session_id}" =~ \$(.*) ]]; then
        session_id="${BASH_REMATCH[1]}"
    fi
    create=0
    if [[ "$3" == "create" ]]; then
        create=1
    fi

    parent_dir="$(get_target_key_pipe_parent_directory "${server_pid}")"
    target_key_pipe_tmp_directory="${parent_dir}/${session_id}"
    if (( create )); then
        mkdir -p "${target_key_pipe_tmp_directory}" || return 1
        chmod 700 "${target_key_pipe_tmp_directory}" || return 1
    fi
    echo "${target_key_pipe_tmp_directory}"
}

ensure_target_key_pipe_exists() {
    local server_pid session_id target_key_pipe_tmp_directory
    server_pid="$1"
    session_id="$2"
    target_key_pipe_tmp_directory="$(get_target_key_pipe_tmp_directory "${server_pid}" "${session_id}" create)" || return 1
    if [[ -e "${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}" && ! -p "${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}" ]]; then
        rm -f "${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}" || return 1
    fi
    if [[ ! -p "${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}" ]]; then
        mkfifo "${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}"
    fi
}

reset_target_key_pipe() {
    local server_pid session_id target_key_pipe_tmp_directory target_key_pipe
    server_pid="$1"
    session_id="$2"

    target_key_pipe_tmp_directory="$(get_target_key_pipe_tmp_directory "${server_pid}" "${session_id}" create)" || return 1
    target_key_pipe="${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}"

    rm -f "${target_key_pipe}" || return 1
    mkfifo "${target_key_pipe}" || return 1
}

get_tmux_server_pid() {
    [[ "${TMUX}" =~ .*,(.*),.* ]] && echo "${BASH_REMATCH[1]}"
}

get_pane_size() {
    local pane_id
    pane_id="$1"
    tmux display-message -p -t "${pane_id}" "#{pane_width}:#{pane_height}"
}

is_pane_zoomed() {
    local pane_id
    pane_id="$1"
    [[ "$(tmux display-message -p -t "${pane_id}" "#{window_zoomed_flag}")" == "1" ]]
}

swap_pane() {
    local target_pane_id source_pane_id
    target_pane_id="$1"
    source_pane_id="$2"
    tmux swap-pane -s "${source_pane_id}" -t "${target_pane_id}"
}

create_empty_swap_pane() {
    local session_id pane_id
    local swap_pane_id swap_window_id
    
    session_id="$1"
    pane_id="$2"
    
    # Create new window with a silent placeholder process
    swap_window_id=$(tmux new-window -t "${session_id}" -P -d -F "#{window_id}" -n "[easy-motion]" "tail -f /dev/null")
    swap_pane_id=$(tmux list-panes -t "${swap_window_id}" -F "#{pane_id}" | head -1)
    
    echo "${swap_window_id}:${swap_pane_id}"
}

read_cursor_position() {
    local pane_id in_copy_mode
    pane_id="$1"
    # Check if pane is in copy-mode
    in_copy_mode="$(tmux display-message -p -t "${pane_id}" "#{pane_in_mode}")"
    if [[ "${in_copy_mode}" == "1" ]]; then
        # In copy-mode, read copy-mode cursor position
        tmux display-message -p -t "${pane_id}" "#{copy_cursor_y}:#{copy_cursor_x}"
    else
        # In normal mode, read normal cursor position
        tmux display-message -p -t "${pane_id}" "#{cursor_y}:#{cursor_x}"
    fi
}

set_cursor_position() {
    local pane_id row_col row col current_row current_col rel_row rel_col
    pane_id="$1"
    row_col="$2"
    IFS=':' read -r row col <<< "${row_col}"
    IFS=':' read -r current_row current_col <<< "$(read_cursor_position "${pane_id}")"
    rel_row="$(( row - current_row ))"
    
    if (( rel_row < 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( -rel_row ))" cursor-up
    elif (( rel_row > 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( rel_row ))" cursor-down
    fi
    IFS=':' read -r current_row current_col <<< "$(read_cursor_position "${pane_id}")"
    rel_col="$(( col - current_col ))"
    if (( rel_col < 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( -rel_col ))" cursor-left
    elif (( rel_col > 0 )); then
        tmux send-keys -t "${pane_id}" -X -N "$(( rel_col ))" cursor-right
    fi
}

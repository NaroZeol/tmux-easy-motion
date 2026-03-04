#!/usr/bin/env bash

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="${CURRENT_DIR}"
ROOT_DIR="$(cd "${SCRIPTS_DIR}/.." && pwd)"
BINARY_PATH="${ROOT_DIR}/target/release/tmux-easy-motion"

CAPTURE_PANE_FILENAME="capture.out"
JUMP_COMMAND_PIPENAME="jump.pipe"

# GitHub release download settings (replace OWNER/REPO with your fork)
GITHUB_REPO="NaroZeol/tmux-easy-motion"
GITHUB_RELEASE_API="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"

# shellcheck disable=SC1091
source "${SCRIPTS_DIR}/common_variables.sh"
# shellcheck disable=SC1091
source "${SCRIPTS_DIR}/helpers.sh"
# shellcheck disable=SC1091
source "${SCRIPTS_DIR}/options.sh"

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Linux)
            case "${arch}" in
                x86_64) echo "linux-x86_64" ;;
                aarch64) echo "linux-aarch64" ;;
                *) return 1 ;;
            esac
            ;;
        Darwin)
            case "${arch}" in
                x86_64) echo "macos-x86_64" ;;
                arm64) echo "macos-aarch64" ;;
                *) return 1 ;;
            esac
            ;;
        *) return 1 ;;
    esac
}

download_binary() {
    local platform binary_url
    platform="$(detect_platform)" || return 1

    # Query latest release and find matching binary asset
    if ! command -v curl >/dev/null 2>&1; then
        return 1
    fi

    binary_url=$(curl -s "${GITHUB_RELEASE_API}" | grep -o "\"browser_download_url\": \"[^\"]*tmux-easy-motion-${platform}\"" | head -1 | grep -o "https[^\"]*")
    
    if [[ -z "${binary_url}" ]]; then
        return 1
    fi

    mkdir -p "${ROOT_DIR}/target/release" || return 1
    
    if curl -L -f -o "${BINARY_PATH}" "${binary_url}" 2>/dev/null; then
        chmod +x "${BINARY_PATH}"
        return 0
    fi
    
    return 1
}

build_binary_locally() {
    if ! command -v cargo >/dev/null 2>&1; then
        return 1
    fi

    if (cd "${ROOT_DIR}" && cargo build --release -q 2>/dev/null); then
        return 0
    fi

    return 1
}

ensure_binary_exists() {
    if [[ -x "${BINARY_PATH}" ]]; then
        return 0
    fi

    # Try downloading from GitHub release first
    if download_binary; then
        return 0
    fi

    # Fallback to local build
    if build_binary_locally; then
        if [[ -x "${BINARY_PATH}" ]]; then
            return 0
        fi
    fi

    # Both attempts failed
    tmux display-message "tmux-easy-motion: unable to obtain binary (GitHub download failed, local build unavailable)"
    return 1
}

main() {
    local server_pid session_id window_id pane_id motion motion_argument
    local capture_tmp_directory capture_file jump_pipe target_key_pipe_tmp_directory target_key_pipe
    local cursor_pos pane_size ready_command jump_command jump_cursor_position
    local swap_window_id swap_pane_id swap_ids

    server_pid="$1"
    session_id="$2"
    window_id="$3"
    pane_id="$4"
    motion="$5"
    motion_argument="$6"

    ensure_binary_exists || return 1
    read_options || return 1

    capture_tmp_directory="$(mktemp -d)" || return 1
    trap 'rm -rf "${capture_tmp_directory}"' EXIT

    capture_file="${capture_tmp_directory}/${CAPTURE_PANE_FILENAME}"
    jump_pipe="${capture_tmp_directory}/${JUMP_COMMAND_PIPENAME}"

    # Capture pane content and setup before entering copy-mode
    tmux capture-pane -t "${pane_id}" -p > "${capture_file}" || return 1
    mkfifo "${jump_pipe}" || return 1

    # Enter copy-mode on original pane
    tmux copy-mode -t "${pane_id}" || return 1
    
    cursor_pos="$(read_cursor_position "${pane_id}")"
    pane_size="$(tmux display-message -p -t "${pane_id}" "#{pane_width}:#{pane_height}")"

    # Create swap pane for showing selection interface
    swap_ids="$(create_empty_swap_pane "${session_id}" "${window_id}" "${pane_id}")" || return 1
    swap_window_id="$(cut -d: -f1 <<< "${swap_ids}")"
    swap_pane_id="$(cut -d: -f2 <<< "${swap_ids}")"
    
    ensure_target_key_pipe_exists "${server_pid}" "${session_id}" || return 1
    target_key_pipe_tmp_directory="$(get_target_key_pipe_tmp_directory "${server_pid}" "${session_id}")" || return 1
    target_key_pipe="${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}"

    # Swap to swap_pane and set key table
    tmux swap-pane -s "${swap_pane_id}" -t "${pane_id}"
    tmux set-window-option -t "${swap_pane_id}" key-table easy-motion-target
    tmux switch-client -t "${swap_pane_id}" -T easy-motion-target

    # Get pane_tty from swap_pane (which now displays on screen)
    local pane_tty
    pane_tty="$(tmux display-message -p -t "${swap_pane_id}" "#{pane_tty}")"

    # Run Rust program in swap pane
    "${BINARY_PATH}" \
        "${EASY_MOTION_DIM_STYLE}" \
        "${EASY_MOTION_HIGHLIGHT_STYLE}" \
        "${EASY_MOTION_HIGHLIGHT_2_FIRST_STYLE}" \
        "${EASY_MOTION_HIGHLIGHT_2_SECOND_STYLE}" \
        "${motion}" \
        "${motion_argument}" \
        "${EASY_MOTION_TARGET_KEYS}" \
        "${cursor_pos}" \
        "${pane_size}" \
        "${capture_file}" \
        "${jump_pipe}" \
        "${target_key_pipe}" \
        < /dev/null > "${pane_tty}" 2>/dev/null &

    {
        read -r ready_command || {
            # User cancelled, swap back without jumping
            tmux set-window-option -t "${swap_pane_id}" key-table root
            tmux switch-client -t "${swap_pane_id}" -T root
            tmux swap-pane -s "${swap_pane_id}" -t "${pane_id}"
            tmux kill-window -t "${swap_window_id}"
            return 0
        }
        if [[ "${ready_command}" != "ready" && "${ready_command}" != "single-target" ]]; then
            tmux set-window-option -t "${swap_pane_id}" key-table root
            tmux switch-client -t "${swap_pane_id}" -T root
            tmux swap-pane -s "${swap_pane_id}" -t "${pane_id}"
            tmux kill-window -t "${swap_window_id}"
            return 0
        fi
        
        read -r jump_command || {
            # User cancelled at selection, swap back
            tmux set-window-option -t "${swap_pane_id}" key-table root
            tmux switch-client -t "${swap_pane_id}" -T root
            tmux swap-pane -s "${swap_pane_id}" -t "${pane_id}"
            tmux kill-window -t "${swap_window_id}"
            return 0
        }
        
        if [[ "$(awk '{ print $1 }' <<< "${jump_command}")" != "jump" ]]; then
            tmux set-window-option -t "${swap_pane_id}" key-table root
            tmux switch-client -t "${swap_pane_id}" -T root
            tmux swap-pane -s "${swap_pane_id}" -t "${pane_id}"
            tmux kill-window -t "${swap_window_id}"
            return 0
        fi
        
        jump_cursor_position="$(awk '{ print $2 }' <<< "${jump_command}")"
        
        # Swap back to original pane (which is still in copy-mode)
        tmux set-window-option -t "${swap_pane_id}" key-table root
        tmux switch-client -t "${pane_id}" -T root
        tmux swap-pane -s "${swap_pane_id}" -t "${pane_id}"
        
        # Now set cursor position (pane_id is back to original pane in copy-mode)
        set_cursor_position "${pane_id}" "${jump_cursor_position}"
        
        # Auto-begin selection if configured
        if [[ "${EASY_MOTION_AUTO_BEGIN_SELECTION}" == "1" || "${EASY_MOTION_AUTO_BEGIN_SELECTION}" == "true" ]]; then
            tmux if -F "#{?selection_present,0,1}" "send-keys -t ${pane_id} -X begin-selection"
        fi
        
        # Kill the swap window
        tmux kill-window -t "${swap_window_id}"
    } < "${jump_pipe}"
}

main "$@"

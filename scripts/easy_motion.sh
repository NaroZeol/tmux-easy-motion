#!/usr/bin/env bash

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="${CURRENT_DIR}"
ROOT_DIR="$(cd "${SCRIPTS_DIR}/.." && pwd)"
BINARY_PATH="${ROOT_DIR}/target/release/tmux-easy-motion"
RELEASE_VERSION_FILE="${ROOT_DIR}/.release-version"
INSTALLED_VERSION_FILE="${ROOT_DIR}/target/release/.tmux-easy-motion-version"

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
    local platform asset_name binary_url release_json
    platform="$(detect_platform)" || return 1
    asset_name="tmux-easy-motion-${platform}"

    # Query latest release
    if ! command -v curl >/dev/null 2>&1; then
        return 1
    fi

    release_json=$(curl -fsSL -H 'Accept: application/vnd.github+json' -H 'User-Agent: tmux-easy-motion' "${GITHUB_RELEASE_API}") || return 1
    
    if [[ -z "${release_json}" ]]; then
        return 1
    fi

    # Try to extract download URL using jq if available, otherwise use grep/sed
    if command -v jq >/dev/null 2>&1; then
        binary_url=$(echo "${release_json}" | jq -r --arg asset_name "${asset_name}" '.assets[] | select(.name == $asset_name) | .browser_download_url' | head -1)
    else
        # Fallback to grep parsing for systems without jq
        # Find the asset block that contains our platform name, then extract browser_download_url
        binary_url=$(echo "${release_json}" | grep -A 60 "\"name\": \"${asset_name}\"" | grep "browser_download_url" | grep -o "https[^\"]*" | head -1)
    fi
    
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

read_expected_binary_version() {
    [[ -f "${RELEASE_VERSION_FILE}" ]] || return 0
    tr -d '[:space:]' < "${RELEASE_VERSION_FILE}"
}

read_installed_binary_version() {
    [[ -f "${INSTALLED_VERSION_FILE}" ]] || return 0
    awk -F= '
        /^version=/ {
            print $2
            found = 1
            exit
        }
        END {
            if (!found && NR == 1 && $0 !~ /=/) {
                print $0
            }
        }
    ' "${INSTALLED_VERSION_FILE}" | tr -d '[:space:]'
}

mark_binary_version_current() {
    local expected_version platform
    expected_version="$(read_expected_binary_version)"
    platform="$(detect_platform)" || return 1

    {
        printf 'platform=%s\n' "${platform}"
        if [[ -n "${expected_version}" ]]; then
            printf 'version=%s\n' "${expected_version}"
        fi
    } > "${INSTALLED_VERSION_FILE}" || return 1
}

read_installed_binary_platform() {
    [[ -f "${INSTALLED_VERSION_FILE}" ]] || return 0
    awk -F= '/^platform=/ { print $2; exit }' "${INSTALLED_VERSION_FILE}" | tr -d '[:space:]'
}

detect_binary_platform_from_file() {
    local description
    command -v file >/dev/null 2>&1 || return 1
    description="$(file -b "${BINARY_PATH}" 2>/dev/null)" || return 1

    case "${description}" in
        *ELF*"x86-64"*) echo "linux-x86_64" ;;
        *ELF*"ARM aarch64"*) echo "linux-aarch64" ;;
        *Mach-O*"arm64"*) echo "macos-aarch64" ;;
        *Mach-O*"x86_64"*) echo "macos-x86_64" ;;
        *) return 1 ;;
    esac
}

binary_matches_current_platform() {
    local current_platform installed_platform
    [[ -x "${BINARY_PATH}" ]] || return 1

    current_platform="$(detect_platform)" || return 1
    installed_platform="$(read_installed_binary_platform)"
    if [[ -z "${installed_platform}" ]]; then
        installed_platform="$(detect_binary_platform_from_file)"
    fi
    [[ -n "${installed_platform}" ]] || return 1
    [[ "${installed_platform}" == "${current_platform}" ]]
}

binary_is_current() {
    local expected_version installed_version
    [[ -x "${BINARY_PATH}" ]] || return 1

    binary_matches_current_platform || return 1

    expected_version="$(read_expected_binary_version)"
    [[ -n "${expected_version}" ]] || return 0

    installed_version="$(read_installed_binary_version)"
    [[ -n "${installed_version}" ]] || return 1
    [[ "${installed_version}" == "${expected_version}" ]]
}

create_temp_dir() {
    local tmp_parent template
    tmp_parent="${TMPDIR:-/tmp}"
    tmp_parent="${tmp_parent%/}"
    template="${tmp_parent}/tmux-easy-motion.XXXXXXXXXX"
    mktemp -d "${template}"
}

write_swap_runner_script() {
    local runner_script runner_stderr motion motion_argument cursor_pos pane_size capture_file jump_pipe target_key_pipe
    runner_script="$1"
    runner_stderr="$2"
    motion="$3"
    motion_argument="$4"
    cursor_pos="$5"
    pane_size="$6"
    capture_file="$7"
    jump_pipe="$8"
    target_key_pipe="$9"

    {
        printf '#!/usr/bin/env bash\n'
        printf '%q \\\n' "${BINARY_PATH}"
        printf '    %q \\\n' "${EASY_MOTION_DIM_STYLE}"
        printf '    %q \\\n' "${EASY_MOTION_HIGHLIGHT_STYLE}"
        printf '    %q \\\n' "${EASY_MOTION_HIGHLIGHT_2_FIRST_STYLE}"
        printf '    %q \\\n' "${EASY_MOTION_HIGHLIGHT_2_SECOND_STYLE}"
        printf '    %q \\\n' "${motion}"
        printf '    %q \\\n' "${motion_argument}"
        printf '    %q \\\n' "${EASY_MOTION_TARGET_KEYS}"
        printf '    %q \\\n' "${cursor_pos}"
        printf '    %q \\\n' "${pane_size}"
        printf '    %q \\\n' "${capture_file}"
        printf '    %q \\\n' "${jump_pipe}"
        printf '    %q' "${target_key_pipe}"
        printf ' 2>%q\n' "${runner_stderr}"
        printf 'exec tail -f /dev/null\n'
    } > "${runner_script}" || return 1

    chmod +x "${runner_script}" || return 1
}

emit_runner_error() {
    local runner_stderr
    runner_stderr="$1"

    [[ -s "${runner_stderr}" ]] || return 1
    cat "${runner_stderr}" >&2
    return 0
}

retry_tmux_command() {
    local attempts delay_ms
    attempts="$1"
    delay_ms="$2"
    shift 2

    local attempt
    for (( attempt = 1; attempt <= attempts; attempt++ )); do
        if "$@"; then
            return 0
        fi
        if (( attempt < attempts )); then
            sleep "0.$(printf '%03d' "${delay_ms}")"
        fi
    done
    return 1
}

show_swap_ui() {
    local swap_pane_id pane_id window_id
    swap_pane_id="$1"
    pane_id="$2"
    window_id="$3"

    retry_tmux_command 5 50 tmux swap-pane -Z -s "${swap_pane_id}" -t "${pane_id}" || return 1
    tmux set-window-option -t "${window_id}" key-table easy-motion-target >/dev/null 2>&1 || true
    tmux switch-client -T easy-motion-target >/dev/null 2>&1 || true
}

restore_original_pane() {
    local swap_pane_id pane_id window_id
    swap_pane_id="$1"
    pane_id="$2"
    window_id="$3"

    tmux set-window-option -t "${window_id}" key-table root >/dev/null 2>&1 || true
    tmux switch-client -T root >/dev/null 2>&1 || true
    retry_tmux_command 5 50 tmux swap-pane -Z -s "${swap_pane_id}" -t "${pane_id}" || return 1
}

ensure_binary_exists() {
    local binary_exists platform_matches
    binary_exists=0
    platform_matches=0
    if [[ -x "${BINARY_PATH}" ]]; then
        binary_exists=1
    fi

    if binary_matches_current_platform; then
        platform_matches=1
    fi

    if binary_is_current; then
        return 0
    fi

    # Try downloading from GitHub release first (preferred for TPM installations)
    if download_binary; then
        mark_binary_version_current || return 1
        return 0
    fi

    # If EASY_MOTION_ALLOW_BUILD is explicitly set to 1, attempt local build
    # Otherwise, just fail and tell user to download/reinstall
    if [[ "${EASY_MOTION_ALLOW_BUILD}" == "1" ]]; then
        if build_binary_locally; then
            if [[ -x "${BINARY_PATH}" ]]; then
                mark_binary_version_current || return 1
                return 0
            fi
        fi
    fi

    if (( binary_exists )) && (( platform_matches )); then
        return 0
    fi

    # Both download and (optional) build failed
    tmux display-message "tmux-easy-motion: binary missing or incompatible at ${BINARY_PATH}. Please run 'cd ${ROOT_DIR} && cargo build --release' or reinstall the plugin."
    return 1
}

main() {
    local server_pid session_id window_id pane_id motion motion_argument
    local capture_tmp_directory capture_file jump_pipe target_key_pipe_tmp_directory target_key_pipe runner_script runner_stderr
    local cursor_pos pane_size ready_command jump_command jump_cursor_position
    local swap_window_id swap_pane_id swap_ids ui_visible

    server_pid="$1"
    session_id="$2"
    window_id="$3"
    pane_id="$4"
    motion="$5"
    motion_argument="$6"

    ensure_binary_exists || return 1
    read_options || return 1

    capture_tmp_directory="$(create_temp_dir)" || return 1
    trap 'rm -rf "${capture_tmp_directory}"' EXIT

    capture_file="${capture_tmp_directory}/${CAPTURE_PANE_FILENAME}"
    jump_pipe="${capture_tmp_directory}/${JUMP_COMMAND_PIPENAME}"

    # Capture pane content - always use viewport (visible content)
    # This ensures consistent coordinate space with terminal rendering
    tmux capture-pane -t "${pane_id}" -p > "${capture_file}" || return 1
    mkfifo "${jump_pipe}" || return 1

    # Save cursor position before entering copy-mode if already in it
    local saved_cursor=""
    local pane_in_mode
    pane_in_mode="$(tmux display-message -p -t "${pane_id}" "#{pane_in_mode}")"
    if [[ "${pane_in_mode}" == "1" ]]; then
        saved_cursor="$(read_cursor_position "${pane_id}")"
    fi
    
    # Always enter copy-mode (safe even if already in it)
    tmux copy-mode -t "${pane_id}" || return 1
    
    # Restore original cursor position if we were already in copy-mode
    # This maintains consistency between capture buffer and cursor location
    if [[ -n "${saved_cursor}" ]]; then
        set_cursor_position "${pane_id}" "${saved_cursor}"
    fi
    
    # Read cursor position and clamp to pane dimensions immediately
    # This ensures all subsequent coordinate calculations are consistent
    local cursor_row cursor_col pane_width
    local raw_cursor
    raw_cursor="$(read_cursor_position "${pane_id}")"
    IFS=':' read -r cursor_row cursor_col <<< "${raw_cursor}"
    pane_width="$(tmux display-message -p -t "${pane_id}" "#{pane_width}" | awk '{print $1}')"
    
    # Clamp column to pane width for non-full-width panes
    # capture-pane can only get content up to pane_width columns
    if (( cursor_col >= pane_width )); then
        cursor_col=$((pane_width - 1))
        # Move cursor to clamped position so capture/cursor align
        set_cursor_position "${pane_id}" "${cursor_row}:${cursor_col}"
    fi
    
    cursor_pos="${cursor_row}:${cursor_col}"
    
    # For split panes (non-full-width), get actual pane width from tmux
    # capture-pane truncates lines to pane width, so we must use tmux's width
    local pane_width
    pane_width="$(tmux display-message -p -t "${pane_id}" "#{pane_width}" | awk '{print $1}')"
    
    # Calculate pane height from capture buffer
    local pane_height
    pane_height="$(wc -l < "${capture_file}" | awk '{print $1}')"
    
    pane_size="${pane_width}:${pane_height}"

    # Create swap pane for showing selection interface
    swap_ids="$(create_empty_swap_pane "${session_id}" "${window_id}" "${pane_id}")" || return 1
    swap_window_id="$(cut -d: -f1 <<< "${swap_ids}")"
    swap_pane_id="$(cut -d: -f2 <<< "${swap_ids}")"
    ui_visible=0
    
    reset_target_key_pipe "${server_pid}" "${session_id}" || return 1
    target_key_pipe_tmp_directory="$(get_target_key_pipe_tmp_directory "${server_pid}" "${session_id}")" || return 1
    target_key_pipe="${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}"
    runner_script="${capture_tmp_directory}/run_binary.sh"
    runner_stderr="${capture_tmp_directory}/run_binary.stderr"
    write_swap_runner_script \
        "${runner_script}" \
        "${runner_stderr}" \
        "${motion}" \
        "${motion_argument}" \
        "${cursor_pos}" \
        "${pane_size}" \
        "${capture_file}" \
        "${jump_pipe}" \
        "${target_key_pipe}" || return 1

    # Run Rust program in the hidden swap pane first.
    # Once it reports ready, swap the pane into view so users never see the blank placeholder.
    tmux respawn-pane -k -t "${swap_pane_id}" "${runner_script}" || return 1

    {
        read -r ready_command || {
            if emit_runner_error "${runner_stderr}"; then
                tmux kill-window -t "${swap_window_id}"
                return 1
            fi
            # Runner exited before showing any UI.
            tmux kill-window -t "${swap_window_id}"
            return 0
        }
        if [[ "${ready_command}" == "ready" ]]; then
            show_swap_ui "${swap_pane_id}" "${pane_id}" "${window_id}" || {
                tmux kill-window -t "${swap_window_id}"
                return 1
            }
            ui_visible=1
        elif [[ "${ready_command}" != "single-target" ]]; then
            emit_runner_error "${runner_stderr}" || true
            tmux kill-window -t "${swap_window_id}"
            return 0
        fi
        
        read -r jump_command || {
            if emit_runner_error "${runner_stderr}"; then
                if (( ui_visible )); then
                    restore_original_pane "${swap_pane_id}" "${pane_id}" "${window_id}" || true
                fi
                tmux kill-window -t "${swap_window_id}"
                return 1
            fi
            if (( ui_visible )); then
                restore_original_pane "${swap_pane_id}" "${pane_id}" "${window_id}" || true
            fi
            tmux kill-window -t "${swap_window_id}"
            return 0
        }
        
        if [[ "$(awk '{ print $1 }' <<< "${jump_command}")" != "jump" ]]; then
            emit_runner_error "${runner_stderr}" || true
            if (( ui_visible )); then
                restore_original_pane "${swap_pane_id}" "${pane_id}" "${window_id}" || true
            fi
            tmux kill-window -t "${swap_window_id}"
            return 0
        fi
        
        jump_cursor_position="$(awk '{ print $2 }' <<< "${jump_command}")"
        
        if (( ui_visible )); then
            restore_original_pane "${swap_pane_id}" "${pane_id}" "${window_id}" || {
                tmux kill-window -t "${swap_window_id}"
                return 1
            }
        fi

        # Some tmux/terminal combinations can drop copy-mode during the swap lifecycle.
        # Re-enter it explicitly before applying the computed jump.
        tmux copy-mode -t "${pane_id}" || return 1
        
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

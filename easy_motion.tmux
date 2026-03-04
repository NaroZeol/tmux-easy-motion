#!/usr/bin/env bash

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="${CURRENT_DIR}/scripts"

source "${SCRIPTS_DIR}/helpers.sh"
source "${SCRIPTS_DIR}/options.sh"

setup_single_key_binding() {
    local server_pid key motion key_table
    server_pid="$1"
    key="$2"
    motion="$3"

    if [[ "${key:0:1}" != "g" ]]; then
        key_table="easy-motion"
    else
        key_table="easy-motion-g"
        key="${key:1}"
    fi

    [[ "${key}" != "" ]] || return

    tmux bind-key -T "${key_table}" "${key}" run-shell -b \
        "${SCRIPTS_DIR}/easy_motion.sh '${server_pid}' '#{session_id}' '#{window_id}' '#{pane_id}' '${motion}' ''"
}

setup_single_key_binding_with_argument() {
    local server_pid key motion key_table
    server_pid="$1"
    key="$2"
    motion="$3"

    if [[ "${key:0:1}" != "g" ]]; then
        key_table="easy-motion"
    else
        key_table="easy-motion-g"
        key="${key:1}"
    fi

    [[ "${key}" != "" ]] || return

    if [[ "${motion}" != "bd-f2" ]]; then
        tmux source - <<-EOF
            bind-key -T "${key_table}" "${key}" command-prompt -1 -p "character:" {
                set -g @tmp-easy-motion-argument "%%%"
                run-shell -b '${SCRIPTS_DIR}/easy_motion.sh "${server_pid}" "\#{session_id}" "#{window_id}" "#{pane_id}" "${motion}" "#{q:@tmp-easy-motion-argument}"'
            }
EOF
    else
        tmux source - <<-EOF
            bind-key -T "${key_table}" "${key}" command-prompt -1 -p "character 1:,character 2:" {
                set -g @tmp-easy-motion-argument1 "%1"
                set -g @tmp-easy-motion-argument2 "%2"
                run-shell -b '${SCRIPTS_DIR}/easy_motion.sh "${server_pid}" "\#{session_id}" "#{window_id}" "#{pane_id}" "${motion}" "#{q:@tmp-easy-motion-argument1}#{q:@tmp-easy-motion-argument2}"'
            }
EOF
    fi
}

main() {
    local server_pid key tmux_key target_key
    server_pid="$(get_tmux_server_pid)"

    read_options || return 1

    tmux bind-key "${EASY_MOTION_PREFIX}" switch-client -T easy-motion
    tmux bind-key -T copy-mode-vi "${EASY_MOTION_COPY_MODE_PREFIX}" switch-client -T easy-motion

    tmux bind-key -T easy-motion "g" switch-client -T easy-motion-g
    tmux bind-key -T easy-motion "Escape" switch-client -T root
    tmux bind-key -T easy-motion-g "Escape" switch-client -T root

    setup_single_key_binding "${server_pid}" "b" "b"
    setup_single_key_binding "${server_pid}" "B" "B"
    setup_single_key_binding "${server_pid}" "ge" "ge"
    setup_single_key_binding "${server_pid}" "gE" "gE"
    setup_single_key_binding "${server_pid}" "e" "e"
    setup_single_key_binding "${server_pid}" "E" "E"
    setup_single_key_binding "${server_pid}" "w" "w"
    setup_single_key_binding "${server_pid}" "W" "W"
    setup_single_key_binding "${server_pid}" "j" "j"
    setup_single_key_binding "${server_pid}" "J" "J"
    setup_single_key_binding "${server_pid}" "k" "k"
    setup_single_key_binding "${server_pid}" "K" "K"
    setup_single_key_binding_with_argument "${server_pid}" "f" "f"
    setup_single_key_binding_with_argument "${server_pid}" "F" "F"
    setup_single_key_binding_with_argument "${server_pid}" "t" "t"
    setup_single_key_binding_with_argument "${server_pid}" "T" "T"
    setup_single_key_binding_with_argument "${server_pid}" "s" "bd-f"
    setup_single_key_binding "${server_pid}" "c" "c"

    while read -n1 key; do
        case "${key}" in
            \;)
                tmux_key="\\${key}"
                ;;
            *)
                tmux_key="${key}"
                ;;
        esac
        case "${key}" in
            \"|\`)
                target_key="\\${key}"
                ;;
            *)
                target_key="${key}"
                ;;
        esac
        tmux source - <<-EOF
            bind-key -T easy-motion-target "${tmux_key}" {
                run-shell -b "${SCRIPTS_DIR}/pipe_target_key.sh '${server_pid}' '#{session_id}' '${target_key}'"
                switch-client -T easy-motion-target
            }
EOF
    done < <(echo -n "${EASY_MOTION_TARGET_KEYS}")
    tmux bind-key -T easy-motion-target "Escape" run-shell -b "${SCRIPTS_DIR}/pipe_target_key.sh '${server_pid}' '#{session_id}' 'esc'"
}

main

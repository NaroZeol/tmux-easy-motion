#!/usr/bin/env bash

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="${CURRENT_DIR}"

# shellcheck disable=SC1091
source "${SCRIPTS_DIR}/common_variables.sh"
# shellcheck disable=SC1091
source "${SCRIPTS_DIR}/helpers.sh"

main() {
    local server_pid session_id target_key target_key_pipe_tmp_directory
    server_pid="$1"
    session_id="$2"
    target_key="$3"

    ensure_target_key_pipe_exists "${server_pid}" "${session_id}" || return 1
    target_key_pipe_tmp_directory="$(get_target_key_pipe_tmp_directory "${server_pid}" "${session_id}")" || return 1
    echo "${target_key}" >> "${target_key_pipe_tmp_directory}/${TARGET_KEY_PIPENAME}"
}

main "$@"

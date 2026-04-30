#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="${HOME}/.local/share/ds2api"
BIN_DIR="${HOME}/.local/bin"
BIN_NAME="ds2api"

log() { printf '%s\n' "$*" >&2; }

uninstall_user_service() {
  local service_dir service_file
  service_dir="${HOME}/.config/systemd/user"
  service_file="${service_dir}/ds2api.service"

  if [ -f "$service_file" ] && command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now ds2api.service >/dev/null 2>&1 || true
    systemctl --user daemon-reload >/dev/null 2>&1 || true
  fi
  rm -f "$service_file"
}

main() {
  case "${1:-}" in
    -h|--help)
      printf 'Usage: uninstall.sh\n' >&2
      exit 0
      ;;
    "")
      uninstall_user_service
      rm -f "${BIN_DIR}/${BIN_NAME}"
      rm -rf "$INSTALL_ROOT"
      log "uninstalled ${BIN_NAME}"
      ;;
    *)
      printf 'error: unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
}

main "$@"

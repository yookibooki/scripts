#!/usr/bin/env bash
set -euo pipefail

REPO="CJackHwang/ds2api"
INSTALL_ROOT="${HOME}/.local/share/ds2api"
BIN_DIR="${HOME}/.local/bin"
BIN_NAME="ds2api"

cmd="install"

api_latest() { printf 'https://api.github.com/repos/%s/releases/latest' "$REPO"; }

log() { printf '%s\n' "$*" >&2; }
die() { log "error: $*"; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"; }

usage() {
  cat <<EOF
Usage: install.sh [install|uninstall]
EOF
}

detect_platform() {
  local os arch
  os=$(uname -s | tr '[:upper:]' '[:lower:]')
  arch=$(uname -m)
  case "$os" in linux|darwin) ;; *) die "unsupported OS: $os" ;; esac
  case "$arch" in x86_64|amd64) arch=amd64 ;; aarch64|arm64) arch=arm64 ;; armv7l|armv7) arch=armv7 ;; *) die "unsupported architecture: $arch" ;; esac
  printf '%s %s' "$os" "$arch"
}

latest_tag() {
  need curl
  curl -fsSL --proto '=https' --tlsv1.2 "$(api_latest)" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1
}

asset_name() {
  local release_tag="$1" os="$2" arch="$3"
  if [ "$os" = linux ] || [ "$os" = darwin ]; then
    printf 'ds2api_%s_%s_%s.tar.gz' "$release_tag" "$os" "$arch"
  else
    printf 'ds2api_%s_%s_%s.zip' "$release_tag" "$os" "$arch"
  fi
}

download() {
  need curl
  curl -fL --proto '=https' --tlsv1.2 --retry 3 --retry-all-errors --connect-timeout 10 --max-time 300 -o "$2" "$1"
}

checksum_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'; else shasum -a 256 "$1" | awk '{print $1}'; fi
}

verify_checksum() {
  local archive="$1" sums_file="$2" expected actual
  expected=$(grep -F " $(basename "$archive")" "$sums_file" | awk '{print $1}' | head -n1)
  [ -n "$expected" ] || die "checksum entry not found for $(basename "$archive")"
  actual=$(checksum_of "$archive")
  [ "$actual" = "$expected" ] || die "checksum mismatch for $(basename "$archive")"
}

extract_archive() {
  case "$1" in
    *.tar.gz) tar -xzf "$1" -C "$2" ;;
    *.zip) unzip -q "$1" -d "$2" ;;
    *) die "unsupported archive format: $1" ;;
  esac
}

parse_args() {
  while [ $# -gt 0 ]; do
    case "$1" in
      install) cmd="$1" ;;
      uninstall|remove) cmd="uninstall" ;;
      -h|--help) usage; exit 0 ;;
      *) die "unknown argument: $1" ;;
    esac
    shift
  done
}

install_release() {
  local release_tag="$1" os arch asset url sums_url tmpdir archive sums_file extract_dir extracted_root extracted_name version_root current_link bin_path current_target

  read -r os arch <<EOF
$(detect_platform)
EOF

  asset=$(asset_name "$release_tag" "$os" "$arch")
  url="https://github.com/${REPO}/releases/download/${release_tag}/${asset}"
  sums_url="https://github.com/${REPO}/releases/download/${release_tag}/sha256sums.txt"
  version_root="${INSTALL_ROOT}/releases/${release_tag}"
  current_link="${INSTALL_ROOT}/current"
  bin_path="${BIN_DIR}/${BIN_NAME}"

  need tar
  tmpdir=$(mktemp -d)
  trap 'rm -rf "${tmpdir:-}"' EXIT

  mkdir -p "$tmpdir/download" "$INSTALL_ROOT/releases" "$BIN_DIR"
  archive="$tmpdir/download/$asset"
  sums_file="$tmpdir/download/sha256sums.txt"
  download "$url" "$archive"
  download "$sums_url" "$sums_file"
  verify_checksum "$archive" "$sums_file"

  extract_dir="$tmpdir/extract"
  mkdir -p "$extract_dir"
  extract_archive "$archive" "$extract_dir"
  extracted_root=$(find "$extract_dir" -mindepth 1 -maxdepth 1 | head -n1)
  [ -n "$extracted_root" ] || die "archive extracted nothing"
  extracted_name=$(basename "$extracted_root")

  rm -rf "$version_root"
  mkdir -p "$version_root"
  mv "$extracted_root" "$version_root/$extracted_name"
  ln -sfn "releases/${release_tag}/${extracted_name}" "$current_link"

  if [ ! -e "${INSTALL_ROOT}/config.json" ] && [ -f "${current_link}/config.example.json" ]; then
    cp "${current_link}/config.example.json" "${INSTALL_ROOT}/config.json"
  fi

  ln -sfn "${INSTALL_ROOT}/current/${BIN_NAME}" "$bin_path"
  log "installed ${release_tag}"
  log "binary: ${bin_path}"
  log "config:  ${INSTALL_ROOT}/config.json"
}

uninstall() {
  rm -f "${BIN_DIR}/${BIN_NAME}" "${INSTALL_ROOT}/current"
  log "uninstalled ${BIN_NAME}"
}

main() {
  parse_args "$@"
  case "$cmd" in
    install)
      install_release "$(latest_tag)"
      ;;
    uninstall) uninstall ;;
  esac
}

main "$@"

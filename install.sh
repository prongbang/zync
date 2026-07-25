#!/bin/sh
# Zync installer.
#
# Downloads the prebuilt zync release binary (single binary, web UI
# embedded) for the current OS/arch and installs it.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/prongbang/zync/main/install.sh | sh
#
# Environment overrides:
#   ZYNC_VERSION      Install this version instead of the latest GitHub
#                      release (e.g. ZYNC_VERSION=0.2.0). No leading "v".
#   ZYNC_INSTALL_DIR   Install zync into this directory instead of the
#                      default (/usr/local/bin if writable, else
#                      $HOME/.local/bin).
#
# This script is POSIX sh (no bashisms) so it runs under dash/sh as well as
# bash/zsh.

set -eu

REPO="prongbang/zync"
GITHUB="https://github.com/${REPO}"
API="https://api.github.com/repos/${REPO}"

log() {
  printf '%s\n' "$*"
}

err() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

# --- Required tools -----------------------------------------------------

if need_cmd curl; then
  DOWNLOADER="curl"
elif need_cmd wget; then
  DOWNLOADER="wget"
else
  err "neither curl nor wget is installed; install one and re-run this script"
fi

need_cmd tar || err "tar is required but not installed"

fetch_to_stdout() {
  # fetch_to_stdout <url>
  if [ "$DOWNLOADER" = "curl" ]; then
    curl -fsSL "$1"
  else
    wget -qO- "$1"
  fi
}

fetch_to_file() {
  # fetch_to_file <url> <output path>
  if [ "$DOWNLOADER" = "curl" ]; then
    curl -fsSL -o "$2" "$1"
  else
    wget -qO "$2" "$1"
  fi
}

# --- Detect OS/arch -> release target triple -----------------------------

os_raw="$(uname -s)"
arch_raw="$(uname -m)"

case "$os_raw" in
  Linux) os_triple="unknown-linux-gnu" ;;
  Darwin) os_triple="apple-darwin" ;;
  *) err "unsupported OS: $os_raw (Zync ships prebuilt binaries for Linux and macOS only)" ;;
esac

case "$arch_raw" in
  x86_64 | amd64) arch="x86_64" ;;
  arm64 | aarch64) arch="aarch64" ;;
  *) err "unsupported architecture: $arch_raw (Zync ships prebuilt binaries for x86_64 and aarch64/arm64 only)" ;;
esac

TARGET="${arch}-${os_triple}"

case "$TARGET" in
  x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu | x86_64-apple-darwin | aarch64-apple-darwin) ;;
  *) err "unsupported OS/architecture combination: $os_raw/$arch_raw" ;;
esac

log "Detected target: $TARGET"

# --- Resolve version -------------------------------------------------------

if [ "${ZYNC_VERSION:-}" != "" ]; then
  VERSION="$ZYNC_VERSION"
  log "Using requested version: $VERSION"
else
  log "Resolving latest release..."
  latest_json="$(fetch_to_stdout "${API}/releases/latest")" || err "failed to query ${API}/releases/latest"
  tag="$(printf '%s' "$latest_json" | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
  if [ -z "$tag" ]; then
    err "no published release found for ${REPO} yet (or the GitHub API request failed). Set ZYNC_VERSION=<version> to install a specific version once one exists."
  fi
  VERSION="${tag#v}"
  log "Latest version: $VERSION"
fi

ASSET="zync-${VERSION}-${TARGET}.tar.gz"
ASSET_URL="${GITHUB}/releases/download/v${VERSION}/${ASSET}"
SHA_URL="${ASSET_URL}.sha256"

# --- Download, verify, extract ---------------------------------------------

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t zync-install)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

log "Downloading ${ASSET}..."
fetch_to_file "$ASSET_URL" "${TMP_DIR}/${ASSET}" || err "failed to download ${ASSET_URL} (does release v${VERSION} exist for ${TARGET}?)"
fetch_to_file "$SHA_URL" "${TMP_DIR}/${ASSET}.sha256" || err "failed to download ${SHA_URL}"

log "Verifying checksum..."
(
  cd "$TMP_DIR"
  if need_cmd sha256sum; then
    sha256sum -c "${ASSET}.sha256"
  elif need_cmd shasum; then
    shasum -a 256 -c "${ASSET}.sha256"
  else
    err "neither sha256sum nor shasum is installed; cannot verify download integrity"
  fi
) || err "checksum verification failed for ${ASSET}"

log "Extracting..."
tar -xzf "${TMP_DIR}/${ASSET}" -C "$TMP_DIR" || err "failed to extract ${ASSET}"

[ -f "${TMP_DIR}/zync" ] || err "zync binary not found in ${ASSET} (unexpected archive layout)"
chmod +x "${TMP_DIR}/zync"

# --- Choose install directory ----------------------------------------------

if [ "${ZYNC_INSTALL_DIR:-}" != "" ]; then
  INSTALL_DIR="$ZYNC_INSTALL_DIR"
  mkdir -p "$INSTALL_DIR" 2>/dev/null || err "could not create ZYNC_INSTALL_DIR: $INSTALL_DIR"
elif [ -w "/usr/local/bin" ]; then
  INSTALL_DIR="/usr/local/bin"
elif need_cmd sudo && [ -t 0 ]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="${HOME}/.local/bin"
  mkdir -p "$INSTALL_DIR" || err "could not create $INSTALL_DIR"
fi

# --- Install -----------------------------------------------------------

DEST="${INSTALL_DIR}/zync"

if [ -w "$INSTALL_DIR" ]; then
  cp "${TMP_DIR}/zync" "$DEST"
  chmod +x "$DEST"
else
  log "Installing to ${INSTALL_DIR} requires elevated privileges."
  sudo cp "${TMP_DIR}/zync" "$DEST"
  sudo chmod +x "$DEST"
fi

log ""
log "Installed zync ${VERSION} to ${DEST}"

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *) log "" ; log "warning: ${INSTALL_DIR} is not on your PATH. Add it, e.g.:" ; log "  export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

log ""
log "Requirements at runtime: a system 'git' (and 'ssh' for SSH remotes)."
log "The credential store (HTTPS token / SSH key) needs ZYNC_SECRET_KEY, a"
log "base64-encoded 32-byte key — generate one with:"
log "  openssl rand -base64 32"
log ""
log "Quick start:"
log "  ZYNC_SECRET_KEY=\$(openssl rand -base64 32) zync serve"
log "  # then open http://127.0.0.1:58271"
log ""
log "For production deployment (env vars, TLS, backups), see:"
log "  ${GITHUB}/blob/main/docs/DEPLOY.md"

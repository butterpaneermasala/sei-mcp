#!/usr/bin/env bash
set -euo pipefail

# Install latest release of sei-mcp-server-rs into ~/.local/bin (default)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/scripts/install.sh | bash
# Or:
#   GITHUB_REPO=<owner>/<repo> ./scripts/install.sh
#   ./scripts/install.sh <owner>/<repo>

BIN_NAME="sei-mcp-server-rs"
INSTALL_DIR="${HOME}/.local/bin"
REPO_SLUG="${GITHUB_REPO:-}"

if [[ -z "${REPO_SLUG}" ]] && command -v git >/dev/null 2>&1; then
  # Try to infer from current repo
  if REMOTE_URL=$(git config --get remote.origin.url 2>/dev/null); then
    # Handle formats like:
    #  - https://github.com/owner/repo.git
    #  - git@github.com:owner/repo.git
    if [[ "$REMOTE_URL" =~ github.com[:/]+([^/]+)/([^/.]+) ]]; then
      REPO_SLUG="${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    fi
  fi
fi

if [[ $# -ge 1 ]]; then
  REPO_SLUG="$1"
fi

if [[ -z "${REPO_SLUG}" ]]; then
  echo "ERROR: Unable to determine GitHub repo. Set GITHUB_REPO=owner/repo or pass as first arg." >&2
  exit 1
fi

# Detect OS
OS="$(uname -s)"
case "$OS" in
  Linux)   OS_SLUG=linux ; EXT=tar.gz ; ;;
  Darwin)  OS_SLUG=macos ; EXT=tar.gz ; ;;
  MINGW*|MSYS*|CYGWIN*) echo "Windows bash not supported by this script. Use PowerShell installer." >&2 ; exit 1 ;;
  *) echo "Unsupported OS: $OS" >&2 ; exit 1 ;;
 esac

# Detect Arch
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64) ARCH_SLUG=x86_64 ; ;;
  aarch64|arm64) ARCH_SLUG=arm64 ; ;;
  *) echo "Unsupported arch: $ARCH_RAW" >&2 ; exit 1 ;;
 esac

ASSET_BASENAME="${BIN_NAME}-${OS_SLUG}-${ARCH_SLUG}.${EXT}"
DOWNLOAD_URL="https://github.com/${REPO_SLUG}/releases/latest/download/${ASSET_BASENAME}"

TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

echo "Downloading: $DOWNLOAD_URL"
HTTP_STATUS=$(curl -w '%{http_code}' -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/asset") || true
if [[ "$HTTP_STATUS" != "200" ]]; then
  echo "ERROR: Download failed (HTTP $HTTP_STATUS). Ensure a release exists with asset ${ASSET_BASENAME}." >&2
  exit 1
fi

echo "Extracting..."
mkdir -p "$TMP_DIR/extract"
case "$EXT" in
  tar.gz) tar -xzf "$TMP_DIR/asset" -C "$TMP_DIR/extract" ; ;;
  zip)    unzip -q "$TMP_DIR/asset" -d "$TMP_DIR/extract" ; ;;
  *) echo "Unknown archive type: $EXT" >&2 ; exit 1 ;;
 esac

if [[ ! -f "$TMP_DIR/extract/${BIN_NAME}" ]]; then
  echo "ERROR: Binary ${BIN_NAME} not found in archive." >&2
  ls -la "$TMP_DIR/extract" >&2 || true
  exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 0755 "$TMP_DIR/extract/${BIN_NAME}" "$INSTALL_DIR/${BIN_NAME}"

if ! command -v "$INSTALL_DIR/${BIN_NAME}" >/dev/null 2>&1; then
  echo "Installed to $INSTALL_DIR. Ensure it is on your PATH." >&2
  echo "Example: export PATH=\"$INSTALL_DIR:\$PATH\"" >&2
fi

"$INSTALL_DIR/${BIN_NAME}" --version || true

echo
echo "Installed ${BIN_NAME} to ${INSTALL_DIR}."
echo "You can now reference 'sei-mcp-server-rs' in your MCP config."

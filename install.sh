#!/usr/bin/env sh
# mdview installer — downloads a prebuilt binary (or builds from source) and
# helps you run `mdview doctor` to wire up your agents. Safe to re-run.
#
#   curl -fsSL https://raw.githubusercontent.com/vantt/mdview/main/install.sh | sh
#
# Env overrides:
#   MDVIEW_INSTALL_DIR   target dir (default: first writable of the fallback chain)
#   MDVIEW_VERSION       release tag (default: latest)
set -eu

REPO="vantt/mdview"
BIN="mdview"

info() { printf '  %s\n' "$*"; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }

detect_target() {
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os" in
    Linux)  os_part="unknown-linux-musl" ;;
    Darwin) os_part="apple-darwin" ;;
    *) err "unsupported OS: $os (build from source: cargo install --git https://github.com/$REPO)" ;;
  esac
  case "$arch" in
    x86_64|amd64) arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *) err "unsupported arch: $arch" ;;
  esac
  echo "${arch_part}-${os_part}"
}

choose_dir() {
  if [ -n "${MDVIEW_INSTALL_DIR:-}" ]; then echo "$MDVIEW_INSTALL_DIR"; return; fi
  for d in "/usr/local/bin" "$HOME/.local/bin" "$HOME/.mdview/bin"; do
    if [ -d "$d" ] && [ -w "$d" ]; then echo "$d"; return; fi
    if mkdir -p "$d" 2>/dev/null && [ -w "$d" ]; then echo "$d"; return; fi
  done
  echo "$HOME/.mdview/bin"
}

echo "Installing mdview…"
TARGET="$(detect_target)"
VERSION="${MDVIEW_VERSION:-latest}"
DIR="$(choose_dir)"; mkdir -p "$DIR"

if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/$REPO/releases/latest/download/${BIN}-${TARGET}"
else
  URL="https://github.com/$REPO/releases/download/${VERSION}/${BIN}-${TARGET}"
fi

info "target: $TARGET"
info "into:   $DIR"

# Download beside the target, then rename over it: writing straight into the
# binary fails with "Text file busy" while an `mdview serve` daemon runs from it,
# whereas a rename replaces it safely (the running daemon keeps the old inode).
TMP="$DIR/.$BIN.download.$$"
trap 'rm -f "$TMP"' EXIT
HTTP_CODE="$(curl -sSL -w '%{http_code}' -o "$TMP" "$URL")" || true

if [ "$HTTP_CODE" = "200" ]; then
  chmod +x "$TMP"
  mv -f "$TMP" "$DIR/$BIN"
  info "downloaded prebuilt binary"
elif [ "$HTTP_CODE" != "404" ]; then
  err "download failed (HTTP ${HTTP_CODE:-none}) from $URL — check your network and re-run"
elif command -v cargo >/dev/null 2>&1; then
  info "no prebuilt binary for $TARGET in release $VERSION; building from source with cargo…"
  cargo install --git "https://github.com/$REPO" "$BIN" --root "${DIR%/bin}"
else
  err "no prebuilt binary for $TARGET and cargo not found. Install Rust, then: cargo install --git https://github.com/$REPO"
fi

case ":$PATH:" in
  *":$DIR:"*) ;;
  *) info "NOTE: $DIR is not on your PATH — add it, e.g.: export PATH=\"$DIR:\$PATH\"" ;;
esac

echo
if [ -x "$DIR/$BIN" ]; then
  echo "Installed $("$DIR/$BIN" --version)."
else
  echo "Installed."
fi
echo "Next:"
echo "  $BIN doctor --fix     # wire up the CLI instruction block and skill"
echo "  $BIN doctor --fix --mcp   # only for Claude Desktop (or other clients without a shell)"
echo "  $BIN serve            # start the viewer (http://localhost:7700)"

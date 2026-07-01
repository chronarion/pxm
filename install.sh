#!/bin/sh
# pxm installer. Pipe me to sh, but read me first — that is just good hygiene.
#
#   curl -fsSL https://raw.githubusercontent.com/chronarion/pxm/main/install.sh | sh
#
# Downloads the prebuilt binary for your platform from the latest GitHub
# release. If there is no binary for your platform, falls back to `cargo`.
set -eu

REPO="chronarion/pxm"
BINDIR="${PXM_BINDIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

asset=""
case "$os" in
  Linux)
    case "$arch" in x86_64 | amd64) asset="pxm-linux-x86_64" ;; esac
    ;;
  Darwin)
    case "$arch" in arm64 | aarch64) asset="pxm-macos-arm64" ;; esac
    ;;
esac

if [ -n "$asset" ]; then
  url="https://github.com/$REPO/releases/latest/download/$asset"
  tmp="$(mktemp)"
  echo "pxm: downloading $asset ..."
  if curl -fsSL "$url" -o "$tmp"; then
    mkdir -p "$BINDIR"
    mv "$tmp" "$BINDIR/pxm"
    chmod +x "$BINDIR/pxm"
    echo "pxm: installed to $BINDIR/pxm"
    case ":$PATH:" in
      *":$BINDIR:"*) ;;
      *) echo "pxm: add $BINDIR to your PATH to run 'pxm'." ;;
    esac
    echo "pxm: now install a coding agent (claude, codex, gemini) and run 'pxm doctor'."
    exit 0
  fi
  rm -f "$tmp"
  echo "pxm: no published binary reachable; trying cargo ..."
fi

if command -v cargo >/dev/null 2>&1; then
  echo "pxm: building from source with cargo ..."
  cargo install --git "https://github.com/$REPO"
  echo "pxm: done. Run 'pxm doctor'."
  exit 0
fi

echo "pxm: no prebuilt binary for $os/$arch and cargo is not installed."
echo "pxm: install Rust (https://rustup.rs) and re-run, or grab a binary from"
echo "     https://github.com/$REPO/releases"
exit 1

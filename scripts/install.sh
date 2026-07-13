#!/usr/bin/env sh
set -eu

repo="${SENTRA_REPO:-flash-dev-ctrl/sentra-cli}"
version="${SENTRA_VERSION:-latest}"
install_dir="${SENTRA_INSTALL_DIR:-$HOME/.local/bin}"

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 1
  fi
}

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)
    case "$arch" in
      x86_64|amd64) asset="sentra-linux-x86_64-musl.tar.gz" ;;
      aarch64|arm64) asset="sentra-linux-aarch64-musl.tar.gz" ;;
      *) echo "error: unsupported Linux architecture: $arch" >&2; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      x86_64|amd64) asset="sentra-macos-x86_64.tar.gz" ;;
      aarch64|arm64) asset="sentra-macos-aarch64.tar.gz" ;;
      *) echo "error: unsupported macOS architecture: $arch" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "error: unsupported OS: $os" >&2
    exit 1
    ;;
esac

need tar
need mktemp

if [ "$version" = "latest" ]; then
  url="https://github.com/$repo/releases/latest/download/$asset"
else
  url="https://github.com/$repo/releases/download/$version/$asset"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

archive="$tmp/$asset"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$archive"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$archive" "$url"
else
  echo "error: curl or wget is required" >&2
  exit 1
fi

mkdir -p "$tmp/extract"
tar -xzf "$archive" -C "$tmp/extract"

bin="$(find "$tmp/extract" -type f -name sentra -perm -u+x 2>/dev/null | head -n 1)"
if [ -z "$bin" ]; then
  bin="$(find "$tmp/extract" -type f -name sentra 2>/dev/null | head -n 1)"
fi
if [ -z "$bin" ]; then
  echo "error: sentra binary not found in $asset" >&2
  exit 1
fi

mkdir -p "$install_dir"
cp "$bin" "$install_dir/sentra"
chmod 755 "$install_dir/sentra"

echo "sentra installed to $install_dir/sentra"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) echo "add this directory to PATH if needed: $install_dir" ;;
esac
"$install_dir/sentra" --help >/dev/null

#!/usr/bin/env sh
set -eu

COMMAND=${1:-install}
[ $# -gt 0 ] && shift || true

CHANNEL=${SIDECAR_CHANNEL:-stable}
VERSION=${SIDECAR_VERSION:-}
PUBLIC_URL=${SIDECAR_RELEASES_PUBLIC_URL:-}
INSTALL_ROOT=${SIDECAR_INSTALL_ROOT:-"$HOME/.local/share/sidecar"}
LOCAL_BIN_DIR=${SIDECAR_LOCAL_BIN_DIR:-"$HOME/.local/bin"}

while [ $# -gt 0 ]; do
  case "$1" in
    --channel)
      CHANNEL=${2:-}
      [ -n "$CHANNEL" ] || { echo "--channel requires a value" >&2; exit 1; }
      shift 2
      ;;
    --channel=*)
      CHANNEL=${1#--channel=}
      shift
      ;;
    --version)
      VERSION=${2:-}
      [ -n "$VERSION" ] || { echo "--version requires a value" >&2; exit 1; }
      shift 2
      ;;
    --version=*)
      VERSION=${1#--version=}
      shift
      ;;
    --public-url)
      PUBLIC_URL=${2:-}
      [ -n "$PUBLIC_URL" ] || { echo "--public-url requires a value" >&2; exit 1; }
      shift 2
      ;;
    --public-url=*)
      PUBLIC_URL=${1#--public-url=}
      shift
      ;;
    --install-root)
      INSTALL_ROOT=${2:-}
      [ -n "$INSTALL_ROOT" ] || { echo "--install-root requires a value" >&2; exit 1; }
      shift 2
      ;;
    --install-root=*)
      INSTALL_ROOT=${1#--install-root=}
      shift
      ;;
    --bin-dir)
      LOCAL_BIN_DIR=${2:-}
      [ -n "$LOCAL_BIN_DIR" ] || { echo "--bin-dir requires a value" >&2; exit 1; }
      shift 2
      ;;
    --bin-dir=*)
      LOCAL_BIN_DIR=${1#--bin-dir=}
      shift
      ;;
    -h|--help|help)
      cat <<'EOF'
sidecar installer

Usage:
  sidecar.sh install [--channel stable|beta] [--version vX.Y.Z] [--public-url <url>]
  sidecar.sh update  [--channel stable|beta] [--version vX.Y.Z] [--public-url <url>]
  sidecar.sh uninstall [--version vX.Y.Z]

Environment:
  SIDECAR_RELEASES_PUBLIC_URL
  SIDECAR_CHANNEL
  SIDECAR_VERSION
  SIDECAR_INSTALL_ROOT
  SIDECAR_LOCAL_BIN_DIR
EOF
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

need_public_url() {
  [ -n "$PUBLIC_URL" ] || {
    echo "SIDECAR_RELEASES_PUBLIC_URL or --public-url is required" >&2
    exit 1
  }
  PUBLIC_URL=${PUBLIC_URL%/}
}

platform_archive() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os:$arch" in
    Linux:x86_64|Linux:amd64) echo "sidecar-x86_64-unknown-linux-gnu.tar.gz" ;;
    Darwin:arm64|Darwin:aarch64) echo "sidecar-aarch64-apple-darwin.tar.gz" ;;
    Darwin:x86_64|Darwin:amd64) echo "sidecar-x86_64-apple-darwin.tar.gz" ;;
    *) echo "unsupported platform: $os $arch" >&2; exit 1 ;;
  esac
}

latest_version() {
  metadata="$1"
  sed -n 's/.*"releaseVersion"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$metadata" | head -n 1
}

install_sidecar() {
  need_public_url
  tmpdir=$(mktemp -d)
  trap 'rm -rf "$tmpdir"' EXIT INT TERM

  if [ -z "$VERSION" ]; then
    curl -fsSL "$PUBLIC_URL/$CHANNEL/latest/metadata.json" -o "$tmpdir/metadata.json"
    VERSION=$(latest_version "$tmpdir/metadata.json")
    [ -n "$VERSION" ] || { echo "failed to resolve latest sidecar version" >&2; exit 1; }
  fi

  archive=$(platform_archive)
  archive_url="$PUBLIC_URL/$CHANNEL/versions/$VERSION/$archive"
  mkdir -p "$INSTALL_ROOT/$VERSION" "$LOCAL_BIN_DIR"
  curl -fsSL "$archive_url" -o "$tmpdir/$archive"
  tar -xzf "$tmpdir/$archive" -C "$INSTALL_ROOT/$VERSION"
  chmod +x "$INSTALL_ROOT/$VERSION/sidecar"

  link="$LOCAL_BIN_DIR/sidecar"
  rm -f "$link"
  ln -s "$INSTALL_ROOT/$VERSION/sidecar" "$link"
  "$link" --version
  printf 'installed sidecar to %s\n' "$link"
}

uninstall_sidecar() {
  link="$LOCAL_BIN_DIR/sidecar"
  rm -f "$link"
  printf 'removed %s\n' "$link"

  if [ -n "$VERSION" ]; then
    rm -rf "$INSTALL_ROOT/$VERSION"
    printf 'removed %s\n' "$INSTALL_ROOT/$VERSION"
  else
    rm -rf "$INSTALL_ROOT"
    printf 'removed %s\n' "$INSTALL_ROOT"
  fi
}

case "$COMMAND" in
  install|update) install_sidecar ;;
  uninstall) uninstall_sidecar ;;
  *)
    echo "unknown command: $COMMAND" >&2
    exit 1
    ;;
esac

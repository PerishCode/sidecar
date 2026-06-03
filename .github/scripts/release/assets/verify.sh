#!/usr/bin/env sh
set -eu

mode=${1:-}
version=${2:-}
release_root=${3:-}

[ -n "$mode" ] || { echo "mode is required" >&2; exit 1; }
[ -n "$version" ] || { echo "release version is required" >&2; exit 1; }
[ -n "$release_root" ] || { echo "release root is required" >&2; exit 1; }
[ -d "$release_root" ] || { echo "release root not found: $release_root" >&2; exit 1; }

for name in \
  sidecar-x86_64-unknown-linux-gnu.tar.gz \
  sidecar-aarch64-apple-darwin.tar.gz \
  sidecar-x86_64-apple-darwin.tar.gz \
  sidecar-x86_64-pc-windows-msvc.zip \
  checksums.txt
do
  [ -f "$release_root/$name" ] || {
    echo "missing release asset: $release_root/$name" >&2
    exit 1
  }
done

if [ "$mode" = "verify" ]; then
  (
    cd "$release_root"
    shasum -a 256 -c checksums.txt
  )
  python3 - "$release_root" <<'PY'
import pathlib
import sys
import tarfile
import zipfile

root = pathlib.Path(sys.argv[1])

def ensure_tar_contains(name, member):
    with tarfile.open(root / name, "r:gz") as archive:
        names = archive.getnames()
    if member not in names:
        raise SystemExit(f"missing {member} in {name}")

def ensure_zip_contains(name, member):
    with zipfile.ZipFile(root / name) as archive:
        names = archive.namelist()
    if member not in names:
        raise SystemExit(f"missing {member} in {name}")

ensure_tar_contains("sidecar-x86_64-unknown-linux-gnu.tar.gz", "sidecar")
ensure_tar_contains("sidecar-aarch64-apple-darwin.tar.gz", "sidecar")
ensure_tar_contains("sidecar-x86_64-apple-darwin.tar.gz", "sidecar")
ensure_zip_contains("sidecar-x86_64-pc-windows-msvc.zip", "sidecar.exe")
PY
elif [ "$mode" != "accept" ]; then
  echo "unsupported verify mode: $mode" >&2
  exit 1
fi

printf 'release assets %s for %s\n' "$mode" "$version"

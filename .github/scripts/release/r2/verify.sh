#!/usr/bin/env bash
set -euo pipefail

[ -n "${R2_METADATA_URL:-}" ] || {
  echo "R2_METADATA_URL is required" >&2
  exit 1
}

tmpfile=$(mktemp)
trap 'rm -f "$tmpfile"' EXIT

curl -fsSL "$R2_METADATA_URL" -o "$tmpfile"
python3 -m json.tool "$tmpfile" >/dev/null
python3 - "$tmpfile" <<'PY'
import json
import sys
from pathlib import Path

metadata = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
manager = metadata.get("manager")
if not isinstance(manager, dict):
    raise SystemExit("metadata is missing manager object")
for key, suffix in (("unix", "/manage.sh"), ("windows", "/manage.ps1")):
    value = manager.get(key)
    if not isinstance(value, str) or not value.endswith(suffix):
        raise SystemExit(f"metadata manager.{key} must end with {suffix}")
PY
echo "R2 metadata ok: $R2_METADATA_URL"

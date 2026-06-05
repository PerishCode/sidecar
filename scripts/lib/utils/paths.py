from __future__ import annotations

import os
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
LOCAL_DIR = Path(os.environ.get("SIDECAR_REPO_LOCAL_DIR", REPO_ROOT / ".local"))
SECRETS_DIR = Path(os.environ.get("SIDECAR_REPO_SECRETS_DIR", LOCAL_DIR / "secrets"))
TMP_DIR = Path(os.environ.get("SIDECAR_REPO_TMP_DIR", LOCAL_DIR / "tmp"))

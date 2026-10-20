"""Per-OS details: data/cache locations, console-window suppression."""

import os
import subprocess
import sys
from pathlib import Path

# Windows: keep child processes (ffmpeg, claude, powershell) from flashing console windows.
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def _base(kind: str) -> Path:
    home = Path.home()
    if sys.platform == "win32":
        return Path(os.environ.get("LOCALAPPDATA", home / "AppData" / "Local")) / "tx" / kind
    if sys.platform == "darwin":
        return home / "Library" / ("Caches" if kind == "cache" else "Application Support") / "tx"
    env, default = ("XDG_CACHE_HOME", ".cache") if kind == "cache" else ("XDG_DATA_HOME", ".local/share")
    return Path(os.environ.get(env) or home / default) / "tx"


def data_dir() -> Path:
    p = _base("data")
    p.mkdir(parents=True, exist_ok=True)
    return p


def cache_dir() -> Path:
    p = _base("cache")
    p.mkdir(parents=True, exist_ok=True)
    return p

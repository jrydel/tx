"""Invoke the user's coding agent CLI (default: Claude Code) headlessly."""

import os
import shlex
import shutil
import subprocess

from .paths import NO_WINDOW


def command(env_key: str, default: str) -> list[str]:
    cmd = shlex.split(os.environ.get(env_key) or default, posix=os.name != "nt")
    cmd[0] = shutil.which(cmd[0]) or cmd[0]  # resolves claude.exe / claude.cmd on Windows
    return cmd


def ask(prompt: str, stdin: str, timeout: int = 900) -> str:
    """One-shot `$TX_AGENT <prompt>` with `stdin` piped in; returns stdout."""
    p = subprocess.run(
        [*command("TX_AGENT", "claude -p"), prompt],
        input=stdin,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        creationflags=NO_WINDOW,
    )
    if p.returncode:
        raise RuntimeError(f"agent exited {p.returncode}: {(p.stderr or p.stdout).strip()[-500:]}")
    return p.stdout.strip()

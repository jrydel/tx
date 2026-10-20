"""Run `tx serve` at login and add an openable "tx" app on Linux (systemd + .desktop),
macOS (launchd + ~/Applications/tx.app) and Windows (Startup script + Start Menu shortcut)."""

import os
import plistlib
import shutil
import subprocess
import sys
import time
import urllib.request
import webbrowser
from pathlib import Path

from .paths import NO_WINDOW, data_dir

PORT = 8765
LABEL = "dev.tx.watch"

ICON_SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
<rect width="64" height="64" rx="14" fill="#111"/>
<rect x="12" y="18" width="40" height="5" rx="2.5" fill="#f4f4f4"/>
<rect x="12" y="30" width="30" height="5" rx="2.5" fill="#f4f4f4"/>
<rect x="12" y="42" width="22" height="5" rx="2.5" fill="#ff4d4d"/>
</svg>"""


def ui_url(port: int = PORT) -> str:
    return f"http://127.0.0.1:{port}"


def is_running(port: int = PORT) -> bool:
    try:
        with urllib.request.urlopen(f"{ui_url(port)}/health", timeout=1) as r:
            return r.read() == b"tx"
    except OSError:
        return False


def _python(windowless: bool = False) -> str:
    exe = Path(sys.executable)
    if windowless and sys.platform == "win32" and (exe.parent / "pythonw.exe").exists():
        return str(exe.parent / "pythonw.exe")
    return str(exe)


def _chromium() -> str | None:
    if sys.platform == "win32":
        roots = [os.environ.get(k, "") for k in ("ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA")]
        rel = [r"Microsoft\Edge\Application\msedge.exe", r"Google\Chrome\Application\chrome.exe"]
        paths = [Path(r, p) for r in roots if r for p in rel]
    elif sys.platform == "darwin":
        apps = ["Google Chrome", "Microsoft Edge", "Chromium", "Brave Browser"]
        paths = [Path(f"/Applications/{a}.app/Contents/MacOS/{a}") for a in apps]
    else:
        names = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge", "brave-browser"]
        return next((p for n in names if (p := shutil.which(n))), None)
    return next((str(p) for p in paths if p.exists()), None)


def open_window(url: str) -> None:
    """Open `url` as a standalone app window (Chromium-family --app), else in the default browser."""
    exe = _chromium()
    if exe:
        subprocess.Popen([exe, f"--app={url}"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    else:
        webbrowser.open(url)


def ensure_running(port: int = PORT) -> None:
    if is_running(port):
        return
    flags = NO_WINDOW | getattr(subprocess, "DETACHED_PROCESS", 0)
    subprocess.Popen(
        [_python(windowless=True), "-m", "tx", "serve", "--port", str(port)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
        creationflags=flags,
    )
    for _ in range(50):
        if is_running(port):
            return
        time.sleep(0.2)
    raise SystemExit(f"tx: server did not start; see {data_dir() / 'tx.log'}")


def open_app(port: int = PORT) -> None:
    ensure_running(port)
    open_window(ui_url(port))


def _stop(port: int) -> None:
    try:
        req = urllib.request.Request(f"{ui_url(port)}/quit", method="POST")
        urllib.request.urlopen(req, timeout=2).close()
    except OSError:
        pass


# --- per-OS install -------------------------------------------------------------------------


def _restart(port: int) -> None:
    """Replace any running server (possibly an older install) with a fresh detached one."""
    _stop(port)
    for _ in range(25):
        if not is_running(port):
            break
        time.sleep(0.2)
    ensure_running(port)


def _linux_paths() -> tuple[Path, Path]:
    cfg = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    apps = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share") / "applications"
    return cfg / "autostart/tx.desktop", apps / "tx.desktop"


def _install_linux(port: int) -> list[Path]:
    # XDG autostart, not a systemd user unit: it runs inside the graphical session, so
    # notifications and the app window get the session's display and D-Bus.
    autostart, desktop = _linux_paths()
    icon = data_dir() / "icon.svg"
    icon.write_text(ICON_SVG, encoding="utf-8")
    entry = "[Desktop Entry]\nType=Application\nName=tx\nComment=TL;DRs of new videos from channels you follow\n"
    autostart.parent.mkdir(parents=True, exist_ok=True)
    autostart.write_text(
        f'{entry}Exec="{_python()}" -m tx serve --port {port}\nIcon={icon}\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n',
        encoding="utf-8",
    )
    desktop.parent.mkdir(parents=True, exist_ok=True)
    desktop.write_text(
        f'{entry}Exec="{_python()}" -m tx app --port {port}\nIcon={icon}\nTerminal=false\nCategories=Utility;\n',
        encoding="utf-8",
    )
    _restart(port)
    return [autostart, desktop]


def _uninstall_linux(port: int) -> list[Path]:
    paths = list(_linux_paths())
    for p in paths:
        p.unlink(missing_ok=True)
    _stop(port)
    return paths


def _mac_paths() -> tuple[Path, Path]:
    return Path.home() / f"Library/LaunchAgents/{LABEL}.plist", Path.home() / "Applications/tx.app"


def _install_mac(port: int) -> list[Path]:
    agent, app = _mac_paths()
    log = str(data_dir() / "tx.log")
    agent.parent.mkdir(parents=True, exist_ok=True)
    with agent.open("wb") as f:
        plistlib.dump(
            {
                "Label": LABEL,
                "ProgramArguments": [_python(), "-m", "tx", "serve", "--port", str(port)],
                "EnvironmentVariables": {"PATH": os.environ.get("PATH", "")},
                "RunAtLoad": True,
                "KeepAlive": {"SuccessfulExit": False},
                "StandardOutPath": log,
                "StandardErrorPath": log,
            },
            f,
        )
    exe = app / "Contents/MacOS/tx"
    exe.parent.mkdir(parents=True, exist_ok=True)
    exe.write_text(f'#!/bin/sh\nexec "{_python()}" -m tx app --port {port}\n', encoding="utf-8")
    exe.chmod(0o755)
    with (app / "Contents/Info.plist").open("wb") as f:
        plistlib.dump({"CFBundleName": "tx", "CFBundleExecutable": "tx", "CFBundleIdentifier": "dev.tx.app", "CFBundlePackageType": "APPL"}, f)
    _stop(port)
    domain = f"gui/{os.getuid()}"
    subprocess.run(["launchctl", "bootout", domain, str(agent)], check=False, capture_output=True)
    subprocess.run(["launchctl", "bootstrap", domain, str(agent)], check=True)
    return [agent, app]


def _uninstall_mac(port: int) -> list[Path]:
    agent, app = _mac_paths()
    subprocess.run(["launchctl", "bootout", f"gui/{os.getuid()}", str(agent)], check=False, capture_output=True)
    agent.unlink(missing_ok=True)
    shutil.rmtree(app, ignore_errors=True)
    return [agent, app]


def _win_paths() -> tuple[Path, Path]:
    programs = Path(os.environ["APPDATA"]) / "Microsoft/Windows/Start Menu/Programs"
    return programs / "Startup/tx.vbs", programs / "tx.lnk"


def _install_windows(port: int) -> list[Path]:
    startup, lnk = _win_paths()
    pyw = _python(windowless=True)
    # Run hidden (window style 0) at login.
    startup.write_text(f'CreateObject("WScript.Shell").Run """{pyw}"" -m tx serve --port {port}", 0, False\n', encoding="utf-8")
    shortcut = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:TX_LNK); $s.TargetPath = $env:TX_PY; $s.Arguments = $env:TX_ARGS; $s.Save()"
    env = {**os.environ, "TX_LNK": str(lnk), "TX_PY": pyw, "TX_ARGS": f"-m tx app --port {port}"}
    subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", shortcut], env=env, check=True, creationflags=NO_WINDOW)
    _restart(port)
    return [startup, lnk]


def _uninstall_windows(port: int) -> list[Path]:
    startup, lnk = _win_paths()
    startup.unlink(missing_ok=True)
    lnk.unlink(missing_ok=True)
    _stop(port)
    return [startup, lnk]


def install(port: int = PORT) -> list[Path]:
    fn = {"win32": _install_windows, "darwin": _install_mac}.get(sys.platform, _install_linux)
    return fn(port)


def uninstall(port: int = PORT) -> list[Path]:
    fn = {"win32": _uninstall_windows, "darwin": _uninstall_mac}.get(sys.platform, _uninstall_linux)
    return fn(port)

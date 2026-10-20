"""Desktop notifications without extra dependencies: notify-send (Linux),
osascript (macOS), WinRT toast via Windows PowerShell (Windows)."""

import os
import shutil
import subprocess
import sys
import threading

from .paths import NO_WINDOW

_WIN_TOAST = r"""
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$e = [System.Security.SecurityElement]
$xml = '<toast activationType="protocol" launch="' + $e::Escape($env:TX_URL) + '"><visual><binding template="ToastGeneric"><text>' + $e::Escape($env:TX_TITLE) + '</text><text>' + $e::Escape($env:TX_BODY) + '</text></binding></visual></toast>'
$doc = New-Object Windows.Data.Xml.Dom.XmlDocument
$doc.LoadXml($xml)
$app = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($app).Show([Windows.UI.Notifications.ToastNotification]::new($doc))
"""


def _linux(title: str, body: str, url: str | None) -> None:
    exe = shutil.which("notify-send")
    if not exe:
        return
    # --wait blocks until the notification closes; prints the action key if clicked.
    p = subprocess.run([exe, "-a", "tx", "-A", "default=Open", "--wait", title, body], capture_output=True, text=True)
    if p.returncode:  # libnotify < 0.7.10: no actions
        subprocess.run([exe, "-a", "tx", title, body])
    elif url and p.stdout.strip() == "default":
        from .service import open_window

        open_window(url)


def _mac(title: str, body: str, url: str | None) -> None:
    script = ["-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e", "end run"]
    subprocess.run(["osascript", *script, title, body])


def _windows(title: str, body: str, url: str | None) -> None:
    env = {**os.environ, "TX_TITLE": title, "TX_BODY": body, "TX_URL": url or ""}
    subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", _WIN_TOAST], env=env, creationflags=NO_WINDOW)


_threads: list[threading.Thread] = []


def notify(title: str, body: str, url: str | None = None) -> None:
    """Show a desktop notification; clicking it opens `url` where the OS supports it."""
    fn = {"win32": _windows, "darwin": _mac}.get(sys.platform, _linux)

    def run() -> None:
        try:
            fn(title, body, url)
        except Exception as e:
            print(f"tx: notification failed: {e}", file=sys.stderr)

    t = threading.Thread(target=run, daemon=True)
    t.start()
    _threads[:] = [x for x in _threads if x.is_alive()] + [t]


def flush(timeout: float = 3) -> None:
    """Give in-flight notifications time to display before a short-lived process exits."""
    for t in _threads:
        t.join(timeout)

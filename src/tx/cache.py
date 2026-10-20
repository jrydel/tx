import hashlib
import json
from pathlib import Path

from .parse import Transcript
from .paths import cache_dir


def load(url: str, lang: str | None = None, whisper: bool = False, model: str = "small", refresh: bool = False) -> tuple[Transcript, Path]:
    """Cached transcript for `url`; returns it with its cache path (.json)."""
    mode = f"whisper:{model}" if whisper else "auto"
    key = hashlib.sha1(f"{url}|{lang}|{mode}".encode()).hexdigest()[:16]
    path = cache_dir() / f"{key}.json"
    if path.exists() and not refresh:
        return Transcript.from_json(json.loads(path.read_text(encoding="utf-8"))), path
    from .sources import fetch  # heavy import; skipped on cache hits

    t = fetch(url, lang, whisper, model)
    path.write_text(json.dumps(t.to_json(), ensure_ascii=False), encoding="utf-8")
    return t, path

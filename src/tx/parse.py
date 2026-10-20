"""Caption parsing and transcript rendering. Pure functions, no I/O."""

import html
import re
from dataclasses import asdict, dataclass


@dataclass
class Seg:
    start: float  # seconds
    text: str


@dataclass
class Transcript:
    title: str
    url: str
    uploader: str | None
    duration: float | None
    source: str  # e.g. "captions en (auto)" or "whisper small (en)"
    segs: list[Seg]

    def to_json(self) -> dict:
        return asdict(self)

    @classmethod
    def from_json(cls, d: dict) -> "Transcript":
        return cls(**{**d, "segs": [Seg(**s) for s in d["segs"]]})


def _clean(text: str) -> str:
    return re.sub(r"\s+", " ", html.unescape(text)).strip()


def parse_json3(data: dict) -> list[Seg]:
    """YouTube json3 captions. `aAppend` events only carry line breaks of rolling
    auto-captions; skipping them yields each word exactly once."""
    segs = []
    for ev in data.get("events", []):
        if ev.get("aAppend") or "segs" not in ev:
            continue
        text = _clean("".join(s.get("utf8", "") for s in ev["segs"]))
        if text:
            segs.append(Seg(ev.get("tStartMs", 0) / 1000, text))
    return segs


_CUE_TIME = re.compile(r"(?:(\d+):)?(\d{1,2}):(\d{2})[.,](\d{3})\s*-->")
_TAG = re.compile(r"<[^>]*>")


def parse_vtt(text: str) -> list[Seg]:
    """WebVTT or SRT. Rolling captions repeat the previous line at the top of
    each cue; lines identical to the last emitted one are dropped."""
    segs: list[Seg] = []
    last = None
    for block in re.split(r"\n\s*\n", text.replace("\r\n", "\n")):
        lines = block.strip().split("\n")
        for i, line in enumerate(lines):
            m = _CUE_TIME.search(line)
            if m:
                h, mnt, s, ms = m.groups()
                start = int(h or 0) * 3600 + int(mnt) * 60 + int(s) + int(ms) / 1000
                for raw in lines[i + 1 :]:
                    t = _clean(_TAG.sub("", raw))
                    if t and t != last:
                        segs.append(Seg(start, t))
                        last = t
                break
    return segs


def paragraphs(segs: list[Seg], soft: float = 30, hard: float = 60) -> list[Seg]:
    """Merge segments into paragraphs: break at sentence end after `soft`
    seconds, unconditionally after `hard` seconds (auto-captions lack punctuation)."""
    out: list[Seg] = []
    cur: Seg | None = None
    for s in segs:
        if cur is None:
            cur = Seg(s.start, s.text)
            continue
        span = s.start - cur.start
        if span >= hard or (span >= soft and cur.text.endswith((".", "?", "!"))):
            out.append(cur)
            cur = Seg(s.start, s.text)
        else:
            cur.text += " " + s.text
    if cur:
        out.append(cur)
    return out


def fmt_time(sec: float) -> str:
    sec = int(sec)
    h, rem = divmod(sec, 3600)
    m, s = divmod(rem, 60)
    return f"{h}:{m:02d}:{s:02d}" if h else f"{m:02d}:{s:02d}"


def render(t: Transcript, timestamps: bool = True) -> str:
    meta = [x for x in (t.uploader, t.duration and fmt_time(t.duration), t.url) if x]
    lines = [f"# {t.title}", " · ".join(meta), f"source: {t.source}", ""]
    for p in paragraphs(t.segs):
        lines.append((f"[{fmt_time(p.start)}] " if timestamps else "") + p.text)
        lines.append("")
    return "\n".join(lines)

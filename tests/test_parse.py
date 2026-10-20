from tx.parse import Seg, fmt_time, paragraphs, parse_json3, parse_vtt
from tx.sources import _pick_track


def test_json3_skips_append_events_so_rolling_words_appear_once():
    data = {
        "events": [
            {"tStartMs": 0, "segs": [{"utf8": "hello"}, {"utf8": " world"}]},
            {"tStartMs": 1500, "aAppend": 1, "segs": [{"utf8": "\n"}]},
            {"tStartMs": 1500, "segs": [{"utf8": "it&#39;s"}, {"utf8": " me"}]},
            {"tStartMs": 3000},  # window-only event, no text
        ]
    }
    assert parse_json3(data) == [Seg(0.0, "hello world"), Seg(1.5, "it's me")]


def test_vtt_rolling_captions_drop_repeated_line_and_inline_tags():
    vtt = """WEBVTT
Kind: captions

00:00:01.000 --> 00:00:03.000 align:start
first line<00:00:01.500><c> here</c>

00:00:03.000 --> 00:00:03.010
first line here

00:00:03.010 --> 00:00:05.000
first line here
second line
"""
    assert parse_vtt(vtt) == [Seg(1.0, "first line here"), Seg(3.01, "second line")]


def test_srt_with_hours_and_comma_millis():
    srt = "1\r\n01:02:03,500 --> 01:02:05,000\r\nHi &amp; bye\r\n\r\n2\r\n01:02:05,000 --> 01:02:06,000\r\nNext\r\n"
    assert parse_vtt(srt) == [Seg(3723.5, "Hi & bye"), Seg(3725.0, "Next")]


def test_paragraphs_break_on_sentence_after_soft_and_always_after_hard():
    segs = [Seg(0, "a."), Seg(10, "b."), Seg(31, "c"), Seg(50, "d"), Seg(95, "e")]
    assert paragraphs(segs, soft=30, hard=60) == [Seg(0, "a. b."), Seg(31, "c d"), Seg(95, "e")]


def test_fmt_time():
    assert (fmt_time(59.9), fmt_time(3725)) == ("00:59", "1:02:05")


def test_pick_track_prefers_original_over_translated_and_skips_live_chat():
    tracks = {"live_chat": [], "de": [], "en": [], "en-orig": []}
    assert _pick_track(tracks, ["en"]) == "en-orig"
    assert _pick_track({"en-US": [], "cs": []}, ["en"]) == "en-US"
    assert _pick_track({"live_chat": [], "fr-orig": [], "fr": []}, ["en"]) == "fr-orig"
    assert _pick_track({"live_chat": []}, ["en"]) is None

from tx.watch import parse_analysis
from tx.web import md, section

URL = "https://www.youtube.com/watch?v=abc"


def test_parse_analysis_extracts_score_and_verdict_and_strips_header():
    out = "**RELEVANCE: 4/5 — covers eval harness design**\n## TL;DR\nText.\n## Takeaways for you\n- point [01:02]"
    rel, verdict, body = parse_analysis(out)
    assert (rel, verdict) == (4, "covers eval harness design")
    assert body.startswith("## TL;DR") and "RELEVANCE" not in body


def test_parse_analysis_without_header_keeps_everything():
    assert parse_analysis("## TL;DR\nno score given") == (None, "", "## TL;DR\nno score given")


def test_parse_analysis_clamps_out_of_range_score():
    assert parse_analysis("Relevance: 9 - wow")[0] == 5


def test_md_escapes_untrusted_html_and_links_timestamps():
    out = md("## TL;DR\n<script>alert(1)</script> **bold** at [1:02:03]\n- item [00:05]", URL)
    assert "<script>" not in out and "&lt;script&gt;" in out
    assert "<strong>bold</strong>" in out
    assert f'href="{URL}&amp;t=3723s"' in out and f'href="{URL}&amp;t=5s"' in out
    assert "<ul>\n<li>item" in out


def test_section_returns_text_until_next_heading():
    text = "## TL;DR\nCore claim.\nMore.\n## Takeaways for you\n- x"
    assert section(text, "TL;DR") == "Core claim.\nMore."
    assert section(text, "Missing") == ""

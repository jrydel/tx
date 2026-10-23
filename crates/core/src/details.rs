//! What the setup panel shows beside a channel: its avatar and a subtitle like
//! "@Fireship · 4.31M subscribers". Imported subscriptions only carry an id and a title, so the
//! rest is read from the channel's public page once and kept in the database.

use crate::db::{self, Db};
use crate::feeds::CHANNEL_ID;
use anyhow::Result;
use futures_util::StreamExt;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

/// Channel pages fetched at once; YouTube serves each as a 1.7 MB page.
const MAX_IN_FLIGHT: usize = 6;
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);
/// Avatars come in whatever size the page used (900 px); the panel shows them small.
const AVATAR_PX: u32 = 176;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Details {
    pub thumbnail: Option<String>,
    pub subtitle: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChannelDetails {
    pub id: String,
    #[serde(flatten)]
    pub details: Details,
}

static OG_IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<meta property="og:image" content="([^"]+)""#).unwrap());
static AVATAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""avatar":\{"thumbnails":\[\{"url":"([^"]+)""#).unwrap());
static HANDLE_VANITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""vanityChannelUrl":"https?://(?:www\.)?youtube\.com/(@[^"/\\]+)""#).unwrap()
});
static HANDLE_CANONICAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""canonicalBaseUrl":"/(@[^"/\\]+)""#).unwrap());
/// The channel header's own count. Other channels on the page (featured, related) carry theirs as
/// `"simpleText"`, and come first, so those must not match.
static SUBSCRIBERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""content":"(\d[\d.,]*\s?[KMB]?\s+subscribers?)""#).unwrap());
static AVATAR_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"=s\d+-").unwrap());

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut headers = reqwest::header::HeaderMap::new();
    // CONSENT/SOCS skip the EU cookie wall, which has none of the channel's data.
    headers.insert(
        reqwest::header::COOKIE,
        "CONSENT=YES+1; SOCS=CAI".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        "en-US,en;q=0.9".parse().unwrap(),
    );
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .default_headers(headers)
        .timeout(FETCH_TIMEOUT)
        .build()
        .unwrap()
});

/// "@handle · 4.3M subscribers"; either part alone when the other is unknown.
pub fn subtitle(handle: Option<&str>, subscribers: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = [handle, subscribers].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A follower count as YouTube's own abbreviations: "4.3M subscribers", "463K subscribers".
pub fn subscribers_label(n: f64) -> String {
    match n {
        n if n >= 1e6 => format!("{:.1}M subscribers", n / 1e6),
        n if n >= 1e3 => format!("{:.0}K subscribers", n / 1e3),
        n => format!("{n:.0} subscribers"),
    }
}

/// An avatar URL asking for a sharper-than-thumbnail but still small size.
fn avatar_url(url: &str) -> String {
    AVATAR_SIZE
        .replace(
            &url.replace("&amp;", "&").replace("\\u0026", "&"),
            format!("=s{AVATAR_PX}-"),
        )
        .into_owned()
}

/// "%E3%81%82" -> "あ": handles of channels with non-Latin names are percent-encoded in the page.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Avatar, @handle and subscriber count from a `youtube.com/channel/UC…` page. Whatever the page
/// doesn't have stays empty.
pub fn parse_channel_page(html: &str) -> Details {
    let first = |re: &Regex| re.captures(html).map(|c| c[1].to_string());
    // og:image is the page's logo when YouTube serves something other than the channel.
    let thumbnail = first(&OG_IMAGE)
        .filter(|u| u.contains("://yt3."))
        .or_else(|| first(&AVATAR))
        .map(|u| avatar_url(&u));
    let handle = first(&HANDLE_VANITY)
        .or_else(|| first(&HANDLE_CANONICAL))
        .map(|h| percent_decode(&h));
    let subscribers = first(&SUBSCRIBERS);
    Details {
        thumbnail,
        subtitle: subtitle(handle.as_deref(), subscribers.as_deref()),
    }
}

/// The same from yt-dlp's info for a channel (or one of its videos), as far as it has them.
pub fn from_ytdlp(info: &Value) -> Details {
    let handle = info["uploader_id"].as_str().filter(|h| h.starts_with('@'));
    let subscribers = info["channel_follower_count"]
        .as_f64()
        .map(subscribers_label);
    // The avatar is the square one; banners and video thumbnails are not.
    let thumbnail = info["thumbnails"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["width"].is_u64() && t["width"] == t["height"])
        .and_then(|t| t["url"].as_str())
        .map(avatar_url);
    Details {
        thumbnail,
        subtitle: subtitle(handle, subscribers.as_deref()),
    }
}

async fn fetch_page(id: &str) -> Result<Details> {
    let html = CLIENT
        .get(format!("https://www.youtube.com/channel/{id}"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(parse_channel_page(&html))
}

/// Details of the requested channels that tx knows, in request order. What the database lacks is
/// read from YouTube (at most `MAX_IN_FLIGHT` pages at once) and stored; a failed fetch gives
/// nothing and stores nothing, so it is tried again next time. Podcasts have no page to read.
pub async fn lookup(db: &Db, ids: &[String]) -> Result<Vec<ChannelDetails>> {
    let known = db::channels(&db.lock().unwrap())?;
    let mut wanted: Vec<&db::Channel> = Vec::new();
    for c in ids
        .iter()
        .filter_map(|id| known.iter().find(|c| c.id == *id))
    {
        if !wanted.iter().any(|w| w.id == c.id) {
            wanted.push(c);
        }
    }
    let fetched: Vec<(&str, Details)> = futures_util::stream::iter(
        wanted
            .iter()
            .filter(|c| !c.podcast && (c.image.is_none() || c.subtitle.is_none()))
            .filter(|c| CHANNEL_ID.is_match(&c.id)),
    )
    .map(|c| async move {
        let found = fetch_page(&c.id).await.unwrap_or_else(|e| {
            log::info!("no details for {}: {e:#}", c.id);
            Details::default()
        });
        (c.id.as_str(), found)
    })
    .buffer_unordered(MAX_IN_FLIGHT)
    .collect()
    .await;
    {
        let conn = db.lock().unwrap();
        for (id, d) in &fetched {
            if d.thumbnail.is_some() || d.subtitle.is_some() {
                db::save_details(&conn, id, d.thumbnail.as_deref(), d.subtitle.as_deref())?;
            }
        }
    }
    Ok(wanted
        .iter()
        .map(|c| {
            let found = fetched
                .iter()
                .find(|(id, _)| *id == c.id)
                .map(|(_, d)| d.clone())
                .unwrap_or_default();
            ChannelDetails {
                id: c.id.clone(),
                details: Details {
                    thumbnail: found.thumbnail.or_else(|| c.image.clone()),
                    subtitle: found.subtitle.or_else(|| c.subtitle.clone()),
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A real channel page cut down to the parts parsed, plus the featured channel
    /// that precedes the channel's own header.
    const FIRESHIP: &str = include_str!("../tests/fixtures/youtube-channel-fireship.html");

    const AVATAR_S176: &str = "https://yt3.googleusercontent.com/3fPNbkf_xPyCleq77ZhcxyeorY97NtMHVNUbaAON_RBDH9ydL4hJkjxC8x_4mpuopkB8oI7Ct6Y=s176-c-k-c0x00ffffff-no-rj";

    #[test]
    fn reads_the_channels_own_page() {
        assert_eq!(
            parse_channel_page(FIRESHIP),
            Details {
                thumbnail: Some(AVATAR_S176.into()),
                subtitle: Some("@Fireship · 4.31M subscribers".into()),
            },
            "og:image, vanity URL and the header's count, not the featured channel's 463K"
        );
    }

    #[test]
    fn avatar_falls_back_to_the_header_thumbnails() {
        let html = FIRESHIP.replace(r#"<meta property="og:image""#, r#"<meta name="x""#);
        assert_eq!(
            parse_channel_page(&html).thumbnail.as_deref(),
            Some(AVATAR_S176)
        );
        // An og:image that isn't an avatar (YouTube's logo on a consent page) doesn't count.
        let logo = FIRESHIP.replace(
            "https://yt3.googleusercontent.com/3fPNbkf_xPyCleq77ZhcxyeorY97NtMHVNUbaAON_RBDH9ydL4hJkjxC8x_4mpuopkB8oI7Ct6Y=s900-c-k-c0x00ffffff-no-rj\">",
            "https://www.youtube.com/img/desktop/yt_1200.png\">",
        );
        assert_eq!(
            parse_channel_page(&logo).thumbnail.as_deref(),
            Some(AVATAR_S176)
        );
    }

    #[test]
    fn handle_from_either_place() {
        let canonical_only = FIRESHIP.replace("vanityChannelUrl", "somethingElse");
        assert_eq!(
            parse_channel_page(&canonical_only).subtitle.as_deref(),
            Some("@Fireship · 4.31M subscribers")
        );
        let vanity_only = FIRESHIP.replace("canonicalBaseUrl", "somethingElse");
        assert_eq!(
            parse_channel_page(&vanity_only).subtitle.as_deref(),
            Some("@Fireship · 4.31M subscribers")
        );
        let encoded = FIRESHIP.replace("/@Fireship\"", "/@%E3%81%82%E3%81%84\"");
        let encoded = encoded.replace("vanityChannelUrl", "somethingElse");
        assert_eq!(
            parse_channel_page(&encoded).subtitle.as_deref(),
            Some("@あい · 4.31M subscribers")
        );
    }

    #[test]
    fn subscriber_texts() {
        let with = |count: &str| {
            parse_channel_page(&format!(
                r#"{{"metadataParts":[{{"text":{{"content":"{count}"}}}}]}}"#
            ))
            .subtitle
        };
        assert_eq!(
            with("463K subscribers").as_deref(),
            Some("463K subscribers")
        );
        assert_eq!(
            with("1.2M subscribers").as_deref(),
            Some("1.2M subscribers")
        );
        assert_eq!(
            with("2.5B subscribers").as_deref(),
            Some("2.5B subscribers")
        );
        assert_eq!(
            with("1,234 subscribers").as_deref(),
            Some("1,234 subscribers")
        );
        assert_eq!(with("87 subscribers").as_deref(), Some("87 subscribers"));
        assert_eq!(with("1 subscriber").as_deref(), Some("1 subscriber"));
        assert_eq!(
            with("4.31\u{a0}M subscribers").as_deref(),
            Some("4.31\u{a0}M subscribers")
        );
        assert_eq!(with("856 videos"), None);
    }

    #[test]
    fn missing_parts_stay_empty() {
        assert_eq!(
            parse_channel_page("<html>consent</html>"),
            Details::default()
        );
        let handle_only = r#"{"vanityChannelUrl":"http://www.youtube.com/@abc"}"#;
        assert_eq!(
            parse_channel_page(handle_only).subtitle.as_deref(),
            Some("@abc")
        );
    }

    #[test]
    fn subtitle_combinations() {
        assert_eq!(
            subtitle(Some("@a"), Some("4.3M subscribers")).as_deref(),
            Some("@a · 4.3M subscribers")
        );
        assert_eq!(subtitle(Some("@a"), None).as_deref(), Some("@a"));
        assert_eq!(
            subtitle(None, Some("87 subscribers")).as_deref(),
            Some("87 subscribers")
        );
        assert_eq!(subtitle(None, None), None);
    }

    #[test]
    fn subscriber_labels() {
        assert_eq!(subscribers_label(4_310_000.0), "4.3M subscribers");
        assert_eq!(subscribers_label(463_000.0), "463K subscribers");
        assert_eq!(subscribers_label(87.0), "87 subscribers");
    }

    #[test]
    fn reads_ytdlp_channel_info() {
        let info = json!({
            "uploader_id": "@Fireship", "channel_follower_count": 4310000,
            "thumbnails": [
                { "url": "https://yt3.googleusercontent.com/b=w1060-fcrop64=1,00005a57ffffa5a8-k-c0xffffffff-no-nd-rj", "width": 1060, "height": 175 },
                { "url": "https://yt3.googleusercontent.com/a=s900-c-k-c0x00ffffff-no-rj", "width": 900, "height": 900 },
                { "url": "https://yt3.googleusercontent.com/a=s0", "id": "avatar_uncropped" },
            ],
        });
        assert_eq!(
            from_ytdlp(&info),
            Details {
                thumbnail: Some(
                    "https://yt3.googleusercontent.com/a=s176-c-k-c0x00ffffff-no-rj".into()
                ),
                subtitle: Some("@Fireship · 4.3M subscribers".into()),
            }
        );
        // A video's info has the channel's handle and count but only video thumbnails.
        let video = json!({ "uploader_id": "@Fireship", "thumbnails": [{ "url": "https://i.ytimg.com/v.jpg", "width": 1280, "height": 720 }] });
        assert_eq!(
            from_ytdlp(&video),
            Details {
                thumbnail: None,
                subtitle: Some("@Fireship".into())
            }
        );
        assert_eq!(
            from_ytdlp(&json!({ "uploader_id": "UCabc" })),
            Details::default()
        );
    }
}

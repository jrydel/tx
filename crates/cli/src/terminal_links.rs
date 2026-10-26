//! OSC 8 links built only from validated HTTP(S) source URLs, never from model text.

use std::fmt::Write;
use url::Url;

pub(crate) struct Links {
    source: String,
    seek: Option<(Seek, String)>,
}

#[derive(Clone, Copy)]
enum Seek {
    YouTube,
    Vimeo,
    Twitch,
    Dailymotion,
    Media,
}

impl Links {
    pub(crate) fn new(raw: &str) -> Option<Self> {
        // Reject rather than repair controls: URL parsing would otherwise strip some of them.
        if raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return None;
        }
        let mut source = Url::parse(raw).ok()?;
        if !matches!(source.scheme(), "http" | "https")
            || source.host_str().is_none()
            || !source.username().is_empty()
            || source.password().is_some()
        {
            return None;
        }
        let original = source.to_string();
        let host = source.host_str()?;
        let host = host.strip_prefix("www.").unwrap_or(host);
        let path = source.path();
        let seek = if matches!(
            host,
            "youtube.com" | "m.youtube.com" | "youtu.be" | "youtube-nocookie.com"
        ) {
            let id = if host == "youtu.be" {
                path.strip_prefix('/')
                    .filter(|id| !id.contains('/'))
                    .map(str::to_owned)
            } else if path == "/watch" {
                source
                    .query_pairs()
                    .find(|(key, _)| key == "v")
                    .map(|(_, value)| value.into_owned())
            } else {
                ["/shorts/", "/embed/", "/live/"]
                    .iter()
                    .find_map(|prefix| path.strip_prefix(prefix))
                    .map(str::to_owned)
            };
            if let Some(id) = id.filter(|id| {
                !id.is_empty()
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
            }) {
                // Open a watch page even when the supplied link points to Shorts or an embed.
                source.set_host(Some("www.youtube.com")).ok()?;
                source.set_path("/watch");
                let query: Vec<_> = source
                    .query_pairs()
                    .filter(|(key, _)| key != "v")
                    .map(|(key, value)| (key.into_owned(), value.into_owned()))
                    .collect();
                source.set_query(None);
                source
                    .query_pairs_mut()
                    .append_pair("v", &id)
                    .extend_pairs(query);
                Some(Seek::YouTube)
            } else {
                None
            }
        } else if matches!(host, "vimeo.com" | "player.vimeo.com") {
            Some(Seek::Vimeo)
        } else if host == "twitch.tv" && path.starts_with("/videos/") {
            Some(Seek::Twitch)
        } else if (host == "dailymotion.com" && path.starts_with("/video/")) || host == "dai.ly" {
            Some(Seek::Dailymotion)
        } else if path.rsplit('.').next().is_some_and(|ext| {
            ["mp3", "m4a", "mp4", "webm", "ogg", "wav"]
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        }) {
            Some(Seek::Media)
        } else {
            None
        };
        // Build the static URL prefix once, not once per clickable timeline cell.
        let seek = seek.map(|kind| {
            source.set_fragment(None);
            match kind {
                Seek::Vimeo | Seek::Media => source.set_fragment(Some("t=")),
                Seek::YouTube | Seek::Twitch | Seek::Dailymotion => {
                    let query: Vec<_> = source
                        .query_pairs()
                        .filter(|(key, _)| !matches!(key.as_ref(), "t" | "start" | "time_continue"))
                        .map(|(key, value)| (key.into_owned(), value.into_owned()))
                        .collect();
                    source.set_query(None);
                    let key = if matches!(kind, Seek::Dailymotion) {
                        "start"
                    } else {
                        "t"
                    };
                    source
                        .query_pairs_mut()
                        .extend_pairs(query)
                        .append_pair(key, "");
                }
            }
            (kind, String::from(source))
        });
        Some(Self {
            source: original,
            seek,
        })
    }

    pub(crate) fn can_seek(&self) -> bool {
        self.seek.is_some()
    }

    pub(crate) fn wrap(&self, label: String, at: Option<f64>) -> String {
        let Some(at) = at else {
            return format!("\x1b]8;;{}\x1b\\{label}\x1b]8;;\x1b\\", self.source);
        };
        let Some((kind, prefix)) = &self.seek else {
            return label;
        };
        if !at.is_finite() || at < 0.0 {
            return label;
        }
        let seconds = at.floor() as u64;
        let mut out = String::with_capacity(prefix.len() + label.len() + 48);
        write!(out, "\x1b]8;;{prefix}").unwrap();
        match kind {
            Seek::YouTube | Seek::Vimeo => write!(out, "{seconds}s").unwrap(),
            Seek::Twitch => write!(
                out,
                "{}h{}m{}s",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )
            .unwrap(),
            Seek::Dailymotion | Seek::Media => write!(out, "{seconds}").unwrap(),
        }
        write!(out, "\x1b\\{label}\x1b]8;;\x1b\\").unwrap();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_links_replace_old_offsets_and_keep_video_context() {
        let links =
            Links::new("https://www.youtube.com/watch?v=abc&list=queue&t=9s&start=2#old").unwrap();
        assert_eq!(
            links.wrap("[01:05]".into(), Some(65.0)),
            "\x1b]8;;https://www.youtube.com/watch?v=abc&list=queue&t=65s\x1b\\[01:05]\x1b]8;;\x1b\\"
        );
        for url in [
            "https://youtu.be/abc",
            "https://m.youtube.com/shorts/abc",
            "https://www.youtube-nocookie.com/embed/abc",
        ] {
            assert!(
                Links::new(url)
                    .unwrap()
                    .wrap("point".into(), Some(3723.0))
                    .contains("https://www.youtube.com/watch?v=abc&t=3723s")
            );
        }
    }

    #[test]
    fn unsafe_sources_cannot_create_terminal_links() {
        for url in [
            "file:///tmp/file",
            "javascript:alert(1)",
            "https://user:password@example.com",
            "https://example.com/\x1b]52;;secret\x07",
            "https://example.com/\npath",
            "https://example.com/\u{9c}path",
        ] {
            assert!(Links::new(url).is_none(), "{url:?}");
        }
        let links = Links::new("https://youtube.com.evil.test/watch?v=abc").unwrap();
        assert!(!links.can_seek());
        assert_eq!(links.wrap("[01:05]".into(), Some(65.0)), "[01:05]");
    }

    #[test]
    fn seek_conventions_and_unknown_sites_do_not_invent_support() {
        for (source, target) in [
            ("https://vimeo.com/123#old", "https://vimeo.com/123#t=65s"),
            (
                "https://www.twitch.tv/videos/123?t=2s",
                "https://www.twitch.tv/videos/123?t=0h1m5s",
            ),
            (
                "https://www.dailymotion.com/video/abc?start=2",
                "https://www.dailymotion.com/video/abc?start=65",
            ),
            (
                "https://example.com/podcast.mp3?token=abc",
                "https://example.com/podcast.mp3?token=abc#t=65",
            ),
        ] {
            assert!(
                Links::new(source)
                    .unwrap()
                    .wrap("point".into(), Some(65.0))
                    .contains(target)
            );
        }
        let links = Links::new("https://www.infoq.com/presentations/rust/").unwrap();
        assert_eq!(links.wrap("point".into(), Some(65.0)), "point");
        assert!(
            links
                .wrap("source".into(), None)
                .contains("https://www.infoq.com/presentations/rust/")
        );
        for at in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(
                Links::new("https://youtu.be/abc")
                    .unwrap()
                    .wrap("point".into(), Some(at)),
                "point"
            );
        }
    }
}

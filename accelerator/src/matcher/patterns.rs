use regex::bytes::Regex;
use lazy_static::lazy_static;

/// YouTube URL pattern with metadata
pub struct YouTubePattern {
    pub name: &'static str,
    pub regex: Regex,
    pub priority: u8,
}

lazy_static! {
    /// Compiled regex patterns
    pub static ref YOUTUBE_PATTERNS: Vec<YouTubePattern> = {
        let patterns = vec![
            // Standard formats (high confidence)
            ("standard", r"https?://(?:www\.)?youtube\.com/watch\?v=([\w-]{11})(?:[&?][^\s]*)?", 10),
            ("short", r"https?://youtu\.be/([\w-]{11})(?:\?[^\s]*)?", 10),  // With optional ?t=123
            ("embed", r"https?://(?:www\.)?youtube\.com/embed/([\w-]{11})(?:\?[^\s]*)?", 9),
            ("v_slash", r"https?://(?:www\.)?youtube\.com/v/([\w-]{11})", 8),
            ("shorts", r"https?://(?:www\.)?youtube\.com/shorts/([\w-]{11})(?:\?[^\s]*)?", 10),
            ("live", r"https?://(?:www\.)?youtube\.com/live/([\w-]{11})", 9),
            ("mobile", r"https?://m\.youtube\.com/watch\?v=([\w-]{11})(?:[&?][^\s]*)?", 9),
            ("gaming", r"https?://gaming\.youtube\.com/watch\?v=([\w-]{11})", 8),
            ("music", r"https?://music\.youtube\.com/watch\?v=([\w-]{11})", 8),
            ("studio", r"https?://studio\.youtube\.com/video/([\w-]{11})/edit", 7),
            ("kids", r"https?://www\.youtubekids\.com/watch\?v=([\w-]{11})", 7),
            ("nocookie", r"https?://www\.youtube-nocookie\.com/embed/([\w-]{11})", 8),
            ("attribution", r"attribution_link\?.*v[=/]([\w-]{11})", 6),
            ("google_redirect", r"google\.com/url\?.*youtube.*v[=/]([\w-]{11})", 6),
            ("user_attribution", r"feature=player_embedded.*v=([\w-]{11})", 6),
            ("app_indexing", r"android-app://com\.google\.android\.youtube/http/www\.youtube\.com/watch\?v=([\w-]{11})", 7),
            // Universal v= parameter (catches playlist URLs and edge cases)
            ("v_param", r"[?&]v=([\w-]{11})(?:[&#\s]|$)", 6),
            // Playlist with video
            ("playlist_video", r"youtube\.com/watch\?.*v=([\w-]{11}).*&list=", 8),
            // Loose patterns (higher false positive risk)
            ("video_id_json", r#"["']video_id["']\s*:\s*["']([\w-]{11})["']"#, 5),
            ("data_video_id", r#"data-video-id=["']([\w-]{11})["']"#, 5),
            ("meta_content", r#"<meta itemprop="videoId" content="([\w-]{11})">"#, 6),
        ];
        
        patterns
            .into_iter()
            .map(|(name, pattern, priority)| YouTubePattern {
                name,
                regex: Regex::new(pattern).expect("Invalid regex pattern"),
                priority,
            })
            .collect()
    };
    
    /// Title extraction patterns
    pub static ref TITLE_PATTERNS: Vec<Regex> = {
        vec![
            r"<title>(.*?)(?:\s*-\s*YouTube)?</title>",
            r#""title"\s*:\s*"((?:[^"\\]|\\.)*)""#,
            r#"<meta name="title" content="((?:[^"\\]|\\.)*)">"#,
            r#""videoTitle"\s*:\s*"((?:[^"\\]|\\.)*?)""#,
            r#"data-video-title="((?:[^"\\]|\\.)*)""#,
            r"<h1[^>]*>(.*?)</h1>",
        ]
        .into_iter()
        .map(|p| Regex::new(p).expect("Invalid title pattern"))
        .collect()
    };
}

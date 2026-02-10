use regex::Regex;
use std::sync::OnceLock;

/// Extract a meaningful title from file content
pub fn extract_title(data: &[u8], file_type: &str) -> Option<String> {
    // Try to convert to UTF-8 string (lossy)
    let content = String::from_utf8_lossy(data);
    
    match file_type {
        "html" | "htm" => extract_html_title(&content),
        "json" => extract_json_title(&content),
        "txt" | "md" => extract_first_line(&content),
        _ => None,
    }
}

fn extract_html_title(content: &str) -> Option<String> {
    static TITLE_REGEX: OnceLock<Regex> = OnceLock::new();
    let re = TITLE_REGEX.get_or_init(|| Regex::new(r"(?i)<title>(.*?)</title>").unwrap());

    re.captures(content)
        .and_then(|cap| cap.get(1))
        .map(|m| sanitize_filename(m.as_str()))
}

fn extract_json_title(content: &str) -> Option<String> {
    // Simple heuristic for JSON titles
    if content.len() > 1024 * 10 
    { 
        return None; // too big to parse with regex safely
    }

    static NAME_REGEX: OnceLock<Regex> = OnceLock::new();
    let re = NAME_REGEX.get_or_init(|| Regex::new(r#""(title|name)"\s*:\s*"([^"]+)""#).unwrap());

    re.captures(content)
        .and_then(|cap| cap.get(2))
        .map(|m| sanitize_filename(m.as_str()))
}

fn extract_first_line(content: &str) -> Option<String> {
    content.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| sanitize_filename(line))
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect();
    
    let trimmed = sanitized.trim();
    if trimmed.len() > 50 {
        format!("{}...", &trimmed[..47])
    } else {
        trimmed.to_string()
    }
}

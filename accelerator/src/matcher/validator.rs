/// Validates if a byte slice is a valid YouTube video ID
///
/// A valid video ID must be:
/// - Exactly 11 bytes long
/// - Contain only alphanumeric characters, - or _
#[inline]
pub fn is_valid_video_id(id: &[u8]) -> bool {
    if id.len() != 11 {
        return false;
    }
    
    // Check if all characters are valid
    id.iter().all(|&b| {
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_id_validation() {
        assert!(is_valid_video_id(b"dQw4w9WgXcQ"));
        assert!(is_valid_video_id(b"_-123456789"));
        assert!(!is_valid_video_id(b"short"));
        assert!(!is_valid_video_id(b"toolongvideo"));
        assert!(!is_valid_video_id(b"invalid$cha"));
    }
}

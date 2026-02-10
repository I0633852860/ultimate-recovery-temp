use std::borrow::Cow;

/// Clean recovered file content based on file type
pub fn clean_file_content<'a>(data: &'a [u8], file_type: &str) -> Cow<'a, [u8]> {
    match file_type {
        "txt" | "json" | "html" | "css" | "js" | "xml" | "md" => clean_text_content(data),
        _ => Cow::Borrowed(data),
    }
}

/// Clean text content by removing null bytes and non-printable characters
fn clean_text_content(data: &[u8]) -> Cow<'_, [u8]> {
    let needs_cleaning = data.iter().any(|&b| b == 0 || (b < 32 && b != b'\n' && b != b'\r' && b != b'\t'));

    if !needs_cleaning {
        return Cow::Borrowed(data);
    }

    let cleaned: Vec<u8> = data
        .iter()
        .filter(|&&b| b != 0) // Remove nulls
        .map(|&b| {
            if b < 32 && b != b'\n' && b != b'\r' && b != b'\t' {
                b' ' // Replace other control chars with space
            } else {
                b
            }
        })
        .collect();

    Cow::Owned(cleaned)
}

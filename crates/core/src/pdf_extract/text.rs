//! PDF text extraction + tier-based capping (L4e).

use super::ExtractError;
use std::path::Path;

pub const MAX_PDF_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
pub const MIN_TEXT_CHARS: usize = 500;
pub const OLLAMA_CAP_BYTES: usize = 32 * 1024;
pub const CLAUDE_CAP_BYTES: usize = 200 * 1024;
/// Maximum decompressed text characters from a PDF. Roughly 20× the raw byte cap —
/// large enough for legitimate dense manuals (a 10 MB raw PDF typically yields
/// 100-500 KB of text), small enough to reject FlateDecode bombs that expand
/// unbounded.
pub const MAX_DECOMPRESSED_CHARS: usize = 2_000_000;

fn enforce_decompressed_cap(text: &str) -> Result<&str, ExtractError> {
    if text.chars().count() > MAX_DECOMPRESSED_CHARS {
        return Err(ExtractError::TooLarge(MAX_PDF_BYTES / (1024 * 1024)));
    }
    Ok(text)
}

pub fn extract_text_from_pdf(path: &Path) -> Result<String, ExtractError> {
    let meta = std::fs::metadata(path).map_err(|e| ExtractError::ReadFailed(e.to_string()))?;
    if meta.len() > MAX_PDF_BYTES {
        return Err(ExtractError::TooLarge(MAX_PDF_BYTES / (1024 * 1024)));
    }
    let bytes = std::fs::read(path).map_err(|e| ExtractError::ReadFailed(e.to_string()))?;
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| ExtractError::ParseFailed(e.to_string()))?;
    let trimmed = text.trim();
    enforce_decompressed_cap(trimmed)?;
    if trimmed.chars().count() < MIN_TEXT_CHARS {
        return Err(ExtractError::ImageOnly);
    }
    Ok(trimmed.to_string())
}

pub fn cap_for_tier(text: &str, tier_is_claude: bool) -> String {
    let cap = if tier_is_claude {
        CLAUDE_CAP_BYTES
    } else {
        OLLAMA_CAP_BYTES
    };
    if text.len() <= cap {
        return text.to_string();
    }
    let mut cut = cap;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_rejects_oversize_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.pdf");
        let bytes = vec![0u8; (MAX_PDF_BYTES + 1) as usize];
        std::fs::write(&path, bytes).unwrap();
        let err = extract_text_from_pdf(&path).unwrap_err();
        assert!(matches!(err, ExtractError::TooLarge(10)), "got: {:?}", err);
    }

    #[test]
    fn extract_rejects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.pdf");
        let err = extract_text_from_pdf(&path).unwrap_err();
        assert!(matches!(err, ExtractError::ReadFailed(_)), "got: {:?}", err);
    }

    #[test]
    fn cap_for_tier_ollama_caps_at_32kb() {
        let big = "x".repeat(100 * 1024);
        let out = cap_for_tier(&big, false);
        assert!(out.len() <= OLLAMA_CAP_BYTES);
    }

    #[test]
    fn cap_for_tier_claude_caps_at_200kb() {
        let big = "x".repeat(300 * 1024);
        let out = cap_for_tier(&big, true);
        assert!(out.len() <= CLAUDE_CAP_BYTES);
    }

    #[test]
    fn cap_for_tier_returns_whole_when_under_cap() {
        let small = "small text";
        let out = cap_for_tier(small, false);
        assert_eq!(out, small);
    }

    #[test]
    fn cap_for_tier_respects_char_boundary() {
        // Multibyte UTF-8 chars at the byte boundary — must not panic.
        let prefix = "a".repeat(OLLAMA_CAP_BYTES - 2);
        let input = format!("{}€€€", prefix); // € is 3 bytes in UTF-8
        let out = cap_for_tier(&input, false);
        assert!(out.len() <= OLLAMA_CAP_BYTES);
    }

    #[test]
    fn enforce_decompressed_cap_rejects_overlimit_text() {
        let huge = "x".repeat(MAX_DECOMPRESSED_CHARS + 1);
        let err = enforce_decompressed_cap(&huge).unwrap_err();
        assert!(matches!(err, ExtractError::TooLarge(_)), "got: {err:?}");
    }

    #[test]
    fn enforce_decompressed_cap_accepts_at_limit_text() {
        let at_limit = "x".repeat(MAX_DECOMPRESSED_CHARS);
        assert!(enforce_decompressed_cap(&at_limit).is_ok());
    }
}

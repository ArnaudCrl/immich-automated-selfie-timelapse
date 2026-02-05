//! Shared utility functions.

use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

/// Create a safe snake_case folder name from person name or ID.
///
/// This sanitizes user input to create safe filesystem paths by:
/// - Using the person name if provided and non-empty, otherwise falling back to ID
/// - Removing accents and diacritical marks
/// - Converting to lowercase
/// - Replacing spaces with underscores
/// - Replacing unsafe characters with underscores
/// - Trimming whitespace and underscores
/// - Limiting length to 50 characters
///
/// Note: The frontend has a JavaScript version of this function that must be
/// kept in sync. See `frontend/src/lib/components/App.svelte`.
pub fn sanitize_folder_name(name: Option<&str>, id: &str) -> String {
    let base = name.filter(|n| !n.is_empty()).unwrap_or(id);

    // Remove accents by decomposing to NFD and filtering combining marks
    let without_accents: String = base.nfd().filter(|c| !is_combining_mark(*c)).collect();

    // Convert to lowercase and replace unsafe characters with underscores
    let sanitized: String = without_accents
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Trim whitespace/underscores and limit length
    let trimmed = sanitized.trim_matches(|c: char| c.is_whitespace() || c == '_');
    if trimmed.len() > 50 {
        trimmed[..50].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_folder_name_with_name() {
        assert_eq!(sanitize_folder_name(Some("John Doe"), "abc123"), "john_doe");
    }

    #[test]
    fn test_sanitize_folder_name_falls_back_to_id() {
        assert_eq!(sanitize_folder_name(None, "abc123"), "abc123");
        assert_eq!(sanitize_folder_name(Some(""), "abc123"), "abc123");
    }

    #[test]
    fn test_sanitize_folder_name_replaces_unsafe_chars() {
        assert_eq!(
            sanitize_folder_name(Some("John/Doe:Test"), "id"),
            "john_doe_test"
        );
    }

    #[test]
    fn test_sanitize_folder_name_trims_whitespace() {
        assert_eq!(sanitize_folder_name(Some("  John Doe  "), "id"), "john_doe");
    }

    #[test]
    fn test_sanitize_folder_name_limits_length() {
        let long_name = "A".repeat(100);
        let result = sanitize_folder_name(Some(&long_name), "id");
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn test_sanitize_folder_name_removes_accents() {
        // French accents
        assert_eq!(sanitize_folder_name(Some("José"), "id"), "jose");
        assert_eq!(sanitize_folder_name(Some("René"), "id"), "rene");
        assert_eq!(sanitize_folder_name(Some("François"), "id"), "francois");

        // German umlauts
        assert_eq!(sanitize_folder_name(Some("Müller"), "id"), "muller");
        assert_eq!(sanitize_folder_name(Some("Björk"), "id"), "bjork");

        // Spanish
        assert_eq!(sanitize_folder_name(Some("Peña"), "id"), "pena");

        // Mixed accents
        assert_eq!(
            sanitize_folder_name(Some("Élève Français"), "id"),
            "eleve_francais"
        );
    }
}

//! Shared utility functions.

/// Create a safe folder name from person name or ID.
///
/// This sanitizes user input to create safe filesystem paths by:
/// - Using the person name if provided and non-empty, otherwise falling back to ID
/// - Replacing unsafe characters with underscores
/// - Trimming whitespace
/// - Limiting length to 50 characters
///
/// Note: The frontend has a JavaScript version of this function that must be
/// kept in sync. See `frontend/src/lib/components/App.svelte`.
pub fn sanitize_folder_name(name: Option<&str>, id: &str) -> String {
    let base = name.filter(|n| !n.is_empty()).unwrap_or(id);

    // Replace unsafe characters with underscores
    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Trim whitespace and limit length
    let trimmed = sanitized.trim();
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
        assert_eq!(sanitize_folder_name(Some("John Doe"), "abc123"), "John Doe");
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
            "John_Doe_Test"
        );
    }

    #[test]
    fn test_sanitize_folder_name_trims_whitespace() {
        assert_eq!(
            sanitize_folder_name(Some("  John Doe  "), "id"),
            "John Doe"
        );
    }

    #[test]
    fn test_sanitize_folder_name_limits_length() {
        let long_name = "A".repeat(100);
        let result = sanitize_folder_name(Some(&long_name), "id");
        assert_eq!(result.len(), 50);
    }
}

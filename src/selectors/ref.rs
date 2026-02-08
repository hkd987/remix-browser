use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveRefError {
    InvalidFormat(String),
    NotFound(String),
}

impl std::fmt::Display for ResolveRefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(selector) => write!(
                f,
                "Invalid ref selector '{}'. Use ref=eN, [ref=eN], or eN.",
                selector
            ),
            Self::NotFound(ref_id) => write!(f, "Ref '{}' not found, call snapshot again.", ref_id),
        }
    }
}

impl std::error::Error for ResolveRefError {}

fn is_valid_ref_token(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some('e'))
        && chars.as_str().chars().all(|c| c.is_ascii_digit())
        && !chars.as_str().is_empty()
}

pub fn parse_ref(selector: &str) -> Option<String> {
    let trimmed = selector.trim();
    let token = if let Some(inner) = trimmed.strip_prefix("[ref=") {
        inner.strip_suffix(']')?
    } else if let Some(inner) = trimmed.strip_prefix("ref=") {
        inner
    } else {
        trimmed
    };

    if is_valid_ref_token(token) {
        Some(token.to_string())
    } else {
        None
    }
}

pub fn resolve_selector(
    selector: &str,
    refs: &HashMap<String, String>,
) -> Result<String, ResolveRefError> {
    let trimmed = selector.trim();
    if let Some(ref_id) = parse_ref(trimmed) {
        return refs
            .get(&ref_id)
            .cloned()
            .ok_or(ResolveRefError::NotFound(ref_id));
    }

    if trimmed.starts_with("ref=") || trimmed.starts_with("[ref=") {
        return Err(ResolveRefError::InvalidFormat(trimmed.to_string()));
    }

    Ok(selector.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ref_supported_formats() {
        assert_eq!(parse_ref("e12"), Some("e12".to_string()));
        assert_eq!(parse_ref("ref=e12"), Some("e12".to_string()));
        assert_eq!(parse_ref("[ref=e12]"), Some("e12".to_string()));
    }

    #[test]
    fn test_parse_ref_rejects_invalid_values() {
        assert_eq!(parse_ref("ref=foo"), None);
        assert_eq!(parse_ref("[ref=foo]"), None);
        assert_eq!(parse_ref("e"), None);
    }

    #[test]
    fn test_resolve_selector_passthrough_for_css() {
        let refs = HashMap::new();
        let resolved =
            resolve_selector("#login-form", &refs).expect("selector should pass through");
        assert_eq!(resolved, "#login-form");
    }

    #[test]
    fn test_resolve_selector_ref_hit() {
        let mut refs = HashMap::new();
        refs.insert("e3".to_string(), "#submit-btn".to_string());

        let resolved = resolve_selector("[ref=e3]", &refs).expect("ref should resolve");
        assert_eq!(resolved, "#submit-btn");
    }

    #[test]
    fn test_resolve_selector_ref_stale() {
        let refs = HashMap::new();

        let err = resolve_selector("e77", &refs).expect_err("missing ref should error");
        assert_eq!(err, ResolveRefError::NotFound("e77".to_string()));
    }

    #[test]
    fn test_resolve_selector_invalid_explicit_ref() {
        let refs = HashMap::new();

        let err = resolve_selector("ref=foo", &refs).expect_err("invalid ref format should error");
        assert_eq!(err, ResolveRefError::InvalidFormat("ref=foo".to_string()));
    }
}

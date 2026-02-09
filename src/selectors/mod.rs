pub mod css;
pub mod r#ref;
pub mod text;
pub mod xpath;

use anyhow::Result;
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

/// The type of selector to use for element resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SelectorType {
    #[default]
    Css,
    Text,
    Xpath,
}

/// Information about a found element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInfo {
    pub index: usize,
    pub tag: String,
    pub text: String,
    pub attributes: serde_json::Value,
    pub backend_node_id: i64,
}

/// Resolve a selector to matching elements on the page.
pub async fn find_elements(
    page: &Page,
    selector: &str,
    selector_type: &SelectorType,
) -> Result<Vec<ElementInfo>> {
    match selector_type {
        SelectorType::Css => css::find_elements(page, selector).await,
        SelectorType::Text => text::find_elements(page, selector).await,
        SelectorType::Xpath => xpath::find_elements(page, selector).await,
    }
}

/// Resolve a selector and get the first matching element's remote object ID for interaction.
pub async fn resolve_selector(
    _page: &Page,
    selector: &str,
    selector_type: &SelectorType,
) -> Result<String> {
    let js = match selector_type {
        SelectorType::Css => {
            format!(
                r#"(() => {{
                    const el = document.querySelector({sel});
                    if (!el) throw new Error('Element not found: ' + {sel});
                    return el;
                }})()"#,
                sel = serde_json::to_string(selector)?
            )
        }
        SelectorType::Text => {
            format!(
                r#"(() => {{
                    const target = {sel};
                    const walker = document.createTreeWalker(
                        document.body,
                        NodeFilter.SHOW_TEXT,
                        null
                    );
                    while (walker.nextNode()) {{
                        if (walker.currentNode.textContent.trim().toLowerCase().includes(target.toLowerCase())) {{
                            return walker.currentNode.parentElement;
                        }}
                    }}
                    throw new Error('Element with text not found: ' + target);
                }})()"#,
                sel = serde_json::to_string(selector)?
            )
        }
        SelectorType::Xpath => {
            format!(
                r#"(() => {{
                    const result = document.evaluate(
                        {sel},
                        document,
                        null,
                        XPathResult.FIRST_ORDERED_NODE_TYPE,
                        null
                    );
                    const el = result.singleNodeValue;
                    if (!el) throw new Error('XPath not found: ' + {sel});
                    return el;
                }})()"#,
                sel = serde_json::to_string(selector)?
            )
        }
    };

    // We return the JS expression that resolves the element.
    // The caller will use Runtime.evaluate to get a remote object reference.
    Ok(js)
}

/// Helper JS to extract element info from a DOM element.
pub fn element_info_js() -> &'static str {
    r#"(el, index) => {
        const attrs = {};
        for (const attr of el.attributes || []) {
            attrs[attr.name] = attr.value;
        }
        return {
            index: index,
            tag: el.tagName.toLowerCase(),
            text: (el.textContent || '').trim().substring(0, 200),
            attributes: attrs
        };
    }"#
}

/// Detect Playwright-style :has-text("...") and convert to text selector.
pub fn normalize_selector_type(selector: &str, selector_type: SelectorType) -> (String, SelectorType) {
    if matches!(selector_type, SelectorType::Css) {
        if let Some(start) = selector.find(":has-text(") {
            let after = &selector[start + ":has-text(".len()..];
            let (quote, rest) = if let Some(stripped) = after.strip_prefix('"') {
                ('"', stripped)
            } else if let Some(stripped) = after.strip_prefix('\'') {
                ('\'', stripped)
            } else {
                return (selector.to_string(), selector_type);
            };
            if let Some(end) = rest.find(quote) {
                let text = &rest[..end];
                return (text.to_string(), SelectorType::Text);
            }
        }
    }
    (selector.to_string(), selector_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_has_text_double_quotes() {
        let (sel, st) = normalize_selector_type(r#"button:has-text("Submit")"#, SelectorType::Css);
        assert_eq!(sel, "Submit");
        assert!(matches!(st, SelectorType::Text));
    }

    #[test]
    fn test_normalize_has_text_single_quotes() {
        let (sel, st) = normalize_selector_type("button:has-text('Create Account')", SelectorType::Css);
        assert_eq!(sel, "Create Account");
        assert!(matches!(st, SelectorType::Text));
    }

    #[test]
    fn test_normalize_plain_css_unchanged() {
        let (sel, st) = normalize_selector_type("#submit-btn", SelectorType::Css);
        assert_eq!(sel, "#submit-btn");
        assert!(matches!(st, SelectorType::Css));
    }

    #[test]
    fn test_normalize_text_type_unchanged() {
        let (sel, st) = normalize_selector_type(r#"button:has-text("Submit")"#, SelectorType::Text);
        assert_eq!(sel, r#"button:has-text("Submit")"#);
        assert!(matches!(st, SelectorType::Text));
    }

    #[test]
    fn test_normalize_bare_has_text() {
        let (sel, st) = normalize_selector_type(r#":has-text("Login")"#, SelectorType::Css);
        assert_eq!(sel, "Login");
        assert!(matches!(st, SelectorType::Text));
    }
}

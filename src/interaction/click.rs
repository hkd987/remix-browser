use anyhow::{Context, Result};
use chromiumoxide::page::Page;

use crate::selectors::SelectorType;

#[derive(Debug, Clone)]
pub struct ClickResult {
    pub success: bool,
    pub method_used: String,
}

/// Convert a selector + type to a JS expression that resolves to the element.
pub fn selector_to_js(selector: &str, selector_type: &SelectorType) -> Result<String> {
    let sel_str = serde_json::to_string(selector)?;
    Ok(match selector_type {
        SelectorType::Css => format!("document.querySelector({})", sel_str),
        SelectorType::Text => format!(
            r#"(() => {{
                const target = {};
                const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
                while (walker.nextNode()) {{
                    if (walker.currentNode.textContent.trim().includes(target)) {{
                        return walker.currentNode.parentElement;
                    }}
                }}
                return null;
            }})()"#,
            sel_str
        ),
        SelectorType::Xpath => format!(
            r#"document.evaluate({}, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue"#,
            sel_str
        ),
    })
}

/// Hybrid click strategy:
/// 1. Resolve selector to element
/// 2. Scroll into view
/// 3. Get bounding box
/// 4. Check visibility and obstruction
/// 5. Try mouse events if visible, fall back to JS click
pub async fn hybrid_click(
    page: &Page,
    selector: &str,
    selector_type: &SelectorType,
    button: &str,
) -> Result<ClickResult> {
    let selector_js = selector_to_js(selector, selector_type)?;

    // Step 1-4: Resolve element, scroll into view, check visibility, get coordinates
    let check_js = format!(
        r#"(() => {{
            const el = {selector_js};
            if (!el) return {{ error: 'Element not found: ' + {sel_str} }};

            // Scroll into view
            el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});

            // Get bounding rect
            const rect = el.getBoundingClientRect();
            if (rect.width === 0 && rect.height === 0) {{
                return {{ error: 'Element has zero size' }};
            }}

            const centerX = rect.left + rect.width / 2;
            const centerY = rect.top + rect.height / 2;

            // Check visibility
            const style = getComputedStyle(el);
            if (style.display === 'none' || style.visibility === 'hidden' || parseFloat(style.opacity) === 0) {{
                return {{ visible: false, x: centerX, y: centerY }};
            }}

            // Check if element is obscured
            const topEl = document.elementFromPoint(centerX, centerY);
            const isUnobscured = topEl && (el === topEl || el.contains(topEl) || topEl.contains(el));

            return {{
                visible: true,
                unobscured: isUnobscured,
                x: centerX,
                y: centerY
            }};
        }})()"#,
        selector_js = selector_js,
        sel_str = serde_json::to_string(selector)?
    );

    let check_result: serde_json::Value = page
        .evaluate(check_js.as_str())
        .await
        .context("Failed to evaluate click check")?
        .into_value()
        .context("Failed to parse click check result")?;

    if let Some(error) = check_result.get("error").and_then(|e| e.as_str()) {
        anyhow::bail!("{}", error);
    }

    let visible = check_result["visible"].as_bool().unwrap_or(false);
    let unobscured = check_result["unobscured"].as_bool().unwrap_or(false);
    let _x = check_result["x"].as_f64().unwrap_or(0.0);
    let _y = check_result["y"].as_f64().unwrap_or(0.0);

    // Wait a moment for scroll/layout to settle
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    if visible && unobscured {
        // Step 6: Try CDP mouse events
        let _mouse_button = match button {
            "right" => "right",
            "middle" => "middle",
            _ => "left",
        };

        let click_js = format!(
            r#"(() => {{
                // Use CDP-style mouse events via JS as a proxy
                const el = {selector_js};
                const rect = el.getBoundingClientRect();
                const x = rect.left + rect.width / 2;
                const y = rect.top + rect.height / 2;

                // Dispatch mouse events in sequence
                const opts = {{ bubbles: true, cancelable: true, clientX: x, clientY: y, button: {button_num} }};
                el.dispatchEvent(new MouseEvent('mousemove', opts));
                el.dispatchEvent(new MouseEvent('mousedown', opts));
                el.dispatchEvent(new MouseEvent('mouseup', opts));
                el.dispatchEvent(new MouseEvent('click', opts));
                return true;
            }})()"#,
            selector_js = selector_js,
            button_num = match button {
                "right" => 2,
                "middle" => 1,
                _ => 0,
            }
        );

        page.evaluate(click_js.as_str())
            .await
            .context("Failed to dispatch mouse events")?;

        Ok(ClickResult {
            success: true,
            method_used: "mouse_event".to_string(),
        })
    } else {
        // Step 7: Fall back to JS click
        let js_click = format!(
            r#"(() => {{
                const el = {selector_js};
                if (!el) throw new Error('Element not found');
                el.click();
                return true;
            }})()"#,
            selector_js = selector_js
        );

        page.evaluate(js_click.as_str())
            .await
            .context("Failed to JS click")?;

        Ok(ClickResult {
            success: true,
            method_used: "js_click".to_string(),
        })
    }
}

use anyhow::{Context, Result};
use chromiumoxide::page::Page;

use crate::selectors::SelectorType;

/// Scroll an element into view, or scroll the page in a direction.
pub async fn scroll(
    page: &Page,
    selector: Option<&str>,
    selector_type: &SelectorType,
    direction: &str,
    amount: Option<i32>,
) -> Result<()> {
    let amount = amount.unwrap_or(300);

    if let Some(selector) = selector {
        // Scroll element into view
        let selector_js =
            crate::interaction::click::selector_to_js(selector, selector_type)?;
        let js = format!(
            r#"(() => {{
                const el = {selector_js};
                if (!el) throw new Error('Element not found');
                el.scrollIntoView({{ block: 'center', behavior: 'smooth' }});
                return true;
            }})()"#,
            selector_js = selector_js
        );
        page.evaluate(js.as_str())
            .await
            .context("Failed to scroll element into view")?;
    } else {
        // Scroll the page
        let (dx, dy) = match direction {
            "up" => (0, -amount),
            "down" => (0, amount),
            "left" => (-amount, 0),
            "right" => (amount, 0),
            _ => (0, amount),
        };

        let js = format!(
            "window.scrollBy({{ left: {}, top: {}, behavior: 'smooth' }})",
            dx, dy
        );
        page.evaluate(js.as_str())
            .await
            .context("Failed to scroll page")?;
    }

    // Wait for scroll to settle
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    Ok(())
}

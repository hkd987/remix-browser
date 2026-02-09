use anyhow::Result;
use chromiumoxide::page::Page;
use crate::interaction::click::selector_to_js;
use crate::selectors::SelectorType;

/// Wait up to `timeout_ms` for a selector to resolve to a non-null element.
/// Returns Ok(()) when found, Err if timeout.
pub async fn wait_for_selector(
    page: &Page,
    selector: &str,
    selector_type: &SelectorType,
    timeout_ms: u64,
) -> Result<()> {
    let selector_js = selector_to_js(selector, selector_type)?;
    let check_js = format!(
        r#"(() => {{ const el = {selector_js}; return el !== null && el !== undefined; }})()"#,
        selector_js = selector_js
    );

    let interval = 100;
    let mut elapsed = 0u64;
    loop {
        let found: bool = page
            .evaluate(check_js.as_str())
            .await
            .ok()
            .and_then(|r| r.into_value().ok())
            .unwrap_or(false);

        if found { return Ok(()); }
        if elapsed >= timeout_ms {
            anyhow::bail!(
                "Timed out after {}ms waiting for element: {}",
                timeout_ms, selector
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        elapsed += interval;
    }
}

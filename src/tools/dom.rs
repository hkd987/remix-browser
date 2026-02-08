use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

use crate::selectors::{self, SelectorType};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FindElementsParams {
    #[schemars(description = "Selector to find elements")]
    pub selector: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
    #[schemars(description = "Maximum number of results to return (default: 50)")]
    pub max_results: Option<u32>,
}

pub async fn find_elements(
    page: &Page,
    params: &FindElementsParams,
) -> Result<serde_json::Value> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let elements = selectors::find_elements(page, &params.selector, &selector_type).await?;
    let max = params.max_results.unwrap_or(50) as usize;
    let total = elements.len();

    if total > max {
        let truncated = &elements[..max];
        Ok(serde_json::json!({
            "elements": truncated,
            "total": total,
            "showing": max,
            "note": format!("Showing {} of {} results. Use max_results to see more.", max, total)
        }))
    } else {
        Ok(serde_json::to_value(elements)?)
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetTextParams {
    #[schemars(description = "Selector to get text from")]
    pub selector: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
}

pub async fn get_text(page: &Page, params: &GetTextParams) -> Result<String> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let selector_js =
        crate::interaction::click::selector_to_js(&params.selector, &selector_type)?;

    let js = format!(
        r#"(() => {{
            const el = {selector_js};
            if (!el) throw new Error('Element not found: ' + {sel_str});
            return (el.textContent || '').trim();
        }})()"#,
        selector_js = selector_js,
        sel_str = serde_json::to_string(&params.selector)?
    );

    let result: String = page
        .evaluate(js)
        .await
        .context("Failed to get text")?
        .into_value()
        .context("Failed to parse text result")?;

    Ok(result)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetHtmlParams {
    #[schemars(description = "Selector to get HTML from (default: entire page)")]
    pub selector: Option<String>,
    #[schemars(description = "If true, return outer HTML; otherwise inner HTML")]
    pub outer: Option<bool>,
    #[schemars(description = "Maximum character length of returned HTML (default: 50000)")]
    pub max_length: Option<u32>,
}

pub async fn get_html(page: &Page, params: &GetHtmlParams) -> Result<String> {
    let outer = params.outer.unwrap_or(false);

    let js = if let Some(ref selector) = params.selector {
        let sel_str = serde_json::to_string(selector)?;
        if outer {
            format!(
                r#"(() => {{
                    const el = document.querySelector({});
                    if (!el) throw new Error('Element not found');
                    return el.outerHTML;
                }})()"#,
                sel_str
            )
        } else {
            format!(
                r#"(() => {{
                    const el = document.querySelector({});
                    if (!el) throw new Error('Element not found');
                    return el.innerHTML;
                }})()"#,
                sel_str
            )
        }
    } else {
        "document.documentElement.outerHTML".to_string()
    };

    let result: String = page
        .evaluate(js)
        .await
        .context("Failed to get HTML")?
        .into_value()
        .context("Failed to parse HTML result")?;

    let max_len = params.max_length.unwrap_or(50000) as usize;
    if result.len() > max_len {
        let mut truncated = result[..max_len].to_string();
        truncated.push_str(&format!("\n\n...[truncated, showing {}/{} chars]", max_len, result.len()));
        Ok(truncated)
    } else {
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WaitForParams {
    #[schemars(description = "Selector to wait for")]
    pub selector: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
    #[schemars(description = "Timeout in milliseconds (default: 5000)")]
    pub timeout_ms: Option<u64>,
    #[schemars(description = "State to wait for: visible, hidden, or attached")]
    pub state: Option<String>,
}

pub async fn wait_for(page: &Page, params: &WaitForParams) -> Result<bool> {
    let timeout = std::time::Duration::from_millis(params.timeout_ms.unwrap_or(5000));
    let state = params.state.as_deref().unwrap_or("visible");
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let selector_js =
        crate::interaction::click::selector_to_js(&params.selector, &selector_type)?;

    let check_js = match state {
        "hidden" => format!(
            r#"(() => {{
                const el = {selector_js};
                if (!el) return true;
                const style = getComputedStyle(el);
                return style.display === 'none' || style.visibility === 'hidden' || parseFloat(style.opacity) === 0;
            }})()"#,
            selector_js = selector_js
        ),
        "attached" => format!(
            r#"(() => {{
                const el = {selector_js};
                return !!el;
            }})()"#,
            selector_js = selector_js
        ),
        _ => {
            // "visible" (default)
            format!(
                r#"(() => {{
                    const el = {selector_js};
                    if (!el) return false;
                    const style = getComputedStyle(el);
                    const rect = el.getBoundingClientRect();
                    return style.display !== 'none'
                        && style.visibility !== 'hidden'
                        && parseFloat(style.opacity) > 0
                        && rect.width > 0
                        && rect.height > 0;
                }})()"#,
                selector_js = selector_js
            )
        }
    };

    let start = std::time::Instant::now();
    loop {
        let result: bool = match page.evaluate(check_js.as_str()).await {
            Ok(eval_result) => eval_result.into_value().unwrap_or(false),
            Err(_) => false,
        };

        if result {
            return Ok(true);
        }

        if start.elapsed() >= timeout {
            return Ok(false);
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

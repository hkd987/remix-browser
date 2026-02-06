use anyhow::{Context, Result};
use chromiumoxide::page::Page;

use super::ElementInfo;

/// Find elements matching a CSS selector.
pub async fn find_elements(page: &Page, selector: &str) -> Result<Vec<ElementInfo>> {
    let js = format!(
        r#"(() => {{
            const elements = document.querySelectorAll({sel});
            return Array.from(elements).map((el, index) => {{
                const attrs = {{}};
                for (const attr of el.attributes || []) {{
                    attrs[attr.name] = attr.value;
                }}
                return {{
                    index: index,
                    tag: el.tagName.toLowerCase(),
                    text: (el.textContent || '').trim().substring(0, 200),
                    attributes: attrs,
                    backendNodeId: 0
                }};
            }});
        }})()"#,
        sel = serde_json::to_string(selector).unwrap_or_default()
    );

    let result: serde_json::Value = page
        .evaluate(js)
        .await
        .context("Failed to evaluate CSS selector")?
        .into_value()
        .context("Failed to parse CSS selector result")?;

    parse_element_results(&result)
}

fn parse_element_results(value: &serde_json::Value) -> Result<Vec<ElementInfo>> {
    let arr = value.as_array().context("Expected array of elements")?;
    let mut elements = Vec::new();
    for item in arr {
        elements.push(ElementInfo {
            index: item["index"].as_u64().unwrap_or(0) as usize,
            tag: item["tag"].as_str().unwrap_or("").to_string(),
            text: item["text"].as_str().unwrap_or("").to_string(),
            attributes: item["attributes"].clone(),
            backend_node_id: item["backendNodeId"].as_i64().unwrap_or(0),
        });
    }
    Ok(elements)
}

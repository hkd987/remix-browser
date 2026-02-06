use anyhow::{Context, Result};
use chromiumoxide::page::Page;

use super::ElementInfo;

/// Find elements matching an XPath expression.
pub async fn find_elements(page: &Page, xpath: &str) -> Result<Vec<ElementInfo>> {
    let js = format!(
        r#"(() => {{
            const xpath = {xpath};
            const results = [];
            const xpathResult = document.evaluate(
                xpath,
                document,
                null,
                XPathResult.ORDERED_NODE_SNAPSHOT_TYPE,
                null
            );
            for (let i = 0; i < xpathResult.snapshotLength; i++) {{
                const el = xpathResult.snapshotItem(i);
                if (el.nodeType === Node.ELEMENT_NODE) {{
                    const attrs = {{}};
                    for (const attr of el.attributes || []) {{
                        attrs[attr.name] = attr.value;
                    }}
                    results.push({{
                        index: i,
                        tag: el.tagName.toLowerCase(),
                        text: (el.textContent || '').trim().substring(0, 200),
                        attributes: attrs,
                        backendNodeId: 0
                    }});
                }}
            }}
            return results;
        }})()"#,
        xpath = serde_json::to_string(xpath).unwrap_or_default()
    );

    let result: serde_json::Value = page
        .evaluate(js)
        .await
        .context("Failed to evaluate XPath")?
        .into_value()
        .context("Failed to parse XPath result")?;

    let arr = result.as_array().context("Expected array of elements")?;
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

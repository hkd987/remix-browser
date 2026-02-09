use anyhow::{Context, Result};
use chromiumoxide::page::Page;

use super::ElementInfo;

/// Find elements matching text content.
pub async fn find_elements(page: &Page, text: &str) -> Result<Vec<ElementInfo>> {
    let js = format!(
        r#"(() => {{
            const target = {text};
            const results = [];
            const walker = document.createTreeWalker(
                document.body,
                NodeFilter.SHOW_TEXT,
                null
            );
            let index = 0;
            const seen = new Set();
            while (walker.nextNode()) {{
                const node = walker.currentNode;
                if (node.textContent.trim().toLowerCase().includes(target.toLowerCase())) {{
                    const el = node.parentElement;
                    if (el && !seen.has(el)) {{
                        seen.add(el);
                        const attrs = {{}};
                        for (const attr of el.attributes || []) {{
                            attrs[attr.name] = attr.value;
                        }}
                        results.push({{
                            index: index++,
                            tag: el.tagName.toLowerCase(),
                            text: (el.textContent || '').trim().substring(0, 200),
                            attributes: attrs,
                            backendNodeId: 0
                        }});
                    }}
                }}
            }}
            return results;
        }})()"#,
        text = serde_json::to_string(text).unwrap_or_default()
    );

    let result: serde_json::Value = page
        .evaluate(js)
        .await
        .context("Failed to evaluate text selector")?
        .into_value()
        .context("Failed to parse text selector result")?;

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

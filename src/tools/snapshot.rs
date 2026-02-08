use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SnapshotParams {
    #[schemars(description = "CSS selector to scope snapshot to a subtree (default: entire page)")]
    pub selector: Option<String>,
}

pub async fn snapshot(page: &Page, params: &SnapshotParams) -> Result<String> {
    let root_selector = params
        .selector
        .as_deref()
        .unwrap_or("body");

    let sel_str = serde_json::to_string(root_selector)?;

    let js = format!(
        r#"(() => {{
            const root = document.querySelector({sel});
            if (!root) return 'No elements found (selector not matched)';

            const INTERACTIVE_TAGS = new Set([
                'a', 'button', 'input', 'select', 'textarea', 'details', 'summary'
            ]);
            const SEMANTIC_TAGS = new Set([
                'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'nav', 'main', 'header', 'footer'
            ]);

            const lines = [];
            let idx = 0;

            const walker = document.createTreeWalker(
                root,
                NodeFilter.SHOW_ELEMENT,
                {{
                    acceptNode: function(node) {{
                        const tag = node.tagName.toLowerCase();
                        const role = node.getAttribute('role');
                        const style = getComputedStyle(node);

                        // Skip hidden elements
                        if (style.display === 'none' || style.visibility === 'hidden') {{
                            return NodeFilter.FILTER_REJECT;
                        }}

                        // Include interactive elements
                        if (INTERACTIVE_TAGS.has(tag)) return NodeFilter.FILTER_ACCEPT;
                        // Include semantic elements
                        if (SEMANTIC_TAGS.has(tag)) return NodeFilter.FILTER_ACCEPT;
                        // Include elements with explicit roles
                        if (role && ['button', 'link', 'textbox', 'checkbox', 'radio', 'tab', 'menuitem', 'option', 'switch', 'combobox', 'listbox', 'menu', 'dialog', 'alert', 'navigation', 'search'].includes(role)) return NodeFilter.FILTER_ACCEPT;
                        // Include images with alt text
                        if (tag === 'img' && node.getAttribute('alt')) return NodeFilter.FILTER_ACCEPT;
                        // Include labeled elements
                        if (tag === 'label') return NodeFilter.FILTER_ACCEPT;

                        return NodeFilter.FILTER_SKIP;
                    }}
                }}
            );

            // Also check the root itself
            function processNode(node) {{
                const tag = node.tagName.toLowerCase();
                const role = node.getAttribute('role');
                let parts = [`[${{idx}}]`];

                // Tag + type info
                if (tag === 'input') {{
                    const type = node.getAttribute('type') || 'text';
                    parts.push(`input[type=${{type}}]`);
                }} else if (role) {{
                    parts.push(`${{tag}}[role=${{role}}]`);
                }} else {{
                    parts.push(tag);
                }}

                // Text content (truncated)
                const text = (node.textContent || '').trim().replace(/\s+/g, ' ');
                if (text && text.length > 0 && !['input', 'select', 'textarea', 'img'].includes(tag)) {{
                    const truncated = text.length > 60 ? text.substring(0, 60) + '...' : text;
                    parts.push(`"${{truncated}}"`);
                }}

                // Key attributes
                const href = node.getAttribute('href');
                if (href && href !== '#') parts.push(`href="${{href}}"`);

                const placeholder = node.getAttribute('placeholder');
                if (placeholder) parts.push(`placeholder="${{placeholder}}"`);

                const value = node.value !== undefined && node.value !== '' ? node.value : null;
                if (value && ['input', 'textarea', 'select'].includes(tag)) parts.push(`value="${{value}}"`);

                const name = node.getAttribute('name');
                if (name) parts.push(`name="${{name}}"`);

                const ariaLabel = node.getAttribute('aria-label');
                if (ariaLabel) parts.push(`aria-label="${{ariaLabel}}"`);

                const id = node.getAttribute('id');
                if (id) parts.push(`id="${{id}}"`);

                // For select elements, show options
                if (tag === 'select') {{
                    const options = Array.from(node.options || []).map(o => o.textContent.trim()).join(', ');
                    if (options) parts.push(`options=[${{options}}]`);
                }}

                // Disabled state
                if (node.disabled) parts.push('(disabled)');

                // Checked state
                if (node.checked) parts.push('(checked)');

                lines.push(parts.join(' '));
                idx++;
            }}

            let current = walker.currentNode;
            // Process root if it matches
            const rootTag = root.tagName.toLowerCase();
            if (INTERACTIVE_TAGS.has(rootTag) || SEMANTIC_TAGS.has(rootTag)) {{
                processNode(root);
            }}

            while (walker.nextNode()) {{
                processNode(walker.currentNode);
                if (idx >= 200) {{
                    lines.push(`... and more elements (showing first 200)`);
                    break;
                }}
            }}

            return lines.length > 0 ? lines.join('\n') : 'No interactive elements found';
        }})()"#,
        sel = sel_str
    );

    let result: String = page
        .evaluate(js.as_str())
        .await
        .context("Failed to get page snapshot")?
        .into_value()
        .context("Failed to parse snapshot result")?;

    Ok(result)
}

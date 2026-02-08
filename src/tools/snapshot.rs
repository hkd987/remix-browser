use std::collections::HashMap;

use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SnapshotParams {
    #[schemars(description = "CSS selector to scope snapshot to a subtree (default: entire page)")]
    pub selector: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotOutput {
    pub text: String,
    pub refs: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct SnapshotPayload {
    lines: Vec<String>,
    refs: HashMap<String, String>,
    message: Option<String>,
}

pub async fn snapshot_with_refs(page: &Page, params: &SnapshotParams) -> Result<SnapshotOutput> {
    let root_selector = params.selector.as_deref().unwrap_or("body");
    let sel_str = serde_json::to_string(root_selector)?;

    let js = format!(
        r#"(() => {{
            const root = document.querySelector({sel});
            if (!root) {{
                return {{
                    lines: [],
                    refs: {{}},
                    message: 'No elements found (selector not matched)'
                }};
            }}

            const INTERACTIVE_TAGS = new Set([
                'a', 'button', 'input', 'select', 'textarea', 'details', 'summary'
            ]);
            const SEMANTIC_TAGS = new Set([
                'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'nav', 'main', 'header', 'footer'
            ]);
            const ROLE_TAGS = new Set([
                'button', 'link', 'textbox', 'checkbox', 'radio', 'tab', 'menuitem', 'option',
                'switch', 'combobox', 'listbox', 'menu', 'dialog', 'alert', 'navigation', 'search'
            ]);

            const lines = [];
            const refs = {{}};
            let idx = 0;

            function cssEscape(value) {{
                if (window.CSS && typeof window.CSS.escape === 'function') {{
                    return window.CSS.escape(value);
                }}
                return value.replace(/[^a-zA-Z0-9_-]/g, '\\\\$&');
            }}

            function buildSelector(node) {{
                if (!node || node.nodeType !== Node.ELEMENT_NODE) return '';
                if (node.id) return '#' + cssEscape(node.id);

                const parts = [];
                let current = node;
                while (current && current.nodeType === Node.ELEMENT_NODE) {{
                    let part = current.tagName.toLowerCase();
                    if (current.id) {{
                        part += '#' + cssEscape(current.id);
                        parts.unshift(part);
                        break;
                    }}

                    const classNames = (current.getAttribute('class') || '')
                        .trim()
                        .split(/\s+/)
                        .filter(Boolean)
                        .slice(0, 2);
                    if (classNames.length > 0) {{
                        part += '.' + classNames.map(cssEscape).join('.');
                    }}

                    let sibling = current;
                    let nth = 1;
                    while ((sibling = sibling.previousElementSibling)) {{
                        if (sibling.tagName === current.tagName) nth++;
                    }}
                    part += `:nth-of-type(${{nth}})`;
                    parts.unshift(part);

                    current = current.parentElement;
                    if (current === document.body) {{
                        parts.unshift('body');
                        break;
                    }}
                }}

                return parts.join(' > ');
            }}

            function isRelevant(node) {{
                const tag = node.tagName.toLowerCase();
                const role = node.getAttribute('role');
                if (INTERACTIVE_TAGS.has(tag)) return true;
                if (SEMANTIC_TAGS.has(tag)) return true;
                if (role && ROLE_TAGS.has(role)) return true;
                if (tag === 'img' && node.getAttribute('alt')) return true;
                if (tag === 'label') return true;
                return false;
            }}

            function isVisible(node) {{
                const style = getComputedStyle(node);
                if (style.display === 'none' || style.visibility === 'hidden') return false;
                return true;
            }}

            function processNode(node) {{
                if (idx >= 200) return false;

                const tag = node.tagName.toLowerCase();
                const role = node.getAttribute('role');
                const refId = `e${{idx}}`;
                const selector = buildSelector(node);
                refs[refId] = selector;

                const parts = [`[${{idx}}]`, `[ref=${{refId}}]`];

                if (tag === 'input') {{
                    const type = node.getAttribute('type') || 'text';
                    parts.push(`input[type=${{type}}]`);
                }} else if (role) {{
                    parts.push(`${{tag}}[role=${{role}}]`);
                }} else {{
                    parts.push(tag);
                }}

                const text = (node.textContent || '').trim().replace(/\s+/g, ' ');
                if (text && !['input', 'select', 'textarea', 'img'].includes(tag)) {{
                    const truncated = text.length > 60 ? text.slice(0, 60) + '...' : text;
                    parts.push(`"${{truncated}}"`);
                }}

                const href = node.getAttribute('href');
                if (href && href !== '#') parts.push(`href="${{href}}"`);

                const placeholder = node.getAttribute('placeholder');
                if (placeholder) parts.push(`placeholder="${{placeholder}}"`);

                const value = node.value !== undefined && node.value !== '' ? node.value : null;
                if (value && ['input', 'textarea', 'select'].includes(tag)) {{
                    parts.push(`value="${{value}}"`);
                }}

                const name = node.getAttribute('name');
                if (name) parts.push(`name="${{name}}"`);

                const ariaLabel = node.getAttribute('aria-label');
                if (ariaLabel) parts.push(`aria-label="${{ariaLabel}}"`);

                const id = node.getAttribute('id');
                if (id) parts.push(`id="${{id}}"`);

                if (tag === 'select') {{
                    const options = Array.from(node.options || [])
                        .map(o => (o.textContent || '').trim())
                        .filter(Boolean)
                        .join(', ');
                    if (options) parts.push(`options=[${{options}}]`);
                }}

                if (node.disabled) parts.push('(disabled)');
                if (node.checked) parts.push('(checked)');

                lines.push(parts.join(' '));
                idx++;
                return true;
            }}

            const walker = document.createTreeWalker(
                root,
                NodeFilter.SHOW_ELEMENT,
                {{
                    acceptNode: function(node) {{
                        if (!isVisible(node)) return NodeFilter.FILTER_REJECT;
                        if (isRelevant(node)) return NodeFilter.FILTER_ACCEPT;
                        return NodeFilter.FILTER_SKIP;
                    }}
                }}
            );

            if (isVisible(root) && isRelevant(root)) {{
                processNode(root);
            }}

            while (walker.nextNode()) {{
                if (!processNode(walker.currentNode)) {{
                    lines.push('... and more elements (showing first 200)');
                    break;
                }}
            }}

            if (lines.length === 0) {{
                return {{
                    lines: [],
                    refs: {{}},
                    message: 'No interactive elements found'
                }};
            }}

            return {{ lines, refs, message: null }};
        }})()"#,
        sel = sel_str
    );

    let payload: SnapshotPayload = page
        .evaluate(js.as_str())
        .await
        .context("Failed to get page snapshot")?
        .into_value()
        .context("Failed to parse snapshot result")?;

    let text = if let Some(message) = payload.message {
        message
    } else {
        payload.lines.join("\n")
    };

    Ok(SnapshotOutput {
        text,
        refs: payload.refs,
    })
}

pub async fn snapshot(page: &Page, params: &SnapshotParams) -> Result<String> {
    Ok(snapshot_with_refs(page, params).await?.text)
}

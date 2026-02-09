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
            const INTERACTIVE_ROLES = new Set([
                'button', 'link', 'textbox', 'checkbox', 'radio', 'combobox',
                'tab', 'menuitem', 'switch', 'listbox', 'option',
                'slider', 'spinbutton'
            ]);
            const CONTEXT_TAGS = new Set([
                'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'nav', 'main'
            ]);

            const lines = [];
            const refs = {{}};
            let idx = 0;
            let totalElements = 0;

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

            function isVisible(node) {{
                const style = getComputedStyle(node);
                if (style.display === 'none' || style.visibility === 'hidden') return false;
                return true;
            }}

            function getAriaRole(node) {{
                const explicitRole = node.getAttribute('role');
                if (explicitRole) return explicitRole;

                const tag = node.tagName.toLowerCase();
                const type = (node.getAttribute('type') || '').toLowerCase();

                switch (tag) {{
                    case 'a': return node.hasAttribute('href') ? 'link' : null;
                    case 'button': return 'button';
                    case 'input':
                        switch (type) {{
                            case 'submit': case 'reset': case 'button': return 'button';
                            case 'checkbox': return 'checkbox';
                            case 'radio': return 'radio';
                            case 'number': return 'spinbutton';
                            case 'range': return 'slider';
                            case 'file': return 'button';
                            case 'hidden': return null;
                            default: return 'textbox';
                        }}
                    case 'textarea': return 'textbox';
                    case 'select': return 'combobox';
                    case 'h1': case 'h2': case 'h3': case 'h4': case 'h5': case 'h6': return 'heading';
                    case 'nav': return 'navigation';
                    case 'main': return 'main';
                    case 'img': return node.getAttribute('alt') ? 'img' : null;
                    case 'details': return 'group';
                    case 'summary': return 'button';
                    default: return null;
                }}
            }}

            function isInteractive(node) {{
                const tag = node.tagName.toLowerCase();
                const type = (node.getAttribute('type') || '').toLowerCase();
                if (tag === 'input' && type === 'hidden') return false;
                if (INTERACTIVE_TAGS.has(tag)) return true;
                const role = node.getAttribute('role');
                if (role && INTERACTIVE_ROLES.has(role)) return true;
                return false;
            }}

            function getAccessibleName(node) {{
                // 1. aria-labelledby
                const labelledBy = node.getAttribute('aria-labelledby');
                if (labelledBy) {{
                    const parts = labelledBy.split(/\s+/).map(function(id) {{
                        const el = document.getElementById(id);
                        return el ? (el.textContent || '').trim() : '';
                    }}).filter(Boolean);
                    if (parts.length) {{
                        const text = parts.join(' ');
                        return text.length > 60 ? text.slice(0, 60) + '...' : text;
                    }}
                }}

                // 2. aria-label
                const ariaLabel = node.getAttribute('aria-label');
                if (ariaLabel) return ariaLabel.trim();

                const tag = node.tagName.toLowerCase();
                const type = (node.getAttribute('type') || '').toLowerCase();

                if (tag === 'input' && type === 'file') {{
                    return 'Choose file';
                }}

                // 3. <label for="id"> association
                if (['input', 'select', 'textarea'].includes(tag) && node.id) {{
                    const label = root.querySelector('label[for="' + cssEscape(node.id) + '"]');
                    if (label) {{
                        const text = (label.textContent || '').trim().replace(/\s+/g, ' ');
                        if (text) return text.length > 60 ? text.slice(0, 60) + '...' : text;
                    }}
                }}

                // 4. Wrapping <label> parent
                if (['input', 'select', 'textarea'].includes(tag)) {{
                    const parentLabel = node.closest('label');
                    if (parentLabel) {{
                        const clone = parentLabel.cloneNode(true);
                        clone.querySelectorAll('input, select, textarea').forEach(function(el) {{ el.remove(); }});
                        const text = (clone.textContent || '').trim().replace(/\s+/g, ' ');
                        if (text) return text.length > 60 ? text.slice(0, 60) + '...' : text;
                    }}
                }}

                // 5. textContent for non-form elements
                if (!['input', 'select', 'textarea', 'img'].includes(tag)) {{
                    const text = (node.textContent || '').trim().replace(/\s+/g, ' ');
                    if (text) {{
                        return text.length > 60 ? text.slice(0, 60) + '...' : text;
                    }}
                }}

                // 6. img alt
                if (tag === 'img') {{
                    const alt = node.getAttribute('alt');
                    if (alt) return alt.trim();
                }}

                // 7. placeholder
                const placeholder = node.getAttribute('placeholder');
                if (placeholder) return placeholder.trim();

                // 8. value for form elements
                const value = node.value !== undefined && node.value !== '' ? String(node.value) : null;
                if (value && ['input', 'textarea'].includes(tag)) return value;

                // 9. alt / title fallbacks
                const alt = node.getAttribute('alt');
                if (alt) return alt.trim();

                const title = node.getAttribute('title');
                if (title) return title.trim();

                // 10. name attribute as last resort (developer-facing but often descriptive)
                if (['input', 'select', 'textarea'].includes(tag)) {{
                    const name = node.getAttribute('name');
                    if (name) return name.replace(/[_\-\[\]]/g, ' ').trim();
                }}

                return '';
            }}

            function isRelevant(node) {{
                const tag = node.tagName.toLowerCase();
                if (tag === 'label') return false;
                const role = getAriaRole(node);
                if (role) return true;
                return false;
            }}

            function processNode(node) {{
                if (totalElements >= 200) return false;

                const tag = node.tagName.toLowerCase();
                const role = getAriaRole(node);
                if (!role) return true;

                const interactive = isInteractive(node);
                const parts = [];

                parts.push(role);

                const name = getAccessibleName(node);
                if (name) {{
                    parts.push(`"${{name}}"`);
                }}

                // Add input type when it's not the default "text"
                if (tag === 'input') {{
                    const inputType = (node.getAttribute('type') || '').toLowerCase();
                    if (inputType && inputType !== 'text') {{
                        parts.push('type=' + inputType);
                    }}
                }}

                // Value + range metadata
                const ariaValueNow = node.getAttribute('aria-valuenow');
                const hasMin = node.getAttribute('aria-valuemin') !== null || node.getAttribute('min') !== null;
                const hasMax = node.getAttribute('aria-valuemax') !== null || node.getAttribute('max') !== null;
                const hasRange = hasMin && hasMax;
                const rangeMin = node.getAttribute('aria-valuemin') || node.getAttribute('min');
                const rangeMax = node.getAttribute('aria-valuemax') || node.getAttribute('max');

                if (tag === 'select') {{
                    const selectedVal = node.value || '';
                    if (selectedVal) {{
                        parts.push(`value="${{selectedVal}}"`);
                    }}
                    const options = Array.from(node.options || [])
                        .map(o => (o.textContent || '').trim())
                        .filter(Boolean);
                    if (options.length > 0) {{
                        parts.push(`[${{options.join(', ')}}]`);
                    }}
                }} else if (ariaValueNow !== null) {{
                    // Custom ARIA widgets (Radix sliders, etc.)
                    if (hasRange) {{
                        parts.push(`value=${{ariaValueNow}} [${{rangeMin}}-${{rangeMax}}]`);
                    }} else {{
                        parts.push(`value=${{ariaValueNow}}`);
                    }}
                }} else if (['input', 'textarea'].includes(tag)) {{
                    const value = node.value !== undefined && node.value !== '' ? String(node.value) : null;
                    if (value && hasRange) {{
                        // Native range/number inputs — show value with range
                        parts.push(`value=${{value}} [${{rangeMin}}-${{rangeMax}}]`);
                    }} else if (value) {{
                        parts.push(`value="${{value}}"`);
                    }}
                }}

                if (node.checked) parts.push('[checked]');
                if (node.disabled) parts.push('[disabled]');
                if (node.required) parts.push('[required]');
                const ariaExpanded = node.getAttribute('aria-expanded');
                if (ariaExpanded === 'true') parts.push('[expanded]');
                if (tag === 'details' && node.open) parts.push('[expanded]');

                if (interactive) {{
                    const refId = `e${{idx}}`;
                    const selector = buildSelector(node);
                    refs[refId] = selector;
                    parts.push(`[ref=${{refId}}]`);
                    idx++;
                }}

                lines.push(parts.join(' '));
                totalElements++;
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

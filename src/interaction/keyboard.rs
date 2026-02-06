use anyhow::{Context, Result};
use chromiumoxide::page::Page;

use crate::selectors::SelectorType;

/// Type text into an element by focusing it and dispatching key events.
pub async fn type_text(
    page: &Page,
    selector: &str,
    selector_type: &SelectorType,
    text: &str,
    clear_first: bool,
) -> Result<()> {
    let selector_js = crate::interaction::click::selector_to_js(selector, selector_type)?;

    let focus_js = format!(
        r#"(() => {{
            const el = {selector_js};
            if (!el) throw new Error('Element not found: ' + {sel_str});
            el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
            el.focus();
            if ({clear}) {{
                el.value = '';
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }}
            return true;
        }})()"#,
        selector_js = selector_js,
        sel_str = serde_json::to_string(selector)?,
        clear = if clear_first { "true" } else { "false" }
    );

    page.evaluate(focus_js.as_str())
        .await
        .context("Failed to focus element")?;

    // Type each character
    let type_js = format!(
        r#"(() => {{
            const el = {selector_js};
            const text = {text};
            // Set value directly and dispatch events
            if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {{
                const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
                    window.HTMLInputElement.prototype, 'value'
                )?.set || Object.getOwnPropertyDescriptor(
                    window.HTMLTextAreaElement.prototype, 'value'
                )?.set;
                if (nativeInputValueSetter) {{
                    nativeInputValueSetter.call(el, el.value + text);
                }} else {{
                    el.value += text;
                }}
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }} else {{
                // For contenteditable elements
                document.execCommand('insertText', false, text);
            }}
            return true;
        }})()"#,
        selector_js = selector_js,
        text = serde_json::to_string(text)?
    );

    page.evaluate(type_js.as_str())
        .await
        .context("Failed to type text")?;

    Ok(())
}

/// Press a key (Enter, Tab, ArrowDown, etc.).
pub async fn press_key(page: &Page, key: &str, modifiers: &[String]) -> Result<()> {
    let key_code = key_to_code(key);
    let js = format!(
        r#"(() => {{
            const el = document.activeElement || document.body;
            const opts = {{
                key: {key},
                code: {code},
                keyCode: {key_code},
                which: {key_code},
                bubbles: true,
                cancelable: true,
                ctrlKey: {ctrl},
                shiftKey: {shift},
                altKey: {alt},
                metaKey: {meta}
            }};
            el.dispatchEvent(new KeyboardEvent('keydown', opts));
            el.dispatchEvent(new KeyboardEvent('keypress', opts));
            el.dispatchEvent(new KeyboardEvent('keyup', opts));
            return true;
        }})()"#,
        key = serde_json::to_string(key)?,
        code = serde_json::to_string(&key_code.0)?,
        key_code = key_code.1,
        ctrl = modifiers.iter().any(|m| m == "ctrl" || m == "control"),
        shift = modifiers.iter().any(|m| m == "shift"),
        alt = modifiers.iter().any(|m| m == "alt"),
        meta = modifiers.iter().any(|m| m == "meta" || m == "command"),
    );

    page.evaluate(js.as_str()).await.context("Failed to press key")?;
    Ok(())
}

fn key_to_code(key: &str) -> (String, u32) {
    match key {
        "Enter" => ("Enter".into(), 13),
        "Tab" => ("Tab".into(), 9),
        "Escape" => ("Escape".into(), 27),
        "Backspace" => ("Backspace".into(), 8),
        "Delete" => ("Delete".into(), 46),
        "ArrowUp" => ("ArrowUp".into(), 38),
        "ArrowDown" => ("ArrowDown".into(), 40),
        "ArrowLeft" => ("ArrowLeft".into(), 37),
        "ArrowRight" => ("ArrowRight".into(), 39),
        "Home" => ("Home".into(), 36),
        "End" => ("End".into(), 35),
        "PageUp" => ("PageUp".into(), 33),
        "PageDown" => ("PageDown".into(), 34),
        "Space" | " " => ("Space".into(), 32),
        _ => (format!("Key{}", key.to_uppercase()), key.chars().next().map(|c| c as u32).unwrap_or(0)),
    }
}

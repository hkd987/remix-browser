use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

use crate::interaction::{click, keyboard, scroll};
use crate::selectors::SelectorType;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClickParams {
    #[schemars(description = "Selector for element to click")]
    pub selector: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
    #[schemars(description = "Mouse button: left, right, or middle")]
    pub button: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClickResult {
    pub success: bool,
    pub method_used: String,
}

pub async fn do_click(page: &Page, params: &ClickParams) -> Result<ClickResult> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let (selector, selector_type) = crate::selectors::normalize_selector_type(&params.selector, selector_type);
    let button = params.button.as_deref().unwrap_or("left");

    let result = click::hybrid_click(page, &selector, &selector_type, button).await?;

    Ok(ClickResult {
        success: result.success,
        method_used: result.method_used,
    })
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TypeTextParams {
    #[schemars(description = "Selector for element to type into")]
    pub selector: String,
    #[schemars(description = "Text to type")]
    pub text: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
    #[schemars(description = "Clear the field before typing")]
    pub clear_first: Option<bool>,
}

pub async fn type_text(page: &Page, params: &TypeTextParams) -> Result<bool> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let (selector, selector_type) = crate::selectors::normalize_selector_type(&params.selector, selector_type);
    let clear_first = params.clear_first.unwrap_or(false);

    keyboard::type_text(
        page,
        &selector,
        &selector_type,
        &params.text,
        clear_first,
    )
    .await?;

    Ok(true)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HoverParams {
    #[schemars(description = "Selector for element to hover over")]
    pub selector: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
}

pub async fn hover(page: &Page, params: &HoverParams) -> Result<bool> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let (selector, selector_type) = crate::selectors::normalize_selector_type(&params.selector, selector_type);
    let selector_js = click::selector_to_js(&selector, &selector_type)?;

    let js = format!(
        r#"(() => {{
            const el = {selector_js};
            if (!el) throw new Error('Element not found');
            el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
            const rect = el.getBoundingClientRect();
            const opts = {{ bubbles: true, clientX: rect.left + rect.width/2, clientY: rect.top + rect.height/2 }};
            el.dispatchEvent(new MouseEvent('mouseenter', opts));
            el.dispatchEvent(new MouseEvent('mouseover', opts));
            el.dispatchEvent(new MouseEvent('mousemove', opts));
            return true;
        }})()"#,
        selector_js = selector_js
    );

    page.evaluate(js.as_str())
        .await
        .context("Failed to hover")?;
    Ok(true)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SelectOptionParams {
    #[schemars(description = "Selector for the <select> element")]
    pub selector: String,
    #[schemars(description = "Value to select")]
    pub value: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
}

pub async fn select_option(page: &Page, params: &SelectOptionParams) -> Result<bool> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let (selector, selector_type) = crate::selectors::normalize_selector_type(&params.selector, selector_type);
    let selector_js = click::selector_to_js(&selector, &selector_type)?;

    let js = format!(
        r#"(() => {{
            const el = {selector_js};
            if (!el) throw new Error('Element not found');
            if (el.tagName !== 'SELECT') throw new Error('Element is not a <select>');
            el.value = {value};
            el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            return true;
        }})()"#,
        selector_js = selector_js,
        value = serde_json::to_string(&params.value)?
    );

    page.evaluate(js.as_str())
        .await
        .context("Failed to select option")?;
    Ok(true)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PressKeyParams {
    #[schemars(description = "Key to press (Enter, Tab, ArrowDown, etc.)")]
    pub key: String,
    #[schemars(description = "Modifier keys (ctrl, shift, alt, meta)")]
    pub modifiers: Option<Vec<String>>,
}

pub async fn press_key(page: &Page, params: &PressKeyParams) -> Result<bool> {
    let modifiers = params.modifiers.as_deref().unwrap_or(&[]);
    keyboard::press_key(page, &params.key, modifiers).await?;
    Ok(true)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScrollParams {
    #[schemars(description = "Selector for element to scroll into view (omit for page scroll)")]
    pub selector: Option<String>,
    #[schemars(description = "Scroll direction: up, down, left, right")]
    pub direction: String,
    #[schemars(description = "Scroll amount in pixels (default: 300)")]
    pub amount: Option<i32>,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
}

pub async fn do_scroll(page: &Page, params: &ScrollParams) -> Result<bool> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    scroll::scroll(
        page,
        params.selector.as_deref(),
        &selector_type,
        &params.direction,
        params.amount,
    )
    .await?;
    Ok(true)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FillParams {
    #[schemars(description = "Selector for the form element")]
    pub selector: String,
    #[schemars(description = "Value to set (text for inputs, 'true'/'false' for checkboxes, numeric string for sliders)")]
    pub value: String,
    #[schemars(description = "Type of selector: css, text, or xpath")]
    pub selector_type: Option<SelectorType>,
}

pub async fn fill(page: &Page, params: &FillParams) -> Result<String> {
    let selector_type = params.selector_type.clone().unwrap_or_default();
    let (selector, selector_type) = crate::selectors::normalize_selector_type(&params.selector, selector_type);

    // Auto-wait for element
    crate::interaction::wait::wait_for_selector(page, &selector, &selector_type, 5000).await?;

    let selector_js = click::selector_to_js(&selector, &selector_type)?;
    let value_json = serde_json::to_string(&params.value)?;

    let js = format!(
        r#"(() => {{
            const el = {selector_js};
            if (!el) throw new Error('Element not found');
            const val = {value_json};
            const tag = el.tagName;

            // SELECT
            if (tag === 'SELECT') {{
                el.value = val;
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return 'selected: ' + val;
            }}

            // CHECKBOX / RADIO
            if (el.type === 'checkbox' || el.type === 'radio') {{
                const want = (val === 'true' || val === '1' || val === 'on');
                if (el.checked !== want) el.click();
                return (el.type) + ': ' + el.checked;
            }}

            // INPUT[type=range] slider
            if (el.type === 'range') {{
                const nativeSetter = Object.getOwnPropertyDescriptor(
                    window.HTMLInputElement.prototype, 'value'
                )?.set;
                if (nativeSetter) nativeSetter.call(el, val);
                else el.value = val;
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return 'range: ' + el.value;
            }}

            // ARIA slider (role="slider" with aria-valuenow)
            if (el.getAttribute('role') === 'slider') {{
                const min = parseFloat(el.getAttribute('aria-valuemin') || '0');
                const max = parseFloat(el.getAttribute('aria-valuemax') || '100');
                const target = parseFloat(val);
                const rect = el.getBoundingClientRect();
                const ratio = (target - min) / (max - min);
                const x = rect.left + rect.width * ratio;
                const y = rect.top + rect.height / 2;
                const opts = {{ bubbles: true, clientX: x, clientY: y }};
                el.dispatchEvent(new PointerEvent('pointerdown', opts));
                el.dispatchEvent(new MouseEvent('mousedown', opts));
                el.dispatchEvent(new PointerEvent('pointermove', opts));
                el.dispatchEvent(new MouseEvent('mousemove', opts));
                el.dispatchEvent(new PointerEvent('pointerup', opts));
                el.dispatchEvent(new MouseEvent('mouseup', opts));
                return 'aria-slider targeted: ' + val;
            }}

            // TEXT INPUT / TEXTAREA (default)
            const nativeSetter = Object.getOwnPropertyDescriptor(
                window.HTMLInputElement.prototype, 'value'
            )?.set || Object.getOwnPropertyDescriptor(
                window.HTMLTextAreaElement.prototype, 'value'
            )?.set;
            if (nativeSetter) nativeSetter.call(el, val);
            else el.value = val;
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            return 'filled: ' + val.substring(0, 50);
        }})()"#,
        selector_js = selector_js,
        value_json = value_json
    );

    let result: String = page
        .evaluate(js.as_str())
        .await
        .context("Failed to fill element")?
        .into_value()
        .unwrap_or_else(|_| "filled".to_string());

    Ok(result)
}

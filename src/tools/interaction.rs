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
    let button = params.button.as_deref().unwrap_or("left");

    let result = click::hybrid_click(page, &params.selector, &selector_type, button).await?;

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
    let clear_first = params.clear_first.unwrap_or(false);

    keyboard::type_text(
        page,
        &params.selector,
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
    let selector_js = click::selector_to_js(&params.selector, &selector_type)?;

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
    let selector_js = click::selector_to_js(&params.selector, &selector_type)?;

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

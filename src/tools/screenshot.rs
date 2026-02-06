use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScreenshotParams {
    #[schemars(description = "CSS selector of element to screenshot (omit for viewport)")]
    pub selector: Option<String>,
    #[schemars(description = "Capture the full scrollable page")]
    pub full_page: Option<bool>,
    #[schemars(description = "Image format: png or jpeg")]
    pub format: Option<String>,
    #[schemars(description = "JPEG quality (1-100, only for jpeg format)")]
    pub quality: Option<u32>,
}

pub async fn screenshot(page: &Page, params: &ScreenshotParams) -> Result<String> {
    use base64::Engine;

    let _format = params.format.as_deref().unwrap_or("png");
    let full_page = params.full_page.unwrap_or(false);

    let bytes = if let Some(ref selector) = params.selector {
        // Screenshot a specific element
        let element = page
            .find_element(selector)
            .await
            .context("Element not found for screenshot")?;
        element
            .screenshot(
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png,
            )
            .await
            .context("Failed to take element screenshot")?
    } else if full_page {
        // Full page screenshot
        // First get the full scroll dimensions
        let dims: serde_json::Value = page
            .evaluate("({ width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight })")
            .await?
            .into_value()?;

        let _width = dims["width"].as_u64().unwrap_or(1280);
        let _height = dims["height"].as_u64().unwrap_or(720);

        // Use the page screenshot with full page option
        page.screenshot(
            chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::builder()
                .capture_beyond_viewport(true)
                .build(),
        )
        .await
        .context("Failed to take full page screenshot")?
    } else {
        // Viewport screenshot
        page.screenshot(
            chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::builder()
                .build(),
        )
        .await
        .context("Failed to take screenshot")?
    };

    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NavigateParams {
    #[schemars(description = "URL to navigate to")]
    pub url: String,
    #[schemars(description = "Wait condition: load, domcontentloaded, or networkidle")]
    pub wait_until: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NavigateResult {
    pub url: String,
    pub title: String,
}

pub async fn navigate(page: &Page, params: &NavigateParams) -> Result<NavigateResult> {
    tracing::info!("Navigating to: {}", params.url);
    page.goto(&params.url)
        .await
        .with_context(|| format!("Failed to navigate to {}", params.url))?;

    // Brief settle time after navigation completes.
    // chromiumoxide's goto() already waits for the page load event.
    // These additional waits handle post-load JS rendering.
    match params.wait_until.as_deref() {
        Some("networkidle") => {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        Some("domcontentloaded") | None => {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        _ => {}
    }

    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();

    Ok(NavigateResult { url, title })
}

pub async fn go_back(page: &Page) -> Result<NavigateResult> {
    page.evaluate("window.history.back()")
        .await
        .context("Failed to go back")?;
    // Settle time for history navigation to update the DOM
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();

    Ok(NavigateResult { url, title })
}

pub async fn go_forward(page: &Page) -> Result<NavigateResult> {
    page.evaluate("window.history.forward()")
        .await
        .context("Failed to go forward")?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();

    Ok(NavigateResult { url, title })
}

pub async fn reload(page: &Page) -> Result<NavigateResult> {
    page.reload().await.context("Failed to reload")?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();

    Ok(NavigateResult { url, title })
}

#[derive(Debug, Serialize)]
pub struct PageInfo {
    pub url: String,
    pub title: String,
    pub viewport_size: ViewportSize,
}

#[derive(Debug, Serialize)]
pub struct ViewportSize {
    pub width: u32,
    pub height: u32,
}

pub async fn get_page_info(page: &Page) -> Result<PageInfo> {
    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();

    let viewport: serde_json::Value = page
        .evaluate(
            "({ width: window.innerWidth, height: window.innerHeight })",
        )
        .await?
        .into_value()?;

    Ok(PageInfo {
        url,
        title,
        viewport_size: ViewportSize {
            width: viewport["width"].as_u64().unwrap_or(1280) as u32,
            height: viewport["height"].as_u64().unwrap_or(720) as u32,
        },
    })
}

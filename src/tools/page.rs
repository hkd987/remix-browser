use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::browser::BrowserSession;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NewTabParams {
    #[schemars(description = "URL to open in the new tab")]
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TabInfo {
    pub tab_id: String,
    pub url: String,
    pub title: String,
}

pub async fn new_tab(session: &BrowserSession, params: &NewTabParams) -> Result<String> {
    let url = params.url.as_deref().unwrap_or("about:blank");
    let page = session.new_page(url).await?;
    Ok(page.target_id().as_ref().to_string())
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CloseTabParams {
    #[schemars(description = "ID of the tab to close (closes active tab if omitted)")]
    pub tab_id: Option<String>,
}

pub async fn close_tab(session: &BrowserSession, params: &CloseTabParams) -> Result<bool> {
    let mut pool = session.pool.lock().await;
    if let Some(ref tab_id) = params.tab_id {
        pool.remove_page(tab_id);
    } else {
        let active = pool.active_page().clone();
        let target_id = active.target_id().as_ref().to_string();
        active.close().await.context("Failed to close tab")?;
        pool.remove_page(&target_id);
    }
    Ok(true)
}

pub async fn list_tabs(session: &BrowserSession) -> Result<Vec<TabInfo>> {
    let pool = session.pool.lock().await;
    let mut tabs = Vec::new();
    for page in pool.list_pages() {
        let url = page.url().await.unwrap_or(None).unwrap_or_default();
        let title = page.get_title().await.unwrap_or(None).unwrap_or_default();
        tabs.push(TabInfo {
            tab_id: page.target_id().as_ref().to_string(),
            url,
            title,
        });
    }
    Ok(tabs)
}

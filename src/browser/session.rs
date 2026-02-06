use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::pool::TabPool;

/// Manages the CDP browser connection and page lifecycle.
pub struct BrowserSession {
    browser: Browser,
    _handler_task: tokio::task::JoinHandle<()>,
    pub pool: Arc<Mutex<TabPool>>,
    headless: bool,
}

impl BrowserSession {
    /// Launch a new browser and establish CDP connection.
    pub async fn launch(headless: bool) -> Result<Self> {
        let mut builder = BrowserConfig::builder();

        if headless {
            builder = builder.arg("--headless=new");
        }

        builder = builder
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-client-side-phishing-detection")
            .arg("--disable-default-apps")
            .arg("--disable-extensions")
            .arg("--disable-hang-monitor")
            .arg("--disable-popup-blocking")
            .arg("--disable-prompt-on-repost")
            .arg("--disable-sync")
            .arg("--disable-translate")
            .arg("--metrics-recording-only")
            .arg("--safebrowsing-disable-auto-update")
            .window_size(1280, 720);

        let config = builder.build().map_err(|e| anyhow::anyhow!("{}", e))?;

        let (browser, mut handler) =
            Browser::launch(config).await.context("Failed to launch Chrome")?;

        let handler_task = tokio::spawn(async move {
            while let Some(_event) = handler.next().await {
                // Process browser events
            }
        });

        // Create initial page
        let page = browser
            .new_page("about:blank")
            .await
            .context("Failed to create initial page")?;

        let pool = Arc::new(Mutex::new(TabPool::new(page)));

        tracing::info!(
            "Browser session started (headless: {})",
            headless
        );

        Ok(Self {
            browser,
            _handler_task: handler_task,
            pool,
            headless,
        })
    }

    /// Get the currently active page.
    pub async fn active_page(&self) -> Result<Page> {
        let pool = self.pool.lock().await;
        Ok(pool.active_page().clone())
    }

    /// Create a new tab/page.
    pub async fn new_page(&self, url: &str) -> Result<Page> {
        let page = self
            .browser
            .new_page(url)
            .await
            .context("Failed to create new page")?;
        let mut pool = self.pool.lock().await;
        pool.add_page(page.clone());
        Ok(page)
    }

    /// Close the browser.
    pub async fn close(self) -> Result<()> {
        // Browser drop will handle cleanup
        drop(self.browser);
        Ok(())
    }

    pub fn is_headless(&self) -> bool {
        self.headless
    }
}

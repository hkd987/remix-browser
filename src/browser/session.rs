use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::handler::Handler;
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
    /// Unique temp dir for this Chrome instance — cleaned up on drop.
    /// `None` for remote connections (connected via `--cdp-url`).
    _user_data_dir: Option<tempfile::TempDir>,
}

/// Spawn a task that drives the CDP event loop (must be polled to keep the connection alive).
fn spawn_handler(mut handler: Handler) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move { while handler.next().await.is_some() {} })
}

impl BrowserSession {
    /// Launch a new browser and establish CDP connection.
    pub async fn launch(headless: bool) -> Result<Self> {
        let user_data_dir = tempfile::tempdir().context("Failed to create temp dir for Chrome")?;

        let mut builder = BrowserConfig::builder().user_data_dir(user_data_dir.path());

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

        let (browser, handler) = Browser::launch(config)
            .await
            .context("Failed to launch Chrome")?;

        let handler_task = spawn_handler(handler);

        let page = browser
            .new_page("about:blank")
            .await
            .context("Failed to create initial page")?;

        let pool = Arc::new(Mutex::new(TabPool::new(page)));

        tracing::info!("Browser launched (headless: {})", headless);

        Ok(Self {
            browser,
            _handler_task: handler_task,
            pool,
            headless,
            _user_data_dir: Some(user_data_dir),
        })
    }

    /// Connect to an already-running browser via a CDP URL.
    ///
    /// Accepts `ws://` WebSocket URLs or `http://` URLs (auto-discovers
    /// the WebSocket URL from the `/json/version` endpoint).
    pub async fn connect(cdp_url: &str) -> Result<Self> {
        let (browser, handler) = Browser::connect(cdp_url)
            .await
            .with_context(|| format!("Failed to connect to browser at {}", cdp_url))?;

        let handler_task = spawn_handler(handler);

        // Use an existing page if available, otherwise create one
        let pages = browser.pages().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list existing pages: {}, creating new tab", e);
            Vec::new()
        });
        let page = if let Some(first_page) = pages.into_iter().next() {
            first_page
        } else {
            browser
                .new_page("about:blank")
                .await
                .context("Failed to create initial page on remote browser")?
        };

        let pool = Arc::new(Mutex::new(TabPool::new(page)));

        tracing::info!("Connected to remote browser at {}", cdp_url);

        Ok(Self {
            browser,
            _handler_task: handler_task,
            pool,
            headless: false,
            _user_data_dir: None,
        })
    }

    /// Whether this session is connected to an external browser (vs locally launched).
    pub fn is_remote(&self) -> bool {
        self._user_data_dir.is_none()
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

    /// Close the browser session.
    /// For remote connections, only disconnects without killing the browser.
    pub async fn close(self) -> Result<()> {
        let mode = if self.is_remote() { "remote" } else { "local" };
        tracing::info!("Closing {} browser session", mode);
        drop(self.browser);
        Ok(())
    }

    pub fn is_headless(&self) -> bool {
        self.headless
    }
}

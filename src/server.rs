use rmcp::model::*;
use rmcp::tool;
use rmcp::{Error as McpError, ServerHandler};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::browser::BrowserSession;
use crate::tools::{dom, interaction, javascript, navigation, network, page, screenshot};

/// The MCP server that routes tool calls to browser automation.
#[derive(Clone)]
pub struct RemixBrowserServer {
    session: Arc<Mutex<Option<BrowserSession>>>,
    console_log: javascript::ConsoleLog,
    network_log: network::NetworkLog,
    headless: bool,
}

impl RemixBrowserServer {
    pub fn new(headless: bool) -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            console_log: javascript::ConsoleLog::new(),
            network_log: network::NetworkLog::new(),
            headless,
        }
    }

    /// Ensure the browser is launched, return a reference to the session.
    async fn ensure_browser(&self) -> Result<(), McpError> {
        let mut session = self.session.lock().await;
        if session.is_none() {
            tracing::info!("Launching browser (headless: {})", self.headless);
            let s = BrowserSession::launch(self.headless).await.map_err(|e| {
                McpError::internal_error(format!("Failed to launch browser: {}", e), None)
            })?;
            *session = Some(s);
        }
        Ok(())
    }

    async fn with_page<F, Fut, T>(&self, f: F) -> Result<T, McpError>
    where
        F: FnOnce(chromiumoxide::page::Page) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        self.ensure_browser().await?;
        let page = {
            let session = self.session.lock().await;
            let session_ref = session.as_ref().unwrap();
            session_ref.active_page().await.map_err(|e| {
                McpError::internal_error(format!("Failed to get active page: {}", e), None)
            })?
            // Lock drops here — other tools can proceed concurrently
        };
        f(page).await.map_err(|e| {
            McpError::internal_error(format!("{}", e), None)
        })
    }

    async fn with_session<F, Fut, T>(&self, f: F) -> Result<T, McpError>
    where
        F: FnOnce(&BrowserSession) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        f(session_ref).await.map_err(|e| {
            McpError::internal_error(format!("{}", e), None)
        })
    }

    fn text_result(msg: impl Into<String>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    fn json_result(value: impl serde::Serialize) -> Result<CallToolResult, McpError> {
        let text = serde_json::to_string_pretty(&value)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    fn image_result(base64_data: String) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::image(base64_data, "image/png")]))
    }
}

#[tool(tool_box)]
impl ServerHandler for RemixBrowserServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "remix-browser: Fast headless Chrome automation via CDP. \
                 Start with `navigate` to open a URL, then use other tools to interact."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool(tool_box)]
impl RemixBrowserServer {
    // ── Navigation ──────────────────────────────────────────────────────

    #[tool(description = "Navigate to a URL. Returns the page URL and title.")]
    async fn navigate(
        &self,
        #[tool(aggr)] params: navigation::NavigateParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { navigation::navigate(&page, &params).await })
            .await?;
        Self::text_result(format!("Navigated to {} — {}", result.title, result.url))
    }

    #[tool(description = "Go back in browser history.")]
    async fn go_back(&self) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { navigation::go_back(&page).await })
            .await?;
        Self::text_result(format!("Navigated back to {} — {}", result.title, result.url))
    }

    #[tool(description = "Go forward in browser history.")]
    async fn go_forward(&self) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { navigation::go_forward(&page).await })
            .await?;
        Self::text_result(format!("Navigated forward to {} — {}", result.title, result.url))
    }

    #[tool(description = "Reload the current page.")]
    async fn reload(&self) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { navigation::reload(&page).await })
            .await?;
        Self::text_result(format!("Reloaded {} — {}", result.title, result.url))
    }

    #[tool(description = "Get current page URL, title, and viewport size.")]
    async fn get_page_info(&self) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { navigation::get_page_info(&page).await })
            .await?;
        Self::text_result(format!(
            "{} — {}\nViewport: {}x{}",
            result.title, result.url, result.viewport_size.width, result.viewport_size.height
        ))
    }

    // ── DOM ─────────────────────────────────────────────────────────────

    #[tool(description = "Find elements matching a selector. Returns array of {index, tag, text, attributes}.")]
    async fn find_elements(
        &self,
        #[tool(aggr)] params: dom::FindElementsParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { dom::find_elements(&page, &params).await })
            .await?;
        Self::json_result(result)
    }

    #[tool(description = "Get text content of an element.")]
    async fn get_text(
        &self,
        #[tool(aggr)] params: dom::GetTextParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { dom::get_text(&page, &params).await })
            .await?;
        Self::text_result(result)
    }

    #[tool(description = "Get HTML content of an element or the entire page.")]
    async fn get_html(
        &self,
        #[tool(aggr)] params: dom::GetHtmlParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { dom::get_html(&page, &params).await })
            .await?;
        Self::text_result(result)
    }

    #[tool(description = "Wait for an element to appear, become visible, or be hidden.")]
    async fn wait_for(
        &self,
        #[tool(aggr)] params: dom::WaitForParams,
    ) -> Result<CallToolResult, McpError> {
        let found = self
            .with_page(|page| async move { dom::wait_for(&page, &params).await })
            .await?;
        if found {
            Self::text_result("Element found")
        } else {
            Self::text_result("Element not found (timeout)")
        }
    }

    // ── Interaction ─────────────────────────────────────────────────────

    #[tool(description = "Click an element. Uses hybrid strategy: mouse events with JS fallback for obscured elements.")]
    async fn click(
        &self,
        #[tool(aggr)] params: interaction::ClickParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { interaction::do_click(&page, &params).await })
            .await?;
        Self::text_result(format!("Clicked element ({})", result.method_used))
    }

    #[tool(description = "Type text into an element.")]
    async fn type_text(
        &self,
        #[tool(aggr)] params: interaction::TypeTextParams,
    ) -> Result<CallToolResult, McpError> {
        self.with_page(|page| async move { interaction::type_text(&page, &params).await })
            .await?;
        Self::text_result("Typed text into element")
    }

    #[tool(description = "Hover over an element.")]
    async fn hover(
        &self,
        #[tool(aggr)] params: interaction::HoverParams,
    ) -> Result<CallToolResult, McpError> {
        self.with_page(|page| async move { interaction::hover(&page, &params).await })
            .await?;
        Self::text_result("Hovered over element")
    }

    #[tool(description = "Select an option from a <select> element.")]
    async fn select_option(
        &self,
        #[tool(aggr)] params: interaction::SelectOptionParams,
    ) -> Result<CallToolResult, McpError> {
        self.with_page(|page| async move { interaction::select_option(&page, &params).await })
            .await?;
        Self::text_result("Selected option")
    }

    #[tool(description = "Press a keyboard key (Enter, Tab, ArrowDown, etc.).")]
    async fn press_key(
        &self,
        #[tool(aggr)] params: interaction::PressKeyParams,
    ) -> Result<CallToolResult, McpError> {
        let key = params.key.clone();
        self.with_page(|page| async move { interaction::press_key(&page, &params).await })
            .await?;
        Self::text_result(format!("Pressed {}", key))
    }

    #[tool(description = "Scroll the page or scroll an element into view.")]
    async fn scroll(
        &self,
        #[tool(aggr)] params: interaction::ScrollParams,
    ) -> Result<CallToolResult, McpError> {
        let direction = params.direction.clone();
        let amount = params.amount.unwrap_or(300);
        self.with_page(|page| async move { interaction::do_scroll(&page, &params).await })
            .await?;
        Self::text_result(format!("Scrolled {} {}px", direction, amount))
    }

    // ── Visual ──────────────────────────────────────────────────────────

    #[tool(description = "Take a screenshot of the page, viewport, or a specific element. Returns base64-encoded image.")]
    async fn screenshot(
        &self,
        #[tool(aggr)] params: screenshot::ScreenshotParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { screenshot::screenshot(&page, &params).await })
            .await?;
        Self::image_result(result)
    }

    // ── JavaScript ──────────────────────────────────────────────────────

    #[tool(description = "Execute a JavaScript expression and return the result.")]
    async fn execute_js(
        &self,
        #[tool(aggr)] params: javascript::ExecuteJsParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { javascript::execute_js(&page, &params).await })
            .await?;
        // Return raw JS result — could be any type
        let text = match &result {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => "null".to_string(),
            other => serde_json::to_string_pretty(other)
                .unwrap_or_else(|_| format!("{:?}", other)),
        };
        Self::text_result(text)
    }

    #[tool(description = "Read console log entries. Can filter by level and pattern.")]
    async fn read_console(
        &self,
        #[tool(aggr)] params: javascript::ReadConsoleParams,
    ) -> Result<CallToolResult, McpError> {
        let result = javascript::read_console(&self.console_log, &params)
            .await
            .map_err(|e| McpError::internal_error(format!("{}", e), None))?;
        Self::json_result(result)
    }

    // ── Network ─────────────────────────────────────────────────────────

    #[tool(description = "Enable network request/response capture.")]
    async fn network_enable(
        &self,
        #[tool(aggr)] params: network::NetworkEnableParams,
    ) -> Result<CallToolResult, McpError> {
        network::network_enable(&self.network_log, &params)
            .await
            .map_err(|e| McpError::internal_error(format!("{}", e), None))?;
        Self::text_result("Network capture enabled")
    }

    #[tool(description = "Get captured network requests. Filter by URL pattern, method, or status code.")]
    async fn get_network_log(
        &self,
        #[tool(aggr)] params: network::GetNetworkLogParams,
    ) -> Result<CallToolResult, McpError> {
        let result = network::get_network_log(&self.network_log, &params)
            .await
            .map_err(|e| McpError::internal_error(format!("{}", e), None))?;
        Self::json_result(result)
    }

    // ── Tabs ────────────────────────────────────────────────────────────

    #[tool(description = "Open a new browser tab.")]
    async fn new_tab(
        &self,
        #[tool(aggr)] params: page::NewTabParams,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        let tab_id = page::new_tab(session_ref, &params).await.map_err(|e| {
            McpError::internal_error(format!("{}", e), None)
        })?;
        Self::text_result(format!("Opened new tab: {}", tab_id))
    }

    #[tool(description = "Close a browser tab.")]
    async fn close_tab(
        &self,
        #[tool(aggr)] params: page::CloseTabParams,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        page::close_tab(session_ref, &params).await.map_err(|e| {
            McpError::internal_error(format!("{}", e), None)
        })?;
        Self::text_result("Closed tab")
    }

    #[tool(description = "List all open browser tabs.")]
    async fn list_tabs(&self) -> Result<CallToolResult, McpError> {
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        let result = page::list_tabs(session_ref).await.map_err(|e| {
            McpError::internal_error(format!("{}", e), None)
        })?;
        Self::json_result(result)
    }
}

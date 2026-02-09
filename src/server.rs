use rmcp::model::*;
use rmcp::tool;
use rmcp::{Error as McpError, ServerHandler};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::browser::BrowserSession;
use crate::selectors::r#ref::{resolve_selector, ResolveRefError};
use crate::tools::{
    dom, interaction, javascript, navigation, network, page, screenshot, script, snapshot,
};

const SERVER_INSTRUCTIONS: &str = "remix-browser provides headless Chrome browser automation via CDP. \
Use these tools when the user wants to use Chrome, use the browser, browse the web, open a URL, \
take screenshots, interact with web pages, fill forms, scrape content, debug UIs, inspect the DOM, \
run JavaScript in the browser, or monitor network requests. \
Use `run_script` for ALL browser interactions — it is much faster than individual tool calls. \
A snapshot is automatically appended after every script, so refs ([ref=eN]) are immediately available. \
Strategy: do the first action with a short script to learn the UI, then batch remaining repetitive work \
into a single run_script with a loop. Use granular tools (click, type_text, etc.) only for 1-2 simple follow-up actions.";

fn format_navigation_response(
    result: &navigation::NavigateResult,
    snapshot_text: Option<&str>,
) -> String {
    match snapshot_text {
        Some(snapshot_text) => {
            format!(
                "Navigated to {} — {}\n\n{}",
                result.title, result.url, snapshot_text
            )
        }
        None => format!("Navigated to {} — {}", result.title, result.url),
    }
}

/// The MCP server that routes tool calls to browser automation.
#[derive(Clone)]
pub struct RemixBrowserServer {
    session: Arc<Mutex<Option<BrowserSession>>>,
    console_log: javascript::ConsoleLog,
    network_log: network::NetworkLog,
    snapshot_refs: Arc<Mutex<HashMap<String, String>>>,
    headless: bool,
}

impl RemixBrowserServer {
    pub fn new(headless: bool) -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            console_log: javascript::ConsoleLog::new(),
            network_log: network::NetworkLog::new(),
            snapshot_refs: Arc::new(Mutex::new(HashMap::new())),
            headless,
        }
    }

    /// Explicitly shut down the browser session, killing Chrome.
    pub async fn shutdown(&self) {
        let session_to_close = {
            let mut session = self.session.lock().await;
            session.take()
        };
        if let Some(s) = session_to_close {
            if let Err(e) = s.close().await {
                tracing::warn!("Failed to close browser: {}", e);
            }
        }
        self.clear_snapshot_refs().await;
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
        f(page)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))
    }

    async fn with_session<F, Fut, T>(&self, f: F) -> Result<T, McpError>
    where
        F: FnOnce(&BrowserSession) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        f(session_ref)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))
    }

    fn text_result(msg: impl Into<String>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    fn json_result(value: impl serde::Serialize) -> Result<CallToolResult, McpError> {
        let text = serde_json::to_string(&value)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    fn image_result(base64_data: String) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::image(
            base64_data,
            "image/png",
        )]))
    }

    async fn clear_snapshot_refs(&self) {
        self.snapshot_refs.lock().await.clear();
    }

    async fn set_snapshot_refs(&self, refs: HashMap<String, String>) {
        *self.snapshot_refs.lock().await = refs;
    }

    async fn normalize_selector(&self, selector: &str) -> Result<String, McpError> {
        let refs = self.snapshot_refs.lock().await;
        match resolve_selector(selector, &refs) {
            Ok(resolved) => Ok(resolved),
            Err(ResolveRefError::NotFound(ref_id)) => Err(McpError::internal_error(
                format!("Ref '{}' not found, call snapshot again.", ref_id),
                None,
            )),
            Err(err) => Err(McpError::internal_error(format!("{}", err), None)),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for RemixBrowserServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(SERVER_INSTRUCTIONS.into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool(tool_box)]
impl RemixBrowserServer {
    // ── Navigation ──────────────────────────────────────────────────────

    #[tool(
        description = "Navigate to a URL. Returns page URL and title, and optionally includes a snapshot of interactive elements."
    )]
    async fn navigate(
        &self,
        #[tool(aggr)] params: navigation::NavigateParams,
    ) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
        let include_snapshot = params.include_snapshot;
        let result = self
            .with_page(|page| async move {
                let nav = navigation::navigate(&page, &params).await?;
                if include_snapshot {
                    let snap_params = snapshot::SnapshotParams { selector: None };
                    let snap = snapshot::snapshot_with_refs(&page, &snap_params).await.ok();
                    Ok((nav, snap))
                } else {
                    Ok((nav, None))
                }
            })
            .await?;

        let snapshot_text = if include_snapshot {
            match &result.1 {
                Some(snap) => {
                    self.set_snapshot_refs(snap.refs.clone()).await;
                    Some(snap.text.as_str())
                }
                None => Some("Snapshot unavailable"),
            }
        } else {
            None
        };

        Self::text_result(format_navigation_response(&result.0, snapshot_text))
    }

    #[tool(description = "Go back in browser history.")]
    async fn go_back(&self) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
        let result = self
            .with_page(|page| async move { navigation::go_back(&page).await })
            .await?;
        Self::text_result(format!(
            "Navigated back to {} — {}",
            result.title, result.url
        ))
    }

    #[tool(description = "Go forward in browser history.")]
    async fn go_forward(&self) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
        let result = self
            .with_page(|page| async move { navigation::go_forward(&page).await })
            .await?;
        Self::text_result(format!(
            "Navigated forward to {} — {}",
            result.title, result.url
        ))
    }

    #[tool(description = "Reload the current page.")]
    async fn reload(&self) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
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

    #[tool(
        description = "Find elements matching a selector. Returns array of {index, tag, text, attributes}."
    )]
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
        let mut params = params;
        params.selector = self.normalize_selector(&params.selector).await?;
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

    #[tool(
        description = "Get a compact snapshot of interactive elements on the page. Returns indexed elements with stable refs like [ref=e0]. Use ref=eN selectors with click/type_text/get_text/wait_for."
    )]
    async fn snapshot(
        &self,
        #[tool(aggr)] params: snapshot::SnapshotParams,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .with_page(|page| async move { snapshot::snapshot_with_refs(&page, &params).await })
            .await?;
        self.set_snapshot_refs(result.refs).await;
        Self::text_result(result.text)
    }

    #[tool(description = "Wait for an element to appear, become visible, or be hidden.")]
    async fn wait_for(
        &self,
        #[tool(aggr)] params: dom::WaitForParams,
    ) -> Result<CallToolResult, McpError> {
        let mut params = params;
        params.selector = self.normalize_selector(&params.selector).await?;
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

    #[tool(
        description = "Click an element. Uses hybrid strategy: mouse events with JS fallback for obscured elements."
    )]
    async fn click(
        &self,
        #[tool(aggr)] params: interaction::ClickParams,
    ) -> Result<CallToolResult, McpError> {
        let mut params = params;
        params.selector = self.normalize_selector(&params.selector).await?;
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
        let mut params = params;
        params.selector = self.normalize_selector(&params.selector).await?;
        self.with_page(|page| async move { interaction::type_text(&page, &params).await })
            .await?;
        Self::text_result("Typed text into element")
    }

    #[tool(description = "Hover over an element.")]
    async fn hover(
        &self,
        #[tool(aggr)] params: interaction::HoverParams,
    ) -> Result<CallToolResult, McpError> {
        let mut params = params;
        params.selector = self.normalize_selector(&params.selector).await?;
        self.with_page(|page| async move { interaction::hover(&page, &params).await })
            .await?;
        Self::text_result("Hovered over element")
    }

    #[tool(description = "Select an option from a <select> element.")]
    async fn select_option(
        &self,
        #[tool(aggr)] params: interaction::SelectOptionParams,
    ) -> Result<CallToolResult, McpError> {
        let mut params = params;
        params.selector = self.normalize_selector(&params.selector).await?;
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

    #[tool(
        description = "Take a screenshot of the page, viewport, or a specific element. Returns base64-encoded image."
    )]
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
            other => serde_json::to_string(other).unwrap_or_else(|_| format!("{:?}", other)),
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
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;
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
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        // Wire up CDP event listeners on the active page
        let log = self.network_log.clone();
        self.with_page(|page| async move { network::start_listening(&page, log).await })
            .await?;

        Self::text_result("Network capture enabled")
    }

    #[tool(
        description = "Get captured network requests. Filter by URL pattern, method, or status code."
    )]
    async fn get_network_log(
        &self,
        #[tool(aggr)] params: network::GetNetworkLogParams,
    ) -> Result<CallToolResult, McpError> {
        let result = network::get_network_log(&self.network_log, &params)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;
        Self::json_result(result)
    }

    // ── Tabs ────────────────────────────────────────────────────────────

    #[tool(description = "Open a new browser tab.")]
    async fn new_tab(
        &self,
        #[tool(aggr)] params: page::NewTabParams,
    ) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        let tab_id = page::new_tab(session_ref, &params)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;
        Self::text_result(format!("Opened new tab: {}", tab_id))
    }

    #[tool(description = "Close a browser tab.")]
    async fn close_tab(
        &self,
        #[tool(aggr)] params: page::CloseTabParams,
    ) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        page::close_tab(session_ref, &params)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;
        Self::text_result("Closed tab")
    }

    #[tool(description = "List all open browser tabs.")]
    async fn list_tabs(&self) -> Result<CallToolResult, McpError> {
        self.ensure_browser().await?;
        let session = self.session.lock().await;
        let session_ref = session.as_ref().unwrap();
        let result = page::list_tabs(session_ref)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;
        Self::json_result(result)
    }

    // ── Scripting ──────────────────────────────────────────────────────

    #[tool(
        description = "Execute a JavaScript automation script with access to a `page` object. \
        MUCH faster than individual tool calls for multi-step workflows. \
        Runs synchronously (no await needed). \
        A snapshot of interactive elements is automatically appended after the script finishes. \
        Refs from the snapshot ([ref=eN]) can be used with click/type_text/get_text tools afterwards.\
        \n\n**Strategy**: First do 1 action with a short script to learn the UI selectors, \
        then batch remaining repetitive work into a single run_script with a loop.\
        \n\nAvailable API:\n\
        - page.navigate(url), page.back(), page.forward(), page.reload()\n\
        - page.click(selector, {type:'text'}), page.type(selector, text, {clear:true})\n\
        - page.press(key, {modifiers:['ctrl']}), page.hover(selector), page.select(selector, value)\n\
        - page.scroll(direction, {amount:500}), page.wait(ms), page.waitFor(selector, {timeout:5000})\n\
        - page.snapshot(), page.screenshot(), page.getText(selector), page.getHtml()\n\
        - page.findElements(selector), page.js(expr), console.log(...)\n\
        - page.readConsole(), page.enableNetwork(), page.getNetworkLog()\n\
        \n\nExample — fill and submit a form, then batch-process a list:\n\
        page.navigate('https://example.com');\n\
        page.type('#email', 'user@test.com');\n\
        page.type('#password', 'secret');\n\
        page.click('Submit', {type:'text'});\n\
        page.wait(2000);\n\
        \n\
        const items = ['item1', 'item2', 'item3'];\n\
        for (const item of items) {\n\
          page.type('input[name=search]', item, {clear:true});\n\
          page.wait(1000);\n\
          page.click(item, {type:'text'});\n\
          page.wait(500);\n\
          page.click('Save', {type:'text'});\n\
          page.wait(1000);\n\
        }"
    )]
    async fn run_script(
        &self,
        #[tool(aggr)] params: script::RunScriptParams,
    ) -> Result<CallToolResult, McpError> {
        self.clear_snapshot_refs().await;
        let console_log = self.console_log.clone();
        let network_log = self.network_log.clone();
        let (result, screenshot_contents, script_refs) = self
            .with_page(|page| async move {
                script::run_script(&page, &params, &console_log, &network_log).await
            })
            .await?;

        // Auto-snapshot: capture page state after script completes
        let auto_snap = self
            .with_page(|page| async move {
                let snap_params = snapshot::SnapshotParams { selector: None };
                snapshot::snapshot_with_refs(&page, &snap_params).await
            })
            .await
            .ok();

        // Set refs from auto-snapshot (preferred) or from script's last page.snapshot() call
        if let Some(ref snap) = auto_snap {
            self.set_snapshot_refs(snap.refs.clone()).await;
        } else if let Some(refs) = script_refs {
            self.set_snapshot_refs(refs).await;
        }

        // Build output with auto-snapshot appended
        let output_text = match &auto_snap {
            Some(snap) => format!(
                "{}\n\nPage state:\n{}",
                result.format_output(),
                snap.text
            ),
            None => result.format_output(),
        };

        let mut contents: Vec<Content> = vec![Content::text(output_text)];
        contents.extend(screenshot_contents);
        Ok(CallToolResult::success(contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_nav_result() -> navigation::NavigateResult {
        navigation::NavigateResult {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
        }
    }

    #[test]
    fn test_server_instructions_include_script_first_policy() {
        assert!(SERVER_INSTRUCTIONS.contains("run_script"));
        assert!(SERVER_INSTRUCTIONS.contains("automatically appended"));
        assert!(SERVER_INSTRUCTIONS.contains("batch remaining repetitive work"));
    }

    #[test]
    fn test_format_navigation_response_with_snapshot() {
        let text = format_navigation_response(&sample_nav_result(), Some("[0] [ref=e0] button"));
        assert!(text.contains("Navigated to Example — https://example.com"));
        assert!(text.contains("[ref=e0]"));
    }

    #[test]
    fn test_format_navigation_response_without_snapshot() {
        let text = format_navigation_response(&sample_nav_result(), None);
        assert_eq!(text, "Navigated to Example — https://example.com");
    }

    #[tokio::test]
    async fn test_normalize_selector_resolves_snapshot_ref() {
        let server = RemixBrowserServer::new(true);
        let refs = HashMap::from([("e4".to_string(), "#submit-btn".to_string())]);
        server.set_snapshot_refs(refs).await;

        let resolved = server
            .normalize_selector("ref=e4")
            .await
            .expect("selector should resolve");

        assert_eq!(resolved, "#submit-btn");
    }

    #[tokio::test]
    async fn test_normalize_selector_stale_ref_has_guidance() {
        let server = RemixBrowserServer::new(true);

        let err = server
            .normalize_selector("e99")
            .await
            .expect_err("missing ref should error");

        assert!(format!("{}", err).contains("Ref 'e99' not found, call snapshot again."));
    }
}

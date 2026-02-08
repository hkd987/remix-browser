use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams, EventRequestWillBeSent, EventResponseReceived,
};
use futures::StreamExt;
use remix_browser::selectors::r#ref::resolve_selector as resolve_ref_selector;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn launch_test_browser() -> (Browser, tokio::task::JoinHandle<()>, tempfile::TempDir) {
    let tmp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = BrowserConfig::builder()
        .arg("--headless=new")
        .arg("--no-sandbox")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg("--disable-popup-blocking")
        .user_data_dir(tmp_dir.path())
        .window_size(1280, 720)
        .build()
        .expect("Failed to build browser config");

    let (browser, mut handler) = Browser::launch(config)
        .await
        .expect("Failed to launch browser");

    let handle = tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    // Keep tmp_dir alive — it gets cleaned up on drop
    (browser, handle, tmp_dir)
}

fn fixture_url(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest_dir).join("fixtures").join(name);
    format!("file://{}", path.display())
}

// ── Navigation Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_navigate_to_page() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let url = fixture_url("basic.html");
    page.goto(&url).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let title = page.get_title().await.unwrap().unwrap_or_default();
    assert_eq!(title, "Basic Test Page");

    let current_url = page.url().await.unwrap().unwrap_or_default();
    assert!(current_url.contains("basic.html"));
}

#[tokio::test]
async fn test_get_page_content() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = page.content().await.unwrap();
    assert!(content.contains("Hello, remix-browser!"));
    assert!(content.contains("Important text"));
}

#[tokio::test]
async fn test_navigate_history() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    page.goto(fixture_url("form.html").as_str()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let title = page.get_title().await.unwrap().unwrap_or_default();
    assert_eq!(title, "Form Test Page");

    let _: serde_json::Value = page
        .evaluate("window.history.back()")
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let title = page.get_title().await.unwrap().unwrap_or_default();
    assert_eq!(title, "Basic Test Page");
}

// ── Interaction Tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_click_element() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let _: serde_json::Value = page
        .evaluate(r#"document.getElementById('test-link').click()"#)
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let result: String = page
        .evaluate(r#"document.getElementById('click-result').textContent"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(result, "Link was clicked!");
}

#[tokio::test]
async fn test_type_into_input() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("form.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let _: serde_json::Value = page
        .evaluate(
            r#"(() => {
            const el = document.getElementById('name');
            el.focus();
            el.value = 'Test User';
            el.dispatchEvent(new Event('input', { bubbles: true }));
        })()"#,
        )
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    let value: String = page
        .evaluate(r#"document.getElementById('name').value"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(value, "Test User");
}

#[tokio::test]
async fn test_select_option() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("form.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let _: serde_json::Value = page
        .evaluate(
            r#"(() => {
            const el = document.getElementById('color');
            el.value = 'blue';
            el.dispatchEvent(new Event('change', { bubbles: true }));
        })()"#,
        )
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    let value: String = page
        .evaluate(r#"document.getElementById('color').value"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(value, "blue");
}

// ── Screenshot Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_take_screenshot() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let screenshot = page
        .screenshot(
            chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::builder().build(),
        )
        .await
        .unwrap();

    assert!(!screenshot.is_empty());
    assert_eq!(&screenshot[0..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[tokio::test]
async fn test_element_screenshot() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let element = page.find_element("#title").await.unwrap();
    let screenshot = element
        .screenshot(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
        .await
        .unwrap();

    assert!(!screenshot.is_empty());
    assert_eq!(&screenshot[0..4], &[0x89, 0x50, 0x4E, 0x47]);
}

// ── JavaScript Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_evaluate_expression() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let result: i64 = page.evaluate("2 + 2").await.unwrap().into_value().unwrap();
    assert_eq!(result, 4);
}

#[tokio::test]
async fn test_evaluate_dom_query() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let text: String = page
        .evaluate("document.getElementById('title').textContent")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(text, "Hello, remix-browser!");
}

#[tokio::test]
async fn test_evaluate_returns_object() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let result: serde_json::Value = page
        .evaluate("({ foo: 'bar', num: 42 })")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(result["foo"], "bar");
    assert_eq!(result["num"], 42);
}

// ── JS Menu Tests (the core differentiator) ─────────────────────────────

#[tokio::test]
async fn test_dropdown_menu_open_and_click() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("js_menu.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Click "File" menu button
    let _: serde_json::Value = page
        .evaluate(r#"document.querySelector('[data-menu="file"]').click()"#)
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify dropdown is visible
    let is_open: bool = page
        .evaluate(r#"document.getElementById('file-menu').classList.contains('open')"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(is_open, "File menu dropdown should be open");

    // Click "Save" option inside the dropdown
    let _: serde_json::Value = page
        .evaluate(r#"document.querySelector('[data-action="save"]').click()"#)
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let log_text: String = page
        .evaluate(r#"document.getElementById('log-entries').textContent"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        log_text.contains("save"),
        "Action log should contain 'save'"
    );
}

#[tokio::test]
async fn test_overlay_menu() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("js_menu.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Open overlay menu
    let _: serde_json::Value = page
        .evaluate(r#"document.getElementById('open-overlay').click()"#)
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let is_open: bool = page
        .evaluate(r#"document.getElementById('overlay-menu').classList.contains('open')"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(is_open, "Overlay menu should be open");

    // Click Option 2 inside overlay
    let _: serde_json::Value = page
        .evaluate(r#"document.querySelector('[data-action="option2"]').click()"#)
        .await
        .unwrap()
        .into_value()
        .unwrap_or_default();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let log_text: String = page
        .evaluate(r#"document.getElementById('log-entries').textContent"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        log_text.contains("option2"),
        "Action log should contain 'option2'"
    );
}

// ── Dynamic Content Tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_wait_for_dynamic_element() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("dynamic.html").as_str())
        .await
        .unwrap();

    // The delayed element appears after 1 second
    // Wait up to 3 seconds for it
    let mut found = false;
    for _ in 0..30 {
        let exists: bool = page
            .evaluate("!!document.getElementById('delayed-element')")
            .await
            .unwrap()
            .into_value()
            .unwrap_or(false);

        if exists {
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(found, "Delayed element should appear within 3 seconds");

    let text: String = page
        .evaluate("document.getElementById('delayed-element').textContent")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(text, "I appeared after 1 second!");
}

// ── Find Elements Tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_find_elements_css() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let count: i64 = page
        .evaluate("document.querySelectorAll('.content p').length")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(count, 2, "Should find 2 paragraphs in .content");
}

#[tokio::test]
async fn test_find_elements_by_text() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let found: bool = page
        .evaluate(
            r#"(() => {
            const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
            while (walker.nextNode()) {
                if (walker.currentNode.textContent.trim().includes('Important text')) return true;
            }
            return false;
        })()"#,
        )
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(found, "Should find element by text content");
}

// ── Network Capture Tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_network_capture() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    // Enable CDP Network domain
    page.execute(EnableParams::default()).await.unwrap();

    // Subscribe to events
    let mut requests = page
        .event_listener::<EventRequestWillBeSent>()
        .await
        .unwrap();
    let mut responses = page
        .event_listener::<EventResponseReceived>()
        .await
        .unwrap();

    // Shared log to collect entries
    let entries: Arc<Mutex<Vec<(String, String, i64)>>> = Arc::new(Mutex::new(Vec::new()));
    let pending: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    let pending_clone = pending.clone();
    let req_handle = tokio::spawn(async move {
        while let Some(req) = requests.next().await {
            let mut p = pending_clone.lock().await;
            p.insert(
                req.request_id.inner().to_string(),
                req.request.method.clone(),
            );
        }
    });

    let entries_clone = entries.clone();
    let pending_clone2 = pending.clone();
    let resp_handle = tokio::spawn(async move {
        while let Some(resp) = responses.next().await {
            let request_id = resp.request_id.inner().to_string();
            let p = pending_clone2.lock().await;
            let method = p.get(&request_id).cloned().unwrap_or_default();
            drop(p);
            let mut e = entries_clone.lock().await;
            e.push((resp.response.url.clone(), method, resp.response.status));
        }
    });

    // Navigate to a real page to generate network requests
    page.goto("https://httpbin.org/get").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Check captured entries
    let captured = entries.lock().await;
    assert!(
        !captured.is_empty(),
        "Should have captured at least one network entry"
    );

    let has_httpbin = captured
        .iter()
        .any(|(url, _, status)| url.contains("httpbin.org") && *status == 200);
    assert!(
        has_httpbin,
        "Should have captured httpbin.org request with status 200, got: {:?}",
        *captured
    );

    // Clean up spawned tasks
    req_handle.abort();
    resp_handle.abort();
}

// ── Snapshot Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_form_page() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("form.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let params = remix_browser::tools::snapshot::SnapshotParams { selector: None };
    let result = remix_browser::tools::snapshot::snapshot(&page, &params)
        .await
        .unwrap();

    // Should contain form elements
    assert!(
        result.contains("input"),
        "Snapshot should contain input elements, got:\n{}",
        result
    );
    assert!(
        result.contains("select"),
        "Snapshot should contain select element, got:\n{}",
        result
    );
    assert!(
        result.contains("textarea"),
        "Snapshot should contain textarea element, got:\n{}",
        result
    );
    assert!(
        result.contains("button"),
        "Snapshot should contain button element, got:\n{}",
        result
    );

    // Should be compact — much less than full HTML
    assert!(
        result.len() < 5000,
        "Snapshot should be compact (<5KB), got {} bytes",
        result.len()
    );

    // Should have indexed lines
    assert!(
        result.contains("[0]"),
        "Snapshot should have indexed elements"
    );
    assert!(
        result.contains("[ref=e"),
        "Snapshot should include ref tokens"
    );
}

#[tokio::test]
async fn test_snapshot_basic_page() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let params = remix_browser::tools::snapshot::SnapshotParams { selector: None };
    let result = remix_browser::tools::snapshot::snapshot(&page, &params)
        .await
        .unwrap();

    // Should find the heading and link
    assert!(
        result.contains("h1"),
        "Snapshot should contain h1 heading, got:\n{}",
        result
    );
    assert!(
        result.contains("a "),
        "Snapshot should contain link element, got:\n{}",
        result
    );
    assert!(
        result.contains("Click me"),
        "Snapshot should contain link text, got:\n{}",
        result
    );
    assert!(
        result.contains("[ref=e"),
        "Snapshot should contain ref tokens"
    );
}

#[tokio::test]
async fn test_snapshot_scoped_selector() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("form.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Scope to just the form
    let params = remix_browser::tools::snapshot::SnapshotParams {
        selector: Some("#test-form".to_string()),
    };
    let result = remix_browser::tools::snapshot::snapshot(&page, &params)
        .await
        .unwrap();

    // Should contain form elements but be scoped
    assert!(
        result.contains("input"),
        "Scoped snapshot should contain inputs"
    );
    assert!(
        result.contains("[0]"),
        "Scoped snapshot should have indexed elements"
    );
    assert!(
        result.contains("[ref=e"),
        "Scoped snapshot should include ref tokens"
    );
}

#[tokio::test]
async fn test_ref_selector_resolution_for_get_text_and_wait_for() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("basic.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let snap = remix_browser::tools::snapshot::snapshot_with_refs(
        &page,
        &remix_browser::tools::snapshot::SnapshotParams { selector: None },
    )
    .await
    .unwrap();

    let title_ref = snap
        .refs
        .iter()
        .find(|(_, selector)| selector.as_str() == "#title")
        .map(|(ref_id, _)| ref_id.clone())
        .expect("expected #title ref in snapshot");

    let resolved = resolve_ref_selector(&format!("ref={}", title_ref), &snap.refs)
        .expect("ref selector should resolve");
    assert_eq!(resolved, "#title");

    let css_text = remix_browser::tools::dom::get_text(
        &page,
        &remix_browser::tools::dom::GetTextParams {
            selector: "#title".to_string(),
            selector_type: Some(remix_browser::selectors::SelectorType::Css),
        },
    )
    .await
    .unwrap();

    let ref_text = remix_browser::tools::dom::get_text(
        &page,
        &remix_browser::tools::dom::GetTextParams {
            selector: resolved.clone(),
            selector_type: Some(remix_browser::selectors::SelectorType::Css),
        },
    )
    .await
    .unwrap();

    assert_eq!(css_text, ref_text);

    let css_wait = remix_browser::tools::dom::wait_for(
        &page,
        &remix_browser::tools::dom::WaitForParams {
            selector: "#title".to_string(),
            selector_type: Some(remix_browser::selectors::SelectorType::Css),
            timeout_ms: Some(1000),
            state: Some("visible".to_string()),
        },
    )
    .await
    .unwrap();

    let ref_wait = remix_browser::tools::dom::wait_for(
        &page,
        &remix_browser::tools::dom::WaitForParams {
            selector: resolved,
            selector_type: Some(remix_browser::selectors::SelectorType::Css),
            timeout_ms: Some(1000),
            state: Some("visible".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(css_wait && ref_wait);
}

#[tokio::test]
async fn test_ref_selector_resolution_for_click_and_type_text() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser
        .new_page(fixture_url("form.html").as_str())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let snap = remix_browser::tools::snapshot::snapshot_with_refs(
        &page,
        &remix_browser::tools::snapshot::SnapshotParams { selector: None },
    )
    .await
    .unwrap();

    let name_ref = snap
        .refs
        .iter()
        .find(|(_, selector)| selector.as_str() == "#name")
        .map(|(ref_id, _)| ref_id.clone())
        .expect("expected #name ref in snapshot");

    let submit_ref = snap
        .refs
        .iter()
        .find(|(_, selector)| selector.as_str() == "#submit-btn")
        .map(|(ref_id, _)| ref_id.clone())
        .expect("expected #submit-btn ref in snapshot");

    let resolved_name =
        resolve_ref_selector(&name_ref, &snap.refs).expect("name ref should resolve");
    let resolved_submit = resolve_ref_selector(&format!("[ref={}]", submit_ref), &snap.refs)
        .expect("submit ref should resolve");

    remix_browser::tools::interaction::type_text(
        &page,
        &remix_browser::tools::interaction::TypeTextParams {
            selector: resolved_name,
            text: "Ref User".to_string(),
            selector_type: Some(remix_browser::selectors::SelectorType::Css),
            clear_first: Some(true),
        },
    )
    .await
    .unwrap();

    let value: String = page
        .evaluate("document.getElementById('name').value")
        .await
        .unwrap()
        .into_value()
        .unwrap();
    assert_eq!(value, "Ref User");

    remix_browser::tools::interaction::do_click(
        &page,
        &remix_browser::tools::interaction::ClickParams {
            selector: resolved_submit,
            selector_type: Some(remix_browser::selectors::SelectorType::Css),
            button: Some("left".to_string()),
        },
    )
    .await
    .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let result_display: String = page
        .evaluate("getComputedStyle(document.getElementById('result')).display")
        .await
        .unwrap()
        .into_value()
        .unwrap();
    assert_eq!(result_display, "block");
}

// ── Circular Buffer Tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_network_log_circular_buffer() {
    let log = remix_browser::tools::network::NetworkLog::new();
    log.enable(None).await;

    // Add 600 entries (cap is 500)
    for i in 0..600 {
        log.add(remix_browser::tools::network::NetworkEntry {
            url: format!("https://example.com/{}", i),
            method: "GET".to_string(),
            status: 200,
            headers: None,
            body_preview: String::new(),
            timing_ms: 0.0,
        })
        .await;
    }

    let entries = log.get_log(None, None, None).await;
    assert_eq!(entries.len(), 500, "Network log should cap at 500 entries");
    // Oldest entries should be dropped (0-99 dropped, 100-599 kept)
    assert!(
        entries[0].url.contains("/100"),
        "First entry should be #100, got: {}",
        entries[0].url
    );
    assert!(
        entries[499].url.contains("/599"),
        "Last entry should be #599, got: {}",
        entries[499].url
    );
}

#[tokio::test]
async fn test_console_log_circular_buffer() {
    let log = remix_browser::tools::javascript::ConsoleLog::new();

    // Add 1200 entries (cap is 1000)
    for i in 0..1200 {
        log.add(remix_browser::tools::javascript::ConsoleEntry {
            level: "log".to_string(),
            text: format!("entry {}", i),
            timestamp: i as f64,
        })
        .await;
    }

    let entries = log.read(None, false, None).await;
    assert_eq!(
        entries.len(),
        1000,
        "Console log should cap at 1000 entries"
    );
    assert!(
        entries[0].text.contains("200"),
        "First entry should be #200, got: {}",
        entries[0].text
    );
    assert!(
        entries[999].text.contains("1199"),
        "Last entry should be #1199, got: {}",
        entries[999].text
    );
}

// ── run_script Tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_run_script_navigate_and_snapshot() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let url = fixture_url("basic.html");
    let script = format!(
        r#"page.navigate('{}');
        const snap = page.snapshot();
        console.log(snap);"#,
        url
    );

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, _screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(
        result.success,
        "Script should succeed, error: {:?}",
        result.error
    );
    assert!(
        result.output.contains("h1"),
        "Output should contain snapshot with h1, got:\n{}",
        result.output
    );
    assert!(
        result.url.contains("basic.html"),
        "Final URL should be basic.html"
    );
    assert_eq!(result.title, "Basic Test Page");
}

#[tokio::test]
async fn test_run_script_form_fill() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let url = fixture_url("form.html");
    let script = format!(
        r#"page.navigate('{}');
        page.type('#name', 'Test User');
        const val = page.js("document.getElementById('name').value");
        console.log('Name value: ' + val);"#,
        url
    );

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, _screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(
        result.success,
        "Script should succeed, error: {:?}",
        result.error
    );
    assert!(
        result.output.contains("Test User"),
        "Output should contain typed value, got:\n{}",
        result.output
    );
}

#[tokio::test]
async fn test_run_script_loop() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let url = fixture_url("basic.html");
    let script = format!(
        r#"page.navigate('{}');
        const items = ['alpha', 'beta', 'gamma'];
        for (const item of items) {{
            console.log('Processing: ' + item);
        }}"#,
        url
    );

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, _screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(
        result.success,
        "Script should succeed, error: {:?}",
        result.error
    );
    assert!(
        result.output.contains("Processing: alpha"),
        "Should log alpha"
    );
    assert!(
        result.output.contains("Processing: beta"),
        "Should log beta"
    );
    assert!(
        result.output.contains("Processing: gamma"),
        "Should log gamma"
    );
}

#[tokio::test]
async fn test_run_script_error_handling() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let url = fixture_url("basic.html");
    let script = format!(
        r#"page.navigate('{}');
        console.log('before error');
        page.click('#nonexistent-element-xyz');
        console.log('after error');"#,
        url
    );

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, _screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(!result.success, "Script should fail on nonexistent element");
    assert!(result.error.is_some(), "Should have error message");
    assert!(
        result.output.contains("before error"),
        "Should have output before the error"
    );
    assert!(
        !result.output.contains("after error"),
        "Should not have output after error"
    );
}

#[tokio::test]
async fn test_run_script_screenshot() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let url = fixture_url("basic.html");
    let script = format!(
        r#"page.navigate('{}');
        page.screenshot();"#,
        url
    );

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(
        result.success,
        "Script should succeed, error: {:?}",
        result.error
    );
    assert_eq!(screenshots.len(), 1, "Should have 1 screenshot");
}

#[tokio::test]
async fn test_run_script_console_log() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let script = r#"
        console.log('hello world');
        console.log('number:', 42);
        console.log('done');
    "#
    .to_string();

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, _screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(
        result.success,
        "Script should succeed, error: {:?}",
        result.error
    );
    assert!(
        result.output.contains("hello world"),
        "Should contain 'hello world'"
    );
    assert!(result.output.contains("42"), "Should contain '42'");
    assert!(result.output.contains("done"), "Should contain 'done'");
}

#[tokio::test]
async fn test_run_script_js_execution() {
    let (browser, _handle, _tmp) = launch_test_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    let console_log = remix_browser::tools::javascript::ConsoleLog::new();
    let network_log = remix_browser::tools::network::NetworkLog::new();

    let url = fixture_url("basic.html");
    let script = format!(
        r#"page.navigate('{}');
        const title = page.js("document.title");
        console.log('Title: ' + title);"#,
        url
    );

    let params = remix_browser::tools::script::RunScriptParams { script };
    let (result, _screenshots) =
        remix_browser::tools::script::run_script(&page, &params, &console_log, &network_log)
            .await
            .unwrap();

    assert!(
        result.success,
        "Script should succeed, error: {:?}",
        result.error
    );
    assert!(
        result.output.contains("Basic Test Page"),
        "Should contain page title, got:\n{}",
        result.output
    );
}

---
name: browse
description: >-
  Use Chrome, use the browser, open a website, browse the web, go to a URL,
  take a screenshot of a page, test a web app, check a website, fill out forms,
  scrape a webpage, interact with a web page, debug a UI, inspect the DOM,
  run JavaScript in the browser, monitor network requests, or automate any
  browser task. Headless Chrome automation via CDP.
---
Use the remix-browser MCP tools (prefixed `mcp__remix-browser__`) for all browser tasks.

**How to use `run_script` effectively**:
- Use `run_script` for ALL browser interactions — it's much faster than individual tool calls.
- A compact ARIA-role snapshot is **automatically appended** after every tool (navigate, click, type_text, scroll, etc.), so you always see page state and `[ref=eN]` refs immediately.
- **Refs work inside scripts**: After `page.snapshot()`, use `[ref=eN]` selectors with `page.click('[ref=e0]')`, `page.type('[ref=e1]', 'text')`, etc.
- **Strategy**: Do the first action with a short `run_script` to learn the UI and selectors. Then batch all remaining repetitive work into a single `run_script` with a loop.
- Use `page.wait(ms)` inside scripts for timing — don't use Bash `sleep`.
- Use `page.waitForNetworkIdle({timeout:30000, idle:500})` to wait for all network requests to complete.

**Snapshot format (ARIA roles)**:
- Elements use ARIA roles: `heading`, `link`, `textbox`, `button`, `combobox`, `checkbox`, `radio`, etc.
- Only interactive elements get `[ref=eN]` — headings and landmarks appear for context without refs.
- State annotations: `[checked]`, `[disabled]`, `[expanded]`, `[required]`

**When to use granular tools instead**:
- For 1-2 simple actions where a script is overkill (`click`, `type_text`, etc.).
- All granular tools auto-append a snapshot, so you don't need to call `snapshot` separately.

**Available tools**:
- Navigation: `navigate`, `go_back`, `go_forward`, `reload`, `get_page_info`
- DOM: `find_elements`, `get_text`, `get_html`, `wait_for`
- Snapshot: `snapshot`
- Interaction: `click`, `type_text`, `hover`, `select_option`, `press_key`, `scroll`
- Visual: `screenshot`
- JavaScript: `execute_js`, `read_console`
- Network: `network_enable`, `get_network_log`
- Tabs: `new_tab`, `close_tab`, `list_tabs`
- Script: `run_script`

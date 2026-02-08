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

**Performance usage pattern**:
- For 1-2 simple actions, use granular tools (`navigate`, `click`, `type_text`, etc.).
- For workflows with 3+ actions, loops, or repeated extraction, prefer `run_script` to reduce tool calls and latency.
- Use `snapshot` only when you need fresh element refs (`[ref=eN]`) for targeting.

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

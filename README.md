<p align="center">
  <h1 align="center">remix-browser</h1>
  <p align="center">
    <strong>Blazing fast headless Chrome automation via CDP — no extension needed.</strong>
  </p>
  <p align="center">
    <a href="#installation">Installation</a> &middot;
    <a href="#tools">Tools</a> &middot;
    <a href="#configuration">Configuration</a> &middot;
    <a href="#architecture">Architecture</a>
  </p>
</p>

[![CI](https://github.com/hkd987/remix-browser/actions/workflows/ci.yml/badge.svg)](https://github.com/hkd987/remix-browser/actions/workflows/ci.yml)

---

A Rust-native [MCP](https://modelcontextprotocol.io/) server that gives AI agents full control over a real Chrome browser through the Chrome DevTools Protocol. No browser extensions, no Puppeteer, no Node.js — just a single binary that speaks CDP.

## Why remix-browser?

| | remix-browser | Extension-based MCPs | Puppeteer wrappers |
|---|---|---|---|
| **Startup** | Single binary, instant | Requires browser extension install | Node.js + npm install |
| **Reliability** | Hybrid click strategy with automatic fallback | Extension message passing | Basic click only |
| **Multi-tab** | Built-in tab pool management | Limited by extension API | Manual page tracking |
| **Network capture** | First-class network monitoring | Not available | Requires extra setup |
| **Console logs** | Built-in capture with filtering | Not available | Requires extra setup |
| **Selectors** | CSS + Text + XPath | CSS only | CSS + XPath |
| **Language** | Rust (fast, safe, single binary) | JavaScript | JavaScript |

## Installation

### Claude Code Plugin (recommended)

```
/plugin marketplace add hkd987/remix-browser
/plugin install remix-browser@hkd987-remix-browser
```

That's it — the binary downloads automatically on first use. No Rust required.

### Download pre-built binary

```bash
curl -fsSL https://raw.githubusercontent.com/hkd987/remix-browser/main/scripts/install.sh | sh
```

### From source

```bash
git clone https://github.com/hkd987/remix-browser.git
cd remix-browser
cargo build --release
```

### Requirements

- **Google Chrome** or **Chromium** installed (auto-detected)
- **Rust 1.88+** only needed if building from source

## Quick Start

### Add to Claude Code

Add to your Claude Code MCP config (`~/.claude/mcp.json`):

```json
{
  "mcpServers": {
    "remix-browser": {
      "command": "/path/to/remix-browser"
    }
  }
}
```

That's it. Claude now has a browser.

### Headed mode (see what's happening)

```json
{
  "mcpServers": {
    "remix-browser": {
      "command": "/path/to/remix-browser",
      "args": ["--headed"]
    }
  }
}
```

### Best performance tip

For the best experience, add this line to your project's `CLAUDE.md` (or `~/.claude/CLAUDE.md` for all projects):

```markdown
When I ask to use Chrome, use the browser, browse a website, or do anything browser-related — use the remix-browser MCP tools. Always start with `navigate`.
```

This tells Claude to automatically reach for remix-browser whenever you mention browser tasks — no need to say "remix-browser" by name.

## Tools

remix-browser exposes **18 tools** organized into 6 categories.

### Navigation

| Tool | Description |
|---|---|
| `navigate` | Go to a URL. Supports `load`, `domcontentloaded`, and `networkidle` wait strategies. |
| `go_back` | Navigate back in history. |
| `go_forward` | Navigate forward in history. |
| `reload` | Reload the current page. |
| `get_page_info` | Get current URL, title, and viewport dimensions. |

### Finding Elements

| Tool | Description |
|---|---|
| `find_elements` | Find elements by CSS selector, text content, or XPath. Returns tag, text, attributes, and node IDs. |
| `get_text` | Extract text content from a matched element. |
| `get_html` | Get inner or outer HTML of the page or a specific element. |
| `wait_for` | Wait for an element to appear, disappear, or become visible. Configurable timeout. |

### Interaction

| Tool | Description |
|---|---|
| `click` | Click elements using a **hybrid strategy** — tries real mouse events first, falls back to JS dispatch if the element is obscured. Reports which method was used. |
| `type_text` | Type into input fields. Optionally clear existing content first. |
| `hover` | Hover over elements (fires `mouseenter`, `mouseover`, `mousemove`). |
| `select_option` | Select an option in a `<select>` dropdown by value. |
| `press_key` | Press keyboard keys (`Enter`, `Tab`, `ArrowDown`, etc.) with optional modifiers. |
| `scroll` | Scroll the page or a specific element in any direction. |

### Screenshots

| Tool | Description |
|---|---|
| `screenshot` | Capture the viewport, full page, or a specific element as base64 PNG/JPEG. |

### JavaScript & Console

| Tool | Description |
|---|---|
| `execute_js` | Run arbitrary JavaScript and get the result back as JSON. |
| `read_console` | Read captured `console.log`/`warn`/`error` output. Filter by level or regex pattern. |

### Network Monitoring

| Tool | Description |
|---|---|
| `network_enable` | Start capturing network requests. Optionally filter by URL patterns. |
| `get_network_log` | Query captured requests by URL pattern, HTTP method, or status code. Includes timing data. |

### Tab Management

| Tool | Description |
|---|---|
| `new_tab` | Open a new tab, optionally navigating to a URL. |
| `close_tab` | Close a specific tab or the active one. |
| `list_tabs` | List all open tabs with their URLs and titles. |

## Selector Types

All element-targeting tools support three selector strategies:

```
CSS (default):  "button.submit", "#login-form", "div > p:first-child"
Text:           "Sign In", "Submit Order", "Click here"
XPath:          "//button[@type='submit']", "//div[contains(@class, 'menu')]"
```

Text selectors use a TreeWalker to find elements by their visible text content — no need to inspect the DOM to find the right CSS class.

## The Hybrid Click

Most browser automation tools fail on modern JS-heavy sites. Dropdown menus, overlays, dynamically positioned elements — they all break simple `element.click()`.

remix-browser uses a **hybrid click strategy**:

1. Scroll the element into view
2. Check visibility and whether it's obscured by other elements
3. Dispatch real mouse events (`mousedown` -> `mouseup` -> `click`) at the element's coordinates
4. If the element is obscured (e.g., behind an overlay), automatically fall back to JavaScript `click()`
5. Report which method was used so you know exactly what happened

This means clicks **just work** — even on sites with complex overlays, sticky headers, and dynamic menus.

## Configuration

| Option | Default | Description |
|---|---|---|
| `--headed` | `false` | Show the browser window instead of running headless |
| `RUST_LOG` env var | `info` | Control log verbosity (`debug`, `trace`, etc.) |

### Chrome Detection

remix-browser automatically finds Chrome on your system:

- **macOS**: `/Applications/Google Chrome.app`, Homebrew paths, Chrome Canary
- **Linux**: `/usr/bin/google-chrome`, snap packages, Chromium
- **Windows**: Program Files, Local AppData

Falls back to `which google-chrome` / `which chromium` if standard paths don't exist.

### Default Browser Settings

- **Viewport**: 1280x720
- **Headless mode**: `--headless=new` (Chrome's latest headless implementation)
- Extensions, sync, popups, and first-run prompts are all disabled for a clean automation environment

## Architecture

```
src/
├── main.rs                # CLI entry point
├── server.rs              # MCP ServerHandler — routes tool calls
├── browser/
│   ├── session.rs         # Browser lifecycle management
│   ├── pool.rs            # Multi-tab tracking (TabPool)
│   └── launcher.rs        # Chrome binary detection & launch config
├── tools/
│   ├── navigation.rs      # navigate, go_back, go_forward, reload
│   ├── dom.rs             # find_elements, get_text, get_html, wait_for
│   ├── interaction.rs     # click, type_text, hover, select_option, press_key, scroll
│   ├── screenshot.rs      # screenshot capture
│   ├── javascript.rs      # execute_js, console log capture
│   ├── network.rs         # network monitoring
│   └── page.rs            # tab management
├── interaction/
│   ├── click.rs           # Hybrid click strategy implementation
│   ├── keyboard.rs        # Key press & text input
│   └── scroll.rs          # Scroll logic
└── selectors/
    ├── css.rs             # CSS selector resolution
    ├── text.rs            # Text content matching via TreeWalker
    └── xpath.rs           # XPath evaluation
```

Built on:
- **[rmcp](https://github.com/anthropics/rmcp)** — Rust MCP framework
- **[chromiumoxide](https://github.com/mattsse/chromiumoxide)** — CDP client for Rust
- **[tokio](https://tokio.rs)** — Async runtime

## Testing

```bash
# Run all 16 integration tests (uses real headless Chrome)
cargo test --test-threads=4

# Run a specific test
cargo test test_navigate
```

Tests use fixture HTML files and spin up isolated Chrome instances with unique profiles — no shared state, no flakiness.

## License

MIT

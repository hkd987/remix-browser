use anyhow::{Result, bail};
use std::path::PathBuf;

/// Find the Chrome/Chromium binary on the current platform.
pub fn find_chrome_binary() -> Result<PathBuf> {
    let candidates = chrome_candidates();

    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            tracing::info!("Found Chrome at: {}", path.display());
            return Ok(path);
        }
    }

    // Try PATH lookup
    for name in &[
        "google-chrome",
        "google-chrome-stable",
        "chromium-browser",
        "chromium",
    ] {
        if let Ok(path) = which::which(name) {
            tracing::info!("Found Chrome in PATH: {}", path.display());
            return Ok(path);
        }
    }

    bail!(
        "Could not find Chrome or Chromium. Searched:\n{}",
        candidates.join("\n")
    )
}

fn chrome_candidates() -> Vec<String> {
    let mut candidates = Vec::new();

    #[cfg(target_os = "macos")]
    {
        candidates.extend([
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
            "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary".into(),
        ]);
        // Homebrew paths
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(format!(
                "{}/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                home
            ));
        }
    }

    #[cfg(target_os = "linux")]
    {
        candidates.extend([
            "/usr/bin/google-chrome".into(),
            "/usr/bin/google-chrome-stable".into(),
            "/usr/bin/chromium-browser".into(),
            "/usr/bin/chromium".into(),
            "/snap/bin/chromium".into(),
        ]);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(pf) = std::env::var("PROGRAMFILES") {
            candidates.push(format!("{}\\Google\\Chrome\\Application\\chrome.exe", pf));
        }
        if let Ok(pf86) = std::env::var("PROGRAMFILES(X86)") {
            candidates.push(format!("{}\\Google\\Chrome\\Application\\chrome.exe", pf86));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(format!("{}\\Google\\Chrome\\Application\\chrome.exe", local));
        }
    }

    candidates
}

/// Build the default Chrome launch arguments.
pub fn default_chrome_args(headless: bool, user_data_dir: &std::path::Path) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", user_data_dir.display()),
        "--remote-debugging-port=0".into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-background-networking".into(),
        "--disable-client-side-phishing-detection".into(),
        "--disable-default-apps".into(),
        "--disable-extensions".into(),
        "--disable-hang-monitor".into(),
        "--disable-popup-blocking".into(),
        "--disable-prompt-on-repost".into(),
        "--disable-sync".into(),
        "--disable-translate".into(),
        "--metrics-recording-only".into(),
        "--safebrowsing-disable-auto-update".into(),
        "--window-size=1280,720".into(),
    ];

    if headless {
        args.push("--headless=new".into());
    }

    args
}

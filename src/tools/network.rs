use anyhow::Result;
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams, EventRequestWillBeSent, EventResponseReceived,
};
use chromiumoxide::page::Page;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A captured network request/response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub url: String,
    pub method: String,
    pub status: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    pub body_preview: String,
    pub timing_ms: f64,
}

/// Shared network log storage.
#[derive(Debug, Clone)]
pub struct NetworkLog {
    pub entries: Arc<Mutex<Vec<NetworkEntry>>>,
    pub enabled: Arc<Mutex<bool>>,
    pub patterns: Arc<Mutex<Vec<String>>>,
    pub pending_count: Arc<AtomicU32>,
}

impl NetworkLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            enabled: Arc::new(Mutex::new(false)),
            patterns: Arc::new(Mutex::new(Vec::new())),
            pending_count: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn pending_requests(&self) -> u32 {
        self.pending_count.load(Ordering::Relaxed)
    }

    pub async fn enable(&self, patterns: Option<Vec<String>>) {
        let mut enabled = self.enabled.lock().await;
        *enabled = true;
        if let Some(patterns) = patterns {
            let mut p = self.patterns.lock().await;
            *p = patterns;
        }
    }

    pub async fn add(&self, entry: NetworkEntry) {
        let enabled = self.enabled.lock().await;
        if !*enabled {
            return;
        }

        let patterns = self.patterns.lock().await;
        if !patterns.is_empty() {
            let matches = patterns.iter().any(|p| entry.url.contains(p));
            if !matches {
                return;
            }
        }
        drop(patterns);

        let mut entries = self.entries.lock().await;
        if entries.len() >= 500 {
            entries.remove(0);
        }
        entries.push(entry);
    }

    pub async fn get_log(
        &self,
        url_pattern: Option<&str>,
        method: Option<&str>,
        status: Option<u32>,
    ) -> Vec<NetworkEntry> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .filter(|e| {
                if let Some(pattern) = url_pattern {
                    if !e.url.contains(pattern) {
                        return false;
                    }
                }
                if let Some(method) = method {
                    if e.method != method {
                        return false;
                    }
                }
                if let Some(status) = status {
                    if e.status != status {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetworkEnableParams {
    #[schemars(description = "URL patterns to capture (captures all if empty)")]
    pub patterns: Option<Vec<String>>,
}

pub async fn network_enable(
    network_log: &NetworkLog,
    params: &NetworkEnableParams,
) -> Result<bool> {
    network_log.enable(params.patterns.clone()).await;
    Ok(true)
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetNetworkLogParams {
    #[schemars(description = "Filter by URL pattern")]
    pub url_pattern: Option<String>,
    #[schemars(description = "Filter by HTTP method")]
    pub method: Option<String>,
    #[schemars(description = "Filter by status code")]
    pub status: Option<u32>,
    #[schemars(description = "Include response headers in output (default: false)")]
    pub include_headers: Option<bool>,
    #[schemars(description = "Maximum number of entries to return (default: 50)")]
    pub limit: Option<u32>,
}

pub async fn get_network_log(
    network_log: &NetworkLog,
    params: &GetNetworkLogParams,
) -> Result<serde_json::Value> {
    let mut entries = network_log
        .get_log(
            params.url_pattern.as_deref(),
            params.method.as_deref(),
            params.status,
        )
        .await;

    // Strip headers unless explicitly requested
    if !params.include_headers.unwrap_or(false) {
        for entry in &mut entries {
            entry.headers = None;
        }
    }

    // Apply limit (take most recent entries)
    let limit = params.limit.unwrap_or(50) as usize;
    let total = entries.len();
    if total > limit {
        let entries = entries.split_off(total - limit);
        return Ok(serde_json::json!({
            "entries": entries,
            "total": total,
            "showing": limit,
            "note": format!("Showing last {} of {} entries. Use limit to see more.", limit, total)
        }));
    }

    Ok(serde_json::to_value(entries)?)
}

/// Subscribe to CDP network events on a page and feed entries into the shared NetworkLog.
pub async fn start_listening(page: &Page, network_log: NetworkLog) -> Result<()> {
    // Enable CDP Network domain on the page
    page.execute(EnableParams::default()).await?;

    // Subscribe to request + response events
    let mut requests = page.event_listener::<EventRequestWillBeSent>().await?;
    let mut responses = page.event_listener::<EventResponseReceived>().await?;

    // Spawn background task: collect requests in a HashMap keyed by request_id,
    // then when a response arrives, merge into a NetworkEntry and add to the log
    let log = network_log.clone();
    let pending_counter = network_log.pending_count.clone();
    tokio::spawn(async move {
        let mut pending_map: HashMap<String, Arc<EventRequestWillBeSent>> = HashMap::new();

        loop {
            tokio::select! {
                Some(req) = requests.next() => {
                    pending_counter.fetch_add(1, Ordering::Relaxed);
                    pending_map.insert(req.request_id.inner().to_string(), req);
                }
                Some(resp) = responses.next() => {
                    let request_id = resp.request_id.inner().to_string();
                    let method = pending_map.get(&request_id)
                        .map(|r| r.request.method.clone())
                        .unwrap_or_default();
                    if pending_map.remove(&request_id).is_some() {
                        pending_counter.fetch_sub(1, Ordering::Relaxed);
                    }
                    let entry = NetworkEntry {
                        url: resp.response.url.clone(),
                        method,
                        status: resp.response.status as u32,
                        headers: Some(resp.response.headers.inner().clone()),
                        body_preview: String::new(),
                        timing_ms: 0.0,
                    };
                    log.add(entry).await;
                }
                else => break,
            }
        }
    });

    Ok(())
}

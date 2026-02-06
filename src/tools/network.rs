use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A captured network request/response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub url: String,
    pub method: String,
    pub status: u32,
    pub headers: serde_json::Value,
    pub body_preview: String,
    pub timing_ms: f64,
}

/// Shared network log storage.
#[derive(Debug, Clone, Default)]
pub struct NetworkLog {
    pub entries: Arc<Mutex<Vec<NetworkEntry>>>,
    pub enabled: Arc<Mutex<bool>>,
    pub patterns: Arc<Mutex<Vec<String>>>,
}

impl NetworkLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            enabled: Arc::new(Mutex::new(false)),
            patterns: Arc::new(Mutex::new(Vec::new())),
        }
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
}

pub async fn get_network_log(
    network_log: &NetworkLog,
    params: &GetNetworkLogParams,
) -> Result<serde_json::Value> {
    let entries = network_log
        .get_log(
            params.url_pattern.as_deref(),
            params.method.as_deref(),
            params.status,
        )
        .await;
    Ok(serde_json::to_value(entries)?)
}

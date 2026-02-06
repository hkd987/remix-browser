use anyhow::{Context, Result};
use chromiumoxide::page::Page;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecuteJsParams {
    #[schemars(description = "JavaScript expression to evaluate")]
    pub expression: String,
}

pub async fn execute_js(page: &Page, params: &ExecuteJsParams) -> Result<serde_json::Value> {
    let result: serde_json::Value = page
        .evaluate(params.expression.as_str())
        .await
        .context("Failed to evaluate JavaScript")?
        .into_value()
        .unwrap_or(serde_json::Value::Null);

    Ok(result)
}

/// Console log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: String,
    pub text: String,
    pub timestamp: f64,
}

/// Shared console log storage.
#[derive(Debug, Clone, Default)]
pub struct ConsoleLog {
    pub entries: Arc<Mutex<Vec<ConsoleEntry>>>,
}

impl ConsoleLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add(&self, entry: ConsoleEntry) {
        let mut entries = self.entries.lock().await;
        entries.push(entry);
    }

    pub async fn read(
        &self,
        level: Option<&str>,
        clear: bool,
        pattern: Option<&str>,
    ) -> Vec<ConsoleEntry> {
        let mut entries = self.entries.lock().await;
        let filtered: Vec<ConsoleEntry> = entries
            .iter()
            .filter(|e| {
                if let Some(level) = level {
                    if e.level != level {
                        return false;
                    }
                }
                if let Some(pattern) = pattern {
                    if !e.text.contains(pattern) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if clear {
            entries.clear();
        }

        filtered
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReadConsoleParams {
    #[schemars(description = "Filter by log level: log, warn, error")]
    pub level: Option<String>,
    #[schemars(description = "Clear console entries after reading")]
    pub clear: Option<bool>,
    #[schemars(description = "Filter entries by pattern")]
    pub pattern: Option<String>,
}

pub async fn read_console(
    console_log: &ConsoleLog,
    params: &ReadConsoleParams,
) -> Result<serde_json::Value> {
    let entries = console_log
        .read(
            params.level.as_deref(),
            params.clear.unwrap_or(false),
            params.pattern.as_deref(),
        )
        .await;

    Ok(serde_json::to_value(entries)?)
}

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
    let eval_result = page
        .evaluate(params.expression.as_str())
        .await
        .with_context(|| {
            let preview = if params.expression.len() > 200 {
                format!("{}...", &params.expression[..200])
            } else {
                params.expression.clone()
            };
            format!("Failed to evaluate JavaScript: {}", preview)
        })?;

    let val: serde_json::Value = eval_result
        .into_value()
        .unwrap_or(serde_json::Value::Null);

    // Detect DOM element results: CDP serializes DOM nodes as empty objects `{}`
    // When a DOM query pattern is present and the result is an empty object,
    // return an actionable error instead of a useless `{}`
    if let serde_json::Value::Object(ref map) = val {
        if map.is_empty() {
            let expr = &params.expression;
            if expr.contains("querySelector") || expr.contains("getElementById")
                || expr.contains("getElementsBy") || expr.contains("elementFromPoint")
            {
                anyhow::bail!(
                    "Expression returned a DOM element which cannot be serialized to JSON. \
                     Append .textContent, .value, .getAttribute('name'), or .outerHTML to extract a serializable value."
                )
            }
        }
    }

    Ok(val)
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
        if entries.len() >= 1000 {
            entries.remove(0);
        }
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
    #[schemars(description = "Maximum number of entries to return (default: 100)")]
    pub limit: Option<u32>,
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

    let limit = params.limit.unwrap_or(100) as usize;
    let total = entries.len();
    if total > limit {
        let showing = &entries[total - limit..];
        return Ok(serde_json::json!({
            "entries": showing,
            "total": total,
            "showing": limit,
            "note": format!("Showing last {} of {} entries. Use limit to see more.", limit, total)
        }));
    }

    Ok(serde_json::to_value(entries)?)
}

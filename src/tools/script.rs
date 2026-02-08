use anyhow::Result;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsError, JsValue, NativeFunction, Source};
use chromiumoxide::page::Page;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::selectors::SelectorType;
use crate::tools::{dom, interaction, javascript, navigation, network, screenshot, snapshot};

use rmcp::model::Content;

// ── Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunScriptParams {
    /// JavaScript to execute with access to the `page` object for browser automation
    pub script: String,
}

pub struct ScriptResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub elapsed_ms: u128,
    pub url: String,
    pub title: String,
}

impl ScriptResult {
    pub fn format_output(&self) -> String {
        let mut out = String::new();
        if self.success {
            out.push_str(&format!("Script completed in {}ms\n", self.elapsed_ms));
        } else {
            out.push_str(&format!("Script failed in {}ms\n", self.elapsed_ms));
        }
        if !self.output.is_empty() {
            out.push_str("Output:\n");
            for line in self.output.lines() {
                out.push_str(&format!("  {}\n", line));
            }
        }
        if let Some(ref err) = self.error {
            out.push_str(&format!("Error: {}\n", err));
        }
        out.push_str(&format!("Final: {} — \"{}\"", self.url, self.title));
        out
    }
}

// ── Shared State ───────────────────────────────────────────────────────

struct ScriptContext {
    handle: tokio::runtime::Handle,
    page: Page,
    console_log: javascript::ConsoleLog,
    network_log: network::NetworkLog,
    output_lines: Mutex<Vec<String>>,
    screenshots: Mutex<Vec<String>>,
}

// ── Entry Point ────────────────────────────────────────────────────────

pub async fn run_script(
    page: &Page,
    params: &RunScriptParams,
    console_log: &javascript::ConsoleLog,
    network_log: &network::NetworkLog,
) -> Result<(ScriptResult, Vec<Content>)> {
    let ctx = Arc::new(ScriptContext {
        handle: tokio::runtime::Handle::current(),
        page: page.clone(),
        console_log: console_log.clone(),
        network_log: network_log.clone(),
        output_lines: Mutex::new(Vec::new()),
        screenshots: Mutex::new(Vec::new()),
    });

    let script = params.script.clone();
    let ctx_clone = ctx.clone();

    let start = Instant::now();

    let result = tokio::task::spawn_blocking(move || execute_in_boa(&ctx_clone, &script)).await?;

    let elapsed_ms = start.elapsed().as_millis();

    // Get final page state
    let url = page.url().await?.unwrap_or_default();
    let title = page.get_title().await?.unwrap_or_default();

    let output = ctx.output_lines.lock().unwrap().join("\n");

    // Build Content items from collected screenshots
    let screenshots = ctx.screenshots.lock().unwrap();
    let contents: Vec<Content> = screenshots
        .iter()
        .map(|b64| Content::image(b64.clone(), "image/png"))
        .collect();

    match result {
        Ok(()) => Ok((
            ScriptResult {
                success: true,
                output,
                error: None,
                elapsed_ms,
                url,
                title,
            },
            contents,
        )),
        Err(err_msg) => Ok((
            ScriptResult {
                success: false,
                output,
                error: Some(err_msg),
                elapsed_ms,
                url,
                title,
            },
            contents,
        )),
    }
}

// ── Boa Execution ──────────────────────────────────────────────────────

fn execute_in_boa(ctx: &Arc<ScriptContext>, script: &str) -> Result<(), String> {
    let mut js_ctx = Context::default();

    // Build the `page` object with all native methods
    let page_obj = build_page_object(ctx, &mut js_ctx);
    js_ctx
        .register_global_property(
            boa_engine::js_string!("page"),
            page_obj,
            Attribute::all(),
        )
        .map_err(|e| format!("Failed to register page object: {}", e))?;

    // Override console.log to collect output
    let console_obj = build_console_object(ctx, &mut js_ctx);
    js_ctx
        .register_global_property(
            boa_engine::js_string!("console"),
            console_obj,
            Attribute::all(),
        )
        .map_err(|e| format!("Failed to register console object: {}", e))?;

    // Execute the script
    match js_ctx.eval(Source::from_bytes(script)) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{}", e)),
    }
}

// ── Page Object Builder ────────────────────────────────────────────────

fn build_page_object(ctx: &Arc<ScriptContext>, js_ctx: &mut Context) -> JsValue {
    let mut builder = ObjectInitializer::new(js_ctx);

    // Navigation
    builder.function(make_navigate(ctx.clone()), boa_engine::js_string!("navigate"), 1);
    builder.function(make_back(ctx.clone()), boa_engine::js_string!("back"), 0);
    builder.function(make_forward(ctx.clone()), boa_engine::js_string!("forward"), 0);
    builder.function(make_reload(ctx.clone()), boa_engine::js_string!("reload"), 0);

    // Interaction
    builder.function(make_click(ctx.clone()), boa_engine::js_string!("click"), 2);
    builder.function(make_type(ctx.clone()), boa_engine::js_string!("type"), 3);
    builder.function(make_hover(ctx.clone()), boa_engine::js_string!("hover"), 2);
    builder.function(make_select(ctx.clone()), boa_engine::js_string!("select"), 3);
    builder.function(make_press(ctx.clone()), boa_engine::js_string!("press"), 2);
    builder.function(make_scroll(ctx.clone()), boa_engine::js_string!("scroll"), 2);

    // Waiting
    builder.function(make_wait(ctx.clone()), boa_engine::js_string!("wait"), 1);
    builder.function(make_wait_for(ctx.clone()), boa_engine::js_string!("waitFor"), 2);

    // Observation
    builder.function(make_snapshot(ctx.clone()), boa_engine::js_string!("snapshot"), 1);
    builder.function(make_screenshot(ctx.clone()), boa_engine::js_string!("screenshot"), 1);
    builder.function(make_get_text(ctx.clone()), boa_engine::js_string!("getText"), 2);
    builder.function(make_get_html(ctx.clone()), boa_engine::js_string!("getHtml"), 2);
    builder.function(make_find_elements(ctx.clone()), boa_engine::js_string!("findElements"), 2);

    // JavaScript
    builder.function(make_js(ctx.clone()), boa_engine::js_string!("js"), 1);

    // Console/Network
    builder.function(make_read_console(ctx.clone()), boa_engine::js_string!("readConsole"), 1);
    builder.function(make_enable_network(ctx.clone()), boa_engine::js_string!("enableNetwork"), 1);
    builder.function(make_get_network_log(ctx.clone()), boa_engine::js_string!("getNetworkLog"), 1);

    builder.build().into()
}

// ── Console Object Builder ─────────────────────────────────────────────

fn build_console_object(ctx: &Arc<ScriptContext>, js_ctx: &mut Context) -> JsValue {
    let mut builder = ObjectInitializer::new(js_ctx);
    builder.function(make_console_log(ctx.clone()), boa_engine::js_string!("log"), 1);
    builder.function(make_console_log(ctx.clone()), boa_engine::js_string!("info"), 1);
    builder.function(make_console_log(ctx.clone()), boa_engine::js_string!("warn"), 1);
    builder.function(make_console_log(ctx.clone()), boa_engine::js_string!("error"), 1);
    builder.build().into()
}

// ── Helper: Extract Options from JsValue ───────────────────────────────

fn get_string_prop(obj: &JsValue, key: &str, js_ctx: &mut Context) -> Option<String> {
    let obj = obj.as_object()?;
    let key = boa_engine::js_string!(key);
    let val = obj.get(key, js_ctx).ok()?;
    if val.is_undefined() || val.is_null() {
        return None;
    }
    Some(val.to_string(js_ctx).ok()?.to_std_string_escaped())
}

fn get_bool_prop(obj: &JsValue, key: &str, js_ctx: &mut Context) -> Option<bool> {
    let obj = obj.as_object()?;
    let key = boa_engine::js_string!(key);
    let val = obj.get(key, js_ctx).ok()?;
    if val.is_undefined() || val.is_null() {
        return None;
    }
    Some(val.to_boolean())
}

fn get_number_prop(obj: &JsValue, key: &str, js_ctx: &mut Context) -> Option<f64> {
    let obj = obj.as_object()?;
    let key = boa_engine::js_string!(key);
    let val = obj.get(key, js_ctx).ok()?;
    if val.is_undefined() || val.is_null() {
        return None;
    }
    val.to_number(js_ctx).ok()
}

fn parse_selector_type(options: &JsValue, js_ctx: &mut Context) -> Option<SelectorType> {
    let type_str = get_string_prop(options, "type", js_ctx)?;
    match type_str.as_str() {
        "text" => Some(SelectorType::Text),
        "xpath" => Some(SelectorType::Xpath),
        "css" => Some(SelectorType::Css),
        _ => None,
    }
}

fn get_string_array_prop(obj: &JsValue, key: &str, js_ctx: &mut Context) -> Option<Vec<String>> {
    let obj = obj.as_object()?;
    let key = boa_engine::js_string!(key);
    let val = obj.get(key, js_ctx).ok()?;
    if val.is_undefined() || val.is_null() {
        return None;
    }
    let arr = val.as_object()?;
    let len_key = boa_engine::js_string!("length");
    let len = arr.get(len_key, js_ctx).ok()?.to_number(js_ctx).ok()? as usize;
    let mut result = Vec::new();
    for i in 0..len {
        if let Ok(item) = arr.get(i, js_ctx) {
            if let Ok(s) = item.to_string(js_ctx) {
                result.push(s.to_std_string_escaped());
            }
        }
    }
    Some(result)
}

fn js_err(msg: impl std::fmt::Display) -> JsError {
    JsError::from_opaque(JsValue::from(boa_engine::js_string!(msg.to_string())))
}

// Convert a serde_json::Value to a JsValue
fn json_to_js(val: &serde_json::Value, js_ctx: &mut Context) -> JsValue {
    match val {
        serde_json::Value::Null => JsValue::null(),
        serde_json::Value::Bool(b) => JsValue::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                JsValue::from(i as f64)
            } else if let Some(f) = n.as_f64() {
                JsValue::from(f)
            } else {
                JsValue::null()
            }
        }
        serde_json::Value::String(s) => {
            JsValue::from(boa_engine::js_string!(s.as_str()))
        }
        serde_json::Value::Array(arr) => {
            let js_arr = boa_engine::object::builtins::JsArray::new(js_ctx);
            for item in arr {
                let js_item = json_to_js(item, js_ctx);
                js_arr.push(js_item, js_ctx).unwrap_or_default();
            }
            js_arr.into()
        }
        serde_json::Value::Object(map) => {
            let obj = boa_engine::JsObject::with_null_proto();
            for (k, v) in map {
                let js_val = json_to_js(v, js_ctx);
                let key = boa_engine::property::PropertyKey::from(boa_engine::js_string!(k.as_str()));
                obj.set(key, js_val, false, js_ctx).unwrap_or_default();
            }
            obj.into()
        }
    }
}

// ── Native Function Factories ──────────────────────────────────────────

fn make_navigate(ctx: Arc<ScriptContext>) -> NativeFunction {
    // Safety: Arc<ScriptContext> is not a JS GC-managed type, so no GC tracing needed
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let url = args.get_or_undefined(0).to_string(js_ctx)?;
            let url_str = url.to_std_string_escaped();

            let params = navigation::NavigateParams {
                url: url_str,
                wait_until: None,
            };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { navigation::navigate(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(format!(
                "{} — {}",
                result.title, result.url
            ))))
        })
    }
}

fn make_back(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, _args, _js_ctx| {
            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { navigation::go_back(&page).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(format!(
                "{} — {}",
                result.title, result.url
            ))))
        })
    }
}

fn make_forward(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, _args, _js_ctx| {
            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { navigation::go_forward(&page).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(format!(
                "{} — {}",
                result.title, result.url
            ))))
        })
    }
}

fn make_reload(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, _args, _js_ctx| {
            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { navigation::reload(&page).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(format!(
                "{} — {}",
                result.title, result.url
            ))))
        })
    }
}

fn make_click(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let params = interaction::ClickParams {
                selector: selector.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
                button: get_string_prop(&options, "button", js_ctx),
            };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { interaction::do_click(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(format!(
                "Clicked ({})",
                result.method_used
            ))))
        })
    }
}

fn make_type(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let text = args.get_or_undefined(1).to_string(js_ctx)?;
            let options = args.get_or_undefined(2).clone();

            let params = interaction::TypeTextParams {
                selector: selector.to_std_string_escaped(),
                text: text.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
                clear_first: get_bool_prop(&options, "clear", js_ctx),
            };

            let page = ctx.page.clone();
            ctx.handle
                .block_on(async { interaction::type_text(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!("Typed text")))
        })
    }
}

fn make_hover(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let params = interaction::HoverParams {
                selector: selector.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
            };

            let page = ctx.page.clone();
            ctx.handle
                .block_on(async { interaction::hover(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!("Hovered")))
        })
    }
}

fn make_select(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let value = args.get_or_undefined(1).to_string(js_ctx)?;
            let options = args.get_or_undefined(2).clone();

            let params = interaction::SelectOptionParams {
                selector: selector.to_std_string_escaped(),
                value: value.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
            };

            let page = ctx.page.clone();
            ctx.handle
                .block_on(async { interaction::select_option(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!("Selected")))
        })
    }
}

fn make_press(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let key = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let modifiers = get_string_array_prop(&options, "modifiers", js_ctx);

            let params = interaction::PressKeyParams {
                key: key.to_std_string_escaped(),
                modifiers,
            };

            let page = ctx.page.clone();
            ctx.handle
                .block_on(async { interaction::press_key(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(format!(
                "Pressed {}",
                params.key
            ))))
        })
    }
}

fn make_scroll(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let direction = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let params = interaction::ScrollParams {
                direction: direction.to_std_string_escaped(),
                selector: get_string_prop(&options, "selector", js_ctx),
                amount: get_number_prop(&options, "amount", js_ctx).map(|n| n as i32),
                selector_type: parse_selector_type(&options, js_ctx),
            };

            let page = ctx.page.clone();
            ctx.handle
                .block_on(async { interaction::do_scroll(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!("Scrolled")))
        })
    }
}

fn make_wait(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let ms = args.get_or_undefined(0).to_number(js_ctx)? as u64;

            ctx.handle.block_on(async {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            });

            Ok(JsValue::undefined())
        })
    }
}

fn make_wait_for(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let params = dom::WaitForParams {
                selector: selector.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
                timeout_ms: get_number_prop(&options, "timeout", js_ctx).map(|n| n as u64),
                state: get_string_prop(&options, "state", js_ctx),
            };

            let page = ctx.page.clone();
            let found = ctx
                .handle
                .block_on(async { dom::wait_for(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(found))
        })
    }
}

fn make_snapshot(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector_arg = args.get_or_undefined(0);
            let selector = if selector_arg.is_undefined() || selector_arg.is_null() {
                None
            } else {
                Some(selector_arg.to_string(js_ctx)?.to_std_string_escaped())
            };

            let params = snapshot::SnapshotParams { selector };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { snapshot::snapshot(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(result)))
        })
    }
}

fn make_screenshot(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let options = args.get_or_undefined(0).clone();

            let params = screenshot::ScreenshotParams {
                selector: get_string_prop(&options, "selector", js_ctx),
                full_page: get_bool_prop(&options, "full_page", js_ctx),
                format: get_string_prop(&options, "format", js_ctx),
                quality: get_number_prop(&options, "quality", js_ctx).map(|n| n as u32),
            };

            let page = ctx.page.clone();
            let base64 = ctx
                .handle
                .block_on(async { screenshot::screenshot(&page, &params).await })
                .map_err(|e| js_err(e))?;

            // Collect screenshot for return as Content::image
            ctx.screenshots.lock().unwrap().push(base64);

            Ok(JsValue::from(boa_engine::js_string!("Screenshot captured")))
        })
    }
}

fn make_get_text(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let params = dom::GetTextParams {
                selector: selector.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
            };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { dom::get_text(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(result)))
        })
    }
}

fn make_get_html(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let first_arg = args.get_or_undefined(0);
            let (selector, options) = if first_arg.is_object() {
                // If first arg is an options object (no selector)
                (None, first_arg.clone())
            } else if first_arg.is_undefined() || first_arg.is_null() {
                (None, JsValue::undefined())
            } else {
                let sel = first_arg.to_string(js_ctx)?.to_std_string_escaped();
                (Some(sel), args.get_or_undefined(1).clone())
            };

            let params = dom::GetHtmlParams {
                selector,
                outer: get_bool_prop(&options, "outer", js_ctx),
                max_length: get_number_prop(&options, "max_length", js_ctx).map(|n| n as u32),
            };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { dom::get_html(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!(result)))
        })
    }
}

fn make_find_elements(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let selector = args.get_or_undefined(0).to_string(js_ctx)?;
            let options = args.get_or_undefined(1).clone();

            let params = dom::FindElementsParams {
                selector: selector.to_std_string_escaped(),
                selector_type: parse_selector_type(&options, js_ctx),
                max_results: get_number_prop(&options, "max_results", js_ctx).map(|n| n as u32),
            };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { dom::find_elements(&page, &params).await })
                .map_err(|e| js_err(e))?;

            // Convert JSON result to JS value
            Ok(json_to_js(&result, js_ctx))
        })
    }
}

fn make_js(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let expression = args.get_or_undefined(0).to_string(js_ctx)?;

            let params = javascript::ExecuteJsParams {
                expression: expression.to_std_string_escaped(),
            };

            let page = ctx.page.clone();
            let result = ctx
                .handle
                .block_on(async { javascript::execute_js(&page, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(json_to_js(&result, js_ctx))
        })
    }
}

fn make_read_console(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let options = args.get_or_undefined(0).clone();

            let params = javascript::ReadConsoleParams {
                level: get_string_prop(&options, "level", js_ctx),
                clear: get_bool_prop(&options, "clear", js_ctx),
                pattern: get_string_prop(&options, "pattern", js_ctx),
                limit: get_number_prop(&options, "limit", js_ctx).map(|n| n as u32),
            };

            let console_log = ctx.console_log.clone();
            let result = ctx
                .handle
                .block_on(async { javascript::read_console(&console_log, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(json_to_js(&result, js_ctx))
        })
    }
}

fn make_enable_network(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let patterns = get_string_array_prop(args.get_or_undefined(0), "patterns", js_ctx)
                .or_else(|| {
                    // Also support passing array directly
                    let first = args.get_or_undefined(0);
                    if first.is_object() && !first.is_null() {
                        get_string_array_prop(first, "length", js_ctx).is_some().then(|| {
                            // It's an array-like
                            let mut patterns = Vec::new();
                            let obj = first.as_object().unwrap();
                            let len_key = boa_engine::js_string!("length");
                            if let Ok(len_val) = obj.get(len_key, js_ctx) {
                                if let Ok(len) = len_val.to_number(js_ctx) {
                                    for i in 0..(len as usize) {
                                        if let Ok(item) = obj.get(i, js_ctx) {
                                            if let Ok(s) = item.to_string(js_ctx) {
                                                patterns.push(s.to_std_string_escaped());
                                            }
                                        }
                                    }
                                }
                            }
                            patterns
                        })
                    } else {
                        None
                    }
                });

            let enable_params = network::NetworkEnableParams {
                patterns: patterns.clone(),
            };

            let network_log = ctx.network_log.clone();
            let page = ctx.page.clone();
            ctx.handle
                .block_on(async {
                    network::network_enable(&network_log, &enable_params).await?;
                    network::start_listening(&page, network_log).await
                })
                .map_err(|e| js_err(e))?;

            Ok(JsValue::from(boa_engine::js_string!("Network capture enabled")))
        })
    }
}

fn make_get_network_log(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let options = args.get_or_undefined(0).clone();

            let params = network::GetNetworkLogParams {
                url_pattern: get_string_prop(&options, "url_pattern", js_ctx),
                method: get_string_prop(&options, "method", js_ctx),
                status: get_number_prop(&options, "status", js_ctx).map(|n| n as u32),
                include_headers: get_bool_prop(&options, "include_headers", js_ctx),
                limit: get_number_prop(&options, "limit", js_ctx).map(|n| n as u32),
            };

            let network_log = ctx.network_log.clone();
            let result = ctx
                .handle
                .block_on(async { network::get_network_log(&network_log, &params).await })
                .map_err(|e| js_err(e))?;

            Ok(json_to_js(&result, js_ctx))
        })
    }
}

fn make_console_log(ctx: Arc<ScriptContext>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, js_ctx| {
            let mut parts = Vec::new();
            for i in 0..args.len() {
                let val = args.get_or_undefined(i);
                let s = val.to_string(js_ctx)?.to_std_string_escaped();
                parts.push(s);
            }
            let line = parts.join(" ");
            ctx.output_lines.lock().unwrap().push(line);
            Ok(JsValue::undefined())
        })
    }
}

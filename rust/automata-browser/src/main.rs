use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::browser::{
    Bounds, GetWindowForTargetParams, SetWindowBoundsParams,
};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams,
    DispatchMouseEventType, InsertTextParams, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::network::{
    ClearBrowserCookiesParams, CookieParam,
};
use chromiumoxide::page::Page;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio::task::JoinHandle;

const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";
type WebResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn ok(v: Value) -> WebResult {
    Ok(Json(json!({ "value": v })))
}

fn fail(status: StatusCode, kind: &str, message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "value": { "error": kind, "message": message.into() } })),
    )
}

fn bad(message: impl Into<String>) -> (StatusCode, Json<Value>) {
    fail(StatusCode::BAD_REQUEST, "invalid argument", message)
}

fn no_session() -> (StatusCode, Json<Value>) {
    fail(
        StatusCode::NOT_FOUND,
        "invalid session id",
        "no browser session is running; call POST /session first",
    )
}

#[derive(Default)]
struct Inner {
    browser: Option<Browser>,
    handler: Option<JoinHandle<()>>,
    pages: Vec<Page>,
    active: usize,
    session_id: Option<String>,
    frame: Option<String>,
    elements: HashMap<String, String>,
    counter: u64,
}

impl Inner {
    fn page(&self) -> Result<Page, (StatusCode, Json<Value>)> {
        self.pages
            .get(self.active)
            .cloned()
            .ok_or_else(no_session)
    }

    fn next_eid(&mut self) -> String {
        self.counter += 1;
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("e{}-{}", t, self.counter)
    }
}

struct AppState {
    inner: tokio::sync::Mutex<Inner>,
}

fn root_expr(frame: &Option<String>) -> String {
    match frame {
        None => "globalThis.document".to_string(),
        Some(sel) => {
            let s = serde_json::to_string(sel).unwrap_or_else(|_| "\"\"".to_string());
            format!(
                "(function(){{try{{var f=globalThis.document.querySelector({s});\
                 return f?(f.contentDocument||(f.contentWindow&&f.contentWindow.document)):null;}}\
                 catch(e){{return null;}}}})()"
            )
        }
    }
}

const FIND_FN: &str = r#"
function __automataFind(root, sel, depth) {
  if (!root) return null;
  var el = null;
  try { el = root.querySelector(sel); } catch (e) {}
  if (el) return el;
  if (depth <= 0) return null;
  var all = root.querySelectorAll('*');
  for (var i = 0; i < all.length; i++) {
    var n = all[i];
    if (n.shadowRoot) {
      var inner = __automataFind(n.shadowRoot, sel, depth - 1);
      if (inner) return inner;
    }
  }
  return null;
}
"#;

async fn eval(page: &Page, script: String) -> Result<Value, String> {
    match page.evaluate_expression(script).await {
        Ok(res) => Ok(res.value().cloned().unwrap_or(Value::Null)),
        Err(e) => Err(e.to_string()),
    }
}

async fn eval_retry(page: &Page, script: String) -> Result<Value, String> {
    let mut last = String::new();
    for attempt in 0..3 {
        match eval(page, script.clone()).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = e;
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
    Err(last)
}

async fn ensure_browser(g: &mut Inner, caps: &Value) -> Result<(), (StatusCode, Json<Value>)> {
    if g.browser.is_some() && !g.pages.is_empty() {
        return Ok(());
    }

    let options = caps
        .get("goog:chromeOptions")
        .or_else(|| caps.get("chromeOptions"))
        .cloned()
        .unwrap_or(Value::Null);

    let binary = options
        .get("binary")
        .and_then(|b| b.as_str())
        .unwrap_or("")
        .to_string();

    let mut builder = BrowserConfig::builder()
        .with_head()
        .no_sandbox()
        .window_size(1280, 900)
        .launch_timeout(Duration::from_secs(30))
        .request_timeout(Duration::from_secs(60));

    let profile = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("automata")
        .join("browser-profile");
    let _ = std::fs::create_dir_all(&profile);
    builder = builder.user_data_dir(profile);

    if !binary.trim().is_empty() {
        builder = builder.chrome_executable(PathBuf::from(&binary));
    }

    if let Some(args) = options.get("args").and_then(|a| a.as_array()) {
        for arg in args.iter().filter_map(|a| a.as_str()) {
            if arg == "--no-sandbox" || arg == "--headless" || arg.starts_with("--headless=") {
                continue;
            }
            builder = builder.arg(arg);
        }
    }

    let config = builder
        .build()
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "session not created", e))?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "session not created", e.to_string()))?;

    let h = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });

    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "session not created", e.to_string()))?;

    g.handler = Some(h);
    g.browser = Some(browser);
    g.pages = vec![page];
    g.active = 0;
    g.session_id = Some(format!(
        "cdp-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    g.frame = None;
    g.elements.clear();
    Ok(())
}

fn handle_of(page: &Page) -> String {
    serde_json::to_value(page.target_id())
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

async fn status() -> WebResult {
    ok(json!({ "ready": true, "message": "Automata CDP engine ready" }))
}

async fn new_session(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> WebResult {
    let caps = body
        .get("capabilities")
        .and_then(|c| c.get("alwaysMatch").or_else(|| Some(c)))
        .cloned()
        .unwrap_or(Value::Null);

    let mut g = state.inner.lock().await;
    ensure_browser(&mut g, &caps).await?;
    let sid = g.session_id.clone().unwrap_or_default();
    ok(json!({
        "sessionId": sid,
        "capabilities": { "browserName": "chrome", "automataEngine": "cdp" }
    }))
}

async fn delete_session(State(state): State<Arc<AppState>>) -> WebResult {
    let mut g = state.inner.lock().await;
    if let Ok(page) = g.page() {
        let _ = page.close().await;
    }
    if let Some(mut browser) = g.browser.take() {
        let _ = browser.close().await;
    }
    if let Some(h) = g.handler.take() {
        h.abort();
    }
    g.pages.clear();
    g.session_id = None;
    g.frame = None;
    g.elements.clear();
    ok(Value::Null)
}

async fn navigate(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let url = body
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| bad("url is required"))?
        .to_string();

    let g = state.inner.lock().await;
    let page = g.page()?;
    page.goto(url.clone())
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while tokio::time::Instant::now() < deadline {
        match eval(&page, "globalThis.document.readyState".to_string()).await {
            Ok(Value::String(s)) if s == "complete" => break,
            _ => tokio::time::sleep(Duration::from_millis(150)).await,
        }
    }
    ok(Value::Null)
}

async fn current_url(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let url = page
        .url()
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    ok(json!(url.unwrap_or_default()))
}

async fn new_window(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let url = body
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("about:blank")
        .to_string();

    let mut g = state.inner.lock().await;
    let browser = g.browser.as_ref().ok_or_else(no_session)?;
    let page = browser
        .new_page(url)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    g.pages.push(page.clone());
    g.active = g.pages.len() - 1;
    let handle = page.target_id().as_ref().to_string();
    ok(json!({ "handle": handle, "type": "tab" }))
}

async fn handles(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    if g.pages.is_empty() {
        return Err(no_session());
    }
    let list: Vec<String> = g.pages.iter().map(handle_of).collect();
    ok(json!(list))
}

async fn current_handle(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    ok(json!(handle_of(&page)))
}

async fn switch_window(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let handle = body
        .get("handle")
        .and_then(|h| h.as_str())
        .or_else(|| body.get("name").and_then(|h| h.as_str()))
        .unwrap_or("");
    let mut g = state.inner.lock().await;
    let idx = g
        .pages
        .iter()
        .position(|p| handle_of(p) == handle)
        .ok_or_else(|| bad("no such window"))?;
    g.active = idx;
    ok(Value::Null)
}

async fn close_window(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let mut g = state.inner.lock().await;
    if g.pages.len() <= 1 {
        return Err(bad("cannot close the last window"));
    }
    let active = g.active;
    let page = g.pages.remove(active);
    let _ = page.close().await;
    if g.active >= g.pages.len() {
        g.active = g.pages.len() - 1;
    }
    ok(Value::Null)
}

async fn get_window_rect(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let params = GetWindowForTargetParams::builder()
        .target_id(page.target_id().clone())
        .build();
    let window = page
        .execute(params)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    let b = window.result.bounds;
    ok(json!({
        "x": b.left.unwrap_or(0),
        "y": b.top.unwrap_or(0),
        "width": b.width.unwrap_or(0),
        "height": b.height.unwrap_or(0),
    }))
}

async fn set_window_rect(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let params = GetWindowForTargetParams::builder()
        .target_id(page.target_id().clone())
        .build();
    let window = page
        .execute(params)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;

    let bounds = Bounds {
        left: body.get("x").and_then(|v| v.as_i64()),
        top: body.get("y").and_then(|v| v.as_i64()),
        width: body.get("width").and_then(|v| v.as_i64()),
        height: body.get("height").and_then(|v| v.as_i64()),
        window_state: None,
    };

    let cmd = SetWindowBoundsParams::new(window.result.window_id.clone(), bounds);
    page.execute(cmd)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    ok(Value::Null)
}

async fn get_cookies(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let cookies = page
        .get_cookies()
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    let value = serde_json::to_value(cookies).unwrap_or_else(|_| json!([]));
    ok(value)
}

async fn add_cookie(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let cookie = body.get("cookie").cloned().unwrap_or(body);
    let name = cookie
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| bad("cookie.name is required"))?;
    let value = cookie
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut builder = CookieParam::builder()
        .name(name.to_string())
        .value(value.to_string());
    if let Some(url) = cookie.get("url").and_then(|v| v.as_str()) {
        builder = builder.url(url.to_string());
    }
    if let Some(domain) = cookie.get("domain").and_then(|v| v.as_str()) {
        builder = builder.domain(domain.to_string());
    }
    if let Some(path) = cookie.get("path").and_then(|v| v.as_str()) {
        builder = builder.path(path.to_string());
    }
    if let Some(secure) = cookie.get("secure").and_then(|v| v.as_bool()) {
        builder = builder.secure(secure);
    }
    if let Some(http_only) = cookie.get("httpOnly").and_then(|v| v.as_bool()) {
        builder = builder.http_only(http_only);
    }
    let param = builder.build().map_err(bad)?;

    let g = state.inner.lock().await;
    let page = g.page()?;
    page.set_cookie(param)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    ok(Value::Null)
}

async fn delete_cookies(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    page.execute(ClearBrowserCookiesParams::default())
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    ok(Value::Null)
}

async fn back(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let _ = eval(&page, "globalThis.history.back(); true".to_string()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    ok(Value::Null)
}

async fn forward(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let _ = eval(&page, "globalThis.history.forward(); true".to_string()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    ok(Value::Null)
}

async fn refresh(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    page.reload()
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    ok(Value::Null)
}

async fn title(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let t = page
        .get_title()
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    ok(json!(t.unwrap_or_default()))
}

async fn source(State(state): State<Arc<AppState>>, AxPath(_sid): AxPath<String>) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let html = page
        .content()
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    ok(json!(html))
}

async fn switch_frame(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let mut g = state.inner.lock().await;
    let id = body.get("id");
    match id {
        None | Some(Value::Null) => {
            g.frame = None;
        }
        Some(Value::Object(map)) => {
            let eid = map
                .get(ELEMENT_KEY)
                .and_then(|v| v.as_str())
                .ok_or_else(|| bad("frame id must be an element reference"))?;
            let selector = g
                .elements
                .get(eid)
                .cloned()
                .ok_or_else(|| bad("unknown frame element"))?;
            g.frame = Some(selector);
        }
        _ => return Err(bad("frame id must be null or an element reference")),
    }
    ok(Value::Null)
}

async fn find_element(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let selector = body
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| bad("element selector is required"))?
        .to_string();

    let mut g = state.inner.lock().await;
    let page = g.page()?;
    let eid = g.next_eid();
    let eid_json = serde_json::to_string(&eid).unwrap();
    let sel_json = serde_json::to_string(&selector).unwrap();
    let script = format!(
        "(function(){{var __root={root};if(!__root)return null;{find}\
         var el=__automataFind(__root,{sel},6);if(!el)return null;\
         el.setAttribute('data-automata-eid',{eid});return {eid};}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        sel = sel_json,
        eid = eid_json
    );

    let value = eval_retry(&page, script)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e))?;

    if value.is_null() || value.as_str().is_none() {
        return Err(fail(
            StatusCode::NOT_FOUND,
            "no such element",
            format!("no such element: {}", selector),
        ));
    }

    g.elements.insert(eid.clone(), selector);
    ok(json!({ ELEMENT_KEY: eid }))
}

fn resolve_args(args: &Value) -> Value {
    match args {
        Value::Array(list) => Value::Array(list.iter().map(resolve_args).collect()),
        Value::Object(map) => {
            if let Some(id) = map.get(ELEMENT_KEY).and_then(|v| v.as_str()) {
                return Value::String(format!("__AUTOMATA_ELEM__:{}", id));
            }
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve_args(v));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

async fn execute_sync(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let script = body
        .get("script")
        .and_then(|s| s.as_str())
        .ok_or_else(|| bad("script is required"))?
        .to_string();
    let args = resolve_args(body.get("args").unwrap_or(&Value::Null));
    let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "[]".to_string());

    let g = state.inner.lock().await;
    let page = g.page()?;

    let wrapped = format!(
        "(function(){{var __root={root};if(!__root)return null;\
         var document=__root;var window=__root.defaultView||globalThis;\
         {find}\
         var __raw={args};\
         function __resolve(v){{if(typeof v==='string'&&v.indexOf('__AUTOMATA_ELEM__:')===0){{\
           return __automataFind(__root,'[data-automata-eid=\"'+v.slice(17)+'\"]',6);}}\
           if(Array.isArray(v))return v.map(__resolve);\
           if(v&&typeof v==='object'){{for(var k in v)v[k]=__resolve(v[k]);return v;}}return v;}}\
         var __args=(Array.isArray(__raw)?__raw:[]).map(__resolve);\
         return (function(){{{script}}}).apply(null,__args);}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        args = args_json,
        script = script
    );

    let value = eval_retry(&page, wrapped)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "javascript error", e))?;
    ok(value)
}

async fn element_text(
    State(state): State<Arc<AppState>>,
    AxPath((_sid, eid)): AxPath<(String, String)>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let id = serde_json::to_string(&eid).unwrap();
    let script = format!(
        "(function(){{var __root={root};if(!__root)return '';{find}\
         var el=__automataFind(__root,'[data-automata-eid='+JSON.stringify({id})+']',6);\
         if(!el)return '';if(typeof el.value==='string')return el.value;\
         return el.innerText||el.textContent||'';}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        id = id
    );
    let value = eval_retry(&page, script)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e))?;
    ok(value)
}

async fn element_click(
    State(state): State<Arc<AppState>>,
    AxPath((_sid, eid)): AxPath<(String, String)>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let id = serde_json::to_string(&eid).unwrap();
    let script = format!(
        "(function(){{var __root={root};if(!__root)return {{ok:false}};{find}\
         var el=__automataFind(__root,'[data-automata-eid='+JSON.stringify({id})+']',6);\
         if(!el)return {{ok:false,error:'element not found'}};\
         el.scrollIntoView({{block:'center',inline:'center'}});\
         try{{el.click();}}catch(e){{el.dispatchEvent(new MouseEvent('click',{{bubbles:true,cancelable:true}}));}}\
         return {{ok:true}};}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        id = id
    );
    let value = eval_retry(&page, script)
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e))?;
    if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        return Err(fail(
            StatusCode::NOT_FOUND,
            "element click intercepted",
            value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("could not click element")
                .to_string(),
        ));
    }
    tokio::time::sleep(Duration::from_millis(120)).await;
    ok(Value::Null)
}

async fn element_clear(
    State(state): State<Arc<AppState>>,
    AxPath((_sid, eid)): AxPath<(String, String)>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let id = serde_json::to_string(&eid).unwrap();
    let script = format!(
        "(function(){{var __root={root};if(!__root)return null;{find}\
         var el=__automataFind(__root,'[data-automata-eid='+JSON.stringify({id})+']',6);\
         if(!el)return null;el.focus();\
         var proto=el instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;\
         try{{var setter=Object.getOwnPropertyDescriptor(proto,'value');\
           if(setter&&setter.set){{setter.set.call(el,'');}}else{{el.value='';}}}}catch(e){{el.value='';}}\
         if(el.isContentEditable){{el.innerHTML='';}}\
         el.dispatchEvent(new Event('input',{{bubbles:true}}));\
         el.dispatchEvent(new Event('change',{{bubbles:true}}));return true;}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        id = id
    );
    let _ = eval_retry(&page, script).await;
    ok(Value::Null)
}

async fn element_value(
    State(state): State<Arc<AppState>>,
    AxPath((_sid, eid)): AxPath<(String, String)>,
    Json(body): Json<Value>,
) -> WebResult {
    let text = body
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let g = state.inner.lock().await;
    let page = g.page()?;

    // File inputs need the real DOM node for DOM.setFileInputFiles.
    let selector = g.elements.get(&eid).cloned();
    if let Some(sel) = selector {
        if let Ok(element) = page.find_element(&sel).await {
            let is_file = element
                .attribute("type")
                .await
                .ok()
                .flatten()
                .map(|t| t.eq_ignore_ascii_case("file"))
                .unwrap_or(false);
            if is_file {
                let path = std::path::PathBuf::from(&text);
                if !path.exists() {
                    return Err(bad(format!("file does not exist: {}", text)));
                }
                let cmd = chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams::builder()
                    .files(vec![text.clone()])
                    .backend_node_id(element.backend_node_id)
                    .build();
                if let Ok(cmd) = cmd {
                    page.execute(cmd).await.map_err(|e| {
                        fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string())
                    })?;
                    return ok(Value::Null);
                }
            }
        }
    }

    let id = serde_json::to_string(&eid).unwrap();
    let text_json = serde_json::to_string(&text).unwrap();
    let focus_script = format!(
        "(function(){{var __root={root};if(!__root)return null;{find}\
         var el=__automataFind(__root,'[data-automata-eid='+JSON.stringify({id})+']',6);\
         if(!el)return null;el.scrollIntoView({{block:'center',inline:'center'}});el.focus();\
         var ok=(el instanceof HTMLInputElement)||(el instanceof HTMLTextAreaElement)||el.isContentEditable;\
         return ok;}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        id = id
    );
    let focused = eval_retry(&page, focus_script).await.unwrap_or(Value::Bool(false));

    if focused.as_bool() == Some(true) {
        let cmd = InsertTextParams::builder().text(text.clone()).build().unwrap();
        if page.execute(cmd).await.is_ok() {
            let after = format!(
                "(function(){{var __root={root};{find}\
                 var el=__automataFind(__root,'[data-automata-eid='+JSON.stringify({id})+']',6);\
                 if(!el)return null;el.dispatchEvent(new Event('input',{{bubbles:true}}));\
                 el.dispatchEvent(new Event('change',{{bubbles:true}}));return true;}})()",
                root = root_expr(&g.frame),
                find = FIND_FN,
                id = id
            );
            let _ = eval_retry(&page, after).await;
            return ok(Value::Null);
        }
    }

    let append = format!(
        "(function(){{var __root={root};if(!__root)return null;{find}\
         var el=__automataFind(__root,'[data-automata-eid='+JSON.stringify({id})+']',6);\
         if(!el)return null;\
         if(el.isContentEditable){{el.textContent=el.textContent+{text};}}\
         else if('value' in el){{el.value=el.value+{text};}}\
         else{{el.textContent=el.textContent+{text};}}\
         el.dispatchEvent(new Event('input',{{bubbles:true}}));\
         el.dispatchEvent(new Event('change',{{bubbles:true}}));return true;}})()",
        root = root_expr(&g.frame),
        find = FIND_FN,
        id = id,
        text = text_json
    );
    let _ = eval_retry(&page, append).await;
    ok(Value::Null)
}

fn wd_key_to_name(value: &str) -> Option<(&'static str, i64, &'static str, Option<&'static str>)> {
    let key = match value {
        "\u{E007}" => ("Enter", 13, "Enter", Some("\r")),
        "\u{E004}" => ("Tab", 9, "Tab", None),
        "\u{E00C}" => ("Escape", 27, "Escape", None),
        "\u{E003}" => ("Backspace", 8, "Backspace", None),
        "\u{E017}" => ("Delete", 46, "Delete", None),
        "\u{E008}" => ("Shift", 16, "ShiftLeft", None),
        "\u{E009}" => ("Control", 17, "ControlLeft", None),
        "\u{E00A}" => ("Alt", 18, "AltLeft", None),
        "\u{E012}" => ("ArrowLeft", 37, "ArrowLeft", None),
        "\u{E013}" => ("ArrowUp", 38, "ArrowUp", None),
        "\u{E014}" => ("ArrowRight", 39, "ArrowRight", None),
        "\u{E015}" => ("ArrowDown", 40, "ArrowDown", None),
        "\u{E011}" => ("Home", 36, "Home", None),
        "\u{E010}" => ("End", 35, "End", None),
        "\u{E00E}" => ("PageUp", 33, "PageUp", None),
        "\u{E00F}" => ("PageDown", 34, "PageDown", None),
        "\u{E00D}" => (" ", 32, "Space", Some(" ")),
        _ => return None,
    };
    Some(key)
}

fn modifiers_bit(mods: &[String]) -> i64 {
    let mut bit = 0;
    for m in mods {
        match m.as_str() {
            "Control" | "ControlLeft" | "ControlRight" => bit |= 2,
            "Shift" | "ShiftLeft" | "ShiftRight" => bit |= 8,
            "Alt" | "AltLeft" | "AltRight" => bit |= 1,
            "Meta" | "MetaLeft" | "MetaRight" => bit |= 4,
            _ => {}
        }
    }
    bit
}

async fn dispatch_key(page: &Page, value: &str, down: bool, mods: &mut Vec<String>) -> Result<(), String> {
    let (key, code, vk, text) = if let Some((k, vk, code, text)) = wd_key_to_name(value) {
        (k.to_string(), code.to_string(), vk, text.map(|t| t.to_string()))
    } else {
        let first = value.chars().next().unwrap_or('\0');
        let upper = first.to_ascii_uppercase();
        let code = if upper.is_ascii_alphabetic() {
            format!("Key{}", upper)
        } else if upper.is_ascii_digit() {
            format!("Digit{}", upper)
        } else {
            "".to_string()
        };
        let vk = if upper.is_ascii_alphabetic() {
            upper as i64
        } else if upper.is_ascii_digit() {
            upper as i64
        } else {
            0
        };
        (value.to_string(), code, vk, Some(value.to_string()))
    };

    if down {
        if key == "Shift" || key == "Control" || key == "Alt" || key == "Meta" {
            if !mods.contains(&key) {
                mods.push(key.clone());
            }
        }
    } else {
        mods.retain(|m| m != &key);
    }

    let mods_bit = modifiers_bit(mods);
    let kind = if down {
        DispatchKeyEventType::KeyDown
    } else {
        DispatchKeyEventType::KeyUp
    };

    let mut b = DispatchKeyEventParams::builder()
        .r#type(kind)
        .key(key.clone())
        .code(code)
        .windows_virtual_key_code(vk)
        .native_virtual_key_code(vk)
        .modifiers(mods_bit);
    if down {
        if let Some(t) = text.clone() {
            b = b.text(t.clone()).unmodified_text(t);
        }
    }
    let cmd = b.build().map_err(|e| e.to_string())?;
    page.execute(cmd).await.map_err(|e| e.to_string())?;
    Ok(())
}

fn mouse_button(idx: i64) -> MouseButton {
    match idx {
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

fn mouse_button_bit(idx: i64) -> i64 {
    match idx {
        1 => 4,
        2 => 2,
        _ => 1,
    }
}

async fn dispatch_mouse(
    page: &Page,
    kind: DispatchMouseEventType,
    x: f64,
    y: f64,
    button: MouseButton,
    buttons: i64,
    click_count: i64,
) -> Result<(), String> {
    let cmd = DispatchMouseEventParams::builder()
        .r#type(kind)
        .x(x)
        .y(y)
        .button(button)
        .buttons(buttons)
        .click_count(click_count)
        .build()
        .map_err(|e| e.to_string())?;
    page.execute(cmd).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn actions(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let sequences = body
        .get("actions")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let mut modifiers: Vec<String> = Vec::new();
    let mut pressed: Vec<i64> = Vec::new();

    for seq in sequences {
        let kind = seq.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let Some(list) = seq.get("actions").and_then(|a| a.as_array()) else {
            continue;
        };

        if kind == "pointer" {
            for action in list {
                let t = action.get("type").and_then(|x| x.as_str()).unwrap_or("");
                let x = action.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = action.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let button_idx = action.get("button").and_then(|v| v.as_i64()).unwrap_or(0);
                let duration = action
                    .get("duration")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                match t {
                    "pointerMove" => {
                        let buttons = pressed.iter().fold(0, |acc, b| acc | mouse_button_bit(*b));
                        dispatch_mouse(
                            &page,
                            DispatchMouseEventType::MouseMoved,
                            x,
                            y,
                            MouseButton::None,
                            buttons,
                            0,
                        )
                        .await
                        .map_err(|e| {
                            fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e)
                        })?;
                        if duration > 0 {
                            tokio::time::sleep(Duration::from_millis(duration)).await;
                        }
                    }
                    "pointerDown" => {
                        if !pressed.contains(&button_idx) {
                            pressed.push(button_idx);
                        }
                        let buttons = pressed.iter().fold(0, |acc, b| acc | mouse_button_bit(*b));
                        dispatch_mouse(
                            &page,
                            DispatchMouseEventType::MousePressed,
                            x,
                            y,
                            mouse_button(button_idx),
                            buttons,
                            1,
                        )
                        .await
                        .map_err(|e| {
                            fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e)
                        })?;
                    }
                    "pointerUp" => {
                        let buttons_before =
                            pressed.iter().fold(0, |acc, b| acc | mouse_button_bit(*b));
                        pressed.retain(|b| *b != button_idx);
                        let buttons = pressed.iter().fold(0, |acc, b| acc | mouse_button_bit(*b));
                        dispatch_mouse(
                            &page,
                            DispatchMouseEventType::MouseReleased,
                            x,
                            y,
                            mouse_button(button_idx),
                            buttons_before,
                            1,
                        )
                        .await
                        .map_err(|e| {
                            fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e)
                        })?;
                        let _ = buttons;
                    }
                    "pause" => {
                        if duration > 0 {
                            tokio::time::sleep(Duration::from_millis(duration)).await;
                        }
                    }
                    _ => {}
                }
            }
        } else if kind == "key" {
            for action in list {
                let t = action.get("type").and_then(|x| x.as_str()).unwrap_or("");
                let value = action.get("value").and_then(|v| v.as_str()).unwrap_or("");
                match t {
                    "keyDown" => {
                        if let Err(e) = dispatch_key(&page, value, true, &mut modifiers).await {
                            return Err(fail(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "unknown error",
                                e,
                            ));
                        }
                    }
                    "keyUp" => {
                        if let Err(e) = dispatch_key(&page, value, false, &mut modifiers).await {
                            return Err(fail(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "unknown error",
                                e,
                            ));
                        }
                    }
                    "pause" => {
                        let duration = action
                            .get("duration")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        if duration > 0 {
                            tokio::time::sleep(Duration::from_millis(duration)).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    ok(Value::Null)
}

async fn release_actions(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    // Release any stuck left mouse button and modifier keys.
    let _ = dispatch_mouse(
        &page,
        DispatchMouseEventType::MouseReleased,
        0.0,
        0.0,
        MouseButton::Left,
        0,
        1,
    )
    .await;
    ok(Value::Null)
}

async fn screenshot(
    State(state): State<Arc<AppState>>,
    AxPath(_sid): AxPath<String>,
) -> WebResult {
    let g = state.inner.lock().await;
    let page = g.page()?;
    let png = page
        .screenshot(
            chromiumoxide::page::ScreenshotParams::builder()
                .full_page(false)
                .build(),
        )
        .await
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, "unknown error", e.to_string()))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    ok(json!(b64))
}

fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/session", post(new_session))
        .route("/session/{sid}", delete(delete_session))
        .route("/session/{sid}/url", post(navigate).get(current_url))
        .route("/session/{sid}/window/new", post(new_window))
        .route("/session/{sid}/window/handles", get(handles))
        .route(
            "/session/{sid}/window",
            get(current_handle).post(switch_window).delete(close_window),
        )
        .route(
            "/session/{sid}/window/rect",
            get(get_window_rect).post(set_window_rect),
        )
        .route(
            "/session/{sid}/cookie",
            get(get_cookies).post(add_cookie).delete(delete_cookies),
        )
        .route("/session/{sid}/back", post(back))
        .route("/session/{sid}/forward", post(forward))
        .route("/session/{sid}/refresh", post(refresh))
        .route("/session/{sid}/title", get(title))
        .route("/session/{sid}/source", get(source))
        .route("/session/{sid}/frame", post(switch_frame))
        .route("/session/{sid}/element", post(find_element))
        .route("/session/{sid}/element/{eid}/text", get(element_text))
        .route("/session/{sid}/element/{eid}/click", post(element_click))
        .route("/session/{sid}/element/{eid}/clear", post(element_clear))
        .route("/session/{sid}/element/{eid}/value", post(element_value))
        .route("/session/{sid}/execute/sync", post(execute_sync))
        .route("/session/{sid}/actions", post(actions).delete(release_actions))
        .route("/session/{sid}/screenshot", get(screenshot))
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let port: u16 = std::env::var("AUTOMATA_BROWSER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9515);

    let state = Arc::new(AppState {
        inner: tokio::sync::Mutex::new(Inner::default()),
    });

    let app = router(state);
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Automata CDP browser engine listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

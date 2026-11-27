use serde_json::Value;

#[derive(Clone)]
pub struct Api {
    pub base: String,
}

fn parse_error(status: u16, body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
            return e.to_string();
        }
    }
    if body.trim().is_empty() {
        format!("HTTP {}", status)
    } else {
        format!("HTTP {}: {}", status, body.trim())
    }
}

impl Api {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let url = format!("{}{}", self.base, path);
        let req = ureq::request(method, &url);
        let result = match body {
            Some(value) => req
                .set("Content-Type", "application/json")
                .send_string(&value.to_string()),
            None => req.call(),
        };

        match result {
            Ok(resp) => {
                let text = resp.into_string().map_err(|e| e.to_string())?;
                if text.trim().is_empty() {
                    Ok(Value::Null)
                } else {
                    serde_json::from_str(&text).map_err(|e| format!("Bad JSON: {}", e))
                }
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(parse_error(code, &text))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn health(&self) -> bool {
        self.call("GET", "/health", None).is_ok()
    }

    pub fn create_session(&self, title: &str) -> Result<i64, String> {
        let v = self.call(
            "POST",
            "/api/sessions",
            Some(&serde_json::json!({ "title": title })),
        )?;
        v.get("sessionID")
            .and_then(|x| x.as_i64())
            .ok_or_else(|| "missing sessionID".to_string())
    }

    pub fn sessions(&self) -> Result<Vec<Value>, String> {
        let v = self.call("GET", "/api/sessions", None)?;
        Ok(v.get("sessions")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default())
    }

    pub fn session(&self, id: i64) -> Result<Value, String> {
        self.call("GET", &format!("/api/sessions/{}", id), None)
    }

    pub fn create_run(&self, session_id: i64, message: &str) -> Result<i64, String> {
        let v = self.call(
            "POST",
            &format!("/api/sessions/{}/runs", session_id),
            Some(&serde_json::json!({ "message": message })),
        )?;
        v.get("runID")
            .and_then(|x| x.as_i64())
            .ok_or_else(|| "missing runID".to_string())
    }

    pub fn run(&self, session_id: i64, run_id: i64) -> Result<Value, String> {
        self.call(
            "GET",
            &format!("/api/sessions/{}/runs/{}", session_id, run_id),
            None,
        )
    }

    pub fn stop_run(&self, session_id: i64, run_id: i64) -> Result<Value, String> {
        self.call(
            "POST",
            &format!("/api/sessions/{}/runs/{}/stop", session_id, run_id),
            Some(&serde_json::json!({})),
        )
    }

    pub fn close_browser(&self) -> Result<Value, String> {
        self.call("POST", "/api/browser/close", Some(&serde_json::json!({})))
    }

    pub fn browser_running(&self) -> bool {
        self.call("GET", "/api/browser/status", None)
            .ok()
            .and_then(|v| v.get("running").and_then(|x| x.as_bool()))
            .unwrap_or(false)
    }

    pub fn settings(&self) -> Result<Value, String> {
        self.call("GET", "/api/settings", None)
    }

    pub fn save_settings(&self, settings: &Value) -> Result<Value, String> {
        self.call("PUT", "/api/settings", Some(settings))
    }

    pub fn providers(&self) -> Result<Vec<Value>, String> {
        let v = self.call("GET", "/api/providers", None)?;
        Ok(v.get("providers")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default())
    }
}

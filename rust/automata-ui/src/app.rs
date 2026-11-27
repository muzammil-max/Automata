use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::api::Api;
use crate::dialog::{Dialog, DialogOutcome, Intent};
use crate::markdown;
use crate::model::{provider_name, short_model_for, MODEL_CATALOG};
use crate::splash::Splash;
use crate::theme::{self, *};

const DEFAULT_BASE: &str = "http://127.0.0.1:8080";
const SAFETY_PREFIX: &str = "SAFETY CONFIRMATION:";
const DEFAULT_SUBTITLE: &str = "Ask anything, or give a browser task";
const SUGGESTIONS: &[(&str, &str)] = &[
    ("Explain recursion", "Explain recursion in simple words."),
    (
        "What can you do?",
        "Who are you and what can you do?",
    ),
    (
        "Open a website",
        "Go to example.com and tell me the main heading.",
    ),
    (
        "Use a notepad",
        "Go to an online notepad and write a short note about OOP.",
    ),
];

/* ============================================================
   MODEL / CHAT STATE
   ============================================================ */

#[derive(Clone, Copy, PartialEq)]
enum Role {
    User,
    Assistant,
}

#[derive(Clone)]
struct ActivityLine {
    text: String,
    is_error: bool,
    at: Instant,
}

#[derive(Clone)]
struct Activity {
    status: String,
    lines: Vec<ActivityLine>,
    done: bool,
    expanded: bool,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            status: String::new(),
            lines: Vec::new(),
            done: false,
            expanded: true,
        }
    }
}

impl Activity {
    fn push_line(&mut self, text: String, is_error: bool) {
        self.lines.push(ActivityLine {
            text,
            is_error,
            at: Instant::now(),
        });
        while self.lines.len() > 9 {
            self.lines.remove(0);
        }
    }

    fn status_label(&self) -> String {
        if self.done {
            self.status.clone()
        } else if self.status.is_empty() {
            "WORKING".to_string()
        } else {
            self.status.to_uppercase()
        }
    }
}

struct ChatItem {
    role: Role,
    text: String,
    note: bool,
    activity: Option<Activity>,
    at: Instant,
}

struct SettingsForm {
    provider: String,
    model: String,
    base_url: String,
    api_key: String,
    clear_key: bool,
    has_key: bool,
    needs_key: bool,
    local: bool,
    env_key: String,
    temperature: f32,
    max_tokens: i64,
    reasoning: String,
}

impl Default for SettingsForm {
    fn default() -> Self {
        Self {
            provider: "nvidia".into(),
            model: "nvidia/nemotron-3.5-super-vl-120b-a12b".into(),
            base_url: String::new(),
            api_key: String::new(),
            clear_key: false,
            has_key: false,
            needs_key: true,
            local: false,
            env_key: "NVIDIA_API_KEY".into(),
            temperature: 0.7,
            max_tokens: 4096,
            reasoning: "high".into(),
        }
    }
}

#[derive(Clone)]
struct ProviderInfo {
    id: String,
    name: String,
    default_model: String,
    default_base_url: String,
    local: bool,
    needs_key: bool,
    env_key: String,
}

fn provider_from_json(v: &Value) -> ProviderInfo {
    ProviderInfo {
        id: v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("custom")
            .to_string(),
        name: v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("Custom")
            .to_string(),
        default_model: v
            .get("default_model")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        default_base_url: v
            .get("default_base_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        local: v.get("local").and_then(|x| x.as_bool()).unwrap_or(false),
        needs_key: v
            .get("needs_key")
            .and_then(|x| x.as_bool())
            .unwrap_or(true),
        env_key: v
            .get("env_key")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

fn default_providers() -> Vec<ProviderInfo> {
    [
        ("nvidia", "NVIDIA NIM", "nvidia/nemotron-3.5-super-vl-120b-a12b", "https://integrate.api.nvidia.com/v1", false, true, "NVIDIA_API_KEY"),
        ("openai", "OpenAI", "gpt-4o-mini", "https://api.openai.com/v1", false, true, "OPENAI_API_KEY"),
        ("anthropic", "Anthropic Claude", "claude-3-5-sonnet-latest", "https://api.anthropic.com/v1", false, true, "ANTHROPIC_API_KEY"),
        ("gemini", "Google Gemini", "gemini-1.5-flash", "https://generativelanguage.googleapis.com/v1beta", false, true, "GEMINI_API_KEY"),
        ("groq", "Groq", "llama-3.3-70b-versatile", "https://api.groq.com/openai/v1", false, true, "GROQ_API_KEY"),
        ("openrouter", "OpenRouter", "openai/gpt-4o-mini", "https://openrouter.ai/api/v1", false, true, "OPENROUTER_API_KEY"),
        ("ollama", "Ollama (local)", "llama3.1", "http://127.0.0.1:11434/v1", true, false, ""),
        ("lmstudio", "LM Studio (local)", "local-model", "http://127.0.0.1:1234/v1", true, false, ""),
        ("custom", "Custom / OpenAI-compatible", "", "", false, false, ""),
    ]
    .into_iter()
    .map(
        |(id, name, m, b, local, needs_key, env)| ProviderInfo {
            id: id.into(),
            name: name.into(),
            default_model: m.into(),
            default_base_url: b.into(),
            local,
            needs_key,
            env_key: env.into(),
        },
    )
    .collect()
}

enum Event {
    Startup {
        backend_ok: bool,
        browser_ok: bool,
        note: String,
    },
    Children(Option<Child>, Option<Child>),
    Sessions(Vec<Value>),
    SessionsOffline,
    SessionAssigned(i64),
    SessionLoaded {
        runs: Vec<Value>,
    },
    RunCreated {
        session_id: i64,
        run_id: i64,
    },
    RunUpdate(Value),
    Browser(bool),
    Settings(Value),
    Providers(Vec<Value>),
    SettingsSaved,
    Toast(String),
    SendFailed(String),
}

/* ============================================================
   APP
   ============================================================ */

struct App {
    api: Api,
    tx: Sender<Event>,
    rx: Receiver<Event>,

    splash: Splash,
    startup_done: bool,
    backend_ok: bool,
    browser_ok: bool,
    startup_note: String,
    startup_note_shown: bool,
    sidebar_open: bool,

    sessions: Vec<Value>,
    sessions_error: bool,
    selected: Option<i64>,
    chat: Vec<ChatItem>,

    busy: bool,
    current_run: Option<i64>,
    stopping: bool,
    stop_requested: bool,
    steps: Vec<Value>,
    last_step_count: Option<usize>,
    last_step_at: Instant,
    activity: Activity,
    stream_text: String,

    input: String,
    subtitle: String,
    status_line: String,
    status_until: Option<Instant>,

    settings_open: bool,
    form: SettingsForm,
    providers: Vec<ProviderInfo>,
    pending_key_hint: bool,

    dialog: Option<Dialog>,
    last_browser_check: Instant,
    grown: bool,
    maximized: bool,
    #[cfg(windows)]
    restore_rect: Option<(i32, i32, i32, i32)>,

    poll_cancel: Arc<AtomicBool>,
    children: Vec<Child>,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        api: Api,
        tx: Sender<Event>,
        rx: Receiver<Event>,
    ) -> Self {
        install_theme(&cc.egui_ctx);
        Self {
            api,
            tx,
            rx,
            splash: Splash::new(),
            startup_done: false,
            backend_ok: false,
            browser_ok: false,
            startup_note: String::new(),
            startup_note_shown: false,
            sidebar_open: false,
            sessions: Vec::new(),
            sessions_error: false,
            selected: None,
            chat: Vec::new(),
            busy: false,
            current_run: None,
            stopping: false,
            stop_requested: false,
            steps: Vec::new(),
            last_step_count: None,
            last_step_at: Instant::now(),
            activity: Activity::default(),
            stream_text: String::new(),
            input: String::new(),
            subtitle: DEFAULT_SUBTITLE.into(),
            status_line: String::new(),
            status_until: None,
            settings_open: false,
            form: SettingsForm::default(),
            providers: default_providers(),
            pending_key_hint: false,
            dialog: None,
            last_browser_check: Instant::now(),
            grown: false,
            maximized: false,
            #[cfg(windows)]
            restore_rect: None,
            poll_cancel: Arc::new(AtomicBool::new(false)),
            children: Vec::new(),
        }
    }

    /* ---------------- small helpers ---------------- */

    fn toast(&mut self, text: impl Into<String>) {
        self.status_line = text.into();
        self.status_until = Some(Instant::now() + Duration::from_secs(6));
    }

    fn note(&mut self, text: impl Into<String>) {
        self.chat.push(ChatItem {
            role: Role::Assistant,
            text: text.into(),
            note: true,
            activity: None,
            at: Instant::now(),
        });
    }

    fn push_message(&mut self, role: Role, text: impl Into<String>) {
        self.chat.push(ChatItem {
            role,
            text: text.into(),
            note: false,
            activity: None,
            at: Instant::now(),
        });
    }

    /* ---------------- data loading ---------------- */

    fn load_sessions(&self) {
        let api = self.api.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || match api.sessions() {
            Ok(list) => {
                let _ = tx.send(Event::Sessions(list));
            }
            Err(_) => {
                let _ = tx.send(Event::SessionsOffline);
            }
        });
    }

    fn load_session(&mut self, id: i64) {
        // Switching chats cancels any running poller.
        self.poll_cancel.store(true, Ordering::SeqCst);
        self.busy = false;
        self.stopping = false;
        self.stop_requested = false;
        self.current_run = None;
        self.steps.clear();
        self.last_step_count = None;
        self.activity = Activity::default();
        self.stream_text.clear();
        self.subtitle = DEFAULT_SUBTITLE.into();

        let api = self.api.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || match api.session(id) {
            Ok(v) => {
                let runs = v
                    .get("runs")
                    .and_then(|r| r.as_array())
                    .cloned()
                    .unwrap_or_default();
                let _ = tx.send(Event::SessionLoaded { runs });
            }
            Err(e) => {
                let _ = tx.send(Event::Toast(format!("Could not open chat: {}", e)));
            }
        });
    }

    fn load_settings(&self) {
        let api = self.api.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Ok(v) = api.settings() {
                let _ = tx.send(Event::Settings(v));
            }
            if let Ok(p) = api.providers() {
                let _ = tx.send(Event::Providers(p));
            }
        });
    }

    fn refresh_browser_status(&self) {
        let api = self.api.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Browser(api.browser_running()));
        });
    }

    /* ---------------- chat actions ---------------- */

    fn new_chat(&mut self) {
        if self.busy {
            self.dialog = Some(Dialog::info(
                "A task is running",
                "Stop the current task before starting a new chat.",
            ));
            return;
        }
        self.poll_cancel.store(true, Ordering::SeqCst);
        self.selected = None;
        self.chat.clear();
        self.current_run = None;
        self.steps.clear();
        self.last_step_count = None;
        self.activity = Activity::default();
        self.stream_text.clear();
        self.clear_stop_state();
        self.input.clear();
        self.subtitle = DEFAULT_SUBTITLE.into();
        self.toast("New chat");
    }

    fn clear_stop_state(&mut self) {
        self.stopping = false;
        self.stop_requested = false;
    }

    fn confirm_close_browser(&mut self) {
        self.dialog = Some(Dialog::confirm(
            "Close browser",
            "Close the Chrome window the agent is using?\n\nAny running browser task will fail.",
            "Close it",
            Intent::CloseBrowser,
            "Cancel",
        ));
    }

    fn close_browser(&mut self) {
        let api = self.api.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = api.close_browser() {
                let _ = tx.send(Event::Toast(format!("Could not close the browser: {}", e)));
            }
            let _ = tx.send(Event::Browser(api.browser_running()));
        });
    }

    fn stop_run(&mut self) {
        if !self.busy {
            return;
        }
        self.stop_requested = true;
        self.stopping = true;
        if let (Some(sid), Some(rid)) = (self.selected, self.current_run) {
            let api = self.api.clone();
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                if let Err(e) = api.stop_run(sid, rid) {
                    let _ = tx.send(Event::Toast(format!("Stop failed: {}", e)));
                }
            });
        }
    }

    fn send(&mut self, raw: impl Into<String>) {
        let message = raw.into().trim().to_string();
        if message.is_empty() || self.busy {
            return;
        }
        if !self.backend_ok {
            self.toast("Backend is not running yet");
            return;
        }

        self.input.clear();
        self.push_message(Role::User, message.clone());
        self.busy = true;
        self.stopping = false;
        self.stop_requested = false;
        self.steps.clear();
        self.last_step_count = None;
        self.current_run = None;
        self.stream_text.clear();
        self.subtitle = "Sending…".into();
        self.activity = Activity {
            status: "Working".into(),
            lines: Vec::new(),
            done: false,
            expanded: true,
        };
        self.activity
            .push_line("Connecting to the agent…".into(), false);

        let api = self.api.clone();
        let tx = self.tx.clone();
        let existing = self.selected;
        let title: String = message.chars().take(42).collect();

        self.spawn_poller_guard();
        let cancel = self.poll_cancel.clone();

        std::thread::spawn(move || {
            let sid = match existing {
                Some(id) => id,
                None => match api.create_session(&title) {
                    Ok(id) => {
                        let _ = tx.send(Event::SessionAssigned(id));
                        id
                    }
                    Err(e) => {
                        let _ = tx.send(Event::SendFailed(format!(
                            "I could not create that chat: {}",
                            e
                        )));
                        return;
                    }
                },
            };

            let run_id = match api.create_run(sid, &message) {
                Ok(id) => id,
                Err(e) => {
                    let _ = tx.send(Event::SendFailed(format!(
                        "I could not start that run: {}",
                        e
                    )));
                    return;
                }
            };

            let _ = tx.send(Event::RunCreated {
                session_id: sid,
                run_id,
            });

            poll_run(api, tx, sid, run_id, cancel);
        });

        self.load_sessions();
    }

    fn spawn_poller_guard(&mut self) {
        self.poll_cancel.store(true, Ordering::SeqCst);
        self.poll_cancel = Arc::new(AtomicBool::new(false));
    }

    fn resume_run(&mut self, run_id: i64, steps: Vec<Value>) {
        self.busy = true;
        self.current_run = Some(run_id);
        self.stopping = false;
        self.stop_requested = false;
        self.steps = steps.clone();
        self.last_step_count = Some(steps.len());
        self.last_step_at = Instant::now();
        self.stream_text.clear();
        self.activity = Activity {
            status: "Resuming".into(),
            lines: Vec::new(),
            done: false,
            expanded: true,
        };
        for step in &steps {
            if let Some((text, is_error)) = describe_step(step) {
                self.activity.push_line(text, is_error);
            }
        }
        if self.activity.lines.is_empty() {
            self.activity
                .push_line("Connecting to the agent…".into(), false);
        }
        self.subtitle = "Working…".into();

        let Some(sid) = self.selected else {
            self.busy = false;
            return;
        };
        self.spawn_poller_guard();
        let api = self.api.clone();
        let tx = self.tx.clone();
        let cancel = self.poll_cancel.clone();
        std::thread::spawn(move || poll_run(api, tx, sid, run_id, cancel));
    }

    /* ---------------- settings ---------------- */

    fn save_settings(&mut self) {
        let mut body = serde_json::json!({
            "provider": self.form.provider,
            "model": self.form.model,
            "base_url": self.form.base_url,
            "temperature": self.form.temperature,
            "max_tokens": self.form.max_tokens,
            "reasoning_effort": self.form.reasoning,
        });
        let key = self.form.api_key.trim().to_string();
        if !key.is_empty() {
            body["api_key"] = Value::String(key);
        } else if self.form.clear_key {
            body["api_key"] = Value::String(String::new());
        }

        let api = self.api.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || match api.save_settings(&body) {
            Ok(_) => {
                let _ = tx.send(Event::SettingsSaved);
            }
            Err(e) => {
                let _ = tx.send(Event::Toast(format!("Could not save settings: {}", e)));
            }
        });
    }

    fn apply_settings(&mut self, v: &Value) {
        let get_s = |k: &str, d: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or(d)
                .to_string()
        };
        self.form.provider = get_s("provider", "nvidia");
        self.form.model = get_s("model", "");
        self.form.base_url = get_s("base_url", "");
        self.form.reasoning = get_s("reasoning_effort", "high");
        self.form.temperature = v
            .get("temperature")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.7) as f32;
        self.form.max_tokens = v
            .get("max_tokens")
            .and_then(|x| x.as_i64())
            .unwrap_or(4096);
        self.form.has_key = v
            .get("has_api_key")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        self.form.needs_key = v
            .get("needs_key")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        self.form.local = v.get("local").and_then(|x| x.as_bool()).unwrap_or(false);
        self.form.env_key = get_s("env_key", "");
        self.form.api_key.clear();
        self.form.clear_key = false;

        if self.pending_key_hint {
            self.pending_key_hint = false;
            if self.form.needs_key && !self.form.has_key && !self.form.local {
                let name = provider_name(&self.form.provider);
                let hint = if self.form.env_key.is_empty() {
                    format!("{} needs an API key — add it in Settings.", name)
                } else {
                    format!(
                        "{} needs an API key — add it in Settings or set {}.",
                        name, self.form.env_key
                    )
                };
                self.toast(hint);
            }
        }
    }

    fn apply_session_runs(&mut self, runs: &[Value]) -> Option<(i64, Vec<Value>)> {
        self.chat.clear();
        let mut resume = None;
        let mut last_status = String::new();

        for run in runs {
            last_status = run
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let msg = run.get("message").and_then(|m| m.as_str()).unwrap_or("");
            if !msg.is_empty() {
                self.push_message(Role::User, msg);
            }
            let out = run.get("output").and_then(|m| m.as_str()).unwrap_or("");
            let terminal = matches!(last_status.as_str(), "completed" | "failed" | "stopped");
            if !out.is_empty() && terminal {
                self.push_message(Role::Assistant, out);
            }
            if matches!(last_status.as_str(), "queued" | "running") {
                let run_id = run.get("runID").and_then(|r| r.as_i64()).unwrap_or(0);
                let steps = run
                    .get("steps")
                    .and_then(|s| s.as_array())
                    .cloned()
                    .unwrap_or_default();
                if run_id > 0 {
                    resume = Some((run_id, steps));
                }
            }
        }

        if resume.is_none() {
            self.subtitle = if last_status.is_empty() {
                DEFAULT_SUBTITLE.into()
            } else {
                last_status
            };
        }
        resume
    }

    /* ---------------- event pump ---------------- */

    fn drain_events(&mut self) -> bool {
        let mut saw = false;
        while let Ok(event) = self.rx.try_recv() {
            saw = true;
            match event {
                Event::Startup {
                    backend_ok,
                    browser_ok,
                    note,
                } => {
                    self.startup_done = true;
                    self.backend_ok = backend_ok;
                    self.browser_ok = browser_ok;
                    self.startup_note = note.clone();
                    if !note.is_empty() && !self.startup_note_shown {
                        self.startup_note_shown = true;
                        self.note(note);
                        if !backend_ok {
                            self.toast("Backend offline — build/run api_server");
                        }
                    }
                }
                Event::Children(backend, browser) => {
                    if let Some(c) = backend {
                        self.children.push(c);
                    }
                    if let Some(c) = browser {
                        self.children.push(c);
                    }
                }
                Event::Sessions(list) => {
                    self.sessions = list;
                    self.sessions_error = false;
                }
                Event::SessionsOffline => {
                    self.sessions.clear();
                    self.sessions_error = true;
                }
                Event::SessionAssigned(id) => {
                    self.selected = Some(id);
                }
                Event::SessionLoaded { runs } => {
                    let resume = self.apply_session_runs(&runs);
                    if let Some((run_id, steps)) = resume {
                        self.resume_run(run_id, steps);
                    }
                }
                Event::RunCreated {
                    session_id,
                    run_id,
                } => {
                    self.selected = Some(session_id);
                    self.current_run = Some(run_id);
                    self.subtitle = "Working…".into();
                    if self.stop_requested {
                        let api = self.api.clone();
                        let tx = self.tx.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = api.stop_run(session_id, run_id) {
                                let _ = tx
                                    .send(Event::Toast(format!("Stop failed: {}", e)));
                            }
                        });
                    }
                }
                Event::RunUpdate(run) => self.on_run_update(run),
                Event::SendFailed(e) => {
                    self.busy = false;
                    self.stopping = false;
                    self.stop_requested = false;
                    self.current_run = None;
                    self.stream_text.clear();
                    self.subtitle = "The last run failed".into();
                    self.push_message(Role::Assistant, e.clone());
                    self.toast(e);
                }
                Event::Browser(running) => {
                    self.browser_ok = running;
                }
                Event::Settings(v) => {
                    self.apply_settings(&v);
                }
                Event::Providers(list) => {
                    if !list.is_empty() {
                        self.providers = list.iter().map(provider_from_json).collect();
                    }
                }
                Event::SettingsSaved => {
                    self.load_settings();
                    self.toast("Settings saved");
                }
                Event::Toast(msg) => {
                    self.toast(msg);
                }
            }
        }
        saw
    }

    fn on_run_update(&mut self, run: Value) {
        let status = run
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let steps = run
            .get("steps")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();

        let previous = self.last_step_count.unwrap_or(0);
        if steps.len() != previous {
            let fresh: &[Value] = if self.last_step_count.is_none() {
                &steps
            } else if steps.len() > previous {
                &steps[previous..]
            } else {
                &[]
            };
            for step in fresh {
                if let Some((text, is_error)) = describe_step(step) {
                    self.activity.push_line(text, is_error);
                }
            }
            self.steps = steps.clone();
            self.last_step_count = Some(steps.len());
            self.last_step_at = Instant::now();

            if let Some(last) = steps.last() {
                let action = last.get("action").and_then(|a| a.as_str()).unwrap_or("");
                match action {
                    "chat" => {
                        self.activity.status = "Working".into();
                        self.subtitle = "Working on your message…".into();
                    }
                    "navigate" | "new_tab" => {
                        self.activity.status = "Browsing".into();
                        let target = last.get("target").and_then(|t| t.as_str()).unwrap_or("");
                        self.subtitle = format!("Browser task · {}", host_of(target));
                    }
                    "type" | "drag" => {
                        self.activity.status = "Working".into();
                        self.subtitle = "Working on the page…".into();
                    }
                    _ => {
                        self.activity.status = "Working".into();
                    }
                }
            }
        }

        let terminal = matches!(status.as_str(), "completed" | "failed" | "stopped");
        if terminal {
            self.stream_text.clear();
            self.finish_run(&run, &steps, &status);
            return;
        }

        let output = run
            .get("output")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if !output.is_empty() {
            self.stream_text = output;
        }

        if self.stopping {
            self.activity.status = "Stopping".into();
        } else if self.last_step_at.elapsed() > Duration::from_millis(1600) {
            self.activity.status = "Working".into();
            self.subtitle = "Working…".into();
        }
    }

    fn finish_run(&mut self, run: &Value, steps: &[Value], status: &str) {
        self.busy = false;
        self.stopping = false;
        self.stop_requested = false;
        self.current_run = None;
        self.last_step_count = None;
        self.stream_text.clear();

        let used_browser = steps
            .iter()
            .any(|s| !matches!(s.get("action").and_then(|a| a.as_str()).unwrap_or(""), "chat" | "browser_reuse"));

        if used_browser {
            let label = match status {
                "completed" => "DONE",
                "stopped" => "STOPPED",
                _ => "FINISHED WITH A PROBLEM",
            };
            let mut snapshot = self.activity.clone();
            snapshot.status = label.into();
            snapshot.done = true;
            self.chat.push(ChatItem {
                role: Role::Assistant,
                text: String::new(),
                note: false,
                activity: Some(snapshot),
                at: Instant::now(),
            });
        }

        let output = run
            .get("output")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if output.starts_with(SAFETY_PREFIX) {
            self.push_message(
                Role::Assistant,
                "I stopped before a risky action. Please confirm if I should continue.",
            );
            let ask = output[SAFETY_PREFIX.len()..].trim().to_string();
            self.dialog = Some(Dialog::confirm(
                "Confirm action",
                if ask.is_empty() {
                    "The agent wants to do something that may be irreversible.".into()
                } else {
                    ask
                },
                "Continue",
                Intent::SafetyContinue,
                "Cancel",
            ));
        } else if !output.is_empty() {
            self.push_message(Role::Assistant, output);
        } else if status == "failed" {
            self.push_message(
                Role::Assistant,
                "I could not finish that task. Please try again or give me a smaller step.",
            );
        } else if status == "stopped" {
            self.push_message(
                Role::Assistant,
                "Stopped. Tell me when you want me to continue.",
            );
        }

        self.subtitle = match status {
            "completed" => {
                if used_browser {
                    "Task completed".into()
                } else {
                    "Ready".into()
                }
            }
            "stopped" => "Stopped".into(),
            _ => "The last run failed".into(),
        };

        self.load_sessions();
        self.refresh_browser_status();
    }

    fn chat_title(&self) -> String {
        if let Some(id) = self.selected {
            if let Some(s) = self.sessions.iter().find(|s| {
                s.get("sessionID").and_then(|v| v.as_i64()) == Some(id)
            }) {
                let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
                if !title.is_empty() && title != "Automata Session" && title != "New chat" {
                    return truncate(title, 60);
                }
            }
            return format!("Chat {}", id);
        }
        "New chat".into()
    }

    fn select_model(&mut self, provider: &str, model: &str) {
        let provider_changed = self.form.provider != provider;
        self.form.provider = provider.to_string();
        self.form.model = model.to_string();
        if provider_changed {
            if let Some(p) = self.providers.iter().find(|p| p.id == provider) {
                self.form.base_url = p.default_base_url.clone();
            }
            // The backend drops the stored key when the provider changes.
            self.form.has_key = false;
            self.pending_key_hint = true;
        }
        self.save_settings();
    }

    fn set_thinking(&mut self, level: &str) {
        if self.form.reasoning != level {
            self.form.reasoning = level.to_string();
            self.save_settings();
        }
    }

    /* ============================================================
       PAINTING
       ============================================================ */

    fn header(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let _ = &frame;
        egui::Panel::top("header")
            .exact_size(58.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(HEADER_FILL)
                    .inner_margin(egui::Margin {
                        left: 30,
                        right: 20,
                        top: 8,
                        bottom: 8,
                    }),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(self.chat_title())
                                .font(body_font(12.0))
                                .strong()
                                .color(NAVY),
                        );
                        ui.label(
                            egui::RichText::new(&self.subtitle)
                                .font(body_font(11.5))
                                .color(MUTED),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ctx = ui.ctx().clone();
                        if theme::win_btn(ui, "×", true) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let maximized_now = self.maximized;
                        let maximize_clicked = theme::win_btn_draw(ui, false, |painter, rect, color| {
                            let size = egui::vec2(10.0, 9.0);
                            let front = egui::Rect::from_center_size(
                                rect.center() + egui::vec2(1.5, 1.5),
                                size,
                            );
                            painter.rect_stroke(
                                front,
                                1.0,
                                egui::Stroke::new(1.2, color),
                                egui::StrokeKind::Inside,
                            );
                            if maximized_now {
                                let back = front.translate(egui::vec2(-2.5, -2.5));
                                painter.rect_stroke(
                                    back,
                                    1.0,
                                    egui::Stroke::new(1.2, color),
                                    egui::StrokeKind::Inside,
                                );
                            }
                        });
                        if maximize_clicked {
                            #[cfg(windows)]
                            toggle_maximize_native(
                                frame,
                                &mut self.maximized,
                                &mut self.restore_rect,
                            );
                            #[cfg(not(windows))]
                            {
                                let maximized = ui
                                    .input(|i| i.viewport().maximized)
                                    .unwrap_or(false);
                                ctx.send_viewport_cmd(
                                    egui::ViewportCommand::Maximized(!maximized),
                                );
                            }
                        }
                        if theme::win_btn(ui, "–", false) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        // `☰` is not in egui's default fonts, so draw it.
                        if theme::win_btn_draw(ui, false, |painter, rect, color| {
                            let w = 13.0;
                            let th = 1.6;
                            for i in -1..=1 {
                                let y = rect.center().y + i as f32 * 4.4;
                                painter.rect_filled(
                                    egui::Rect::from_center_size(
                                        egui::pos2(rect.center().x, y),
                                        egui::vec2(w, th),
                                    ),
                                    th * 0.5,
                                    color,
                                );
                            }
                        }) {
                            self.sidebar_open = !self.sidebar_open;
                        }

                        // Frameless window: the empty part of the header is the
                        // title bar you grab to move the window.
                        let strip = ui.available_width();
                        if strip > 0.0 {
                            let (_, resp) = ui.allocate_exact_size(
                                egui::vec2(strip, ui.available_height()),
                                egui::Sense::click_and_drag(),
                            );
                            if resp.drag_started() {
                                ui.ctx()
                                    .send_viewport_cmd(egui::ViewportCommand::StartDrag);
                            }
                            if resp.double_clicked() {
                                #[cfg(windows)]
                                toggle_maximize_native(
                                    frame,
                                    &mut self.maximized,
                                    &mut self.restore_rect,
                                );
                                #[cfg(not(windows))]
                                ui.ctx().send_viewport_cmd(
                                    egui::ViewportCommand::Maximized(true),
                                );
                            }
                        }
                    });
                });
            });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .exact_size(278.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(INK)
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 22,
                        bottom: 16,
                    }),
            )
            .show(ui, |ui| {
                // Deep blue sidebar with a soft glow in the top-left corner.
                let rect = ui.max_rect().expand2(egui::vec2(16.0, 16.0));
                theme::fill_vertical_gradient(ui.painter(), rect, INK, INK_2);
                theme::fill_radial(
                    ui.painter(),
                    rect.left_top(),
                    egui::vec2(rect.width() * 1.2, rect.height() * 0.8),
                    48,
                    |t| {
                        if t < 0.55 {
                            with_alpha(GLOW_BLUE, (36.0 * (1.0 - t / 0.55)).round() as u8)
                        } else {
                            Color32::TRANSPARENT
                        }
                    },
                );

                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(42.0, 35.0), egui::Sense::hover());
                    paint_logo(ui.painter(), rect, CREAM);
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Automata")
                                .font(display_font(24.0))
                                .color(CREAM),
                        );
                        ui.label(
                            egui::RichText::new("OOP LAB PROJECT")
                                .font(mono_font(8.0))
                                .color(INK_MUTED),
                        );
                    });
                });

                ui.add_space(16.0);
                let (btn_rect, btn_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 40.0),
                    egui::Sense::click(),
                );
                let hot = btn_resp.hovered() || btn_resp.is_pointer_button_down_on();
                ui.painter().rect_stroke(
                    btn_rect,
                    8,
                    egui::Stroke::new(1.0, if hot { AMBER } else { AMBER_LINE }),
                    egui::StrokeKind::Inside,
                );
                ui.painter().rect_filled(
                    btn_rect,
                    8,
                    if hot { AMBER_SOFT } else { AMBER_TINT },
                );
                let plus = egui::RichText::new("+")
                    .font(mono_font(15.0))
                    .color(AMBER);
                let new_chat = egui::RichText::new("NEW CHAT")
                    .font(mono_font(11.0))
                    .color(CREAM);
                let galley_plus = ui.painter().layout_no_wrap(
                    "+".into(),
                    mono_font(15.0),
                    AMBER,
                );
                let galley_text = ui.painter().layout_no_wrap(
                    "NEW CHAT".into(),
                    mono_font(11.0),
                    CREAM,
                );
                let plus_size = galley_plus.size();
                let text_size = galley_text.size();
                let total = plus_size.x + 8.0 + text_size.x;
                let start = btn_rect.left() + (btn_rect.width() - total) * 0.5;
                let cy = btn_rect.center().y;
                ui.painter()
                    .galley(egui::pos2(start, cy - plus_size.y * 0.5), galley_plus, AMBER);
                ui.painter().galley(
                    egui::pos2(start + plus_size.x + 8.0, cy - text_size.y * 0.5),
                    galley_text,
                    CREAM,
                );
                let _ = (plus, new_chat);
                if btn_resp.clicked() && !self.busy {
                    self.new_chat();
                } else if btn_resp.clicked() && self.busy {
                    self.dialog = Some(Dialog::info(
                        "A task is running",
                        "Stop the current task before starting a new chat.",
                    ));
                }

                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("HISTORY")
                        .font(mono_font(9.0))
                        .color(INK_MUTED),
                );
                ui.add_space(4.0);

                let list_height = (ui.available_height() - 150.0).max(80.0);
                let mut clicked: Option<i64> = None;
                egui::ScrollArea::vertical()
                    .id_salt("history")
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.sessions.is_empty() {
                            let text = if self.sessions_error {
                                "History is offline"
                            } else {
                                "No chats yet"
                            };
                            ui.label(
                                egui::RichText::new(text)
                                    .font(body_font(12.0))
                                    .color(INK_MUTED),
                            );
                            return;
                        }
                        let mut ordered = self.sessions.clone();
                        ordered.reverse();
                        for session in ordered {
                            let id = session
                                .get("sessionID")
                                .and_then(|x| x.as_i64())
                                .unwrap_or(0);
                            let title = session
                                .get("title")
                                .and_then(|x| x.as_str())
                                .filter(|t| !t.is_empty() && *t != "Automata Session")
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| format!("Chat {}", id));
                            let run_count = session
                                .get("runCount")
                                .and_then(|x| x.as_i64())
                                .unwrap_or(0);
                            let meta = if run_count == 1 {
                                "1 message".to_string()
                            } else {
                                format!("{} messages", run_count)
                            };
                            let active = self.selected == Some(id);

                            let width = ui.available_width();
                            let (rect, resp) =
                                ui.allocate_exact_size(egui::vec2(width, 46.0), egui::Sense::click());
                            if resp.hovered() && !active {
                                ui.painter().rect_filled(rect, 8.0, WHITE_05);
                            }
                            if active {
                                ui.painter().rect_filled(rect, 8.0, WHITE_07);
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(rect.left(), rect.top() + 6.0),
                                        egui::vec2(2.0, rect.height() - 12.0),
                                    ),
                                    1.0,
                                    AMBER,
                                );
                            }
                            let x = rect.left() + 12.0;
                            ui.painter().text(
                                egui::pos2(x, rect.top() + 10.0),
                                egui::Align2::LEFT_TOP,
                                title,
                                body_font(12.5),
                                SESSION_TITLE,
                            );
                            ui.painter().text(
                                egui::pos2(x, rect.top() + 28.0),
                                egui::Align2::LEFT_TOP,
                                meta,
                                mono_font(9.0),
                                INK_MUTED,
                            );
                            if resp.clicked() {
                                clicked = Some(id);
                            }
                        }
                    });

                if let Some(id) = clicked {
                    if self.busy {
                        self.dialog = Some(Dialog::info(
                            "A task is running",
                            "Stop the current task before opening another chat.",
                        ));
                    } else {
                        self.selected = Some(id);
                        self.load_session(id);
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(12.0);
                    let (pill_rect, pill_resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 34.0),
                        egui::Sense::click(),
                    );
                    ui.painter().rect_filled(pill_rect, 17.0, WHITE_04);
                    ui.painter().rect_stroke(
                        pill_rect,
                        17.0,
                        egui::Stroke::new(1.0, INK_LINE),
                        egui::StrokeKind::Inside,
                    );
                    let dot_c = egui::pos2(pill_rect.left() + 13.0, pill_rect.center().y);
                    if self.browser_ok {
                        ui.painter()
                            .circle_filled(dot_c, 6.0, with_alpha(GREEN, 41));
                        ui.painter().circle_filled(dot_c, 3.5, GREEN);
                    } else {
                        ui.painter()
                            .circle_filled(dot_c, 6.0, with_alpha(CREAM, 13));
                        ui.painter()
                            .circle_filled(dot_c, 3.5, Color32::from_rgba_premultiplied(60, 60, 61, 71));
                    }
                    let pill_text = if self.browser_ok {
                        "BROWSER RUNNING"
                    } else {
                        "BROWSER OFF"
                    };
                    ui.painter().text(
                        egui::pos2(pill_rect.left() + 26.0, pill_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        pill_text,
                        mono_font(9.5),
                        PILL_TEXT,
                    );
                    if pill_resp.clicked() {
                        self.refresh_browser_status();
                    }

                    ui.add_space(6.0);
                    if theme::ghost_button(ui, "Close browser") {
                        if self.browser_ok {
                            self.confirm_close_browser();
                        } else {
                            self.toast("The browser is not running.");
                        }
                    }

                    ui.add_space(2.0);
                    if theme::ghost_button(ui, "Settings") {
                        self.settings_open = true;
                        self.load_settings();
                    }
                });
            });
    }

    fn hero(&mut self, ui: &mut egui::Ui) {
        let height = (ui.available_height() - 30.0).max(260.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let screen_w = ui.input(|i| i.viewport_rect().width());
        let title_size = (screen_w * 0.07).clamp(48.0, 74.0);

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
            |ui| {
                ui.add_space(24.0);
                let (logo_rect, _) =
                    ui.allocate_exact_size(egui::vec2(92.0, 92.0 * 40.0 / 48.0), egui::Sense::hover());
                paint_logo(ui.painter(), logo_rect, NAVY);
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Automata")
                        .font(display_font(title_size))
                        .color(NAVY),
                );
                ui.add_space(10.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "A browser agent that can also talk. Ask a question, or tell me exactly what to do on a website.",
                        )
                        .font(body_font(14.0))
                        .color(MUTED),
                    )
                    .wrap(),
                );
                ui.add_space(28.0);
                ui.horizontal_wrapped(|ui| {
                    for (label, prompt) in SUGGESTIONS {
                        if prompt_chip(ui, label) {
                            let text = (*prompt).to_string();
                            self.send(text);
                        }
                    }
                });
            },
        );
    }

    fn chat_area(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(PAPER)
                    .inner_margin(egui::Margin {
                        left: 30,
                        right: 30,
                        top: 26,
                        bottom: 10,
                    }),
            )
            .show(ui, |ui| {
                let rect = ui.max_rect();
                theme::fill_radial(
                    ui.painter(),
                    egui::pos2(rect.right(), rect.top()),
                    egui::vec2(rect.width() * 0.9, rect.height() * 0.6),
                    48,
                    |t| {
                        if t < 0.55 {
                            with_alpha(GLOW_BLUE, (31.0 * (1.0 - t / 0.55)).round() as u8)
                        } else {
                            Color32::TRANSPARENT
                        }
                    },
                );

                let width = ui.available_width();
                ui.vertical(|ui| {
                    ui.set_width(width);
                    if self.chat.is_empty() && !self.busy {
                        self.hero(ui);
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("chat")
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for index in 0..self.chat.len() {
                                self.message_item(ui, index);
                            }
                            if self.busy {
                                ui.add_space(14.0);
                                if self.stream_text.is_empty() {
                                    let mut activity = std::mem::take(&mut self.activity);
                                    activity_row(ui, &mut activity, false);
                                    self.activity = activity;
                                } else {
                                    streaming_item(ui, &self.stream_text);
                                }
                            }
                            ui.add_space(18.0);
                        });
                });
            });
    }

    fn message_item(&mut self, ui: &mut egui::Ui, index: usize) {
        let mut copied = false;
        {
            let item = &mut self.chat[index];
            if item.note {
                ui.add_space(6.0);
                ui.vertical_centered(|ui| {
                    let galley = ui.painter().layout_no_wrap(
                        item.text.clone(),
                        mono_font(10.0),
                        MUTED,
                    );
                    let size = galley.size() + egui::vec2(26.0, 12.0);
                    let (rect, _) =
                        ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter().rect_stroke(
                        rect,
                        255,
                        egui::Stroke::new(1.0, LINE),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().rect_filled(rect, 255, CHIP_FILL);
                    ui.painter().galley(
                        egui::pos2(
                            rect.center().x - galley.size().x * 0.5,
                            rect.top() + 6.0,
                        ),
                        galley,
                        MUTED,
                    );
                });
                return;
            }

            if let Some(activity) = &mut item.activity {
                ui.add_space(14.0);
                activity_row(ui, activity, true);
                return;
            }

            // New messages fade in instead of popping.
            let appear = (item.at.elapsed().as_secs_f32() / 0.18).clamp(0.0, 1.0);
            if appear < 1.0 {
                ui.ctx().request_repaint_after(Duration::from_millis(24));
            }
            let alpha = (255.0 * appear) as u8;

            ui.add_space(14.0);
            let max = (ui.available_width() * 0.82).min(680.0);
            match item.role {
                Role::User => {
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::TOP),
                        |ui| {
                            paint_user_avatar(ui);
                            ui.add_space(10.0);
                            egui::Frame::new()
                                .fill(with_alpha(USER_BUBBLE, alpha))
                                .stroke(egui::Stroke::new(1.0, USER_BORDER))
                                .corner_radius(egui::CornerRadius {
                                    nw: 20,
                                    ne: 4,
                                    sw: 20,
                                    se: 20,
                                })
                                .inner_margin(egui::Margin::symmetric(16, 12))
                                .show(ui, |ui| {
                                    ui.set_max_width(max);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&item.text)
                                                .font(body_font(14.0))
                                                .color(with_alpha(CREAM, alpha)),
                                        )
                                        .wrap(),
                                    );
                                });
                        },
                    );
                }
                Role::Assistant => {
                    let text = item.text.clone();
                    ui.with_layout(
                        egui::Layout::left_to_right(egui::Align::TOP),
                        |ui| {
                            paint_assistant_avatar(ui);
                            ui.add_space(10.0);
                            ui.vertical(|ui| {
                                ui.set_max_width(max);
                                egui::Frame::new()
                                    .fill(with_alpha(
                                        Color32::from_rgba_premultiplied(
                                            18, 34, 60, 150,
                                        ),
                                        alpha,
                                    ))
                                    .stroke(egui::Stroke::new(1.0, with_alpha(LINE, alpha)))
                                    .corner_radius(14)
                                    .inner_margin(egui::Margin::symmetric(16, 12))
                                    .show(ui, |ui| {
                                        ui.set_max_width(max);
                                        markdown::render(
                                            ui,
                                            &text,
                                            with_alpha(NAVY, alpha),
                                        );

                                        // Copy button, visible on hover.
                                        ui.add_space(2.0);
                                        let copy = ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("COPY")
                                                        .font(mono_font(8.5))
                                                        .color(MUTED),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    with_alpha(LINE, alpha),
                                                ))
                                                .corner_radius(8),
                                            );
                                        if copy.clicked() {
                                            ui.ctx().copy_text(text.clone());
                                            copied = true;
                                        }
                                    });
                            });
                        },
                    );
                }
            }
        }
        if copied {
            self.toast("Copied to clipboard");
        }
    }

}

fn activity_row(ui: &mut egui::Ui, activity: &mut Activity, _finished: bool) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        paint_assistant_avatar(ui);
        ui.add_space(10.0);
        ui.vertical(|ui| {
            let time = ui.input(|i| i.time);
            let mut toggle = false;

            ui.horizontal(|ui| {
                let (spinner, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
                let center = spinner.center();
                if activity.done {
                    let failed = activity.status.contains("PROBLEM")
                        || activity.status.contains("FAIL");
                    let color = if failed { RED } else { GREEN };
                    ui.painter().line_segment(
                        [
                            center + egui::vec2(-4.5, 0.0),
                            center + egui::vec2(-1.0, 3.5),
                        ],
                        egui::Stroke::new(1.6, color),
                    );
                    ui.painter().line_segment(
                        [
                            center + egui::vec2(-1.0, 3.5),
                            center + egui::vec2(4.5, -4.0),
                        ],
                        egui::Stroke::new(1.6, color),
                    );
                } else {
                    for i in 0..8 {
                        let angle =
                            time as f32 * 3.4 + i as f32 * std::f32::consts::TAU / 8.0;
                        let alpha = (35.0 + (i as f32 / 8.0) * 220.0) as u8;
                        let pos = center + egui::vec2(angle.cos(), angle.sin()) * 5.5;
                        ui.painter()
                            .circle_filled(pos, 1.35, with_alpha(AMBER, alpha));
                    }
                }

                let chevron = if activity.expanded { "▾" } else { "▸" };
                let chevron_resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(chevron)
                            .font(mono_font(10.0))
                            .color(MUTED),
                    )
                    .sense(egui::Sense::click()),
                );
                if chevron_resp.clicked() {
                    toggle = true;
                }

                let status_text = activity.status_label();
                let status_color = if activity.done {
                    NAVY_SOFT
                } else {
                    let wave = ((time * 1.1f64).sin() * 0.5 + 0.5) as f32;
                    theme::lerp(
                        Color32::from_rgba_premultiplied(214, 217, 222, 190),
                        AMBER,
                        wave,
                    )
                };
                let status_resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(status_text)
                            .font(mono_font(11.0))
                            .color(status_color),
                    )
                    .sense(egui::Sense::click()),
                );
                if status_resp.clicked() {
                    toggle = true;
                }

                if !activity.done {
                    ui.add_space(8.0);
                    let elapsed = activity
                        .lines
                        .last()
                        .map(|line| line.at.elapsed().as_secs_f32())
                        .unwrap_or(0.0);
                    ui.label(
                        egui::RichText::new(format!("{:.1}s", elapsed))
                            .font(mono_font(9.5))
                            .color(MUTED),
                    );
                }
            });

            if toggle {
                activity.expanded = !activity.expanded;
            }

            if activity.expanded && !activity.lines.is_empty() {
                ui.add_space(10.0);
                for (index, line) in activity.lines.iter().enumerate() {
                    let latest = index + 1 == activity.lines.len();
                    let appear =
                        (line.at.elapsed().as_secs_f32() / 0.26).clamp(0.0, 1.0);
                    let alpha = (255.0 * appear) as u8;
                    let slide = (1.0 - appear) * 7.0;

                    ui.horizontal(|ui| {
                        ui.add_space(slide);
                        let (dot, _) = ui.allocate_exact_size(
                            egui::vec2(5.0, 5.0),
                            egui::Sense::hover(),
                        );
                        let base = if line.is_error {
                            RED
                        } else if latest && !activity.done {
                            AMBER
                        } else {
                            with_alpha(AMBER, 217)
                        };
                        ui.painter()
                            .circle_filled(dot.center(), 2.5, with_alpha(base, alpha));
                        if latest && !activity.done {
                            let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
                            ui.painter().circle_filled(
                                dot.center(),
                                4.5 + pulse * 1.5,
                                with_alpha(AMBER, (18.0 + pulse * 26.0) as u8),
                            );
                        }
                        ui.add_space(3.0);
                        let color = if line.is_error {
                            RED
                        } else if latest {
                            NAVY
                        } else {
                            MUTED
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&line.text)
                                    .font(body_font(12.5))
                                    .color(with_alpha(color, alpha)),
                            )
                            .wrap(),
                        );
                    });
                }
            } else if !activity.expanded && !activity.done {
                if let Some(line) = activity.lines.last() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(&line.text)
                            .font(body_font(12.0))
                            .color(MUTED),
                    );
                }
            }

            if !activity.done {
                ui.ctx().request_repaint_after(Duration::from_millis(60));
            }
        });
    });
}

fn streaming_item(ui: &mut egui::Ui, text: &str) {
    let max = (ui.available_width() * 0.82).min(680.0);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        paint_assistant_avatar(ui);
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_max_width(max);
            let time = ui.input(|i| i.time);
            let on = ((time * 2.4) as i64) % 2 == 0;
            let mut body = text.to_string();
            body.push(if on { '▍' } else { ' ' });
            egui::Frame::new()
                .fill(Color32::from_rgba_premultiplied(18, 34, 60, 150))
                .stroke(egui::Stroke::new(1.0, LINE))
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_max_width(max);
                    markdown::render(ui, &body, NAVY);
                });
        });
    });
}

impl App {
    fn composer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("composer")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(PAPER)
                    .inner_margin(egui::Margin {
                        left: 30,
                        right: 30,
                        top: 8,
                        bottom: 20,
                    }),
            )
            .show(ui, |ui| {
                let width = ui.available_width();
                ui.vertical(|ui| {
                    ui.set_width(width);

                        if let Some(until) = self.status_until {
                            if Instant::now() < until {
                                ui.label(
                                    egui::RichText::new(&self.status_line)
                                        .font(mono_font(9.5))
                                        .color(MUTED),
                                );
                                ui.add_space(6.0);
                            } else {
                                self.status_until = None;
                                self.status_line.clear();
                            }
                        }

                        let mut picked_model: Option<(String, String)> = None;
                        let mut picked_level: Option<String> = None;

                        egui::Frame::new()
                            .fill(PAPER_2)
                            .stroke(egui::Stroke::new(1.0, LINE))
                            .corner_radius(20)
                            .inner_margin(egui::Margin::symmetric(16, 8))
                            .show(ui, |ui| {
                                ui.set_width(width - 32.0);

                                ui.horizontal(|ui| {
                                    let label = if self.form.model.is_empty() {
                                        "Select model".to_string()
                                    } else {
                                        format!(
                                            "{} · {}",
                                            provider_name(&self.form.provider),
                                            short_model_for(&self.form.provider, &self.form.model)
                                        )
                                    };
                                    egui::ComboBox::from_id_salt("composer_model")
                                        .selected_text(
                                            egui::RichText::new(label)
                                                .font(mono_font(10.0))
                                                .color(NAVY),
                                        )
                                        .width(260.0)
                                        .show_ui(ui, |ui| {
                                            for (provider, models) in MODEL_CATALOG {
                                                ui.label(
                                                    egui::RichText::new(
                                                        provider_name(provider).to_uppercase(),
                                                    )
                                                    .font(mono_font(8.0))
                                                    .color(MUTED),
                                                );
                                                for (model, short) in *models {
                                                    let selected = self.form.provider == *provider
                                                        && self.form.model == *model;
                                                    if ui
                                                        .selectable_label(
                                                            selected,
                                                            egui::RichText::new(*short)
                                                                .font(mono_font(10.0)),
                                                        )
                                                        .on_hover_text(*model)
                                                        .clicked()
                                                    {
                                                        picked_model = Some((
                                                            provider.to_string(),
                                                            model.to_string(),
                                                        ));
                                                    }
                                                }
                                                ui.separator();
                                            }
                                        });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            for level in ["high", "medium", "low"] {
                                                if chip_toggle(
                                                    ui,
                                                    &level.to_uppercase(),
                                                    self.form.reasoning == level,
                                                ) {
                                                    picked_level = Some(level.to_string());
                                                }
                                            }
                                        },
                                    );
                                });

                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    let available = (ui.available_width() - 54.0).max(120.0);
                                    let rows = self
                                        .input
                                        .lines()
                                        .count()
                                        .clamp(1, 8);
                                    let edit = egui::TextEdit::multiline(&mut self.input)
                                        .id(egui::Id::unique("automata_composer_input"))
                                        .desired_rows(rows)
                                        .desired_width(available)
                                        .frame(egui::Frame::NONE)
                                        .hint_text(
                                            egui::RichText::new("Message Automata…")
                                                .font(body_font(14.5))
                                                .color(PLACEHOLDER),
                                        )
                                        .font(body_font(14.5));
                                    let resp = ui.add(edit);

                                    let enter = ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                                    });
                                    if enter
                                        && resp.has_focus()
                                        && !self.busy
                                        && !self.input.trim().is_empty()
                                    {
                                        ui.input_mut(|i| {
                                            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                                        });
                                        let text = self.input.clone();
                                        self.send(text);
                                    }

                                    if self.busy {
                                        let stop = egui::Button::new(
                                            egui::RichText::new("Stop")
                                                .font(mono_font(12.0))
                                                .color(RED),
                                        )
                                        .fill(STOP_FILL)
                                        .stroke(egui::Stroke::new(1.0, STOP_LINE))
                                        .corner_radius(12)
                                        .min_size(egui::vec2(64.0, 38.0));
                                        if ui
                                            .add(stop)
                                            .on_hover_text("Stop the current task")
                                            .clicked()
                                        {
                                            self.stop_run();
                                        }
                                    } else {
                                        let can_send =
                                            !self.input.trim().is_empty() && self.backend_ok;
                                        let send = egui::Button::new(
                                            egui::RichText::new("↑")
                                                .font(mono_font(18.0))
                                                .color(DARK_TEXT),
                                        )
                                        .fill(AMBER)
                                        .corner_radius(12)
                                        .min_size(egui::vec2(42.0, 38.0));
                                        if ui
                                            .add_enabled(can_send, send)
                                            .on_hover_text("Send")
                                            .clicked()
                                        {
                                            let text = self.input.clone();
                                            self.send(text);
                                        }
                                    }
                                });
                            });

                        if let Some((provider, model)) = picked_model {
                            self.select_model(&provider, &model);
                        }
                        if let Some(level) = picked_level {
                            self.set_thinking(&level);
                        }

                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(
                                    "Enter to send · Shift+Enter for a new line · Browser opens only for web tasks",
                                )
                                .font(mono_font(9.0))
                                .color(HINT),
                            );
                        });
                });
            });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("settings-modal"))
            .backdrop_color(BACKDROP)
            .frame(
                egui::Frame::new()
                    .fill(PAPER_2)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .corner_radius(18)
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(396.0);
                ui.set_max_width(396.0);

                ui.label(
                    egui::RichText::new("Settings")
                        .font(display_font(26.0))
                        .color(NAVY),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(
                        "Any provider. Cloud keys stay on this machine; local endpoints need no key.",
                    )
                    .font(body_font(12.5))
                    .color(NAVY_SOFT),
                );
                ui.add_space(12.0);

                let mut chosen: Option<ProviderInfo> = None;
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("PROVIDER")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    egui::ComboBox::from_id_salt("settings_provider")
                        .selected_text(
                            self.providers
                                .iter()
                                .find(|p| p.id == self.form.provider)
                                .map(|p| p.name.clone())
                                .unwrap_or_else(|| self.form.provider.clone()),
                        )
                        .width(280.0)
                        .show_ui(ui, |ui| {
                            for p in &self.providers {
                                if ui
                                    .selectable_label(self.form.provider == p.id, &p.name)
                                    .clicked()
                                {
                                    chosen = Some(p.clone());
                                }
                            }
                        });
                });

                if let Some(p) = chosen {
                    if p.id != self.form.provider {
                        self.form.provider = p.id.clone();
                        if !p.default_model.is_empty() {
                            self.form.model = p.default_model.clone();
                        }
                        self.form.base_url = p.default_base_url.clone();
                        self.form.has_key = false;
                        self.form.needs_key = p.needs_key;
                        self.form.local = p.local;
                        self.form.env_key = p.env_key.clone();
                        self.form.api_key.clear();
                        self.form.clear_key = false;
                        self.pending_key_hint = true;
                    }
                }

                if self.form.local {
                    ui.label(
                        egui::RichText::new(
                            "Local model — no API key needed. Make sure the server is running.",
                        )
                        .font(body_font(11.5))
                        .color(GREEN),
                    );
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("MODEL")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    let model_hint = short_model_for(&self.form.provider, &self.form.model);
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.form.model)
                                .hint_text(model_hint)
                                .desired_width(f32::INFINITY)
                                .font(mono_font(10.0)),
                        )
                        .changed()
                    {
                        self.form.clear_key = false;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("BASE URL")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.form.base_url)
                                .hint_text("leave empty for the provider default")
                                .desired_width(f32::INFINITY)
                                .font(mono_font(10.0)),
                        )
                        .changed()
                    {
                        self.form.clear_key = false;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("API KEY")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.form.api_key)
                            .password(true)
                            .hint_text(if self.form.has_key {
                                "key stored — type to replace it"
                            } else {
                                "paste your API key"
                            })
                            .desired_width(f32::INFINITY)
                            .font(mono_font(10.0)),
                    );
                    if resp.changed() {
                        self.form.clear_key = false;
                    }
                    if self.form.has_key {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Clear")
                                        .font(mono_font(9.5))
                                        .color(RED),
                                )
                                .fill(STOP_FILL)
                                .stroke(egui::Stroke::new(1.0, STOP_LINE))
                                .corner_radius(8)
                                .min_size(egui::vec2(64.0, 26.0)),
                            )
                            .clicked()
                        {
                            self.form.clear_key = true;
                            self.form.api_key.clear();
                        }
                    }
                });

                let key_status = if self.form.local {
                    "Local endpoint — no key needed.".to_string()
                } else if self.form.has_key && !self.form.api_key.trim().is_empty() {
                    "Saving will replace the stored key.".to_string()
                } else if self.form.has_key {
                    "A key is stored for this account (never displayed). Leave the box blank to keep it."
                        .to_string()
                } else if self.form.env_key.is_empty() {
                    "No key stored yet. Paste one above.".to_string()
                } else {
                    format!(
                        "No key stored yet. Paste one above, or set {}.",
                        self.form.env_key
                    )
                };
                ui.label(
                    egui::RichText::new(key_status)
                        .font(mono_font(9.0))
                        .color(INK_MUTED),
                );

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("TEMPERATURE")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    // The theme paints widget fills with the panel colour, which
                    // would make both the rail and the handle vanish on a card.
                    ui.scope(|ui| {
                        {
                            let style = ui.style_mut();
                            style.visuals.widgets.inactive.bg_fill = AMBER;
                            style.visuals.widgets.inactive.weak_bg_fill = PAPER_3;
                            style.visuals.widgets.inactive.fg_stroke =
                                egui::Stroke::new(1.5, AMBER);
                            style.visuals.widgets.hovered.bg_fill = AMBER;
                            style.visuals.widgets.hovered.fg_stroke =
                                egui::Stroke::new(1.5, AMBER);
                            style.visuals.widgets.active.bg_fill = AMBER;
                            style.visuals.widgets.active.fg_stroke =
                                egui::Stroke::new(1.5, AMBER);
                            style.visuals.selection.bg_fill = AMBER;
                            style.visuals.handle_shape = egui::style::HandleShape::Circle;
                            style.spacing.slider_width = 170.0;
                        }
                        ui.add(
                            egui::Slider::new(&mut self.form.temperature, 0.0..=2.0)
                                .show_value(true)
                                .fixed_decimals(2),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("MAX TOKENS")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.form.max_tokens)
                            .range(256..=32768)
                            .speed(64.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("THINKING")
                            .font(mono_font(9.0))
                            .color(INK_MUTED),
                    );
                    for level in ["low", "medium", "high"] {
                        if chip_toggle(ui, &level.to_uppercase(), self.form.reasoning == level) {
                            self.form.reasoning = level.to_string();
                        }
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().button_padding = egui::vec2(16.0, 9.0);

                    let save = egui::Button::new(dialog_label("Save", 10.0, DARK_TEXT))
                        .fill(AMBER)
                        .stroke(egui::Stroke::new(1.0, AMBER))
                        .corner_radius(10);
                    if ui.add(save).clicked() {
                        self.save_settings();
                    }

                    ui.add_space(8.0);
                    let reload = egui::Button::new(dialog_label("Reload", 10.0, NAVY))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(1.0, LINE))
                        .corner_radius(10);
                    if ui.add(reload).clicked() {
                        self.load_settings();
                    }

                    ui.add_space(8.0);
                    let keys = egui::Button::new(dialog_label("Provider keys", 10.0, NAVY))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(1.0, LINE))
                        .corner_radius(10);
                    if ui.add(keys).clicked() {
                        let url = match self.form.provider.as_str() {
                            "openai" => "https://platform.openai.com/api-keys",
                            "anthropic" => "https://console.anthropic.com/settings/keys",
                            "gemini" => "https://aistudio.google.com/app/apikey",
                            "groq" => "https://console.groq.com/keys",
                            "openrouter" => "https://openrouter.ai/keys",
                            "nvidia" => "https://build.nvidia.com",
                            _ => "https://ollama.com/download",
                        };
                        let _ = open::that(url);
                    }
                });
            });
        if response.should_close() {
            self.settings_open = false;
        }
    }

    /// Invisible drag strips along the window edges. The window has no native
    /// chrome, so resizing has to be done by hand (like Electron's frame:false).
    fn resize_handles(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        const T: f32 = 8.0;
        let ctx = ui.ctx().clone();
        let add = |ui: &mut egui::Ui, r: egui::Rect, dir: egui::ResizeDirection| {
            if r.width() <= 0.0 || r.height() <= 0.0 {
                return;
            }
            let resp = ui.allocate_rect(r, egui::Sense::click_and_drag());
            if resp.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        };

        let (l, t, rr, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
        use egui::ResizeDirection as D;
        add(ui, egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(l + T, b)), D::West);
        add(ui, egui::Rect::from_min_max(egui::pos2(rr - T, t), egui::pos2(rr, b)), D::East);
        add(ui, egui::Rect::from_min_max(egui::pos2(l + T, t), egui::pos2(rr - T, t + T)), D::North);
        add(ui, egui::Rect::from_min_max(egui::pos2(l + T, b - T), egui::pos2(rr - T, b)), D::South);
        add(ui, egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(l + T, t + T)), D::NorthWest);
        add(ui, egui::Rect::from_min_max(egui::pos2(rr - T, t), egui::pos2(rr, t + T)), D::NorthEast);
        add(ui, egui::Rect::from_min_max(egui::pos2(l, b - T), egui::pos2(l + T, b)), D::SouthWest);
        add(ui, egui::Rect::from_min_max(egui::pos2(rr - T, b - T), egui::pos2(rr, b)), D::SouthEast);
    }

    fn dialog_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let outcome = crate::dialog::show(ctx, &dialog);
        match outcome {
            None => {}
            Some(DialogOutcome::Dismiss) => {
                self.dialog = None;
            }
            Some(DialogOutcome::Action(intent)) => {
                self.dialog = None;
                match intent {
                    Intent::None => {}
                    Intent::CloseBrowser => self.close_browser(),
                    Intent::SafetyContinue => {
                        let text =
                            "USER CONFIRMED: I approve this action. Continue the task.".to_string();
                        self.send(text);
                    }
                    Intent::SafetyCancel => {
                        let text =
                            "CANCELLED: Do not perform that action. Finish safely without it."
                                .to_string();
                        self.send(text);
                    }
                }
            }
        }
    }
}

/* ============================================================
   RUN POLLING (background thread)
   ============================================================ */

fn poll_run(api: Api, tx: Sender<Event>, sid: i64, run_id: i64, cancel: Arc<AtomicBool>) {
    let mut failures: u32 = 0;
    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(90));
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        match api.run(sid, run_id) {
            Ok(run) => {
                failures = 0;
                let status = run
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let terminal = matches!(status.as_str(), "completed" | "failed" | "stopped");
                let _ = tx.send(Event::RunUpdate(run));
                if terminal {
                    break;
                }
            }
            Err(_) => {
                failures += 1;
                if failures % 10 == 0 {
                    let _ = tx.send(Event::Toast("Reconnecting to the agent…".into()));
                }
                if failures >= 30 {
                    let _ = tx.send(Event::SendFailed(
                        "I lost contact with the backend. Is api_server still running?".into(),
                    ));
                    break;
                }
            }
        }
    }
}

/* ============================================================
   STEP DESCRIPTION (mirrors describeStep in js/app.js)
   ============================================================ */

fn describe_step(step: &Value) -> Option<(String, bool)> {
    let action = step.get("action").and_then(|x| x.as_str()).unwrap_or("");
    let target = step.get("target").and_then(|x| x.as_str()).unwrap_or("");
    let value = step.get("value").and_then(|x| x.as_str()).unwrap_or("");
    let error = step.get("error").and_then(|x| x.as_str()).unwrap_or("");
    let failed = step
        .get("status")
        .and_then(|x| x.as_str())
        .map(|s| s == "failed")
        .unwrap_or(false);

    let text = match action {
        "browser_start" => "Opening the browser".to_string(),
        "browser_reuse" => "Using the browser that is already open".to_string(),
        "navigate" => format!("Opening {}", host_of(target)),
        "new_tab" => format!("Opening a new tab at {}", host_of(target)),
        "go_back" => "Going back to the previous page".to_string(),
        "reload" => "Reloading the page".to_string(),
        "switch_frame" => "Moving into an editor frame".to_string(),
        "click" => "Clicking an element on the page".to_string(),
        "scroll" => "Scrolling the page".to_string(),
        "press" => "Pressing a key".to_string(),
        "select" => "Choosing an option".to_string(),
        "drag" => "Drawing with the mouse".to_string(),
        "wait" => "Waiting for the page to settle".to_string(),
        "wait_for" => "Waiting for an element to appear".to_string(),
        "wait_for_navigation" => "Waiting for the page".to_string(),
        "remember" => "Saving a note".to_string(),
        "extract" => "Reading the page".to_string(),
        "type" => format!("Writing {} characters into the page", value.chars().count()),
        "finish" => "Finishing the task".to_string(),
        "chat" => return None,
        "safety_gate" => "Pausing for your confirmation".to_string(),
        "invalid_response" => return Some(("Fixing the agent response".to_string(), true)),
        "stop" => "Stopping the run".to_string(),
        "get_html" | "evaluate" => "Reading the page".to_string(),
        "cookies" | "set_cookie" | "delete_cookies" => "Managing cookies".to_string(),
        "tabs" | "switch_tab" | "close_tab" => "Managing tabs".to_string(),
        "go_back_go_forward" => "Moving through history".to_string(),
        "double_click" => "Double-clicking".to_string(),
        "right_click" => "Opening the context menu".to_string(),
        "hover" => "Hovering over an element".to_string(),
        "screenshot" => "Taking a screenshot".to_string(),
        "scroll_into_view" => "Scrolling to the element".to_string(),
        "upload_file" | "upload" => "Uploading a file".to_string(),
        "clear" | "check" | "uncheck" | "focus" => "Editing a field".to_string(),
        "set_window_size" => "Resizing the window".to_string(),
        "close" => "Closing the browser".to_string(),
        "" => "Working".to_string(),
        other => {
            if failed {
                return Some((
                    if error.is_empty() {
                        "That step failed".to_string()
                    } else {
                        error.to_string()
                    },
                    true,
                ));
            }
            format!("{} step completed", other)
        }
    };

    if failed {
        return Some((
            if error.is_empty() {
                text
            } else {
                error.to_string()
            },
            true,
        ));
    }
    Some((text, false))
}

fn host_of(url: &str) -> String {
    let trimmed = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = trimmed.split('/').next().unwrap_or(trimmed);
    let host = host.split('@').last().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        truncate(url, 44)
    } else {
        host.to_string()
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut)
}

/* ============================================================
   EFRAME
   ============================================================ */

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let window_rect = ui.available_rect_before_wrap();
        let saw_event = self.drain_events();

        let (splash_status, splash_done) = self.splash.tick(self.backend_ok);
        if !splash_done {
            crate::splash::show(ui, &splash_status);
            ui.ctx().request_repaint_after(Duration::from_millis(120));
            return;
        }

        if !self.grown {
            self.grown = true;
            grow_window(ui, frame);
        }

        if saw_event {
            ui.ctx().request_repaint();
        }

        // Keep the browser pill fresh (the reference polls every 5s).
        if self.last_browser_check.elapsed() > Duration::from_secs(5) {
            self.last_browser_check = Instant::now();
            self.refresh_browser_status();
        }

        if self.busy {
            ui.ctx().request_repaint_after(Duration::from_millis(60));
        } else if self.status_until.is_some()
            || self.dialog.is_some()
            || self.splash.active()
        {
            ui.ctx().request_repaint_after(Duration::from_millis(150));
        } else {
            // Idle: one frame a second keeps the timers (browser pill, splash) alive.
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }

        if self.sidebar_open {
            self.sidebar(ui);
        }
        self.header(ui, frame);
        self.composer(ui);
        self.chat_area(ui);

        self.resize_handles(ui, window_rect);

        if std::env::var("AUTOMATA_DEBUG").as_deref() == Ok("1") {
            let monitor = ui.input(|i| i.viewport().monitor_size);
            let inner = ui.input(|i| i.viewport().inner_rect);
            let maximized = ui.input(|i| i.viewport().maximized);
            let text = format!(
                "mon={monitor:?} inner={inner:?} screen={window_rect:?} max={maximized:?} grown={}",
                self.grown
            );
            eprintln!("[dbg] {text}");
            ui.painter().text(
                egui::pos2(12.0, 300.0),
                egui::Align2::LEFT_TOP,
                &text,
                egui::FontId::monospace(12.0),
                egui::Color32::YELLOW,
            );
        }

        let ctx = ui.ctx().clone();
        debug_hook(self);
        self.settings_window(&ctx);
        self.dialog_window(&ctx);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.poll_cancel.store(true, Ordering::SeqCst);
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Dev only: `AUTOMATA_DEBUG=settings|sidebar|safety|close-browser` opens that
/// surface on the first frame after the splash so it can be screenshotted.
fn debug_hook(app: &mut App) {
    let Ok(mode) = std::env::var("AUTOMATA_DEBUG") else {
        return;
    };
    match mode.as_str() {
        "settings" => app.settings_open = true,
        "sidebar" => app.sidebar_open = true,
        "safety" => app.dialog = Some(Dialog::confirm(
            "Confirm action",
            "Clicking this button submits the form with the data currently in it. Continue?",
            "Continue",
            Intent::SafetyContinue,
            "Cancel",
        )),
        "close-browser" => app.confirm_close_browser(),
        "chat" => {
            if app.chat.is_empty() {
                app.chat.push(ChatItem {
                    role: Role::User,
                    text: "Open the docs site and summarise the pricing page for me.".to_string(),
                    note: false,
                    activity: None,
                    at: Instant::now(),
                });
                app.chat.push(ChatItem {
                    role: Role::Assistant,
                    text: "I opened https://example.com/pricing and pulled the three plans. \
The free tier covers 500 requests a month, Pro is $20 a month for 50k requests, \
and Enterprise is priced per seat. The full breakdown is in the pricing page."
                        .to_string(),
                    note: false,
                    activity: None,
                    at: Instant::now(),
                });
                app.chat.push(ChatItem {
                    role: Role::User,
                    text: "Which plan has the best rate limit?".to_string(),
                    note: false,
                    activity: None,
                    at: Instant::now(),
                });
                app.chat.push(ChatItem {
                    role: Role::Assistant,
                    text: "Pro — 50,000 requests a month is the highest published limit \
that is not gated behind a sales conversation."
                        .to_string(),
                    note: false,
                    activity: None,
                    at: Instant::now(),
                });
            }
        }
        "markdown" => {
            if app.chat.is_empty() {
                app.chat.push(ChatItem {
                    role: Role::User,
                    text: "Show me a markdown + math sample.".to_string(),
                    note: false,
                    activity: None,
                    at: Instant::now(),
                });
                app.chat.push(ChatItem {
                    role: Role::Assistant,
                    text: "# Markdown sample\n\nSome **bold**, *italic*, `inline code` and a [link](https://example.com).\n\n- first item\n- second item\n  - nested item\n\n1. one\n2. two\n\n> a quoted line\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n| Name | Value |\n| --- | --- |\n| alpha | 1 |\n| beta | 2 |\n\nInline math: $E = mc^2$ and $\\frac{a}{b}$.\n\n$$\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}$$"
                        .to_string(),
                    note: false,
                    activity: None,
                    at: Instant::now(),
                });
            }
        }
        _ => {}
    }
}

/// Grow the 760x480 splash window into the chat window, exactly like
/// `createWindow()` in `frontend/main.js` does after the 10 s splash.
fn grow_window(ui: &egui::Ui, frame: &eframe::Frame) {
    let _ = &frame;
    let ctx = ui.ctx().clone();
    let fallback = ui
        .input(|i| i.viewport().monitor_size)
        .unwrap_or_else(|| egui::vec2(1920.0, 1080.0));

    #[cfg(windows)]
    let area = work_area_logical(frame, ctx.pixels_per_point()).unwrap_or_else(|| {
        egui::Rect::from_min_size(egui::Pos2::ZERO, fallback)
    });
    #[cfg(not(windows))]
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, fallback);

    let w = (area.width() * 0.70)
        .clamp(760.0, 1040.0)
        .min((area.width() - 96.0).max(760.0));
    let h = (area.height() * 0.80)
        .clamp(540.0, 720.0)
        .min((area.height() - 96.0).max(540.0));

    eprintln!("[grow] area={area:?} -> {w}x{h}");

    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
        760.0, 540.0,
    )));
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
        area.min.x + ((area.width() - w) * 0.5).max(0.0),
        area.min.y + ((area.height() - h) * 0.5).max(0.0),
    )));
}

#[cfg(windows)]
fn hwnd_of(frame: &eframe::Frame) -> Option<*mut core::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = frame.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win) => Some(win.hwnd.get() as *mut core::ffi::c_void),
        _ => None,
    }
}

/// The monitor work area (screen minus taskbar / docked toolbars) in egui
/// points. This is what keeps a frameless maximized window from covering the
/// Windows taskbar.
#[cfg(windows)]
fn work_area_logical(frame: &eframe::Frame, pixels_per_point: f32) -> Option<egui::Rect> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let hwnd = hwnd_of(frame)?;
    let scale = if pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };

    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let work: RECT = info.rcWork;
        Some(egui::Rect::from_min_max(
            egui::pos2(work.left as f32 / scale, work.top as f32 / scale),
            egui::pos2(work.right as f32 / scale, work.bottom as f32 / scale),
        ))
    }
}

#[cfg(windows)]
fn toggle_maximize_native(
    frame: &eframe::Frame,
    maximized: &mut bool,
    restore_rect: &mut Option<(i32, i32, i32, i32)>,
) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, IsZoomed, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER,
        SW_RESTORE,
    };

    let Some(hwnd) = hwnd_of(frame) else {
        return;
    };

    unsafe {
        if *maximized {
            if IsZoomed(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            if let Some((x, y, w, h)) = restore_rect.take() {
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    x,
                    y,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            *maximized = false;
            return;
        }

        if IsZoomed(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }

        let mut current: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut current) != 0 {
            *restore_rect = Some((
                current.left,
                current.top,
                current.right - current.left,
                current.bottom - current.top,
            ));
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return;
        }

        let width = info.rcWork.right - info.rcWork.left;
        let height = info.rcWork.bottom - info.rcWork.top;
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            info.rcWork.left,
            info.rcWork.top,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        *maximized = true;
    }
}

fn spawn_startup(tx: Sender<Event>, base: String) {
    std::thread::spawn(move || {
        let st = crate::launch::ensure(&base);
        let _ = tx.send(Event::Startup {
            backend_ok: st.backend_ok,
            browser_ok: st.browser_ok,
            note: st.note.clone(),
        });
        let _ = tx.send(Event::Children(st.backend, st.browser));

        if st.backend_ok {
            let api = Api::new(base);
            if let Ok(list) = api.sessions() {
                let _ = tx.send(Event::Sessions(list));
            } else {
                let _ = tx.send(Event::SessionsOffline);
            }
            if let Ok(v) = api.settings() {
                let _ = tx.send(Event::Settings(v));
            }
            if let Ok(p) = api.providers() {
                let _ = tx.send(Event::Providers(p));
            }
            let _ = tx.send(Event::Browser(api.browser_running()));
        }
    });
}

pub fn run() -> eframe::Result<()> {
    let base = std::env::var("AUTOMATA_API").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let api = Api::new(&base);

    let (tx, rx) = std::sync::mpsc::channel::<Event>();
    spawn_startup(tx.clone(), base);

    // The reference window is frameless (`frame: false` in main.js): the UI
    // draws its own title bar. It opens at the splash size (760x480) and
    // grows into the chat window once the splash is done.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([760.0, 480.0])
            .with_min_inner_size([760.0, 480.0])
            .with_title("Automata"),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Automata",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, api, tx, rx)))),
    )
}

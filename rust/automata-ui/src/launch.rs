use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::api::Api;

pub struct Startup {
    pub backend: Option<Child>,
    pub browser: Option<Child>,
    pub backend_ok: bool,
    pub browser_ok: bool,
    pub note: String,
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

fn ancestors(start: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut cur = Some(start);
    for _ in 0..6 {
        match cur {
            Some(p) => {
                out.push(p.to_path_buf());
                cur = p.parent();
            }
            None => break,
        }
    }
    out
}

fn candidate_paths(name: &str) -> Vec<PathBuf> {
    let exe = exe_name(name);
    let mut bases: Vec<PathBuf> = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        bases.extend(ancestors(&cwd));
    }
    if let Some(dir) = exe_dir() {
        bases.extend(ancestors(&dir));
    }

    let mut out = Vec::new();
    for base in bases {
        out.push(base.join(&exe));
        out.push(base.join("target").join("debug").join(&exe));
        out.push(base.join("target").join("release").join(&exe));
        out.push(base.join("backend").join(&exe));
        out.push(base.join("rust").join("target").join("debug").join(&exe));
        out.push(base.join("rust").join("target").join("release").join(&exe));
    }
    out
}

fn find_env_or(name: &str, env_key: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var(env_key) {
        if !p.trim().is_empty() && Path::new(&p).exists() {
            return Some(PathBuf::from(p));
        }
    }
    for path in candidate_paths(name) {
        if path.is_file() {
            return Some(path);
        }
    }
    // Last resort: anything on PATH.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(exe_name(name));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn log_file(name: &str) -> Stdio {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("automata");
    let _ = std::fs::create_dir_all(&dir);
    match OpenOptions::new().create(true).append(true).open(dir.join(name)) {
        Ok(f) => Stdio::from(f),
        Err(_) => Stdio::null(),
    }
}

fn wait_for<F: Fn() -> bool>(check: F, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    check()
}

fn spawn(path: &Path, log: &str, cwd: Option<&Path>, envs: &[(&str, &str)]) -> Option<Child> {
    let mut cmd = Command::new(path);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null())
        .stdout(log_file(log))
        .stderr(log_file(log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn().ok()
}

pub fn ensure(base: &str) -> Startup {
    let api = Api::new(base);
    let mut notes: Vec<String> = Vec::new();

    // ---- fast browser engine (CDP over the WebDriver port) ----
    let browser_alive = ureq::get("http://127.0.0.1:9515/status")
        .timeout(Duration::from_millis(600))
        .call()
        .is_ok();

    let mut browser_child = None;
    let mut browser_ok = browser_alive;
    if !browser_alive {
        if let Some(path) = find_env_or("automata-browser", "AUTOMATA_BROWSER_BIN") {
            browser_child = spawn(&path, "browser-engine.log", None, &[]);
            browser_ok = wait_for(
                || {
                    ureq::get("http://127.0.0.1:9515/status")
                        .timeout(Duration::from_millis(400))
                        .call()
                        .is_ok()
                },
                Duration::from_secs(4),
            );
            if !browser_ok {
                notes.push("browser engine did not answer on port 9515".to_string());
            }
        } else {
            notes.push(
                "automata-browser not found; build it with `cargo build -p automata-browser` \
                 (the engine falls back to ChromeDriver if one is running)"
                    .to_string(),
            );
        }
    }

    // ---- C++ backend ----
    let mut backend_child = None;
    let mut backend_ok = api.health();
    if !backend_ok {
        if let Some(path) = find_env_or("api_server", "AUTOMATA_BACKEND") {
            let cwd = path.parent().map(|p| p.to_path_buf());
            backend_child = spawn(
                &path,
                "backend.log",
                cwd.as_deref(),
                &[
                    ("CHROMEDRIVER_URL", "http://127.0.0.1:9515"),
                    ("AUTOMATA_FRONTEND_DIR", "../frontend"),
                ],
            );
            backend_ok = wait_for(|| api.health(), Duration::from_secs(10));
            if !backend_ok {
                notes.push("backend did not answer on port 8080".to_string());
            }
        } else {
            notes.push(
                "api_server backend not found; build it first or set AUTOMATA_BACKEND"
                    .to_string(),
            );
        }
    }

    Startup {
        backend: backend_child,
        browser: browser_child,
        backend_ok,
        browser_ok,
        note: notes.join(" | "),
    }
}

//! Model catalogue + the "major name" short label used by the model toggle.

/// (provider, &[(full model id, short label shown in the toggle)])
pub const MODEL_CATALOG: &[(&str, &[(&str, &str)])] = &[
    (
        "nvidia",
        &[
            (
                "nvidia/nemotron-3.5-super-vl-120b-a12b",
                "nemotron-3.5-super",
            ),
            (
                "nvidia/llama-3.1-nemotron-70b-instruct",
                "nemotron-70b",
            ),
            ("meta/llama-3.3-70b-instruct", "llama-3.3-70b"),
        ],
    ),
    (
        "openai",
        &[
            ("gpt-4o-mini", "gpt-4o-mini"),
            ("gpt-4o", "gpt-4o"),
            ("gpt-4.1-mini", "gpt-4.1-mini"),
            ("o4-mini", "o4-mini"),
        ],
    ),
    (
        "anthropic",
        &[
            ("claude-3-5-sonnet-latest", "claude-3.5-sonnet"),
            ("claude-3-5-haiku-latest", "claude-3.5-haiku"),
            ("claude-3-7-sonnet-latest", "claude-3.7-sonnet"),
        ],
    ),
    (
        "gemini",
        &[
            ("gemini-1.5-flash", "gemini-1.5-flash"),
            ("gemini-1.5-pro", "gemini-1.5-pro"),
            ("gemini-2.0-flash", "gemini-2.0-flash"),
        ],
    ),
    (
        "groq",
        &[
            ("llama-3.3-70b-versatile", "llama-3.3-70b"),
            ("llama-3.1-8b-instant", "llama-3.1-8b"),
            ("mixtral-8x7b-32768", "mixtral-8x7b"),
        ],
    ),
    (
        "openrouter",
        &[
            ("openai/gpt-4o-mini", "gpt-4o-mini"),
            ("anthropic/claude-3.5-sonnet", "claude-3.5-sonnet"),
            ("google/gemini-flash-1.5", "gemini-1.5-flash"),
        ],
    ),
    (
        "ollama",
        &[
            ("llama3.1", "llama3.1"),
            ("qwen2.5", "qwen2.5"),
            ("mistral", "mistral"),
            ("deepseek-r1", "deepseek-r1"),
        ],
    ),
    ("lmstudio", &[("local-model", "local-model")]),
];

pub fn provider_name(id: &str) -> &'static str {
    match id {
        "nvidia" => "NVIDIA NIM",
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "gemini" => "Gemini",
        "groq" => "Groq",
        "openrouter" => "OpenRouter",
        "ollama" => "Ollama",
        "lmstudio" => "LM Studio",
        _ => "Custom",
    }
}

fn is_size_token(token: &str) -> bool {
    let t = token.to_ascii_lowercase();
    // 120b, a12b, 8x7b, 70b, 32768, 2024 …
    if t.chars().all(|c| c.is_ascii_digit()) && t.len() >= 3 {
        return true;
    }
    if t.ends_with('b') && t.len() > 1 {
        let head = &t[..t.len() - 1];
        if head.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
        // 8x7b / a12b
        if head.contains('x') || head.starts_with('a') {
            let cleaned: String = head.chars().filter(|c| c.is_ascii_digit() || *c == 'x').collect();
            if !cleaned.is_empty() && cleaned.chars().all(|c| c.is_ascii_digit() || c == 'x') {
                return true;
            }
        }
    }
    matches!(t.as_str(), "latest" | "versatile" | "instant" | "preview" | "experimental")
}

/// The "major name" of a model: vendor prefix, size/context tokens and
/// marketing suffixes are dropped so the toggle stays short and readable.
pub fn short_model(full: &str) -> String {
    let name = full.rsplit('/').next().unwrap_or(full).trim();
    if name.is_empty() {
        return full.to_string();
    }

    let tokens: Vec<&str> = name.split('-').filter(|t| !t.is_empty()).collect();
    let mut kept: Vec<&str> = Vec::new();
    for token in tokens {
        if kept.len() >= 4 {
            break;
        }
        if is_size_token(token) && kept.len() >= 2 {
            break;
        }
        kept.push(token);
    }
    if kept.is_empty() {
        return name.to_string();
    }
    kept.join("-")
}

/// Short label for a model, using the curated list when the model is known.
pub fn short_model_for(provider: &str, full: &str) -> String {
    if let Some((_, models)) = MODEL_CATALOG.iter().find(|(id, _)| *id == provider) {
        if let Some((_, short)) = models.iter().find(|(m, _)| *m == full) {
            return (*short).to_string();
        }
    }
    short_model(full)
}

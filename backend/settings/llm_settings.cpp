#include "llm_settings.h"

#include <cctype>
#include <cstdlib>
#include <fstream>
#include <sstream>

#ifdef _WIN32
#include <direct.h>
#include <windows.h>
#else
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>
#endif

using namespace std;

namespace
{

string trim_copy(string text)
{
    size_t start = text.find_first_not_of(" \t\r\n");
    if (start == string::npos)
    {
        return "";
    }
    size_t end = text.find_last_not_of(" \t\r\n");
    return text.substr(start, end - start + 1);
}

string lower_copy(string text)
{
    for (char &c : text)
    {
        c = static_cast<char>(tolower(static_cast<unsigned char>(c)));
    }
    return text;
}

string env_value(const char *name)
{
    const char *value = getenv(name);
    return value == nullptr ? "" : string(value);
}

void ensure_dir(const string &path)
{
    if (path.empty())
    {
        return;
    }
#ifdef _WIN32
    _mkdir(path.c_str());
#else
    mkdir(path.c_str(), 0700);
#endif
}

} // namespace

const vector<ProviderMeta> &provider_catalog()
{
    static const vector<ProviderMeta> providers = {
        {"nvidia",
         "NVIDIA NIM",
         "nvidia/nemotron-3.5-super-vl-120b-a12b",
         "https://integrate.api.nvidia.com/v1",
         "NVIDIA_API_KEY",
         true,
         false},
        {"openai",
         "OpenAI",
         "gpt-4o-mini",
         "https://api.openai.com/v1",
         "OPENAI_API_KEY",
         true,
         false},
        {"anthropic",
         "Anthropic Claude",
         "claude-3-5-sonnet-latest",
         "https://api.anthropic.com/v1",
         "ANTHROPIC_API_KEY",
         true,
         false},
        {"gemini",
         "Google Gemini",
         "gemini-1.5-flash",
         "https://generativelanguage.googleapis.com/v1beta",
         "GEMINI_API_KEY",
         true,
         false},
        {"groq",
         "Groq",
         "llama-3.3-70b-versatile",
         "https://api.groq.com/openai/v1",
         "GROQ_API_KEY",
         true,
         false},
        {"openrouter",
         "OpenRouter",
         "openai/gpt-4o-mini",
         "https://openrouter.ai/api/v1",
         "OPENROUTER_API_KEY",
         true,
         false},
        {"ollama",
         "Ollama (local)",
         "llama3.1",
         "http://127.0.0.1:11434/v1",
         "",
         false,
         true},
        {"lmstudio",
         "LM Studio (local)",
         "local-model",
         "http://127.0.0.1:1234/v1",
         "",
         false,
         true},
        {"custom",
         "Custom / OpenAI-compatible",
         "",
         "",
         "CUSTOM_API_KEY",
         false,
         false},
    };
    return providers;
}

const ProviderMeta *find_provider(const string &id)
{
    string wanted = lower_copy(trim_copy(id));
    for (const ProviderMeta &provider : provider_catalog())
    {
        if (provider.id == wanted)
        {
            return &provider;
        }
    }
    return nullptr;
}

string LLMSettings::effective_model() const
{
    if (!trim_copy(model).empty())
    {
        return trim_copy(model);
    }
    const ProviderMeta *meta = find_provider(provider);
    return meta == nullptr ? "" : meta->default_model;
}

string LLMSettings::effective_base_url() const
{
    string configured = trim_copy(base_url);
    if (!configured.empty())
    {
        return configured;
    }
    const ProviderMeta *meta = find_provider(provider);
    return meta == nullptr ? "" : meta->default_base_url;
}

string LLMSettings::env_key_name() const
{
    const ProviderMeta *meta = find_provider(provider);
    return meta == nullptr ? "" : meta->env_key;
}

bool LLMSettings::is_local() const
{
    const ProviderMeta *meta = find_provider(provider);
    return meta != nullptr && meta->local;
}

bool LLMSettings::needs_key() const
{
    const ProviderMeta *meta = find_provider(provider);
    return meta == nullptr ? true : meta->needs_key;
}

namespace
{

string resolve_key(const LLMSettings &settings)
{
    string key = trim_copy(settings.api_key);
    if (!key.empty())
    {
        return key;
    }
    string env_name = settings.env_key_name();
    if (!env_name.empty())
    {
        return env_value(env_name.c_str());
    }
    return "";
}

} // namespace

SettingsStore::SettingsStore()
{
    load();
}

string SettingsStore::config_path()
{
#ifdef _WIN32
    string base = env_value("APPDATA");
    if (base.empty())
    {
        base = env_value("LOCALAPPDATA");
    }
    if (base.empty())
    {
        base = ".";
    }
    return base + "\\Automata\\config.json";
#elif __APPLE__
    string home = env_value("HOME");
    if (home.empty())
    {
        home = ".";
    }
    return home + "/Library/Application Support/Automata/config.json";
#else
    string base = env_value("XDG_CONFIG_HOME");
    if (base.empty())
    {
        string home = env_value("HOME");
        base = (home.empty() ? string(".") : home) + "/.config";
    }
    return base + "/automata/config.json";
#endif
}

void SettingsStore::load()
{
    lock_guard<mutex> lock(mutex_);
    const string path = config_path();

    ifstream input(path);
    if (!input)
    {
        return;
    }

    try
    {
        nlohmann::json data;
        input >> data;

        if (data.contains("provider") && data["provider"].is_string())
        {
            settings_.provider = lower_copy(trim_copy(data["provider"].get<string>()));
        }
        if (data.contains("model") && data["model"].is_string())
        {
            settings_.model = data["model"].get<string>();
        }
        if (data.contains("base_url") && data["base_url"].is_string())
        {
            settings_.base_url = data["base_url"].get<string>();
        }
        if (data.contains("api_key") && data["api_key"].is_string())
        {
            settings_.api_key = data["api_key"].get<string>();
        }
        if (data.contains("temperature") && data["temperature"].is_number())
        {
            settings_.temperature = data["temperature"].get<double>();
        }
        if (data.contains("max_tokens") && data["max_tokens"].is_number_integer())
        {
            settings_.max_tokens = data["max_tokens"].get<int>();
        }
        if (data.contains("reasoning_effort") && data["reasoning_effort"].is_string())
        {
            settings_.reasoning_effort = lower_copy(data["reasoning_effort"].get<string>());
        }

        if (find_provider(settings_.provider) == nullptr)
        {
            settings_.provider = "nvidia";
        }
    }
    catch (...)
    {
        // A broken config must never stop the app from starting.
    }
}

void SettingsStore::save() const
{
    const string path = config_path();
    const size_t slash = path.find_last_of("/\\");
    if (slash != string::npos)
    {
        ensure_dir(path.substr(0, slash));
    }

    nlohmann::json data;
    data["provider"] = settings_.provider;
    data["model"] = settings_.model;
    data["base_url"] = settings_.base_url;
    data["api_key"] = settings_.api_key;
    data["temperature"] = settings_.temperature;
    data["max_tokens"] = settings_.max_tokens;
    data["reasoning_effort"] = settings_.reasoning_effort;

    const string tmp = path + ".tmp";
    {
        ofstream output(tmp, ios::trunc);
        if (!output)
        {
            return;
        }
        output << data.dump(2);
    }

#ifdef _WIN32
    MoveFileExA(tmp.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING);
#else
    chmod(tmp.c_str(), 0600);
    rename(tmp.c_str(), path.c_str());
#endif
}

LLMSettings SettingsStore::get() const
{
    lock_guard<mutex> lock(mutex_);
    return settings_;
}

bool SettingsStore::update_from_json(const nlohmann::json &input, string &error)
{
    error.clear();

    if (!input.is_object())
    {
        error = "Settings must be a JSON object";
        return false;
    }

    lock_guard<mutex> lock(mutex_);
    LLMSettings next = settings_;
    bool provider_changed = false;

    if (input.contains("provider"))
    {
        if (!input["provider"].is_string())
        {
            error = "provider must be a string";
            return false;
        }
        string provider = lower_copy(trim_copy(input["provider"].get<string>()));
        if (find_provider(provider) == nullptr)
        {
            error = "Unknown provider: " + provider;
            return false;
        }
        provider_changed = provider != next.provider;
        next.provider = provider;
    }

    // Switching provider must not leak the previous provider's key/model/URL.
    if (provider_changed)
    {
        if (!input.contains("model"))
            next.model.clear();
        if (!input.contains("base_url"))
            next.base_url.clear();
        if (!input.contains("api_key"))
            next.api_key.clear();
    }

    if (input.contains("model"))
    {
        if (!input["model"].is_string())
        {
            error = "model must be a string";
            return false;
        }
        next.model = trim_copy(input["model"].get<string>());
    }

    if (input.contains("base_url"))
    {
        if (!input["base_url"].is_string())
        {
            error = "base_url must be a string";
            return false;
        }
        next.base_url = trim_copy(input["base_url"].get<string>());
    }

    if (input.contains("api_key"))
    {
        if (!input["api_key"].is_string())
        {
            error = "api_key must be a string";
            return false;
        }
        next.api_key = input["api_key"].get<string>();
    }

    if (input.contains("temperature"))
    {
        if (!input["temperature"].is_number())
        {
            error = "temperature must be a number";
            return false;
        }
        next.temperature = input["temperature"].get<double>();
        if (next.temperature < 0.0)
            next.temperature = 0.0;
        if (next.temperature > 2.0)
            next.temperature = 2.0;
    }

    if (input.contains("max_tokens"))
    {
        if (!input["max_tokens"].is_number_integer())
        {
            error = "max_tokens must be an integer";
            return false;
        }
        next.max_tokens = input["max_tokens"].get<int>();
        if (next.max_tokens < 256)
            next.max_tokens = 256;
        if (next.max_tokens > 32768)
            next.max_tokens = 32768;
    }

    if (input.contains("reasoning_effort"))
    {
        if (!input["reasoning_effort"].is_string())
        {
            error = "reasoning_effort must be a string";
            return false;
        }
        string effort = lower_copy(trim_copy(input["reasoning_effort"].get<string>()));
        if (effort != "low" && effort != "medium" && effort != "high")
        {
            error = "reasoning_effort must be low, medium or high";
            return false;
        }
        next.reasoning_effort = effort;
    }

    settings_ = next;
    save();
    return true;
}

nlohmann::json SettingsStore::public_json() const
{
    lock_guard<mutex> lock(mutex_);

    nlohmann::json data;
    data["provider"] = settings_.provider;
    data["model"] = settings_.effective_model();
    data["base_url"] = settings_.effective_base_url();
    data["temperature"] = settings_.temperature;
    data["max_tokens"] = settings_.max_tokens;
    data["reasoning_effort"] = settings_.reasoning_effort;
    data["has_api_key"] = !trim_copy(settings_.api_key).empty();
    data["needs_key"] = settings_.needs_key();
    data["local"] = settings_.is_local();
    data["env_key"] = settings_.env_key_name();
    return data;
}

nlohmann::json SettingsStore::providers_json()
{
    nlohmann::json list = nlohmann::json::array();
    for (const ProviderMeta &provider : provider_catalog())
    {
        nlohmann::json item;
        item["id"] = provider.id;
        item["name"] = provider.name;
        item["default_model"] = provider.default_model;
        item["default_base_url"] = provider.default_base_url;
        item["needs_key"] = provider.needs_key;
        item["local"] = provider.local;
        item["env_key"] = provider.env_key;
        list.push_back(item);
    }

    nlohmann::json result;
    result["providers"] = list;
    return result;
}

string current_api_key(const LLMSettings &settings)
{
    return resolve_key(settings);
}

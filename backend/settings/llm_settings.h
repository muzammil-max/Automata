#pragma once

#include <mutex>
#include <string>
#include <vector>

#include "../libraries/json.hpp"

// One configuration for every supported AI provider. A single config is
// enough because the agent runs one task at a time.
struct LLMSettings
{
    std::string provider = "nvidia";
    std::string model;       // empty -> provider default
    std::string base_url;    // empty -> provider default
    std::string api_key;     // BYOK; empty -> environment variable
    double temperature = 0.7;
    int max_tokens = 4096;
    std::string reasoning_effort = "high";

    std::string effective_model() const;
    std::string effective_base_url() const;
    std::string env_key_name() const;
    bool is_local() const;
    bool needs_key() const;
};

struct ProviderMeta
{
    std::string id;
    std::string name;
    std::string default_model;
    std::string default_base_url;
    std::string env_key;
    bool needs_key = true;
    bool local = false;
};

const std::vector<ProviderMeta> &provider_catalog();
const ProviderMeta *find_provider(const std::string &id);

// BYOK key if set, otherwise the provider's environment variable.
std::string current_api_key(const LLMSettings &settings);

// Loads and saves settings in the user's config directory. The API key is
// never logged and the file is restricted to the current user on POSIX.
class SettingsStore
{
public:
    SettingsStore();

    LLMSettings get() const;
    bool update_from_json(const nlohmann::json &input, std::string &error);

    nlohmann::json public_json() const;
    static nlohmann::json providers_json();
    static std::string config_path();

private:
    void load();
    void save() const;

    mutable std::mutex mutex_;
    LLMSettings settings_;
};

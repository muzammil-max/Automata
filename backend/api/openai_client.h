#pragma once

#include "llm_client.h"
#include "../settings/llm_settings.h"

#include <string>

// Works with any OpenAI-compatible API: OpenAI, NVIDIA NIM, Groq,
// OpenRouter, Ollama, LM Studio, vLLM and custom local servers.
class OpenAIClient : public LLMClient
{
public:
    explicit OpenAIClient(const LLMSettings &settings);

    bool generate_response(
        const std::string &prompt,
        const std::string &instructions,
        std::string &response,
        std::string &error) override;

    bool generate_response(
        const std::string &prompt,
        const std::string &instructions,
        const std::vector<RunData> &history,
        std::string &response,
        std::string &error) override;

    bool generate_response_stream(
        const std::string &prompt,
        const std::string &instructions,
        const std::vector<RunData> &history,
        std::string &response,
        std::string &error,
        const std::function<void(const std::string &)> &on_delta) override;

private:
    LLMSettings settings_;
    std::string api_key_;
    std::string base_url_;
    std::string model_;
};

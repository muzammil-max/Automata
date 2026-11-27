#pragma once

#include "llm_client.h"
#include "../settings/llm_settings.h"

#include <string>

class AnthropicClient : public LLMClient
{
public:
    explicit AnthropicClient(const LLMSettings &settings);

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

private:
    LLMSettings settings_;
    std::string api_key_;
    std::string base_url_;
    std::string model_;
};

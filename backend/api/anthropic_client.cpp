#include "anthropic_client.h"

#include "http_client.h"
#include "llm_util.h"
#include "../libraries/json.hpp"

using namespace std;

AnthropicClient::AnthropicClient(const LLMSettings &settings)
    : settings_(settings),
      api_key_(current_api_key(settings)),
      base_url_(settings.effective_base_url()),
      model_(settings.effective_model())
{
}

bool AnthropicClient::generate_response(
    const string &prompt,
    const string &instructions,
    string &response,
    string &error)
{
    vector<RunData> empty;
    return generate_response(prompt, instructions, empty, response, error);
}

bool AnthropicClient::generate_response(
    const string &prompt,
    const string &instructions,
    const vector<RunData> &history,
    string &response,
    string &error)
{
    response.clear();
    error.clear();

    if (llm_trim(prompt).empty())
    {
        error = "Prompt cannot be empty!";
        return false;
    }
    if (llm_trim(api_key_).empty())
    {
        error = "No Anthropic API key is configured. Add one in Settings (BYOK) "
                "or set ANTHROPIC_API_KEY.";
        return false;
    }

    string base = llm_trim(base_url_);
    while (!base.empty() && base.back() == '/')
    {
        base.pop_back();
    }
    if (base.empty())
    {
        base = "https://api.anthropic.com/v1";
    }
    const string url = base + "/messages";

    nlohmann::json messages = nlohmann::json::array();
    size_t used = 0;
    for (const RunData &run : history)
    {
        if (used >= 6)
        {
            break;
        }
        if ((run.status != "completed" && run.status != "stopped") ||
            run.message.empty())
        {
            continue;
        }
        used++;
        messages.push_back(
            {{"role", "user"}, {"content", llm_truncate(run.message, 600)}});
        if (!run.output.empty())
        {
            messages.push_back(
                {{"role", "assistant"}, {"content", llm_truncate(run.output, 1600)}});
        }
    }
    messages.push_back({{"role", "user"}, {"content", prompt}});

    nlohmann::json request;
    request["model"] = model_.empty() ? "claude-3-5-sonnet-latest" : model_;
    request["max_tokens"] = settings_.max_tokens;
    request["temperature"] = settings_.temperature;
    request["messages"] = messages;
    if (!llm_trim(instructions).empty())
    {
        request["system"] = instructions;
    }

    const string extra_headers =
        "x-api-key: " + api_key_ + "\r\n" +
        "anthropic-version: 2023-06-01\r\n";

    HttpResponse http = http_request("POST", url, request.dump(),
                                     "application/json", "", 10, 240,
                                     extra_headers);

    if (!http.ok)
    {
        error = "Network error: " + http.error;
        return false;
    }

    if (http.status < 200 || http.status >= 300)
    {
        string detail = llm_truncate(http.body, 600);
        try
        {
            nlohmann::json parsed = nlohmann::json::parse(http.body);
            if (parsed.contains("error") && parsed["error"].is_object() &&
                parsed["error"].contains("message"))
            {
                detail = parsed["error"]["message"].get<string>();
            }
        }
        catch (...)
        {
        }
        error = "Anthropic error (" + to_string(http.status) + "): " + detail;
        return false;
    }

    try
    {
        nlohmann::json parsed = nlohmann::json::parse(http.body);
        if (parsed.contains("content") && parsed["content"].is_array())
        {
            for (const auto &block : parsed["content"])
            {
                if (block.is_object() && block.contains("text") &&
                    block["text"].is_string())
                {
                    response += block["text"].get<string>();
                }
            }
        }
    }
    catch (const nlohmann::json::exception &e)
    {
        error = string("Could not parse Anthropic response: ") + e.what();
        return false;
    }

    if (llm_trim(response).empty())
    {
        error = "Anthropic returned an empty response.";
        return false;
    }
    return true;
}

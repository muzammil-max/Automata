#include "openai_client.h"

#include "http_client.h"
#include "llm_util.h"
#include "../libraries/json.hpp"

#include <iostream>

using namespace std;

namespace
{

string normalize_url(string base)
{
    base = llm_trim(base);
    while (!base.empty() && base.back() == '/')
    {
        base.pop_back();
    }
    if (base.empty())
    {
        return "";
    }
    if (base.size() >= 17 &&
        base.compare(base.size() - 17, 17, "/chat/completions") == 0)
    {
        return base;
    }
    return base + "/chat/completions";
}

string message_content(const nlohmann::json &message)
{
    if (!message.contains("content") || message["content"].is_null())
    {
        if (message.contains("reasoning_content") &&
            message["reasoning_content"].is_string())
        {
            return message["reasoning_content"].get<string>();
        }
        return "";
    }

    const nlohmann::json &content = message["content"];
    if (content.is_string())
    {
        return content.get<string>();
    }
    if (content.is_array())
    {
        string out;
        for (const auto &part : content)
        {
            if (part.is_string())
            {
                out += part.get<string>();
            }
            else if (part.is_object() && part.contains("text") &&
                     part["text"].is_string())
            {
                out += part["text"].get<string>();
            }
        }
        return out;
    }
    return "";
}

nlohmann::json build_messages(const string &prompt,
                              const string &instructions,
                              const vector<RunData> &history)
{
    nlohmann::json messages = nlohmann::json::array();
    if (!llm_trim(instructions).empty())
    {
        messages.push_back({{"role", "system"}, {"content", instructions}});
    }

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
    return messages;
}

nlohmann::json build_request(const LLMSettings &settings,
                             const string &model,
                             const nlohmann::json &messages,
                             bool stream)
{
    nlohmann::json request;
    request["model"] = model;
    request["messages"] = messages;
    request["temperature"] = settings.temperature;
    request["max_tokens"] = settings.max_tokens;
    request["stream"] = stream;

    if (settings.provider == "nvidia")
    {
        request["reasoning_effort"] = settings.reasoning_effort;
        request["seed"] = 42;
    }
    return request;
}

string http_error_detail(const HttpResponse &http)
{
    string detail = llm_truncate(http.body, 600);
    try
    {
        nlohmann::json parsed = nlohmann::json::parse(http.body);
        if (parsed.contains("error"))
        {
            const auto &err = parsed["error"];
            if (err.is_string())
            {
                detail = err.get<string>();
            }
            else if (err.is_object() && err.contains("message") &&
                     err["message"].is_string())
            {
                detail = err["message"].get<string>();
            }
        }
    }
    catch (...)
    {
    }
    return detail;
}

enum class SseResult
{
    NeedMore,
    Ignored,
    Delta,
    Done,
    Error
};

// Reads one complete SSE line from `buffer`. Returns Delta when the line
// carried text, Done for the [DONE] marker, Error for a provider error event,
// and Ignored for comments/blank lines.
SseResult next_sse_delta(string &buffer, string &delta, string &error)
{
    size_t newline = buffer.find('\n');
    if (newline == string::npos)
    {
        return SseResult::NeedMore;
    }

    string line = buffer.substr(0, newline);
    buffer.erase(0, newline + 1);
    if (!line.empty() && line.back() == '\r')
    {
        line.pop_back();
    }
    if (line.rfind("data:", 0) != 0)
    {
        return SseResult::Ignored;
    }

    string data = llm_trim(line.substr(5));
    if (data.empty())
    {
        return SseResult::Ignored;
    }
    if (data == "[DONE]")
    {
        return SseResult::Done;
    }

    try
    {
        nlohmann::json parsed = nlohmann::json::parse(data);
        if (parsed.contains("error"))
        {
            const auto &err = parsed["error"];
            if (err.is_string())
            {
                error = err.get<string>();
            }
            else if (err.is_object() && err.contains("message") &&
                     err["message"].is_string())
            {
                error = err["message"].get<string>();
            }
            else
            {
                error = data;
            }
            return SseResult::Error;
        }
        if (!parsed.contains("choices") || !parsed["choices"].is_array() ||
            parsed["choices"].empty())
        {
            return SseResult::Ignored;
        }
        const auto &choice = parsed["choices"][0];
        string piece;
        if (choice.contains("delta"))
        {
            piece = message_content(choice["delta"]);
        }
        if (piece.empty() && choice.contains("message"))
        {
            piece = message_content(choice["message"]);
        }
        if (piece.empty() && choice.contains("text") &&
            choice["text"].is_string())
        {
            piece = choice["text"].get<string>();
        }
        if (!piece.empty())
        {
            delta = piece;
            return SseResult::Delta;
        }
    }
    catch (...)
    {
    }
    return SseResult::Ignored;
}

} // namespace

OpenAIClient::OpenAIClient(const LLMSettings &settings)
    : settings_(settings),
      api_key_(current_api_key(settings)),
      base_url_(settings.effective_base_url()),
      model_(settings.effective_model())
{
}

bool OpenAIClient::generate_response(
    const string &prompt,
    const string &instructions,
    string &response,
    string &error)
{
    vector<RunData> empty;
    return generate_response(prompt, instructions, empty, response, error);
}

bool OpenAIClient::generate_response(
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
    if (base_url_.empty())
    {
        error = "No base URL is configured for this provider.";
        return false;
    }
    if (settings_.needs_key() && llm_trim(api_key_).empty())
    {
        error = "No API key is configured. Add one in Settings (BYOK) or set " +
                settings_.env_key_name() + ".";
        return false;
    }
    if (model_.empty())
    {
        error = "No model is configured.";
        return false;
    }

    nlohmann::json messages = build_messages(prompt, instructions, history);
    nlohmann::json request = build_request(settings_, model_, messages, false);

    string authorization;
    if (!llm_trim(api_key_).empty())
    {
        authorization = "Bearer " + api_key_;
    }

    string extra_headers;
    if (settings_.provider == "openrouter")
    {
        extra_headers =
            "HTTP-Referer: https://automata-app.vercel.app\r\n"
            "X-Title: Automata\r\n";
    }

    HttpResponse http = http_request("POST", normalize_url(base_url_),
                                     request.dump(), "application/json",
                                     authorization, 10, 240, extra_headers);

    if (!http.ok)
    {
        error = "Network error: " + http.error;
        return false;
    }

    if (http.status < 200 || http.status >= 300)
    {
        error = "Provider error (" + to_string(http.status) + "): " +
                http_error_detail(http);
        return false;
    }

    try
    {
        nlohmann::json parsed = nlohmann::json::parse(http.body);
        if (!parsed.contains("choices") || !parsed["choices"].is_array() ||
            parsed["choices"].empty())
        {
            error = "Provider returned no choices.";
            return false;
        }
        const auto &choice = parsed["choices"][0];
        if (choice.contains("message"))
        {
            response = message_content(choice["message"]);
        }
        if (response.empty() && choice.contains("text") &&
            choice["text"].is_string())
        {
            response = choice["text"].get<string>();
        }
    }
    catch (const nlohmann::json::exception &e)
    {
        error = string("Could not parse provider response: ") + e.what();
        return false;
    }

    if (llm_trim(response).empty())
    {
        error = "The model returned an empty response.";
        return false;
    }

    return true;
}

bool OpenAIClient::generate_response_stream(
    const string &prompt,
    const string &instructions,
    const vector<RunData> &history,
    string &response,
    string &error,
    const function<void(const string &)> &on_delta)
{
    response.clear();
    error.clear();

    if (llm_trim(prompt).empty())
    {
        error = "Prompt cannot be empty!";
        return false;
    }
    if (base_url_.empty())
    {
        error = "No base URL is configured for this provider.";
        return false;
    }
    if (settings_.needs_key() && llm_trim(api_key_).empty())
    {
        error = "No API key is configured. Add one in Settings (BYOK) or set " +
                settings_.env_key_name() + ".";
        return false;
    }
    if (model_.empty())
    {
        error = "No model is configured.";
        return false;
    }

    nlohmann::json messages = build_messages(prompt, instructions, history);
    nlohmann::json request = build_request(settings_, model_, messages, true);

    string authorization;
    if (!llm_trim(api_key_).empty())
    {
        authorization = "Bearer " + api_key_;
    }

    string extra_headers = "Accept: text/event-stream\r\n";
    if (settings_.provider == "openrouter")
    {
        extra_headers +=
            "HTTP-Referer: https://automata-app.vercel.app\r\n"
            "X-Title: Automata\r\n";
    }

    string buffer;
    string stream_error;
    bool saw_done = false;
    bool saw_delta = false;

    auto consume_buffer = [&]()
    {
        for (;;)
        {
            string delta;
            SseResult result = next_sse_delta(buffer, delta, stream_error);
            if (result == SseResult::NeedMore)
            {
                break;
            }
            if (result == SseResult::Done)
            {
                saw_done = true;
                break;
            }
            if (result == SseResult::Error)
            {
                break;
            }
            if (result == SseResult::Delta)
            {
                saw_delta = true;
                response += delta;
                if (on_delta)
                {
                    on_delta(delta);
                }
            }
        }
    };

    HttpResponse http = http_request_stream(
        "POST", normalize_url(base_url_), request.dump(), "application/json",
        authorization, 10, 240,
        [&](const string &chunk)
        {
            buffer += chunk;
            consume_buffer();
        },
        extra_headers);

    if (!http.ok)
    {
        error = "Network error: " + http.error;
        return false;
    }

    if (http.status < 200 || http.status >= 300)
    {
        error = "Provider error (" + to_string(http.status) + "): " +
                http_error_detail(http);
        return false;
    }

    // Some servers omit the final newline.
    if (!buffer.empty())
    {
        buffer.push_back('\n');
        consume_buffer();
    }

    if (!stream_error.empty() && llm_trim(response).empty())
    {
        error = "Provider error: " + stream_error;
        return false;
    }

    if (!saw_delta)
    {
        // The provider may have ignored stream=true and sent a normal JSON
        // body. Fall back to the non-streaming parser.
        try
        {
            nlohmann::json parsed = nlohmann::json::parse(http.body);
            if (parsed.contains("choices") && parsed["choices"].is_array() &&
                !parsed["choices"].empty())
            {
                const auto &choice = parsed["choices"][0];
                if (choice.contains("message"))
                {
                    response = message_content(choice["message"]);
                }
                if (response.empty() && choice.contains("text") &&
                    choice["text"].is_string())
                {
                    response = choice["text"].get<string>();
                }
            }
        }
        catch (...)
        {
        }
        if (!response.empty() && on_delta)
        {
            on_delta(response);
        }
    }

    (void)saw_done;

    if (llm_trim(response).empty())
    {
        error = "The model returned an empty response.";
        return false;
    }

    return true;
}

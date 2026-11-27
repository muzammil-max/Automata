#include "gemini_client.h"

#include "http_client.h"
#include "llm_util.h"
#include "../libraries/json.hpp"

using namespace std;

GeminiClient::GeminiClient(const LLMSettings &settings)
    : settings_(settings),
      api_key_(current_api_key(settings)),
      base_url_(settings.effective_base_url()),
      model_(settings.effective_model())
{
}

bool GeminiClient::generate_response(
    const string &prompt,
    const string &instructions,
    string &response,
    string &error)
{
    vector<RunData> empty;
    return generate_response(prompt, instructions, empty, response, error);
}

bool GeminiClient::generate_response(
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
        error = "No Gemini API key is configured. Add one in Settings (BYOK) "
                "or set GEMINI_API_KEY.";
        return false;
    }

    string base = llm_trim(base_url_);
    while (!base.empty() && base.back() == '/')
    {
        base.pop_back();
    }
    if (base.empty())
    {
        base = "https://generativelanguage.googleapis.com/v1beta";
    }
    string model = model_.empty() ? "gemini-1.5-flash" : model_;
    const string url =
        base + "/models/" + model + ":generateContent?key=" + api_key_;

    nlohmann::json contents = nlohmann::json::array();
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
        contents.push_back(
            {{"role", "user"}, {"parts", {{{"text", llm_truncate(run.message, 600)}}}}});
        if (!run.output.empty())
        {
            contents.push_back(
                {{"role", "model"},
                 {"parts", {{{"text", llm_truncate(run.output, 1600)}}}}});
        }
    }
    contents.push_back({{"role", "user"}, {"parts", {{{"text", prompt}}}}});

    nlohmann::json request;
    request["contents"] = contents;
    if (!llm_trim(instructions).empty())
    {
        request["systemInstruction"] = {
            {"parts", {{{"text", instructions}}}}};
    }
    request["generationConfig"] = {
        {"temperature", settings_.temperature},
        {"maxOutputTokens", settings_.max_tokens}};

    HttpResponse http = http_request("POST", url, request.dump(),
                                     "application/json", "", 10, 240);

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
        error = "Gemini error (" + to_string(http.status) + "): " + detail;
        return false;
    }

    try
    {
        nlohmann::json parsed = nlohmann::json::parse(http.body);
        if (parsed.contains("candidates") && parsed["candidates"].is_array() &&
            !parsed["candidates"].empty())
        {
            const auto &candidate = parsed["candidates"][0];
            if (candidate.contains("content") &&
                candidate["content"].contains("parts"))
            {
                for (const auto &part : candidate["content"]["parts"])
                {
                    if (part.is_object() && part.contains("text") &&
                        part["text"].is_string())
                    {
                        response += part["text"].get<string>();
                    }
                }
            }
        }
    }
    catch (const nlohmann::json::exception &e)
    {
        error = string("Could not parse Gemini response: ") + e.what();
        return false;
    }

    if (llm_trim(response).empty())
    {
        error = "Gemini returned an empty response.";
        return false;
    }
    return true;
}

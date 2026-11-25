
#include "nvidia_client.h"

#include "../libraries/json.hpp"
#include "http_client.h" // cross-platform HTTP (curl / WinHTTP)

#include <iostream>
#include <cstdlib>
#include <string>
#include <vector>
#include <thread>
#include <chrono>

using namespace std;


// Cuts a string at a UTF-8 character boundary (json throws on broken UTF-8).
static string utf8_truncate(string text, size_t max_bytes)
{
    if (text.size() <= max_bytes)
    {
        return text;
    }

    text.resize(max_bytes);

    size_t i = text.size();
    while (i > 0 && (static_cast<unsigned char>(text[i - 1]) & 0xC0) == 0x80)
    {
        i--;
    }

    if (i == 0)
    {
        text.clear();
        return text;
    }

    unsigned char lead = static_cast<unsigned char>(text[i - 1]);
    int expected = 1;
    if ((lead & 0xE0) == 0xC0)      expected = 2;
    else if ((lead & 0xF0) == 0xE0) expected = 3;
    else if ((lead & 0xF8) == 0xF0) expected = 4;
    else if ((lead & 0x80) != 0)
    {
        text.clear();
        return text;
    }

    int have = static_cast<int>(text.size() - (i - 1));
    if (have != expected)
    {
        text.resize(i - 1);
    }

    return text;
}


// Receives response data from NVIDIA
// (purana curl callback — ab http_client.cpp handle karta hai)


// Four-argument function
bool NVIDIAClient::generate_response(
    const string& prompt,
    const string& instructions,
    string& response,
    string& error
)
{
    vector<RunData> empty_history;

    return generate_response(
        prompt,
        instructions,
        empty_history,
        response,
        error
    );
}


// Five-argument function with conversation history
bool NVIDIAClient::generate_response(
    const string& prompt,
    const string& instructions,
    const vector<RunData>& history,
    string& response,
    string& error
)
{
    response.clear();
    error.clear();


    // Get NVIDIA API key from the environment (documented in the README).
    // Kabhi bhi key ko source code mein hard-code na karo: ye repo public hai.
    const char* env_key = getenv("NVIDIA_API_KEY");
    const string api_key = (env_key != nullptr) ? string(env_key) : "";

    // Validate prompt
    if (prompt.empty())
    {
        error = "Prompt cannot be empty!";
        return false;
    }

    if (api_key.empty())
    {
        error = "NVIDIA_API_KEY is not set. Set the environment variable and restart the app.";
        return false;
    }


    // NVIDIA API configuration
    const string API_URL =
        "https://integrate.api.nvidia.com/v1/chat/completions";

    const string MODEL =
        "nvidia/nemotron-3.5-super-vl-120b-a12b";


    // Build conversation messages
    nlohmann::json messages =
        nlohmann::json::array();


    // Add system instructions
    messages.push_back({
        {"role", "system"},
        {"content", instructions}
    });


    // Add previous session history.
    // Only completed runs, only the user message and the final answer,
    // each capped in size: failed or looping runs would bias the model.
    size_t history_runs = 0;

    for (const RunData& run : history)
    {
        if (history_runs >= 6)
        {
            break;
        }

        if (run.status != "completed" || run.message.empty())
        {
            continue;
        }

        history_runs++;

        messages.push_back({
            {"role", "user"},
            {"content", utf8_truncate(run.message, 400)}
        });

        if (!run.output.empty())
        {
            messages.push_back({
                {"role", "assistant"},
                {"content", utf8_truncate(run.output, 1200)}
            });
        }
    }


    // Add current prompt
    messages.push_back({
        {"role", "user"},
        {"content", prompt}
    });


    // Construct NVIDIA request
    nlohmann::json request;

    request["model"] = MODEL;
    request["messages"] = messages;

    request["temperature"] = 1;
    request["top_p"] = 0.95;
    request["max_tokens"] = 16384;

    request["seed"] = 42;

    // NVIDIA Nemotron reasoning configuration
    request["reasoning_effort"] = "high";

    // We need a complete response because
    // ActionParser expects one complete JSON object.
    request["stream"] = false;


    string request_body = request.dump();

    string api_response;


    // ============================================================
    // HTTP request (Linux par curl, Windows par WinHTTP).
    // Retry wahi hai: connection error, 429 aur 5xx par dobara try.
    // ============================================================
    nlohmann::json data;
    long http_status = 0;
    string last_error;
    bool got_answer = false;

    const int MAX_ATTEMPTS = 3;

    for (int attempt = 1; attempt <= MAX_ATTEMPTS; attempt++)
    {
        HttpResponse http = http_request("POST",
                                         API_URL,
                                         request_body,
                                         "application/json",
                                         "Bearer " + string(api_key),
                                         20,
                                         180);

        api_response = http.body;
        http_status = http.status;

        // Connection error: dobara try karna chahiye.
        if (!http.ok)
        {
            last_error = http.error;

            cout << "NVIDIA request failed (attempt " << attempt
                 << "/" << MAX_ATTEMPTS << "): " << last_error << endl;

            if (attempt < MAX_ATTEMPTS)
            {
                this_thread::sleep_for(chrono::seconds(attempt));
                continue;
            }
            break;
        }

        // HTTP 429 (rate limit) aur 5xx: dobara try karo.
        if (http_status == 429 || (http_status >= 500 && http_status < 600))
        {
            last_error = "HTTP " + to_string(http_status);

            cout << "NVIDIA " << last_error << " (attempt " << attempt
                 << "/" << MAX_ATTEMPTS << ")" << endl;

            if (attempt < MAX_ATTEMPTS)
            {
                this_thread::sleep_for(chrono::seconds(attempt));
                continue;
            }
            break;
        }

        // NVIDIA ka jawab parse karo.
        try
        {
            data = nlohmann::json::parse(api_response);
        }
        catch (const nlohmann::json::exception& e)
        {
            error = "Invalid NVIDIA API response: ";
            error += e.what();

            error += "\nResponse: ";
            error += utf8_truncate(api_response, 1000);

            return false;
        }

        got_answer = true;
        break;
    }


    // Every attempt failed (connection or 429 / 5xx).
    if (!got_answer)
    {
        error = "NVIDIA API failed after " + to_string(MAX_ATTEMPTS) +
                " attempts: " + last_error;
        return false;
    }


    // Handle HTTP errors (final answer, not retried)
    if (http_status < 200 || http_status >= 300)
    {
        error = "NVIDIA API error (HTTP ";
        error += to_string(http_status);
        error += "): ";

        if (data.contains("error"))
        {
            if (data["error"].is_object() &&
                data["error"].contains("message") &&
                data["error"]["message"].is_string())
            {
                error += data["error"]["message"].get<string>();
            }
            else
            {
                error += data["error"].dump();
            }
        }
        else
        {
            error += utf8_truncate(api_response, 1000);
        }

        return false;
    }


    // Check response choices
    if (!data.contains("choices") ||
        !data["choices"].is_array() ||
        data["choices"].empty())
    {
        error = "NVIDIA returned no response choices!";
        return false;
    }


    // Get first choice
    const nlohmann::json& choice =
        data["choices"][0];

    if (!choice.contains("message") ||
        !choice["message"].is_object())
    {
        error = "NVIDIA response does not contain a message!";
        return false;
    }


    const nlohmann::json& message =
        choice["message"];


    // Extract final response content
    if (!message.contains("content") ||
        !message["content"].is_string())
    {
        error = "NVIDIA returned no final text content.";

        string finish_reason =
            choice.value("finish_reason", string());

        if (finish_reason == "length")
        {
            error += " Output token limit may have been reached.";
        }

        if (message.contains("reasoning_content"))
        {
            error += " Reasoning was returned, but final content was empty.";
        }

        return false;
    }


    response = message["content"].get<string>();


    if (response.empty())
    {
        error = "NVIDIA returned an empty response!";
        return false;
    }


    cout << "NVIDIA Nemotron response received successfully."
         << endl;

    return true;
}

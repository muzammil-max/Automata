#pragma once

#include <functional>
#include <string>
#include <vector>

#include "../storage/session_data.h"

using namespace std;

class LLMClient
{
public:

    // every ai will use this
    virtual bool generate_response(
        const string& prompt,
        const string& instructions,
        string& response,
        string& error
    ) = 0;


    virtual bool generate_response(
        const string& prompt,
        const string& instructions,
        const vector<RunData>& history,
        string& response,
        string& error
    )
    {
     

        return generate_response(
            prompt,
            instructions,
            response,
            error
        );
    }

    // Streaming variant used by the chat path. `on_delta` is called with each
    // new piece of text; `response` always contains the full answer so far.
    // The default implementation falls back to a normal request and emits the
    // complete answer in one chunk (providers without SSE support).
    virtual bool generate_response_stream(
        const string& prompt,
        const string& instructions,
        const vector<RunData>& history,
        string& response,
        string& error,
        const function<void(const string&)>& on_delta)
    {
        bool ok = generate_response(prompt, instructions, history, response, error);
        if (ok && on_delta)
        {
            on_delta(response);
        }
        return ok;
    }


    virtual ~LLMClient() = default;
};
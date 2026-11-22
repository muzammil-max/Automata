
#pragma once

#include "llm_client.h"
#include "../storage/session_data.h"

#include <string>
#include <vector>

using namespace std;

class NVIDIAClient : public LLMClient
{
public:

    // Original LLMClient interface
    bool generate_response(
        const string& prompt,
        const string& instructions,
        string& response,
        string& error
    ) override;


    // History-aware function
    bool generate_response(
        const string& prompt,
        const string& instructions,
        const vector<RunData>& history,
        string& response,
        string& error
    ) override;
};

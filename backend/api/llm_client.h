#pragma once

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


    virtual ~LLMClient() = default;
};
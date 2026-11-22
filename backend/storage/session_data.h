#pragma once

#include <string>
#include <vector>
#include "../libraries/json.hpp"

using namespace std;

class RunData
{
public:
    int runID = 0;

    string message;
    string status = "queued";
    string output;

    nlohmann::json steps = nlohmann::json::array();
};

class SessionData
{
public:
    int sessionID = 0;

    string title;

    vector<RunData> runs;
};
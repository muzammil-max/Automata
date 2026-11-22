#pragma once
#include <iostream>
using namespace std;

//actions stored made by AI

struct BrowserAction
{

    string action;
    string target;
    string value;
};

class ActionParser
{
    public:
    bool parse_action (
        const string& ai_response,
        BrowserAction& action,
        string& error
    );
};
#include <iostream>
#pragma once
using namespace std;
class Instruction

{
    public:
    static string get_text();       // system prompt for browser actions
    static string get_chat_text();  // system prompt for normal chat (no browser)
};
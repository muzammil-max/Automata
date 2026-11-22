#pragma once

#include <string>
#include <vector>
#include <map>
#include <memory>
#include <atomic>
#include <functional>

#include "../api/llm_client.h"
#include "../browser/browser_controller.h"
#include "../parser/parser.h"
#include "../storage/session_data.h"

using namespace std;
class Agent
{
//llm cleint so that we cna use multiple AI provier
//giving agnent the control to chromium by using Browser
//paser conrverts the AI json response to cpp browser action that actually execute function
    private:
    LLMClient & llm_client;
    BrowserController browser;
    ActionParser  parser;

    string notes;                // NOTES: saved with remember + every successful extract
    string page_text;            // automatic capture of the last opened page
    map<string, int> visited;    // PAGES ALREADY OPENED: url -> how often
    map<string, int> nav_tries;  // every navigate try (even failed) -> how many times
    map<string, string> saved_pages; // url -> text copy we already put into NOTES
    bool did_open = false;       // at least one successful navigate
    bool did_read = false;       // at least one successful extract
    bool did_write = false;      // at least one successful type

    
    void capture_page_text(const string &url);

    bool execute_action( //one browser action
        const BrowserAction& action,
        string& page_source,
        string & result,
        string& error
    );

    public:
    Agent(LLMClient& client);

   
    shared_ptr<atomic<bool>> cancel_flag;

  
    function<void(const RunData&)> on_step;

    bool run_task ( //the entire task like brwoser
        const string& user_prompt,
        const vector<RunData>& history,
        RunData& current_run,
        string& final_output,   string& error
    );
};
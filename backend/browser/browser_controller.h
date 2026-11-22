
#pragma once

#include <iostream>
#include <string>
#include <mutex>
#include "../libraries/json.hpp"

using namespace std;

// Talks to ChromeDriver (W3C WebDriver protocol) and controls one browser.
class BrowserController
{
public:
    BrowserController();
    ~BrowserController();

    bool close_on_exit = false;

    bool start_browser(string &error);
    bool close_browser(string &error);

    // session_url is STATIC: every BrowserController shares ONE browser.
    bool is_running();

    bool navigate(const string &url, string &error);
    bool new_tab(const string &url, string &error);
    bool go_back(string &error);

    bool switch_frame(const string &selector, string &error);

    bool get_title(string &title, string &error);
    bool getCode(string &page_source, string &error);

    bool get_page_summary(string &summary, string &error);

    bool find_element_id(const string &selector, string &element_id, string &error);

    bool execute_script(const string &script, const string &args_json,
                        string &response_body, string &error);

    bool scroll(const string &target, const string &pixels, string &error);
    bool drag(const string &target, const string &points, string &error);
    bool press(const string &selector, const string &key, string &error);
    bool selection_option(const string &selector, const string &option, string &error);
    bool wait_for_element(const string &selector, int timeout_ms, string &error);
    bool click(const string &selector, string &error);
    bool type(const string &selector, const string &text, string &error);
    bool extract_text(const string &selector, string &text, string &error);
    bool reload(string &error);

private:
    string driver_url;
    string browser_path;

    // The one browser session, shared by ALL BrowserController objects.
    static string session_url;

    // Guards every ChromeDriver request and session changes,
    // because the HTTP server works on several threads.
    static recursive_mutex browser_mutex;

    bool request_bhejo(const string &method, const string &endpoint,
                       const string &request_body, string &reponse_body,
                       string &error);

    bool run_js(const string &script, const nlohmann::json &args,
                nlohmann::json &value, string &error);

    void wait_until_loaded(int timeout_ms = 5000);
    void scroll_and_focus(const string &element_id, bool focus);
};

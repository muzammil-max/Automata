
#pragma once

#include <iostream>
#include <string>
#include <mutex>
#include <vector>
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
    bool go_forward(string &error);

    bool list_tabs(vector<string> &tabs, int &active_index, string &error);
    bool switch_tab(const string &which, string &error);
    bool close_tab(const string &which, string &error);

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
    bool double_click(const string &selector, string &error);
    bool right_click(const string &selector, string &error);
    bool hover(const string &selector, string &error);
    bool focus_element(const string &selector, string &error);
    bool clear_element(const string &selector, string &error);
    bool set_checked(const string &selector, bool checked, string &error);
    bool scroll_into_view(const string &selector, string &error);
    bool get_html(const string &selector, string &html, string &error);
    bool type(const string &selector, const string &text, string &error);
    bool extract_text(const string &selector, string &text, string &error);
    bool upload_file(const string &selector, const string &file_path, string &error);
    bool screenshot(const string &file_path, string &saved_path, string &error);
    bool reload(string &error);
    bool wait_for_navigation(int timeout_ms, string &error);

    bool get_cookies(string &cookies_json, string &error);
    bool set_cookie(const string &cookie_json, string &error);
    bool delete_cookies(string &error);

    bool set_window_size(int width, int height, string &error);

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
    bool element_center(const string &element_id, double &x, double &y,
                        string &error);
};

#include "agent.h"

#include <iostream>
#include <thread>
#include <chrono>
#include <mutex>
#include <fstream>
#include <cstdlib>
#include <cctype>
#include <exception>
#include <algorithm>

#include "../instructions/instruction.h"

using namespace std;

static string normalize_url(string url)
{
    while (!url.empty() && (url.front() == ' ' || url.front() == '\t' ||
                            url.front() == '\n' || url.front() == '\r'))
    {
        url.erase(url.begin());
    }
    while (!url.empty() && (url.back() == ' ' || url.back() == '\t' ||
                            url.back() == '\n' || url.back() == '\r'))
    {
        url.pop_back();
    }

    size_t hash = url.find('#'); // fix neeed!!! xD
    if (hash != string::npos)
    {
        url.resize(hash);
    }

    // "https://site.com/" and "https://site.com" are the same page.
    while (!url.empty() && url.back() == '/')
    {
        url.pop_back();
    }

    transform(url.begin(), url.end(), url.begin(),
              [](unsigned char c)
              { return static_cast<char>(tolower(c)); });

    return url;
}

static string page_url_from_summary(const string &page_source)
{
    try
    {
        return nlohmann::json::parse(page_source).value("url", string());
    }
    catch (...)
    {
        return "";
    }
}

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
        text.clear(); // only
        return text;
    }

    unsigned char lead = static_cast<unsigned char>(text[i - 1]);
    int expected = 1;
    if ((lead & 0xE0) == 0xC0)
        expected = 2;
    else if ((lead & 0xF0) == 0xE0)
        expected = 3;
    else if ((lead & 0xF8) == 0xF0)
        expected = 4;
    else if ((lead & 0x80) != 0)
    {
        text.clear(); // stray continuation byte
        return text;
    }

    int have = static_cast<int>(text.size() - (i - 1));
    if (have != expected)
    {
        text.resize(i - 1); //
    }

    return text;
}

// Chat ka chhota recap — taake agent purani baatein na bhoole.
// Sirf last few runs: user ka message + hamara jawab.
static string conversation_context(const vector<RunData> &history, size_t max_runs)
{
    if (history.empty() || max_runs == 0)
    {
        return "";
    }

    size_t start = history.size() > max_runs ? history.size() - max_runs : 0;
    string recap;

    for (size_t i = start; i < history.size(); i++)
    {
        const RunData &run = history[i];
        if (run.message.empty())
        {
            continue;
        }

        recap += "User: " + utf8_truncate(run.message, 300) + "\n";

        if (!run.output.empty())
        {
            recap += "You: " + utf8_truncate(run.output, 600) + "\n";
        }
    }

    return recap;
}

// Constructor
Agent::Agent(LLMClient &client)
    : llm_client(client)
{
}

bool Agent::execute_action(
    const BrowserAction &action,
    string &page_source,
    string &result,
    string &error)
{
    error.clear();
    result.clear();

    bool success = false;

    auto navigation_allowed = [&](const string &target) -> bool
    {
        // An empty URL is used by new_tab for a blank tab. Nothing to check.
        if (target.empty())
        {
            return true;
        }

        string key = normalize_url(target);

        // Already opened successfully once? No second visit.
        if (visited[key] > 0)
        {
            error = "BLOCKKKED: this page was already opened (" + target + "). "
                                                                           "Do not open the same page again. The text you read from it "
                                                                           "is already saved in NOTES. Continue with the NEXT step of "
                                                                           "the task (for example: open the notepad and use type).";
            return false;
        }

        // A URL that failed two times is probably dead. Do not loop on it.
        if (nav_tries[key] >= 2)
        {
            error = "BLOCKED: this URL already failed two times (" + target + "). "
                                                                              "Stop trying it. Use reRload, choose a different page, or "
                                                                              "finish and tell the user that the website is not reachable.";
            return false;
        }

        nav_tries[key]++;
        return true;
    };

    if (action.action == "navigate")
    {
        if (navigation_allowed(action.target))
        {
            success = browser.navigate(action.target, error);
        }

        if (success)
        {
            result = "Navigation completed successfully.";
            visited[normalize_url(action.target)]++;
            did_open = true;
            capture_page_text(action.target); // remember what the page saysss
        }
    }
    else if (action.action == "go_back")
    {
        success = browser.go_back(error);
        if (success)
            result = "Went back to the previous page.";
    }
    else if (action.action == "new_tab")
    {
        // Opens a second tab; it becomes the active one!!!!
        if (navigation_allowed(action.target))
        {
            success = browser.new_tab(action.target, error);
        }

        if (success)
        {
            result = action.target.empty()
                         ? "New empty tab opened and focused."
                         : "New tab opened at " + action.target + ".";
            if (!action.target.empty())
            {
                visited[normalize_url(action.target)]++;
            }
            did_open = true;
            capture_page_text(action.target);
        }
    }
    else if (action.action == "reload")
    {
        success = browser.reload(error);
        if (success)
            result = "Page reloaded.";
    }
    else if (action.action == "remember")
    {
        // TELLING THE STUPUD AGENT
        // TS HALUCINATE LAOT
        if (!notes.empty())
            notes += "\n";
        notes += action.value;

        if (notes.size() > 6000)
            notes = utf8_truncate(notes, 6000);

        success = true;
        result = "Saved to NOTES: " + utf8_truncate(action.value, 200);
    }
    else if (action.action == "switch_frame")
    {
        success = browser.switch_frame(action.target, error);
        if (success)
            result = "Switched frame. The page content below now shows this frame only.";
    }
    else if (action.action == "click")
    {
        success = browser.click(action.target, error);
        if (success)
            result = "Click completed successfully.";
    }
    else if (action.action == "type")
    {
        success = browser.type(action.target, action.value, error);
        if (success)
        {
            result = "Text entered successfully.";
            did_write = true;
        }
    }
    else if (action.action == "extract")
    {
        string extracted_text;
        success = browser.extract_text(action.target, extracted_text, error);
        if (success)
        {
            // Keep the answer to a sensible size.
            extracted_text = utf8_truncate(extracted_text, 6000);

            result = "Extracted text:\n";
            result += extracted_text;

            if (!notes.empty())
                notes += "\n";
            notes += utf8_truncate(extracted_text, 3000);
            if (notes.size() > 9000)
                notes = utf8_truncate(notes, 9000);

            did_read = true;
        }
    }
    else if (action.action == "wait")
    {
        try
        {
            size_t processed = 0;
            int milliseconds = stoi(action.value, &processed);

            if (processed != action.value.size() || milliseconds < 100 || milliseconds > 10000)
            {
                error = "Wait duration must be between 100 and 10000 milliseconds!";
            }
            else
            {
                this_thread::sleep_for(chrono::milliseconds(milliseconds));
                success = true;
                result = "Wait completed successfully.";
            }
        }
        catch (const exception &)
        {
            error = "Invalid wait duration!";
        }
    }
    else if (action.action == "scroll")
    {
        success = browser.scroll(action.target, action.value, error);
        if (success)
            result = "Page scrolled successfully.";
    }
    else if (action.action == "press")
    {
        success = browser.press(action.target, action.value, error);
        if (success)
            result = "Keyboard key pressed successfully.";
    }
    else if (action.action == "select")
    {
        success = browser.selection_option(action.target, action.value, error);
        if (success)
            result = "Dropdown option selected successfully.";
    }
    else if (action.action == "drag")
    {
        // Drag = press the mouse, move it, release it. This is the action
        // that a drawing canvas needs (whiteboard, paint app, signature box).
        // The real mouse work is inside BrowserController::drag().
        success = browser.drag(action.target, action.value, error);
        if (success)
            result = "Drag done (mouse pressed, moved and released).";
    }
    else if (action.action == "wait_for")
    {
        try
        {
            size_t processed = 0;
            int timeout_ms = stoi(action.value, &processed);

            if (processed != action.value.size())
            {
                error = "Invalid wait timeout!";
            }
            else
            {
                success = browser.wait_for_element(action.target, timeout_ms, error);
                if (success)
                    result = "Requested element appeared.";
            }
        }
        catch (const exception &)
        {
            error = "Invalid wait timeout!";
        }
    }
    else
    {
        error = "Unsupported browser action: " + action.action;
    }

    // Refresh the page description (even after a failure).
    string summary;
    string page_error;

    if (browser.get_page_summary(summary, page_error))
    {
        page_source = utf8_truncate(summary, 15000);

        // A site can redirect: "example.org" becomes "example.org/".
        // A click can also open a new page without a "navigate" action.
        // Save the real URL too, so the loop guard catches both cases and
        // the page text goes into NOTES one time.
        if (success)
        {
            string actual_url = page_url_from_summary(page_source);
            string actual_key = normalize_url(actual_url);

            if (!actual_key.empty() && actual_key != "about:blank" &&
                visited[actual_key] == 0)
            {
                visited[actual_key] = 1;
                capture_page_text(actual_url);
            }
        }
    }
    else if (success)
    {
        result += "\nCould not refresh webpage: " + page_error;
    }

    return success;
}

//   2. open notepad.cc           -> NOTES still has the text
//   3. model tries the site again -> blocked, because it is already in visited
//   4. model types the notes in the notepad -> task done
void Agent::capture_page_text(const string &url)
{
    string text;
    string capture_error;

    if (browser.extract_text("", text, capture_error))
    {
        page_text = utf8_truncate(text, 4000);

        // Save each page only once. A long page can eat all the space in
        // NOTES, so we keep a smaller piece (1200 bytes) for the memory.
        string key = normalize_url(url);
        if (!key.empty() && saved_pages.find(key) == saved_pages.end())
        {
            string memory_copy = utf8_truncate(text, 1200);
            saved_pages[key] = memory_copy;

            if (!notes.empty())
                notes += "\n";
            notes += "[PAGE " + url + "]\n" + memory_copy;

            // Keep NOTES below the same limit used by remember/extract.
            if (notes.size() > 9000)
                notes = utf8_truncate(notes, 9000);
        }
    }
}

// Detects pages that block automation: Google captcha, consent pages
// and generic cookie banners. Returns a message for the model, or "".
static string detect_blocking_page(const string &page_source)
{
    // Lower-case copy for a case-insensitive search (ASCII keywords).
    string text = page_source;
    transform(text.begin(), text.end(), text.begin(),
              [](unsigned char c)
              { return static_cast<char>(tolower(c)); });

    auto has = [&text](const char *needle)
    {
        return text.find(needle) != string::npos;
    };

    // Captcha / bot check ("unusual traffic", "captcha", ...)
    if (has("unusual traffic") || has("verify you are human") ||
        has("not a robot") || has("captcha") || has("recaptcha"))
    {
        return "THIS PAGE IS BLOCKED by a captcha or bot check. "
               "Do NOT stay on this site and do not retry it. "
               "Search with DuckDuckGo instead: "
               "https://duckduckgo.com/?q=YOUR+QUERY "
               "(use + for spaces, take the query from the user's task).";
    }

    // Google consent page ("before you continue", ...)
    if (has("before you continue") || has("consent.google"))
    {
        return "THIS PAGE IS A CONSENT PAGE (\"before you continue\"). "
               "Do not fill in any form here. Either click an \"Accept all\" / "
               "\"I agree\" button from the element list, or leave this page "
               "and search with DuckDuckGo instead: "
               "https://duckduckgo.com/?q=YOUR+QUERY";
    }

    // Generic cookie banner covering the page
    if (has("accept all cookies") || has("allow all cookies") ||
        has("we use cookies") || has("cookie policy") ||
        has("cookies help us") || has("manage cookies"))
    {
        return "A COOKIE BANNER is covering the page. First click an "
               "\"Accept\" / \"Allow all\" / \"Accept cookies\" button from "
               "the element list, then continue with the task.";
    }

    return "";
}

// ---------------
static string to_lower_copy(string s)
{
    transform(s.begin(), s.end(), s.begin(),
              [](unsigned char c)
              { return static_cast<char>(tolower(c)); });
    return s;
}

// Reads a comma separated list from the environment.
static vector<string> env_list(const char *name, const char *fallback)
{
    const char *raw = getenv(name);
    string text = (raw != nullptr && *raw != '\0') ? raw : fallback;

    vector<string> out;
    string current;
    for (char c : text)
    {
        if (c == ',')
        {
            if (!current.empty())
                out.push_back(to_lower_copy(current));
            current.clear();
        }
        else if (c != ' ')
        {
            current += c;
        }
    }
    if (!current.empty())
        out.push_back(to_lower_copy(current));

    return out;
}

static bool contains_any(const string &haystack, const vector<string> &needles)
{
    for (const string &needle : needles)
    {
        if (!needle.empty() && haystack.find(needle) != string::npos)
        {
            return true;
        }
    }
    return false;
}

// Returns a description of the risk, or "" when the action is safe.

static string safety_risk(const BrowserAction &action,
                          const string &page_source,
                          const string &user_prompt)
{
    // The user already approved in this run.
    if (user_prompt.find("USER CONFIRMED") != string::npos)
    {
        return "";
    }

    // Only actions that can buy, delete, send or enter secrets.
    if (action.action != "click" && action.action != "type" &&
        action.action != "select" && action.action != "press")
    {
        return "";
    }

    // Current page URL (page_source is the summary JSON).
    string url;
    try
    {
        url = nlohmann::json::parse(page_source).value("url", string());
    }
    catch (...)
    {
        // No URL: fall back to keyword checks only.
    }

    string page = to_lower_copy(url);

    // Allowlist wins: these URLs are always safe.
    if (contains_any(page, env_list("AGENT_ALLOW_URLS", "")))
    {
        return "";
    }

    // The page itself is a payment / deletion page.
    if (contains_any(page, env_list(
                               "AGENT_RISK_URLS",
                               "checkout,payment,/cart,billing,order-confirmation,"
                               "delete,remove,unsubscribe,transfer")))
    {
        return "the current page (" + url + ") is a payment, order or "
                                            "deletion page";
    }

    // Words in the button / field that was targeted.
    string target_text = to_lower_copy(action.target + " " + action.value);

    if (action.action == "type")
    {
        // Typing is only risky when it is a secret or payment detail.
        if (contains_any(target_text, env_list(
                                          "AGENT_RISK_KEYWORDS",
                                          "password,passcode,credit card,card number,cvv,iban,"
                                          "sort code,ssn,passport")))
        {
            return "the text to type looks like a password or payment detail";
        }
        return "";
    }

    if (contains_any(target_text, env_list(
                                      "AGENT_RISK_KEYWORDS",
                                      "buy now,buy,purchase,checkout,pay now,pay,place order,"
                                      "order now,delete,remove,send,submit,publish,post,"
                                      "unsubscribe,transfer,book now,confirm order")))
    {
        return "the action \"" + action.action + "\" on \"" + action.target +
               "\" looks like a purchase, deletion or sending action";
    }

    return "";
}

// True when `word` appears as a whole word (not inside \"research\" or \"table\").
static bool contains_whole_word(const string &haystack, const string &word)
{
    size_t pos = 0;
    while ((pos = haystack.find(word, pos)) != string::npos)
    {
        bool left_ok = pos == 0 ||
                       !isalnum(static_cast<unsigned char>(haystack[pos - 1]));
        size_t after = pos + word.size();
        bool right_ok = after >= haystack.size() ||
                        !isalnum(static_cast<unsigned char>(haystack[after]));
        if (left_ok && right_ok)
        {
            return true;
        }
        pos = after;
    }
    return false;
}

static bool task_needs_browser(const string &prompt)
{
    string p = to_lower_copy(prompt);

    // A real URL is the strongest signal.
    if (p.find("http://") != string::npos ||
        p.find("https://") != string::npos ||
        p.find("www.") != string::npos)
    {
        return true;
    }
    // curl" contains "url".
    if (contains_whole_word(p, "url") ||
        contains_whole_word(p, "search") ||
        contains_whole_word(p, "form") ||
        contains_whole_word(p, "tab"))
    {
        // These are only strong signals in a web context.
        if (contains_whole_word(p, "url") ||
            p.find("search for") != string::npos ||
            p.find("search the web") != string::npos ||
            p.find("search online") != string::npos ||
            p.find("web form") != string::npos ||
            p.find("fill out") != string::npos ||
            p.find("fill in") != string::npos ||
            p.find("new tab") != string::npos ||
            p.find("browser tab") != string::npos)
        {
            return true;
        }
    }

    const char *browser_words[] = {
        "go to", "open the", "open a", "open this", "open website",
        "navigate", "browse", "browser", "website", "web site",
        "webpage", "web page", "on the internet", "online",
        "search for", "search the web", "search online",
        "google", "duckduckgo", "bing ", "click", "scroll",
        "download", "upload", "login", "log in", "sign in", "sign up",
        "notepad", "html viewer", "html editor", "code editor",
        "codepen", "jsfiddle", "w3schools", "youtube", "wikipedia",
        "new tab", "browser tab", "type into", "fill in", "fill out",
        "find me", "amazon", "linkedin", "instagram", "facebook",
        "twitter", "gmail", "outlook",
        ".com", ".org", ".net", ".io", ".dev", ".ai", // ai helped in this!

        // Drawing / art tasks: user says "draw" -> agent must use the
        // browser drawing tool (canvas / paint app), not just chat.
        "draw", "sketch", "sketchy", "paint", "painting", "canvas",
        "whiteboard", "doodle", "drawing"}; // drawing words -> browser task

    // const char *browser_words[] = {
    //     "go to", "open the", "open a", "open this", "open website",
    //     "navigate", "browse", "browser", "website", "web site",
    //     "webpage", "web page", "on the internet", "online",
    //     "search for", "search the web", "search online",
    //     "google", "duckduckgo", "bing ", "click", "scroll",
    //     "download", "upload", "login", "log in", "sign in", "sign up",
    //     "notepad", "html viewer", "html editor", "code editor",
    //     "codepen", "jsfiddle", "w3schools", "youtube", "wikipedia",
    //     "new tab", "browser tab", "type into", "fill in", "fill out",
    //     "find me", "amazon", "linkedin", "instagram", "facebook",
    //     "twitter", "gmail", "outlook",
    //     ".com", ".org", ".net", ".io", ".dev", ".ai"
    // };

    for (const char *word : browser_words)
    {
        if (p.find(word) != string::npos)
        {
            return true;
        }
    }

    return false;
}

static bool prompt_is_edit_followup(const string &prompt)
{
    string p = to_lower_copy(prompt);

    const char *phrases[] = {
        "make some changes", "make changes", "make a change", "make it",
        "make the", "change the", "change it", "change this", "change that",
        "change the code", "change this code", "change that code",
        "update the code", "update this code", "edit the code", "edit this code",
        "fix the code", "fix this code", "add to the code", "add to this code",
        "modify", "edit the", "edit it", "update the", "update it",
        "add a", "add an", "add another", "add some", "remove the",
        "remove this", "delete the", "replace the", "insert ",
        "fix the", "fix it", "improve the", "adjust the", "style the",
        "style it", "center the", "align the", "rename the", "resize"};

    for (const char *phrase : phrases)
    {
        if (p.find(phrase) != string::npos)
        {
            return true;
        }
    }

    // Catch the singular/plural forms of the most common change verbs.
    if (p.find("change") != string::npos ||
        contains_whole_word(p, "add") ||
        contains_whole_word(p, "remove") ||
        contains_whole_word(p, "delete") ||
        contains_whole_word(p, "fix") ||
        contains_whole_word(p, "edit"))
    {
        return true;
    }

    return false;
}

// Main agent function
bool Agent::run_task(
    const string &user_prompt,
    const vector<RunData> &history,
    RunData &current_run,
    string &final_output,
    string &error)
{
    final_output.clear();
    error.clear();

    if (user_prompt.empty())
    {
        error = "User prompt cannot be empty!";
        return false;
    }

    if (!current_run.steps.is_array())
    {
        current_run.steps = nlohmann::json::array();
    }

    // Only one task may drive the browser at a time.
    // Two simultaneous requests are handled one after the other.
    static mutex browser_task_mutex;
    lock_guard<mutex> task_lock(browser_task_mutex);

    // A new task starts with a clean memory.
    notes.clear();
    page_text.clear();
    visited.clear();
    nav_tries.clear();
    saved_pages.clear();
    did_open = false;
    did_read = false;
    did_write = false;

    // Saves one step into the run history.
    auto record_step = [&](const string &action,
                           const string &target,
                           const string &value,
                           const string &status,
                           const string &result,
                           const string &step_error)
    {
        nlohmann::json step;
        step["step"] = current_run.steps.size() + 1;
        step["action"] = action;
        step["target"] = target;
        step["value"] = value;
        step["status"] = status;
        step["result"] = result;

        if (!step_error.empty())
        {
            step["error"] = step_error;
        }
        current_run.steps.push_back(step);

        // Let the server save this step immediately (live log).
        if (on_step)
        {
            try
            {
                on_step(current_run);
            }
            catch (...)
            {
                // Never let a save problem kill the run.
            }
        }
    };

    bool followup_edit = browser.is_running() &&
                         prompt_is_edit_followup(user_prompt);
    bool needs_browser = task_needs_browser(user_prompt) || followup_edit;

    if (!needs_browser)
    {
        cout << "\nChat task (no browser needed)." << endl;

        string chat_prompt;

        // Pehle purani baatein — warna model har message ko naya samajhta hai.
        string recap = conversation_context(history, 8);
        if (!recap.empty())
        {
            chat_prompt += "CONVERSATION SO FAR (older messages in this chat):\n";
            chat_prompt += recap;
            chat_prompt += "\n";
        }

        chat_prompt += "USER MESSAGE:\n";
        chat_prompt += user_prompt;
        chat_prompt += "\n\nReply directly to the user in plain text.\n";
        chat_prompt += "Reminder: you DO have browser tools (navigate, click, "
                       "type, extract, scroll, press, select, new_tab, drag, "
                       "switch_frame, reload). If the user asks for something "
                       "on a website, you can do it yourself — just tell them "
                       "to send it as a direct command.";

        string answer;
        if (!llm_client.generate_response(chat_prompt,
                                          Instruction::get_chat_text(),
                                          history, answer, error))
        {
            record_step("chat", "", "", "failed", "", error);
            return false;
        }

        final_output = answer;
        record_step("chat", "", "", "completed",
                    utf8_truncate(answer, 300), "");
        return true;
    }

    string effective_prompt = user_prompt;

    if (followup_edit && !task_needs_browser(user_prompt))
    {
        effective_prompt =
            "CONTINUE THE BROWSER TASK THAT IS ALREADY OPEN.\n"
            "The user wants you to change the page or code that is currently loaded.\n"
            "Do not tell the user to copy or paste code.\n"
            "If you need the current editor content, use extract on the editor first.\n"
            "Then use type on the same editor with the complete updated code.\n"
            "The type action replaces the old text. Do not navigate away from the editor.\n\n"
            "USER REQUEST:\n" +
            user_prompt;
    }

    string page_source = "No webpage opened yet.";
    string previous_result;

    // ---- Reuse the open browser, or launch a new one ----
    bool reuse_browser = false;

    if (browser.is_running())
    {
        // Check that the old window still exists (the user may have closed it).
        string title, title_error;
        if (browser.get_title(title, title_error))
        {
            reuse_browser = true;
        }
        else
        {
            cout << "\nOld browser window is gone. Starting a new one." << endl;
        }
    }

    if (reuse_browser)
    {
        cout << "\nUsing the browser that is already open." << endl;

        did_open = true;

        string ignored_error;
        browser.switch_frame("default", ignored_error);

        string summary, page_error;
        if (browser.get_page_summary(summary, page_error))
        {
            page_source = utf8_truncate(summary, 15000);

            string open_url = page_url_from_summary(page_source);
            string open_key = normalize_url(open_url);
            if (!open_key.empty())
            {
                visited[open_key] = 1;
            }
        }

        previous_result = "Continuing in the browser that is already open. "
                          "The webpage shown is the current page. "
                          "Do not navigate to the current URL again; use reload "
                          "if the page must be refreshed.";

        record_step("browser_reuse", "", "", "completed",
                    "Reused the already open browser.", "");
    }
    else
    {
        cout << "\nLaunching browser..." << endl;

        if (!browser.start_browser(error))
        {
            record_step("browser_start", "", "", "failed", "", error);
            return false;
        }

        previous_result = "Browser launched successfully.";
        record_step("browser_start", "", "", "completed",
                    "Browser launched successfully.", "");
    }

    // The static rules live in the system instructions.
    string instructions = Instruction::get_text();

    int MAX_ACTIONS = 100; // real browser actions
    const char *env_actions = getenv("AGENT_MAX_ACTIONS");
    if (env_actions != nullptr && string(env_actions).empty() == 0)
    {
        try
        {
            int parsed = stoi(string(env_actions));
            if (parsed >= 1 && parsed <= 1000)
            {
                MAX_ACTIONS = parsed;
            }
        }
        catch (...)
        {
            // Bad value: keep the default 100.
        }
    }
    const int MAX_PARSE_FAILURES = 2; // bad AI answers: 1 try + 1 retry
    const int MAX_HISTORY_STEPS = 10; // steps sent back to the AI
    const int MAX_SAME_ACTION = 3;    // identical action repeated

    int actions_used = 0;
    int parse_failures = 0;

    string last_signature;
    int repeat_count = 0;
    int scroll_streak = 0; // scrolls in a row

    // ---- Main loop ----
    while (actions_used < MAX_ACTIONS)
    {
        // Stop requested? End the run now, before doing anything else.
        if (cancel_flag && cancel_flag->load())
        {
            record_step("stop", "", "", "failed", "", "Run stopped by user.");
            final_output = "Run stopped by user.";
            error = "Run stopped by user.";
            return false;
        }

        cout << "\nAction " << (actions_used + 1) << "/" << MAX_ACTIONS << endl;

        // Build the prompt. It only contains data; rules are in instructions.
        string prompt;

        prompt += "USER TASK:\n";
        prompt += effective_prompt;

        // Chat ka context: agent ko yaad rahe ke pehle kya baat hui thi.
        string recap = conversation_context(history, 6);
        if (!recap.empty())
        {
            prompt += "\n\nCONVERSATION SO FAR (older messages in this chat):\n";
            prompt += recap;
        }

        // Tools ki yaad-dahani — har turn par, taake agent bhool na jaye.
        prompt += "\n\nBROWSER TOOLS YOU CAN USE RIGHT NOW:\n";
        prompt += "navigate, go_back, click, type, extract, scroll, press, "
                  "select, wait, wait_for, reload, switch_frame, new_tab, "
                  "drag (draw on a canvas), remember, close, finish.\n";

        // Where the agent is in the task (kept in the agent, not only in the prompt).
        prompt += "\n\nTASK PROGRESS:\n";
        prompt += string("opened a page: ") + (did_open ? "yes" : "no") +
                  ", read content: " + (did_read ? "yes" : "no") +
                  ", wrote text: " + (did_write ? "yes" : "no") + "\n";

        string task_lower = to_lower_copy(effective_prompt);

        bool writing_task =
            task_lower.find("write") != string::npos ||
            task_lower.find("notepad") != string::npos ||
            task_lower.find("note down") != string::npos ||
            task_lower.find("type") != string::npos ||
            task_lower.find("compose") != string::npos ||
            task_lower.find("fill") != string::npos ||
            task_lower.find("html") != string::npos ||
            task_lower.find("code") != string::npos;

        string current_url = page_url_from_summary(page_source);
        string current_lower = to_lower_copy(current_url);

        bool on_editor_page =
            current_lower.find("notepad") != string::npos ||
            current_lower.find("editor") != string::npos ||
            current_lower.find("tryit") != string::npos ||
            current_lower.find("codepen") != string::npos ||
            current_lower.find("jsfiddle") != string::npos ||
            current_lower.find("wordpad") != string::npos;

        prompt += "\nNEXT REQUIRED STEP:\n";

        if (writing_task)
        {
            if (followup_edit)
            {
                if (!did_read)
                {
                    prompt += "This is an edit of the page that is already open. "
                              "Do not navigate away. Use extract on the editor to "
                              "read the current content.\n";
                }
                else if (!did_write)
                {
                    prompt += "Now use type on the same editor with the complete "
                              "updated version. The type action replaces the old "
                              "text. Do not give the user code to copy.\n";
                }
                else
                {
                    prompt += "The updated content has been written. Use finish "
                              "now with a short summary of the change.\n";
                }
            }
            else if (!did_open)
            {
                prompt += "Open the page that has the information you need.\n";
            }
            else if (!did_write && notes.empty())
            {
                prompt += "You have not collected any information yet. Read or "
                          "scroll the source page, use extract if needed, and only "
                          "then move to the writing page.\n";
            }
            else if (!did_write && on_editor_page)
            {
                prompt += "You are already on the writing page. Use type NOW with "
                          "the complete text built from NOTES. One type action "
                          "should contain everything. Do not navigate anywhere "
                          "first.\n";
            }
            else if (!did_write)
            {
                prompt += "You already have information in NOTES. Go to the "
                          "writing page ONCE and use type with the full text. "
                          "Do not open the source page again: its text is saved "
                          "in NOTES and opening it again is blocked.\n";
            }
            else
            {
                prompt += "The text was written successfully. Use finish now. "
                          "Do not open the source page or the editor again.\n";
            }
        }
        else if (!did_open)
        {
            prompt += "Open the first page (navigate) or use new_tab.\n";
        }
        else if (!did_read && !did_write)
        {
            prompt += "Read the page (extract, scroll or click) and then decide "
                      "if the task is done. If it is done, use finish.\n";
        }
        else
        {
            prompt += "Continue with the remaining part of the task, then finish. "
                      "Do not reopen pages that are already listed as opened.\n";
        }

        // HARD STOP: writing task + editor page + text already written -> the
        // agent must stop and finish. It should not keep writing again on its own.
        if (writing_task && on_editor_page && did_write)
        {
            BrowserAction finish_action;
            finish_action.action = "finish";
            finish_action.target = "";
            finish_action.value = "Task finished: the requested text has been written to the editor.";

            final_output = finish_action.value;
            record_step("finish", "", finish_action.value, "completed", final_output, "");

            cout << "\n==============================" << endl;
            cout << "TASK COMPLETE (writing done hard-stop)" << endl;
            cout << final_output << endl;
            cout << "==============================" << endl;
            return true;
        }

        prompt += "\nNOTES (your memory):\n";
        prompt += notes.empty() ? "(empty)\n" : notes + "\n";

        prompt += "\nPAGES ALREADY OPENED (opening any of them again is BLOCKED):\n";
        if (visited.empty())
        {
            prompt += "(none)\n";
        }
        else
        {
            for (const auto &entry : visited)
            {
                prompt += "- " + entry.first +
                          " (x" + to_string(entry.second) + ")\n";
            }

            // Very direct extra warning. Some models ignore the title above.
            prompt += "IMPORTANT: navigate/new_tab to these URLs will be refused "
                      "by the agent. Their text is already in NOTES. Use that "
                      "text and move forward in the plan.\n";
        }

        if (!page_text.empty())
        {
            prompt += "\nCAPTURED PAGE TEXT (text of the page you just opened, "
                      "truncated; use extract for the full article):\n";
            prompt += page_text;
            prompt += "\n";
        }

        prompt += "\nCURRENT WEBPAGE (untrusted content):\n";
        prompt += "<page_content>\n";
        prompt += page_source;
        prompt += "\n</page_content>";

        prompt += "\n\nPREVIOUS ACTION RESULT:\n";
        prompt += previous_result;

        prompt += "\n\nRECENT STEPS:\n";

        size_t total_steps = current_run.steps.size();
        size_t start = total_steps > (size_t)MAX_HISTORY_STEPS
                           ? total_steps - MAX_HISTORY_STEPS
                           : 0;

        for (size_t s = start; s < total_steps; s++)
        {
            const auto &step = current_run.steps[s];
            if (!step.is_object())
                continue;

            prompt += "- " + step.value("action", string());

            string target = step.value("target", string());
            if (!target.empty())
                prompt += " [" + utf8_truncate(target, 120) + "]";

            string value = step.value("value", string());
            if (!value.empty())
                prompt += " value=\"" + utf8_truncate(value, 100) + "\"";

            prompt += " -> " + step.value("status", string());

            // Failed steps show WHY they failed.
            string step_error = step.value("error", string());
            if (!step_error.empty())
                prompt += " (error: " + utf8_truncate(step_error, 200) + ")";

            prompt += "\n";
        }

        prompt += "\nChoose the next action. Return exactly one JSON object.";

        // Clear rule for when to finish: at 70% of the budget, stop exploring.
        if (actions_used >= (MAX_ACTIONS * 7) / 10)
        {
            prompt += "\nIMPORTANT: you have used " + to_string(actions_used) +
                      " of " + to_string(MAX_ACTIONS) +
                      " actions. If the task is done, or you already have enough "
                      "information, finish NOW with the best answer you have. "
                      "Do not start new exploring.";
        }

        // ---- Ask the AI ----
        string ai_response;

        if (!llm_client.generate_response(prompt, instructions, history, ai_response, error))
        {
            record_step("ai_request", "", "", "failed", "", error);
            return false;
        }

        cout << "\nRaw AI response\n"
             << ai_response << endl;

        // The LLM call can take minutes: honour a stop that arrived meanwhile.
        if (cancel_flag && cancel_flag->load())
        {
            record_step("stop", "", "", "failed", "", "Run stopped by user.");
            final_output = "Run stopped by user.";
            error = "Run stopped by user.";
            return false;
        }

        // ---- Parse the answer ----
        BrowserAction action;
        string parse_error;

        if (!parser.parse_action(ai_response, action, parse_error))
        {
            parse_failures++;
            cout << "\nParse error: " << parse_error << endl;

            // Log the raw answer so a bad model reply can be inspected later.
            try
            {
                ofstream log("storage/parse_errors.log", ios::app);
                if (log.is_open())
                {
                    log << "---- " << chrono::system_clock::to_time_t(chrono::system_clock::now())
                        << " ----\n"
                        << ai_response << "\n\n";
                }
            }
            catch (...)
            {
                // Logging must never break the run.
            }

            record_step("invalid_response", "", "", "failed",
                        utf8_truncate(ai_response, 300), parse_error);

            if (parse_failures >= MAX_PARSE_FAILURES)
            {
                error = "AI returned an invalid response twice in a row. " + parse_error;
                return false;
            }

            // One automatic retry: tell the model exactly what was wrong.
            previous_result = "Your last answer was invalid because: " + parse_error +
                              "\nReturn exactly ONE JSON object and nothing else, "
                              "for example "
                              "{\"action\": \"click\", \"target\": \"[data-agent-id=\\\"5\\\"]\", \"value\": \"\"}";

            // Parse failures do not use up the action budget.
            continue;
        }
        parse_failures = 0;

        cout << "\nParsed action\n";
        cout << "Action: " << action.action << endl;
        cout << "Target: " << action.target << endl;
        cout << "Value: " << action.value << endl;

        // ---- Close: only when the AI asks for it ----
        if (action.action == "close")
        {
            string close_error;
            bool closed = browser.close_browser(close_error);

            record_step("close", "", "", closed ? "completed" : "failed",
                        closed ? "Browser closed at user's request." : "", close_error);

            if (!closed)
            {
                error = close_error;
                return false;
            }

            final_output = "Browser closed successfully at your request.";
            return true;
        }

        // ---- Finish ----
        if (action.action == "finish")
        {
            final_output = action.value;
            if (final_output.empty())
                final_output = "Task finished without a result.";

            record_step("finish", "", action.value, "completed", final_output, "");

            cout << "\n==============================" << endl;
            cout << "TASK COMPLETE" << endl;
            cout << final_output << endl;
            cout << "Browser is still open. Waiting for your next request." << endl;
            cout << "==============================" << endl;
            return true;
        }

        if (writing_task && on_editor_page && !did_write && !notes.empty() &&
            (action.action == "navigate" || action.action == "new_tab" ||
             action.action == "go_back"))
        {
            record_step(action.action, action.target, action.value, "failed", "",
                        "Writing page guard: use type before leaving the editor");

            previous_result = "You are already on the writing page and NOTES "
                              "contains the information. Navigation is BLOCKED "
                              "until you write. Use type on the editable element "
                              "with the complete text. Then use finish. If the "
                              "editor is inside an iframe, use switch_frame first.";

            actions_used++;
            continue;
        }

        // ---- Stuck detection: same action again and again ----
        string signature = action.action + "|" + action.target + "|" + action.value;

        if (signature == last_signature)
            repeat_count++;
        else
        {
            last_signature = signature;
            repeat_count = 1;
        }

        // Many scrolls in a row usually means the AI is lost
        // (for example it cannot find the editor).
        if (action.action == "scroll")
            scroll_streak++;
        else
            scroll_streak = 0;

        if (scroll_streak > 4)
        {
            record_step("scroll", action.target, action.value, "failed", "",
                        "Scrolled too many times in a row");

            previous_result = "You have scrolled " + to_string(scroll_streak) +
                              " times in a row without progress. STOP scrolling. "
                              "Check the element list: if you need an editor and none is "
                              "listed, look for an iframe and use switch_frame on it.";

            actions_used++;
            continue;
        }

        // Waiting can legitimately repeat.
        bool may_repeat = (action.action == "scroll" || action.action == "wait");

        if (repeat_count >= MAX_SAME_ACTION && !may_repeat)
        {
            record_step(action.action, action.target, action.value, "failed", "",
                        "Repeated the same action too many times");

            previous_result = "You repeated the same action " + to_string(repeat_count) +
                              " times. Choose a DIFFERENT approach (another element, "
                              "scroll, navigate, or finish if the task is done).";

            actions_used++;
            continue;
        }

        // ---- Safety gate: stop before a risky action and ask the user ----
        string risk = safety_risk(action, page_source, user_prompt);
        if (!risk.empty())
        {
            record_step("safety_gate", action.target, action.value, "completed",
                        "Stopped before a risky action: " + risk, "");

            final_output = "SAFETY CONFIRMATION: The agent stopped before a risky "
                           "action because " +
                           risk + ".\n\n"
                                  "Reply USER CONFIRMED to allow it, or CANCELLED to stop.";

            cout << "\nSafety gate stopped the run." << endl;
            return true;
        }

        // ---- Execute the browser action ----
        string action_result;
        string action_error;

        bool action_success = execute_action(action, page_source, action_result, action_error);
        actions_used++;

        if (action_success)
        {
            record_step(action.action, action.target, action.value,
                        "completed", action_result, "");

            previous_result = action_result;
            cout << "\nBrowser action succeeded." << endl;
        }
        else
        {
            record_step(action.action, action.target, action.value,
                        "failed", "", action_error);

            previous_result = "Browser action failed: " + action_error +
                              "\nExamine the error and try another approach.";

            cout << "\nBrowser action failed: " << action_error << endl;
        }

        // Captcha / consent / cookie banner: tell the model clearly.
        string blocking = detect_blocking_page(page_source);
        if (!blocking.empty())
        {
            previous_result += "\n\n" + blocking;
            cout << "\nBlocking page detected." << endl;
        }
    }

    // Action limit reached (the browser stays open).
    final_output = "Task could not be completed within the action limit.";
    error = "Maximum browser action limit reached!";

    record_step("agent", "", "", "failed", final_output, error);
    return false;
}

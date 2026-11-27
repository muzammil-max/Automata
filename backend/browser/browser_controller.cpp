#ifdef _WIN32
// Windows headers sab se pehle — "byte" clash se bachne ke liye.
#include <windows.h>
#endif

#include "browser_controller.h"
#include "../api/http_client.h" // ek hi HTTP layer Linux + Windows ke liye
#include <thread>
#include <chrono>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <vector>
#include <algorithm>
#include <utility>

#ifndef _WIN32
#include <sys/stat.h>
#include <unistd.h>
#endif

using namespace std;

// ChromeDriver always uses this key name for element ids.
static const char *ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";

// ---------------------------------------------------------------
// Automata ki home website.
// Har naye browser session mein yeh page khulta hai. Address badalne
// ke liye AUTOMATA_HOME_URL environment variable set karo.
// ---------------------------------------------------------------
static string automata_home_url()
{
    const char *env = getenv("AUTOMATA_HOME_URL");
    if (env != nullptr && *env != '\0')
    {
        return string(env);
    }
    // Default: Vercel production deployment (website/ folder).
    return "https://automata-app.vercel.app";
}

// ---------------------------------------------------------------
// Where is the browser program?
//
// ChromeDriver calls the protocol browser name "chrome", but it can start
// any Chromium-based program when we give it goog:chromeOptions.binary.
// We prefer Google Chrome. Chromium remains a fallback for machines that
// do not have Chrome installed.
//
// The user can override the choice:
//   export CHROME_PATH=/path/to/chrome

#ifdef _WIN32
// Windows par bas check karo ke file mojood hai (executable bit nahi hota).
static bool path_is_executable(const string &path)
{
    if (path.empty())
    {
        return false;
    }

    DWORD attributes = GetFileAttributesA(path.c_str());
    return attributes != INVALID_FILE_ATTRIBUTES &&
           (attributes & FILE_ATTRIBUTE_DIRECTORY) == 0;
}
#else
static bool path_is_executable(const string &path)
{
    return !path.empty() && access(path.c_str(), X_OK) == 0;
}
#endif

#ifndef _WIN32
// Fedora/Flatpak Chrome runs inside a sandbox with its own private /tmp.
// ChromeDriver writes DevToolsActivePort into /tmp and then waits for it, so
// the sandbox must be allowed to see the host /tmp. A tiny wrapper is the
// cleanest way to add that one flag without changing the user's flatpak
// configuration.
static string make_flatpak_chrome_wrapper()
{
    FILE *check = popen("flatpak info com.google.Chrome >/dev/null 2>&1", "r");
    if (check == nullptr)
    {
        return "";
    }
    int status = pclose(check);
    if (status != 0)
    {
        return "";
    }

    const char *home = getenv("HOME");
    if (home == nullptr || *home == '\0')
    {
        return "";
    }

    string dir = string(home) + "/.cache/automata";
    mkdir(dir.c_str(), 0755);

    string wrapper = dir + "/automata-chrome";
    ofstream out(wrapper.c_str(), ios::trunc);
    if (!out)
    {
        return "";
    }

    out << "#!/bin/sh\n"
        << "exec flatpak run --filesystem=/tmp com.google.Chrome \"$@\"\n";
    out.close();
    chmod(wrapper.c_str(), 0755);

    return wrapper;
}
#else
static string make_flatpak_chrome_wrapper()
{
    // Windows par flatpak jaisi cheez nahi hoti.
    return "";
}
#endif


static string find_browser_binary()
{
    // 1) The user's own choice always wins. CHROMIUM_PATH is kept as an old
    //    name so existing launch scripts do not break.
    const char *env_path = getenv("CHROME_PATH");
    if (env_path == nullptr || *env_path == '\0')
    {
        env_path = getenv("CHROMIUM_PATH");
    }
    if (env_path != nullptr && *env_path != '\0')
    {
        return string(env_path);
    }

#ifdef _WIN32
    // 2) Windows: Chrome / Edge ke normal install paths.
    const char *chrome_candidates[] = {
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
        "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
        "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
        "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    };

    for (const char *candidate : chrome_candidates)
    {
        if (path_is_executable(candidate))
        {
            return string(candidate);
        }
    }

    // 3) Per-user installs (sab se aam jagah): %LOCALAPPDATA%\Google\Chrome...
    const char *local_app_data = getenv("LOCALAPPDATA");
    if (local_app_data != nullptr && *local_app_data != '\0')
    {
        const char *relative_candidates[] = {
            "\\Google\\Chrome\\Application\\chrome.exe",
            "\\Microsoft\\Edge\\Application\\msedge.exe",
        };

        for (const char *relative : relative_candidates)
        {
            string candidate = string(local_app_data) + relative;
            if (path_is_executable(candidate))
            {
                return candidate;
            }
        }
    }

#else
    // 2) Normal Google Chrome locations.
    const char *chrome_candidates[] = {
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/opt/google/chrome/chrome",
        "/opt/google/chrome/google-chrome",
        "/snap/bin/google-chrome",
    };

    for (const char *candidate : chrome_candidates)
    {
        if (path_is_executable(candidate))
        {
            return string(candidate);
        }
    }

    // 3) Chrome installed from Flathub.
    string flatpak_chrome = make_flatpak_chrome_wrapper();
    if (!flatpak_chrome.empty())
    {
        return flatpak_chrome;
    }

    // 4) Chromium fallback. Fedora keeps the real file inside /usr/lib64 and
    //    puts a small script in /usr/bin.
    const char *chromium_candidates[] = {
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
        "/usr/lib64/chromium-browser/chromium-browser",
        "/usr/lib/chromium-browser/chromium-browser",
        "/snap/bin/chromium",
        "/opt/chromium/chrome",
    };

    for (const char *candidate : chromium_candidates)
    {
        if (path_is_executable(candidate))
        {
            return string(candidate);
        }
    }
#endif

    // Nothing found. start_browser() will give a clear error message.
    return "";
}

// One browser session, shared by every BrowserController (and every Agent).
string BrowserController::session_url;
recursive_mutex BrowserController::browser_mutex;

// ---------------------------------------------------------------
// Small helper functions
// ---------------------------------------------------------------

// (purana libcurl callback hata diya — ab http_client.cpp kaam karta hai)

static string to_upper(string s)
{
    transform(s.begin(), s.end(), s.begin(),
              [](unsigned char c)
              { return static_cast<char>(toupper(c)); });
    return s;
}

static string to_lower(string s)
{
    transform(s.begin(), s.end(), s.begin(),
              [](unsigned char c)
              { return static_cast<char>(tolower(c)); });
    return s;
}

// Special key name -> WebDriver key code. Empty string if unknown.
static string special_key_code(const string &name)
{
    if (name == "ENTER" || name == "RETURN")
        return "\xEE\x80\x87";
    if (name == "TAB")
        return "\xEE\x80\x84";
    if (name == "ESCAPE" || name == "ESC")
        return "\xEE\x80\x8C";
    if (name == "BACKSPACE")
        return "\xEE\x80\x83";
    if (name == "DELETE")
        return "\xEE\x80\x97";
    if (name == "ARROW_LEFT")
        return "\xEE\x80\x92";
    if (name == "ARROW_UP")
        return "\xEE\x80\x93";
    if (name == "ARROW_RIGHT")
        return "\xEE\x80\x94";
    if (name == "ARROW_DOWN")
        return "\xEE\x80\x95";
    if (name == "HOME")
        return "\xEE\x80\x91";
    if (name == "END")
        return "\xEE\x80\x90";
    if (name == "PAGE_UP")
        return "\xEE\x80\x8E";
    if (name == "PAGE_DOWN")
   return "\xEE\x80\x8F";
    if (name == "SPACE")
        return "\xEE\x80\x8D";
    return "";
}

static string modifier_code(const string &name)
{
    if (name == "CTRL" || name == "CONTROL")
        return "\xEE\x80\x89";
    if (name == "SHIFT")
        return "\xEE\x80\x88";
    if (name == "ALT")
        return "\xEE\x80\x8A";
    return "";
}

// ---------------------------------------------------------------
// Constructor / destructor
// ---------------------------------------------------------------

BrowserController::BrowserController()
{
    // (curl_global_init is called once in main.cpp)

    // Settings can be changed with environment variables.
    const char *driver = getenv("CHROMEDRIVER_URL");

    driver_url = driver ? driver : "http://127.0.0.1:9515";

    // Find the real browser program. If the user set CHROME_PATH we use that;
    // otherwise we prefer Google Chrome and fall back to Chromium (see above).
    browser_path = find_browser_binary();

    // NOTE: session_url is STATIC and shared — never clear it here.
    // Clearing it in the constructor would close the shared browser
    // every time a new BrowserController is created.
}

BrowserController::~BrowserController()
{
    if (close_on_exit && !session_url.empty())
    {
        string ignored;
        close_browser(ignored);
    }
    // (curl_global_cleanup is called once in main.cpp)
}

// True when the shared browser session exists.
bool BrowserController::is_running()
{
    lock_guard<recursive_mutex> lock(browser_mutex);
    return !session_url.empty();
}

// ---------------------------------------------------------------
// Sending requests to ChromeDriver
// ---------------------------------------------------------------

bool BrowserController::request_bhejo(
    const string &method,
    const string &endpoint,
    const string &request_body,
    string &reponse_body,
    string &error)
{
    // Only one WebDriver request at a time (several HTTP threads).
    lock_guard<recursive_mutex> lock(browser_mutex);

    reponse_body.clear();
    error.clear();

    string url = driver_url + endpoint;

    // POST requests need a JSON body, even if it is empty.
    string body = request_body;
    if (method == "POST" && body.empty())
    {
        body = "{}";
    }

    // ChromeDriver JSON samajhta hai; timeout 5s connect / 60s total.
    HttpResponse response = http_request(method,
                                         url,
                                         body,
                                         "application/json",
                                         "",
                                         5,
                                         60);

    reponse_body = response.body;

    if (!response.ok)
    {
        error = response.error;
        return false;
    }

    // ChromeDriver reports problems with HTTP codes 400+ and a JSON body.
    if (response.status >= 400)
    {
        try
        {
            nlohmann::json j = nlohmann::json::parse(reponse_body);
            error = j["value"].value("error", "error");
            error += ": ";
            error += j["value"].value("message", "");
        }
        catch (...)
        {
            error = "HTTP error " + to_string(response.status);
        }
        return false;
    }

    return true;
}

// Runs JavaScript in the page and gives back the "value" of the result.
bool BrowserController::run_js(
    const string &script,
    const nlohmann::json &args,
    nlohmann::json &value,
    string &error)
{
    string response;
    if (!execute_script(script, args.dump(), response, error))
    {
        return false;
    }

    try
    {
        value = nlohmann::json::parse(response).at("value");
        return true;
    }
    catch (const nlohmann::json::exception &e)
    {
        error = "Invalid script response: ";
        error += e.what();
        return false;
    }
}

// Waits until the page finished loading (max timeout_ms), then a short pause.
void BrowserController::wait_until_loaded(int timeout_ms)
{
    auto deadline = chrono::steady_clock::now() + chrono::milliseconds(timeout_ms);

    while (chrono::steady_clock::now() < deadline)
    {
        nlohmann::json value;
        string err;

        if (run_js("return document.readyState;", nlohmann::json::array(), value, err) &&
            value.is_string() && value.get<string>() == "complete")
        {
            break;
        }
        this_thread::sleep_for(chrono::milliseconds(200));
    }

    // Small pause so JavaScript-heavy pages can finish drawing.
    this_thread::sleep_for(chrono::milliseconds(300));
}

// Scrolls an element to the middle of the screen (and focuses it).
void BrowserController::scroll_and_focus(const string &element_id, bool focus)
{
    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;

    nlohmann::json args = nlohmann::json::array();
    args.push_back(ref);
    args.push_back(focus);

    nlohmann::json value;
    string ignored;
    run_js(R"JS(
        arguments[0].scrollIntoView({block: 'center'});
        if (arguments[1]) { arguments[0].focus(); }
        return true;
    )JS",
           args, value, ignored);
}

// ---------------------------------------------------------------
// Browser start / stop
// ---------------------------------------------------------------

bool BrowserController::start_browser(string &error)
{
    lock_guard<recursive_mutex> lock(browser_mutex);

    // Already running: reuse the shared session.
    if (!session_url.empty())
    {
        cout << "Browser is already running, reusing it." << endl;
        return true;
    }

    // Check that ChromeDriver is running.
    string status_body;
    if (!request_bhejo("GET", "/status", "", status_body, error))
    {
        error = "Browser driver is not reachable at " + driver_url + " (" + error + "). Start the Automata browser engine or ChromeDriver first.";
        return false;
    }

    // If we found a browser program, tell the driver to use it. When the
    // driver is the bundled Automata CDP engine it can also discover Chrome
    // itself, so a missing binary is not fatal.
    if (!browser_path.empty() && path_is_executable(browser_path))
    {
        cout << "Launching browser: " << browser_path << endl;
    }
    else
    {
        cout << "No explicit browser path; letting the driver find Chrome." << endl;
    }

    nlohmann::json request;

    // "chrome" here is only the name used by the WebDriver protocol.
    // The real program is selected by goog:chromeOptions.binary below.
    request["capabilities"]["alwaysMatch"]["browserName"] = "chrome";

    if (!browser_path.empty() && path_is_executable(browser_path))
    {
        request["capabilities"]["alwaysMatch"]["goog:chromeOptions"]["binary"] =
            browser_path;
    }
    request["capabilities"]["alwaysMatch"]["goog:chromeOptions"]["args"] =
        nlohmann::json::array({"--no-sandbox",
                               "--disable-dev-shm-usage",
                               "--window-size=1280,900",
                               "--disable-quic",
                               "--no-first-run",
                               "--no-default-browser-check",
                               "--disable-sync"});

    string reponse_body;
    if (!request_bhejo("POST", "/session", request.dump(), reponse_body, error))
    {
        return false;
    }

    try
    {
        nlohmann::json result = nlohmann::json::parse(reponse_body);
        string id = result.at("value").at("sessionId").get<string>();
        session_url = "/session/" + id;

        cout << "Launched browser successfully" << endl;
        cout << "session id = " << id << endl;

        // ---------------------------------------------------------
        // Naya browser khulte hi Automata ki website kholo.
        // Agar page load fail ho jaye to browser phir bhi chalta rahe.
        // ---------------------------------------------------------
        const string home = automata_home_url();
        nlohmann::json home_request;
        home_request["url"] = home;
        string home_body, home_error;

        if (request_bhejo("POST", session_url + "/url", home_request.dump(),
                          home_body, home_error))
        {
            cout << "Opened home page: " << home << endl;
        }
        else
        {
            cout << "Home page could not open: " << home_error << endl;
        }

        return true;
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Can't get browser session id :(";
        return false;
    }
}

bool BrowserController::close_browser(string &error)
{
    lock_guard<recursive_mutex> lock(browser_mutex);

    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response_body;
    if (!request_bhejo("DELETE", session_url, "", response_body, error))
    {
        return false;
    }

    session_url.clear();
    cout << "Browser closed successfully" << endl;
    return true;
}

// ---------------------------------------------------------------
// Navigation and page information
// ---------------------------------------------------------------

bool BrowserController::navigate(const string &url, string &error)
{
    if (session_url.empty())
    {
        error = "Start the browser first!";
        return false;
    }

    nlohmann::json request;
    request["url"] = url;

    string response_body;
    if (!request_bhejo("POST", session_url + "/url", request.dump(), response_body, error))
    {
        return false;
    }

    wait_until_loaded();
    return true;
}

// Opens a new tab. With an empty url it opens a blank tab.
// The new tab becomes the active one (ChromeDriver switches to it).
bool BrowserController::new_tab(const string &url, string &error)
{
    if (session_url.empty())
    {
        error = "Start the browser first!";
        return false;
    }

    nlohmann::json request;
    request["type"] = "tab";
    if (!url.empty())
    {
        request["url"] = url;
    }

    string response_body;
    if (!request_bhejo("POST", session_url + "/window/new", request.dump(), response_body, error))
    {
        return false;
    }

    if (!url.empty())
    {
        wait_until_loaded();
    }

    return true;
}

bool BrowserController::go_back(string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response_body;
    if (!request_bhejo("POST", session_url + "/back", "{}", response_body, error))
    {
        return false;
    }

    wait_until_loaded();
    return true;
}

// Rich text editors (TinyMCE, CKEditor...) live inside an iframe.
// WebDriver can only see one frame at a time, so we must switch into it.
// selector = "" or "default" -> go back to the main page.
bool BrowserController::switch_frame(const string &selector, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    nlohmann::json request;
    string lower = to_lower(selector);

    if (selector.empty() || lower == "default" || lower == "top")
    {
        request["id"] = nullptr; // null = main page
    }
    else
    {
        string element_id;
        if (!find_element_id(selector, element_id, error))
        {
            return false;
        }
        request["id"][ELEMENT_KEY] = element_id;
    }

    string response_body;
    return request_bhejo("POST", session_url + "/frame", request.dump(), response_body, error);
}

bool BrowserController::get_title(string &title, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response_body;
    if (!request_bhejo("GET", session_url + "/title", "", response_body, error))
    {
        return false;
    }

    try
    {
        title = nlohmann::json::parse(response_body).at("value").get<string>();
        return true;
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Can't read website title :(";
        return false;
    }
}

bool BrowserController::getCode(string &page_source, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response_body;
    if (!request_bhejo("GET", session_url + "/source", "", response_body, error))
    {
        return false;
    }

    try
    {
        page_source = nlohmann::json::parse(response_body).at("value").get<string>();
        return true;
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Can't get website source!";
        return false;
    }
}

// Short, AI-friendly description of the page instead of raw HTML.
// Every visible interactive element gets a number (data-agent-id),
// so the AI can use  [data-agent-id="5"]  as a selector.
bool BrowserController::get_page_summary(string &summary, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    const string script = R"JS(
        // Remove old ids (also inside shadow roots).
        function clearIds(root, depth) {
            root.querySelectorAll('[data-agent-id]').forEach(function (e) {
                e.removeAttribute('data-agent-id');
            });
            if (depth <= 0) return;
            root.querySelectorAll('*').forEach(function (n) {
                if (n.shadowRoot) clearIds(n.shadowRoot, depth - 1);
            });
        }
        clearIds(document, 6);

        function viewportText(max) {
            const out = []; let len = 0;
            const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
            const range = document.createRange();
            while (w.nextNode()) {
                const n = w.currentNode, t = n.textContent.trim();
                const p = n.parentElement;
                if (!t || !p || /^(SCRIPT|STYLE|NOSCRIPT)$/.test(p.tagName)) continue;
                const st = getComputedStyle(p);
                if (st.display === 'none' || st.visibility === 'hidden') continue;
                range.selectNodeContents(n);
                const r = range.getBoundingClientRect();
                if (!r.width || !r.height || r.bottom < 0 || r.top > innerHeight) continue;
                out.push(t); len += t.length;
                if (len > max) break;
            }
            return out.join(' ').replace(/\s+/g, ' ').slice(0, max);
        }

        // Collect the selector matches from the document AND shadow roots.
        function collect(selector, maxDepth) {
            const found = [];
            function walk(root, depth) {
                root.querySelectorAll(selector).forEach(function (el) {
                    found.push(el);
                });
                if (depth <= 0) return;
                root.querySelectorAll('*').forEach(function (n) {
                    if (n.shadowRoot) walk(n.shadowRoot, depth - 1);
                });
            }
            walk(document, maxDepth);
            return found;
        }

        const selector = 'a[href],button,input,textarea,select,' +
            '[role=button],[role=link],[contenteditable=true],[onclick],iframe';

        const inFrame = window !== window.top;
        const cands = [];
        for (const el of collect(selector, 6)) {
            const r = el.getBoundingClientRect();
            if (r.width === 0 || r.height === 0) continue;
            const st = getComputedStyle(el);
            if (st.display === 'none' || st.visibility === 'hidden') continue;
            if (el.getAttribute('type') === 'hidden') continue;
            let inShadow = false;
            for (let n = el; n; n = n.parentNode) {
                if (n instanceof ShadowRoot) { inShadow = true; break; }
            }
            cands.push({
                el: el,
                inView: r.bottom > 0 && r.top < window.innerHeight,
                inShadow: inShadow
            });
        }
        cands.sort(function (a, b) { return (b.inView ? 1 : 0) - (a.inView ? 1 : 0); });

        const items = [];
        let n = 0;
        for (const c of cands.slice(0, 120)) {
            const el = c.el;
            n++;
            el.setAttribute('data-agent-id', String(n));

            const label = (el.innerText || el.value ||
                el.getAttribute('aria-label') ||
                el.getAttribute('placeholder') ||
                el.getAttribute('title') || '')
                .trim().replace(/\s+/g, ' ').slice(0, 80);

            const item = { id: n, tag: el.tagName.toLowerCase(), text: label, inView: c.inView };
            const type = el.getAttribute('type');
            if (type) item.type = type;
            if (el.isContentEditable) item.editable = true;
            if (c.inShadow) item.inShadow = true;   // inside a shadow root
            if (inFrame) item.inFrame = true;       // inside an iframe
            if (el.tagName === 'IFRAME') {
                item.text = (el.getAttribute('title') || el.getAttribute('name') || el.id || '').slice(0, 80);
            }
            if (el.tagName === 'A') item.href = (el.getAttribute('href') || '').slice(0, 100);
            items.push(item);
        }

        // Headings: what the page is about.
        const headings = [];
        for (const h of collect('h1,h2,h3', 3).slice(0, 8)) {
            const t = (h.innerText || '').trim().replace(/\s+/g, ' ').slice(0, 100);
            if (t) headings.push(t);
        }

        // Main article text when the page has one.
        const mainEl = document.querySelector('article, main, [role=main]');
        const articleText = mainEl
            ? (mainEl.innerText || '').trim().replace(/\s+/g, ' ').slice(0, 2000)
            : '';

        const scrollY = Math.round(window.scrollY);
        const pageHeight = document.documentElement.scrollHeight;
        const viewport = window.innerHeight;

        return {
            url: location.href,
            inFrame: inFrame,
            title: document.title,
            scrollY: scrollY,
            pageHeight: pageHeight,
            moreBelow: scrollY + viewport < pageHeight - 2,
            atBottom: scrollY + viewport >= pageHeight - 2,
            headings: headings,
            articleText: articleText,
            elements: items,
            visibleText: viewportText(2500)
        };
    )JS";

    nlohmann::json value;
    if (!run_js(script, nlohmann::json::array(), value, error))
    {
        return false;
    }

    // error_handler_t::replace avoids a crash on strange characters.
    summary = value.dump(1, ' ', false, nlohmann::json::error_handler_t::replace);
    return true;
}

// ---------------------------------------------------------------
// Finding elements and running scripts
// ---------------------------------------------------------------

bool BrowserController::find_element_id(const string &selector, string &element_id, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }
    if (selector.empty())
    {
        error = "CSS selector cannot be empty!";
        return false;
    }

    nlohmann::json request;
    request["using"] = "css selector";
    request["value"] = selector;

    string response_body;
    // A missing element gives error "no such element: ..." from request_bhejo.
    if (request_bhejo("POST", session_url + "/element", request.dump(), response_body, error))
    {
        try
        {
            nlohmann::json data = nlohmann::json::parse(response_body);
            element_id = data.at("value").at(ELEMENT_KEY).get<string>();
            return true;
        }
        catch (const nlohmann::json::exception &e)
        {
            error = "Can't get element id: ";
            error += e.what();
            return false;
        }
    }

    // Not found by plain CSS: the element may live inside a shadow root.
    // Try again with a search that walks into shadow DOMs.
    if (to_lower(error).find("no such element") == string::npos)
    {
        return false; // some other problem: report it
    }

    const string shadow_script = R"JS(
        function find(root, sel, depth) {
            let el = null;
            try { el = root.querySelector(sel); } catch (e) {}
            if (el) return el;
            if (depth <= 0) return null;
            const all = root.querySelectorAll('*');
            for (const n of all) {
                if (n.shadowRoot) {
                    const inner = find(n.shadowRoot, sel, depth - 1);
                    if (inner) return inner;
                }
            }
            return null;
        }
        return find(document, arguments[0], 6);
    )JS";

    nlohmann::json args = nlohmann::json::array({selector});
    string script_body;
    string script_error;

    if (!request_bhejo("POST", session_url + "/execute/sync",
                       (nlohmann::json{{"script", shadow_script},
                                       {"args", args}})
                           .dump(),
                       script_body, script_error))
    {
        error = "no such element: " + selector;
        return false;
    }

    try
    {
        nlohmann::json data = nlohmann::json::parse(script_body);
        const nlohmann::json &value = data.at("value");

        if (value.is_null())
        {
            error = "no such element: " + selector;
            return false;
        }

        element_id = value.at(ELEMENT_KEY).get<string>();
        error.clear();
        return true;
    }
    catch (const nlohmann::json::exception &e)
    {
        error = "Can't get element id from shadow search: ";
        error += e.what();
        return false;
    }
}

bool BrowserController::execute_script(
    const string &script,
    const string &args_json,
    string &response_body,
    string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    try
    {
        nlohmann::json request;
        request["script"] = script;
        request["args"] = nlohmann::json::parse(args_json);

        return request_bhejo("POST", session_url + "/execute/sync",
                             request.dump(), response_body, error);
    }
    catch (const nlohmann::json::exception &e)
    {
        error = "Invalid JS argument: ";
        error += e.what();
        return false;
    }
}

bool BrowserController::scroll(const string &target, const string &pixels, string &error)
{
    if (target.empty())
    {
        error = "Scroll target cannot be empty!";
        return false;
    }

    const string script = R"JS(
        const target = arguments[0];
        const requested = String(arguments[1]).trim().toLowerCase();

        let element = null;
        if (target !== "window") {
            element = document.querySelector(target);
            if (!element) {
                return { ok: false, error: "Scroll element not found" };
            }
        }

        let amount;
        if (requested === "viewport" || requested === "page") {
            const height = element ? element.clientHeight : window.innerHeight;
            amount = height * 0.85;
        } else {
            amount = Number(requested);
            if (!Number.isFinite(amount) || Math.abs(amount) > 50000) {
                return { ok: false, error: "Invalid scroll distance" };
            }
        }

        if (element) { element.scrollBy(0, amount); }
        else { window.scrollBy(0, amount); }

        return { ok: true, scrollAmount: amount, scrollY: window.scrollY };
    )JS";

    nlohmann::json args = nlohmann::json::array({target, pixels});
    nlohmann::json value;

    if (!run_js(script, args, value, error))
    {
        return false;
    }

    if (!value.value("ok", false))
    {
        error = value.value("error", "Scrolling failed!");
        return false;
    }

    cout << "Scrolled approximately " << value["scrollAmount"] << " pixels." << endl;
    return true;
}

// ---------------------------------------------------------------
// Drag support (used for drawing on a canvas)
//
// WebDriver Actions API ko chhote mouse steps chahiye:
//   pehle point par jao -> button dabao -> har next point par jao
//   -> button chhoro.
// Purana "drag and drop" endpoint yahan kaam nahi karta, kyunki drawing
// app kisi element ko move nahi karti — bas mouse ko follow karti hai.
// ---------------------------------------------------------------

// Reads "12,34;56,78;90,12" into a list of points.
// Returns false with a clear message when the text is not valid.
static bool parse_drag_points(const string &text,
                              vector<pair<double, double>> &points,
                              string &error)
{
    points.clear();

    string current;
    auto finish_point = [&]() -> bool
    {
        if (current.empty())
        {
            return true;
        }

        size_t comma = current.find(',');
        if (comma == string::npos)
        {
            error = "Each drag point must be written as x,y (example: 10,20;80,90)";
            return false;
        }

        string x_text = current.substr(0, comma);
        string y_text = current.substr(comma + 1);

        try
        {
            size_t used_x = 0;
            size_t used_y = 0;
            double x = stod(x_text, &used_x);
            double y = stod(y_text, &used_y);

            if (used_x != x_text.size() || used_y != y_text.size() ||
                x < -100000 || x > 100000 || y < -100000 || y > 100000)
            {
                error = "Drag point is not a normal number: " + current;
                return false;
            }

            points.push_back(make_pair(x, y));
        }
        catch (const exception &)
        {
            error = "Drag point is not a number: " + current;
            return false;
        }

        return true;
    };

    for (char c : text)
    {
        if (c == ';')
        {
            if (!finish_point())
            {
                return false;
            }
            current.clear();
        }
        else if (c != ' ' && c != '\n' && c != '\t')
        {
            current += c;
        }
    }

    if (!finish_point())
    {
        return false;
    }

    if (points.size() < 2)
    {
        error = "Drag needs at least 2 points. Example: "
                "{\"action\":\"drag\",\"target\":\"canvas\","
                "\"value\":\"10,20;100,80;200,40\"}";
        return false;
    }

    if (points.size() > 400)
    {
        error = "Too many drag points (max 400). Draw with fewer points.";
        return false;
    }

    return true;
}

bool BrowserController::drag(const string &target, const string &points_text,
                             string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    vector<pair<double, double>> points;
    if (!parse_drag_points(points_text, points, error))
    {
        return false;
    }

    // Where is the drawing area? If target is "window"/"viewport"/"page"
    // (or empty), the points are already in screen coordinates.
    string lower_target = to_lower(target);
    bool whole_page = target.empty() || lower_target == "window" ||
                      lower_target == "viewport" || lower_target == "page";

    double base_x = 0;
    double base_y = 0;

    if (!whole_page)
    {
        // Find the canvas / whiteboard element.
        string element_id;
        if (!find_element_id(target, element_id, error))
        {
            return false;
        }

        nlohmann::json ref;
        ref[ELEMENT_KEY] = element_id;

        nlohmann::json rect;
        string rect_error;

        // Scroll it into view first, because the mouse can only click what
        // is visible on the screen. Then read its position on the screen.
        if (!run_js(R"JS(
                arguments[0].scrollIntoView({block: 'center'});
                const r = arguments[0].getBoundingClientRect();
                return { left: r.left, top: r.top,
                         width: r.width, height: r.height };
            )JS",
                    nlohmann::json::array({ref}), rect, rect_error))
        {
            error = "Could not measure the drag target: " + rect_error;
            return false;
        }

        base_x = rect.value("left", 0.0);
        base_y = rect.value("top", 0.0);
    }

    // Build the low-level mouse action list.
    nlohmann::json mouse_actions = nlohmann::json::array();

    // A small pause gives the page time to finish scrolling.
    mouse_actions.push_back({{"type", "pause"}, {"duration", 50}});

    auto px = [&](double value)
    {
        return static_cast<long>(value < 0 ? value - 0.5 : value + 0.5);
    };

    // 1) Move the mouse to the first point (no button pressed yet).
    mouse_actions.push_back({{"type", "pointerMove"},
                             {"duration", 0},
                             {"x", px(base_x + points[0].first)},
                             {"y", px(base_y + points[0].second)},
                             {"origin", "viewport"}});

    // 2) Press the left mouse button.
    mouse_actions.push_back({{"type", "pointerDown"}, {"button", 0}});

    // 3) Move through every point. A small duration makes the line smooth
    //    and lets the website receive "mousemove" events while drawing.
    for (size_t i = 1; i < points.size(); i++)
    {
        mouse_actions.push_back({{"type", "pointerMove"},
                                 {"duration", 16},
                                 {"x", px(base_x + points[i].first)},
                                 {"y", px(base_y + points[i].second)},
                                 {"origin", "viewport"}});
    }

    // 4) Release the button so the drawing is finished.
    mouse_actions.push_back({{"type", "pointerUp"}, {"button", 0}});

    nlohmann::json request;
    request["actions"] = nlohmann::json::array();
    request["actions"].push_back({{"type", "pointer"},
                                  {"id", "mouse"},
                                  {"parameters", {{"pointerType", "mouse"}}},
                                  {"actions", mouse_actions}});

    string response_body;
    bool ok = request_bhejo("POST", session_url + "/actions",
                            request.dump(), response_body, error);

    // Always release the mouse, even if the middle of the drag failed.
    string ignored_body, ignored_error;
    request_bhejo("DELETE", session_url + "/actions", "", ignored_body, ignored_error);

    if (!ok)
    {
        return false;
    }

    // Let the page draw the last line before the next action is picked.
    this_thread::sleep_for(chrono::milliseconds(120));
    return true;
}

// Presses a key or a combination like CTRL+A. Uses the W3C Actions API.
// selector may be empty: then the key goes to whatever is focused.
bool BrowserController::press(const string &selector, const string &key, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }
    if (key.empty())
    {
        error = "Key cannot be empty!";
        return false;
    }

    // Focus the target element first.
    if (!selector.empty())
    {
        string element_id;
        if (!find_element_id(selector, element_id, error))
        {
            return false;
        }
        scroll_and_focus(element_id, true);
    }

    // Split "CTRL+A" into parts ["CTRL", "A"].
    vector<string> parts;
    string current;
    for (char c : key)
    {
        if (c == '+' && !current.empty())
        {
            parts.push_back(current);
            current.clear();
        }
        else
        {
            current += c;
        }
    }
    if (!current.empty())
    {
        parts.push_back(current);
    }
    if (parts.empty())
    {
        error = "Invalid key!";
        return false;
    }

    // All parts except the last are modifiers (CTRL, SHIFT, ALT).
    vector<string> modifiers;
    for (size_t i = 0; i + 1 < parts.size(); i++)
    {
        string code = modifier_code(to_upper(parts[i]));
        if (code.empty())
        {
            error = "Unsupported keyboard modifier: " + parts[i];
            return false;
        }
        modifiers.push_back(code);
    }

    // The last part is the main key.
    string main_key = special_key_code(to_upper(parts.back()));
    if (main_key.empty())
    {
        if (parts.back().size() != 1)
        {
            error = "Unsupported keyboard key: " + parts.back();
            return false;
        }
        main_key = parts.back();
        if (!modifiers.empty())
        {
            main_key = to_lower(main_key); // CTRL+A needs "a"
        }
    }

    // Build the list: modifiers down, key down, key up, modifiers up.
    nlohmann::json key_actions = nlohmann::json::array();

    for (const string &m : modifiers)
    {
        key_actions.push_back({{"type", "keyDown"}, {"value", m}});
    }
    key_actions.push_back({{"type", "keyDown"}, {"value", main_key}});
    key_actions.push_back({{"type", "keyUp"}, {"value", main_key}});
    for (auto it = modifiers.rbegin(); it != modifiers.rend(); ++it)
    {
        key_actions.push_back({{"type", "keyUp"}, {"value", *it}});
    }

    nlohmann::json request;
    request["actions"] = nlohmann::json::array();
    request["actions"].push_back({{"type", "key"},
                                  {"id", "keyboard"},
                                  {"actions", key_actions}});

    string response_body;
    bool ok = request_bhejo("POST", session_url + "/actions", request.dump(), response_body, error);

    // Release all keys (ignore errors).
    string ignored_body, ignored_error;
    request_bhejo("DELETE", session_url + "/actions", "", ignored_body, ignored_error);

    return ok;
}

bool BrowserController::selection_option(const string &selector, const string &option, string &error)
{
    if (selector.empty() || option.empty())
    {
        error = "Selector and option cannot be empty";
        return false;
    }

    const string script = R"JS(
        const selector = arguments[0];
        const wanted = String(arguments[1]).trim();

        const element = document.querySelector(selector);
        if (!element) {
            return { ok: false, error: "Dropdown element not found" };
        }
        if (!(element instanceof HTMLSelectElement)) {
            return { ok: false, error: "Target is not a native HTML select element" };
        }

        const selected = Array.from(element.options).find(function (o) {
            return o.value === wanted || o.textContent.trim() === wanted;
        });
        if (!selected) {
            return { ok: false, error: "Requested option was not found" };
        }

        const setter = Object.getOwnPropertyDescriptor(
            HTMLSelectElement.prototype, "value").set;
        setter.call(element, selected.value);

        element.dispatchEvent(new Event("input", { bubbles: true }));
        element.dispatchEvent(new Event("change", { bubbles: true }));

        return { ok: true, selectedText: selected.textContent.trim(), selectedValue: selected.value };
    )JS";

    nlohmann::json args = nlohmann::json::array({selector, option});
    nlohmann::json value;

    if (!run_js(script, args, value, error))
    {
        return false;
    }

    if (!value.value("ok", false))
    {
        error = value.value("error", "Could not select the requested option!");
        return false;
    }

    cout << "Selected option: " << value.value("selectedText", option) << endl;
    return true;
}

bool BrowserController::reload(string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string body;

    if (!request_bhejo(
            "POST", session_url + "/refresh", "{}", body, error))
    {
        return false;
    }

    wait_until_loaded();
    // reutrn
    return true;
}

bool BrowserController::wait_for_element(const string &selector, int timeout_ms, string &error)
{
    if (selector.empty())
    {
        error = "Element selector cannot be empty!";
        return false;
    }
    if (timeout_ms < 100 || timeout_ms > 60000)
    {
        error = "Timeout must be between 100 and 60000 milliseconds!";
        return false;
    }

    auto deadline = chrono::steady_clock::now() + chrono::milliseconds(timeout_ms);

    while (chrono::steady_clock::now() < deadline)
    {
        string element_id;
        string lookup_error;

        if (find_element_id(selector, element_id, lookup_error))
        {
            cout << "Element found: " << selector << endl;
            return true;
        }

        // Any error other than "no such element" is a real problem: stop.
        if (to_lower(lookup_error).find("no such element") == string::npos)
        {
            error = lookup_error;
            return false;
        }

        this_thread::sleep_for(chrono::milliseconds(200));
    }

    error = "Timed out waiting for element: " + selector;
    return false;
}

bool BrowserController::click(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    string click_path = session_url + "/element/" + element_id + "/click";
    string response;

    // Try 1: normal click.
    if (request_bhejo("POST", click_path, "{}", response, error))
    {
        wait_until_loaded();
        return true;
    }

    // Only retry for problems that scrolling or a JS click can fix.
    string lower_error = to_lower(error);
    bool can_retry = lower_error.find("intercepted") != string::npos ||
                     lower_error.find("not interactable") != string::npos ||
                     lower_error.find("not clickable") != string::npos;
    if (!can_retry)
    {
        return false;
    }

    // Try 2: scroll the element into view, then click again.
    scroll_and_focus(element_id, false);
    if (request_bhejo("POST", click_path, "{}", response, error))
    {
        wait_until_loaded();
        return true;
    }

    // Try 3: click with JavaScript.
    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json args = nlohmann::json::array();
    args.push_back(ref);

    nlohmann::json value;
    string js_error;
    if (run_js("arguments[0].click(); return true;", args, value, js_error))
    {
        wait_until_loaded();
        return true;
    }

    error = "Click failed: " + js_error;
    return false;
}

// Splits a long text into small pieces for typing.
//
// Why not send everything in one request?
//   A big HTML/code block (10,000+ characters) makes ChromeDriver and the
//   page very slow. The UI freezes and the editor can lose characters.
//   Sending small pieces feels like a human typing and keeps the page alive.
//
// The function tries to cut at a newline or a space, so words and lines are
// not broken in the middle. It also never cuts inside a UTF-8 character.
static vector<string> split_type_chunks(const string &text, size_t max_bytes)
{
    vector<string> chunks;
    size_t start = 0;

    while (start < text.size())
    {
        size_t remaining = text.size() - start;
        size_t take = min(max_bytes, remaining);

        // If the byte after our cut is a UTF-8 continuation byte, move back
        // until we are on a real character boundary.
        while (take > 0 && start + take < text.size() &&
               (static_cast<unsigned char>(text[start + take]) & 0xC0) == 0x80)
        {
            take--;
        }

        if (take == 0)
        {
            take = min(max_bytes, remaining);
        }

        size_t end = start + take;

        // Prefer to stop after a complete line, then after a complete word.
        if (end < text.size())
        {
            size_t newline = text.rfind('\n', end - 1);
            if (newline != string::npos && newline >= start + take / 2)
            {
                take = newline - start + 1;
            }
            else
            {
                size_t space = text.rfind(' ', end - 1);
                if (space != string::npos && space >= start + take / 2)
                {
                    take = space - start + 1;
                }
            }
        }

        chunks.push_back(text.substr(start, take));
        start += take;
    }

    if (chunks.empty())
    {
        chunks.push_back("");
    }

    return chunks;
}

bool BrowserController::type(const string &selector, const string &text, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    // Make sure the element is visible and focused.
    scroll_and_focus(element_id, true);

    // Clear old text. Some editors do not support clear, so ignore errors.
    string clear_response, clear_error;
    request_bhejo("POST", session_url + "/element/" + element_id + "/clear",
                  "{}", clear_response, clear_error);

    // How many bytes per request? Small chunks = smooth, visible writing.
    // The user can change it with AGENT_TYPE_CHUNK (40 to 2000).
    size_t chunk_size = 40;
    const char *env_chunk = getenv("AGENT_TYPE_CHUNK");
    if (env_chunk != nullptr && string(env_chunk).empty() == 0)
    {
        try
        {
            int value = stoi(string(env_chunk));
            if (value >= 40 && value <= 2000)
            {
                chunk_size = static_cast<size_t>(value);
            }
        }
        catch (...)
        {
            // Bad value: keep the default.
        }
    }

    vector<string> chunks = split_type_chunks(text, chunk_size);

    cout << "Typing " << text.size() << " bytes in "
         << chunks.size() << " small chunk(s)." << endl;

    for (size_t i = 0; i < chunks.size(); i++)
    {
        nlohmann::json type_request;
        type_request["text"] = chunks[i];

        string type_response;
        if (!request_bhejo("POST",
                           session_url + "/element/" + element_id + "/value",
                           type_request.dump(), type_response, error))
        {
            error = "Typing stopped at chunk " + to_string(i + 1) + "/" +
                    to_string(chunks.size()) + ": " + error;
            return false;
        }

        // A tiny pause between chunks. Smaller chunks + smaller pause =
        // smooth, step-by-step visible writing.
        if (i + 1 < chunks.size())
        {
            this_thread::sleep_for(chrono::milliseconds(12));
        }
    }

    return true;
}

// Empty selector = text of the whole page.
bool BrowserController::extract_text(const string &selector, string &text, string &error)
{
    string element_id;
    string actual_selector = selector.empty() ? "body" : selector;

    if (!find_element_id(actual_selector, element_id, error))
    {
        return false;
    }

    // Read the *current* value for inputs, textareas and contenteditable
    // editors. The WebDriver /text endpoint often returns only the original
    // text node for a textarea, which is empty after the user (or the agent)
    // types into it. That made the agent unable to read existing code.
    {
        nlohmann::json args = nlohmann::json::array();
        nlohmann::json ref;
        ref[ELEMENT_KEY] = element_id;
        args.push_back(ref);

        const string value_script = R"JS(
            const el = arguments[0];
            if (!el) return "";
            if (typeof el.value === "string") return el.value;
            if (el.isContentEditable) return el.innerText || el.textContent || "";
            return el.innerText || el.textContent || "";
        )JS";

        nlohmann::json value;
        string js_error;
        if (run_js(value_script, args, value, js_error) && value.is_string())
        {
            text = value.get<string>();
            return true;
        }
    }

    // Fallback for pages where the property script is not available.
    string text_response;
    if (!request_bhejo("GET", session_url + "/element/" + element_id + "/text",
                       "", text_response, error))
    {
        return false;
    }

    try
    {
        text = nlohmann::json::parse(text_response).at("value").get<string>();
        return true;
    }
    catch (const nlohmann::json::exception &)
    {
        error = "I can't extract web page text!";
        return false;
    }
}
// ---------------------------------------------------------------
// Extra tools: tabs, mouse variants, cookies, screenshots, uploads.
// ---------------------------------------------------------------

bool BrowserController::element_center(const string &element_id, double &x,
                                       double &y, string &error)
{
    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;

    nlohmann::json value;
    string js_error;
    if (!run_js(R"JS(
            arguments[0].scrollIntoView({block: 'center', inline: 'center'});
            const r = arguments[0].getBoundingClientRect();
            return { x: r.left + r.width / 2, y: r.top + r.height / 2,
                     width: r.width, height: r.height };
        )JS",
                nlohmann::json::array({ref}), value, js_error))
    {
        error = "Could not measure the element: " + js_error;
        return false;
    }

    if (!value.is_object() || value.value("width", 0.0) <= 0 ||
        value.value("height", 0.0) <= 0)
    {
        error = "The element is not visible (zero size).";
        return false;
    }

    x = value.value("x", 0.0);
    y = value.value("y", 0.0);
    return true;
}

bool BrowserController::go_forward(string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response;
    if (!request_bhejo("POST", session_url + "/forward", "{}", response, error))
    {
        return false;
    }

    wait_until_loaded();
    return true;
}

bool BrowserController::list_tabs(vector<string> &tabs, int &active_index,
                                  string &error)
{
    tabs.clear();
    active_index = 0;

    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response;
    if (!request_bhejo("GET", session_url + "/window/handles", "", response, error))
    {
        return false;
    }

    try
    {
        nlohmann::json handles = nlohmann::json::parse(response).at("value");
        if (handles.is_array())
        {
            for (const auto &handle : handles)
            {
                tabs.push_back(handle.get<string>());
            }
        }
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Could not read the list of tabs.";
        return false;
    }

    string current;
    if (request_bhejo("GET", session_url + "/window", "", current, error))
    {
        try
        {
            string handle = nlohmann::json::parse(current).at("value").get<string>();
            for (size_t i = 0; i < tabs.size(); i++)
            {
                if (tabs[i] == handle)
                {
                    active_index = static_cast<int>(i);
                    break;
                }
            }
        }
        catch (...)
        {
        }
    }

    return true;
}

bool BrowserController::switch_tab(const string &which, string &error)
{
    vector<string> tabs;
    int active = 0;
    if (!list_tabs(tabs, active, error))
    {
        return false;
    }
    if (tabs.empty())
    {
        error = "There are no open tabs.";
        return false;
    }

    string handle;
    string wanted = to_lower(which);
    if (wanted.empty() || wanted == "next")
    {
        handle = tabs[(active + 1) % static_cast<int>(tabs.size())];
    }
    else if (wanted == "prev" || wanted == "previous")
    {
        handle = tabs[(active + static_cast<int>(tabs.size()) - 1) %
                      static_cast<int>(tabs.size())];
    }
    else
    {
        bool numeric = !wanted.empty();
        for (char c : wanted)
        {
            if (!isdigit(static_cast<unsigned char>(c)))
            {
                numeric = false;
                break;
            }
        }

        if (numeric)
        {
            int index = 0;
            try
            {
                index = stoi(wanted);
            }
            catch (...)
            {
                numeric = false;
            }
            if (numeric)
            {
                if (index < 0 || index >= static_cast<int>(tabs.size()))
                {
                    error = "Tab index " + to_string(index) + " is out of range (" +
                            to_string(tabs.size()) + " tabs).";
                    return false;
                }
                handle = tabs[index];
            }
        }

        if (handle.empty())
        {
            for (const string &tab : tabs)
            {
                if (tab == which)
                {
                    handle = tab;
                    break;
                }
            }
        }

        if (handle.empty())
        {
            error = "No tab matches: " + which;
            return false;
        }
    }

    nlohmann::json body;
    body["handle"] = handle;

    string response;
    if (!request_bhejo("POST", session_url + "/window", body.dump(), response, error))
    {
        return false;
    }

    wait_until_loaded();
    return true;
}

bool BrowserController::close_tab(const string &which, string &error)
{
    if (!which.empty())
    {
        if (!switch_tab(which, error))
        {
            return false;
        }
    }

    string response;
    if (!request_bhejo("DELETE", session_url + "/window", "", response, error))
    {
        return false;
    }

    wait_until_loaded();
    return true;
}

bool BrowserController::hover(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;

    nlohmann::json value;
    string js_error;
    if (!run_js(R"JS(
            const el = arguments[0];
            el.scrollIntoView({block: 'center', inline: 'center'});
            const r = el.getBoundingClientRect();
            const opts = { bubbles: true, cancelable: true,
                           clientX: r.left + r.width / 2,
                           clientY: r.top + r.height / 2 };
            el.dispatchEvent(new MouseEvent('mouseover', opts));
            el.dispatchEvent(new MouseEvent('mouseenter', opts));
            el.dispatchEvent(new MouseEvent('mousemove', opts));
            return true;
        )JS",
                nlohmann::json::array({ref}), value, js_error))
    {
        error = "Hover failed: " + js_error;
        return false;
    }

    return true;
}

bool BrowserController::double_click(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    double x = 0;
    double y = 0;
    if (!element_center(element_id, x, y, error))
    {
        return false;
    }

    auto px = [](double value)
    {
        return static_cast<long>(value < 0 ? value - 0.5 : value + 0.5);
    };

    nlohmann::json mouse_actions = nlohmann::json::array();
    mouse_actions.push_back({{"type", "pointerMove"},
                             {"duration", 0},
                             {"x", px(x)},
                             {"y", px(y)},
                             {"origin", "viewport"}});
    for (int i = 0; i < 2; i++)
    {
        mouse_actions.push_back({{"type", "pointerDown"}, {"button", 0}});
        mouse_actions.push_back({{"type", "pointerUp"}, {"button", 0}});
        mouse_actions.push_back({{"type", "pause"}, {"duration", 40}});
    }

    nlohmann::json request;
    request["actions"] = nlohmann::json::array();
    request["actions"].push_back({{"type", "pointer"},
                                  {"id", "mouse"},
                                  {"parameters", {{"pointerType", "mouse"}}},
                                  {"actions", mouse_actions}});

    string response;
    if (!request_bhejo("POST", session_url + "/actions", request.dump(), response, error))
    {
        return false;
    }

    string ignored_body, ignored_error;
    request_bhejo("DELETE", session_url + "/actions", "", ignored_body, ignored_error);

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json value;
    string js_error;
    run_js(R"JS(
        arguments[0].dispatchEvent(new MouseEvent('dblclick',
            { bubbles: true, cancelable: true }));
        return true;
    )JS",
           nlohmann::json::array({ref}), value, js_error);

    wait_until_loaded();
    return true;
}

bool BrowserController::right_click(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    double x = 0;
    double y = 0;
    if (!element_center(element_id, x, y, error))
    {
        return false;
    }

    auto px = [](double value)
    {
        return static_cast<long>(value < 0 ? value - 0.5 : value + 0.5);
    };

    nlohmann::json mouse_actions = nlohmann::json::array();
    mouse_actions.push_back({{"type", "pointerMove"},
                             {"duration", 0},
                             {"x", px(x)},
                             {"y", px(y)},
                             {"origin", "viewport"}});
    mouse_actions.push_back({{"type", "pointerDown"}, {"button", 2}});
    mouse_actions.push_back({{"type", "pointerUp"}, {"button", 2}});

    nlohmann::json request;
    request["actions"] = nlohmann::json::array();
    request["actions"].push_back({{"type", "pointer"},
                                  {"id", "mouse"},
                                  {"parameters", {{"pointerType", "mouse"}}},
                                  {"actions", mouse_actions}});

    string response;
    if (!request_bhejo("POST", session_url + "/actions", request.dump(), response, error))
    {
        return false;
    }

    string ignored_body, ignored_error;
    request_bhejo("DELETE", session_url + "/actions", "", ignored_body, ignored_error);

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json value;
    string js_error;
    run_js(R"JS(
        const r = arguments[0].getBoundingClientRect();
        arguments[0].dispatchEvent(new MouseEvent('contextmenu',
            { bubbles: true, cancelable: true, button: 2,
              clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 }));
        return true;
    )JS",
           nlohmann::json::array({ref}), value, js_error);

    return true;
}

bool BrowserController::focus_element(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json value;
    string js_error;
    if (!run_js(R"JS(
            arguments[0].scrollIntoView({block: 'center'});
            arguments[0].focus();
            return true;
        )JS",
                nlohmann::json::array({ref}), value, js_error))
    {
        error = "Focus failed: " + js_error;
        return false;
    }
    return true;
}

bool BrowserController::clear_element(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    string response;
    string http_error;
    if (request_bhejo("POST", session_url + "/element/" + element_id + "/clear",
                      "{}", response, http_error))
    {
        return true;
    }

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json value;
    string js_error;
    if (!run_js(R"JS(
            const el = arguments[0];
            if (el.isContentEditable) { el.innerHTML = ''; }
            else if ('value' in el) { el.value = ''; }
            else { el.textContent = ''; }
            el.dispatchEvent(new Event('input', { bubbles: true }));
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return true;
        )JS",
                nlohmann::json::array({ref}), value, js_error))
    {
        error = "Clear failed: " + http_error;
        return false;
    }
    return true;
}

bool BrowserController::set_checked(const string &selector, bool checked,
                                    string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json args = nlohmann::json::array({ref, checked});
    nlohmann::json value;
    string js_error;
    if (!run_js(R"JS(
            const el = arguments[0];
            const wanted = !!arguments[1];
            el.scrollIntoView({block: 'center'});
            if ('checked' in el && el.checked !== wanted) {
                el.click();
                if (el.checked !== wanted) { el.checked = wanted; }
            }
            el.dispatchEvent(new Event('input', { bubbles: true }));
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return { checked: !!el.checked };
        )JS",
                args, value, js_error))
    {
        error = "Check failed: " + js_error;
        return false;
    }

    bool actual = value.value("checked", checked);
    if (actual != checked)
    {
        error = "The element did not change to the requested state.";
        return false;
    }
    return true;
}

bool BrowserController::scroll_into_view(const string &selector, string &error)
{
    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json value;
    string js_error;
    if (!run_js(R"JS(
            arguments[0].scrollIntoView({block: 'center', inline: 'center'});
            return true;
        )JS",
                nlohmann::json::array({ref}), value, js_error))
    {
        error = "Scroll into view failed: " + js_error;
        return false;
    }
    return true;
}

bool BrowserController::get_html(const string &selector, string &html,
                                 string &error)
{
    string actual = selector.empty() ? "body" : selector;
    string element_id;
    if (!find_element_id(actual, element_id, error))
    {
        return false;
    }

    nlohmann::json ref;
    ref[ELEMENT_KEY] = element_id;
    nlohmann::json value;
    string js_error;
    if (!run_js("return arguments[0].outerHTML;",
                nlohmann::json::array({ref}), value, js_error))
    {
        error = "Could not read the HTML: " + js_error;
        return false;
    }

    if (!value.is_string())
    {
        error = "The element has no HTML.";
        return false;
    }

    html = value.get<string>();
    return true;
}

bool BrowserController::upload_file(const string &selector,
                                    const string &file_path, string &error)
{
    if (file_path.empty())
    {
        error = "File path cannot be empty!";
        return false;
    }

    string element_id;
    if (!find_element_id(selector, element_id, error))
    {
        return false;
    }

    nlohmann::json body;
    body["text"] = file_path;

    string response;
    if (!request_bhejo("POST", session_url + "/element/" + element_id + "/value",
                       body.dump(), response, error))
    {
        return false;
    }
    return true;
}

static string base64_decode(const string &input)
{
    static const string alphabet =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    int lookup[256];
    for (int i = 0; i < 256; i++)
    {
        lookup[i] = -1;
    }
    for (int i = 0; i < 64; i++)
    {
        lookup[static_cast<unsigned char>(alphabet[i])] = i;
    }

    string output;
    int buffer = 0;
    int bits = 0;

    for (char c : input)
    {
        if (c == '=')
        {
            break;
        }
        int value = lookup[static_cast<unsigned char>(c)];
        if (value < 0)
        {
            continue;
        }
        buffer = (buffer << 6) | value;
        bits += 6;
        if (bits >= 8)
        {
            bits -= 8;
            output.push_back(static_cast<char>((buffer >> bits) & 0xFF));
        }
    }

    return output;
}

bool BrowserController::screenshot(const string &file_path, string &saved_path,
                                   string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response;
    if (!request_bhejo("GET", session_url + "/screenshot", "", response, error))
    {
        return false;
    }

    string encoded;
    try
    {
        encoded = nlohmann::json::parse(response).at("value").get<string>();
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Could not read the screenshot data.";
        return false;
    }

    string path = file_path;
    if (path.empty())
    {
        auto now = chrono::system_clock::now().time_since_epoch();
        auto ms = chrono::duration_cast<chrono::milliseconds>(now).count();
        path = "automata-screenshot-" + to_string(ms) + ".png";
    }

    ofstream output(path, ios::binary | ios::trunc);
    if (!output)
    {
        error = "Could not write the screenshot to " + path;
        return false;
    }

    string png = base64_decode(encoded);
    output.write(png.data(), static_cast<streamsize>(png.size()));
    output.close();

    saved_path = path;
    return true;
}

bool BrowserController::wait_for_navigation(int timeout_ms, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }
    if (timeout_ms < 100)
    {
        timeout_ms = 100;
    }
    if (timeout_ms > 60000)
    {
        timeout_ms = 60000;
    }

    wait_until_loaded(timeout_ms);
    return true;
}

bool BrowserController::get_cookies(string &cookies_json, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response;
    if (!request_bhejo("GET", session_url + "/cookie", "", response, error))
    {
        return false;
    }

    try
    {
        cookies_json = nlohmann::json::parse(response).at("value").dump();
        return true;
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Could not read cookies.";
        return false;
    }
}

bool BrowserController::set_cookie(const string &cookie_json, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    nlohmann::json cookie;
    try
    {
        cookie = nlohmann::json::parse(cookie_json);
    }
    catch (const nlohmann::json::exception &)
    {
        error = "Cookie must be a valid JSON object.";
        return false;
    }

    if (!cookie.is_object() || !cookie.contains("name") ||
        !cookie.contains("value"))
    {
        error = "Cookie needs at least name and value.";
        return false;
    }

    nlohmann::json body;
    body["cookie"] = cookie;

    string response;
    if (!request_bhejo("POST", session_url + "/cookie", body.dump(), response, error))
    {
        return false;
    }
    return true;
}

bool BrowserController::delete_cookies(string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }

    string response;
    if (!request_bhejo("DELETE", session_url + "/cookie", "", response, error))
    {
        return false;
    }
    return true;
}

bool BrowserController::set_window_size(int width, int height, string &error)
{
    if (session_url.empty())
    {
        error = "Browser is not running!";
        return false;
    }
    if (width < 200 || width > 10000 || height < 200 || height > 10000)
    {
        error = "Window size must be between 200 and 10000 pixels.";
        return false;
    }

    nlohmann::json body;
    body["width"] = width;
    body["height"] = height;

    string response;
    if (!request_bhejo("POST", session_url + "/window/rect", body.dump(),
                       response, error))
    {
        return false;
    }

    wait_until_loaded();
    return true;
}

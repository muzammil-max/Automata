#include "api_server.h"
#include "nvidia_client.h"
#include <iostream>
#include <fstream>
#include <sstream>
#include <cstdlib>
using namespace std;
#include "../libraries/json.hpp"
#include <mutex>
#include <map>
#include <memory>
#include <atomic>
#include <vector>
#include "../agent/agent.h" // agent.h already includes BrowserController, which close_browser() below uses
#include <thread>
// #include <wayland-egl-backend.h>
// #include <X11/Xcursor/Xcursor.h>

#include "../instructions/instruction.h"

// ---------------------------------------------------------------
// Frontend folder ka pata.
// Default layout mein backend/ se "../frontend" hota hai. Packaged
// Windows app main.js se AUTOMATA_FRONTEND_DIR env var bhejti hai.
// ---------------------------------------------------------------
static string frontend_dir()
{
    const char *env = getenv("AUTOMATA_FRONTEND_DIR");
    if (env != nullptr && *env != '\0')
    {
        return string(env);
    }
    return "../frontend";
}

// ---------------------------------------------------------------
// Per-run stop flags (thread-safe).
// The stop endpoint sets the flag, Agent::run_task reads it.
// ---------------------------------------------------------------
static mutex g_cancel_mutex;
static map<string, shared_ptr<atomic<bool>>> g_cancel_flags;

static string cancel_key(int session_id, int run_id)
{
    return to_string(session_id) + "/" + to_string(run_id);
}

// Finds (or creates) the stop flag of a run.
static shared_ptr<atomic<bool>> get_cancel_flag(int session_id, int run_id)
{
    lock_guard<mutex> lock(g_cancel_mutex);

    string key = cancel_key(session_id, run_id);
    auto it = g_cancel_flags.find(key);

    if (it != g_cancel_flags.end())
    {
        return it->second;
    }

    auto flag = make_shared<atomic<bool>>(false);
    g_cancel_flags[key] = flag;
    return flag;
}

// Forgets the flag once the run is really over.
static void release_cancel_flag(int session_id, int run_id)
{
    lock_guard<mutex> lock(g_cancel_mutex);
    g_cancel_flags.erase(cancel_key(session_id, run_id));
}

// True when the run already reached an end state.
static bool run_is_finished(const string &status)
{
    return status == "completed" || status == "failed" || status == "stopped";
}

// Cuts a string at a UTF-8 character boundary.
// nlohmann::json throws on broken UTF-8, so we never cut mid-character.
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
        text.clear();
        return text;
    }

    unsigned char lead = static_cast<unsigned char>(text[i - 1]);
    int expected = 1;
    if ((lead & 0xE0) == 0xC0)      expected = 2;
    else if ((lead & 0xF0) == 0xE0) expected = 3;
    else if ((lead & 0xF8) == 0xF0) expected = 4;
    else if ((lead & 0x80) != 0)
    {
        text.clear();
        return text;
    }

    int have = static_cast<int>(text.size() - (i - 1));
    if (have != expected)
    {
        text.resize(i - 1);
    }

    return text;
}

// Reads a positive int from a URL id (the route already matched \d+,
// but a huge digit string would make stoi throw). False = bad id.
static bool parse_id(const string &text, int &out)
{
    if (text.empty() || text.size() > 9)
    {
        return false;
    }

    for (char c : text)
    {
        if (c < '0' || c > '9')
        {
            return false;
        }
    }

    try
    {
        out = stoi(text);
    }
    catch (...)
    {
        return false;
    }

    return out > 0;
}

// Runs one route handler. Any exception becomes a JSON 500 answer
// instead of crashing the whole server.
template <typename Fn>
static void guard(httplib::Response &res, Fn fn)
{
    try
    {
        fn();
    }
    catch (const exception &e)
    {
        nlohmann::json error_data;
        error_data["error"] = string("Internal server error: ") + e.what();
        res.status = 500;
        res.set_content(error_data.dump(), "application/json");
    }
    catch (...)
    {
        nlohmann::json error_data;
        error_data["error"] = "Internal server error";
        res.status = 500;
        res.set_content(error_data.dump(), "application/json");
    }
}

void server::health(const httplib::Request &req, httplib::Response &res)
{

    nlohmann::json result; // makes json object
    result["ok"] = true;   // field is added that is ok and its value will be boolean

    /*

    "ok" : true

    */
    res.set_content(result.dump(), "application/json");
} /// result.dump converts the json object into text and set_content sets the HTTP response body, application/json tells the client that the response is in JSON

void server::create_session(const httplib::Request &req, httplib::Response &res)
{
    nlohmann::json final_data;

    if (!req.body.empty())
    {
        try
        {
            final_data = nlohmann::json::parse(req.body);
        }
        catch (const nlohmann::json::parse_error &)
        {
            nlohmann::json error;
            error["error"] = "Invalid JSON";
            res.status = 400;
            res.set_content(error.dump(), "application/json");
            return;
        }

        if (final_data.is_object() == 0)
        {
            nlohmann::json error;
            error["error"] = "Invalid JSON object";
            res.status = 400;
            res.set_content(error.dump(), "application/json");
            return;
        }
    }

    SessionData new_session;

    // validating the inputs
    if (final_data.contains("title"))
    {
        if (final_data["title"].is_string() == 0)
        {
            nlohmann::json error;
            error["error"] = "Title must be a string";
            res.status = 400;
            res.set_content(error.dump(), "application/json");
            return;
        }

        new_session.title = final_data["title"].get<string>();
    }

    string error;

    //! Storing session data
    // SessionStorage will create the ID and save the Session in its JSON file.

    if (storage.create_session(new_session, error) == 0)
    {
        nlohmann::json error_data;
        error_data["error"] = error;

        res.status = 500;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    nlohmann::json result;
    result["sessionID"] = new_session.sessionID;

    res.status = 201; // 201 means created
    res.set_content(result.dump(), "application/json");

    //! notes:
    // Code      Meaning
    // 200       OK
    // 201       Created
    // 400       Bad request (e.g. malformed or missing JSON fields)
    // 401 / 403 Unauthorized / Forbidden
    // 404       Not found
    // 500       Server error/Bad gateway
    // 429       Too many requests / rate limit exceeded
}

void server::get_all_sessions(const httplib::Request &req, httplib::Response &res)
{
    // Get all saved Sessions from storage

    vector<SessionData> sessions = storage.get_all_sessions();

    nlohmann::json result;
    result["sessions"] = nlohmann::json::array();

    for (int i = 0; i < sessions.size(); i++)
    {
        nlohmann::json item;

        item["sessionID"] = sessions[i].sessionID;
        item["title"] = sessions[i].title;
        item["runCount"] = sessions[i].runs.size();

        result["sessions"].push_back(item);
    }

    res.set_content(result.dump(), "application/json");
}

void server::get_session(const httplib::Request &req, httplib::Response &res)
{
    string requested_id = req.matches[1].str(); // gets the id in URL GET /api/sessions/5 so id will be 5

    int session_id_number = 0;
    if (!parse_id(requested_id, session_id_number))
    {
        nlohmann::json error_data;
        error_data["error"] = "Invalid session id";

        res.status = 400;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    SessionData found_session;
    string error;

    //! search block
    if (storage.get_session(session_id_number, found_session, error) == 0)
    {
        nlohmann::json error_data;
        error_data["error"] = error;

        res.status = 404;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    nlohmann::json result;

    result["sessionID"] = found_session.sessionID;
    result["title"] = found_session.title;
    result["runs"] = nlohmann::json::array();

    // Add all Runs belonging to this Session

    for (int i = 0; i < found_session.runs.size(); i++)
    {
        nlohmann::json run;

        run["runID"] = found_session.runs[i].runID;
        run["message"] = found_session.runs[i].message;
        run["status"] = found_session.runs[i].status;
        run["output"] = found_session.runs[i].output;
        run["steps"] = found_session.runs[i].steps;

        result["runs"].push_back(run);
    }

    res.set_content(result.dump(), "application/json");
}

void server::create_run(const httplib::Request &req, httplib::Response &res)
{
    // Gets the Session ID from the URL
    int sessionID = 0;
    if (!parse_id(req.matches[1].str(), sessionID))
    {
        nlohmann::json error;
        error["error"] = "Invalid session id";
        res.status = 400;
        res.set_content(error.dump(), "application/json");
        return;
    }

    nlohmann::json final_data;

    try
    {
        final_data = nlohmann::json::parse(req.body);
    }
    catch (const nlohmann::json::parse_error &)
    {
        nlohmann::json error;
        error["error"] = "Invalid JSON";
        res.status = 400;
        res.set_content(error.dump(), "application/json");
        return;
    }

    // Validating the inputs

    if (final_data.is_object() == 0 ||
        final_data.contains("message") == 0 ||
        final_data["message"].is_string() == 0)
    {
        nlohmann::json error;
        error["error"] = "Message is needed";
        res.status = 400;
        res.set_content(error.dump(), "application/json");
        return;
    }

    string message = final_data["message"].get<string>();

    if (message.empty())
    {
        nlohmann::json error;
        error["error"] = "Message is NEEDED!";
        res.status = 400;
        res.set_content(error.dump(), "application/json");
        return;
    }

    cout << "Message: " << message << endl;

    // storing the previous runs so that model remebers

    SessionData existing_sessions;
    string error;

    if (!storage.get_session(sessionID, existing_sessions, error))
    {

        nlohmann::json error_data;
        error_data["error"] = "SESSION WAS NOT FOUND !!!";
        res.status = 404;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    // Purane runs model ko wapas dikhate hain taake chat yaad rahe.
    // Sirf completed / stopped runs, sirf user message aur final answer,
    // size capped — failed runs ka loop model ko galat raste par le jata hai.
    const size_t MAX_HISTORY_RUNS = 12;
    const size_t MAX_HISTORY_MESSAGE = 800;
    const size_t MAX_HISTORY_OUTPUT = 2400;

    vector<RunData> history;
    for (int i = static_cast<int>(existing_sessions.runs.size()) - 1;
         i >= 0 && history.size() < MAX_HISTORY_RUNS;
         i--)
    {
        const RunData &old_run = existing_sessions.runs[i];

        if (old_run.status != "completed" && old_run.status != "stopped")
        {
            continue;
        }

        RunData slim;
        slim.runID = old_run.runID;
        slim.status = old_run.status;
        slim.message = utf8_truncate(old_run.message, MAX_HISTORY_MESSAGE);
        slim.output = utf8_truncate(old_run.output, MAX_HISTORY_OUTPUT);
        slim.steps = nlohmann::json::array();   // steps are far too noisy

        history.insert(history.begin(), slim);
    }

    cout << "Previous Runs loaded: " << history.size() << endl;
    //! Storing Run data

    RunData new_run;

    new_run.message = message;
    new_run.status = "queued";
    new_run.output = "";

    // string error;

    if (!storage.add_run(sessionID, new_run, error))
    {
        nlohmann::json error_data;
        error_data["error"] = error;

        SessionData check_session;
        string check_error;

        if (!storage.get_session(sessionID, check_session, check_error))
        {
            res.status = 404;
        }
        else
        {
            res.status = 500;
        }

        res.set_content(error_data.dump(), "application/json");
        return;
    }

    //! Sending message to the model (status update before the worker starts)

    new_run.status = "running";

    if (!storage.update_run(sessionID, new_run, error))
    {
        nlohmann::json error_data;
        error_data["error"] = error;
        res.status = 500;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    // The run now happens on a worker thread, so this request returns
    // immediately (202) and the client polls GET /runs/{id} for progress.
    auto cancel_flag = get_cancel_flag(sessionID, new_run.runID);
    RunData run_copy = new_run;

    thread([this, sessionID, message, history, run_copy, cancel_flag]() mutable
    {
        try
        {
            NVIDIAClient nvidia;

            //
            // A NEW Agent oobject is created for every request (this is fine), but the
            // BROWSER is not created again. BrowserController keeps ONE shared session
            // for the whole program, so every Agent finds the browser that is already open.
            //
            /*
                    three cases how the agent uses browser
                    1) browser action , if no browser found the agent will open browsr ohterwise it will reuse it
                    2) can close the broweer too
            */
            //
            // Agent::run_task holds a lock for the whole task, so if two requests IMPORTAN!!!
            // arrive at the same time, the second one WAITS until the first task
            // is finished. (Two tasks must never control the same browser together.)
            Agent agent(nvidia);

            agent.cancel_flag = cancel_flag;

            agent.on_step = [this, sessionID, cancel_flag](const RunData &snapshot)
            {
                if (cancel_flag->load())
                {
                    return;
                }

                string ignored;
                storage.update_run(sessionID, snapshot, ignored);
            };

            string response;
            string agent_error;

            bool success = agent.run_task(
                message,
                history,
                run_copy,
                response,
                agent_error);

            if (cancel_flag->load())
            {
                run_copy.status = "stopped";
                run_copy.output = response.empty() ? "Run stopped by user." : response;

                cout << "Agent stopped by user." << endl;
            }
            else if (success)
            {
                run_copy.status = "completed";
                run_copy.output = response;

                cout << "Agent completed: " << response << endl;
            }
            else
            {
                run_copy.status = "failed";
                run_copy.output = response.empty() ? agent_error : response;

                
                cout << "Agent error: " << agent_error << endl;
            }

            // The run is over: forget the flag so it cannot affect a later run.
            release_cancel_flag(sessionID, run_copy.runID);

            string save_error;
            if (!storage.update_run(sessionID, run_copy, save_error))
            {
                cout << "Could not save run result: " << save_error << endl;
            }

            cout << "Run " << run_copy.runID
                 << " finished with status " << run_copy.status << endl;
        }
        catch (const exception &e)
        {
            release_cancel_flag(sessionID, run_copy.runID);
            cout << "Run thread error: " << e.what() << endl;
        }
        catch (...)
        {
            release_cancel_flag(sessionID, run_copy.runID);
            cout << "Run thread error: unknown error" << endl;
        }
    }).detach();

    nlohmann::json result;

    result["sessionID"] = sessionID;
    result["runID"] = new_run.runID;
    result["status"] = "running";

    // 202 Accepted: the task runs in the background now.
    res.status = 202;
    res.set_content(result.dump(), "application/json");
}

void server::get_run(const httplib::Request &req, httplib::Response &res)
{
    // Gets both IDs from URL:
    // GET /api/sessions/1/runs/2
    // Session ID = 1, Run ID = 2

    int sessionID = 0;
    int runID = 0;
    if (!parse_id(req.matches[1].str(), sessionID) ||
        !parse_id(req.matches[2].str(), runID))
    {
        nlohmann::json error_data;
        error_data["error"] = "Invalid session id or run id";
        res.status = 400;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    SessionData found_session;
    string error;

    if (storage.get_session(sessionID, found_session, error) == 0)
    {
        nlohmann::json error_data;
        error_data["error"] = "Session not found";
        res.status = 404;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    bool isfound = false;
    RunData found_run;

    //! search block

    for (int i = 0; i < found_session.runs.size(); i++)
    {
        // if Run ID matches with requested ID then give true

        if (found_session.runs[i].runID == runID)
        {
            // will copy the matching Run's data into found_run

            found_run = found_session.runs[i];
            isfound = true;
            break;

            //  found_run = found_sessssion.runs[i+4]; not works :(
            // isfound = true;
            // break;

            // if the Session contains Runs 1, 2, and 3,
            // and the request asks for Run 2, the loop finds it and copies it.
        }
    }

    if (isfound == false)
    {
        nlohmann::json error_data;
        error_data["error"] = "Run not found!";
        res.status = 404;
        res.set_content(error_data.dump(), "application/json");
        return;
    }
    else
    {
        nlohmann::json result;

        result["sessionID"] = sessionID;
        result["runID"] = found_run.runID;
        result["message"] = found_run.message;
        result["status"] = found_run.status;
        result["output"] = found_run.output;
        result["steps"] = found_run.steps;

        res.set_content(result.dump(), "application/json");
    }

    //----
}

void server::stop_run(const httplib::Request &req, httplib::Response &res)
{
    int sessionID = 0;
    int runID = 0;
    if (!parse_id(req.matches[1].str(), sessionID) ||
        !parse_id(req.matches[2].str(), runID))
    {
        nlohmann::json error_data;
        error_data["error"] = "Invalid session id or run id";
        res.status = 400;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    SessionData found_session;
    string error;

    if (storage.get_session(sessionID, found_session, error) == 0)
    {
        nlohmann::json error_data;
        error_data["error"] = "Session not found";
        res.status = 404;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    bool isfound = false;
    RunData updated_run;

    //! stopping

    for (int i = 0; i < found_session.runs.size(); i++)
    {
        if (found_session.runs[i].runID == runID)
        {
            updated_run = found_session.runs[i];

            // A finished run must not be flipped to "stopped".
            if (run_is_finished(updated_run.status))
            {
                nlohmann::json error_data;
                error_data["error"] = "Run already finished (" +
                                      updated_run.status + ")";
                res.status = 409;
                res.set_content(error_data.dump(), "application/json");
                return;
            }

            updated_run.status = "stopped";

            isfound = true;
            break;
        }
    }

    if (isfound == 0)
    {
        nlohmann::json error_data;
        error_data["error"] = "Run not found";
        res.status = 404;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    // Tell the running Agent to stop at the next loop turn.
    get_cancel_flag(sessionID, runID)->store(true);

    if (storage.update_run(sessionID, updated_run, error) == 0)
    {
        nlohmann::json error_data;
        error_data["error"] = error;
        res.status = 500;
        res.set_content(error_data.dump(), "application/json");
        return;
    }

    nlohmann::json result;
    result["ok"] = true;

    res.set_content(result.dump(), "application/json");
}


void server::close_browser(const httplib::Request &req, httplib::Response &res)
{
    BrowserController browser; // shares the session of the running browser
    string error;

    nlohmann::json result;
    result["ok"] = browser.close_browser(error);

    if (!error.empty())
    {
        result["error"] = error;
    }

    res.set_content(result.dump(), "application/json");
}

void server::browser_status(const httplib::Request &req, httplib::Response &res)
{
    BrowserController browser; // only reads the shared static session
    nlohmann::json result;
    result["running"] = browser.is_running();

    res.set_content(result.dump(), "application/json");
}

void server::start()
{

    //AHADS PART!
    // Serve the Electron UI (index.html, css, js) at "/". The desktop
    // window loads http://127.0.0.1:8080/, so without this the page 404s
    // and the window shows only its blue background. A missing file falls
    // through to the API routes below, so /health and /api/* still work.
    //TODO:
    // IMPLEMENT RUST EGUI VERY GOOD!AND FAST
    vector<string> ui_dirs = {frontend_dir()};
    if (frontend_dir() == "../frontend")
    {
        // Dev fallback: kuch log backend/ ke bahar se bhi server chalate hain.
        ui_dirs.push_back("frontend");
        ui_dirs.push_back("./frontend");
    }

    for (const string &dir : ui_dirs)
    {
        if (serve.set_mount_point("/", dir))
        {
            cout << "Serving UI from " << dir << endl;
            break;
        }
    }

    // Boot splash page (the Electron window loads this first, then swaps
    // to "/" once the backend is healthy). Read from disk like the mount.
    serve.Get(
        "/boot",
        [](const httplib::Request &req, httplib::Response &res) -> bool
        {
            ifstream f(frontend_dir() + "/boot.html");
            stringstream ss;
            ss << f.rdbuf();
            res.set_content(ss.str(), "text/html");
            return true;
        });

    // setting up routes
    // Every handler runs inside guard(): an exception becomes a JSON 500
    // answer instead of killing the server.

    serve.Get(
        "/health",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { health(req, res); });
        });

    // Session routes

    serve.Post(
        "/api/sessions",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { create_session(req, res); });
        });

    serve.Get(
        "/api/sessions",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { get_all_sessions(req, res); });
        });

    serve.Get(
        R"(/api/sessions/(\d+))",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { get_session(req, res); });
        });

    // Run routes

    serve.Post(
        R"(/api/sessions/(\d+)/runs)",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { create_run(req, res); });
        });

    serve.Get(
        R"(/api/sessions/(\d+)/runs/(\d+))",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { get_run(req, res); });
        });
    serve.Post(
        R"(/api/sessions/(\d+)/runs/(\d+)/stop)",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { stop_run(req, res); });
        });

    serve.Post(
        "/api/browser/close",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { close_browser(req, res); });
        });

    serve.Get(
        "/api/browser/status",
        [this](const httplib::Request &req, httplib::Response &res)
        {
            guard(res, [&] { browser_status(req, res); });
        });

    cout << "Server runing on port 8080" << endl;

    serve.listen("127.0.0.1", 8080);
}
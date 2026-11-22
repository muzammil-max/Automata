#include "session_storage.h"

#include <fstream>
#include <iostream>
#include <algorithm>
#include <stdexcept>

using namespace std;

SessionStorage::SessionStorage()
{
    storage_path = "storage/sessions";

    try
    {
        filesystem::create_directories(storage_path);
    }
    catch (const filesystem::filesystem_error& e)
    {
        throw runtime_error("Could not create storage directory: " + string(e.what()));
    }
}

string SessionStorage::get_file_path(int sessionID)
{
    filesystem::path file_path = filesystem::path(storage_path) /
                                 (to_string(sessionID) + ".json");

    return file_path.string();
}

bool SessionStorage::load_session(
    int sessionID,
    SessionData& session,
    string& error
)
{
    string file_path = get_file_path(sessionID);

    ifstream file(file_path);

    if (!file.is_open())
    {
        error = "Session file not found!";
        return false;
    }

    try
    {
        nlohmann::json data;
        file >> data;

        if (!data.is_object())
        {
            error = "Invalid session data!";
            return false;
        }

        SessionData loaded_session;

        loaded_session.sessionID = data.at("sessionID").get<int>();

        if (loaded_session.sessionID != sessionID)
        {
            error = "Session ID mismatch!";
            return false;
        }

        loaded_session.title = data.value("title", string());

        if (!data.contains("runs") || !data["runs"].is_array())
        {
            error = "Invalid runs data!";
            return false;
        }

        for (int i = 0; i < data["runs"].size(); i++)
        {
            nlohmann::json item = data["runs"][i];

            if (!item.is_object())
            {
                error = "Invalid run data!";
                return false;
            }

            RunData run;

            run.runID = item.at("runID").get<int>();
            run.message = item.value("message", string());
            run.status = item.value("status", string("queued"));
            run.output = item.value("output", string());

            run.steps = item.value(
                "steps",
                nlohmann::json::array()
            );

            if (!run.steps.is_array())
            {
                error = "Invalid run steps!";
                return false;
            }

            loaded_session.runs.push_back(run);
        }

        session = loaded_session;

        return true;
    }
    catch (const nlohmann::json::exception& e)
    {
        error = "JSON error: " + string(e.what());
        return false;
    }
}

bool SessionStorage::save_session(
    const SessionData& session,
    string& error
)
{
    // Poora session ek JSON file mein likho: storage/sessions/<id>.json.
    // Har step ke baad save hota hai, is liye app band ho jaye tab bhi
    // chat aur run ka progress mehfooz rehta hai.
    nlohmann::json data;

    data["sessionID"] = session.sessionID;
    data["title"] = session.title;
    data["runs"] = nlohmann::json::array();

    for (int i = 0; i < session.runs.size(); i++)
    {
        nlohmann::json item;

        item["runID"] = session.runs[i].runID;
        item["message"] = session.runs[i].message;
        item["status"] = session.runs[i].status;
        item["output"] = session.runs[i].output;
        item["steps"] = session.runs[i].steps;

        data["runs"].push_back(item);
    }

    string file_path = get_file_path(session.sessionID);

    try
    {
        filesystem::create_directories(storage_path);
    }
    catch (const filesystem::filesystem_error& e)
    {
        error = "Storage directory error: " + string(e.what());
        return false;
    }

    ofstream file(file_path);

    if (!file.is_open())
    {
        error = "Could not open session file for writing!";
        return false;
    }

    file << data.dump(4);

    if (!file.good())
    {
        error = "Could not save session data!";
        return false;
    }

    return true;
}

bool SessionStorage::create_session(
    SessionData& session,
    string& error
)
{
    lock_guard<mutex> lock(storage_mutex);

    int highest_id = 0;

    try
    {
        for (const auto& entry : filesystem::directory_iterator(storage_path))
        {
            if (entry.path().extension() == ".json")
            {
                try
                {
                    int file_id = stoi(entry.path().stem().string());

                    if (file_id > highest_id)
                    {
                        highest_id = file_id;
                    }
                }
                catch (...)
                {
                    continue;
                }
            }
        }
    }
    catch (const filesystem::filesystem_error& e)
    {
        error = "Could not read sessions directory: " + string(e.what());
        return false;
    }

    session.sessionID = highest_id + 1;
    session.runs.clear();

    if (session.title.empty())
    {
        session.title = "New Session";
    }

    return save_session(session, error);
}

bool SessionStorage::get_session(
    int sessionID,
    SessionData& session,
    string& error
)
{
    lock_guard<mutex> lock(storage_mutex);

    if (sessionID <= 0)
    {
        error = "Invalid Session ID!";
        return false;
    }

    return load_session(sessionID, session, error);
}

vector<SessionData> SessionStorage::get_all_sessions()
{
    lock_guard<mutex> lock(storage_mutex);

    vector<SessionData> sessions;

    try
    {
        for (const auto& entry : filesystem::directory_iterator(storage_path))
        {
            if (entry.path().extension() != ".json")
            {
                continue;
            }

            try
            {
                int sessionID = stoi(entry.path().stem().string());

                SessionData session;
                string error;

                if (load_session(sessionID, session, error))
                {
                    sessions.push_back(session);
                }
                else
                {
                    cerr << "Could not load Session "
                         << sessionID << ": " << error << endl;
                }
            }
            catch (...)
            {
                continue;
            }
        }
    }
    catch (const filesystem::filesystem_error& e)
    {
        cerr << "Storage error: " << e.what() << endl;
    }

    sort(
        sessions.begin(),
        sessions.end(),
        [](const SessionData& a, const SessionData& b)
        {
            return a.sessionID < b.sessionID;
        }
    );

    return sessions;
}

bool SessionStorage::add_run(
    int sessionID,
    RunData& run,
    string& error
)
{
    lock_guard<mutex> lock(storage_mutex);

    SessionData session;

    if (!load_session(sessionID, session, error))
    {
        return false;
    }

    if (run.message.empty())
    {
        error = "Message cannot be empty!";
        return false;
    }

    int highest_id = 0;

    for (int i = 0; i < session.runs.size(); i++)
    {
        if (session.runs[i].runID > highest_id)
        {
            highest_id = session.runs[i].runID;
        }
    }

    run.runID = highest_id + 1;

    if (run.status.empty())
    {
        run.status = "queued";
    }

    if (!run.steps.is_array())
    {
        run.steps = nlohmann::json::array();
    }

    session.runs.push_back(run);

    return save_session(session, error);
}

bool SessionStorage::update_run(
    int sessionID,
    const RunData& updatedRun,
    string& error
)
{
    lock_guard<mutex> lock(storage_mutex);

    SessionData session;

    if (!load_session(sessionID, session, error))
    {
        return false;
    }

    for (int i = 0; i < session.runs.size(); i++)
    {
        if (session.runs[i].runID == updatedRun.runID)
        {
            session.runs[i] = updatedRun;

            return save_session(session, error);
        }
    }

    error = "Run not found!";
    return false;
}
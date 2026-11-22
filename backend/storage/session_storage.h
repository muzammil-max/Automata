#pragma once

#include "session_data.h"
#include <filesystem>
#include <mutex>
#include <string>
#include <vector>

using namespace std;

class SessionStorage
{
private:
    string storage_path;
    mutex storage_mutex;

    string get_file_path(int sessionID);

    bool load_session(
        int sessionID,
        SessionData& session,
        string& error
    );

    bool save_session(
        const SessionData& session,
        string& error
    );

public:
    SessionStorage();

    bool create_session(
        SessionData& session,
        string& error
    );

    bool get_session(
        int sessionID,
        SessionData& session,
        string& error
    );

    vector<SessionData> get_all_sessions();

    bool add_run(
        int sessionID,
        RunData& run,
        string& error
    );

    bool update_run(
        int sessionID,
        const RunData& updatedRun,
        string& error
    );
};
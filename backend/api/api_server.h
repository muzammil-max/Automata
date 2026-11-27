#pragma once

#include "../libraries/httplib.h"
#include "../storage/session_storage.h"
#include "../settings/llm_settings.h"

using namespace std;

class server
{
private:
    httplib::Server serve;

    SessionStorage storage;
    SettingsStore settings_store;

    void close_browser(const httplib::Request &req, 
        httplib::Response &res);

    void browser_status(
        const httplib::Request& req,
        httplib::Response& res
    );

    void health(
        const httplib::Request& req,
        httplib::Response& res
    );

    void create_session(
        const httplib::Request& req,
        httplib::Response& res
    );

    void get_all_sessions(
        const httplib::Request& req,
        httplib::Response& res
    );

    void get_session(
        const httplib::Request& req,
        httplib::Response& res
    );

    void create_run(
        const httplib::Request& req,
        httplib::Response& res
    );

    void get_run(
        const httplib::Request& req,
        httplib::Response& res
    );

    void stop_run(
        const httplib::Request& req,
        httplib::Response& res
    );

    void get_settings(
        const httplib::Request& req,
        httplib::Response& res
    );

    void update_settings(
        const httplib::Request& req,
        httplib::Response& res
    );

    void get_providers(
        const httplib::Request& req,
        httplib::Response& res
    );

public:
    void start();
};
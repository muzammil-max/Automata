#pragma once

#include "../libraries/httplib.h"
#include "../storage/session_storage.h"

using namespace std;

class server
{
private:
    httplib::Server serve;

    SessionStorage storage;
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

public:
    void start();
};
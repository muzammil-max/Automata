#include "session_storage.h"

#include <iostream>
#include <filesystem>

using namespace std;

int main()
{
    string error;
    int test_session_id;

    // First storage instance
    {
        SessionStorage storage;

        SessionData session;
        session.title = "Storage Test";

        if (!storage.create_session(session, error))
        {
            cout << error << endl;
            return 1;
        }

        test_session_id = session.sessionID;

        cout << "Session created: "
             << session.sessionID << endl;

        // First message
        RunData run1;
        run1.message = "Find five laptops.";

        if (!storage.add_run(session.sessionID, run1, error))
        {
            cout << error << endl;
            return 1;
        }

        run1.status = "completed";
        run1.output = "Found five laptops.";

        if (!storage.update_run(session.sessionID, run1, error))
        {
            cout << error << endl;
            return 1;
        }

        // Second message
        RunData run2;
        run2.message = "Compare their specifications.";

        if (!storage.add_run(session.sessionID, run2, error))
        {
            cout << error << endl;
            return 1;
        }

        cout << "Two Runs saved successfully!" << endl;
    }

    // Create a new storage instance
    {
        SessionStorage storage;

        SessionData loaded_session;

        if (!storage.get_session(
            test_session_id,
            loaded_session,
            error
        ))
        {
            cout << error << endl;
            return 1;
        }

        cout << "\nSESSION RETRIEVED" << endl;

        cout << "Session ID: "
             << loaded_session.sessionID << endl;

        cout << "Title: "
             << loaded_session.title << endl;

        cout << "Total Runs: "
             << loaded_session.runs.size() << endl;

        for (int i = 0; i < loaded_session.runs.size(); i++)
        {
            cout << "\nRun ID: "
                 << loaded_session.runs[i].runID << endl;

            cout << "Message: "
                 << loaded_session.runs[i].message << endl;

            cout << "Status: "
                 << loaded_session.runs[i].status << endl;

            cout << "Output: "
                 << loaded_session.runs[i].output << endl;
        }

        if (loaded_session.runs.size() != 2)
        {
            cout << "\nStorage test failed!" << endl;
            return 1;
        }

        if (loaded_session.runs[0].output != "Found five laptops.")
        {
            cout << "\nOutput verification failed!" << endl;
            return 1;
        }

        cout << "\nStorage test passed!" << endl;
    }

    return 0;
}
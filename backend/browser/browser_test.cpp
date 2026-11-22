
#include "browser_controller.h"

#include <curl/curl.h>
#include <iostream>

using namespace std;

int main()
{
    // Initialize libcurl
    curl_global_init(CURL_GLOBAL_DEFAULT);

    BrowserController browser;

    string error;
    string title;
    string page_text;

    // Starting browser
    cout << "Starting browser..." << endl;

    if (!browser.start_browser(error))
    {
        cout << "Error: " << error << endl;
        curl_global_cleanup();
        return 1;
    }

    // Opening website
    cout << "Opening website..." << endl;

    if (!browser.navigate("https://example.com", error))
    {
        cout << "Error: " << error << endl;
        browser.close_browser(error);
        curl_global_cleanup();
        return 1;
    }

    // Getting page title
    if (browser.get_title(title, error))
    {
        cout << "\nPage title: " << title << endl;
    }
    else
    {
        cout << "Title error: " << error << endl;
    }

    // Extracting webpage text
    if (browser.extract_text("body", page_text, error))
    {
        cout << "\nWebpage content:\n";
        cout << page_text << endl;
    }
    else
    {
        cout << "Extraction error: " << error << endl;
    }

    // Closing browser
    if (!browser.close_browser(error))
    {
        cout << "Closing error: " << error << endl;
    }

    // Cleaning up libcurl
    curl_global_cleanup();

    return 0;
}

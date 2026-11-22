#include "api/api_server.h"
#include "api/http_client.h"
#include <iostream>

using namespace std;

int main()
{
    // HTTP layer ko ek dafa initialize karo.
    // Linux par yeh curl hai, Windows par WinHTTP — is liye Windows
    // build ko alag se curl install karne ki zarurat nahi hoti.
    http_global_init();

    server api;
    api.start();

    http_global_cleanup();
}

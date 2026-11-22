#pragma once

#include <string>

// ================================================================
// Chhota HTTP client — ek hi interface Linux aur Windows dono par.
// Linux par libcurl use hota hai, Windows par WinHTTP (built-in),
// is liye Windows build ko curl ki library install karne ki zarurat
// nahi padti.
//
// Note: yahan "using namespace std" nahi hai, kyunki Windows headers
// apna "byte" typedef rakhte hain aur std::byte se clash ho jata hai.
// ================================================================

struct HttpResponse
{
    bool ok = false;      // transport theek chala (HTTP status 4xx/5xx bhi ho sakta hai)
    long status = 0;      // HTTP status code
    std::string body;     // response body
    std::string error;    // transport error (sirf tab jab ok == false)
};

// Ek blocking request bhejta hai. method: "GET", "POST", "DELETE" ...
// content_type / authorization khali string ho sakti hai.
HttpResponse http_request(const std::string &method,
                          const std::string &url,
                          const std::string &body,
                          const std::string &content_type,
                          const std::string &authorization,
                          long connect_timeout_seconds,
                          long total_timeout_seconds);

// Process start/end par ek dafa call karo (curl global state).
void http_global_init();
void http_global_cleanup();

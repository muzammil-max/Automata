#include "http_client.h"

#include <cstdlib>

// ================================================================
// Platform chunna: Linux par curl, Windows par WinHTTP.
// ================================================================

#ifdef _WIN32

// ---------------- Windows (WinHTTP) ----------------
#include <windows.h>
#include <winhttp.h>
#include <vector>

// Windows headers ke baad hi "using namespace std" — warna "byte" clash.
using namespace std;

static wstring to_wide(const string &text)
{
    if (text.empty())
    {
        return wstring();
    }

    int need = MultiByteToWideChar(CP_UTF8, 0, text.data(),
                                   static_cast<int>(text.size()), nullptr, 0);
    if (need <= 0)
    {
        return wstring();
    }

    wstring wide(need, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, text.data(),
                        static_cast<int>(text.size()), &wide[0], need);
    return wide;
}

static HttpResponse http_request_impl(
    const string &method,
    const string &url,
    const string &body,
    const string &content_type,
    const string &authorization,
    long connect_timeout_seconds,
    long total_timeout_seconds,
    const string &extra_headers,
    const function<void(const string &)> *on_chunk)
{
    HttpResponse out;

    wstring wide_url = to_wide(url);
    if (wide_url.empty())
    {
        out.error = "URL khali hai";
        return out;
    }

    // URL ko host / path / port mein todna.
    URL_COMPONENTS parts;
    ZeroMemory(&parts, sizeof(parts));
    parts.dwStructSize = sizeof(parts);

    wchar_t host[512] = {0};
    wchar_t path[4096] = {0};

    parts.lpszHostName = host;
    parts.dwHostNameLength = 511;
    parts.lpszUrlPath = path;
    parts.dwUrlPathLength = 4095;

    if (!WinHttpCrackUrl(wide_url.c_str(), 0, 0, &parts))
    {
        out.error = "URL ko parse nahi kar saka: " + url;
        return out;
    }

    const bool secure = (parts.nScheme == INTERNET_SCHEME_HTTPS);

    HINTERNET session = WinHttpOpen(L"Automata/1.0",
                                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                    WINHTTP_NO_PROXY_NAME,
                                    WINHTTP_NO_PROXY_BYPASS, 0);
    if (!session)
    {
        out.error = "WinHttpOpen fail ho gaya";
        return out;
    }

    WinHttpSetTimeouts(session,
                       static_cast<int>(connect_timeout_seconds * 1000),
                       static_cast<int>(connect_timeout_seconds * 1000),
                       static_cast<int>(total_timeout_seconds * 1000),
                       static_cast<int>(total_timeout_seconds * 1000));

    HINTERNET connection = WinHttpConnect(session, host, parts.nPort, 0);
    if (!connection)
    {
        out.error = "WinHttpConnect fail ho gaya";
        WinHttpCloseHandle(session);
        return out;
    }

    wstring wide_method = to_wide(method);
    HINTERNET request = WinHttpOpenRequest(connection,
                                           wide_method.c_str(),
                                           path,
                                           nullptr,
                                           WINHTTP_NO_REFERER,
                                           WINHTTP_DEFAULT_ACCEPT_TYPES,
                                           secure ? WINHTTP_FLAG_SECURE : 0);
    if (!request)
    {
        out.error = "WinHttpOpenRequest fail ho gaya";
        WinHttpCloseHandle(connection);
        WinHttpCloseHandle(session);
        return out;
    }

    wstring headers;
    if (!content_type.empty())
    {
        headers += L"Content-Type: " + to_wide(content_type) + L"\r\n";
    }
    if (!authorization.empty())
    {
        headers += L"Authorization: " + to_wide(authorization) + L"\r\n";
    }
    if (!extra_headers.empty())
    {
        headers += to_wide(extra_headers);
    }

    const void *body_ptr = body.empty() ? nullptr : body.data();
    DWORD body_len = static_cast<DWORD>(body.size());

    BOOL sent = WinHttpSendRequest(request,
                                   headers.empty() ? WINHTTP_NO_ADDITIONAL_HEADERS
                                                   : headers.c_str(),
                                   headers.empty() ? 0 : static_cast<DWORD>(-1L),
                                   const_cast<void *>(body_ptr),
                                   body_len,
                                   body_len,
                                   0);

    if (sent)
    {
        sent = WinHttpReceiveResponse(request, nullptr);
    }

    if (!sent)
    {
        out.error = "WinHTTP request fail ho gayi (error " +
                    to_string(GetLastError()) + ")";
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connection);
        WinHttpCloseHandle(session);
        return out;
    }

    // Status code nikalna.
    DWORD status = 0;
    DWORD status_len = sizeof(status);
    WinHttpQueryHeaders(request,
                        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                        WINHTTP_HEADER_NAME_BY_INDEX,
                        &status, &status_len, WINHTTP_NO_HEADER_INDEX);
    out.status = static_cast<long>(status);

    // Body ko tukron mein padhna.
    for (;;)
    {
        DWORD available = 0;
        if (!WinHttpQueryDataAvailable(request, &available) || available == 0)
        {
            break;
        }

        vector<char> buffer(available);
        DWORD read = 0;
        if (!WinHttpReadData(request, buffer.data(), available, &read) || read == 0)
        {
            break;
        }
        out.body.append(buffer.data(), read);
        if (on_chunk != nullptr && *on_chunk)
        {
            (*on_chunk)(string(buffer.data(), read));
        }
    }

    out.ok = true;

    WinHttpCloseHandle(request);
    WinHttpCloseHandle(connection);
    WinHttpCloseHandle(session);

    return out;
}

HttpResponse http_request(const string &method,
                          const string &url,
                          const string &body,
                          const string &content_type,
                          const string &authorization,
                          long connect_timeout_seconds,
                          long total_timeout_seconds,
                          const string &extra_headers)
{
    return http_request_impl(method, url, body, content_type, authorization,
                             connect_timeout_seconds, total_timeout_seconds,
                             extra_headers, nullptr);
}

HttpResponse http_request_stream(
    const string &method,
    const string &url,
    const string &body,
    const string &content_type,
    const string &authorization,
    long connect_timeout_seconds,
    long total_timeout_seconds,
    const function<void(const string &)> &on_chunk,
    const string &extra_headers)
{
    return http_request_impl(method, url, body, content_type, authorization,
                             connect_timeout_seconds, total_timeout_seconds,
                             extra_headers, &on_chunk);
}

void http_global_init() {}
void http_global_cleanup() {}

#else

// ---------------- Linux / macOS (libcurl) ----------------
#include <curl/curl.h>

using namespace std;

static size_t write_body(void *contents, size_t size, size_t nmemb, void *user_data)
{
    size_t total = size * nmemb;
    string *response = static_cast<string *>(user_data);
    response->append(static_cast<char *>(contents), total);
    return total;
}

struct StreamSink
{
    string *body = nullptr;
    const function<void(const string &)> *on_chunk = nullptr;
};

static size_t write_stream(void *contents, size_t size, size_t nmemb, void *user_data)
{
    size_t total = size * nmemb;
    StreamSink *sink = static_cast<StreamSink *>(user_data);
    if (sink->body != nullptr)
    {
        sink->body->append(static_cast<char *>(contents), total);
    }
    if (sink->on_chunk != nullptr && *sink->on_chunk)
    {
        (*sink->on_chunk)(string(static_cast<char *>(contents), total));
    }
    return total;
}

static HttpResponse http_request_impl(
    const string &method,
    const string &url,
    const string &body,
    const string &content_type,
    const string &authorization,
    long connect_timeout_seconds,
    long total_timeout_seconds,
    const string &extra_headers,
    const function<void(const string &)> *on_chunk)
{
    HttpResponse out;

    CURL *curl = curl_easy_init();
    if (curl == nullptr)
    {
        out.error = "CURL initialize nahi hua";
        return out;
    }

    struct curl_slist *headers = nullptr;
    if (!content_type.empty())
    {
        headers = curl_slist_append(headers,
                                    ("Content-Type: " + content_type).c_str());
    }
    if (!authorization.empty())
    {
        headers = curl_slist_append(headers,
                                    ("Authorization: " + authorization).c_str());
    }
    if (!extra_headers.empty())
    {
        size_t start = 0;
        while (start < extra_headers.size())
        {
            size_t end = extra_headers.find("\n", start);
            string line = extra_headers.substr(
                start, end == string::npos ? string::npos : end - start);
            if (!line.empty() && line.back() == '\r')
            {
                line.pop_back();
            }
            if (!line.empty())
            {
                headers = curl_slist_append(headers, line.c_str());
            }
            if (end == string::npos)
            {
                break;
            }
            start = end + 1;
        }
    }

    char error_buffer[CURL_ERROR_SIZE] = {0};

    StreamSink sink;
    sink.body = &out.body;
    sink.on_chunk = on_chunk;

    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, method.c_str());
    if (headers != nullptr)
    {
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
    }
    if (on_chunk != nullptr)
    {
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_stream);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA, &sink);
    }
    else
    {
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_body);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA, &out.body);
    }
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT, connect_timeout_seconds);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, total_timeout_seconds);
    curl_easy_setopt(curl, CURLOPT_NOSIGNAL, 1L);
    curl_easy_setopt(curl, CURLOPT_ERRORBUFFER, error_buffer);

    if (!body.empty())
    {
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
        curl_easy_setopt(curl, CURLOPT_POSTFIELDSIZE,
                         static_cast<long>(body.size()));
    }

    CURLcode result = curl_easy_perform(curl);

    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &out.status);

    if (result != CURLE_OK)
    {
        out.error = (error_buffer[0] != '\0') ? string(error_buffer)
                                              : string(curl_easy_strerror(result));
    }
    else
    {
        out.ok = true;
    }

    if (headers != nullptr)
    {
        curl_slist_free_all(headers);
    }
    curl_easy_cleanup(curl);

    return out;
}

HttpResponse http_request(const string &method,
                          const string &url,
                          const string &body,
                          const string &content_type,
                          const string &authorization,
                          long connect_timeout_seconds,
                          long total_timeout_seconds,
                          const string &extra_headers)
{
    return http_request_impl(method, url, body, content_type, authorization,
                             connect_timeout_seconds, total_timeout_seconds,
                             extra_headers, nullptr);
}

HttpResponse http_request_stream(
    const string &method,
    const string &url,
    const string &body,
    const string &content_type,
    const string &authorization,
    long connect_timeout_seconds,
    long total_timeout_seconds,
    const function<void(const string &)> &on_chunk,
    const string &extra_headers)
{
    return http_request_impl(method, url, body, content_type, authorization,
                             connect_timeout_seconds, total_timeout_seconds,
                             extra_headers, &on_chunk);
}

void http_global_init()
{
    curl_global_init(CURL_GLOBAL_DEFAULT);
}

void http_global_cleanup()
{
    curl_global_cleanup();
}

#endif

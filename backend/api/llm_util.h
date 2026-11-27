#pragma once

#include <string>

// UTF-8 safe truncation used for history sent back to a model.
inline std::string llm_truncate(std::string text, size_t max_bytes)
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
        return "";
    }

    unsigned char lead = static_cast<unsigned char>(text[i - 1]);
    int expected = 1;
    if ((lead & 0xE0) == 0xC0)
        expected = 2;
    else if ((lead & 0xF0) == 0xE0)
        expected = 3;
    else if ((lead & 0xF8) == 0xF0)
        expected = 4;
    else if ((lead & 0x80) != 0)
        return "";

    int have = static_cast<int>(text.size() - (i - 1));
    if (have != expected)
    {
        text.resize(i - 1);
    }

    return text;
}

inline std::string llm_trim(std::string text)
{
    size_t start = text.find_first_not_of(" \t\r\n");
    if (start == std::string::npos)
    {
        return "";
    }
    size_t end = text.find_last_not_of(" \t\r\n");
    return text.substr(start, end - start + 1);
}

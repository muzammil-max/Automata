#include "parser.h"
#include "../libraries/json.hpp"
// Parser AI ka JSON answer leta hai aur usse BrowserAction banata hai.
// Yahan action ki validation hoti hai — naya action add karna ho to
// is file mein allowlist bhi update karo.

using namespace std;

// Some AI models wrap the JSON in ```json fences or add a sentence before it.
// This keeps only the part from the first '{' to the last '}'.
// Safe for UTF-8: '{' and '}' are ASCII bytes, and an ASCII byte can never
// appear inside a multi-byte character, so the cut lands on a character edge.
static string extract_json_object(const string &text)
{
    size_t first = text.find('{');
    size_t last = text.rfind('}');

    if (first == string::npos || last == string::npos || last < first)
    {
        return text; // let the JSON parser report the real error
    }

    return text.substr(first, last - first + 1);
}

bool ActionParser::parse_action(
    const string &ai_response,
    BrowserAction &action,
    string &error)
{
    try
    {
        nlohmann::json data =
            nlohmann::json::parse(extract_json_object(ai_response));

        if (!data.is_object())
        {
            error = "AI response must be a JSON object!";
            return false;
        }

        // Check field data types
        if (!data.contains("action") ||
            !data["action"].is_string())
        {
            error = "Missing required field: action (a string)!";
            return false;
        }

        action.action = data["action"].get<string>();

        // target and value are optional: many actions do not need them
        // (Nemotron often omits an empty "value").
        // An optional "thought" field is allowed and simply ignored.
        action.target = "";
        action.value = "";

        if (data.contains("target") && !data["target"].is_null())
        {
            if (!data["target"].is_string())
            {
                error = "Field target must be a string!";
                return false;
            }
            action.target = data["target"].get<string>();
        }

        if (data.contains("value") && !data["value"].is_null())
        {
            if (!data["value"].is_string())
            {
                error = "Field value must be a string!";
                return false;
            }
            action.value = data["value"].get<string>();
        }

        // Allowed actions
        if (action.action != "navigate" &&
            action.action != "go_back" &&
            action.action != "go_forward" &&
            action.action != "new_tab" &&
            action.action != "reload" &&
            action.action != "remember" &&
            action.action != "click" &&
            action.action != "double_click" &&
            action.action != "right_click" &&
            action.action != "hover" &&
            action.action != "focus" &&
            action.action != "clear" &&
            action.action != "check" &&
            action.action != "uncheck" &&
            action.action != "scroll_into_view" &&
            action.action != "type" &&
            action.action != "extract" &&
            action.action != "get_html" &&
            action.action != "evaluate" &&
            action.action != "wait" &&
            action.action != "wait_for_navigation" &&
            action.action != "scroll" &&
            action.action != "press" &&
            action.action != "select" &&
            action.action != "drag" &&
            action.action != "wait_for" &&
            action.action != "switch_frame" &&
            action.action != "upload_file" &&
            action.action != "screenshot" &&
            action.action != "tabs" &&
            action.action != "switch_tab" &&
            action.action != "close_tab" &&
            action.action != "cookies" &&
            action.action != "set_cookie" &&
            action.action != "delete_cookies" &&
            action.action != "set_window_size" &&
            action.action != "finish" &&
            action.action != "close")
        {
            error = "Unsupported browser action: " + action.action;
            return false;
        }

        // Actions that need a target.
        // (extract may have an empty target = whole page)
        if ((action.action == "navigate" ||
             action.action == "click" ||
             action.action == "double_click" ||
             action.action == "right_click" ||
             action.action == "hover" ||
             action.action == "focus" ||
             action.action == "clear" ||
             action.action == "check" ||
             action.action == "uncheck" ||
             action.action == "scroll_into_view" ||
             action.action == "type" ||
             action.action == "scroll" ||
             action.action == "press" ||
             action.action == "select" ||
             action.action == "drag" ||
             action.action == "wait_for" ||
             action.action == "switch_frame" ||
             action.action == "switch_tab" ||
             action.action == "upload_file") &&
            action.target.empty())
        {
            error = "Action target cannot be empty!";
            return false;
        }

        // remember saves the text in "value", so it cannot be empty
        if (action.action == "remember" && action.value.empty())
        {
            error = "remember needs the text to save in value!";
            return false;
        }

        // Actions that need their payload in "value".
        if ((action.action == "evaluate" ||
             action.action == "set_cookie" ||
             action.action == "set_window_size" ||
             action.action == "upload_file") &&
            action.value.empty())
        {
            error = action.action + " needs its payload in value!";
            return false;
        }

        // drag needs the path in "value": "x1,y1;x2,y2;..."
        if (action.action == "drag" && action.value.empty())
        {
            error = "drag needs points in value, example: 10,20;100,80;200,40";
            return false;
        }

        return true;
    }
    catch (const nlohmann::json::exception &e)
    {
        error = "Invalid AI JSON: ";
        error += e.what();

        return false;
    }
}

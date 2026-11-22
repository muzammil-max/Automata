# 🤖 Automata — A Very Detailed Explanation (For a 5-Year-Old Who Loves Robots)

Hello, little friend! 🤖 This is **Automata**. It is a robot that lives inside your
computer. The robot can do two big things:

1. **Talk to you** like a friendly chat friend. 💬
2. **Drive a real web browser** (like Chrome) and do things on the internet. 🌐🖱️

And guess what? It can even **draw** on websites! 🎨🖍️

---

## 🧸 Part 1: The Simple Story (Explain Like I Am 5)

Imagine you have a tiny robot helper. You tell it:

> "Go to a website, read the news, then write it in a notepad."

The robot:
1. Opens Chrome (the car! 🚗).
2. Drives the car to the website.
3. Reads the page (like reading a book 📖).
4. Remembers what it read (in its memory 🧠).
5. Drives to a notepad website.
6. Writes the notes there (using the typing tool ✍️).
7. Comes back and tells you what it did. ✅

If you just say: *"What is the capital of France?"* — the robot doesn't need the car.
It just **talks** to you. 🗣️

---

## 🏗️ Part 2: The Big Picture (3 Helpers)

The project has **3 big parts**:

```
+----------------------+        HTTP/JSON         +----------------------+
|   FRONTEND (Electron)|  <-------------------> |   BACKEND (C++17)    |
|   (what you see)     |        (port 8080)       |   (the brain)         |
|   main.js, index,   |                           |   api/, agent/,      |
|   css, js, boot...) |                           |   browser/, parser/,  |
+----------------------+                           |   storage/, instruct. |
                                                      +----------------------+
                                                                    |
                                                                    | ChromeDriver (WebDriver)
                                                                    v
                                                          +----------------------+
                                                          |   CHROME / CHROMIUM |
                                                          |   (the actual car)   |
                                                          +----------------------+
                                                                    |
                                                                    | NVIDIA API (the AI brain
                                                                    |  that thinks & answers)
                                                                    v
                                                          +----------------------+
                                                          |      NVIDIA AI       |
                                                          | (nemotron model)     |
                                                          +----------------------+
```

- **Frontend** = the window you click. Electron app. Blue. Splash. Chat. Drawing tool. 🎨
- **Backend** = C++ program that runs a server (like a tiny internet helper) on port 8080.
- **Agent** = the part of the backend that talks to the AI and tells the browser what to do.

---

## 📂 Part 3: Backend Files — Explained (C++ Only), Each File, Each Line, Simply

The backend is in `backend/`. Here are all the C++ files, explained in a simple way.

### 3.1 `backend/main.cpp` — The Front Door of the Backend 🕳️

This is the **first file** that runs. It is like the front door of the robot house.

```cpp
#include "api/api_server.h"      // The big server thing (class "server").
#include <iostream>               // Lets the program print words to the screen.
#include <curl/curl.h>             // Lets C++ talk to the internet (like a walkie-talkie).

int main() {                      // The program starts here. Like turning on the robot.
    CURLcode result = curl_global_init(CURL_GLOBAL_DEFAULT);  // Wake up the internet walkie-talkie.
    if (result != CURLE_OK) { ... return 1; }                  // If walkie-talkie broke, stop.

    server api;                   // Make a new "server" robot.
    api.start();                  // Tell the server robot: "Open the door! Listen on port 8080!"

    http_global_cleanup();       // Go to sleep the internet walkie-talkie.
}
```

**Line-by-line simple meaning:**
- Line 1-3: We bring in tools we need.
- Line 5: `int main()` — the program begins.
- Lines 6-9: Start + check the internet tool.
- Lines 11-12: Make the server and start it.
- Line 14: Clean up before we close.

> Simple: This file just starts the whole backend. Nothing fancy. It says "hey, start the server!" and then stops.

---

### 3.2 `backend/api/api_server.h` — The Server's Shopping List 📋

This file says what the server has inside it (like a shopping list).

```cpp
#pragma once

#include "../libraries/httplib.h"   // The big tool that lets C++ make web servers (tiny).
#include "../storage/session_storage.h"

using namespace std;

class server
{
private:
    httplib::Server serve;              // The actual web server part.
    SessionStorage storage;             // Where chats are saved (like a toy box).
    ...
public:
    void start();                       // Start the server.
    void create_run(...);               // Make a new task/run.
    ...
};
```

**Simple meaning:** This file just says: "The server will have a web server part, a storage box for chats, and some buttons (functions) like start, create_run, etc."

---

### 3.3 `backend/api/api_server.cpp` — The Server's Job (Very Big File) 🏢

This is the **biggest file**. It does many jobs. Let's go through the important parts simply.

**A) Starting the server (`server::start()`):**
```cpp
void server::start()
{
    // Serve the Electron UI at "/"
    for (const string &dir : {"../frontend", "frontend", "./frontend"})
    {
        if (serve.set_mount_point("/", dir)) { ... break; }
    }

    // Boot splash page at "/boot"
    serve.Get("/boot", ...);

    // /health route
    serve.Get("/health", ...);

    // /api/sessions ... etc
}
```

**Simple meaning:**
- The server opens a door at address `http://127.0.0.1:8080/`.
- It serves the pretty frontend files (html/css/js) from the `frontend` folder. 📄
- It also opens a door at `/boot` that shows the splash page. 💦
- It opens doors for health checks, sessions, runs, browser status, etc.

**B) Creating a run (`server::create_run`):** This is what happens when you hit "Run".
```cpp
void server::create_run(const httplib::Request &req, httplib::Response &res)
{
    // 1. Get session id from URL. (Like getting which chat you're in.)
    // 2. Parse JSON body to get the message. (What you typed.)
    // 3. Load old sessions/runs. (Load memory of old chats.)
    // 4. Build history: last 12 runs -> user msg + answer. (Memory!)
    // 5. Save new run (status queued).
    // 6. Set status running.
    // 7. Run agent in a new thread. (Do the task in background.)
}
```

**Simple meaning:** When you press Run, the server:
- Finds your chat (session).
- Looks at old chats (last 12) to remember context. 🧠
- Saves the new task.
- Sends it to the agent to do in the background. ⏳

---

### 3.4 `backend/api/nvidia_client.h` / `nvidia_client.cpp` — The AI's Phone Call 📞🤖

This is how the backend talks to the **NVIDIA AI** (the robot brain that thinks).

```cpp
class NVIDIAClient : public LLMClient
{
public:
    bool generate_response(prompt, instructions, response, error) override;
    bool generate_response(prompt, instructions, history, response, error) override;
};
```

**In `nvidia_client.cpp`:**
- It builds a list of messages (system instructions + history + current user prompt).
- It calls the NVIDIA API: `https://integrate.api.nvidia.com/v1/chat/completions`.
- It sends the request.
- It gets back an answer.
- It retries up to 3 times if there is a network problem or server error (like when your friend tries again). 🔁

> Simple: This file is the phone line. It says: "AI, here is what you asked, here is what you said before, now answer."

---

### 3.5 `backend/api/http_client.h` / `http_client.cpp` — The Internet Helper 🌐 (NEW, for Windows)

This is a **new helper** we made. It helps the backend talk to the internet.

- On **Linux**: uses `libcurl` (a tool that talks to internet). 🐧
- On **Windows**: uses `WinHTTP` (built into Windows). 🪟

Why? Because the **Windows version** needed this so it didn't need extra internet tools installed.

```cpp
HttpResponse http_request(method, url, body, content_type, authorization, connect_timeout_seconds, total_timeout_seconds);
```

**Simple meaning:** "Hey, send this message to this internet address and tell me the answer."

---

### 3.6 `backend/agent/agent.h` / `agent.cpp` — The Robot Brain 🧠 (THE MOST IMPORTANT FILE)

This is the **brain** of the robot.

```cpp
class Agent
{
private:
    LLMClient &llm_client;     // The AI phone.
    BrowserController browser; // The car (browser).
    ActionParser parser;       // Reads AI's JSON answer into a real action.

    string notes;               // The robot's memory. 🧠
    string page_text;           // What the page said.
    map<string, int> visited;  // Pages visited.

    bool execute_action(...);   // Does one action like click/type.

public:
    bool run_task(user_prompt, history, current_run, final_output, error);
};
```

**`run_task` does this (step by step):**
1. It clears old memory for this task. 🧹
2. It records each step into `current_run.steps` (for live updates). 📝
3. It checks: does the task need the browser? (`task_needs_browser`)
4. If **no** browser → just chat. 💬
5. If **yes** → build prompt and start the loop:
   - Ask AI what to do next.
   - AI returns JSON like `{"action": "click", "target": "...", "value": ""}`.
   - Parser turns JSON into an action.
   - Browser executes the action.
   - Repeat until done or action limit. 🔁

**The "memory" part we added (conversation context):**
```cpp
static string conversation_context(const vector<RunData> &history, size_t max_runs)
{
    // Build a string of old user messages + old answers.
}
```
And in the prompt:
```cpp
prompt += "\n\nCONVERSATION SO FAR (older messages in this chat):\n";
prompt += recap;
```

> Simple: The brain keeps a little memory of what you said before, so it doesn't forget.

---

### 3.7 `backend/browser/browser_controller.h` / `browser_controller.cpp` — The Car Driver 🚗

This drives **Chrome** using something called **ChromeDriver** (a remote control for Chrome).

```cpp
class BrowserController
{
public:
    bool is_running();
    bool start_browser();
    bool close_browser();
    bool request_bhejo(method, endpoint, body, reponse_body, error); // sends HTTP to driver
    bool drag(target, points, error);  // NEW: draws on canvas!
};
```

**`drag` (the drawing tool):**
- Reads points like `"10,20;100,80;200,40"`.
- Uses WebDriver Actions: press mouse → move → release.
- This makes the agent able to **draw** on canvas/whiteboard websites. 🎨

---

### 3.8 `backend/parser/parser.h` / `parser.cpp` — The Translator 🈯

The AI speaks JSON. The robot speaks actions.

```cpp
class ActionParser
{
public:
    bool parse_action(ai_response, action, error);
};
```

It takes the AI's JSON like:
```json
{"action": "click", "target": "[data-agent-id=\"5\"]", "value": ""}
```
and turns it into a real `BrowserAction`.

It also **allows** actions: navigate, go_back, click, type, extract, scroll, press, select, wait, wait_for, finish, close, switch_frame, reload, remember, new_tab, **drag**, etc.

---

### 3.9 `backend/storage/session_storage.h` / `session_storage.cpp` — The Memory Box 📦

Saves chats to files. Like writing in a notebook.

- `save_session`, `load_session`, `add_run`, etc.

Each chat is saved as a JSON file: `storage/sessions/<sessionID>.json`.

---

### 3.10 `backend/instructions/instruction.h` / `instruction.cpp` — The Robot's Rulebook 📜

This tells the AI **how to behave**.

```cpp
string Instruction::get_text() {
    return R"INSTR( ... big rules ... )INSTR";
}
string Instruction::get_chat_text() { ... };
```

The big rules include:
- Output format: one JSON object with `action`, `target`, `value`.
- Supported actions (including `drag` for drawing!).
- What the agent sees each turn: USER TASK, TASK PROGRESS, NEXT REQUIRED STEP, NOTES, PAGES ALREADY OPENED, CAPTURED PAGE TEXT, CURRENT WEBPAGE, PREVIOUS ACTION RESULT, RECENT STEPS.
- Drawing instructions: how to use `drag` for canvas drawing.

---

## 🖥️ Part 4: Frontend Files (C++ Only Was Requested, So Brief)

Since you asked for **C++ only**, here are the frontend files just listed (not deep):

- `frontend/main.js` — Electron window. Starts backend. Loads splash. Swaps to app.
- `frontend/boot.html` + `frontend/js/boot.js` — Splash page. Blue. Big. Logo. Status.
- `frontend/index.html` — The chat app page.
- `frontend/js/app.js` — Chat logic. Boot. Sessions. Sending messages.
- `frontend/js/api.js` — Talking to backend (fetch).
- `frontend/js/components.js` — UI building blocks (Logo, etc).
- `frontend/js/draw.js` — User drawing overlay (canvas, colors, eraser, save).
- `frontend/css/theme.css`, `css/app.css`, `css/splash.css` — Colors.

> Note: You asked for C++ explanation. The frontend is JavaScript. So I explained only C++ here.

---

## 🧠 Part 5: How the Agent Decides — Browser or Chat? (Simple)

The robot brain has a helper:

```cpp
bool task_needs_browser(const string &prompt)
{
    // Checks if prompt has words like "go to", "open", "navigate", "click",
    // "draw", "sketch", "paint", "canvas", ... (we added drawing words!).
    // Also checks if there is a URL (http:// or https://).
}
```

- If **true** → the agent uses the **browser** (car) and drawing tools.
- If **false** → the agent just **talks** (chat).

We added drawing words so "draw a circle" → browser. ✅

---

## 🛠️ Part 6: How to Build (Simple)

**Linux:**
```sh
cd backend
g++ -std=c++17 -Wall -Wextra main.cpp api/api_server.cpp api/nvidia_client.cpp api/http_client.cpp agent/agent.cpp browser/browser_controller.cpp parser/parser.cpp storage/session_storage.cpp instructions/instruction.cpp -lcurl -lpthread -o api_server
```

**Windows:** Cross-compile with Zig → produces `api_server.exe` (PE32+ x64) → works with wine.

---

## 🌐 Part 7: API (Simple List)

- `GET /health` → Is the backend alive? ✅
- `GET /boot` → Splash page.
- `GET /` → Main app page.
- `GET /api/sessions` → List chats.
- `POST /api/sessions` → New chat.
- `GET /api/sessions/{id}` → One chat.
- `POST /api/sessions/{id}/runs` → Start a task (202).
- `GET /api/sessions/{id}/runs/{runId}` → Get task progress.
- `POST /api/sessions/{id}/runs/{runId}/stop` → Stop task.
- `GET /api/browser/status` → Is browser running?
- `POST /api/browser/close` → Close browser.

---

## 🔐 Part 8: Environment Variables (Simple)

- `CHROMEDRIVER_URL` → where ChromeDriver is (default `http://127.0.0.1:9515`).
- `CHROMIUM_PATH` / `CHROME_PATH` → which browser to drive.
- `NVIDIA_API_KEY` → AI key.
- `AGENT_MAX_ACTIONS` → how many actions per task (default 100).

---

## 🧒 Part 9: The "Explain Like I Am 5" Summary

- **main.cpp**: turns on the server. 🚪
- **api_server**: opens doors (routes). 🚪
- **nvidia_client**: talks to AI. 📞🤖
- **http_client**: internet helper. 🌐
- **agent**: the brain. 🧠
- **browser_controller**: drives Chrome. 🚗
- **parser**: reads AI's JSON. 🈯
- **storage**: saves chats. 📦
- **instructions**: robot's rules. 📜

The robot (agent) thinks (AI) → says what to do (JSON) → parses (translator) → drives (browser/car) → draws (drag) → remembers (memory) → tells you (answer). 🤖🎨🚗

---

## ✅ Part 10: All the Things You Asked For (Done)

1. Agent memory + browser tools reminder → ✅ done + tested.
2. Drawing tool → ✅ agent can draw via drag; fixed "draw" keyword routing.
3. Comments (Roman Urdu + English) → ✅ added.
4. Loading screen bigger + blue → ✅ done.
5. Windows version → ✅ built (zig + WinHTTP, tested on wine).
6. Splash scale bigger → ✅ done.
7. Main UI blue → ✅ done.
8. New chat every launch → ✅ done.

---

*This README explains the C++ backend in a very detailed, beginner-friendly way. Frontend files are JavaScript/HTML/CSS and were listed briefly as you asked for C++ only.*

/* ============================================================
   AUTOMATA — CHAT APPLICATION
   - Chat messages + live agent activity
   - Sidebar session history
   - Thinking shimmer + blinking cursor while the agent works
   - Chat-only requests do not open the browser (backend decides)
   ============================================================ */

(function () {
  "use strict";

  const UI = window.AutomataUI;
  const API = window.AutomataAPI;

  const $ = (id) => document.getElementById(id);

  const TERMINAL = { completed: 1, failed: 1, stopped: 1 };
  const SAFETY_PREFIX = "SAFETY CONFIRMATION:";

  const app = {
    sessionId: null,
    runId: null,
    running: false,
    pollTimer: null,
    browserTimer: null,

    steps: [],
    lastStepCount: -1,
    lastStepAt: 0,

    sidebarOpen: false,
    activeActivity: null,
    sessions: [],
    pendingMessages: 0
  };

  /* ============================================================
     SMALL HELPERS
     ============================================================ */

  function truncate(text, max) {
    text = String(text == null ? "" : text);
    return text.length > max ? text.slice(0, max - 1) + "…" : text;
  }

  function hostOf(url) {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch (e) {
      return truncate(url, 44);
    }
  }

  function titleForSession(session) {
    const saved = localStorage.getItem("automata.title." + session.sessionID);
    if (saved) return saved;
    if (session.title && session.title !== "Automata Session" && session.title !== "New chat") {
      return session.title;
    }
    return "Chat " + session.sessionID;
  }

  function rememberTitle(sessionId, message) {
    if (!localStorage.getItem("automata.title." + sessionId)) {
      localStorage.setItem("automata.title." + sessionId, truncate(message, 42));
    }
  }

  function setChatTitle(title, subtitle) {
    if (title !== undefined) $("chat-title-text").textContent = title;
    if (subtitle !== undefined) $("chat-subtitle").textContent = subtitle;
  }

  function setHeroVisible(visible) {
    $("hero").classList.toggle("is-hidden", !visible);
  }

  function scrollToBottom(force) {
    const box = $("chat-scroll");
    if (!box) return;
    const nearBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 160;
    if (force || nearBottom) {
      box.scrollTop = box.scrollHeight;
    }
  }

  /* ============================================================
     MESSAGE RENDERING
     ============================================================ */

  function avatarFor(role) {
    const avatar = document.createElement("div");
    avatar.className = "avatar";
    avatar.setAttribute("aria-hidden", "true");

    if (role === "assistant") {
      avatar.appendChild(UI.Logo(22));
    } else {
      const dot = document.createElement("span");
      dot.className = "user-dot";
      avatar.appendChild(dot);
    }
    return avatar;
  }

  function addMessage(role, text, options) {
    options = options || {};

    const row = document.createElement("div");
    row.className = "message message--" + role;

    const bubble = document.createElement("div");
    bubble.className = "bubble" + (options.plain ? " is-plain" : "");
    bubble.textContent = text;

    row.append(avatarFor(role), bubble);
    $("messages").appendChild(row);

    setHeroVisible(false);
    scrollToBottom(true);
    return row;
  }

  function addSystemNote(text) {
    const note = document.createElement("div");
    note.className = "system-note";
    note.textContent = text;
    $("messages").appendChild(note);
    scrollToBottom(true);
  }

  /* ============================================================
     LIVE ACTIVITY CARD
     ============================================================ */

  function createActivity() {
    const row = document.createElement("div");
    row.className = "message message--assistant";

    const bubble = document.createElement("div");
    bubble.className = "bubble";

    const card = document.createElement("div");
    card.className = "activity";

    const head = document.createElement("div");
    head.className = "activity-head";

    const cursor = document.createElement("span");
    cursor.className = "cursor-loader";
    cursor.setAttribute("aria-hidden", "true");

    const status = document.createElement("span");
    status.className = "activity-status shimmer";
    status.textContent = "Thinking";

    head.append(cursor, status);

    const list = document.createElement("div");
    list.className = "activity-list";

    card.append(head, list);
    bubble.appendChild(card);
    row.append(avatarFor("assistant"), bubble);
    $("messages").appendChild(row);

    app.activeActivity = {
      row: row,
      card: card,
      status: status,
      list: list,
      cursor: cursor
    };

    addActivityLine("Connecting to the agent…", false);
    setHeroVisible(false);
    scrollToBottom(true);
  }

  function setActivityStatus(text, shimmer) {
    const a = app.activeActivity;
    if (!a) return;
    a.status.textContent = text;
    a.status.classList.toggle("shimmer", !!shimmer);
  }

  function addActivityLine(text, isError) {
    const a = app.activeActivity;
    if (!a || !text) return;

    a.list.querySelectorAll(".activity-line.is-latest")
      .forEach((el) => el.classList.remove("is-latest"));

    const line = document.createElement("div");
    line.className = "activity-line is-latest" + (isError ? " is-error" : "");

    const dot = document.createElement("span");
    dot.className = "activity-dot";

    const label = document.createElement("span");
    label.className = "activity-text";
    label.textContent = text;

    line.append(dot, label);
    a.list.appendChild(line);

    // Keep the card short. Old lines fall off the top.
    while (a.list.children.length > 9) {
      a.list.removeChild(a.list.firstChild);
    }

    scrollToBottom(false);
  }

  function finishActivity(kind, text) {
    const a = app.activeActivity;
    if (!a) return;

    a.status.classList.remove("shimmer");
    a.status.textContent = kind === "completed" ? "Done"
      : kind === "stopped" ? "Stopped" : "Finished with a problem";

    a.cursor.style.display = "none";
    a.card.classList.add("is-done");

    if (text) addActivityLine(text, kind === "failed");
    app.activeActivity = null;
  }

  function removeActivity() {
    const a = app.activeActivity;
    if (!a) return;
    a.row.remove();
    app.activeActivity = null;
  }

  /* ============================================================
     STEP -> HUMAN SENTENCE
     ============================================================ */

  function describeStep(step) {
    const action = step.action || "";

    if (action === "browser_start") return ["Opening the browser", false];
    if (action === "browser_reuse") return ["Using the browser that is already open", false];
    if (action === "navigate") return ["Opening " + hostOf(step.target || ""), false];
    if (action === "new_tab") return ["Opening a new tab at " + hostOf(step.target || ""), false];
    if (action === "go_back") return ["Going back to the previous page", false];
    if (action === "reload") return ["Reloading the page", false];
    if (action === "switch_frame") return ["Moving into an editor frame", false];

    if (action === "click") return ["Clicking an element on the page", false];
    if (action === "scroll") return ["Scrolling the page", false];
    if (action === "press") return ["Pressing a key", false];
    if (action === "select") return ["Choosing an option", false];
    if (action === "drag") return ["Drawing with the mouse", false];
    if (action === "wait") return ["Waiting for the page to settle", false];
    if (action === "wait_for") return ["Waiting for an element to appear", false];
    if (action === "remember") return ["Saving a note", false];
    if (action === "extract") return ["Reading the page", false];

    if (action === "type") {
      const size = String(step.value || "").length;
      return ["Writing " + size + " characters into the page", false];
    }

    if (action === "finish") return ["Finishing the task", false];
    if (action === "chat") return ["", false];
    if (action === "safety_gate") return ["Pausing for your confirmation", false];
    if (action === "invalid_response") return ["Fixing the agent response", true];
    if (action === "stop") return ["Stopping the run", false];

    if (step.status === "failed") {
      return [(step.error || "That step failed"), true];
    }
    return [action ? action + " step completed" : "Working", false];
  }

  /* ============================================================
     RUN HANDLING
     ============================================================ */

  function resetRunUi() {
    app.steps = [];
    app.lastStepCount = -1;
    app.lastStepAt = Date.now();
  }

  function startRunUi() {
    app.running = true;
    resetRunUi();
    $("send-btn").disabled = true;
    $("stop-btn").classList.remove("hidden");
    setChatTitle(undefined, "Working…");
    createActivity();
  }

  function updateFromRun(run) {
    const steps = Array.isArray(run.steps) ? run.steps : [];

    if (steps.length !== app.lastStepCount) {
      const fresh = app.lastStepCount < 0
        ? steps
        : steps.slice(app.lastStepCount);

      app.steps = steps;
      app.lastStepCount = steps.length;
      app.lastStepAt = Date.now();

      fresh.forEach(function (step) {
        const described = describeStep(step);
        if (described[0]) addActivityLine(described[0], described[1]);
      });

      const last = steps[steps.length - 1];
      if (last) {
        if (last.action === "chat") {
          setActivityStatus("Thinking", true);
          setChatTitle(undefined, "Thinking about your message…");
        } else if (last.action === "navigate") {
          setActivityStatus("Browsing", true);
          setChatTitle(undefined, "Browser task · " + hostOf(last.target || ""));
        } else if (last.action === "type" || last.action === "drag") {
          setActivityStatus("Working", true);
          setChatTitle(undefined, "Working on the page…");
        } else {
          setActivityStatus("Working", true);
        }
      }
    }

    if (!app.running) return;

    if (TERMINAL[run.status]) {
      finishRun(run);
      return;
    }

    // No new step for a while: the model is probably thinking.
    if (Date.now() - app.lastStepAt > 1600) {
      setActivityStatus("Thinking", true);
      setChatTitle(undefined, "Thinking…");
    }
  }

  function finishRun(run) {
    app.running = false;
    clearInterval(app.pollTimer);
    app.pollTimer = null;

    $("send-btn").disabled = false;
    $("stop-btn").classList.add("hidden");

    const steps = app.steps || [];
    const usedBrowser = steps.some(function (s) {
      return s.action !== "chat" && s.action !== "browser_reuse";
    });

    if (!usedBrowser) {
      // A normal chat answer does not need the activity card.
      removeActivity();
    } else {
      finishActivity(run.status, "");
    }

    const output = String(run.output || "").trim();

    if (output.indexOf(SAFETY_PREFIX) === 0) {
      addMessage("assistant",
        "I stopped before a risky action. Please confirm if I should continue.",
        { plain: true });
      askSafety(output.slice(SAFETY_PREFIX.length).trim());
    } else if (output) {
      addMessage("assistant", output);
    } else if (run.status === "failed") {
      addMessage("assistant", "I could not finish that task. Please try again or give me a smaller step.");
    } else if (run.status === "stopped") {
      addMessage("assistant", "Stopped. Tell me when you want me to continue.");
    }

    if (run.status === "completed") {
      setChatTitle(undefined, usedBrowser ? "Task completed" : "Ready");
    } else if (run.status === "stopped") {
      setChatTitle(undefined, "Stopped");
    } else {
      setChatTitle(undefined, "The last run failed");
    }

    app.runId = null;
    scrollToBottom(true);
    loadSessions();
    refreshBrowserStatus();
  }

  function poll() {
    clearInterval(app.pollTimer);
    app.pollTimer = setInterval(async function () {
      if (!app.running || !app.sessionId || !app.runId) return;
      try {
        const run = await API.getRun(app.sessionId, app.runId);
        updateFromRun(run);
      } catch (e) {
        // Keep polling: a temporary network problem should not kill the run.
        setActivityStatus("Reconnecting", true);
      }
    }, 700);
  }

  async function sendMessage(message) {
    message = String(message || "").trim();
    if (!message || app.running) return;

    try {
      if (!app.sessionId) app.sessionId = await ensureSession();
      rememberTitle(app.sessionId, message);
    } catch (e) {
      addMessage("assistant", "I cannot reach the backend at " + API.baseUrl() +
        ". Start the server, then try again.");
      return;
    }

    addMessage("user", message);
    startRunUi();
    setChatTitle(titleForSession({ sessionID: app.sessionId, title: "" }), "Sending…");
    scrollToBottom(true);

    try {
      const res = await API.createRun(app.sessionId, message);
      app.runId = res.runID;
      setChatTitle(undefined, "Working…");
      poll();
    } catch (e) {
      app.running = false;
      $("send-btn").disabled = false;
      $("stop-btn").classList.add("hidden");
      finishActivity("failed", e.message);
      addMessage("assistant", "I could not start that run: " + e.message);
    }

    loadSessions();
  }

  async function stopTask() {
    if (!app.running || !app.sessionId || !app.runId) return;
    $("stop-btn").classList.add("hidden");
    setActivityStatus("Stopping", true);
    try {
      await API.stopRun(app.sessionId, app.runId);
    } catch (e) {
      setActivityStatus("Could not stop", false);
      addActivityLine(e.message, true);
    }
  }

  /* ============================================================
     SAFETY CONFIRMATION
     ============================================================ */

  function askSafety(message) {
    UI.openDialog({
      title: "Confirm action",
      message: message || "The agent wants to do something that may be irreversible.",
      blocking: true,
      buttons: [
        {
          label: "Continue",
          primary: true,
          onClick: function () {
            sendMessage("USER CONFIRMED: I approve this action. Continue the task.");
          }
        },
        {
          label: "Cancel",
          onClick: function () {
            sendMessage("CANCELLED: Do not perform that action. Finish safely without it.");
          }
        }
      ]
    });
  }

  /* ============================================================
     SIDEBAR / HISTORY
     ============================================================ */

  async function loadSessions() {
    try {
      const data = await API.getSessions();
      app.sessions = (data.sessions || []).slice().reverse();
      renderSessions();
    } catch (e) {
      const list = $("session-list");
      list.innerHTML = "";
      const empty = document.createElement("div");
      empty.className = "session-empty";
      empty.textContent = "History is offline";
      list.appendChild(empty);
    }
  }

  function renderSessions() {
    const list = $("session-list");
    list.innerHTML = "";

    if (!app.sessions.length) {
      const empty = document.createElement("div");
      empty.className = "session-empty";
      empty.textContent = "No chats yet";
      list.appendChild(empty);
      return;
    }

    app.sessions.forEach(function (session) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "session-item" +
        (session.sessionID === app.sessionId ? " is-active" : "");

      const title = document.createElement("span");
      title.className = "session-title";
      title.textContent = titleForSession(session);

      const meta = document.createElement("span");
      meta.className = "session-meta";
      meta.textContent = session.runCount + (session.runCount === 1 ? " message" : " messages");

      button.append(title, meta);
      button.addEventListener("click", function () {
        if (app.running) {
          UI.openDialog({
            title: "A task is running",
            message: "Stop the current task before opening another chat."
          });
          return;
        }
        loadSession(session.sessionID);
      });

      list.appendChild(button);
    });
  }

  async function ensureSession() {
    const saved = Number(localStorage.getItem("automata.session")) || 0;
    if (saved) {
      try {
        const session = await API.getSession(saved);
        if (session && session.sessionID) return session.sessionID;
      } catch (e) {
        // Saved session is gone (ya app ka data folder badal gaya).
      }
    }

    // Chat yaad rakhne ke liye: agar saved session nahi mila to sab se
    // recent session hi reuse karo. Naya chat sirf tab banao jab koi
    // session mojood na ho. Is se "agent chat bhool gaya" wala masla
    // restart ke baad bhi nahi hota.
    try {
      const data = await API.getSessions();
      const list = Array.isArray(data && data.sessions) ? data.sessions.slice() : [];
      if (list.length) {
        let newest = list[0];
        list.forEach(function (session) {
          if (session.sessionID > newest.sessionID) newest = session;
        });
        localStorage.setItem("automata.session", newest.sessionID);
        return newest.sessionID;
      }
    } catch (e) {
      // List bhi nahi mili: neeche naya session bana do.
    }

    return createSession();
  }

  async function createSession() {
    const created = await API.createSession("New chat");
    localStorage.setItem("automata.session", created.sessionID);
    return created.sessionID;
  }

  async function newChat() {
    if (app.running) {
      UI.openDialog({
        title: "A task is running",
        message: "Stop the current task before starting a new chat."
      });
      return;
    }
    try {
      const id = await createSession();
      app.sessionId = id;
      app.runId = null;
      $("messages").innerHTML = "";
      setHeroVisible(true);
      setChatTitle("New chat", "Ask anything, or give a browser task");
      $("message-input").value = "";
      $("message-input").focus();
      await loadSessions();
      renderSessions();
    } catch (e) {
      UI.openDialog({ title: "Error", message: e.message });
    }
  }

  async function loadSession(sessionId) {
    clearInterval(app.pollTimer);
    app.pollTimer = null;
    app.running = false;

    app.sessionId = sessionId;
    localStorage.setItem("automata.session", sessionId);
    app.runId = null;
    app.steps = [];
    app.lastStepCount = -1;
    app.activeActivity = null;

    $("messages").innerHTML = "";
    $("stop-btn").classList.add("hidden");
    $("send-btn").disabled = false;

    let session;
    try {
      session = await API.getSession(sessionId);
    } catch (e) {
      addMessage("assistant", "I could not load that chat: " + e.message);
      return;
    }

    const runs = Array.isArray(session.runs) ? session.runs : [];

    if (!runs.length) {
      setHeroVisible(true);
      setChatTitle("New chat", "Ask anything, or give a browser task");
      renderSessions();
      return;
    }

    setHeroVisible(false);

    let lastRun = null;
    runs.forEach(function (run) {
      if (run.message) addMessage("user", run.message);
      if (run.output) addMessage("assistant", run.output);
      lastRun = run;
    });

    setChatTitle(titleForSession(session),
      lastRun ? String(lastRun.status) : "Ready");

    // If the app was closed while a run was still going, resume watching it.
    if (lastRun && (lastRun.status === "running" || lastRun.status === "queued")) {
      app.running = true;
      app.runId = lastRun.runID;
      app.steps = Array.isArray(lastRun.steps) ? lastRun.steps : [];
      app.lastStepCount = app.steps.length;
      app.lastStepAt = Date.now();

      $("send-btn").disabled = true;
      $("stop-btn").classList.remove("hidden");
      createActivity();
      app.steps.forEach(function (step) {
        const described = describeStep(step);
        if (described[0]) addActivityLine(described[0], described[1]);
      });
      setActivityStatus("Resuming", true);
      poll();
    }

    renderSessions();
    scrollToBottom(true);
  }

  /* ============================================================
     BROWSER STATUS / SETTINGS / WINDOW
     ============================================================ */

  async function refreshBrowserStatus() {
    const pill = $("browser-pill");
    const text = $("browser-text");
    try {
      const result = await API.browserStatus();
      const on = !!(result && result.running);
      pill.classList.toggle("is-on", on);
      text.textContent = on ? "Browser running" : "Browser off";
      pill.title = on ? "Chrome is connected" : "No browser is open";
    } catch (e) {
      pill.classList.remove("is-on");
      text.textContent = "Browser off";
    }
  }

  function confirmCloseBrowser() {
    UI.openDialog({
      title: "Close browser",
      message: "Close the Chrome window the agent is using?\n\nAny running browser task will fail.",
      blocking: true,
      buttons: [
        {
          label: "Close it",
          primary: true,
          onClick: async function () {
            try {
              const result = await API.closeBrowser();
              if (!result.ok) addSystemNote("The browser was not running.");
            } catch (e) {
              addSystemNote("Could not close the browser: " + e.message);
            }
            refreshBrowserStatus();
          }
        },
        { label: "Cancel" }
      ]
    });
  }

  function openSettings() {
    UI.openDialog({
      title: "Settings",
      message: "Where the Automata backend listens.",
      input: { value: API.baseUrl(), placeholder: "http://127.0.0.1:8080" },
      buttons: [
        { label: "Cancel" },
        {
          label: "Save",
          primary: true,
          onClick: function (value) {
            API.setBaseUrl((value || "").trim() || "http://127.0.0.1:8080");
            addSystemNote("Backend address saved.");
          }
        }
      ]
    });
  }

  function wireWindowControls() {
    const win = window.automataWindow;
    if (!win) return;
    $("btn-min").addEventListener("click", function () { win.minimize(); });
    $("btn-max").addEventListener("click", function () { win.maximize(); });
    $("btn-quit").addEventListener("click", function () { win.quit(); });
  }

  function setSidebarOpen(open) {
    app.sidebarOpen = !!open;
    const sidebar = $("sidebar");
    if (sidebar) sidebar.classList.toggle("is-closed", !app.sidebarOpen);
  }

  /* ============================================================
     COMPOSER
     ============================================================ */

  function resizeComposer() {
    const input = $("message-input");
    input.style.height = "auto";
    input.style.height = Math.min(input.scrollHeight, 168) + "px";
  }

  function wireComposer() {
    const form = $("composer");
    const input = $("message-input");

    form.addEventListener("submit", function (e) {
      e.preventDefault();
      const message = input.value.trim();
      if (!message || app.running) return;
      input.value = "";
      resizeComposer();
      sendMessage(message);
    });

    input.addEventListener("input", resizeComposer);
    input.addEventListener("keydown", function (e) {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        form.requestSubmit();
      }
    });

    $("stop-btn").addEventListener("click", stopTask);

    document.querySelectorAll(".chip").forEach(function (chip) {
      chip.addEventListener("click", function () {
        input.value = chip.getAttribute("data-prompt") || "";
        resizeComposer();
        form.requestSubmit();
      });
    });
  }

  /* ============================================================
     BOOT
     ============================================================ */

  async function boot() {
    $("brand-logo").appendChild(UI.Logo(38));
    $("hero-logo").appendChild(UI.Logo(92));

    setSidebarOpen(false);
    $("toggle-sidebar").addEventListener("click", function () {
      setSidebarOpen(!app.sidebarOpen);
    });

    wireWindowControls();
    wireComposer();
    resizeComposer();

    $("new-chat").addEventListener("click", newChat);
    $("btn-close-browser").addEventListener("click", confirmCloseBrowser);
    $("browser-pill").addEventListener("click", refreshBrowserStatus);
    $("btn-settings").addEventListener("click", openSettings);

    setChatTitle("New chat", "Ask anything, or give a browser task");

    try {
      // User want: har app launch par NAYI chat se shuru karna hai.
      // Isliye saved session nahi use karte — brand new session banate hain.
      localStorage.removeItem("automata.session");
      app.sessionId = await createSession();
      app.runId = null;
      app.running = false;
      app.steps = [];
      app.lastStepCount = -1;
      app.activeActivity = null;
      $("messages").innerHTML = "";
      setHeroVisible(true);
      setChatTitle("New chat", "Ask anything, or give a browser task");
      $("message-input").value = "";
      await loadSessions();
      await loadSession(app.sessionId);
    } catch (e) {
      addMessage("assistant",
        "I cannot reach the backend at " + API.baseUrl() +
        ". Start the server and refresh this window.");
      setChatTitle("Backend offline", e.message);
    }

    refreshBrowserStatus();
    clearInterval(app.browserTimer);
    app.browserTimer = setInterval(refreshBrowserStatus, 5000);

    $("message-input").focus();
  }

  document.addEventListener("DOMContentLoaded", boot);
})();

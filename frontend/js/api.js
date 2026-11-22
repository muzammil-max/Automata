/* ============================================================
   API — thin wrapper around the Automata backend (C++ server).
   Base URL is stored in localStorage (Settings screen).
   ============================================================ */

(function () {
  "use strict";

  function baseUrl() {
    return localStorage.getItem("automata.api") || "http://127.0.0.1:8080";
  }

  function setBaseUrl(value) {
    localStorage.setItem("automata.api", value.replace(/\/+$/, ""));
  }

  async function request(method, path, body) {
    const opts = { method: method, headers: {} };
    if (body !== undefined) {
      opts.headers["Content-Type"] = "application/json";
      opts.body = JSON.stringify(body);
    }
    const res = await fetch(baseUrl() + path, opts);
    let data = null;
    try { data = await res.json(); } catch (e) { data = { error: "Bad response" }; }
    if (!res.ok) {
      const err = new Error((data && data.error) || ("HTTP " + res.status));
      err.status = res.status;
      throw err;
    }
    return data;
  }

  window.AutomataAPI = {
    baseUrl: baseUrl,
    setBaseUrl: setBaseUrl,
    health: () => request("GET", "/health"),
    createSession: (title) => request("POST", "/api/sessions", { title: title }),
    getSessions: () => request("GET", "/api/sessions"),
    getSession: (id) => request("GET", "/api/sessions/" + id),
    createRun: (sessionId, message) =>
      request("POST", "/api/sessions/" + sessionId + "/runs", { message: message }),
    getRun: (sessionId, runId) =>
      request("GET", "/api/sessions/" + sessionId + "/runs/" + runId),
    stopRun: (sessionId, runId) =>
      request("POST", "/api/sessions/" + sessionId + "/runs/" + runId + "/stop"),
    closeBrowser: () => request("POST", "/api/browser/close"),
    browserStatus: () => request("GET", "/api/browser/status")
  };
})();

/* ============================================================
   AUTOMATA — BOOT SPLASH
   Shows for at least 10 seconds, then the Electron main process
   swaps to the chat page. If the page is opened in a normal browser,
   this script also navigates to "/" after 10 seconds.

   The splash has no progress bar. It only cycles through three
   plain status lines: browser assets, model, getting ready.
   ============================================================ */

(function () {
  "use strict";

  const UI = window.AutomataUI;
  const API = window.AutomataAPI;

  const MIN_SPLASH_MS = 10000;
  const PHASES = [
    { at: 0, text: "Loading browser assets" },
    { at: 3400, text: "Starting model" },
    { at: 6800, text: "Getting ready" }
  ];

  const startedAt = Date.now();
  const IS_LOCAL_FILE = window.location.protocol === "file:";

  let healthy = false;
  let checking = false;
  let lastCheck = 0;
  let finished = false;

  function $(id) { return document.getElementById(id); }

  function buildSplash() {
    const logoBox = $("splash-logo");
    // Logo wahi hai, sirf bara kiya hai taake splash scale ke saath match kare.
    if (logoBox) logoBox.appendChild(UI.Logo(68));

    tick();
    setInterval(tick, 100);
  }

  async function checkHealth() {
    if (checking || Date.now() - lastCheck < 700) return;
    checking = true;
    lastCheck = Date.now();
    try {
      const result = await API.health();
      healthy = !!(result && result.ok);
    } catch (e) {
      healthy = false;
    }
    checking = false;
  }

  function phaseFor(elapsed) {
    let text = PHASES[0].text;
    for (const phase of PHASES) {
      if (elapsed >= phase.at) text = phase.text;
    }
    return text;
  }

  function tick() {
    if (finished) return;

    const statusEl = $("splash-status-text");
    const elapsed = Date.now() - startedAt;

    // Electron loads this file directly from disk so the splash appears even
    // before the backend is alive. Do not bother with a health request there.
    if (IS_LOCAL_FILE) {
      healthy = true;
    } else {
      checkHealth();
    }

    if (elapsed < MIN_SPLASH_MS) {
      if (statusEl) statusEl.textContent = phaseFor(elapsed);
      return;
    }

    if (!healthy) {
      if (statusEl) statusEl.textContent = "Waiting for backend";
      return;
    }

    // 10 seconds are over and the backend is alive.
    finished = true;
    if (statusEl) statusEl.textContent = "Ready";

    // In Electron, main.js swaps the page. In a normal browser, do it here.
    if (!window.automataWindow) {
      setTimeout(function () {
        location.replace(API.baseUrl() + "/");
      }, 300);
    }
  }

  document.addEventListener("DOMContentLoaded", buildSplash);
})();

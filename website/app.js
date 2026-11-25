/* ============================================================
 A UTOMATA — LANDING PAGE LOGIC               *
 ------------------------------------------------------------
 1. Draws the pixel cursor
 2. Plays the music files in music/ one after another
 3. Starfield
 4. Random sleep
 5. Random language shift
 ============================================================ */

/* ------------------------------------------------------------
 1 . PIXEL CURSOR                             *
 ------------------------------------------------------------ */

const GRID_W = 12;
const GRID_H = 18;

const CURSOR_POINTS = [
  [0, 0],
[0, 16],
[4, 12],
[6.5, 17],
[9.5, 15.5],
[7, 10.5],
[12, 10.5]
];

function insidePolygon(x, y, points) {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const [xi, yi] = points[i];
    const [xj, yj] = points[j];
    const crosses = (yi > y) !== (yj > y);
    if (crosses && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) {
      inside = !inside;
    }
  }
  return inside;
}

function drawCursor() {
  const grid = document.getElementById("pixel-grid");
  if (!grid) return;

  const filled = [];
  for (let y = 0; y < GRID_H; y++) {
    filled[y] = [];
    for (let x = 0; x < GRID_W; x++) {
      filled[y][x] = insidePolygon(x + 0.5, y + 0.5, CURSOR_POINTS);
    }
  }

  const isFilled = (x, y) =>
  x >= 0 && y >= 0 && x < GRID_W && y < GRID_H && filled[y][x];

  const cells = [];
  for (let y = 0; y < GRID_H; y++) {
    for (let x = 0; x < GRID_W; x++) {
      if (!filled[y][x]) {
        cells.push('<span class="pixel"></span>');
        continue;
      }

      const edge =
      !isFilled(x - 1, y) ||
      !isFilled(x + 1, y) ||
      !isFilled(x, y - 1) ||
      !isFilled(x, y + 1);

      cells.push(
        `<span class="pixel ${edge ? "pixel--outline" : "pixel--fill"}"></span>`
      );
    }
  }

  grid.innerHTML = cells.join("");
}

/* ------------------------------------------------------------
 2 . MUSIC PLAYER                             *
 ------------------------------------------------------------
 Add songs as music/1.mp3, music/2.mp3 ...
 OR list them in music/playlist.json (an array of filenames).
 They play in order, then loop.

 Autoplay: we try to play immediately. If the browser blocks
 it (most do for first-time visitors), the music starts on the
 very first click / tap / key press anywhere on the page.
 ------------------------------------------------------------ */

const MAX_AUTO_TRACKS = 20;

const audio = document.getElementById("audio");
const musicBtn = document.getElementById("music-btn");
const trackTitle = document.getElementById("track-title");
const trackSub = document.getElementById("track-sub");
const equalizer = document.getElementById("equalizer");
const iconPlay = musicBtn ? musicBtn.querySelector(".icon-play") : null;
const iconPause = musicBtn ? musicBtn.querySelector(".icon-pause") : null;

let playlist = [];
let trackIndex = 0;
let playing = false;
let unlockArmed = false;

function prettyName(file) {
  const base = decodeURIComponent(file.split("/").pop() || file);
  return (
    base
    .replace(/\.[a-z0-9]+$/i, "")
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase()) || "Untitled"
  );
}

async function fileExists(url) {
  try {
    const res = await fetch(url, { method: "HEAD", cache: "no-store" });
    return res.ok;
  } catch (e) {
    return false;
  }
}

async function buildPlaylist() {
  const tracks = [];

  // A) playlist.json
  try {
    const res = await fetch("music/playlist.json", { cache: "no-store" });
    if (res.ok) {
      const data = await res.json();
      const list = Array.isArray(data) ? data : data && data.playlist;
      if (Array.isArray(list)) {
        for (const item of list) {
          if (typeof item === "string" && item.trim()) {
            const file = item.trim();
            tracks.push(file.startsWith("music/") ? file : "music/" + file);
          }
        }
      }
    }
  } catch (e) {
    // fall back to numbered files
  }

  if (tracks.length > 0) return tracks;

  // B) probe music/1.mp3 ... music/20.mp3
  const probes = [];
  for (let i = 1; i <= MAX_AUTO_TRACKS; i++) {
    const url = `music/${i}.mp3`;
    probes.push(fileExists(url).then((ok) => (ok ? url : null)));
  }

  const found = await Promise.all(probes);
  for (const url of found) {
    if (url) tracks.push(url);
  }

  return tracks;
}

function setPlayingUI(isPlaying) {
  playing = isPlaying;
  if (equalizer) equalizer.classList.toggle("is-on", isPlaying);
  if (iconPlay) iconPlay.classList.toggle("hidden", isPlaying);
  if (iconPause) iconPause.classList.toggle("hidden", !isPlaying);
  if (musicBtn) {
    musicBtn.setAttribute("aria-label", isPlaying ? "Pause music" : "Play music");
  }
}

function showTrack() {
  if (!playlist.length) {
    trackTitle.textContent = "No music yet";
    trackSub.textContent = "Add music/1.mp3 or music/playlist.json";
    return;
  }
  trackTitle.textContent = prettyName(playlist[trackIndex]);
  trackSub.textContent = `Track ${trackIndex + 1} of ${playlist.length}`;
}

// Wait for the first real user gesture, then start the music.
function armUnlock() {
  if (unlockArmed) return;
  unlockArmed = true;

  const events = ["click", "keydown", "touchend", "pointerup"];
  const go = () => {
    events.forEach((e) => document.removeEventListener(e, go, true));
    unlockArmed = false;
    if (!playing) playCurrent();
  };
    events.forEach((e) => document.addEventListener(e, go, true));
}

async function playCurrent() {
  if (!playlist.length) return;

  audio.src = playlist[trackIndex];
  audio.muted = false;
  showTrack();

  try {
    await audio.play();
    setPlayingUI(true);
  } catch (e) {
    // Autoplay blocked: start on the first tap / click / key.
    setPlayingUI(false);
    trackSub.textContent = "Tap anywhere to start the music";
    armUnlock();
  }
}

function nextTrack() {
  if (!playlist.length) return;
  trackIndex = (trackIndex + 1) % playlist.length;
  playCurrent();
}

async function toggleMusic() {
  if (!playlist.length) return;

  if (playing) {
    audio.pause();
    setPlayingUI(false);
    trackSub.textContent = "Paused";
    return;
  }

  // Resume if a song was already loaded, otherwise start fresh.
  if (audio.src) {
    try {
      await audio.play();
      setPlayingUI(true);
      showTrack();
      return;
    } catch (e) {
      // fall through to restart
    }
  }
  playCurrent();
}

/* ------------------------------------------------------------
 H TML PREVIEW (optional overlay)             *
 ------------------------------------------------------------ */

function openHtmlPreview() {
  const overlay = document.getElementById("htmlPreviewOverlay");
  const code = document.getElementById("htmlPreviewCode");
  if (!overlay || !code) return;

  fetch("index.html")
  .then((r) => r.text())
  .then((text) => {
    // textContent is already safe, no manual escaping needed.
    code.textContent = text;
  })
  .catch(() => {
    code.textContent = "<html></html>";
  });

  overlay.hidden = false;
  overlay.classList.add("open");
}

function closeHtmlPreview() {
  const overlay = document.getElementById("htmlPreviewOverlay");
  if (!overlay) return;
  overlay.classList.remove("open");
  overlay.hidden = true;
}

function wireHtmlPreview() {
  const fab = document.getElementById("htmlPreviewFab");
  const closeBtn = document.getElementById("htmlPreviewClose");
  const copyBtn = document.getElementById("htmlPreviewCopy");

  if (fab) fab.addEventListener("click", openHtmlPreview);
  if (closeBtn) closeBtn.addEventListener("click", closeHtmlPreview);

  if (copyBtn) {
    copyBtn.addEventListener("click", async () => {
      const code = document.getElementById("htmlPreviewCode");
      if (!code) return;
      const raw = await fetch("index.html")
      .then((r) => r.text())
      .catch(() => code.textContent);
      try {
        const blob = new Blob([raw], { type: "text/html" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = "index.html";
        a.click();
        setTimeout(() => URL.revokeObjectURL(url), 4000);
      } catch (e) {
        /* ignore */
      }
    });
  }

  const overlay = document.getElementById("htmlPreviewOverlay");
  if (overlay) {
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) closeHtmlPreview();
    });
  }
}

function wireMusic() {
  if (!audio) return;

  audio.addEventListener("ended", nextTrack);

  // A broken file should not stop the whole loop.
  audio.addEventListener("error", () => {
    if (!playlist.length) return;
    if (trackIndex + 1 < playlist.length) {
      trackIndex++;
      playCurrent();
    } else {
      setPlayingUI(false);
      trackSub.textContent = "Could not play the music files";
    }
  });

  if (musicBtn) musicBtn.addEventListener("click", toggleMusic);

  buildPlaylist().then((tracks) => {
    playlist = tracks;
    trackIndex = 0;
    showTrack();
    if (playlist.length) playCurrent();
  });
}

/* ------------------------------------------------------------
 3 . STARFIELD                                *
 ------------------------------------------------------------ */

function drawStars() {
  const holder = document.getElementById("stars");
  if (!holder) return;

  const count = 140;
  const frag = document.createDocumentFragment();

  for (let i = 0; i < count; i++) {
    const star = document.createElement("span");
    star.className = "star" + (Math.random() < 0.12 ? " star--bright" : "");

    const size =
    Math.random() < 0.85 ? 1 + Math.random() * 1.6 : 2 + Math.random() * 1.8;

    star.style.left = (Math.random() * 100).toFixed(2) + "%";
    star.style.top = (Math.random() * 100).toFixed(2) + "%";
    star.style.setProperty("--s", size.toFixed(2) + "px");
    star.style.setProperty("--d", (2.6 + Math.random() * 4).toFixed(2) + "s");
    star.style.setProperty("--delay", (-Math.random() * 6).toFixed(2) + "s");

    frag.appendChild(star);
  }

  holder.appendChild(frag);
}

/* ------------------------------------------------------------
 4 . RANDOM SLEEP                             *
 ------------------------------------------------------------ */

const cursorWrap = document.getElementById("cursor-wrap");
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const randomBetween = (min, max) => min + Math.random() * (max - min);

let sleeping = false;

function scheduleSleep() {
  setTimeout(async () => {
    if (sleeping || !cursorWrap) {
      scheduleSleep();
      return;
    }

    sleeping = true;
    cursorWrap.classList.add("is-sleeping");

    await wait(randomBetween(3500, 5500));

    cursorWrap.classList.remove("is-sleeping");
    sleeping = false;
    scheduleSleep();
  }, randomBetween(9000, 18000));
}

/* ------------------------------------------------------------
 5 . RANDOM LANGUAGE SHIFT                    *
 ------------------------------------------------------------ */

const wordmark = document.getElementById("wordmark");

const LANGUAGES = [
  { text: "آٹوماٹا", cls: "is-arabic", rtl: true },     // Urdu
{ text: "自动机", cls: "is-chinese", rtl: false },     // Chinese
{ text: "Autómata", cls: "is-spanish", rtl: false },   // Spanish
{ text: "オートマタ", cls: "is-japanese", rtl: false }  // Japanese
];

let translating = false;
let lastLanguage = -1;

async function translateOnce() {
  if (translating || !wordmark) return;
  translating = true;

  let index = Math.floor(Math.random() * LANGUAGES.length);
  if (index === lastLanguage) index = (index + 1) % LANGUAGES.length;
  lastLanguage = index;

  const language = LANGUAGES[index];

  wordmark.classList.add("is-swapping");
  await wait(380);

  wordmark.textContent = language.text;
  wordmark.className = "wordmark " + language.cls;
  wordmark.dir = language.rtl ? "rtl" : "ltr";
  void wordmark.offsetWidth;
  wordmark.classList.remove("is-swapping");

  await wait(randomBetween(4000, 5000));

  wordmark.classList.add("is-swapping");
  await wait(380);

  wordmark.className = "wordmark";
  wordmark.textContent = wordmark.dataset.en || "Automata";
  wordmark.dir = "ltr";
  void wordmark.offsetWidth;
  wordmark.classList.remove("is-swapping");

  translating = false;
}

function scheduleTranslation() {
  setTimeout(async () => {
    await translateOnce();
    scheduleTranslation();
  }, randomBetween(7000, 14000));
}

/* ------------------------------------------------------------ */

function init() {
  drawCursor();
  drawStars();
  wireMusic();
  wireHtmlPreview();
  wireHtmlPreview();

  // Motion-sensitive visitors keep a calm, still page.
  if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    scheduleSleep();
    scheduleTranslation();
  }
}

init();

// Debug hooks: index.html#sleep or index.html#lang
if (cursorWrap && location.hash === "#sleep") cursorWrap.classList.add("is-sleeping");
if (location.hash === "#lang") translateOnce();

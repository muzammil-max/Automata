/* ============================================================
   AUTOMATA — Electron main process
   Opens the UI as its own desktop window (not a browser tab).
   Starts the C++ backend if it is not already running, then
   loads the UI from the backend address so there is one origin.
   ============================================================ */

const { app, BrowserWindow, ipcMain, Menu, screen } = require("electron");
const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

const API_BASE = "http://127.0.0.1:8080";
let mainWindow = null;
let backendProcess = null;

/* ---- backend health ---- */
async function healthy() {
  try {
    const res = await fetch(API_BASE + "/health", { signal: AbortSignal.timeout(1500) });
    return res.ok;
  } catch (e) {
    return false;
  }
}

/* ---- start the C++ server if it is not up ---- */
async function ensureBackend() {
  if (await healthy()) return true;

  // Packaged app mein backend "resources/backend/api_server(.exe)" hota hai,
  // dev mein project ke backend/ folder mein. Dono jagah dhoondo.
  const exeName = process.platform === "win32" ? "api_server.exe" : "api_server";
  const candidates = [
    process.env.AUTOMATA_BACKEND,
    app.isPackaged ? path.join(process.resourcesPath, "backend", exeName) : null,
    path.join(__dirname, "..", "backend", exeName)
  ].filter(Boolean);

  const binary = candidates.find((p) => fs.existsSync(p));
  if (binary) {
    try {
      // Packaged app ko apne folder mein likhne ki ijazat nahi hoti, is liye
      // sessions userData folder mein rakhte hain.
      const backendCwd = app.isPackaged ? app.getPath("userData") : path.dirname(binary);
      fs.mkdirSync(backendCwd, { recursive: true });

      backendProcess = spawn(binary, [], {
        cwd: backendCwd,
        stdio: "inherit",
        windowsHide: true,
        env: Object.assign({}, process.env, {
          // Backend ko batao frontend kahan hai (packaged app ke liye zaruri).
          AUTOMATA_FRONTEND_DIR: __dirname
        })
      });
      backendProcess.on("error", (e) => console.error("backend spawn:", e.message));
    } catch (e) {
      console.error("could not start backend:", e.message);
    }
  } else {
    console.error("backend binary not found; start it manually");
  }

  // Wait up to 6 seconds for it to answer.
  for (let i = 0; i < 30; i++) {
    if (await healthy()) return true;
    await new Promise((r) => setTimeout(r, 200));
  }
  return false;
}

/* ---- Windows: ChromeDriver ko khud start karo ---- */
async function ensureChromedriver() {
  if (process.platform !== "win32") return;

  try {
    const res = await fetch("http://127.0.0.1:9515/status", {
      signal: AbortSignal.timeout(800)
    });
    if (res.ok) return;
  } catch (e) {
    // neeche start karne ki koshish karo
  }

  const candidates = [
    process.env.CHROMEDRIVER_PATH,
    app.isPackaged ? path.join(process.resourcesPath, "driver", "chromedriver.exe") : null,
    path.join(__dirname, "..", "driver", "chromedriver.exe")
  ].filter(Boolean);

  const binary = candidates.find((p) => fs.existsSync(p));
  if (!binary) {
    console.error("chromedriver.exe not found; browser tasks will not work");
    return;
  }

  try {
    const driver = spawn(binary, ["--port=9515"], {
      stdio: "ignore",
      windowsHide: true
    });
    driver.on("error", (e) => console.error("chromedriver spawn:", e.message));
  } catch (e) {
    console.error("could not start chromedriver:", e.message);
  }
}

function createWindow() {
  mainWindow = new BrowserWindow({
    // Splash thoda bara hai taake logo aur text saans le sakein.
    width: 760,
    height: 480,
    minWidth: 760,
    minHeight: 480,
    center: true,
    resizable: false,
    maximizable: false,
    fullscreenable: false,
    frame: false,                 // the UI draws its own title bar
    show: false,                  // wait until the local splash is ready
    backgroundColor: "#00000000",
    icon: path.join(__dirname, "icon.png"),
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  mainWindow.setMenuBarVisibility(false);

  // IMPORTANT: load the splash from disk, not from the backend.
  // The backend may still be starting, and loading "/boot" too early shows
  // only the empty window background. This local file always renders.
  mainWindow.loadFile(path.join(__dirname, "boot.html")).catch(function (e) {
    console.error("splash load failed:", e.message);
    if (mainWindow) mainWindow.show();
  });

  mainWindow.once("ready-to-show", function () {
    if (mainWindow) {
      mainWindow.center();
      mainWindow.show();
    }
  });

  mainWindow.on("closed", function () {
    mainWindow = null;
  });
}

/* ---- window controls from the UI ---- */
ipcMain.on("window:minimize", () => { if (mainWindow) mainWindow.minimize(); });
ipcMain.on("window:maximize", () => {
  if (!mainWindow) return;
  if (mainWindow.isMaximized()) mainWindow.unmaximize();
  else mainWindow.maximize();
});
ipcMain.on("window:quit", () => app.quit());

app.whenReady().then(async function () {
  Menu.setApplicationMenu(null);
  app.setName("Automata");

  const startedAt = Date.now();
  createWindow();

  // Windows par ChromeDriver bhi khud start karo (agar mojood ho).
  ensureChromedriver();

  // Boot splash is on screen (createWindow loads boot.html from disk).
  // Wait for the backend AND for at least 10 seconds, then swap to the app.
  const up = await ensureBackend();
  const elapsed = Date.now() - startedAt;
  const remaining = Math.max(0, 10000 - elapsed);

  setTimeout(function () {
    if (!mainWindow) return;

    // Grow the small splash into a compact chat window. It should feel like
    // a desktop app, not a full-screen takeover.
    try {
      const area = screen.getPrimaryDisplay().workAreaSize;
      const width = Math.max(760, Math.min(1040, Math.floor(area.width * 0.70), area.width - 96));
      const height = Math.max(540, Math.min(720, Math.floor(area.height * 0.80), area.height - 96));

      mainWindow.setResizable(true);
      mainWindow.setMaximizable(true);
      mainWindow.setMinimumSize(760, 540);
      mainWindow.setSize(width, height);
      mainWindow.center();
    } catch (e) {
      console.error("could not resize window:", e.message);
    }

    mainWindow.loadURL(API_BASE + "/");
    if (!up) console.error("backend did not answer; showing the UI anyway");
  }, remaining);

  app.on("activate", function () {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", function () {
  if (process.platform !== "darwin") app.quit();
});

// Leaving the app should not leave a stray backend behind
// (only if we started it).
app.on("quit", function () {
  if (backendProcess && !backendProcess.killed) {
    try { backendProcess.kill(); } catch (e) { /* already gone */ }
  }
});

/* Preload: expose only the window controls the UI needs. */
const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("automataWindow", {
  minimize: () => ipcRenderer.send("window:minimize"),
  maximize: () => ipcRenderer.send("window:maximize"),
  quit: () => ipcRenderer.send("window:quit"),
  platform: process.platform
});

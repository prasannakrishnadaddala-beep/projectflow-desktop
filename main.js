/**
 * ProjectFlow Desktop — main.js
 * Connects to a remote Flask backend deployed via GitLab CI/CD.
 */

const { app, BrowserWindow, shell, ipcMain, dialog, Menu } = require('electron');
const path  = require('path');
const http  = require('http');
const https = require('https');
const urlMod = require('url');
const fs    = require('fs');

// ── Backend URL resolution ────────────────────────────────────────────────────
let BACKEND_URL = process.env.PROJECTFLOW_URL || process.env.BACKEND_URL || '';

// In packaged app, read from build-config.json baked in by CI
if (!BACKEND_URL && app.isPackaged) {
  try {
    const cfg = JSON.parse(fs.readFileSync(path.join(__dirname, 'build-config.json'), 'utf8'));
    BACKEND_URL = cfg.backendUrl || '';
  } catch (_) {}
}
if (!BACKEND_URL) BACKEND_URL = 'https://projectflow.example.com';
BACKEND_URL = BACKEND_URL.replace(/\/$/, '');

console.log(`[Electron] Backend URL: ${BACKEND_URL}`);

let mainWindow  = null;
let splashWindow = null;
let isQuitting  = false;

// ── Wait for remote server ────────────────────────────────────────────────────
function waitForServer(targetUrl, retries = 15, delay = 1500) {
  return new Promise((resolve, reject) => {
    const parsed    = urlMod.parse(targetUrl);
    const requester = parsed.protocol === 'https:' ? https : http;
    const attempt = (n) => {
      if (n <= 0) return reject(new Error(`Server at ${targetUrl} did not respond`));
      const req = requester.get(targetUrl + '/', (res) => {
        if (res.statusCode < 500) resolve();
        else setTimeout(() => attempt(n - 1), delay);
      });
      req.on('error', () => setTimeout(() => attempt(n - 1), delay));
      req.setTimeout(2000, () => { req.destroy(); setTimeout(() => attempt(n - 1), delay); });
    };
    attempt(retries);
  });
}

// ── Splash ────────────────────────────────────────────────────────────────────
function createSplash() {
  splashWindow = new BrowserWindow({
    width: 420, height: 280, frame: false, transparent: true,
    alwaysOnTop: true, resizable: false, center: true,
    webPreferences: { nodeIntegration: false, contextIsolation: true },
  });
  splashWindow.loadFile(path.join(__dirname, 'splash.html'));
  splashWindow.show();
}

function setSplashStatus(msg) {
  if (splashWindow && !splashWindow.isDestroyed()) {
    splashWindow.webContents
      .executeJavaScript(`document.getElementById('status').textContent = ${JSON.stringify(msg)}`)
      .catch(() => {});
  }
}

// ── Main window ───────────────────────────────────────────────────────────────
function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 1400, height: 900, minWidth: 900, minHeight: 600,
    show: false, title: 'ProjectFlow',
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    backgroundColor: '#0d0d1a',
    webPreferences: {
      nodeIntegration: false, contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  mainWindow.loadURL(BACKEND_URL + '/');

  mainWindow.once('ready-to-show', () => {
    if (splashWindow && !splashWindow.isDestroyed()) { splashWindow.close(); splashWindow = null; }
    mainWindow.show();
    mainWindow.focus();
  });

  mainWindow.webContents.setWindowOpenHandler(({ url: openUrl }) => {
    if (!openUrl.startsWith(BACKEND_URL)) { shell.openExternal(openUrl); return { action: 'deny' }; }
    return { action: 'allow' };
  });

  mainWindow.on('close', (e) => {
    if (!isQuitting && process.platform === 'darwin') { e.preventDefault(); mainWindow.hide(); }
  });
  mainWindow.on('closed', () => { mainWindow = null; });

  buildMenu();
}

// ── Menu ──────────────────────────────────────────────────────────────────────
function buildMenu() {
  const isMac = process.platform === 'darwin';
  const template = [
    ...(isMac ? [{ label: 'ProjectFlow', submenu: [
      { role: 'about' }, { type: 'separator' }, { role: 'services' }, { type: 'separator' },
      { role: 'hide' }, { role: 'hideOthers' }, { role: 'unhide' }, { type: 'separator' },
      { label: 'Quit', accelerator: 'Cmd+Q', click: () => { isQuitting = true; app.quit(); } },
    ]}] : []),
    { label: 'File', submenu: [
      isMac ? { role: 'close' } : { label: 'Quit', accelerator: 'Alt+F4', click: () => { isQuitting = true; app.quit(); } },
    ]},
    { label: 'Edit', submenu: [
      { role: 'undo' }, { role: 'redo' }, { type: 'separator' },
      { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' },
    ]},
    { label: 'View', submenu: [
      { role: 'reload' }, { role: 'forceReload' }, { type: 'separator' },
      { role: 'resetZoom' }, { role: 'zoomIn' }, { role: 'zoomOut' }, { type: 'separator' },
      { role: 'togglefullscreen' }, { type: 'separator' },
      { label: 'Developer Tools', accelerator: 'CmdOrCtrl+Shift+I', click: () => mainWindow?.webContents.toggleDevTools() },
      { type: 'separator' },
      { label: `Server: ${BACKEND_URL}`, enabled: false },
    ]},
    { label: 'Window', submenu: [
      { role: 'minimize' },
      ...(isMac ? [{ role: 'zoom' }, { type: 'separator' }, { role: 'front' }] : [{ role: 'close' }]),
    ]},
    { label: 'Help', submenu: [
      { label: 'Open in Browser', click: () => shell.openExternal(BACKEND_URL) },
      { label: `Version ${app.getVersion()}`, enabled: false },
    ]},
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

// ── IPC ───────────────────────────────────────────────────────────────────────
ipcMain.handle('get-backend-url', () => BACKEND_URL);
ipcMain.handle('get-app-version', () => app.getVersion());
ipcMain.handle('show-error', (e, msg) => dialog.showErrorBox('ProjectFlow Error', msg));

// ── Lifecycle ─────────────────────────────────────────────────────────────────
app.whenReady().then(async () => {
  createSplash();
  setSplashStatus('Connecting to server...');
  try {
    await waitForServer(BACKEND_URL, 15, 1500);
    setSplashStatus('Loading app...');
    createMainWindow();
  } catch (err) {
    if (splashWindow && !splashWindow.isDestroyed()) splashWindow.close();
    const choice = dialog.showMessageBoxSync({
      type: 'error', title: 'ProjectFlow — Cannot Connect',
      message: 'Could not connect to the ProjectFlow server.',
      detail: `URL: ${BACKEND_URL}\n\n${err.message}\n\nCheck your internet connection or contact your administrator.`,
      buttons: ['Retry', 'Quit'], defaultId: 0,
    });
    if (choice === 0) { app.relaunch(); app.exit(0); } else { app.quit(); }
  }
});

app.on('window-all-closed', () => { if (process.platform !== 'darwin') { isQuitting = true; app.quit(); } });
app.on('activate', () => { if (mainWindow) mainWindow.show(); else createMainWindow(); });
app.on('before-quit', () => { isQuitting = true; });
process.on('uncaughtException', err => console.error('[Electron]', err));

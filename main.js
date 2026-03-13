/**
 * ProjectFlow Desktop — main.js
 *
 * Backend URL is resolved in this priority order:
 *   1. PROJECTFLOW_URL environment variable  (dev / CI override)
 *   2. config.json → backendUrl              (baked in at CI build time)
 *   3. Prompt the user on first launch       (fallback)
 */

const { app, BrowserWindow, shell, ipcMain, dialog, Menu } = require('electron');
const path   = require('path');
const fs     = require('fs');
const http   = require('http');
const https  = require('https');
const urlMod = require('url');

// ── Resolve backend URL ───────────────────────────────────────────────────────
function loadConfig() {
  const configPath = path.join(__dirname, 'config.json');
  try {
    return JSON.parse(fs.readFileSync(configPath, 'utf8'));
  } catch (_) {
    return {};
  }
}

function resolveBackendUrl() {
  // 1. Env var — useful for dev and CI
  if (process.env.PROJECTFLOW_URL) {
    return process.env.PROJECTFLOW_URL.replace(/\/$/, '');
  }
  // 2. config.json
  const cfg = loadConfig();
  if (cfg.backendUrl && !cfg.backendUrl.includes('your-app.up.railway.app')) {
    return cfg.backendUrl.replace(/\/$/, '');
  }
  // 3. No URL configured yet — will prompt user
  return null;
}

// ── State ─────────────────────────────────────────────────────────────────────
let mainWindow   = null;
let splashWindow = null;
let isQuitting   = false;
let backendUrl   = resolveBackendUrl();

// ── Network helpers ───────────────────────────────────────────────────────────
function waitForServer(targetUrl, retries = 20, delay = 1500) {
  return new Promise((resolve, reject) => {
    const parsed    = urlMod.parse(targetUrl);
    const requester = parsed.protocol === 'https:' ? https : http;

    const attempt = (n) => {
      if (n <= 0) return reject(new Error(`Server at ${targetUrl} is not responding.`));
      const req = requester.get(targetUrl + '/', (res) => {
        res.resume();
        if (res.statusCode < 500) resolve();
        else setTimeout(() => attempt(n - 1), delay);
      });
      req.on('error', () => setTimeout(() => attempt(n - 1), delay));
      req.setTimeout(2500, () => { req.destroy(); setTimeout(() => attempt(n - 1), delay); });
    };
    attempt(retries);
  });
}

// ── Splash ────────────────────────────────────────────────────────────────────
function createSplash() {
  splashWindow = new BrowserWindow({
    width: 420, height: 280,
    frame: false, transparent: true,
    alwaysOnTop: true, resizable: false, center: true,
    webPreferences: { nodeIntegration: false, contextIsolation: true },
  });
  splashWindow.loadFile(path.join(__dirname, 'splash.html'));
  splashWindow.show();
}

function setSplashStatus(msg) {
  if (!splashWindow || splashWindow.isDestroyed()) return;
  splashWindow.webContents
    .executeJavaScript(`document.getElementById('status').textContent = ${JSON.stringify(msg)}`)
    .catch(() => {});
}

function closeSplash() {
  if (splashWindow && !splashWindow.isDestroyed()) {
    splashWindow.close();
    splashWindow = null;
  }
}

// ── URL setup dialog (first-run or misconfigured) ─────────────────────────────
async function promptForUrl() {
  closeSplash();

  // Show a simple input dialog using a small BrowserWindow with inline HTML
  return new Promise((resolve) => {
    const win = new BrowserWindow({
      width: 480, height: 300,
      resizable: false, center: true,
      title: 'ProjectFlow — Server Setup',
      backgroundColor: '#0d0d1a',
      webPreferences: { nodeIntegration: false, contextIsolation: true,
        preload: path.join(__dirname, 'preload.js') },
    });

    const html = `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
  *{margin:0;padding:0;box-sizing:border-box}
  body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;
       background:#0d0d1a;color:#fff;display:flex;align-items:center;
       justify-content:center;height:100vh;padding:28px;}
  .card{width:100%;max-width:420px}
  h2{font-size:18px;font-weight:700;margin-bottom:6px;color:#aaff00}
  p{font-size:12px;color:rgba(255,255,255,.5);margin-bottom:20px;line-height:1.5}
  label{font-size:12px;color:rgba(255,255,255,.6);display:block;margin-bottom:6px}
  input{width:100%;padding:10px 14px;background:rgba(255,255,255,.06);
        border:1.5px solid rgba(255,255,255,.15);border-radius:10px;
        color:#fff;font-size:13px;outline:none;transition:border .15s}
  input:focus{border-color:#aaff00}
  .hint{font-size:11px;color:rgba(255,255,255,.3);margin-top:6px}
  button{margin-top:18px;width:100%;padding:11px;background:#aaff00;
         border:none;border-radius:10px;font-size:13px;font-weight:700;
         color:#0d0d1a;cursor:pointer;transition:opacity .15s}
  button:hover{opacity:.9}
  .err{font-size:11px;color:#f87171;margin-top:8px;display:none}
</style>
</head>
<body>
<div class="card">
  <h2>⚡ Connect to Server</h2>
  <p>Enter your Railway (or other) backend URL to continue.</p>
  <label for="url">Backend URL</label>
  <input id="url" type="url" placeholder="https://your-app.up.railway.app"
         value="" autocomplete="off" spellcheck="false"/>
  <div class="hint">Find this in your Railway dashboard under your service's public URL.</div>
  <div class="err" id="err">Please enter a valid URL starting with http:// or https://</div>
  <button onclick="save()">Connect →</button>
</div>
<script>
  document.getElementById('url').focus();
  document.getElementById('url').addEventListener('keydown', e => { if(e.key==='Enter') save(); });
  function save(){
    const val = document.getElementById('url').value.trim().replace(/\\/$/, '');
    if(!val.match(/^https?:\\/\\//)){
      document.getElementById('err').style.display='block'; return;
    }
    window.electronAPI.saveUrl(val);
  }
</script>
</body>
</html>`;

    win.loadURL('data:text/html;charset=utf-8,' + encodeURIComponent(html));

    ipcMain.once('save-url', (event, savedUrl) => {
      win.close();
      resolve(savedUrl);
    });

    win.on('closed', () => resolve(null));
  });
}

// ── Save URL to config.json ───────────────────────────────────────────────────
function saveUrlToConfig(url) {
  const configPath = path.join(__dirname, 'config.json');
  const cfg = loadConfig();
  cfg.backendUrl = url;
  cfg.savedAt = new Date().toISOString();
  fs.writeFileSync(configPath, JSON.stringify(cfg, null, 2));
}

// ── Main window ───────────────────────────────────────────────────────────────
function createMainWindow(url) {
  mainWindow = new BrowserWindow({
    width: 1400, height: 900,
    minWidth: 900, minHeight: 600,
    show: false,
    title: 'ProjectFlow',
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    backgroundColor: '#0d0d1a',
    webPreferences: {
      nodeIntegration: false, contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  mainWindow.loadURL(url + '/');

  mainWindow.once('ready-to-show', () => {
    closeSplash();
    mainWindow.show();
    mainWindow.focus();
  });

  // External links → OS browser
  mainWindow.webContents.setWindowOpenHandler(({ url: openUrl }) => {
    if (!openUrl.startsWith(url)) { shell.openExternal(openUrl); return { action: 'deny' }; }
    return { action: 'allow' };
  });

  mainWindow.on('close', (e) => {
    if (!isQuitting && process.platform === 'darwin') { e.preventDefault(); mainWindow.hide(); }
  });
  mainWindow.on('closed', () => { mainWindow = null; });

  buildMenu(url);
}

// ── App menu ──────────────────────────────────────────────────────────────────
function buildMenu(url) {
  const isMac = process.platform === 'darwin';
  const template = [
    ...(isMac ? [{ label: 'ProjectFlow', submenu: [
      { role: 'about' }, { type: 'separator' }, { role: 'hide' },
      { role: 'hideOthers' }, { role: 'unhide' }, { type: 'separator' },
      { label: 'Quit', accelerator: 'Cmd+Q', click: () => { isQuitting = true; app.quit(); } },
    ]}] : []),
    { label: 'File', submenu: [
      { label: 'Change Server URL…', click: () => changeServerUrl() },
      { type: 'separator' },
      isMac ? { role: 'close' } : { label: 'Quit', accelerator: 'Alt+F4',
        click: () => { isQuitting = true; app.quit(); } },
    ]},
    { label: 'Edit', submenu: [
      { role: 'undo' }, { role: 'redo' }, { type: 'separator' },
      { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' },
    ]},
    { label: 'View', submenu: [
      { role: 'reload' }, { role: 'forceReload' }, { type: 'separator' },
      { role: 'resetZoom' }, { role: 'zoomIn' }, { role: 'zoomOut' },
      { type: 'separator' }, { role: 'togglefullscreen' }, { type: 'separator' },
      { label: 'Developer Tools', accelerator: 'CmdOrCtrl+Shift+I',
        click: () => mainWindow?.webContents.toggleDevTools() },
      { type: 'separator' },
      { label: `Server: ${url}`, enabled: false },
    ]},
    { label: 'Window', submenu: [
      { role: 'minimize' },
      ...(isMac ? [{ role: 'zoom' }, { type: 'separator' }, { role: 'front' }] : [{ role: 'close' }]),
    ]},
    { label: 'Help', submenu: [
      { label: 'Open in Browser', click: () => shell.openExternal(url) },
      { label: `v${app.getVersion()}`, enabled: false },
    ]},
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

// ── Change server URL at runtime ──────────────────────────────────────────────
async function changeServerUrl() {
  const newUrl = await promptForUrl();
  if (!newUrl) return;
  saveUrlToConfig(newUrl);
  backendUrl = newUrl;
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.loadURL(newUrl + '/');
    buildMenu(newUrl);
  }
}

// ── IPC ───────────────────────────────────────────────────────────────────────
ipcMain.handle('get-backend-url', () => backendUrl);
ipcMain.handle('get-app-version', () => app.getVersion());
ipcMain.handle('get-config',      () => loadConfig());
ipcMain.on('save-url', (e, url) => { /* handled inline in promptForUrl */ });

// ── Lifecycle ─────────────────────────────────────────────────────────────────
app.whenReady().then(async () => {
  createSplash();

  // First-run: no URL configured
  if (!backendUrl) {
    setSplashStatus('Setup required...');
    const entered = await promptForUrl();
    if (!entered) { app.quit(); return; }
    saveUrlToConfig(entered);
    backendUrl = entered;
    createSplash(); // re-show splash for connection check
  }

  setSplashStatus(`Connecting to ${backendUrl}…`);

  try {
    await waitForServer(backendUrl, 20, 1500);
    setSplashStatus('Loading...');
    createMainWindow(backendUrl);
  } catch (err) {
    closeSplash();
    const choice = dialog.showMessageBoxSync({
      type: 'error',
      title: 'ProjectFlow — Cannot Connect',
      message: `Could not reach the ProjectFlow server.`,
      detail: `URL: ${backendUrl}\n\n${err.message}\n\nMake sure the Railway service is running.`,
      buttons: ['Retry', 'Change URL', 'Quit'],
      defaultId: 0,
    });
    if (choice === 0) { app.relaunch(); app.exit(0); }
    else if (choice === 1) { await changeServerUrl(); }
    else { app.quit(); }
  }
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') { isQuitting = true; app.quit(); }
});
app.on('activate', () => {
  if (mainWindow) mainWindow.show();
  else if (backendUrl) createMainWindow(backendUrl);
});
app.on('before-quit', () => { isQuitting = true; });
process.on('uncaughtException', err => console.error('[Electron]', err));

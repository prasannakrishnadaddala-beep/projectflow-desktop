const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  getBackendUrl:  () => ipcRenderer.invoke('get-backend-url'),
  getAppVersion:  () => ipcRenderer.invoke('get-app-version'),
  showError:      (msg) => ipcRenderer.invoke('show-error', msg),
  platform: process.platform,
});

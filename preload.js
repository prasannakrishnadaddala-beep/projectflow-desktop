const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  getBackendUrl: ()      => ipcRenderer.invoke('get-backend-url'),
  getAppVersion: ()      => ipcRenderer.invoke('get-app-version'),
  getConfig:     ()      => ipcRenderer.invoke('get-config'),
  saveUrl:       (url)   => ipcRenderer.send('save-url', url),
  platform: process.platform,
});

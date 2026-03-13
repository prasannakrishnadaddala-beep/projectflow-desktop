use tauri::{
    AppHandle, Manager,
    tray::{TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct Config {
    #[serde(rename = "backendUrl", default)]
    backend_url: String,
}

fn config_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_config_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("config.json")
}

fn load_config(app: &AppHandle) -> Config {
    std::fs::read_to_string(config_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(app: &AppHandle, cfg: &Config) {
    let p = config_path(app);
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    if let Ok(j) = serde_json::to_string_pretty(cfg) { let _ = std::fs::write(p, j); }
}

/// Default Railway URL — patched by CI from RAILWAY_URL secret
const DEFAULT_URL: &str = "https://web-production-bde95.up.railway.app";

fn get_url(app: &AppHandle) -> String {
    let saved = load_config(app).backend_url;
    if saved.is_empty() || saved.contains("your-app") {
        DEFAULT_URL.to_string()
    } else {
        saved
    }
}

fn has_valid_url(_app: &AppHandle) -> bool {
    // Always true — we always have a default URL hardcoded
    true
}

#[tauri::command]
fn focus_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn navigate_to(app: AppHandle, view: String) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        let js = format!("window.__pfNavigate&&window.__pfNavigate({})",
            serde_json::to_string(&view).unwrap_or_default());
        let _ = win.eval(&js);
    }
}

#[tauri::command]
fn connect(app: AppHandle, url: String) -> Result<(), String> {
    let clean = url.trim_end_matches('/').to_string();
    save_config(&app, &Config { backend_url: clean });
    open_main_window(app)
}

#[tauri::command]
fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

const SETUP_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
* { margin:0; padding:0; box-sizing:border-box }
body { font-family:system-ui,sans-serif; background:#0d0d1a; color:#fff;
       display:flex; align-items:center; justify-content:center; height:100vh; padding:28px }
.card { width:100%; max-width:420px }
h2 { font-size:18px; font-weight:700; margin-bottom:6px; color:#aaff00 }
p  { font-size:12px; color:rgba(255,255,255,.5); margin-bottom:20px; line-height:1.5 }
label { font-size:12px; color:rgba(255,255,255,.6); display:block; margin-bottom:6px }
input { width:100%; padding:10px 14px; background:rgba(255,255,255,.06);
        border:1.5px solid rgba(255,255,255,.15); border-radius:10px;
        color:#fff; font-size:13px; outline:none }
input:focus { border-color:#aaff00 }
.hint { font-size:11px; color:rgba(255,255,255,.3); margin-top:6px }
button { margin-top:18px; width:100%; padding:11px; background:#aaff00;
         border:none; border-radius:10px; font-size:13px; font-weight:700;
         color:#0d0d1a; cursor:pointer }
.err { font-size:11px; color:#f87171; margin-top:8px; display:none }
</style>
</head>
<body>
<div class="card">
  <h2>⚡ Connect to Server</h2>
  <p>Enter your Railway backend URL. You only need to do this once.</p>
  <label>Backend URL</label>
  <input id="u" type="url" placeholder="https://your-app.up.railway.app"
         autocomplete="off" spellcheck="false"/>
  <div class="hint">Find this in your Railway dashboard.</div>
  <div class="err" id="err">Please enter a valid https:// URL</div>
  <button onclick="go()">Connect →</button>
</div>
<script>
const { invoke } = window.__TAURI__.core;
document.getElementById('u').focus();
document.getElementById('u').addEventListener('keydown', e => { if(e.key==='Enter') go(); });
async function go() {
  const v = document.getElementById('u').value.trim().replace(/\/$/,'');
  if (!v.match(/^https?:\/\//)) { document.getElementById('err').style.display='block'; return; }
  document.querySelector('button').textContent = 'Connecting…';
  try { await invoke('connect', { url: v }); }
  catch(e) { document.querySelector('button').textContent='Connect →'; alert('Error: '+e); }
}
</script>
</body>
</html>"#;

fn open_setup_window(app: &AppHandle) {
    let html_path = app.path().app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("setup.html");
    if let Some(d) = html_path.parent() { let _ = std::fs::create_dir_all(d); }
    let _ = std::fs::write(&html_path, SETUP_HTML);

    let _ = tauri::WebviewWindowBuilder::new(app, "setup", tauri::WebviewUrl::App("".into()))
        .title("ProjectFlow — Connect to Server")
        .inner_size(480.0, 320.0)
        .resizable(false)
        .center()
        .initialization_script(&format!(
            r#"window.addEventListener('DOMContentLoaded',()=>{{document.open();document.write({});document.close();}});"#,
            serde_json::to_string(SETUP_HTML).unwrap()
        ))
        .build();
}

const BRIDGE_SCRIPT: &str = r#"
(function() {
  if (window.__pfBridge) return;
  window.__pfBridge = true;
  window.electronAPI = {
    focusWindow: () => window.__TAURI__?.core.invoke('focus_window').catch(()=>{}),
    navigateTo:  (v) => window.__TAURI__?.core.invoke('navigate_to', {view:v}).catch(()=>{}),
    onNavigateTo:(cb) => { window.__pfNavigate = cb; },
    getAppVersion: () => window.__TAURI__?.core.invoke('get_app_version') ?? Promise.resolve('4.1.0'),
    platform: 'tauri',
  };
})();
"#;

fn open_main_window(app: AppHandle) -> Result<(), String> {
    let url = get_url(&app);
    if let Some(w) = app.get_webview_window("setup") { let _ = w.close(); }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show(); let _ = win.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app, "main",
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("{e}"))?),
    )
    .title("ProjectFlow")
    .inner_size(1400.0, 900.0)
    .min_inner_size(900.0, 600.0)
    .center()
    .initialization_script(BRIDGE_SCRIPT)
    .build()
    .map_err(|e| format!("{e}"))?;
    Ok(())
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open ProjectFlow", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                focus_window(tray.app_handle().clone());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => focus_window(app.clone()),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            focus_window, navigate_to, connect, get_app_version,
        ])
        .setup(|app| {
            let _ = build_tray(app);
            if has_valid_url(app.handle()) {
                open_main_window(app.handle().clone()).ok();
            } else {
                open_setup_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { api: _, .. } = event {
                #[cfg(target_os = "macos")]
                { _window.hide().ok(); }
            }
        })
        .run(tauri::generate_context!())
        .expect("error running ProjectFlow");
}

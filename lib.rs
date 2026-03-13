use tauri::{
    AppHandle, Manager,
    tray::{TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem},
};
use serde::{Deserialize, Serialize};

/// Default Railway URL — patched by CI from RAILWAY_URL secret
const DEFAULT_URL: &str = "https://web-production-bde95.up.railway.app";

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

fn get_url(app: &AppHandle) -> String {
    let saved = load_config(app).backend_url;
    if saved.is_empty() || saved.contains("your-app") {
        DEFAULT_URL.to_string()
    } else {
        saved
    }
}

// ── IPC Commands ──────────────────────────────────────────────────────────────

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

// ── Auto-updater ──────────────────────────────────────────────────────────────

fn check_for_updates(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(u) => u,
            Err(_) => return, // updater not configured
        };
        match updater.check().await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                let handle = app.clone();
                // Show dialog asking user to update
                let _ = tauri::WebviewWindowBuilder::new(
                    &handle,
                    "updater",
                    tauri::WebviewUrl::App("".into()),
                )
                .title("Update Available")
                .inner_size(400.0, 220.0)
                .resizable(false)
                .center()
                .initialization_script(&format!(r#"
                    window.addEventListener('DOMContentLoaded', () => {{
                        document.open();
                        document.write(`<!DOCTYPE html>
<html><head><meta charset="UTF-8">
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:system-ui,sans-serif;background:#0d0d1a;color:#fff;
     display:flex;align-items:center;justify-content:center;height:100vh;padding:24px}}
.card{{width:100%;max-width:360px;text-align:center}}
h2{{font-size:16px;font-weight:700;margin-bottom:8px;color:#aaff00}}
p{{font-size:13px;color:rgba(255,255,255,.6);margin-bottom:20px;line-height:1.5}}
.btns{{display:flex;gap:10px}}
button{{flex:1;padding:10px;border:none;border-radius:8px;font-size:13px;font-weight:600;cursor:pointer}}
.install{{background:#aaff00;color:#0d0d1a}}
.skip{{background:rgba(255,255,255,.1);color:#fff}}
</style></head>
<body><div class="card">
<h2>⚡ Update Available</h2>
<p>ProjectFlow <strong>{version}</strong> is ready to install.<br>The app will restart automatically.</p>
<div class="btns">
  <button class="install" onclick="install()">Install & Restart</button>
  <button class="skip" onclick="window.__TAURI__.core.invoke('skip_update').then(()=>window.close())">Later</button>
</div>
</div>
<script>
async function install() {{
  document.querySelector('.install').textContent = 'Installing…';
  await window.__TAURI__.core.invoke('install_update');
}}
</script>
</body></html>`);
                        document.close();
                    }});
                "#))
                .build()
                .ok();

                // Store update handle for IPC
                app.manage(std::sync::Mutex::new(Some(update)));
            }
            Ok(None) => { /* No update available */ }
            Err(_) => { /* Network error — silently ignore */ }
        }
    });
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update_opt = app.state::<std::sync::Mutex<Option<tauri_plugin_updater::Update>>>();
    let mut lock = update_opt.lock().unwrap();
    if let Some(update) = lock.take() {
        update.download_and_install(|_, _| {}, || {}).await
            .map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}

#[tauri::command]
fn skip_update(app: AppHandle) {
    if let Some(w) = app.get_webview_window("updater") { let _ = w.close(); }
}

// ── Bridge script injected into every page ────────────────────────────────────

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

// ── Tray ──────────────────────────────────────────────────────────────────────

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

// ── Entry ─────────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(std::sync::Mutex::new(None::<tauri_plugin_updater::Update>))
        .invoke_handler(tauri::generate_handler![
            focus_window, navigate_to, connect, get_app_version,
            install_update, skip_update,
        ])
        .setup(|app| {
            let _ = build_tray(app);
            open_main_window(app.handle().clone()).ok();
            // Check for updates 3 seconds after launch
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                check_for_updates(handle);
            });
            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { api: _, .. } = event {
                #[cfg(target_os = "macos")]
                { let _ = _window.hide(); }
            }
        })
        .run(tauri::generate_context!())
        .expect("error running ProjectFlow");
}

use tauri::{
    AppHandle, Manager,
    tray::{TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem},
};
use serde::{Deserialize, Serialize};

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
    save_config(&app, &Config { backend_url: url.trim_end_matches('/').to_string() });
    open_main_window(app)
}

#[tauri::command]
fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

const BRIDGE: &str = r#"
(function() {
  if (window.__pfBridge) return;
  window.__pfBridge = true;
  window.electronAPI = {
    focusWindow:   () => window.__TAURI__?.core.invoke('focus_window').catch(()=>{}),
    navigateTo:    (v) => window.__TAURI__?.core.invoke('navigate_to',{view:v}).catch(()=>{}),
    onNavigateTo:  (cb) => { window.__pfNavigate = cb; },
    getAppVersion: () => window.__TAURI__?.core.invoke('get_app_version') ?? Promise.resolve('4.1.0'),
    platform: 'tauri',
  };
})();
"#;

fn open_main_window(app: AppHandle) -> Result<(), String> {
    let url = get_url(&app);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show(); let _ = win.set_focus(); return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app, "main",
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("{e}"))?),
    )
    .title("ProjectFlow")
    .inner_size(1400.0, 900.0)
    .min_inner_size(900.0, 600.0)
    .center()
    .initialization_script(BRIDGE)
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
            open_main_window(app.handle().clone()).ok();
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

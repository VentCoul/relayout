#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod platform;

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_HOTKEY: &str = "Control+Shift+Space";

/// The only thing this app ever writes to disk.
#[derive(Serialize, Deserialize)]
struct Config {
    hotkey: String,
}

struct AppState {
    hotkey: Mutex<String>,
}

#[derive(Serialize)]
struct Settings {
    hotkey: String,
    autostart: bool,
    permission: bool,
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("settings.json"))
}

fn load_hotkey(app: &AppHandle) -> String {
    config_path(app)
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<Config>(&raw).ok())
        .map(|config| config.hotkey)
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
}

fn save_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let path = config_path(app).ok_or("no config directory")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let config = Config { hotkey: hotkey.to_string() };
    let raw = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

fn register(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcut: Shortcut = hotkey.parse().map_err(|_| format!("Невідома комбінація: {hotkey}"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|_| format!("Комбінація {hotkey} зайнята іншою програмою"))
}

#[tauri::command]
fn get_settings(app: AppHandle, state: State<AppState>) -> Settings {
    Settings {
        hotkey: state.hotkey.lock().unwrap().clone(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        permission: platform::has_permission(false),
    }
}

#[tauri::command]
fn set_hotkey(app: AppHandle, state: State<AppState>, hotkey: String) -> Result<(), String> {
    let mut current = state.hotkey.lock().unwrap();
    let _ = app.global_shortcut().unregister_all();
    if let Err(error) = register(&app, &hotkey) {
        let _ = register(&app, &current);
        return Err(error);
    }
    save_hotkey(&app, &hotkey)?;
    *current = hotkey;
    Ok(())
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
fn request_permission() -> bool {
    platform::has_permission(true)
}

fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Released {
                        engine::trigger(app.clone());
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_hotkey,
            set_autostart,
            request_permission
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let hotkey = load_hotkey(&handle);
            let registered = register(&handle, &hotkey);
            app.manage(AppState { hotkey: Mutex::new(hotkey) });

            let settings = MenuItem::with_id(app, "settings", "Налаштування…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Вийти", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &quit])?;
            // macOS recolours a monochrome template icon for light and dark
            // menu bars; elsewhere the taskbar needs the coloured icon.
            #[cfg(target_os = "macos")]
            let tray = TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/tray.png"))
                .icon_as_template(true);
            #[cfg(not(target_os = "macos"))]
            let tray = TrayIconBuilder::new().icon(tauri::include_image!("icons/32x32.png"));
            tray
                .tooltip("Relayout")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => show_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // First run, or something needs the user's attention.
            if registered.is_err() || !platform::has_permission(false) {
                show_settings(&handle);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start Relayout");

    app.run(|_app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event {
            // Closing the settings window must not quit a tray app.
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}

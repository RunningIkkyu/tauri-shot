use arboard::{Clipboard, ImageData};
use base64::Engine;
use image::DynamicImage;
use mouse_position::mouse_position::Mouse;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::utils::config::BackgroundThrottlingPolicy;
/// NSScreenSaverWindowLevel — high enough to paint above native fullscreen spaces.
#[cfg(target_os = "macos")]
const CAPTURE_WINDOW_LEVEL: i64 = 1000;

#[cfg(target_os = "macos")]
fn activate_app_for_capture() {
    use cocoa::appkit::NSApplicationActivationOptions;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let current: cocoa::base::id =
            msg_send![class!(NSRunningApplication), currentApplication];
        if !current.is_null() {
            let opts = (NSApplicationActivationOptions::NSApplicationActivateAllWindows as usize)
                | (NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps as usize);
            let _: cocoa::base::BOOL = msg_send![current, activateWithOptions: opts];
        }
    }
    // Legacy API as a fallback; combining both is more reliable on some OS versions.
    use cocoa::appkit::{NSApp, NSApplication};
    use cocoa::base::YES;
    unsafe {
        NSApp().activateIgnoringOtherApps_(YES);
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_app_for_capture() {}

#[cfg(target_os = "macos")]
fn elevate_capture_window(window: &tauri::WebviewWindow) {
    use cocoa::appkit::{NSWindow, NSWindowCollectionBehavior};
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};

    let ns_window = match window.ns_window() {
        Ok(p) => p as id,
        Err(_) => return,
    };
    unsafe {
        let behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary;
        ns_window.setCollectionBehavior_(behavior);
        let _: () = msg_send![ns_window, setLevel: CAPTURE_WINDOW_LEVEL];
        let _: () = msg_send![ns_window, orderFrontRegardless];
        let _: () = msg_send![ns_window, makeKeyAndOrderFront: cocoa::base::nil];
    }
}

#[cfg(not(target_os = "macos"))]
fn elevate_capture_window(_window: &tauri::WebviewWindow) {}

use tauri::menu::{Menu, MenuItem};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use xcap::Monitor;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub screenshot_shortcut: String,
    pub default_export_format: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            screenshot_shortcut: "cmd+shift+2".into(),
            default_export_format: "webp".into(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapturePayload {
    png_base64: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale_factor: f64,
}

static PENDING_CAPTURE: Mutex<Option<CapturePayload>> = Mutex::new(None);

struct AppState {
    settings_path: PathBuf,
    registered_shortcut: Mutex<String>,
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve("settings.json", BaseDirectory::AppConfig)
        .map_err(|e| e.to_string())
}

fn load_settings_disk(path: &PathBuf) -> AppSettings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_settings_disk(path: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn monitor_under_cursor() -> Result<Monitor, String> {
    let (mx, my) = match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        Mouse::Error => return Err("Failed to read mouse position".into()),
    };
    Monitor::from_point(mx, my).map_err(|e| e.to_string())
}

fn encode_monitor_png(monitor: &Monitor) -> Result<Vec<u8>, String> {
    let img = monitor.capture_image().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    DynamicImage::ImageRgba8(img)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

fn open_capture_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.close();
    }

    let monitor = monitor_under_cursor()?;
    let png = encode_monitor_png(&monitor)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);

    let x = monitor.x().map_err(|e| e.to_string())?;
    let y = monitor.y().map_err(|e| e.to_string())?;
    let width = monitor.width().map_err(|e| e.to_string())?;
    let height = monitor.height().map_err(|e| e.to_string())?;
    let scale_factor = monitor.scale_factor().map_err(|e| e.to_string())? as f64;

    *PENDING_CAPTURE.lock().unwrap() = Some(CapturePayload {
        png_base64: b64,
        x,
        y,
        width,
        height,
        scale_factor,
    });

    // Load route /capture; avoids an extra client-side goto; WebView runs scripts when visible.
    let window = WebviewWindowBuilder::new(app, "capture", WebviewUrl::App(PathBuf::from("capture")))
        .title("Screenshot")
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(true)
        .background_throttling(BackgroundThrottlingPolicy::Disabled)
        .position(x as f64, y as f64)
        .inner_size(width as f64, height as f64)
        .build()
        .map_err(|e| e.to_string())?;

    elevate_capture_window(&window);
    // Activate after capture finishes to avoid stealing focus from fullscreen; brings WKWebView forward to load.
    activate_app_for_capture();
    let _ = window.show();
    let _ = window.set_focus();

    Ok(())
}

fn register_capture_shortcut(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    Shortcut::from_str(shortcut_str).map_err(|e| format!("Invalid shortcut syntax: {e}"))?;
    app.global_shortcut()
        .on_shortcut(shortcut_str, |app, _, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let app_main = app.clone();
            let app_cap = app.clone();
            let _ = app_main.run_on_main_thread(move || {
                let _ = open_capture_window(&app_cap);
            });
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn take_pending_capture() -> Option<CapturePayload> {
    PENDING_CAPTURE.lock().unwrap().take()
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let state = app.state::<AppState>();
    Ok(load_settings_disk(&state.settings_path))
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    Shortcut::from_str(&settings.screenshot_shortcut)
        .map_err(|e| format!("Invalid shortcut: {e}"))?;

    let state = app.state::<AppState>();
    save_settings_disk(&state.settings_path, &settings)?;

    let reg = state.registered_shortcut.lock().unwrap();
    if reg.as_str() != settings.screenshot_shortcut.as_str() {
        let old = reg.clone();
        drop(reg);
        let old_sc = Shortcut::from_str(&old).map_err(|e| e.to_string())?;
        app.global_shortcut()
            .unregister(old_sc)
            .map_err(|e| e.to_string())?;
        register_capture_shortcut(&app, &settings.screenshot_shortcut)?;
        *state.registered_shortcut.lock().unwrap() = settings.screenshot_shortcut.clone();
    }

    Ok(())
}

#[tauri::command]
fn close_capture_window(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("capture") {
        w.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn show_capture_window(app: AppHandle) -> Result<(), String> {
    activate_app_for_capture();
    if let Some(w) = app.get_webview_window("capture") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn copy_image_to_clipboard(image_base64: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_base64.trim())
        .map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_image(ImageData {
            width: w as usize,
            height: h as usize,
            bytes: std::borrow::Cow::Owned(rgba.into_raw()),
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn write_image_file(path: String, format: String, image_base64: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_base64.trim())
        .map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let fmt = format.to_lowercase();
    match fmt.as_str() {
        "jpeg" | "jpg" => img
            .save_with_format(&path, image::ImageFormat::Jpeg)
            .map_err(|e| e.to_string()),
        "webp" => img
            .save_with_format(&path, image::ImageFormat::WebP)
            .map_err(|e| e.to_string()),
        _ => img
            .save_with_format(&path, image::ImageFormat::Png)
            .map_err(|e| e.to_string()),
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let path = settings_path(&app.handle())?;
            let mut settings = load_settings_disk(&path);
            if Shortcut::from_str(&settings.screenshot_shortcut).is_err() {
                settings.screenshot_shortcut = AppSettings::default().screenshot_shortcut;
                let _ = save_settings_disk(&path, &settings);
            }

            let shortcut_registered = settings.screenshot_shortcut.clone();
            app.manage(AppState {
                settings_path: path,
                registered_shortcut: Mutex::new(shortcut_registered.clone()),
            });

            register_capture_shortcut(&app.handle(), &shortcut_registered)?;

            let show_item = MenuItem::with_id(app, "show", "Open settings", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .tooltip("tauri-shot")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            take_pending_capture,
            get_settings,
            save_settings,
            close_capture_window,
            show_capture_window,
            copy_image_to_clipboard,
            write_image_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

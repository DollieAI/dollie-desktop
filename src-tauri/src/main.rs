#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::WebviewWindowBuilder,
    Listener, Manager, WebviewUrl,
};

/// JavaScript injected into every page to handle theme detection
const THEME_INIT_SCRIPT: &str = r#"
(function() {
    if (window.__TAURI_INIT__) return;
    window.__TAURI_INIT__ = true;

    // ===== Theme Detection =====
    window.__TAURI_THEME_DARK__ = window.matchMedia('(prefers-color-scheme: dark)').matches;

    let applying = false;

    function applyTauriClasses() {
        if (!document.body || applying) return;
        applying = true;

        const needsTauri = !document.body.classList.contains('tauri');
        const needsDark = window.__TAURI_THEME_DARK__ && !document.body.classList.contains('dark');
        const needsRemoveDark = !window.__TAURI_THEME_DARK__ && document.body.classList.contains('dark');

        if (needsTauri) document.body.classList.add('tauri');
        if (needsDark) document.body.classList.add('dark');
        if (needsRemoveDark) document.body.classList.remove('dark');

        applying = false;
    }

    applyTauriClasses();
    document.addEventListener('DOMContentLoaded', applyTauriClasses);

    const observer = new MutationObserver(function() {
        if (!applying) applyTauriClasses();
    });

    function startObserver() {
        if (document.body) {
            observer.observe(document.body, { attributes: true, attributeFilter: ['class'] });
            applyTauriClasses();
        } else {
            requestAnimationFrame(startObserver);
        }
    }
    startObserver();

    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function(e) {
        window.__TAURI_THEME_DARK__ = e.matches;
        applyTauriClasses();
    });

    document.addEventListener('livewire:navigated', applyTauriClasses);
    document.addEventListener('livewire:init', applyTauriClasses);

})();
"#;

/// Set the dock/taskbar badge count (macOS primarily)
#[tauri::command]
#[allow(unused_variables)]
fn set_badge_count(app: tauri::AppHandle, count: u32) {
    // macOS badge via cocoa
    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    {
        use cocoa::appkit::NSApp;
        use cocoa::base::nil;
        use cocoa::foundation::NSString;
        use objc::runtime::Object;
        unsafe {
            let app_instance: *mut Object = NSApp();
            let dock_tile: *mut Object = msg_send![app_instance, dockTile];
            if count > 0 {
                let label = NSString::alloc(nil).init_str(&count.to_string());
                let _: () = msg_send![dock_tile, setBadgeLabel: label];
            } else {
                let _: () = msg_send![dock_tile, setBadgeLabel: nil];
            }
        }
    }

    // Windows/Linux: Update tray icon tooltip as fallback
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(tray) = app.tray_by_id("main-tray") {
            let tooltip = if count > 0 {
                format!("Dollie ({} unread)", count)
            } else {
                "Dollie".to_string()
            };
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    }
}

/// Clear the badge
#[tauri::command]
fn clear_badge(_app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    {
        use cocoa::appkit::NSApp;
        use cocoa::base::nil;
        use objc::runtime::Object;
        unsafe {
            let app_instance: *mut Object = NSApp();
            let dock_tile: *mut Object = msg_send![app_instance, dockTile];
            let _: () = msg_send![dock_tile, setBadgeLabel: nil];
        }
    }
}

/// Show the main window and bring to front
#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Open a URL in the system's default browser
#[tauri::command]
fn open_in_browser(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Get the URL from config (devUrl in dev, frontendDist in prod)
            let url = if cfg!(debug_assertions) {
                "https://api-layer.social"
            } else {
                "https://app.dollie.pro"
            };

            // Create the main window with initialization script
            let _window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse().unwrap()))
                .title("Dollie")
                .inner_size(1600.0, 1000.0)
                .min_inner_size(800.0, 600.0)
                .center()
                .maximized(true)
                .decorations(true)
                .resizable(true)
                .initialization_script(THEME_INIT_SCRIPT)
                .build()?;

            // Build tray menu
            let show_item = MenuItem::with_id(app, "show", "Show Dollie", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // Create tray icon
            let tray_icon = tauri::image::Image::from_path("icons/32x32.png")
                .unwrap_or_else(|_| app.default_window_icon().unwrap().clone());
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Handle deep links (dollie://...)
            let handle = app.handle().clone();
            let dev_mode = cfg!(debug_assertions);
            app.listen("deep-link://new-url", move |event| {
                let payload = event.payload();
                // Navigate to the deep link URL
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();

                    // The URL will be something like dollie://tasks/123
                    if let Some(path) = payload.strip_prefix("dollie://") {
                        let base_url = if dev_mode {
                            "https://api-layer.social"
                        } else {
                            "https://app.dollie.pro"
                        };

                        let web_url = format!("{}/{}", base_url, path);
                        let _ = window.eval(&format!("window.location.href = '{}'", web_url));
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_badge_count,
            clear_badge,
            show_window,
            open_in_browser,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

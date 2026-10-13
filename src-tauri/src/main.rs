#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod installation;
mod menu;
#[cfg(target_os = "macos")]
mod notifications;
mod state;
mod updater;
mod window;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, RunEvent, WebviewWindow, WindowEvent,
};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_notification::NotificationExt;
use url::Url;

use state::{parse_hex, Prefs, Shell};

/// A count when something needs the person; otherwise `dot` marks unread news.
#[tauri::command]
fn set_badge_count(app: AppHandle, window: WebviewWindow, count: u32, dot: Option<bool>) {
    let dot = dot.unwrap_or(false);

    #[cfg(target_os = "macos")]
    {
        let _ = app;
        let _ = if count > 0 {
            window.set_badge_count(Some(count as i64))
        } else {
            window.set_badge_label(dot.then(|| "•".to_string()))
        };
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = (app, dot);
        let _ = window.set_badge_count((count > 0).then_some(count as i64));
    }

    #[cfg(windows)]
    {
        let _ = window;
        if let Some(tray) = app.tray_by_id("main-tray") {
            let tooltip = if count > 0 {
                format!("Dollie ({count} unread)")
            } else if dot {
                "Dollie (new notifications)".into()
            } else {
                "Dollie".into()
            };
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    }
}

/// Shows a system notification while the window is in the background.
#[tauri::command]
fn notify(app: AppHandle, window: WebviewWindow, title: String, body: Option<String>) {
    if window.is_focused().unwrap_or(false) && window.is_visible().unwrap_or(false) {
        return;
    }
    #[cfg(target_os = "macos")]
    if notifications::show(&title, body.as_deref()) {
        return;
    }
    let mut builder = app.notification().builder().title(title);
    if let Some(body) = body {
        builder = builder.body(body);
    }
    let _ = builder.show();
}

/// Asks the system once for permission to show notifications; the web app calls it after sign-in.
#[tauri::command]
fn request_notifications() {
    #[cfg(target_os = "macos")]
    notifications::request_permission();
}

/// The web app reports its theme so the next launch paints the right colour before it loads.
#[tauri::command]
fn report_theme(app: AppHandle, dark: bool, background: Option<String>) {
    let background = background.filter(|hex| parse_hex(hex).is_some());
    let shell = app.state::<Shell>();
    let mut prefs = shell.prefs.lock().unwrap();
    if prefs.dark == Some(dark) && prefs.background == background {
        return;
    }
    prefs.dark = Some(dark);
    prefs.background = background;
    prefs.save(&app);
    let color = prefs.background_color();
    for window in app.webview_windows().values() {
        let _ = window.set_background_color(Some(color));
    }
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    let url = Url::parse(&url).map_err(|e| e.to_string())?;
    window::open_external(&url);
    Ok(())
}

#[tauri::command]
fn install_update(app: AppHandle) -> Result<(), String> {
    updater::install(&app)
}

/// Opens a web session from the saved sign-in, or starts one in the browser unless `silent`.
#[tauri::command]
async fn sign_in(app: AppHandle, silent: bool) -> Result<&'static str, String> {
    Ok(match auth::sign_in(&app, silent).await? {
        auth::Outcome::SignedIn => "signed-in",
        auth::Outcome::Browser => "browser",
        auth::Outcome::NeedsBrowser => "needs-browser",
    })
}

/// `dollie://tasks/123?x=1` → `<installation>/tasks/123?x=1`.
fn route_deep_link(app: &AppHandle, link: &Url) {
    if link.scheme() != "dollie" {
        return;
    }
    if link.host_str() == Some("auth") && link.path() == "/callback" {
        let app = app.clone();
        let callback = link.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(message) = auth::complete(&app, &callback).await {
                let _ = app
                    .notification()
                    .builder()
                    .title("Sign-in failed")
                    .body(message)
                    .show();
            }
        });
        return;
    }
    let path = format!(
        "/{}{}{}",
        link.host_str().unwrap_or_default(),
        link.path(),
        link.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    if let Some(url) = app.state::<Shell>().app_url(&path) {
        window::navigate(app, &url);
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "tray-show", "Show Dollie", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit Dollie", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("Dollie")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(main) = tray.app_handle().get_webview_window(window::MAIN) {
                    window::reveal(&main);
                }
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray-show" => {
                if let Some(main) = app.get_webview_window(window::MAIN) {
                    window::reveal(&main);
                }
            }
            "tray-quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut builder = tauri::Builder::default();

    // Must be first: a second launch hands its arguments (and deep link) to this process.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(main) = app.get_webview_window(window::MAIN) {
                window::reveal(&main);
            }
        }));
    }

    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let handle = app.handle().clone();
            let prefs = Prefs::load(&handle);
            let (origin, client_id) = prefs
                .installation
                .as_ref()
                .and_then(|saved| Some((Url::parse(&saved.url).ok()?, saved.client_id.clone())))
                .unwrap_or((
                    Url::parse(installation::DOLLIE_CLOUD)?,
                    installation::DOLLIE_CLOUD_CLIENT_ID.to_owned(),
                ));
            app.manage(Shell::new(origin.clone(), client_id, prefs));
            app.manage(installation::Granted::default());
            installation::grant(&handle, &origin)?;
            app.manage(auth::PendingSignIn::default());

            #[cfg(target_os = "macos")]
            app.set_menu(menu::build(&handle)?)?;
            app.on_menu_event(menu::handle);
            build_tray(&handle)?;

            #[cfg(any(windows, target_os = "linux"))]
            let _ = app.deep_link().register_all();

            let shell = app.state::<Shell>();
            let launched_with = app
                .deep_link()
                .get_current()
                .ok()
                .flatten()
                .and_then(|links| links.into_iter().next())
                .and_then(|link| shell.app_url(&format!("/{}{}", link.host_str().unwrap_or_default(), link.path())));
            let start_hidden = std::env::args().any(|arg| arg == "--hidden");
            window::open(
                &handle,
                window::MAIN,
                launched_with.as_ref().unwrap_or(&origin),
                !start_hidden,
            )?;

            let links = handle.clone();
            app.deep_link().on_open_url(move |event| {
                for link in event.urls() {
                    route_deep_link(&links, &link);
                }
            });

            updater::start(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            // macOS keeps the app running when its last window closes, as native apps do.
            #[cfg(target_os = "macos")]
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == window::MAIN {
                    api.prevent_close();
                    if window.is_fullscreen().unwrap_or(false) {
                        let _ = window.set_fullscreen(false);
                    }
                    let _ = window.hide();
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (window, event);
        })
        .invoke_handler(tauri::generate_handler![
            set_badge_count,
            request_notifications,
            notify,
            report_theme,
            open_external,
            install_update,
            sign_in,
            installation::find_installation,
            installation::connect_installation,
            installation::choose_installation,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Dollie")
        .run(|app, event| {
            if let RunEvent::ExitRequested { code: None, api, .. } = &event {
                if app.state::<Shell>().reopening.load(std::sync::atomic::Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
            #[cfg(target_os = "macos")]
            if let RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                if let Some(main) = app.get_webview_window(window::MAIN) {
                    window::reveal(&main);
                }
            }
        });
}

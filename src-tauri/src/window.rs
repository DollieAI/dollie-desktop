use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::{
    webview::{DownloadEvent, NewWindowResponse},
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use url::Url;

use crate::state::Shell;

pub const MAIN: &str = "main";

const BRIDGE: &str = include_str!("bridge.js");
const AWAY: &str = include_str!("away.js");

/// Opens an app window. It starts on the bundled boot page, which shows an offline
/// screen until the installation answers, then replaces itself with `target`.
pub fn open(app: &AppHandle, label: &str, target: &Url, visible: bool) -> tauri::Result<WebviewWindow> {
    open_with(app, label, target, visible, false)
}

/// Closes every app window and opens the main one again, on the connect screen when `connect`.
/// Windows are rebuilt rather than navigated because each one carries its installation's bridge.
pub fn reopen_main(app: &AppHandle, target: &Url, connect: bool) {
    app.state::<Shell>().reopening.store(true, std::sync::atomic::Ordering::SeqCst);
    for (label, window) in app.webview_windows() {
        if label == MAIN || label.starts_with("window-") {
            let _ = window.destroy();
        }
    }
    let app = app.clone();
    let target = target.clone();
    std::thread::spawn(move || {
        for _ in 0..100 {
            if app.get_webview_window(MAIN).is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if let Ok(window) = open_with(&app, MAIN, &target, true, connect) {
            let _ = window.set_focus();
        }
        app.state::<Shell>().reopening.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

fn open_with(app: &AppHandle, label: &str, target: &Url, visible: bool, connect: bool) -> tauri::Result<WebviewWindow> {
    let shell = app.state::<Shell>();
    let background = shell.prefs.lock().unwrap().background_color();
    let script = init_script(app, &shell, target, connect);

    let app_for_popups = app.clone();
    let app_for_downloads = app.clone();
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Dollie")
        .inner_size(1280.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .visible(visible)
        .background_color(background)
        .initialization_script(script)
        .on_navigation(route_navigation)
        .on_page_load(|window, payload| {
            // Development only: chq's Vite dev server can take a while on a cold webview cache.
            if cfg!(debug_assertions) {
                eprintln!(
                    "[{}] {:?} {} at {:?}",
                    window.label(),
                    payload.event(),
                    payload.url(),
                    std::time::SystemTime::now()
                );
            }
        })
        .on_new_window(move |url, features| route_new_window(&app_for_popups, url, features))
        .on_download(move |webview, event| route_download(&app_for_downloads, webview.label(), event));

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(16.0, 14.0));

    builder.build()
}

fn init_script(app: &AppHandle, shell: &Shell, target: &Url, connect: bool) -> String {
    let origin = shell.origin();
    let installation_name = shell
        .prefs
        .lock()
        .unwrap()
        .installation
        .as_ref()
        .and_then(|i| i.name.clone());
    let config = json!({
        "origin": origin.origin().ascii_serialization(),
        "platform": std::env::consts::OS,
        "version": app.package_info().version.to_string(),
        "boot": {
            "to": target.as_str(),
            "background": shell.prefs.lock().unwrap().background_hex(),
            "mode": if connect { "connect" } else { "open" },
            "installation": { "url": origin.as_str(), "name": installation_name },
            "cloud": crate::installation::DOLLIE_CLOUD,
        },
    });

    format!(
        "const DOLLIE_DESKTOP_CONFIG = {config};\n\
         if (['localhost', 'tauri.localhost', '127.0.0.1'].includes(location.hostname)) {{ window.__DOLLIE_BOOT__ = DOLLIE_DESKTOP_CONFIG.boot; }}\n\
         {BRIDGE}\n{AWAY}"
    )
}

/// Top-level navigation stays in the window so sign-in, billing and connection
/// redirects keep their session; link clicks to other sites are sent out by the bridge.
fn route_navigation(url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" | "tauri" | "about" | "data" | "blob" => true,
        _ => {
            open_external(url);
            false
        }
    }
}

/// `window.open`: app pages get their own app window, sized popups (OAuth, payment
/// confirmation) open as an in-app popup that keeps its opener, everything else goes to the browser.
fn route_new_window(
    app: &AppHandle,
    url: Url,
    features: tauri::webview::NewWindowFeatures,
) -> NewWindowResponse<tauri::Wry> {
    let shell = app.state::<Shell>();

    if shell.is_app_url(&url) {
        let label = shell.next_label("window");
        let app = app.clone();
        std::thread::spawn(move || {
            if let Ok(window) = open(&app, &label, &url, true) {
                let _ = window.set_focus();
            }
        });
        return NewWindowResponse::Deny;
    }

    let is_web = matches!(url.scheme(), "http" | "https");
    if is_web && features.size().is_some() {
        let built = WebviewWindowBuilder::new(
            app,
            shell.next_label("popup"),
            WebviewUrl::External("about:blank".parse().unwrap()),
        )
        .window_features(features)
        .title(url.host_str().unwrap_or("Dollie"))
        .on_document_title_changed(|window, title| {
            let _ = window.set_title(&title);
        })
        .build();
        if let Ok(window) = built {
            return NewWindowResponse::Create { window };
        }
    }

    open_external(&url);
    NewWindowResponse::Deny
}

fn route_download(app: &AppHandle, label: &str, event: DownloadEvent<'_>) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => {
            if let Some(path) = download_path(app, &url, destination) {
                *destination = path;
            }
        }
        DownloadEvent::Finished { url, path, success } => {
            if let Some(window) = app.get_webview_window(label) {
                dispatch(
                    &window,
                    json!({ "type": "download-finished", "url": url.as_str(), "path": path, "success": success }),
                );
            }
        }
        _ => {}
    }
    true
}

/// `~/Downloads/name.ext`, or `name (1).ext` when that exists.
fn download_path(app: &AppHandle, url: &Url, suggested: &Path) -> Option<PathBuf> {
    let dir = app.path().download_dir().ok()?;
    let name = suggested
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| url.path_segments()?.next_back().map(str::to_owned))
        .filter(|n| !n.is_empty())?;
    let candidate = PathBuf::from(&name);
    let stem = candidate.file_stem()?.to_string_lossy().into_owned();
    let extension = candidate
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    (0..100)
        .map(|n| {
            if n == 0 {
                dir.join(&name)
            } else {
                dir.join(format!("{stem} ({n}){extension}"))
            }
        })
        .find(|path| !path.exists())
}

pub fn open_external(url: &Url) {
    if matches!(url.scheme(), "http" | "https" | "mailto" | "tel") {
        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
    }
}

/// Sends a message to the page's bridge. The payload is JSON, never interpolated text.
pub fn dispatch(window: &WebviewWindow, message: Value) {
    let _ = window.eval(format!(
        "window.__dollieDesktopDispatch && window.__dollieDesktopDispatch({message})"
    ));
}

pub fn reveal(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// The window menu commands act on: the focused one, else the main one.
pub fn active(app: &AppHandle) -> Option<WebviewWindow> {
    app.webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false))
        .or_else(|| app.get_webview_window(MAIN))
}

/// Brings the app forward on `path`, opening the main window if it was never created.
pub fn navigate(app: &AppHandle, url: &Url) {
    let path = format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    match app.get_webview_window(MAIN) {
        Some(window) => {
            reveal(&window);
            dispatch(&window, json!({ "type": "navigate", "path": path }));
        }
        None => {
            let _ = open(app, MAIN, url, true);
        }
    }
}

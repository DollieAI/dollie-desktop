use serde_json::json;
use tauri::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Manager, Wry,
};

use crate::{state::Shell, updater, window};

pub const CHECK_FOR_UPDATES: &str = "check_for_updates";

/// Chrome's zoom steps, as Linear uses them.
const ZOOM_STEPS: [f64; 13] = [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

/// App commands the web app handles through `DollieDesktop.onCommand`; the path is
/// where the shell goes when the page does not handle the command.
const WEB_COMMANDS: [(&str, &str, Option<&str>, &str); 4] = [
    ("new-conversation", "New Conversation", Some("CmdOrCtrl+N"), "/"),
    ("preferences", "Settings…", Some("CmdOrCtrl+,"), "/user/profile"),
    ("home", "Home", Some("Shift+CmdOrCtrl+H"), "/"),
    ("help", "Dollie Help", None, "/docs"),
];

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let item = |id: &str, text: &str, accelerator: Option<&str>| MenuItem::with_id(app, id, text, true, accelerator);
    let web = |id: &str| {
        let (id, text, accelerator, _) = WEB_COMMANDS.iter().find(|c| c.0 == id).copied().unwrap();
        item(id, text, accelerator)
    };
    let separator = || PredefinedMenuItem::separator(app);
    let name = app.package_info().name.clone();

    let app_menu = Submenu::with_items(
        app,
        &name,
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &item(CHECK_FOR_UPDATES, "Check for Updates…", None)?,
            &item("choose-installation", "Connect to Another Installation…", None)?,
            &separator()?,
            &web("preferences")?,
            &separator()?,
            &PredefinedMenuItem::services(app, None)?,
            &separator()?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &separator()?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &web("new-conversation")?,
            &item("new-window", "New Window", Some("Shift+CmdOrCtrl+N"))?,
            &separator()?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &separator()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &item("reload", "Reload", Some("CmdOrCtrl+R"))?,
            &separator()?,
            &item("zoom-reset", "Actual Size", Some("CmdOrCtrl+0"))?,
            &item("zoom-in", "Zoom In", Some("CmdOrCtrl+="))?,
            &item("zoom-out", "Zoom Out", Some("CmdOrCtrl+-"))?,
            &separator()?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;

    let history = Submenu::with_items(
        app,
        "History",
        true,
        &[
            &web("home")?,
            &item("back", "Back", Some("CmdOrCtrl+["))?,
            &item("forward", "Forward", Some("CmdOrCtrl+]"))?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &separator()?,
            &PredefinedMenuItem::bring_all_to_front(app, None)?,
        ],
    )?;

    let help = Submenu::with_items(app, "Help", true, &[&web("help")?])?;

    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &history, &window_menu, &help])
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    let shell = app.state::<Shell>();

    if id == CHECK_FOR_UPDATES {
        updater::check_now(app.clone(), true);
        return;
    }

    if id == "choose-installation" {
        crate::installation::choose_installation(app.clone());
        return;
    }

    if id == "new-window" {
        let label = shell.next_label("window");
        let _ = window::open(app, &label, &shell.origin(), true);
        return;
    }

    let Some(target) = window::active(app) else { return };

    if let Some((name, _, _, fallback)) = WEB_COMMANDS.iter().find(|c| c.0 == id) {
        window::reveal(&target);
        window::dispatch(
            &target,
            json!({ "type": "command", "name": name, "fallbackPath": fallback }),
        );
        return;
    }

    match id {
        "reload" => {
            let _ = target.reload();
        }
        "back" => {
            let _ = target.eval("history.back()");
        }
        "forward" => {
            let _ = target.eval("history.forward()");
        }
        "zoom-in" | "zoom-out" | "zoom-reset" => {
            let mut zoom = shell.zoom.lock().unwrap();
            *zoom = match id {
                "zoom-in" => ZOOM_STEPS.iter().copied().find(|s| *s > *zoom + 0.01).unwrap_or(*zoom),
                "zoom-out" => ZOOM_STEPS
                    .iter()
                    .rev()
                    .copied()
                    .find(|s| *s < *zoom - 0.01)
                    .unwrap_or(*zoom),
                _ => 1.0,
            };
            for window in app.webview_windows().values() {
                let _ = window.set_zoom(*zoom);
            }
        }
        _ => {}
    }
}

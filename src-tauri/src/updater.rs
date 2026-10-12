use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

use crate::{
    state::{PendingUpdate, Shell},
    window,
};

const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);

/// Checks hourly, downloads in the background and tells the web app a restart will update.
pub fn start(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            check(&app, false).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

pub fn check_now(app: AppHandle, user_initiated: bool) {
    tauri::async_runtime::spawn(async move { check(&app, user_initiated).await });
}

async fn check(app: &AppHandle, user_initiated: bool) {
    let shell = app.state::<Shell>();
    let pending = shell.update.lock().unwrap().as_ref().map(|p| p.update.version.clone());

    let found = match app.updater() {
        Ok(updater) => updater.check().await.ok().flatten(),
        Err(_) => None,
    };

    let Some(update) = found else {
        if user_initiated && pending.is_none() {
            notify(
                app,
                "You're up to date",
                &format!("Dollie {} is the latest version.", app.package_info().version),
            );
        }
        return;
    };

    if pending.as_deref() != Some(update.version.as_str()) {
        let Ok(bytes) = update.download(|_, _| {}, || {}).await else {
            return;
        };
        *shell.update.lock().unwrap() = Some(PendingUpdate { update, bytes });
    }

    let version = shell
        .update
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.update.version.clone())
        .unwrap_or_default();
    for window in app.webview_windows().values() {
        window::dispatch(window, json!({ "type": "update-ready", "version": version }));
    }
    if user_initiated {
        notify(
            app,
            "Update ready",
            &format!("Dollie {version} installs when you restart."),
        );
    }
}

/// Installs the downloaded update and relaunches.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let pending = app.state::<Shell>().update.lock().unwrap().take();
    let Some(PendingUpdate { update, bytes }) = pending else {
        return Err("No update is ready.".into());
    };
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

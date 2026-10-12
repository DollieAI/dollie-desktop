//! Which Dollie installation the app shows. Dollie Cloud by default; any other
//! installation that advertises the desktop client through its public
//! `/api/v1/app/discover` endpoint can be connected instead, so private
//! installations need no build of their own.

use std::{collections::HashSet, sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{ipc::CapabilityBuilder, AppHandle, Manager};
use url::Url;

use crate::{
    auth,
    state::{Installation, Shell},
    window,
};

/// Dollie Cloud, which the app connects to until someone picks another installation.
pub const DOLLIE_CLOUD: &str = if cfg!(debug_assertions) {
    "https://api-layer.social"
} else {
    "https://app.dollie.pro"
};
pub const DOLLIE_CLOUD_CLIENT_ID: &str = "5059a42d-93c8-4d27-b24e-f4fcda05b8e4";

/// What the web app may call from an installation's pages. Nothing else.
const BRIDGE_PERMISSIONS: [&str; 10] = [
    "core:window:allow-start-dragging",
    "core:window:allow-internal-toggle-maximize",
    "notification:allow-is-permission-granted",
    "allow-set-badge-count",
    "allow-notify",
    "allow-report-theme",
    "allow-open-external",
    "allow-install-update",
    "allow-sign-in",
    "allow-choose-installation",
];

/// Origins already granted the bridge in this run.
#[derive(Default)]
pub struct Granted(Mutex<HashSet<String>>);

/// An installation as its discovery endpoint describes it.
#[derive(Clone, Serialize)]
pub struct Discovered {
    url: String,
    client_id: String,
    name: String,
    description: String,
    logo_url: Option<String>,
    logo_dark_url: Option<String>,
}

#[derive(Deserialize)]
struct Discovery {
    version: u32,
    installation_url: String,
    branding: Option<Branding>,
    desktop: Option<DesktopClient>,
}

#[derive(Deserialize)]
struct Branding {
    name: Option<String>,
    description: Option<String>,
    logo_url: Option<String>,
    logo_dark_url: Option<String>,
}

#[derive(Deserialize)]
struct DesktopClient {
    client_id: String,
    redirect_uri: String,
    scopes: Vec<String>,
}

/// `example.com`, `https://example.com/` → `https://example.com/`. Only an HTTPS server
/// address: no path, query, fragment or login details.
fn normalize(address: &str) -> Result<Url, String> {
    let address = address.trim();
    let with_scheme = if address.contains("://") {
        address.to_owned()
    } else {
        format!("https://{address}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| "Enter the address of your Dollie installation.".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().trim_matches('/').is_empty()
    {
        return Err("Enter the HTTPS address of your Dollie installation, without a path or login details.".into());
    }
    Url::parse(&url.origin().ascii_serialization()).map_err(|e| e.to_string())
}

pub async fn discover(address: &str) -> Result<Discovered, String> {
    let origin = normalize(address)?;
    let response = auth::http()
        .get(origin.join("/api/v1/app/discover").map_err(|e| e.to_string())?)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| "Could not reach that address. Check it and your connection.".to_string())?;
    if !response.status().is_success() {
        return Err("No Dollie installation answered at that address.".into());
    }
    let discovery: Discovery = response
        .json()
        .await
        .map_err(|_| "That address is not a Dollie installation.".to_string())?;

    let installation = normalize(&discovery.installation_url).ok();
    if discovery.version != 1 || installation.as_ref().map(Url::origin) != Some(origin.origin()) {
        return Err("That address is not a compatible Dollie installation.".into());
    }
    let Some(desktop) = discovery
        .desktop
        .filter(|d| d.redirect_uri == auth::REDIRECT_URI && d.scopes.iter().any(|s| s == auth::SCOPE))
        .filter(|d| !d.client_id.trim().is_empty())
    else {
        return Err(
            "This installation does not support the desktop app yet. Ask its administrator to update Dollie.".into(),
        );
    };

    let branding = discovery.branding;
    let https_only = |url: Option<String>| url.filter(|u| u.starts_with("https://"));
    Ok(Discovered {
        url: origin.to_string(),
        client_id: desktop.client_id,
        name: branding
            .as_ref()
            .and_then(|b| b.name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| origin.host_str().unwrap_or_default().to_owned()),
        description: branding
            .as_ref()
            .and_then(|b| b.description.clone())
            .unwrap_or_default(),
        logo_url: https_only(branding.as_ref().and_then(|b| b.logo_url.clone())),
        logo_dark_url: https_only(branding.and_then(|b| b.logo_dark_url)),
    })
}

/// Lets the installation's pages use the desktop bridge, once per run.
pub fn grant(app: &AppHandle, origin: &Url) -> tauri::Result<()> {
    let key = origin.origin().ascii_serialization();
    if !app.state::<Granted>().0.lock().unwrap().insert(key.clone()) {
        return Ok(());
    }
    let capability = BRIDGE_PERMISSIONS.iter().fold(
        CapabilityBuilder::new(format!("installation-{}", origin.host_str().unwrap_or("unknown")))
            .remote(format!("{key}/*"))
            .windows(["main", "window-*"]),
        |capability, permission| capability.permission(*permission),
    );
    app.add_capability(capability)
}

/// Switches the app to `installation`, remembers it, and reopens the app on it.
pub fn switch(app: &AppHandle, installation: Installation) -> Result<(), String> {
    let origin = Url::parse(&installation.url).map_err(|e| e.to_string())?;
    grant(app, &origin).map_err(|e| e.to_string())?;

    let shell = app.state::<Shell>();
    shell.set_installation(origin.clone(), installation.client_id.clone());
    {
        let mut prefs = shell.prefs.lock().unwrap();
        prefs.installation = Some(installation);
        prefs.background = None;
        prefs.dark = None;
        prefs.save(app);
    }

    window::reopen_main(app, &origin, false);
    Ok(())
}

#[tauri::command]
pub async fn find_installation(address: String) -> Result<Discovered, String> {
    discover(&address).await
}

#[tauri::command]
pub async fn connect_installation(app: AppHandle, address: String) -> Result<(), String> {
    let found = discover(&address).await?;
    switch(
        &app,
        Installation {
            url: found.url,
            client_id: found.client_id,
            name: Some(found.name),
        },
    )
}

/// Shows the connect screen in the main window.
#[tauri::command]
pub fn choose_installation(app: AppHandle) {
    let origin = app.state::<Shell>().origin();
    window::reopen_main(&app, &origin, true);
}

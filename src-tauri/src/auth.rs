//! Signing in through the system browser with the installation's Passport:
//! a public PKCE client, the token kept in the OS keychain, and a one-time
//! ticket that opens the web session in the app window.

use std::sync::Mutex;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::{state::Shell, window};

pub const REDIRECT_URI: &str = "dollie://auth/callback";
pub const SCOPE: &str = "desktop:session";

/// A browser sign-in that has started and waits for its callback.
#[derive(Default)]
pub struct PendingSignIn(Mutex<Option<(String, String)>>);

#[derive(Serialize, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct Ticket {
    url: String,
}

pub enum Outcome {
    /// The window is on its way to a signed-in page.
    SignedIn,
    /// The browser is open on the installation's sign-in page.
    Browser,
    /// No stored grant, and the caller asked not to open the browser.
    NeedsBrowser,
}

/// Opens a session from the stored grant, refreshing it once if needed; without
/// one, starts a browser sign-in unless `silent`.
pub async fn sign_in(app: &AppHandle, silent: bool) -> Result<Outcome, String> {
    if let Some(tokens) = load(app) {
        if open_session(app, &tokens.access_token).await? {
            return Ok(Outcome::SignedIn);
        }
        if let Some(refreshed) = refresh(app, &tokens).await {
            store(app, &refreshed);
            if open_session(app, &refreshed.access_token).await? {
                return Ok(Outcome::SignedIn);
            }
        }
        forget(app);
    }

    if silent {
        return Ok(Outcome::NeedsBrowser);
    }
    start_browser(app)?;
    Ok(Outcome::Browser)
}

/// Handles `dollie://auth/callback?code=…&state=…`.
pub async fn complete(app: &AppHandle, callback: &Url) -> Result<(), String> {
    let param = |name: &str| {
        callback
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    let pending = app.state::<PendingSignIn>().0.lock().unwrap().take();
    let Some((state, verifier)) = pending else {
        return Err("No sign-in is in progress.".into());
    };
    if param("state").as_deref() != Some(state.as_str()) {
        return Err("The sign-in did not match this app.".into());
    }
    let code = param("code").ok_or("The sign-in was cancelled.")?;

    let origin = app.state::<Shell>().origin();
    let client_id = app.state::<Shell>().client_id();
    let response = http()
        .post(origin.join("/oauth/token").map_err(|e| e.to_string())?)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", verifier.as_str()),
            ("code", code.as_str()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err("Dollie did not accept the sign-in.".into());
    }
    let tokens: Tokens = response.json().await.map_err(|e| e.to_string())?;
    store(app, &tokens);

    if open_session(app, &tokens.access_token).await? {
        Ok(())
    } else {
        forget(app);
        Err("Dollie did not open a session.".into())
    }
}

fn start_browser(app: &AppHandle) -> Result<(), String> {
    let verifier: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let state: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let client_id = app.state::<Shell>().client_id();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));

    let mut url = app
        .state::<Shell>()
        .origin()
        .join("/oauth/authorize")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id.as_str())
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");

    *app.state::<PendingSignIn>().0.lock().unwrap() = Some((state, verifier));
    tauri_plugin_opener::open_url(url.as_str(), None::<&str>).map_err(|e| e.to_string())
}

/// Trades the token for a one-time URL and loads it in the main window.
/// Returns false when the installation refuses the token.
async fn open_session(app: &AppHandle, access_token: &str) -> Result<bool, String> {
    let origin = app.state::<Shell>().origin();
    let response = http()
        .post(
            origin
                .join("/api/v1/desktop/session-ticket")
                .map_err(|e| e.to_string())?,
        )
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(false);
    }
    if !response.status().is_success() {
        return Err(format!("Dollie answered {}.", response.status()));
    }
    let ticket: Ticket = response.json().await.map_err(|e| e.to_string())?;
    let url = Url::parse(&ticket.url).map_err(|e| e.to_string())?;
    if !app.state::<Shell>().is_app_url(&url) {
        return Err("Dollie returned a sign-in link on another server.".into());
    }

    let main = match app.get_webview_window(window::MAIN) {
        Some(main) => main,
        None => window::open(app, window::MAIN, &url, true).map_err(|e| e.to_string())?,
    };
    window::reveal(&main);
    main.navigate(url).map_err(|e| e.to_string())?;
    Ok(true)
}

async fn refresh(app: &AppHandle, tokens: &Tokens) -> Option<Tokens> {
    let refresh_token = tokens.refresh_token.as_deref()?;
    let origin = app.state::<Shell>().origin();
    let client_id = app.state::<Shell>().client_id();
    let response = http()
        .post(origin.join("/oauth/token").ok()?)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

pub fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

fn keychain(app: &AppHandle) -> Option<keyring::Entry> {
    let host = app.state::<Shell>().origin().host_str()?.to_owned();
    keyring::Entry::new(&app.config().identifier, &host).ok()
}

fn load(app: &AppHandle) -> Option<Tokens> {
    serde_json::from_str(&keychain(app)?.get_password().ok()?).ok()
}

fn store(app: &AppHandle, tokens: &Tokens) {
    if let (Some(entry), Ok(raw)) = (keychain(app), serde_json::to_string(tokens)) {
        let _ = entry.set_password(&raw);
    }
}

fn forget(app: &AppHandle) {
    if let Some(entry) = keychain(app) {
        let _ = entry.delete_credential();
    }
}

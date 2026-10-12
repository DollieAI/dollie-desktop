use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tauri::{window::Color, AppHandle, Manager};
use tauri_plugin_updater::Update;
use url::Url;

/// The Dollie installation this app is connected to.
#[derive(Clone, Serialize, Deserialize)]
pub struct Installation {
    pub url: String,
    pub client_id: String,
    pub name: Option<String>,
}

/// What the shell remembers between launches.
#[derive(Default, Serialize, Deserialize)]
pub struct Prefs {
    pub dark: Option<bool>,
    pub background: Option<String>,
    pub installation: Option<Installation>,
}

impl Prefs {
    fn path(app: &AppHandle) -> Option<PathBuf> {
        app.path().app_config_dir().ok().map(|dir| dir.join("desktop.json"))
    }

    pub fn load(app: &AppHandle) -> Self {
        Self::path(app)
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app: &AppHandle) {
        if let Some(path) = Self::path(app) {
            let _ = path.parent().map(fs::create_dir_all);
            if let Ok(raw) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, raw);
            }
        }
    }

    /// The colour a window paints before the page does, so a dark theme never flashes white.
    pub fn background_hex(&self) -> &str {
        match (&self.background, self.dark) {
            (Some(hex), _) => hex,
            (None, Some(true)) => "#0a0a0a",
            _ => "#ffffff",
        }
    }

    pub fn background_color(&self) -> Color {
        parse_hex(self.background_hex()).unwrap_or(Color(255, 255, 255, 255))
    }
}

pub fn parse_hex(hex: &str) -> Option<Color> {
    let digits = hex.strip_prefix('#').filter(|d| d.len() == 6)?;
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some(Color(channel(0)?, channel(2)?, channel(4)?, 255))
}

pub struct PendingUpdate {
    pub update: Update,
    pub bytes: Vec<u8>,
}

pub struct Shell {
    installation: Mutex<(Url, String)>,
    pub prefs: Mutex<Prefs>,
    pub zoom: Mutex<f64>,
    pub update: Mutex<Option<PendingUpdate>>,
    windows: AtomicUsize,
    /// Set while the app windows are being rebuilt, so the moment with no window does not quit the app.
    pub reopening: AtomicBool,
}

impl Shell {
    pub fn new(origin: Url, client_id: String, prefs: Prefs) -> Self {
        Self {
            installation: Mutex::new((origin, client_id)),
            prefs: Mutex::new(prefs),
            zoom: Mutex::new(1.0),
            update: Mutex::new(None),
            windows: AtomicUsize::new(0),
            reopening: AtomicBool::new(false),
        }
    }

    pub fn next_label(&self, prefix: &str) -> String {
        format!("{prefix}-{}", self.windows.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// The installation's address, `https://host/`.
    pub fn origin(&self) -> Url {
        self.installation.lock().unwrap().0.clone()
    }

    /// The installation's desktop OAuth client.
    pub fn client_id(&self) -> String {
        self.installation.lock().unwrap().1.clone()
    }

    pub fn set_installation(&self, origin: Url, client_id: String) {
        *self.installation.lock().unwrap() = (origin, client_id);
    }

    /// Resolves a path or URL to a page of this installation, refusing anything on another origin.
    pub fn app_url(&self, path_or_url: &str) -> Option<Url> {
        let origin = self.origin();
        let url = origin.join(path_or_url).ok()?;
        (url.origin() == origin.origin()).then_some(url)
    }

    pub fn is_app_url(&self, url: &Url) -> bool {
        url.origin() == self.origin().origin()
    }
}

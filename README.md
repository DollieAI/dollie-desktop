# Dollie Desktop App

A thin Tauri 2 shell around a Dollie installation's web app. The web app owns behaviour; the shell provides native capabilities through one bridge, `window.DollieDesktop`. Design and decisions: `dollie/work-items/desktop-app/` in the Dollie vault. Linear's desktop app was the reference (`linear-desktop-map.md` there).

## What the shell does

- Opens Dollie Cloud, or any installation the person connects through its `/api/v1/app/discover` endpoint (**Connect to Another Installation…**).
- Signs in through the default browser with the installation's Passport (public PKCE client, redirect `dollie://auth/callback`), keeps the token in the OS keychain per installation, and opens the web session with a one-time ticket.
- Keeps the window on the installation: other sites open in the browser; off-site flows started in the app are sent to the browser by chq and handed back with a `dollie://` link.
- Native menu, close-to-hide on macOS, single instance, deep links (`dollie://path`), Dock badge, background notifications, downloads to ~/Downloads, an offline screen, and a signed auto-updater.
- macOS: overlay title bar; chq reserves the traffic-light row when `html[data-desktop="macos"]` is set.

## The bridge

`src-tauri/src/bridge.js` is injected into the installation's pages and defines `window.DollieDesktop` (contract `2`). chq consumes it in `lib/desktop.ts` and `components/providers/DesktopBridge.tsx`. Add new capabilities to the bridge with a contract bump; keep app-specific behaviour in chq so it ships without a desktop release.

The bridge's commands are granted at runtime to the connected installation only (`src-tauri/src/installation.rs`). The bundled start page (`src/index.html`) has its own capability (`src-tauri/capabilities/boot.json`).

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- Node.js 18+
- Xcode Command Line Tools (macOS): `xcode-select --install`

## Development

```bash
npm install
npm run dev
```

Debug builds open `https://api-layer.social` (the local chq); release builds open `https://app.dollie.pro`. Both are set in `src-tauri/src/installation.rs`.

## Building and updates

```bash
npm run build
```

Outputs to `src-tauri/target/release/bundle/`. Updates are signed with the key in `~/.tauri/dollie-desktop.key`; CI needs it as `TAURI_SIGNING_PRIVATE_KEY` and its password as `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The app checks `https://github.com/DollieAI/dollie-desktop/releases/latest/download/latest.json`.

## Icons

Place your app icon as `src-tauri/icons/app-icon.png` (1024x1024), then run:

```bash
npm run icons
```

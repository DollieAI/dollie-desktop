# Dollie Desktop App

Tauri wrapper for the Dollie web app with native desktop features.

## Features

- System tray with quick access
- Native notifications (bridged from Laravel Echo)
- Dock badge count (macOS) / tray tooltip (Windows/Linux)
- Deep linking (`dollie://path/to/page`)
- Auto-start on login
- Window state persistence
- Global shortcut to summon app

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- Node.js 18+
- Xcode Command Line Tools (macOS): `xcode-select --install`

## Development

```bash
cd desktop-app
npm install
npm run dev
```

This opens the app pointing to `https://app.dollie.pro`. For local development, edit `src-tauri/tauri.conf.json` and change `frontendDist` to your local URL.

## Building

```bash
npm run build
```

Outputs to `src-tauri/target/release/bundle/`.

## Icons

Place your app icon as `src-tauri/icons/app-icon.png` (1024x1024), then run:

```bash
npm run icons
```

This generates all required icon sizes.

## Integration with Web App

Copy `tauri-bridge.js` to your web app or import it. Usage:

```javascript
import DollieBridge from './tauri-bridge.js';

// In your Echo notification listener
Echo.private(`App.Models.User.${userId}`)
    .notification((notification) => {
        // Show native notification when app is in background
        DollieBridge.notifyIfBackground(
            notification.title,
            notification.body
        );
    });

// Update badge when unread count changes
Livewire.on('unreadCountChanged', (count) => {
    DollieBridge.setBadgeCount(count);
});
```

## Deep Links

Register the app to handle `dollie://` URLs. After installing the built app, links like `dollie://tasks/123` will open the app and navigate to `https://app.dollie.pro/tasks/123`.

## Configuration

Key settings in `src-tauri/tauri.conf.json`:

- `build.frontendDist`: The URL to load (production or local dev)
- `app.security.dangerousRemoteUrlIpcAccess`: Domains allowed to use Tauri APIs
- `plugins.deep-link.desktop.schemes`: URL schemes to register

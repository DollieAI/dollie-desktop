// The desktop contract the Dollie web app reads as `window.DollieDesktop`.
// The shell prepends `const DOLLIE_DESKTOP_CONFIG = {...}` before this file.
(function () {
  if (window.DollieDesktop || location.origin !== DOLLIE_DESKTOP_CONFIG.origin) return;

  const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args);
  const handlers = { command: new Set(), navigate: new Set(), updateReady: new Set(), downloadFinished: new Set() };

  function subscribe(kind) {
    return (handler) => {
      handlers[kind].add(handler);
      return () => handlers[kind].delete(handler);
    };
  }

  function emit(kind, payload) {
    let handled = false;
    for (const handler of handlers[kind]) {
      if (handler(payload) !== false) handled = true;
    }
    return handled;
  }

  // Called by the shell with a JSON-encoded message.
  Object.defineProperty(window, "__dollieDesktopDispatch", {
    value(message) {
      switch (message.type) {
        case "navigate":
          if (!emit("navigate", message.path)) location.assign(message.path);
          break;
        case "command":
          if (!emit("command", message.name) && message.fallbackPath) location.assign(message.fallbackPath);
          break;
        case "update-ready":
          emit("updateReady", message.version);
          break;
        case "download-finished":
          emit("downloadFinished", { url: message.url, path: message.path, success: message.success });
          break;
      }
    },
  });

  // External links leave the app; same-origin links stay in it.
  document.addEventListener(
    "click",
    (event) => {
      if (event.defaultPrevented || event.button > 1) return;
      const anchor = event.target instanceof Element ? event.target.closest("a[href]") : null;
      if (!anchor || anchor.hasAttribute("download")) return;
      const url = new URL(anchor.href, location.href);
      if (url.origin === location.origin || !/^(https?|mailto|tel):$/.test(url.protocol)) return;
      event.preventDefault();
      invoke("open_external", { url: url.href });
    },
    true,
  );

  const root = document.documentElement;
  root.dataset.desktop = DOLLIE_DESKTOP_CONFIG.platform;
  window.addEventListener("focus", () => root.classList.remove("desktop-blur"));
  window.addEventListener("blur", () => root.classList.add("desktop-blur"));

  Object.defineProperty(window, "DollieDesktop", {
    value: Object.freeze({
      contract: 2,
      platform: DOLLIE_DESKTOP_CONFIG.platform,
      version: DOLLIE_DESKTOP_CONFIG.version,
      setBadgeCount: (count) => invoke("set_badge_count", { count: Math.max(0, Math.floor(count || 0)) }),
      notify: ({ title, body }) => invoke("notify", { title, body: body ?? null }),
      reportTheme: ({ dark, background }) => invoke("report_theme", { dark: !!dark, background: background ?? null }),
      openExternal: (url) => invoke("open_external", { url }),
      installUpdate: () => invoke("install_update"),
      // Resolves "signed-in", "browser" (the browser is open on the sign-in page) or "needs-browser" (silent, no saved sign-in).
      signIn: ({ silent = false } = {}) => invoke("sign_in", { silent }),
      // Contract 2: shows the screen for connecting to another Dollie installation.
      chooseInstallation: () => invoke("choose_installation"),
      onCommand: subscribe("command"),
      onNavigate: subscribe("navigate"),
      onUpdateReady: subscribe("updateReady"),
      onDownloadFinished: subscribe("downloadFinished"),
    }),
  });
})();

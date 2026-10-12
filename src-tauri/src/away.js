// An app window has no browser chrome. When a flow (connecting an integration,
// billing, a provider's sign-in) takes it to another site, this bar is the way back.
(function () {
  const boot = ["localhost", "tauri.localhost", "127.0.0.1"].includes(location.hostname);
  if (boot || location.origin === DOLLIE_DESKTOP_CONFIG.origin || !/^https?:$/.test(location.protocol)) return;

  function mount() {
    if (document.getElementById("dollie-away")) return;
    const host = document.createElement("div");
    host.id = "dollie-away";
    host.style.cssText = "position:fixed;left:16px;bottom:16px;z-index:2147483647;";
    const root = host.attachShadow({ mode: "closed" });
    root.innerHTML = `
      <style>
        div { display:flex; align-items:center; gap:2px; padding:4px; border-radius:999px;
              font:500 13px/1 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
              background:rgba(24,24,27,.92); color:#fafafa; box-shadow:0 4px 16px rgba(0,0,0,.25);
              -webkit-backdrop-filter:blur(8px); backdrop-filter:blur(8px); }
        button { all:unset; cursor:default; padding:7px 11px; border-radius:999px; }
        button:hover { background:rgba(255,255,255,.12); }
        span { padding:0 8px; color:#a1a1aa; max-width:220px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
      </style>
      <div role="toolbar" aria-label="Dollie">
        <button type="button" id="back">← Back to Dollie</button>
        <span></span>
        <button type="button" id="browser">Open in browser</button>
      </div>`;
    root.querySelector("span").textContent = location.hostname;
    root.getElementById("back").addEventListener("click", () => {
      if (history.length > 1) history.back();
      else location.assign(DOLLIE_DESKTOP_CONFIG.origin);
    });
    // The shell sends a plain window.open of another site to the system browser.
    root.getElementById("browser").addEventListener("click", () => window.open(location.href, "_blank"));
    document.documentElement.appendChild(host);
  }

  if (document.documentElement) mount();
  document.addEventListener("DOMContentLoaded", mount);
})();

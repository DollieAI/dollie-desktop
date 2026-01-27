/**
 * Dollie Desktop Bridge
 *
 * Include this in your web app to enable desktop features when running in Tauri.
 * All methods are safe to call even when not in Tauri (they'll just no-op).
 */

const DollieBridge = {
    /**
     * Check if we're running inside Tauri
     */
    isTauri() {
        return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;
    },

    /**
     * Set the dock/taskbar badge count
     * @param {number} count - Number to show (0 to clear)
     */
    async setBadgeCount(count) {
        if (!this.isTauri()) return;

        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('set_badge_count', { count });
    },

    /**
     * Clear the badge
     */
    async clearBadge() {
        if (!this.isTauri()) return;

        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('clear_badge');
    },

    /**
     * Send a native notification
     * @param {string} title - Notification title
     * @param {string} body - Notification body
     */
    async notify(title, body) {
        if (!this.isTauri()) return;

        const { isPermissionGranted, requestPermission, sendNotification } =
            await import('@tauri-apps/plugin-notification');

        let permitted = await isPermissionGranted();
        if (!permitted) {
            const permission = await requestPermission();
            permitted = permission === 'granted';
        }

        if (permitted) {
            sendNotification({ title, body });
        }
    },

    /**
     * Show notification only when window is not focused
     * @param {string} title - Notification title
     * @param {string} body - Notification body
     */
    async notifyIfBackground(title, body) {
        if (document.visibilityState === 'hidden' || !document.hasFocus()) {
            await this.notify(title, body);
        }
    },

    /**
     * Bring the app window to front
     */
    async showWindow() {
        if (!this.isTauri()) return;

        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('show_window');
    },

    /**
     * Enable auto-start on login
     */
    async enableAutostart() {
        if (!this.isTauri()) return;

        const { enable } = await import('@tauri-apps/plugin-autostart');
        await enable();
    },

    /**
     * Disable auto-start on login
     */
    async disableAutostart() {
        if (!this.isTauri()) return;

        const { disable } = await import('@tauri-apps/plugin-autostart');
        await disable();
    },

    /**
     * Check if auto-start is enabled
     * @returns {Promise<boolean>}
     */
    async isAutostartEnabled() {
        if (!this.isTauri()) return false;

        const { isEnabled } = await import('@tauri-apps/plugin-autostart');
        return await isEnabled();
    },

    /**
     * Register a global keyboard shortcut
     * @param {string} shortcut - e.g., "CommandOrControl+Shift+D"
     * @param {Function} callback - Function to call when triggered
     */
    async registerShortcut(shortcut, callback) {
        if (!this.isTauri()) return;

        const { register } = await import('@tauri-apps/plugin-global-shortcut');
        await register(shortcut, callback);
    },
};

// Auto-export for different module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = DollieBridge;
}
if (typeof window !== 'undefined') {
    window.DollieBridge = DollieBridge;
}

export default DollieBridge;

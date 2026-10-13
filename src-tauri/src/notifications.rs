//! System notifications on macOS through UserNotifications. The notification
//! plugin posts through the deprecated NSUserNotification API and never asks
//! for permission, so macOS drops what it sends from an app nobody allowed.

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNUserNotificationCenter,
};

/// UserNotifications aborts the process outside an app bundle, as in `tauri dev`.
fn bundled() -> bool {
    std::env::current_exe()
        .map(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

/// Asks once; macOS remembers the answer and later calls return it without a prompt.
pub fn request_permission() -> bool {
    if !bundled() {
        return false;
    }
    let done = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
    UNUserNotificationCenter::currentNotificationCenter().requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound | UNAuthorizationOptions::Badge,
        &done,
    );
    true
}

/// Returns false when it cannot post, so the caller can fall back to the plugin.
pub fn show(title: &str, body: Option<&str>) -> bool {
    if !bundled() {
        return false;
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    if let Some(body) = body {
        content.setBody(&NSString::from_str(body));
    }
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&id.to_string()),
        &content,
        None,
    );
    UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&request, None);
    true
}

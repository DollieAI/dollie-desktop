fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "set_badge_count",
            "request_notifications",
            "notify",
            "report_theme",
            "open_external",
            "install_update",
            "sign_in",
            "find_installation",
            "connect_installation",
            "choose_installation",
        ])),
    )
    .expect("failed to run tauri-build");
}

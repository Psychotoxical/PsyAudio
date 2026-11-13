/// Every command the frontend may call. Each gets an `allow-…` permission that the main
/// window's capability grants explicitly.
const COMMANDS: &[&str] = &["frontend_ready", "file_manager_menu", "load_settings", "save_settings", "demo_state"];

fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)))
        .expect("failed to run tauri-build");
}

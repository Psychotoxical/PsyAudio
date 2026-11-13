/// Every command the frontend may call. Each gets an `allow-…` permission that the main
/// window's capability grants explicitly.
const COMMANDS: &[&str] = &["frontend_ready", "file_manager_menu", "load_settings", "save_settings", "demo_state", "add_paths", "read_file_info", "reveal_folder", "input_extensions", "encode_options", "tool_status", "codec_problems", "plan_requirement", "estimate_kbps", "set_language", "naming_options", "preview_name", "tags_from_name", "open_folder", "effect_catalogue", "effect_lines", "check_existing", "start_conversion", "cancel_conversion", "clear_run_title", "scan_folder", "first_folder", "programs", "reset_settings", "system_info"];

fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)))
        .expect("failed to run tauri-build");
}

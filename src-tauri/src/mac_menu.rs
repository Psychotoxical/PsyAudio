//! The macOS menu bar: Tauri's default menu, except that "Quit" (⌘Q) goes through the same
//! question as the close button while work runs. The system's own Quit item ends the app at once
//! (no exit request reaches PsyAudio), so it is replaced by an item of our own.

use tauri::menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

/// The id of our Quit item.
pub const QUIT_ID: &str = "quit";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let pkg = app.package_info();
    let about = AboutMetadata {
        name: Some(pkg.name.clone()),
        version: Some(pkg.version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        authors: app.config().bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };
    let quit = MenuItem::with_id(app, QUIT_ID, format!("Quit {}", pkg.name), true, Some("CmdOrCtrl+Q"))?;
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                pkg.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?,
            &Submenu::with_items(app, "File", true, &[&PredefinedMenuItem::close_window(app, None)?])?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(app, "View", true, &[&PredefinedMenuItem::fullscreen(app, None)?])?,
            &Submenu::with_items(
                app,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None)?,
                    &PredefinedMenuItem::maximize(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::close_window(app, None)?,
                ],
            )?,
        ],
    )
}

pub fn on_event(app: &AppHandle, event: MenuEvent) {
    if event.id() == QUIT_ID {
        crate::close::close_or_ask(app);
    }
}

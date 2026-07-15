pub mod background;
mod commands;
pub mod db;
pub mod dll;
pub mod downloads;
mod indicator;
pub mod models;
pub mod presets;
mod remote;
mod scanners;
pub mod swap;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = db::Db::open().expect("failed to open Uplift database");

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .manage(db)
        .invoke_handler(tauri::generate_handler![
            commands::get_library,
            commands::scan_games,
            commands::add_manual_game,
            commands::refresh_remote,
            commands::download_dll,
            commands::swap_dll,
            commands::restore_dll,
            commands::set_game_prefs,
            commands::get_game_presets,
            commands::set_game_preset,
            commands::get_dlss_indicator,
            commands::set_dlss_indicator,
            commands::get_settings,
            commands::set_settings,
        ])
        .setup(|app| {
            // Tray: Uplift lives here between sessions so the poll loop keeps
            // running after the window is closed.
            let open = MenuItem::with_id(app, "open", "Open Uplift", true, None::<&str>)?;
            let check = MenuItem::with_id(app, "check", "Check for updates now", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &check, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Uplift")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "check" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = background::run_cycle(&handle).await;
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Start hidden when launched at login with --hidden.
            if std::env::args().any(|a| a == "--hidden") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            background::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides to tray when the setting is on.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let db = window.app_handle().state::<db::Db>();
                let minimize = db
                    .get_settings()
                    .map(|s| s.minimize_to_tray)
                    .unwrap_or(true);
                if minimize {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Uplift");
}

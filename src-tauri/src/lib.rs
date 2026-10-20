mod commands;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tx_core::engine::{Engine, Notice};
use tx_core::paths;

/// Started at login: keep watching in the tray without opening the window.
const HIDDEN_ARG: &str = "--hidden";

fn show_main(app: &AppHandle, path: Option<String>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if let Some(path) = path {
        let _ = app.emit("navigate", path);
    }
}

fn notifier(app: AppHandle) -> impl Fn(Notice) + Send + Sync + 'static {
    move |n: Notice| {
        #[cfg(target_os = "linux")]
        {
            // notify-rust can wait for a click on Linux; the plugin cannot.
            let app = app.clone();
            std::thread::spawn(move || {
                let shown = notify_rust::Notification::new()
                    .appname("tx")
                    .summary(&n.title)
                    .body(&n.body)
                    .action("default", "Open")
                    .show();
                if let Ok(handle) = shown {
                    handle.wait_for_action(|action| {
                        if action == "default" {
                            show_main(&app, Some(format!("/v/{}", n.video_id)));
                        }
                    });
                }
            });
        }
        #[cfg(not(target_os = "linux"))]
        {
            use tauri_plugin_notification::NotificationExt;
            let _ = n.video_id;
            if let Err(e) = app
                .notification()
                .builder()
                .title(&n.title)
                .body(&n.body)
                .show()
            {
                log::warn!("notification failed: {e}");
            }
        }
    }
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open tx", true, None::<&str>)?;
    let check = MenuItem::with_id(app, "check", "Check now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit tx", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &check, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    TrayIconBuilder::with_id("tx")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("tx")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app, None),
            "check" => app.state::<std::sync::Arc<Engine>>().wake.notify_one(),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app, None)
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("tx".into()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stderr,
                ))
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_ARG]),
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let engine = Engine::open(notifier(app.handle().clone()))?;
            app.manage(engine.clone());
            tauri::async_runtime::spawn(engine.run_forever());
            tray(app.handle())?;
            // A watcher is only useful while running: start at login unless the user turned it off.
            {
                use tauri_plugin_autostart::ManagerExt;
                let marker = paths::data_dir().join(".autostart-initialized");
                if !marker.exists() {
                    let _ = app.autolaunch().enable();
                    let _ = std::fs::write(marker, "");
                }
            }
            if !std::env::args().any(|a| a == HIDDEN_ARG) {
                show_main(app.handle(), None);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Keep watching in the tray; quit from the tray menu.
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::videos,
            commands::video,
            commands::transcript,
            commands::analyze,
            commands::check,
            commands::channels,
            commands::set_watching,
            commands::import_subscriptions,
            commands::add_channel,
            commands::settings,
            commands::save_settings,
            commands::models,
            commands::autostart,
            commands::set_autostart,
            commands::open_url,
            commands::chat,
            commands::chat_send,
            commands::chat_stop,
            commands::chat_clear,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tx");
}

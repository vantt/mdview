//! mdview desktop shell (Tauri v2). A thin native window onto the local mdview
//! daemon (PRD §7.1/§7.5): ensure the daemon is up (spawn `mdview serve` if
//! not), open a window pointing at its URL, keep it alive in the tray, and
//! coordinate a single instance.
//!
//! NOTE: not compiled in the default workspace build — Tauri needs system libs
//! (webkit2gtk/gtk3 on Linux). See README.md for prerequisites.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mdview_core::daemon;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

fn main() {
    // Single daemon owns the registry; the window is just a client.
    let base = ensure_daemon();

    // A native window needs a graphical display. Over a plain SSH session there
    // is none, and GTK init would panic with a cryptic error. Detect that and
    // point the user at the web UI (the daemon is already running) instead.
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("mdview-desktop needs a graphical display, but none was found");
        eprintln!("(DISPLAY / WAYLAND_DISPLAY unset — e.g. a plain SSH session).\n");
        eprintln!("The mdview server is running — just open it in a browser:");
        eprintln!("    {base}\n");
        eprintln!("To get the native window, run this on a desktop session, or over");
        eprintln!("SSH with X forwarding: ssh -X <host>");
        std::process::exit(0);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Second launch → focus the existing window instead of opening another.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(move |app| {
            let url = tauri::Url::parse(&base).expect("valid daemon url");
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("mdview")
                .inner_size(1200.0, 800.0)
                .min_inner_size(600.0, 400.0)
                .build()?;

            // Close-to-tray: hide instead of quitting, so the daemon keeps
            // serving the agent workflow (PRD §7.5).
            let hide_target = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = hide_target.hide();
                }
            });

            build_tray(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mdview desktop");
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show mdview", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .tooltip("mdview")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                show_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Attach to a running daemon, or spawn `mdview serve` and wait for it.
fn ensure_daemon() -> String {
    if let Some(info) = daemon::running_daemon() {
        return info.base_url();
    }
    let _ = spawn_mdview_serve();
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(150));
        if let Some(info) = daemon::running_daemon() {
            return info.base_url();
        }
    }
    let cfg = mdview_core::Config::load();
    format!("http://{}:{}", cfg.server.host, cfg.server.port)
}

fn spawn_mdview_serve() -> std::io::Result<()> {
    std::process::Command::new(find_mdview())
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

/// Prefer a `mdview` binary next to this executable; else rely on PATH.
fn find_mdview() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(if cfg!(windows) {
                "mdview.exe"
            } else {
                "mdview"
            });
            if sibling.exists() {
                return sibling;
            }
        }
    }
    PathBuf::from("mdview")
}

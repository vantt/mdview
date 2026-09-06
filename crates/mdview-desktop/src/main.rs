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
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

fn main() {
    // Single daemon owns the registry; the window is just a client.
    let startup = ensure_daemon();

    // A native window needs a graphical display. Over a plain SSH session there
    // is none, and GTK init would panic with a cryptic error. Detect that and
    // point the user at the web UI (the daemon is already running) instead.
    // There is no window here to show a dialog in either, so this branch keeps
    // reporting everything (URL, token, any startup issue) on stderr — the
    // console is exactly what a headless SSH session still has.
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("mdview-desktop needs a graphical display, but none was found");
        eprintln!("(DISPLAY / WAYLAND_DISPLAY unset — e.g. a plain SSH session).\n");
        eprintln!("The mdview server is running — just open it in a browser:");
        eprintln!("    {}\n", startup.url);
        if let Some(token) = &startup.token {
            eprintln!("First-time sign-in token (also saved to ~/.mdview/config.toml):");
            eprintln!("    {token}\n");
        }
        if let Some(issue) = &startup.issue {
            eprintln!("warning: {issue}\n");
        }
        eprintln!("To get the native window, run this on a desktop session, or over");
        eprintln!("SSH with X forwarding: ssh -X <host>");
        std::process::exit(0);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Second launch → focus the existing window instead of opening another.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(move |app| {
            let handle = app.handle();

            // Surface anything the user needs to act on via a native dialog —
            // a release build has no console (`windows_subsystem = "windows"`)
            // for eprintln to reach, so this is the only way a low-tech user
            // ever sees it.
            if let Some(issue) = &startup.issue {
                report_daemon_issue(handle, issue);
            }
            if let Some(token) = &startup.token {
                show_login_token(handle, token);
            }

            let url = tauri::Url::parse(&startup.url).expect("valid daemon url");
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

/// Non-blocking native dialog showing the first-run login token so the user
/// can copy it into the Sign in page. Only shown when this launch itself
/// spawned the daemon (see `ensure_daemon`), since a fresh daemon process
/// always invalidates every prior session (`auth.rs` — sessions are
/// in-memory, daemon-lifetime only), so a fresh login is genuinely required.
fn show_login_token(app: &tauri::AppHandle, token: &str) {
    app.dialog()
        .message(format!(
            "mdview needs a one-time sign-in for this session.\n\n\
             Token (also saved to ~/.mdview/config.toml):\n\n    {token}\n\n\
             Copy it and paste it into the Token field on the Sign in page \
             that just opened. You can change it later in Settings."
        ))
        .title("mdview — sign-in token")
        .kind(MessageDialogKind::Info)
        .show(|_| {});
}

/// Non-blocking native dialog reporting a daemon spawn/readiness failure —
/// the desktop shell used to fall back to this silently (bug: falling back
/// without telling the user anything failed).
fn report_daemon_issue(app: &tauri::AppHandle, issue: &str) {
    app.dialog()
        .message(issue)
        .title("mdview — startup problem")
        .kind(MessageDialogKind::Error)
        .show(|_| {});
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

/// How many times, and how often, the desktop shell polls for daemon
/// readiness after spawning it before giving up and falling back (2s total,
/// matching the CLI's own `mdview::runtime::ensure_bind`).
const READY_POLL_ATTEMPTS: u32 = 20;
const READY_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// What `ensure_daemon` resolved before any window exists to show it in.
struct DaemonStartup {
    /// A real, navigable URL for this machine's WebView — never the raw bind
    /// host (which may be a wildcard like `0.0.0.0` that nothing can dial).
    url: String,
    /// The current login token, when this launch is the one that spawned the
    /// daemon (so a fresh sign-in is genuinely required — see
    /// `show_login_token`).
    token: Option<String>,
    /// A human-readable description of a spawn/readiness failure, if one
    /// happened, so the caller can surface it instead of failing silently.
    issue: Option<String>,
}

/// Attach to a running daemon, or spawn `mdview serve` and wait for it.
/// Shares the CLI's own spawn-gate/readiness coordination
/// (`mdview_core::daemon::ensure_bind`) instead of an independent, less
/// robust copy, so both launchers behave the same on a failed spawn or a
/// port the server auto-incremented past.
fn ensure_daemon() -> DaemonStartup {
    let spawned_by_us = daemon::running_daemon().is_none();
    let result = daemon::ensure_bind(
        READY_POLL_ATTEMPTS,
        READY_POLL_INTERVAL,
        spawn_mdview_serve,
        |e| eprintln!("mdview-desktop: failed to auto-spawn daemon: {e}"),
    );
    let (issue, host, port) = match result {
        Ok((host, port)) => (None, host, port),
        Err((host, port)) => (
            Some(
                "mdview couldn't confirm its background service started in time. \
                 The window may show a connection error — try restarting mdview."
                    .to_string(),
            ),
            host,
            port,
        ),
    };
    // A fresh daemon process starts with no sessions (`auth.rs`: sessions are
    // in-memory, daemon-lifetime only), so whoever caused it to spawn just
    // now is exactly the case that needs the token to sign back in.
    let token = if spawned_by_us {
        mdview_core::Config::load().server.web_secret
    } else {
        None
    };
    DaemonStartup {
        url: daemon::loopback_url(&host, port),
        token,
        issue,
    }
}

fn spawn_mdview_serve() -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(find_mdview());
    cmd.arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    mdview_core::process::apply_detach(&mut cmd);
    cmd.spawn().map(|_| ())
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

# mdview-desktop

Native desktop shell for mdview (Tauri v2). A thin window onto the local mdview
daemon: it ensures the daemon is running (spawning `mdview serve` if needed),
opens a native window pointing at the daemon URL, keeps running in the system
tray when the window is closed, and coordinates a single instance. See PRD.md
§7.1/§7.5 for the design.

> **Status: compiles, links, and runs** (verified on Linux once webkit2gtk/gtk3/
> dbus dev libs were installed). On a headless machine the process starts and
> only stops at GTK init because there is no display — expected. The GUI window
> itself has not been visually exercised (no display in the build environment);
> run it on a desktop session to see the window. Bundling (.dmg/.deb/.AppImage/
> .exe) still needs icons + `bundle.active = true`.

It is intentionally **excluded from the root Cargo workspace** so the verified
core/CLI build never depends on these system libraries.

## Prerequisites

- Rust (stable)
- **Linux:** `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev build-essential`
- **macOS:** Xcode command line tools
- **Windows:** WebView2 runtime (preinstalled on Win 11) + MSVC toolchain
- The `mdview` binary must be installed (the shell spawns `mdview serve`).

## Build & run

```sh
cd crates/mdview-desktop

# icons are required only for bundling; generate them once:
#   cargo tauri icon path/to/logo.png
# (bundle is disabled by default in tauri.conf.json, so dev build/run works without icons)

cargo build            # or: cargo run
```

For distributable bundles (.dmg/.deb/.AppImage/.exe), set `bundle.active = true`
in `tauri.conf.json`, add icons, and use `cargo tauri build`.

## How it works

1. `ensure_daemon()` checks `~/.mdview/daemon.lock` + `/health`
   (`mdview_core::daemon`). If no daemon answers, it spawns `mdview serve` and
   waits for the lock.
2. Opens a `WebviewWindow` at the daemon URL (e.g. `http://127.0.0.1:7700`).
3. Closing the window hides to tray (daemon keeps serving); Quit exits.
4. `tauri-plugin-single-instance` focuses the existing window on a second launch.

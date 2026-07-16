# Reading Map

Where each area of this project lives. bee-scribing owns this file: it is
updated whenever an area spec is created or moved. Read this before any broad
search — it answers "where does X live" without a grep.

| Area | Spec | Code entry points |
|---|---|---|
| Settings | `docs/specs/settings.md` | `crates/mdview-core/src/config.rs`, `crates/mdview/src/server.rs`, `crates/mdview/src/views.rs`, `crates/mdview/src/runtime.rs` |
| Doctor | `docs/specs/doctor.md` | `crates/mdview/src/doctor.rs`, `crates/mdview/src/cli.rs` |
| Daemon lifecycle | `docs/specs/daemon.md` | `crates/mdview/src/runtime.rs`, `crates/mdview-core/src/daemon.rs`, `crates/mdview/src/server.rs`, `crates/mdview/src/cli.rs` |
| Web interface (nav chrome) | `docs/specs/web-interface.md` | `crates/mdview/src/views.rs`, `crates/mdview/assets/app.js`, `crates/mdview/assets/app.css`, `crates/mdview/assets/atelier/components.css` |
| Appearance (visual style + Light/Dark scheme) | `docs/specs/appearance.md` | `crates/mdview/assets/atelier/`, `crates/mdview/assets/app.css`, `crates/mdview/src/views.rs`, `crates/mdview/assets/app.js`, `crates/mdview/src/server.rs`, `crates/mdview-desktop/ui/index.html` |

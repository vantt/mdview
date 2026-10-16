# Code Review: `mdview update` self-upgrade

## Scope
- Files: crates/mdview/src/update.rs (new, 379 LOC), crates/mdview/src/cli.rs, crates/mdview/src/main.rs, Cargo.toml / Cargo.lock / crates/mdview/Cargo.toml (sha2), README.md, docs/usage.md
- Checked against: .github/workflows/release.yml (asset `mdview-${{ matrix.target }}`, 4 targets), install.sh, runtime.rs (`stop_daemon`, `spawn_daemon_detached`, `replace_stale_daemon`), mdview-core config (`data_dir` = `$HOME/.mdview`)
- Asset names, repo slug (`CARGO_PKG_REPOSITORY` = https://github.com/vantt/mdview) and the target mapping match the release workflow.

## Overall
The happy path is sound. Verification happens before anything changes, the binary is smoke-tested before the swap, the Unix swap is an atomic same-directory rename, and the token goes only to api.github.com. The real defects are in the failure and privilege paths. A failed swap leaves the daemon stopped. Under `sudo` (which the docs recommend), the daemon can be restarted as root. On Windows, a parked `.old` that is still in use blocks every later update and shows a misleading error.

## High

### H1. `sudo mdview update` can restart the user's daemon as root, or skip the restart without saying so
`update.rs:284-300` together with `config.rs:130` (`data_dir()` comes from `$HOME`).
- If sudo keeps HOME (`sudo -E`, or a sudoers `env_keep += HOME`, which macOS used to ship by default), then `running_daemon()` finds the user's daemon, root stops it, and `exe restart` starts `mdview serve` **as root**. That root daemon serves the user's files over HTTP and creates root-owned lock and registry/SQLite files in `~/.mdview`. Later non-root `mdview` calls then hit permission errors, and the HTTP file server runs with root privileges.
- If sudo resets HOME (current Ubuntu/Debian), root looks in `/root/.mdview`, finds no daemon, and quietly skips the restart. The user's daemon keeps running the old version. Upgrades fix themselves on the next CLI/MCP call through `replace_stale_daemon`. Downgrades (`--version` to an older tag) never do, because `needs_restart` refuses to downgrade.
- Fix: when euid is 0 and `SUDO_UID` is set (Unix), skip the stop/restart. Print "binary updated; run `mdview restart` as your user to restart the daemon." Never start `serve` as root on the user's behalf.

## Medium

### M1. A failed swap leaves the daemon stopped
`update.rs:284-288`. The daemon is stopped before `swap_in`, and the `?` on `swap_in` returns before any restart. On Windows this is a realistic path: the `.old` file is still in use (see M2), or antivirus holds the newly written exe. On Unix it is rarer (for example a sticky-bit directory, or an exe owned by another user), but possible. The user ends up with no daemon, and the only message is a permission error.
- Fix: if `swap_in` fails and `daemon_was_running`, restart the **old** binary (`std::process::Command::new(&exe).arg("restart")`). After a Windows rollback, `exe` is the old file again. On Unix the stop is not needed before the swap at all: swap first, then call `exe restart`, which stops and starts by itself. Only Windows needs the stop before the swap, and even there renaming a running exe is allowed, so the stop-first ordering can go entirely. `restart` already handles stop+start.

### M2. Windows: a `.old` that is still in use blocks every later update, and the error is misleading
`update.rs:218-221`. `remove_file(&parked)` failures are ignored. If the previous binary is still mapped, the delete fails. Old `mdview mcp` processes that editors keep alive for hours or days hold it, and so does a daemon that is not tracked by the lock. The next `rename(exe, parked)` then has to replace a running image, which Windows rejects with ACCESS_DENIED. `permission_hint` turns that into "no write permission … `sudo mdview update`". That advice is wrong on Windows and wrong about the cause. Combined with M1, the daemon is also left stopped.
- Fix: park under a unique name (`<exe>.old-<pid>` or a timestamp). Have `cleanup_parked_binary` delete matching `<file_name>.old*` siblings with best-effort, ignored failures. Give errors from the Windows rename their own message (for example "the previous binary is still in use; close running mdview processes").

### M3. Windows: if rollback fails, the user has no binary and no recovery instructions
`update.rs:222-225`. If `rename(staged, exe)` fails and the rollback `rename(parked, exe)` also fails (ignored with `let _`), then `exe` no longer exists. The only copy is `<exe>.old`, and then `remove_file(&staged)` at :304 also deletes the verified new binary. The error says only "installing the new binary".
- Fix: check the rollback result. If it fails, return an error that names both paths ("restore with: move `<exe>.old` `<exe>`"), and keep the staged file in that case (skip the remove at :304).

### M4. No network or process timeouts: `update` can hang forever
`update.rs:98-101`: the reqwest client has no connect or overall timeout. `update.rs:200-203`: `smoke_test` uses `.output()` with no time limit. A stalled TLS connection or a hanging downloaded binary blocks the command indefinitely. The smoke-test hang happens before the daemon stop, so it is only a hang, but it is the uninterruptible kind a script running `mdview update` cannot recover from.
- Fix: `Client::builder().connect_timeout(10s).timeout(300s)`. For the smoke test, `spawn()` then poll with `try_wait` and kill after about 10s.

## Low

### L1. Smoke-test version check is a substring match
`update.rs:205`: `text.contains(version)`. Target `0.8.4` is accepted when the binary prints `mdview 0.8.40` or `mdview 10.8.4`. Practical impact is small (the digest already ties the bytes to the release), but the check claims more than it proves. Fix: parse the last whitespace token of stdout with `parse_version` and compare it to the `target` tuple. That also handles pre-release suffixes correctly.

### L2. `parse_version` is implemented twice
`update.rs:55` duplicates `runtime.rs:186` (the only difference is the `v` trim). Reuse one function (make the runtime one `pub(crate)` and trim `v` at the call site) so the update path and the daemon-staleness path cannot drift apart.

### L3. A partially written staging file is left behind on failure
`update.rs:177-181`: if `fs::write` fails partway (disk full) or `set_permissions` fails, `stage` returns `Err` before `run` reaches the cleanup at :304. A `.mdview-update-<pid>.tmp` file then stays next to the binary. Fix: remove `staged` inside `stage` on any error after creation.

### L4. `--force` without `--version` silently downgrades
`update.rs:254`: with `force` and target < installed (for example a source-built 0.9.0 when the latest release is 0.8.4), it installs 0.8.4. The help text only says "reinstall even when the target version is already installed". Either refuse a downgrade unless `--version` is given, or update the help and docs to say `--force` also allows downgrades.

### L5. `--check` reports "Update available" where `update` will fail
`update.rs:245-251` returns before `asset_name()`. On Intel macOS (no release asset), `--check` says "run `mdview update`" and that command then fails. Also, `--check --version <older>` prints "latest release: <older>". Fix: check the asset first, and word the message by mode.

### L6. Informational: the Windows smoke test runs a file with a `.tmp` extension
`update.rs:176`. Rust's Windows `Command` tries `<path>.exe` first, then hands the original path to `CreateProcessW`, which runs a PE image regardless of extension, so this should work. I could not test it here. Naming the staged file `.mdview-update-<pid>.exe` removes the uncertainty (and some AV/AppLocker policies treat non-.exe images differently).

### L7. Informational: after an in-place update on Linux, old long-running processes cannot respawn the daemon
`runtime.rs:139` (pre-existing): `current_exe()` in an old `mdview mcp` process now resolves to `/…/mdview (deleted)`, so if the daemon later dies, `spawn_daemon_detached` fails with ENOENT. Before this feature it was rare; `update` makes it routine. The fix belongs in `spawn_daemon_detached`: strip a trailing ` (deleted)` on Linux, which launches the newer binary at that path.

## Verified non-issues
- Token leak: `GITHUB_TOKEN` is attached only to the api.github.com request. The asset download uses a fresh request with no auth, and reqwest strips `Authorization` on cross-host redirects anyway.
- Integrity ordering: size and digest are checked before anything is written to disk, and the smoke test runs before the daemon stop and the swap. A bad download never touches the installed binary.
- Unix swap: staged in the same directory as the canonicalized target, then renamed, which is atomic. Symlinks keep pointing at the replaced file. A running process keeps its old inode.
- Windows renaming a running exe: allowed (deleting is not). The update process renaming its own image is the standard pattern. The `restart` child's `cleanup_parked_binary` silently fails while the parent is alive, and a later run removes `.old`. Fine.
- Asset names and targets match release.yml and install.sh. The `latest` endpoint excludes drafts and pre-releases.
- Trust model: the digest comes from the same GitHub API response as the URL, so it protects against truncation and CDN corruption, not against a compromised repo or release. That matches install.sh's trust level. There is no signature verification, but the docs do not claim any.

## Recommended actions (priority order)
1. H1: detect root via sudo and skip the daemon stop/restart, telling the user to run `mdview restart`.
2. M1: never leave the daemon stopped. Swap first (Unix), and restart the old binary when the swap fails.
3. M2/M3: unique parked names plus a glob cleanup, a Windows-specific error message, a checked rollback with recovery instructions, and keep the staged file when the rollback fails.
4. M4: client timeouts and a bounded smoke test.
5. L1–L5 as convenient.

## Unresolved questions
- Is downgrading across a registry schema generation supported? (`--version` allows downgrades, and the daemon's `needs_restart` never downgrades a newer-schema daemon. This review did not check whether an older binary can work with the registry a newer one left behind.)
- Is the macOS release binary codesigned or notarized? A rename-based swap keeps whatever signature the asset carries, so nothing regresses here, but it was not checked.

Status: DONE_WITH_CONCERNS
Summary: The happy path and download/verify ordering are correct. One High finding: the `sudo mdview update` path the docs recommend can start the daemon as root, or skip its restart without saying so. Three Medium findings: a failed swap leaves the daemon stopped, a `.old` still in use on Windows blocks later updates with a wrong "sudo" hint and an unchecked rollback, and there are no network/process timeouts.

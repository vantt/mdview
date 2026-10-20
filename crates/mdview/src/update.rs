//! `mdview update`: replace the running binary with a GitHub release build.
//!
//! The asset for this platform is picked by the same `mdview-<target>` naming
//! the release workflow and `install.sh` use, verified against the release's
//! byte size and published sha256 digest, smoke-tested (`--version`), and then
//! swapped in place of the current executable. A running daemon is stopped
//! before the swap and restarted from the new binary afterwards.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `owner/repo` derived from the workspace `repository` URL so the update
/// source cannot drift from the published package metadata.
fn repo() -> &'static str {
    env!("CARGO_PKG_REPOSITORY")
        .trim_start_matches("https://github.com/")
        .trim_end_matches('/')
}

/// Release asset name for the platform this binary was built for, or `None`
/// when the release workflow publishes no prebuilt binary for it.
fn asset_name() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("mdview-x86_64-unknown-linux-musl")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("mdview-aarch64-unknown-linux-musl")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("mdview-aarch64-apple-darwin")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("mdview-x86_64-pc-windows-msvc.exe")
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    size: u64,
    /// `sha256:<hex>`; GitHub computes it for every uploaded asset.
    digest: Option<String>,
    browser_download_url: String,
}

/// `major.minor.patch` from a release tag like `v0.8.4` or `0.8.4-rc.1`
/// (pre-release and build suffixes are ignored for ordering).
fn parse_tag(tag: &str) -> Option<(u64, u64, u64)> {
    crate::runtime::parse_version(tag.trim_start_matches('v'))
}

/// Lowercase hex sha256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Check a download against the release metadata: exact byte size, and the
/// published `sha256:` digest when the release carries one.
fn verify_download(bytes: &[u8], asset: &Asset) -> Result<bool> {
    if bytes.len() as u64 != asset.size {
        bail!(
            "downloaded {} bytes but the release lists {} for {}",
            bytes.len(),
            asset.size,
            asset.name
        );
    }
    let Some(digest) = asset.digest.as_deref() else {
        return Ok(false);
    };
    let Some(expected) = digest.strip_prefix("sha256:") else {
        bail!("unsupported digest format for {}: {digest}", asset.name);
    };
    let actual = sha256_hex(bytes);
    if !actual.eq_ignore_ascii_case(expected) {
        bail!(
            "sha256 mismatch for {}: expected {expected}, got {actual}",
            asset.name
        );
    }
    Ok(true)
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("mdview/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()?)
}

/// Fetch release metadata: the latest release, or the one tagged `tag`.
async fn fetch_release(client: &reqwest::Client, tag: Option<&str>) -> Result<Release> {
    let url = match tag {
        None => format!("https://api.github.com/repos/{}/releases/latest", repo()),
        Some(t) => {
            let t = if t.starts_with('v') {
                t.to_string()
            } else {
                format!("v{t}")
            };
            format!("https://api.github.com/repos/{}/releases/tags/{t}", repo())
        }
    };
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    // Unauthenticated API calls are rate-limited per IP (60/h); a token lifts it.
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            req = req.bearer_auth(token);
        }
    }
    let resp = req.send().await.with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        bail!("release not found: {}", tag.unwrap_or("latest"));
    }
    if !status.is_success() {
        bail!("GET {url} failed: HTTP {status}");
    }
    resp.json().await.context("parsing GitHub release JSON")
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url}"))?;
    Ok(resp.bytes().await?.to_vec())
}

/// The real file behind the running executable (symlinks resolved, so a
/// `/usr/local/bin/mdview -> …` link keeps pointing at the replaced file).
pub(crate) fn current_exe_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("locating the running mdview binary")?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

/// Path the replaced Windows binary is parked at: a running `.exe` can be
/// renamed but not deleted, so it is removed on a later run instead. The pid
/// suffix keeps a parked file still held open by an old `mdview mcp` process
/// from blocking the next update.
#[cfg(windows)]
fn parked_path(exe: &Path) -> PathBuf {
    let mut p = exe.as_os_str().to_owned();
    p.push(format!(".old-{}", std::process::id()));
    PathBuf::from(p)
}

/// Remove binaries parked by previous Windows updates (no-op elsewhere).
/// Files still in use are skipped and retried on a later run.
pub fn cleanup_parked_binary() {
    #[cfg(windows)]
    if let Ok(exe) = current_exe_path() {
        let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) else {
            return;
        };
        let prefix = format!("{}.old", name.to_string_lossy());
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with(&prefix) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

/// Write `bytes` next to `exe` as an executable staging file. Staying in the
/// same directory keeps the final rename on one filesystem (atomic on Unix).
fn stage(exe: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let dir = exe.parent().context("binary has no parent directory")?;
    let staged = dir.join(format!(
        ".mdview-update-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    let written = std::fs::write(&staged, bytes).map_err(|e| permission_hint(e, dir));
    #[cfg(unix)]
    let written = written.and_then(|()| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .map_err(anyhow::Error::from)
    });
    if let Err(e) = written {
        let _ = std::fs::remove_file(&staged);
        return Err(e);
    }
    Ok(staged)
}

fn permission_hint(e: std::io::Error, dir: &Path) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        anyhow::anyhow!(
            "no write permission for {} ({e}). Re-run with elevated rights (e.g. `sudo mdview update`) or reinstall into a user-writable directory.",
            dir.display()
        )
    } else {
        anyhow::Error::new(e).context(format!("writing into {}", dir.display()))
    }
}

/// How long the downloaded binary gets to answer `--version`.
const SMOKE_TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Run the staged binary's `--version` and require it to report exactly
/// `version`, so a truncated or wrong-platform download never replaces a
/// working binary.
fn smoke_test(staged: &Path, version: (u64, u64, u64)) -> Result<()> {
    use std::io::Read;
    let mut child = std::process::Command::new(staged)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("running downloaded binary {}", staged.display()))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > SMOKE_TEST_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            bail!("downloaded binary did not answer `--version` within {SMOKE_TEST_TIMEOUT:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut text = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut text);
    }
    let reported = text.split_whitespace().last().and_then(parse_tag);
    if !status.success() || reported != Some(version) {
        bail!(
            "downloaded binary did not report version {} (got: {})",
            fmt_version(version),
            text.trim()
        );
    }
    Ok(())
}

/// Move `staged` over `exe`. A running binary may be replaced on every
/// platform: Unix renames over it atomically; Windows renames the running
/// `.exe` aside first, rolling back if the new file cannot take its place.
fn swap_in(staged: &Path, exe: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let parked = parked_path(exe);
        std::fs::rename(exe, &parked).with_context(|| {
            format!(
                "moving the current binary aside ({} -> {}); close other running mdview processes and retry",
                exe.display(),
                parked.display()
            )
        })?;
        if let Err(e) = std::fs::rename(staged, exe) {
            if let Err(rollback) = std::fs::rename(&parked, exe) {
                bail!(
                    "installing the new binary failed ({e}) and restoring the old one failed ({rollback}). \
                     Recover manually: rename {} or {} to {}",
                    staged.display(),
                    parked.display(),
                    exe.display()
                );
            }
            return Err(e).context("installing the new binary (old binary restored)");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(staged, exe).map_err(|e| permission_hint(e, exe.parent().unwrap_or(exe)))
    }
}

/// True when running as root through `sudo`: the daemon belongs to the
/// invoking user, so this process must not stop it or start a root-owned one.
pub(crate) fn running_under_sudo() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions and cannot fail.
        let is_root = unsafe { libc::geteuid() } == 0;
        is_root && std::env::var_os("SUDO_UID").is_some()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Whether to install `target` over `installed`. Only an explicit `--version`
/// pin may downgrade; `--force` alone just reinstalls the same version.
fn should_install(
    pinned: bool,
    force: bool,
    target: (u64, u64, u64),
    installed: (u64, u64, u64),
) -> bool {
    if pinned {
        target != installed || force
    } else {
        target > installed || (force && target == installed)
    }
}

fn fmt_version(v: (u64, u64, u64)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

pub fn run(check: bool, version: Option<String>, force: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let installed = parse_tag(current).context("unrecognised own version")?;
    let rt = tokio::runtime::Runtime::new()?;
    let client = client()?;
    let release = rt.block_on(fetch_release(&client, version.as_deref()))?;
    let target = parse_tag(&release.tag_name)
        .with_context(|| format!("unrecognised release tag {}", release.tag_name))?;
    let target_str = fmt_version(target);
    let label = if version.is_some() {
        "release"
    } else {
        "latest release"
    };
    let name = asset_name();

    if check {
        if target > installed {
            match name {
                Some(_) => println!("Update available: {current} -> {target_str} (run `mdview update`)."),
                None => println!(
                    "Update available: {current} -> {target_str}. No prebuilt binary exists for this platform; \
                     update from source: cargo install --git https://github.com/vantt/mdview mdview --force"
                ),
            }
        } else {
            println!("mdview {current} is up to date ({label}: {target_str}).");
        }
        return Ok(());
    }
    // Only an explicit `--version` pin may downgrade; `--force` alone just
    // reinstalls the same version.
    if !should_install(version.is_some(), force, target, installed) {
        if target == installed && version.is_some() {
            println!("mdview {current} is already installed (use --force to reinstall).");
        } else {
            println!("mdview {current} is up to date ({label}: {target_str}).");
        }
        return Ok(());
    }

    let name = name.context(
        "no prebuilt mdview binary is published for this platform; update from source instead: \
         cargo install --git https://github.com/vantt/mdview mdview --force",
    )?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .with_context(|| format!("release {} has no asset {name}", release.tag_name))?;

    let exe = current_exe_path()?;
    println!("Downloading {name} ({})…", release.tag_name);
    let bytes = rt.block_on(download(&client, &asset.browser_download_url))?;
    if verify_download(&bytes, asset)? {
        println!("Verified sha256.");
    } else {
        println!("Note: the release publishes no sha256 digest; verified size only.");
    }

    let staged = stage(&exe, &bytes)?;
    let swapped = smoke_test(&staged, target).and_then(|()| swap_in(&staged, &exe));
    if swapped.is_ok() || staged.exists() && exe.exists() {
        let _ = std::fs::remove_file(&staged);
    }
    swapped?;
    println!("Updated {}: {current} -> {target_str}", exe.display());

    if running_under_sudo() {
        // HOME is root's under sudo: the skill and daemon to refresh are the
        // invoking user's, so leave both to a run as that user.
        println!(
            "Running under sudo; run `mdview restart` and `mdview doctor --fix` as your user."
        );
        return Ok(());
    }
    // Run the new file by path: after the swap, this process's own
    // `current_exe()` names the replaced binary (on Linux a deleted inode),
    // whose compiled-in templates are the old ones.
    let refreshed = std::process::Command::new(&exe)
        .args(["doctor", "--after-update"])
        .status();
    if !refreshed.is_ok_and(|s| s.success()) {
        println!("Note: refreshing the installed skill failed; run `mdview doctor --fix`.");
    }

    if crate::runtime::running_daemon().is_none() {
        return Ok(());
    }
    let status = std::process::Command::new(&exe)
        .arg("restart")
        .status()
        .context("restarting the daemon with the new binary")?;
    if !status.success() {
        bail!("the new binary failed to restart the daemon; run `mdview restart`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(size: u64, digest: Option<&str>) -> Asset {
        Asset {
            name: "mdview-test".into(),
            size,
            digest: digest.map(str::to_string),
            browser_download_url: String::new(),
        }
    }

    #[test]
    fn parses_tags_and_versions() {
        assert_eq!(parse_tag("v0.8.4"), Some((0, 8, 4)));
        assert_eq!(parse_tag("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_tag("v1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(parse_tag("v1.2"), None);
        assert_eq!(parse_tag("v1.2.3.4"), None);
        assert_eq!(parse_tag("latest"), None);
    }

    #[test]
    fn orders_versions_numerically() {
        assert!(parse_tag("v0.10.0") > parse_tag("v0.9.9"));
        assert!(parse_tag("v1.0.0") > parse_tag("v0.99.99"));
    }

    #[test]
    fn install_decision_never_downgrades_without_a_pin() {
        let (old, cur, new) = ((0, 8, 3), (0, 8, 4), (0, 8, 5));
        assert!(should_install(false, false, new, cur));
        assert!(!should_install(false, false, cur, cur));
        assert!(should_install(false, true, cur, cur));
        assert!(!should_install(false, false, old, cur));
        assert!(!should_install(false, true, old, cur));
        assert!(should_install(true, false, old, cur));
        assert!(!should_install(true, false, cur, cur));
        assert!(should_install(true, true, cur, cur));
    }

    #[test]
    fn repo_is_owner_slash_name() {
        assert_eq!(repo(), "vantt/mdview");
    }

    #[test]
    fn this_platform_has_an_asset_name_matching_the_release_workflow() {
        if let Some(name) = asset_name() {
            assert!(name.starts_with("mdview-"));
        }
    }

    #[test]
    fn verify_accepts_matching_size_and_digest() {
        let bytes = b"hello";
        let digest = format!("sha256:{}", sha256_hex(bytes));
        assert!(verify_download(bytes, &asset(5, Some(&digest))).unwrap());
    }

    #[test]
    fn verify_without_digest_checks_size_only() {
        assert!(!verify_download(b"hello", &asset(5, None)).unwrap());
    }

    #[test]
    fn verify_rejects_wrong_size() {
        assert!(verify_download(b"hello", &asset(6, None)).is_err());
    }

    #[test]
    fn verify_rejects_digest_mismatch() {
        let digest = format!("sha256:{}", sha256_hex(b"other"));
        assert!(verify_download(b"hello", &asset(5, Some(&digest))).is_err());
    }

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

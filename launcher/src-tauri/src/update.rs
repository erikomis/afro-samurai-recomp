//! Updates: the port's sources (the checkout the builds come from) and the
//! launcher itself.
//!
//! - **Sources.** When the port folder is a git checkout with an upstream,
//!   `git fetch` and the count of commits it is behind; "Get the new sources"
//!   is `git pull --ff-only` (never a merge, never a reset: a checkout with
//!   changes of its own is left to the player). The builds are "older than
//!   the sources" when the newest commit touching what goes into them
//!   (port/, patches/, tools/) is newer than the EBOOT build.
//! - **New launcher version.** The update manifest (`latest.json`) the
//!   release workflow publishes (scripts/updater_json.py), compared with this
//!   app's version. Fetched with the system's `curl` from the first of
//!   `ENDPOINTS` that has one (the same list as `plugins.updater.endpoints`
//!   in tauri.conf.json, a test checks): the fixed `updater` release, which
//!   `.github/workflows/updater.yml` points at the newest published version
//!   (pre-releases included, drafts never), then GitHub's "latest release"
//!   for when releases stop being pre-releases.
//! - **Download and install** (macOS on Apple silicon, a packaged .app):
//!   tauri-plugin-updater checks the same endpoints, downloads the
//!   `.app.tar.gz` the manifest names, verifies it against the public key in
//!   tauri.conf.json (a minisign key whose private half only the release
//!   workflow has; the signature must also carry the announced version,
//!   `requireSignedVersion`) and swaps the bundle in place. "Restart to
//!   finish" relaunches into the new version. Not while the game or a
//!   rebuild runs, and not from the disk image or a translocated copy (macOS
//!   runs a downloaded app from a read-only place until it is moved).
//! - **Rebuild after an update.** The install leaves `app-updated.json`
//!   (from, to, when) in the launcher's folder; while this app is that
//!   version and the EBOOT build is older than the update, Updates and Home
//!   offer "Rebuild the game" (after "Get the new sources" when the checkout
//!   is behind), since a new launcher can build the game differently.
//!
//! `AFRO_NO_UPDATE_CHECK=1` turns the launcher check and the install off; the
//! setting "Check for updates when the app opens" only skips the check at
//! start.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = crate::no_console(&mut Command::new("git"))
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sources {
    /// The port folder is a git checkout.
    pub git: bool,
    pub branch: String,
    pub commit: String,
    pub subject: String,
    /// Seconds since the epoch of the commit checked out.
    pub commit_time: u64,
    /// The upstream it follows, and how far behind/ahead it is (after a fetch).
    pub upstream: String,
    pub behind: u32,
    pub ahead: u32,
    /// Files changed in the working tree (git status --porcelain, tracked only).
    pub dirty: u32,
    /// The newest commit touching what goes into the builds.
    pub sources_time: u64,
    pub fetched: bool,
    pub error: Option<String>,
}

/// The checkout's state; `fetch` asks the remote first (network).
pub fn sources(root: &Path, fetch: bool) -> Sources {
    let mut s = Sources::default();
    if git(root, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        return s;
    }
    s.git = true;
    s.branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    s.commit = git(root, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    s.subject = git(root, &["log", "-1", "--format=%s"]).unwrap_or_default();
    s.commit_time = git(root, &["log", "-1", "--format=%ct"]).and_then(|v| v.parse().ok()).unwrap_or(0);
    s.sources_time = git(root, &["log", "-1", "--format=%ct", "--", "port", "patches", "tools"])
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    s.dirty = git(root, &["status", "--porcelain", "--untracked-files=no"]).map(|v| v.lines().count() as u32).unwrap_or(0);
    s.upstream = git(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).unwrap_or_default();
    if s.upstream.is_empty() {
        return s;
    }
    if fetch {
        let remote = s.upstream.split('/').next().unwrap_or("origin").to_string();
        match crate::no_console(&mut Command::new("git"))
            .arg("-C")
            .arg(root)
            .args(["fetch", "--quiet", &remote])
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=10")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(o) if o.status.success() => s.fetched = true,
            Ok(o) => s.error = Some(String::from_utf8_lossy(&o.stderr).trim().lines().last().unwrap_or("git fetch failed").to_string()),
            Err(e) => s.error = Some(format!("git: {e}")),
        }
    }
    if let Some(v) = git(root, &["rev-list", "--left-right", "--count", "HEAD...@{u}"]) {
        let mut it = v.split_whitespace().map(|n| n.parse::<u32>().unwrap_or(0));
        s.ahead = it.next().unwrap_or(0);
        s.behind = it.next().unwrap_or(0);
    }
    s
}


// ---------------------------------------------------------------------------
// The launcher: latest.json, the install, the restart
// ---------------------------------------------------------------------------

/// Where the update manifest is looked for, in order (tauri.conf.json's
/// `plugins.updater.endpoints` must be the same; a test checks).
pub const ENDPOINTS: [&str; 2] = [
    "https://github.com/erikomis/afro-samurai-recomp/releases/download/updater/latest.json",
    "https://github.com/erikomis/afro-samurai-recomp/releases/latest/download/latest.json",
];
/// The release pages (open_url allows the repository).
const RELEASES_PAGE: &str = "https://github.com/erikomis/afro-samurai-recomp/releases";
/// The manifest's entry for this build (the only one the releases carry: the
/// Windows installers stay a download from the release page).
const PLATFORM: &str = "darwin-aarch64";
/// What install_update leaves for the next start (in the launcher's folder).
const MARKER: &str = "app-updated.json";

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub current: String,
    /// The online check ran (it is skipped when not asked or turned off).
    pub checked: bool,
    pub latest: Option<String>,
    pub newer: bool,
    /// The release page of `latest`.
    pub url: Option<String>,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    /// AFRO_NO_UPDATE_CHECK is set: no check, no install.
    pub disabled: bool,
    pub error: Option<String>,
    /// Why "Download and install" cannot work here (None: it can):
    /// "platform" (not macOS on Apple silicon, or no download for it),
    /// "dev" (not a packaged .app), "dmg" (running from the disk image),
    /// "translocated" (macOS runs it from a read-only copy until it is moved).
    pub install_blocker: Option<String>,
    /// A version installed in this session, waiting for the restart.
    pub installed_pending: Option<String>,
    /// This app was updated from that version, after the builds were made.
    pub updated_from: Option<String>,
}

/// "1.2.3" (a leading v allowed) -> (1, 2, 3).
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u32>().ok());
    let r = (it.next()??, it.next().unwrap_or(Some(0))?, it.next().unwrap_or(Some(0))?);
    it.next().is_none().then_some(r)
}

/// Whether `latest` is newer than `current`, as the updater decides it
/// (semver: a pre-release such as `1.0.0-rc1` comes before `1.0.0`).
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(a), Some(b)) if a != b => a > b,
        (Some(_), Some(_)) => {
            let pre = |v: &str| v.split('+').next().unwrap_or("").contains('-');
            pre(current) && !pre(latest)
        }
        _ => false,
    }
}

/// What the app shows of an update manifest.
#[derive(Debug, PartialEq)]
struct Manifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    /// It has a download for this build (darwin-aarch64), with a signature.
    has_platform: bool,
}

/// An update manifest (`latest.json`, scripts/updater_json.py) in the shape
/// tauri-plugin-updater reads: version, notes, pub_date and
/// platforms.<os>-<arch>.{url, signature}.
fn parse_manifest(json: &str) -> Option<Manifest> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let version = v.get("version")?.as_str()?.trim().trim_start_matches('v').to_string();
    parse_version(&version)?;
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let has_platform = v.get("platforms").and_then(|p| p.get(PLATFORM)).is_some_and(|p| {
        p.get("url").and_then(|u| u.as_str()).is_some_and(|u| u.starts_with("https://"))
            && p.get("signature").and_then(|s| s.as_str()).is_some_and(|s| !s.trim().is_empty())
    });
    Some(Manifest { version, notes: text("notes").map(|n| n.chars().take(4000).collect()), pub_date: text("pub_date"), has_platform })
}

/// The release page of a version.
fn release_page(version: &str) -> String {
    format!("{RELEASES_PAGE}/tag/v{}", version.trim_start_matches('v'))
}

/// The first endpoint's manifest. Ok(None): none published yet (404 everywhere).
fn fetch_latest() -> Result<Option<Manifest>, String> {
    let curl = if cfg!(windows) { "curl.exe" } else { "/usr/bin/curl" };
    let mut last_err = None;
    for url in ENDPOINTS {
        let out = crate::no_console(&mut Command::new(curl))
            .args(["-sSL", "--max-time", "10", "-H", "Accept: application/json", "-A", "afro-samurai-launcher", "-w", "\n%{http_code}", url])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("curl: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_ref()));
        match code.trim() {
            "200" => match parse_manifest(body) {
                Some(m) => return Ok(Some(m)),
                None => last_err = Some("unexpected answer from GitHub".to_string()),
            },
            "404" => {}
            "000" | "" => last_err = Some(String::from_utf8_lossy(&out.stderr).trim().to_string()),
            c => last_err = Some(format!("GitHub answered HTTP {c}")),
        }
    }
    match last_err {
        Some(e) => Err(e),
        None => Ok(None),
    }
}

/// `AFRO_NO_UPDATE_CHECK` set to anything but empty or 0.
pub fn disabled() -> bool {
    std::env::var_os("AFRO_NO_UPDATE_CHECK").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Whether this build installs signed updates in place. Off until the
/// project has an updater signing key: then `plugins.updater` (the public key,
/// ENDPOINTS, requireSignedVersion) goes into tauri.conf.json and this turns
/// on (README, "App updates"). Off, a newer release is still found and shown,
/// with a link to its page instead of Download and install, and lib.rs does
/// not register the updater plugin (it refuses to start without a key).
pub const SIGNED_UPDATES: bool = false;

/// Why the bundle at `bundle` cannot be replaced in place (see AppUpdate).
fn blocker_for(bundle: Option<&Path>) -> Option<&'static str> {
    if !SIGNED_UPDATES || !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Some("platform");
    }
    let Some(b) = bundle else { return Some("dev") };
    if b.to_string_lossy().contains("/AppTranslocation/") {
        Some("translocated")
    } else if b.starts_with("/Volumes") && !writable_dir(b.parent()) {
        Some("dmg")
    } else {
        None
    }
}

/// Whether a file can be made in `dir` (not a mounted disk image).
fn writable_dir(dir: Option<&Path>) -> bool {
    let Some(dir) = dir else { return false };
    let probe = dir.join(format!(".afro-samurai-write-test-{}", std::process::id()));
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn blocker() -> Option<&'static str> {
    blocker_for(crate::app_icon::app_bundle().as_deref())
}

/// The record of an install (`app-updated.json`).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Updated {
    from: String,
    to: String,
    /// Seconds since the epoch.
    at: u64,
}

fn marker_path() -> PathBuf {
    crate::data_root().join(MARKER)
}

/// The version this app was updated from, when the builds are older than
/// that update (`built_at` 0: no builds, nothing to rebuild).
fn updated_from(marker: Option<&str>, current: &str, built_at: u64) -> Option<String> {
    let u: Updated = serde_json::from_str(marker?).ok()?;
    (u.to.trim_start_matches('v') == current.trim_start_matches('v') && built_at > 0 && built_at < u.at).then_some(u.from)
}

/// The launcher's side of check_updates (`online`: ask the manifest).
pub fn check_app(app: &AppHandle, online: bool, built_at: u64) -> AppUpdate {
    let current = app.package_info().version.to_string();
    let marker = fs::read_to_string(marker_path()).ok();
    let mut u = AppUpdate {
        updated_from: updated_from(marker.as_deref(), &current, built_at),
        installed_pending: app.state::<crate::AppState>().update_installed.lock().unwrap().clone(),
        install_blocker: blocker().map(str::to_string),
        disabled: disabled(),
        current,
        ..Default::default()
    };
    if !online || u.disabled {
        return u;
    }
    u.checked = true;
    match fetch_latest() {
        Ok(Some(m)) => {
            u.newer = is_newer(&m.version, &u.current);
            if !m.has_platform && u.install_blocker.is_none() {
                u.install_blocker = Some("platform".into());
            }
            u.url = Some(release_page(&m.version));
            u.latest = Some(m.version);
            u.notes = m.notes;
            u.pub_date = m.pub_date;
        }
        Ok(None) => u.error = Some("no release with an update manifest (latest.json) yet".into()),
        Err(e) => u.error = Some(e),
    }
    u
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
    /// "download", then "install" once the file is in and verified.
    phase: &'static str,
}

/// Clears AppState::updating however install_update ends.
struct Updating(AppHandle);

impl Drop for Updating {
    fn drop(&mut self) {
        *self.0.state::<crate::AppState>().updating.lock().unwrap() = false;
    }
}

/// "Download and install": the newest version from the manifest, verified
/// and swapped in (progress as `update-progress` events). Returns the
/// version now on disk; the app runs the old one until restart_app.
/// Errors that the window explains start with "blocked:<reason>".
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_updater::UpdaterExt;
    if disabled() {
        return Err("updates are turned off (AFRO_NO_UPDATE_CHECK)".into());
    }
    if let Some(b) = blocker() {
        return Err(format!("blocked:{b}"));
    }
    let guard = {
        let st = app.state::<crate::AppState>();
        if let Some(v) = st.update_installed.lock().unwrap().clone() {
            return Err(format!("version {v} is already installed: restart the app to finish"));
        }
        let mut updating = st.updating.lock().unwrap();
        if *updating {
            return Err("the update is already being installed".into());
        }
        if crate::game_busy(&st) {
            return Err("blocked:game".into());
        }
        if st.rebuild.lock().unwrap().is_some() || st.setup.lock().unwrap().is_some() {
            return Err("blocked:rebuild".into());
        }
        *updating = true;
        Updating(app.clone())
    };
    let from = app.package_info().version.to_string();
    let updater = app.updater_builder().timeout(Duration::from_secs(300)).build().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "there is no newer version".to_string())?;
    let version = update.version.clone();
    let (a, b) = (app.clone(), app.clone());
    let mut got = 0u64;
    let mut last: Option<Instant> = None;
    update
        .download_and_install(
            move |chunk, total| {
                got += chunk as u64;
                if last.is_none_or(|t| t.elapsed() >= Duration::from_millis(100)) || total == Some(got) {
                    last = Some(Instant::now());
                    let _ = a.emit("update-progress", Progress { downloaded: got, total, phase: "download" });
                }
            },
            move || {
                let _ = b.emit("update-progress", Progress { downloaded: 0, total: None, phase: "install" });
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    let rec = Updated { from, to: version.clone(), at: crate::now_secs() };
    let _ = fs::create_dir_all(crate::data_root());
    if let Ok(text) = serde_json::to_string(&rec) {
        let _ = fs::write(marker_path(), text);
    }
    *app.state::<crate::AppState>().update_installed.lock().unwrap() = Some(version.clone());
    drop(guard);
    Ok(version)
}

/// "Restart to finish": relaunch into the installed version.
#[tauri::command]
pub fn restart_app(app: AppHandle) -> Result<(), String> {
    let st = app.state::<crate::AppState>();
    if st.game.lock().unwrap().is_some() {
        return Err("blocked:game".into());
    }
    if st.rebuild.lock().unwrap().is_some() || st.setup.lock().unwrap().is_some() {
        return Err("blocked:rebuild".into());
    }
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v0.2.10"), Some((0, 2, 10)));
        assert_eq!(parse_version("1.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("0.3.0-beta.1"), Some((0, 3, 0)));
        assert_eq!(parse_version("x"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert!(parse_version("v0.2.0") > parse_version("0.1.9"));
        assert!(is_newer("v0.3.0", "0.2.2"));
        assert!(is_newer("0.2.10", "0.2.9"));
        assert!(!is_newer("v0.2.2", "0.2.2"));
        assert!(!is_newer("0.2.1", "0.2.2"));
        assert!(!is_newer("nightly", "0.2.2"));
        // as semver (and so the updater) orders them
        assert!(is_newer("0.3.0", "0.3.0-rc1"));
        assert!(!is_newer("0.3.0-rc1", "0.3.0"));
        assert!(!is_newer("0.3.0+build5", "0.3.0"));
    }

    #[test]
    fn not_a_checkout() {
        let d = std::env::temp_dir().join(format!("rl-nogit-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let s = sources(&d, false);
        assert!(!s.git);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// What scripts/updater_json.py writes (test_updater_json.py checks the
    /// script's side of it).
    const MANIFEST: &str = r#"{
      "version": "0.3.0",
      "notes": "Launcher v0.3.0.",
      "pub_date": "2026-11-06T12:00:00Z",
      "platforms": {
        "darwin-aarch64": {
          "url": "https://github.com/erikomis/afro-samurai-recomp/releases/download/v0.3.0/afro-samurai-launcher_0.3.0_macos_aarch64.app.tar.gz",
          "signature": "dW50cnVzdGVk"
        }
      }
    }"#;

    #[test]
    fn manifest() {
        let m = parse_manifest(MANIFEST).unwrap();
        assert_eq!(
            m,
            Manifest {
                version: "0.3.0".into(),
                notes: Some("Launcher v0.3.0.".into()),
                pub_date: Some("2026-11-06T12:00:00Z".into()),
                has_platform: true,
            }
        );
        assert_eq!(release_page(&m.version), "https://github.com/erikomis/afro-samurai-recomp/releases/tag/v0.3.0");
        assert!(release_page(&m.version).starts_with(crate::URL_ALLOWED[2]));
        // a "v" is dropped; an entry without a signature does not count
        let m = parse_manifest(r#"{"version":"v0.4.0","platforms":{"darwin-aarch64":{"url":"https://x/y","signature":""}}}"#).unwrap();
        assert_eq!((m.version.as_str(), m.has_platform, m.notes), ("0.4.0", false, None));
        let other = r#"{"version":"0.4.0","platforms":{"windows-x86_64":{"url":"https://x","signature":"s"}}}"#;
        assert!(!parse_manifest(other).unwrap().has_platform);
        assert_eq!(parse_manifest(r#"{"version":"nightly"}"#), None);
        assert_eq!(parse_manifest(r#"{"message":"Not Found"}"#), None);
        assert_eq!(parse_manifest("Not Found"), None);
    }

    #[test]
    fn rebuild_offered_after_an_update() {
        let rec = |to: &str, at: u64| serde_json::to_string(&Updated { from: "0.2.2".into(), to: to.into(), at }).unwrap();
        // updated to this version after the builds were made
        assert_eq!(updated_from(Some(&rec("0.3.0", 2000)), "0.3.0", 1000).as_deref(), Some("0.2.2"));
        // rebuilt since
        assert_eq!(updated_from(Some(&rec("0.3.0", 2000)), "0.3.0", 2500), None);
        // no builds at all: nothing to rebuild (Install the game does it)
        assert_eq!(updated_from(Some(&rec("0.3.0", 2000)), "0.3.0", 0), None);
        // an older record (this app is another version now)
        assert_eq!(updated_from(Some(&rec("0.3.0", 2000)), "0.3.1", 1000), None);
        assert_eq!(updated_from(None, "0.3.0", 1000), None);
        assert_eq!(updated_from(Some("not json"), "0.3.0", 1000), None);
    }

    fn decode_b64(s: &str) -> Vec<u8> {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let (mut acc, mut bits) = (0u32, 0);
        for c in s.trim().bytes().filter(|&c| c != b'=') {
            acc = (acc << 6 | T.iter().position(|&t| t == c).expect("base64") as u32) & 0xffff;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }

    /// The updater's settings in tauri.conf.json: the same endpoints as
    /// here, a minisign public key, the signed version required, and the
    /// updater artifacts only in the CI-only config (a local `tauri build`
    /// without the private key keeps working).
    #[test]
    fn updater_config() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        if !SIGNED_UPDATES {
            // no key yet: no updater section (the plugin is not registered)
            assert!(conf["plugins"].get("updater").is_none());
            assert!(conf["bundle"].get("createUpdaterArtifacts").is_none());
            return;
        }
        let u = &conf["plugins"]["updater"];
        let eps: Vec<&str> = u["endpoints"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
        assert_eq!(eps, ENDPOINTS);
        assert_eq!(u["requireSignedVersion"], serde_json::Value::Bool(true));
        let text = String::from_utf8(decode_b64(u["pubkey"].as_str().unwrap())).unwrap();
        assert!(text.starts_with("untrusted comment: minisign public key"), "{text}");
        let raw = decode_b64(text.lines().nth(1).unwrap());
        assert_eq!(raw.len(), 42, "\"Ed\", the key id and the 32-byte key");
        assert_eq!(&raw[..2], b"Ed");
        assert!(conf["bundle"].get("createUpdaterArtifacts").is_none());
        let ci: serde_json::Value = serde_json::from_str(include_str!("../tauri.updater.conf.json")).unwrap();
        assert_eq!(ci["bundle"]["createUpdaterArtifacts"], serde_json::Value::Bool(true));
    }

    #[test]
    fn install_blockers() {
        if SIGNED_UPDATES && cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(blocker_for(None), Some("dev"));
            let t = Path::new("/private/var/folders/x/T/AppTranslocation/ABC/d/Afro Samurai Launcher.app");
            assert_eq!(blocker_for(Some(t)), Some("translocated"));
            assert_eq!(blocker_for(Some(Path::new("/Volumes/afro-samurai-launcher-none/Afro Samurai Launcher.app"))), Some("dmg"));
            assert_eq!(blocker_for(Some(&std::env::temp_dir().join("Afro Samurai Launcher.app"))), None);
        } else {
            assert_eq!(blocker_for(None), Some("platform"));
        }
    }
}

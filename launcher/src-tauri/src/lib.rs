//! Launcher core for the Afro Samurai (BLUS30264) port: the settings, the
//! player's disc (a mounted image or a folder), the build of a port
//! checkout, the rebuild (tools/rebuild.sh) as a child process whose output
//! is streamed to the window, and "Play", which starts
//! `build/BLUS30264/AfroSamuraiRecomp game/EBOOT.elf` with the environment
//! tools/run.sh gives it and the settings mapped onto the port's variables.
//! No game code or data is part of the app: everything it shows or starts
//! comes from the player's own disc and build folder.
//!
//! The port builds and runs on Apple-silicon macOS only. The Windows paths
//! (PowerShell's Mount-DiskImage, the drive letters, %APPDATA%) come from the
//! launcher this one is modelled on and are kept so the code stays shared;
//! the port itself has no Windows build.

mod app_icon;
mod input_map;
mod play_shortcut;
mod report;
mod saves;
mod steam;
mod steam_art;
mod update;
mod vdf;
mod zipfile;

use input_map::{Bindings, InputEnv};
use saves::{read_sfo, BackupMeta, Side, Slot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Emitter, Manager, State};

const TITLE_ID: &str = "BLUS30264";
/// The game's executable (port/CMakeLists.txt) in build/BLUS30264/.
const GAME_EXE: &str = if cfg!(windows) { "AfroSamuraiRecomp.exe" } else { "AfroSamuraiRecomp" };
/// The window title the port takes from PS3_TITLE (tools/run.sh sets the same).
const WINDOW_TITLE: &str = "Afro Samurai";

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// What the player can change. Each field maps onto one of the environment
/// variables the port reads (see `launch_env`) or onto how Play starts it.
/// Every one is passed only when it differs from the port's default.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Launcher UI language: "pt-BR" or "en".
    pub ui_lang: String,
    /// The port checkout whose build Play runs: it holds
    /// build/BLUS30264/AfroSamuraiRecomp and game/EBOOT.elf (this repository
    /// after tools/rebuild.sh). Empty = found on its own.
    pub port_dir: String,
    /// The disc: the folder holding PS3_GAME/ (a mounted image or an
    /// extracted copy). Empty = the first mounted BLUS30264 under /Volumes.
    pub disc_path: String,
    /// The disc image (.iso) to attach when the disc is not mounted.
    pub disc_image: String,
    /// Attach `disc_image` on Play when no disc is mounted.
    pub auto_mount: bool,
    /// Where saves, the game's HDD data and last.log live (PS3_HDD0_ROOT,
    /// PS3_HDD1_ROOT, PS3_GAMEDATA_ROOT, the cwd). Empty =
    /// ~/Library/Application Support/afro-samurai-recomp. tools/run.sh uses
    /// the repository's rpcs3/ folder: pick it here to share its saves.
    pub play_dir: String,
    /// PS3_LANG: the console language; empty = the Mac's.
    pub game_lang: String,
    /// PS3RECOMP_METAL_WINDOW: the window's size "<w>x<h>"; empty = 1280x720.
    pub window_size: String,
    /// PS3_WINDOW_MODE: "windowed" (the default), "fullscreen" (macOS's own)
    /// or "borderless". (Before it, a `fullscreen` switch: see
    /// `parse_settings`.)
    pub window_mode: String,
    /// PS3_DISPLAY: the screen's name (or its 1-based number when two share
    /// a name); empty = the main screen.
    pub display: String,
    /// PS3_VSYNC=0 when off.
    pub vsync: bool,
    /// PS3_UPSCALE: "" (a plain scale) or "metalfx" (macOS 13+).
    pub upscale: String,
    /// PS3_RES_SCALE: the internal resolution as a multiple of 1280x720,
    /// "" (native), "1.5" (1080p), "2" (1440p) or "3" (4K). Experimental.
    pub res_scale: String,
    /// The player's own ICON0.PNG in the Dock instead of the placeholder.
    pub game_icon: bool,
    /// PS3_AA: "" (off), "fxaa" or "smaa".
    pub aa: String,
    /// PS3_SHARPEN: 0 (off) to 1.
    pub sharpen: f32,
    /// PS3_ANISO: 0 (off), 2, 4, 8 or 16.
    pub aniso: u32,
    /// PS3_DISPLAY_GAMMA when set; otherwise the game's own brightness.
    pub gamma_override: bool,
    pub gamma: f32,
    /// PS3_VIBRANCE: -1 (muted) .. 0 (off) .. 1 (vivid).
    pub vibrance: f32,
    /// PS3_CONTRAST: 0.5 .. 1 (off) .. 1.5.
    pub contrast: f32,
    /// PS3_VIGNETTE and PS3_GRAIN: 0 (off) .. 1.
    pub vignette: f32,
    pub grain: f32,
    /// PS3_LUT: the player's colour LUT (ReShade's PNG layout), and how much
    /// of it (PS3_LUT_STRENGTH, 0..1).
    pub lut_path: String,
    pub lut_strength: f32,
    /// PS3_INPUT_MODE: "both" (the default), "controller" or "keyboard".
    /// (Before it, a `keyboard` switch: see `parse_settings`.)
    pub input_mode: String,
    /// PS3_MOUSE, PS3_MOUSE_SENS, PS3_MOUSE_INVERT_Y, PS3_MOUSE_INVERT_X,
    /// PS3_MOUSE_DEADZONE. The mouse camera is off in this port's layout
    /// (the runtime's own defaults: on, 1.0, off, off, 30).
    pub mouse: bool,
    pub mouse_sens: f32,
    pub mouse_invert_y: bool,
    pub mouse_invert_x: bool,
    pub mouse_deadzone: u32,
    /// PS3_MOUSE_CAPTURE: a click captures the cursor (Esc/Cmd releases it).
    pub mouse_capture: bool,
    /// PS3_RUMBLE=0 when off; PS3_RUMBLE_STRENGTH in percent (0-200).
    pub rumble: bool,
    pub rumble_strength: u32,
    /// PS3_AUDIO_VOLUME (0-100, 100 = the default), PS3_AUDIO_MUTE=1 when
    /// muted, PS3_AUDIO_DEVICE (the output whose name contains it; empty =
    /// the system's default).
    pub audio_volume: u32,
    pub audio_mute: bool,
    pub audio_device: String,
    /// PS3_AUDIO_DOWNMIX=itu: the multichannel mix folded down to stereo
    /// with ITU weights (off: the port's own fold-down).
    pub audio_downmix: bool,
    /// PS3RECOMP_METAL_FPS=1: frame rate and frame times in the window title.
    pub show_fps: bool,
    /// PS3RECOMP_MSL_CACHE=<data>/shader-cache when on.
    pub shader_cache: bool,
    /// SPURS_JC_JOBLIST_PAR=0 when off: the skinning jobs one after another
    /// instead of on four threads (port/main.cpp's default).
    pub parallel_jobs: bool,
    /// PS3_QOS=0 when off: macOS's default priority for the game's threads
    /// instead of user-interactive (off the efficiency cores).
    pub high_priority: bool,
    /// PS3_FRAMECLOCK_US: the host frame loop's tick (1000 = the default).
    pub frame_clock_us: u32,
    /// Run under `caffeinate -i` so the Mac does not sleep mid-game.
    pub keep_awake: bool,
    /// PS3RECOMP_FFMPEG=0 when off (no movies).
    pub video: bool,
    /// PS3RECOMP_FFMPEG_AVCODEC / _AVUTIL; empty = Homebrew's.
    pub ffmpeg_avcodec: String,
    pub ffmpeg_avutil: String,
    /// PS3_VERBOSE=0: the runtime's per-event lines stay out of last.log.
    pub quiet_log: bool,
    /// Extra KEY=VALUE lines (diagnostics), one per line; '#' starts a comment.
    pub extra_env: String,
    /// The rebuild's steps and JOBS (Advanced > Install / rebuild).
    pub rebuild_steps: Vec<String>,
    pub jobs: u32,
    /// Automatic backups of the saves made before each "Play" that are kept.
    pub save_backups_keep: u32,
    /// Look for a new launcher version and new port sources on start.
    pub auto_update_check: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ui_lang: "pt-BR".into(),
            port_dir: String::new(),
            disc_path: String::new(),
            disc_image: String::new(),
            auto_mount: true,
            play_dir: String::new(),
            game_lang: String::new(),
            window_size: String::new(),
            window_mode: "windowed".into(),
            display: String::new(),
            vsync: true,
            upscale: String::new(),
            res_scale: String::new(),
            game_icon: false,
            aa: String::new(),
            sharpen: 0.0,
            aniso: 0,
            gamma_override: false,
            gamma: 1.0,
            vibrance: 0.0,
            contrast: 1.0,
            vignette: 0.0,
            grain: 0.0,
            lut_path: String::new(),
            lut_strength: 1.0,
            input_mode: "both".into(),
            mouse: false,
            mouse_sens: 1.0,
            mouse_invert_y: false,
            mouse_invert_x: false,
            mouse_deadzone: 30,
            mouse_capture: true,
            rumble: true,
            rumble_strength: 100,
            audio_volume: 100,
            audio_mute: false,
            audio_device: String::new(),
            audio_downmix: false,
            show_fps: false,
            shader_cache: true,
            parallel_jobs: true,
            high_priority: true,
            frame_clock_us: 1000,
            keep_awake: true,
            video: true,
            ffmpeg_avcodec: String::new(),
            ffmpeg_avutil: String::new(),
            quiet_log: true,
            extra_env: String::new(),
            rebuild_steps: Vec::new(),
            jobs: 0,
            save_backups_keep: 10,
            auto_update_check: true,
        }
    }
}

fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// %APPDATA% (Windows).
fn appdata() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| home().join("AppData").join("Roaming"))
}

/// The launcher's own folder: launcher.json, backups, reports, the shader
/// cache, and the disc SELF keys tools/fetch_keys.py writes (keys/). Also the
/// default play folder.
fn data_root() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support/afro-samurai-recomp")
    } else if cfg!(windows) {
        appdata().join("afro-samurai-recomp")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share")).join("afro-samurai-recomp")
    }
}

fn settings_path() -> PathBuf {
    data_root().join("launcher.json")
}

/// Where "Install the game" puts the port's sources when this Mac has no
/// checkout of them (a player who only downloaded the app).
fn src_dir() -> PathBuf {
    data_root().join("src")
}

/// The last install's output, as the window showed it.
fn install_log_path() -> PathBuf {
    data_root().join("install.log")
}

/// The port's repository ("Install the game" clones it; AFRO_SOURCES_URL
/// overrides it, e.g. with a fork or a mirror).
const SOURCES_URL: &str = "https://github.com/erikomis/afro-samurai-recomp.git";

fn sources_url() -> String {
    std::env::var("AFRO_SOURCES_URL").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| SOURCES_URL.into())
}

/// The free space a full install asks for. Measured: the lifted sources
/// 1.4 GB, the build 0.4 GB, the toolkit and the venv about 0.3 GB, the
/// disc's EBOOT 0.03 GB; tools/build.sh then wants 4 GB free while it
/// compiles (each lifted chunk takes 3-4 GB of clang memory and its object
/// files). tools/install.sh asks for the same.
const INSTALL_NEED_GB: u64 = 6;

fn reports_dir() -> PathBuf {
    data_root().join("reports")
}

/// The disc SELF key file tools/setup.sh writes (tools/env.sh's AFRO_KEYS).
fn keys_path() -> PathBuf {
    std::env::var_os("AFRO_KEYS").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| data_root().join("keys/self_keys.txt"))
}

/// The player's controls file (PS3_INPUT_MAP), next to launcher.json. It
/// exists once the Controls page has changed a binding (or the mouse camera
/// is on); until then Play passes the port's own, port/input.toml.
fn input_map_path() -> PathBuf {
    data_root().join("input.toml")
}

/// The controls file Play passes, always one when there is one: the player's
/// when it exists, else the port checkout's port/input.toml. With neither
/// (a checkout from before that file) the runtime would use its built-in
/// first-person layout, which is not this port's.
fn pick_input_map(player: &Path, port_root: Option<&Path>) -> InputEnv {
    for p in [Some(player.to_path_buf()), port_root.map(|r| r.join("port/input.toml"))].into_iter().flatten() {
        if let Ok(text) = fs::read_to_string(&p) {
            return InputEnv { path: Some(s(&p)), has_mouse_section: input_map::parse(&text).mouse.is_some() };
        }
    }
    InputEnv::default()
}

/// `[mouse]` of the player's file, from the mouse settings.
fn mouse_section(s: &Settings) -> input_map::Mouse {
    input_map::Mouse {
        camera: s.mouse,
        sensitivity: (s.mouse_sens.clamp(0.05, 10.0) * 100.0).round() / 100.0,
        invert_x: s.mouse_invert_x,
        invert_y: s.mouse_invert_y,
        deadzone: s.mouse_deadzone.min(120),
        capture: s.mouse_capture,
    }
}

/// Write the player's file in full: the port's layout with `bindings` on
/// top, every control listed (one the file left out would get the runtime's
/// first-person default, not the port's), and `[mouse]` from the settings.
/// Sections the launcher does not know stay as they were.
fn write_input_map(path: &Path, bindings: &Bindings, s: &Settings) -> Result<(), String> {
    input_map::validate(bindings)?;
    let mut file = fs::read_to_string(path).map(|t| input_map::parse(&t)).unwrap_or_default();
    file.bindings = input_map::full(bindings);
    file.mouse = Some(mouse_section(s));
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, input_map::render(&file)).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Keep the player's file in step with the mouse settings: its `[mouse]` is
/// rewritten when they changed, and with the mouse camera on and no file yet
/// one is written (the port's layout), because the port's own file leaves
/// the mouse buttons at the runtime's first-person defaults.
fn sync_input_map(path: &Path, s: &Settings) -> Result<(), String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let f = input_map::parse(&text);
            if f.mouse.as_ref() == Some(&mouse_section(s)) {
                return Ok(());
            }
            write_input_map(path, &f.bindings, s)
        }
        Err(_) if s.mouse => write_input_map(path, &Bindings::default(), s),
        Err(_) => Ok(()),
    }
}

fn rpcs3_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support/rpcs3")
    } else if cfg!(windows) {
        // RPCS3 for Windows keeps dev_hdd0 next to rpcs3.exe
        find_rpcs3().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| home().join("rpcs3"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config")).join("rpcs3")
    }
}

/// launcher.json's text as settings. A file from before the window modes and
/// the input mode had two switches instead, which still count: `fullscreen`
/// on is the "fullscreen" window mode, `keyboard` off (PAD_NO_KEYBOARD) is
/// the "controller" input mode.
fn parse_settings(text: &str) -> Option<Settings> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let old = |new: &str, old: &str| if v.get(new).is_none() { v.get(old).and_then(|x| x.as_bool()) } else { None };
    let (fullscreen, keyboard) = (old("windowMode", "fullscreen"), old("inputMode", "keyboard"));
    let mut s: Settings = serde_json::from_value(v).ok()?;
    if fullscreen == Some(true) {
        s.window_mode = "fullscreen".into();
    }
    if keyboard == Some(false) {
        s.input_mode = "controller".into();
    }
    Some(s)
}

fn load_settings_file() -> Settings {
    fs::read_to_string(settings_path()).ok().and_then(|s| parse_settings(&s)).unwrap_or_default()
}

fn write_settings(s: &Settings) -> Result<(), String> {
    let p = settings_path();
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// The play folder: the setting, else the data root.
fn play_dir(s: &Settings) -> PathBuf {
    let p = s.play_dir.trim();
    if p.is_empty() {
        data_root()
    } else {
        PathBuf::from(p)
    }
}

fn parse_env_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let l = l.strip_prefix("export ").unwrap_or(l);
            let (k, v) = l.split_once('=')?;
            let k = k.trim();
            if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            Some((k.to_string(), v.trim().trim_matches('"').to_string()))
        })
        .collect()
}

fn extra_var(s: &Settings, key: &str) -> Option<String> {
    parse_env_lines(&s.extra_env).into_iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn mtime(p: &Path) -> u64 {
    fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

fn data_url(path: &Path, mime: &str) -> Option<String> {
    let mut buf = Vec::new();
    fs::File::open(path).ok()?.read_to_end(&mut buf).ok()?;
    Some(format!("data:{mime};base64,{}", base64(&buf)))
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// The disc
// ---------------------------------------------------------------------------

/// A folder that holds a PS3 disc's PS3_GAME/ (or is PS3_GAME itself).
fn disc_root_of(p: &Path) -> Option<PathBuf> {
    if p.join("PS3_GAME/PARAM.SFO").is_file() {
        Some(p.to_path_buf())
    } else if p.file_name().is_some_and(|n| n == "PS3_GAME") && p.join("PARAM.SFO").is_file() {
        p.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn disc_title_id(root: &Path) -> Option<String> {
    read_sfo(&root.join("PS3_GAME/PARAM.SFO")).and_then(|v| v.get("TITLE_ID").cloned())
}

/// The places a disc can be mounted: the volumes under /Volumes (macOS), or
/// the drive roots A:\ to Z:\ (Windows).
fn volume_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        (b'A'..=b'Z').map(|c| PathBuf::from(format!("{}:\\", c as char))).collect()
    } else {
        fs::read_dir("/Volumes").into_iter().flatten().flatten().map(|e| e.path()).collect()
    }
}

/// Mounted volumes that are this game's disc (as tools/env.sh finds them).
fn mounted_discs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = volume_roots()
        .into_iter()
        .filter(|p| p.join("PS3_GAME/USRDIR/EBOOT.BIN").is_file() && disc_title_id(p).as_deref() == Some(TITLE_ID))
        .collect();
    v.sort();
    v
}

/// The disc Play uses: the setting when it points at one, else the first
/// mounted BLUS30264.
fn find_disc(s: &Settings) -> Option<PathBuf> {
    let p = s.disc_path.trim();
    if !p.is_empty() {
        if let Some(r) = disc_root_of(Path::new(p)) {
            return Some(r);
        }
    }
    mounted_discs().into_iter().next()
}

/// The string values of `key` in an XML plist, in order.
fn plist_strings(xml: &str, key: &str) -> Vec<String> {
    let tag = format!("<key>{key}</key>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&tag) {
        rest = &rest[i + tag.len()..];
        let t = rest.trim_start();
        if let Some(v) = t.strip_prefix("<string>").and_then(|v| v.split_once("</string>")).map(|(v, _)| v) {
            out.push(v.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"));
        }
    }
    out
}

/// Disc images attached now: (image path, its mount points).
fn attached_images() -> Vec<(String, Vec<String>)> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let Some(xml) = command_stdout(Command::new("/usr/bin/hdiutil").args(["info", "-plist"])) else { return Vec::new() };
    // one <key>image-path</key> per attached image; its mount points follow
    // before the next one
    let tag = "<key>image-path</key>";
    let mut out = Vec::new();
    let parts: Vec<&str> = xml.split(tag).collect();
    for part in parts.iter().skip(1) {
        let chunk = format!("{tag}{part}");
        let image = plist_strings(&chunk, "image-path").into_iter().next().unwrap_or_default();
        out.push((image, plist_strings(&chunk, "mount-point")));
    }
    out
}

fn image_of(mount: &Path) -> Option<String> {
    let m = s(mount);
    attached_images().into_iter().find(|(_, mps)| mps.iter().any(|p| *p == m)).map(|(i, _)| i)
}

/// The drive letter of a drive root ("E:\\" or "E:"), Windows.
fn drive_letter(root: &Path) -> Option<char> {
    let t = s(root);
    let b = t.as_bytes();
    let ok = matches!(b.len(), 2 | 3) && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'\\' || b[2] == b'/');
    ok.then(|| (b[0] as char).to_ascii_uppercase())
}

/// Mount-DiskImage the image in $env:AFRO_IMAGE (read only; reused when it is
/// attached already) and print the drive letter it got. A fixed script: the
/// path reaches PowerShell only through the environment, never as source.
const PS_ATTACH: &str = "$ErrorActionPreference = 'Stop'
$p = $env:AFRO_IMAGE
$i = Get-DiskImage -ImagePath $p
if (-not $i.Attached) { $i = Mount-DiskImage -ImagePath $p -Access ReadOnly -PassThru }
$l = $null
for ($n = 0; $n -lt 40 -and -not $l; $n++) {
  $l = ($i | Get-Volume -ErrorAction SilentlyContinue).DriveLetter
  if (-not $l) { Start-Sleep -Milliseconds 250; $i = Get-DiskImage -ImagePath $p }
}
Write-Output ([string]$l)";

/// Dismount-DiskImage the image behind the drive letter in $env:AFRO_DRIVE.
const PS_DETACH: &str = "$ErrorActionPreference = 'Stop'
Get-Volume -DriveLetter $env:AFRO_DRIVE | Get-DiskImage | Dismount-DiskImage | Out-Null";

/// -EncodedCommand's form of a script: base64 of its UTF-16LE text.
fn ps_encoded(script: &str) -> String {
    base64(&script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>())
}

/// The PowerShell command line for a fixed `script` (-EncodedCommand, so no
/// command-line quoting is involved), and the environment that carries its
/// inputs (paths, drive letters) out of band.
fn powershell_cmd(script: &'static str, inputs: &[(&str, &std::ffi::OsStr)]) -> Command {
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &ps_encoded(script)]).stdin(Stdio::null());
    for (k, v) in inputs {
        cmd.env(k, v);
    }
    no_console(&mut cmd);
    cmd
}

/// Run a fixed PowerShell script (Windows) and return its output.
fn powershell(script: &'static str, inputs: &[(&str, &std::ffi::OsStr)]) -> Result<String, String> {
    let out = powershell_cmd(script, inputs).output().map_err(|e| format!("powershell: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Windows: attach the image and return the drive root it got.
fn attach_image_windows(image: &Path) -> Result<PathBuf, String> {
    let out = powershell(PS_ATTACH, &[("AFRO_IMAGE", image.as_os_str())]).map_err(|e| format!("Mount-DiskImage: {e}"))?;
    let letter = out.lines().last().unwrap_or("").trim().chars().next().filter(char::is_ascii_alphabetic).ok_or_else(|| format!("{}: mounted, but Windows gave it no drive letter", image.display()))?;
    let root = PathBuf::from(format!("{letter}:\\"));
    disc_root_of(&root).ok_or_else(|| format!("{}: mounted as {letter}:, but it holds no PS3_GAME folder (not a PS3 disc image?)", image.display()))
}

/// Windows: Dismount-DiskImage the image behind a drive letter.
fn detach_windows(letter: char) -> Result<(), String> {
    let l = letter.to_string();
    powershell(PS_DETACH, &[("AFRO_DRIVE", std::ffi::OsStr::new(&l))])
        .map(|_| ())
        .map_err(|e| format!("{letter}: is not a mounted disc image, or Windows could not eject it ({e})"))
}

/// Attach the image (read-only) and return the disc's mount point:
/// `hdiutil attach` on macOS, Mount-DiskImage on Windows.
fn attach_image(image: &Path) -> Result<PathBuf, String> {
    if !image.is_file() {
        return Err(format!("{}: no such file", image.display()));
    }
    if cfg!(windows) {
        return attach_image_windows(image);
    }
    if !cfg!(target_os = "macos") {
        return Err("mounting a disc image is macOS and Windows only".into());
    }
    // already attached: its mount point
    let img = s(image);
    if let Some((_, mps)) = attached_images().into_iter().find(|(i, _)| *i == img) {
        if let Some(r) = mps.iter().map(PathBuf::from).find_map(|p| disc_root_of(&p)) {
            return Ok(r);
        }
    }
    let out = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-readonly", "-plist"])
        .arg(image)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("hdiutil: {e}"))?;
    if !out.status.success() {
        return Err(format!("hdiutil attach: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let xml = String::from_utf8_lossy(&out.stdout);
    plist_strings(&xml, "mount-point")
        .into_iter()
        .map(PathBuf::from)
        .find_map(|p| disc_root_of(&p))
        .ok_or_else(|| format!("{}: mounted, but it holds no PS3_GAME folder (not a PS3 disc image?)", image.display()))
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscInfo {
    root: Option<String>,
    title: String,
    title_id: String,
    app_ver: String,
    /// The image it is mounted from, when it is one.
    image: Option<String>,
    /// The disc is this game (BLUS30264).
    ours: bool,
    /// Other BLUS30264 volumes mounted now.
    mounted: Vec<String>,
}

fn disc_info(s: &Settings) -> DiscInfo {
    let mut d = DiscInfo { mounted: mounted_discs().iter().map(|p| self::s(p)).collect(), ..Default::default() };
    if let Some(r) = find_disc(s) {
        let v = read_sfo(&r.join("PS3_GAME/PARAM.SFO")).unwrap_or_default();
        d.title = v.get("TITLE").cloned().unwrap_or_default();
        d.title_id = v.get("TITLE_ID").cloned().unwrap_or_default();
        d.app_ver = v.get("APP_VER").or(v.get("VERSION")).cloned().unwrap_or_default();
        d.ours = d.title_id == TITLE_ID;
        if cfg!(windows) {
            // the image this launcher mounted there (no PowerShell per status poll)
            let same = |a: &Path, b: &Path| self::s(a).trim_end_matches(['\\', '/']).eq_ignore_ascii_case(self::s(b).trim_end_matches(['\\', '/']));
            if drive_letter(&r).is_some() && !s.disc_image.trim().is_empty() && same(Path::new(s.disc_path.trim()), &r) {
                d.image = Some(s.disc_image.trim().to_string());
            }
        } else if r.starts_with("/Volumes") {
            d.image = image_of(&r);
        }
        d.root = Some(self::s(&r));
    }
    d
}

/// The disc's ICON0.PNG and PIC1.PNG (the player's own; nothing ships).
fn disc_art(s: &Settings) -> (Option<PathBuf>, Option<PathBuf>) {
    match find_disc(s) {
        Some(r) => {
            let g = r.join("PS3_GAME");
            (Some(g.join("ICON0.PNG")).filter(|p| p.is_file()), Some(g.join("PIC1.PNG")).filter(|p| p.is_file()))
        }
        None => (None, None),
    }
}

// ---------------------------------------------------------------------------
// The builds
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Builds {
    root: String,
    /// build/BLUS30264/AfroSamuraiRecomp and the decrypted game/EBOOT.elf.
    exe: Option<String>,
    eboot_elf: Option<String>,
    /// Both are there: Play can run.
    ok: bool,
    missing: Vec<String>,
    /// tools/rebuild.sh, when this is the port's own repository.
    rebuild_sh: Option<String>,
    /// When the executable was built (its mtime).
    built_at: u64,
}

fn resolve_builds(root: &Path) -> Builds {
    let file = |p: PathBuf| Some(p).filter(|p| p.is_file());
    let exe = file(root.join("build").join(TITLE_ID).join(GAME_EXE));
    let eboot_elf = file(root.join("game/EBOOT.elf"));
    let mut missing = Vec::new();
    for (what, v) in [(format!("build/{TITLE_ID}/{GAME_EXE}"), exe.is_some()), ("game/EBOOT.elf".to_string(), eboot_elf.is_some())] {
        if !v {
            missing.push(what);
        }
    }
    let rebuild_sh = file(root.join("tools/rebuild.sh"));
    let o = |p: &Option<PathBuf>| p.as_deref().map(s);
    Builds {
        root: s(root),
        built_at: exe.as_deref().map(mtime).unwrap_or(0),
        ok: missing.is_empty(),
        exe: o(&exe),
        eboot_elf: o(&eboot_elf),
        missing,
        rebuild_sh: o(&rebuild_sh),
    }
}

/// The checkout this launcher was built from (development builds, and an
/// .app built inside the repository by tools/rebuild.sh launcher).
fn own_repo() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    p.join("tools/rebuild.sh").is_file().then(|| p.canonicalize().unwrap_or(p))
}

/// Port folders found on this Mac: AFRO_ROOT, the repository the launcher
/// comes from, then a folder one level under the usual places that has the
/// EBOOT build. Complete builds first, the newest build first.
fn find_port_dirs_now() -> Vec<Builds> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(r) = std::env::var_os("AFRO_ROOT") {
        cands.push(PathBuf::from(r));
    }
    cands.extend(own_repo());
    // "Install the game"'s own copy of the sources
    if src_dir().join("tools/rebuild.sh").is_file() {
        cands.push(src_dir());
    }
    for base in ["Desktop", "Developer", "Documents", "Projects", "src", "code", "git", ""] {
        let dir = home().join(base);
        let Ok(it) = fs::read_dir(&dir) else { continue };
        for e in it.flatten() {
            let p = e.path();
            if p.join("build").join(TITLE_ID).is_dir() && !e.file_name().to_string_lossy().starts_with('.') {
                cands.push(p);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut v: Vec<Builds> = cands
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|p| p.canonicalize().unwrap_or(p))
        .filter(|p| seen.insert(p.clone()))
        .map(|p| resolve_builds(&p))
        .collect();
    v.sort_by(|a, b| b.ok.cmp(&a.ok).then(b.built_at.cmp(&a.built_at)));
    v
}

/// The port folder in use: the setting, else the best one found.
fn port_root(s: &Settings) -> Option<PathBuf> {
    let p = s.port_dir.trim();
    if !p.is_empty() {
        return Some(PathBuf::from(p));
    }
    find_port_dirs_now().into_iter().find(|b| b.ok || b.rebuild_sh.is_some()).map(|b| PathBuf::from(b.root))
}

// ---------------------------------------------------------------------------
// The play folder
// ---------------------------------------------------------------------------

/// The VFS root Play assembles in the play folder, as tools/run.sh does: the
/// disc's PS3_GAME and PS3_DISC.SFB linked in at leaf level, so /dev_bdvd
/// resolves into the disc and nothing is ever written to it.
fn vfs_root(play: &Path) -> PathBuf {
    play.join("dev_hdd0/disc").join(TITLE_ID)
}

#[cfg(unix)]
fn link_vfs(vfs: &Path, disc: &Path) -> Result<(), String> {
    fs::create_dir_all(vfs).map_err(|e| format!("{}: {e}", vfs.display()))?;
    for leaf in ["PS3_GAME", "PS3_DISC.SFB"] {
        let link = vfs.join(leaf);
        let target = disc.join(leaf);
        if fs::read_link(&link).is_ok_and(|t| t == target) {
            continue;
        }
        match fs::symlink_metadata(&link) {
            Ok(m) if m.file_type().is_symlink() => fs::remove_file(&link).map_err(|e| format!("{}: {e}", link.display()))?,
            Ok(_) => return Err(format!("{}: in the way of the disc link (not a link; move it)", link.display())),
            Err(_) => {}
        }
        if target.exists() {
            std::os::unix::fs::symlink(&target, &link).map_err(|e| format!("{}: {e}", link.display()))?;
        }
    }
    Ok(())
}
#[cfg(not(unix))]
fn link_vfs(_vfs: &Path, _disc: &Path) -> Result<(), String> {
    Ok(())
}

/// The VFS root Play passes: the linked one on macOS, the disc itself
/// elsewhere (no symlinks needed there).
fn vfs_for(play: &Path, disc: &Path) -> PathBuf {
    if cfg!(unix) {
        vfs_root(play)
    } else {
        disc.to_path_buf()
    }
}

/// The cellSaveData root: PS3_GAMEDATA_ROOT, else the play folder; the
/// runtime appends dev_hdd0/home/00000001/savedata (tools/run.sh's layout).
fn save_locations(s: &Settings) -> saves::Locations {
    let play = play_dir(s);
    let gamedata = extra_var(s, "PS3_GAMEDATA_ROOT").map(PathBuf::from).unwrap_or_else(|| play.clone());
    saves::Locations {
        port_savedata: gamedata.join("dev_hdd0/home").join(saves::PORT_USER).join("savedata"),
        rpcs3_hdd0: rpcs3_dir().join("dev_hdd0"),
        backups: data_root().join("save-backups"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveInfo {
    name: String,
    title: String,
    subtitle: String,
    modified: u64,
    folder: String,
}

fn last_save(s: &Settings) -> Option<SaveInfo> {
    saves::list_slots(&save_locations(s), Side::Port, "", false)
        .into_iter()
        .max_by_key(|x| x.modified)
        .map(|x| SaveInfo { name: x.name, title: x.title, subtitle: x.subtitle, modified: x.modified, folder: x.path })
}

/// The guest shaders' MSL translations (PS3RECOMP_MSL_CACHE): a first-seen
/// shader costs a glslang + spirv-cross pass only once.
fn shader_cache_dir(s: &Settings) -> Option<PathBuf> {
    match extra_var(s, "PS3RECOMP_MSL_CACHE") {
        Some(v) if v.is_empty() || v == "0" => None,
        Some(v) => Some(PathBuf::from(v)),
        None => s.shader_cache.then(|| data_root().join("shader-cache")),
    }
}

// ---------------------------------------------------------------------------
// Processes
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct AppState {
    rebuild: Mutex<Option<u32>>,
    game: Mutex<Option<u32>>,
    setup: Mutex<Option<u32>>,
    /// The report folder of the game started in debug mode, and whether that
    /// game is still running.
    report: Mutex<Option<(PathBuf, bool)>>,
    /// Where the last game this launcher started writes its output.
    last_log: Mutex<Option<PathBuf>>,
    /// The launcher's Dock icon as last set (app_icon::key; "" = the bundle's).
    dock_icon: Mutex<String>,
    play_request: Mutex<bool>,
    exit_with_game: Mutex<bool>,
    /// `--view <page>`: the page to open on (play_shortcut::start_view).
    start_view: Mutex<Option<String>>,
    /// build_features' cache.
    features: Mutex<FeatureCache>,
    /// An app update is being downloaded and installed (update.rs).
    updating: Mutex<bool>,
    /// The version an update put on disk this session (restart to finish).
    update_installed: Mutex<Option<String>>,
}

/// Play, the rebuild and the install wait while the app updates itself.
fn refuse_while_updating(state: &AppState) -> Result<(), String> {
    if *state.updating.lock().unwrap() {
        return Err("the launcher is installing its update: wait for it to finish".into());
    }
    Ok(())
}

/// A GUI app on macOS starts with a bare PATH; Homebrew's folders and the
/// usual install places are added in front. Windows: PATH as it is.
fn tool_path() -> std::ffi::OsString {
    let mut parts: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        parts.extend(["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"].map(PathBuf::from));
    }
    if let Some(p) = std::env::var_os("PATH") {
        parts.extend(std::env::split_paths(&p));
    }
    if cfg!(unix) {
        parts.extend(["/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(PathBuf::from));
    }
    let mut seen = std::collections::HashSet::new();
    parts.retain(|p| seen.insert(p.clone()));
    std::env::join_paths(parts).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}

/// The game builds and runs on macOS (Metal) only. AFRO_ANY_HOST lets the
/// window be tried elsewhere (development).
fn host_supported() -> bool {
    cfg!(target_os = "macos") || std::env::var_os("AFRO_ANY_HOST").is_some_and(|v| !v.is_empty())
}

fn require_supported_host() -> Result<(), String> {
    if host_supported() {
        Ok(())
    } else {
        Err(format!("the game runs on macOS only; this is {}", std::env::consts::OS))
    }
}

/// No console window for a child of this GUI app (Windows).
#[cfg(windows)]
pub(crate) fn no_console(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}
#[cfg(not(windows))]
pub(crate) fn no_console(cmd: &mut Command) -> &mut Command {
    cmd
}

fn clt_installed() -> bool {
    if !cfg!(target_os = "macos") {
        return true;
    }
    Command::new("xcode-select")
        .arg("-p")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success() && Path::new(String::from_utf8_lossy(&o.stdout).trim()).is_dir())
        .unwrap_or(false)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&tool_path()).map(|d| d.join(&file)).find(|p| p.is_file())
}

fn find_brew() -> Option<PathBuf> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].iter().map(PathBuf::from).find(|p| p.is_file())
}

fn brew_prefix() -> PathBuf {
    match find_brew() {
        Some(b) => b.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/opt/homebrew")),
        None => PathBuf::from("/opt/homebrew"),
    }
}

/// libavcodec / libavutil for the movie decoders: the settings, else
/// Homebrew's (the unversioned link, or the newest versioned one). The
/// runtime dlopens them by name, which a GUI app's search path misses.
fn ffmpeg_libs(s: &Settings) -> (Option<PathBuf>, Option<PathBuf>) {
    if cfg!(windows) {
        // optional there: the paths set, else whatever the port finds itself
        let over = |v: &str| Some(PathBuf::from(v.trim())).filter(|_| !v.trim().is_empty());
        return (over(&s.ffmpeg_avcodec), over(&s.ffmpeg_avutil));
    }
    let libs = [brew_prefix().join("lib")];
    let pick = |over: &str, name: &str| -> Option<PathBuf> {
        if !over.trim().is_empty() {
            return Some(PathBuf::from(over.trim()));
        }
        libs.iter().find_map(|lib| {
            let plain = lib.join(format!("{name}.dylib"));
            if plain.exists() {
                return Some(plain);
            }
            let mut v: Vec<PathBuf> = fs::read_dir(lib)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{name}.")) && n.to_string_lossy().ends_with(".dylib")))
                .collect();
            v.sort();
            v.pop()
        })
    };
    (pick(&s.ffmpeg_avcodec, "libavcodec"), pick(&s.ffmpeg_avutil, "libavutil"))
}

#[cfg(unix)]
fn own_process_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0); // so a stop reaches what it started too
}
#[cfg(not(unix))]
fn own_process_group(cmd: &mut Command) {
    no_console(cmd);
}

/// Stop what this launcher started, with what it started in turn: SIGTERM
/// (SIGKILL when `force`) to the process group `pid` leads; on Windows
/// `taskkill /T` (a close request to its windows; /F when `force`).
fn kill_tree(pid: u32, force: bool) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").arg(if force { "-KILL" } else { "-TERM" }).arg(format!("-{pid}")).status();
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T"]);
        if force {
            cmd.arg("/F");
        }
        let _ = no_console(&mut cmd).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (pid, force);
}

fn command_stdout(cmd: &mut Command) -> Option<String> {
    let out = cmd.stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// A process with this exact name is running (the game started from a
/// terminal). Only looked at, never signalled.
#[cfg(not(windows))]
fn process_running(name: &str) -> bool {
    Command::new("pgrep").args(["-x", name]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}
#[cfg(windows)]
fn process_running(name: &str) -> bool {
    let image = if name.to_ascii_lowercase().ends_with(".exe") { name.to_string() } else { format!("{name}.exe") };
    let mut cmd = Command::new("tasklist");
    cmd.args(["/FI", &format!("IMAGENAME eq {image}"), "/NH", "/FO", "CSV"]);
    no_console(&mut cmd)
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains(&format!("\"{}\"", image.to_ascii_lowercase())))
}

fn shell_quote(v: &str) -> String {
    if !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:,=+@".contains(c)) {
        v.to_string()
    } else {
        format!("'{}'", v.replace('\'', "'\\''"))
    }
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

/// PS3_RES_SCALE for the `res_scale` setting: a multiple above 1 and up to 4
/// (the runtime's range, in quarter steps), or None for native (also for
/// anything unparsable).
fn res_scale_value(v: &str) -> Option<String> {
    let x: f32 = v.trim().parse().ok()?;
    (x > 1.0 && x <= 4.0).then(|| format!("{}", (x * 4.0).round() / 4.0))
}

/// "<w>x<h>", as PS3RECOMP_METAL_WINDOW takes it (the runtime accepts
/// 160x90 .. 16384x16384; the launcher keeps to sane screen sizes).
fn valid_window_size(v: &str) -> bool {
    let mut it = v.splitn(2, 'x').map(|n| n.parse::<u32>().ok());
    matches!((it.next(), it.next()), (Some(Some(w)), Some(Some(h))) if (320..=8192).contains(&w) && (180..=8192).contains(&h))
}

/// The environment Play gives the game, on top of the launcher's own: what
/// tools/run.sh sets (the VFS root, the play folder's dev_hdd0/dev_hdd1 and
/// save root, the boot path, the title, the shader cache, the firmware tree
/// when RPCS3 has one), FFmpeg, then every setting that differs from the
/// port's default; the extra variables last (they win).
/// `icon`: the game window's Dock icon (game_icon below). `input`: the
/// controls file (`pick_input_map`).
fn launch_env(s: &Settings, vfs: &Path, play: &Path, icon: Option<&Path>, input: &InputEnv) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = Vec::new();
    let mut set = |k: &str, v: String| env.push((k.to_string(), v));
    if let Some(i) = icon {
        set("PS3_APP_ICON", s_(i));
    }
    set("PS3_VFS_ROOT", s_(vfs));
    set("PS3_HDD0_ROOT", s_(&play.join("dev_hdd0")));
    set("PS3_HDD1_ROOT", s_(&play.join("dev_hdd1")));
    set("PS3_GAMEDATA_ROOT", s_(play));
    set("PS3_BOOT_PATH", "/dev_bdvd/PS3_GAME/USRDIR/EBOOT.BIN".into());
    set("PS3_TITLE", WINDOW_TITLE.into());
    // FMOD may load its MP3 codec from /dev_flash: RPCS3's firmware tree,
    // read-only, when it is installed.
    let flash = rpcs3_dir().join("dev_flash");
    if flash.is_dir() {
        set("PS3_DEV_FLASH", s_(&flash));
    }
    // tools/run.sh's: the audio clock follows the host's output.
    set("AUDIO_ELASTIC", "1".into());
    if s.video {
        let (codec, util) = ffmpeg_libs(s);
        if let Some(c) = codec {
            set("PS3RECOMP_FFMPEG_AVCODEC", s_(&c));
        }
        if let Some(u) = util {
            set("PS3RECOMP_FFMPEG_AVUTIL", s_(&u));
        }
    } else {
        set("PS3RECOMP_FFMPEG", "0".into());
    }
    let lang = s.game_lang.trim();
    if !lang.is_empty() && lang.len() <= 8 && lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        set("PS3_LANG", lang.to_string());
    }
    // The window.
    if valid_window_size(&s.window_size) {
        set("PS3RECOMP_METAL_WINDOW", s.window_size.clone());
    }
    if matches!(s.window_mode.as_str(), "fullscreen" | "borderless") {
        set("PS3_WINDOW_MODE", s.window_mode.clone());
    }
    let display = s.display.trim();
    if !display.is_empty() && !display.contains(['\n', '\r']) {
        set("PS3_DISPLAY", display.to_string());
    }
    if !s.vsync {
        set("PS3_VSYNC", "0".into());
    }
    if let Some(k) = res_scale_value(&s.res_scale) {
        set("PS3_RES_SCALE", k);
    }
    if s.upscale == "metalfx" {
        set("PS3_UPSCALE", "metalfx".into());
    }
    // The present pass: only what differs from the game's picture.
    if s.aa == "fxaa" || s.aa == "smaa" {
        set("PS3_AA", s.aa.clone());
    }
    if s.sharpen >= 0.005 {
        set("PS3_SHARPEN", format!("{:.2}", s.sharpen.min(1.0)));
    }
    if s.aniso > 1 {
        set("PS3_ANISO", s.aniso.min(16).to_string());
    }
    if s.gamma_override {
        set("PS3_DISPLAY_GAMMA", format!("{:.2}", s.gamma.clamp(0.5, 1.5)));
    }
    if s.vibrance.abs() >= 0.005 {
        set("PS3_VIBRANCE", format!("{:.2}", s.vibrance.clamp(-1.0, 1.0)));
    }
    if (s.contrast - 1.0).abs() >= 0.005 && s.contrast > 0.0 {
        set("PS3_CONTRAST", format!("{:.2}", s.contrast.clamp(0.5, 1.5)));
    }
    if s.vignette >= 0.005 {
        set("PS3_VIGNETTE", format!("{:.2}", s.vignette.min(1.0)));
    }
    if s.grain >= 0.005 {
        set("PS3_GRAIN", format!("{:.2}", s.grain.min(1.0)));
    }
    let lut = s.lut_path.trim();
    if !lut.is_empty() {
        set("PS3_LUT", lut.to_string());
        if s.lut_strength < 0.995 {
            set("PS3_LUT_STRENGTH", format!("{:.2}", s.lut_strength.clamp(0.0, 1.0)));
        }
    }
    // Input: the controls file (always, when there is one: the runtime's
    // built-in layout is not this port's), then the mouse and rumble
    // settings. The variables win over the file's [mouse]. The mouse camera
    // is on in the runtime and off in this port, so off is always said; on,
    // every mouse setting is passed when the file has a [mouse] (the port's
    // and the launcher's own do), so the launcher's stand, and otherwise
    // only what differs from the runtime's defaults.
    if let Some(p) = &input.path {
        set("PS3_INPUT_MAP", p.clone());
    }
    if matches!(s.input_mode.as_str(), "controller" | "keyboard") {
        set("PS3_INPUT_MODE", s.input_mode.clone());
    }
    let all = input.has_mouse_section;
    if !s.mouse {
        set("PS3_MOUSE", "0".into());
    } else {
        if all {
            set("PS3_MOUSE", "1".into());
        }
        if all || (s.mouse_sens - 1.0).abs() > 1e-3 {
            set("PS3_MOUSE_SENS", format!("{:.2}", s.mouse_sens.clamp(0.05, 10.0)));
        }
        if all || s.mouse_invert_y {
            set("PS3_MOUSE_INVERT_Y", if s.mouse_invert_y { "1" } else { "0" }.into());
        }
        if all || s.mouse_invert_x {
            set("PS3_MOUSE_INVERT_X", if s.mouse_invert_x { "1" } else { "0" }.into());
        }
        if all || s.mouse_deadzone != 30 {
            set("PS3_MOUSE_DEADZONE", s.mouse_deadzone.min(120).to_string());
        }
        if all || !s.mouse_capture {
            set("PS3_MOUSE_CAPTURE", if s.mouse_capture { "1" } else { "0" }.into());
        }
    }
    if !s.rumble {
        set("PS3_RUMBLE", "0".into());
    } else if s.rumble_strength != 100 {
        set("PS3_RUMBLE_STRENGTH", s.rumble_strength.min(200).to_string());
    }
    // Audio: only what differs from the default (full volume, the system's
    // output). The runtime mutes on any PS3_AUDIO_MUTE value, so it is
    // passed only when on.
    if s.audio_mute {
        set("PS3_AUDIO_MUTE", "1".into());
    } else if s.audio_volume < 100 {
        set("PS3_AUDIO_VOLUME", s.audio_volume.to_string());
    }
    let device = s.audio_device.trim();
    if !device.is_empty() && !device.contains(['\n', '\r']) {
        set("PS3_AUDIO_DEVICE", device.to_string());
    }
    if s.audio_downmix {
        set("PS3_AUDIO_DOWNMIX", "itu".into());
    }
    // Performance.
    if s.show_fps {
        set("PS3RECOMP_METAL_FPS", "1".into()); // exactly "1"
    }
    if let Some(d) = shader_cache_dir(s) {
        set("PS3RECOMP_MSL_CACHE", s_(&d));
    }
    if !s.parallel_jobs {
        set("SPURS_JC_JOBLIST_PAR", "0".into());
    }
    if !s.high_priority {
        set("PS3_QOS", "0".into());
    }
    if s.frame_clock_us > 0 && s.frame_clock_us != 1000 {
        set("PS3_FRAMECLOCK_US", s.frame_clock_us.clamp(250, 16000).to_string());
    }
    set("PS3_VERBOSE", if s.quiet_log { "0" } else { "1" }.into());
    env.extend(parse_env_lines(&s.extra_env));
    env
}

fn s_(p: &Path) -> String {
    s(p)
}

/// The program and arguments: [caffeinate -i] AfroSamuraiRecomp EBOOT.elf.
fn launch_argv(s: &Settings, b: &Builds) -> Result<Vec<String>, String> {
    let need = |v: &Option<String>, what: &str| v.clone().ok_or_else(|| format!("{what} is missing in {} (Advanced > Install / rebuild)", b.root));
    let exe = need(&b.exe, &format!("build/{TITLE_ID}/{GAME_EXE}"))?;
    let elf = need(&b.eboot_elf, "game/EBOOT.elf")?;
    let mut argv = Vec::new();
    if s.keep_awake && Path::new("/usr/bin/caffeinate").is_file() {
        argv.extend(["/usr/bin/caffeinate".to_string(), "-i".to_string()]);
    }
    argv.extend([exe, elf]);
    Ok(argv)
}

/// The equivalent command line, shown in Advanced > Environment.
#[tauri::command]
fn launch_preview(settings: Settings) -> Result<String, String> {
    let root = port_root(&settings).ok_or("no port folder yet (Advanced > Install / rebuild)")?;
    let b = resolve_builds(&root);
    let play = play_dir(&settings);
    let disc = find_disc(&settings).unwrap_or_else(|| PathBuf::from("<your mounted disc>"));
    let input = pick_input_map(&input_map_path(), Some(&root));
    let env = launch_env(&settings, &vfs_for(&play, &disc), &play, game_icon(&settings).as_deref(), &input);
    let mut out = vec![format!("cd {} &&", shell_quote(&s(&play)))];
    out.extend(env.into_iter().map(|(k, v)| format!("{k}={}", shell_quote(&v))));
    out.extend(launch_argv(&settings, &b).unwrap_or_else(|e| vec![format!("# {e}")]).iter().map(|a| shell_quote(a)));
    out.push(format!("> {} 2>&1", shell_quote(&s(&play.join("last.log")))));
    Ok(out.join(" "))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStarted {
    log: String,
    backup: Option<String>,
    backup_error: Option<String>,
    report: Option<String>,
    /// The disc was mounted from the image for this game.
    mounted: Option<String>,
}

/// Start the game. `diagnostic`: debug mode (report.rs), with its log and
/// dumps in a new report folder.
#[tauri::command]
fn play(app: AppHandle, state: State<'_, AppState>, diagnostic: Option<bool>) -> Result<PlayStarted, String> {
    require_supported_host()?;
    refuse_while_updating(&state)?;
    let mut slot = state.game.lock().unwrap();
    if slot.is_some() {
        return Err("the game is already running".into());
    }
    let s = load_settings_file();
    let root = port_root(&s).ok_or("no port folder: pick it in Advanced > Install / rebuild")?;
    let b = resolve_builds(&root);
    if !b.ok {
        return Err(format!("missing in {}: {}", b.root, b.missing.join(", ")));
    }
    let play = play_dir(&s);
    let mut mounted = None;
    let disc = match find_disc(&s) {
        Some(d) => d,
        None if s.auto_mount && !s.disc_image.trim().is_empty() => {
            let d = attach_image(Path::new(s.disc_image.trim()))?;
            mounted = Some(self::s(&d));
            d
        }
        None => return Err("no-disc".into()),
    };
    if disc_title_id(&disc).as_deref() != Some(TITLE_ID) {
        return Err(format!("{}: not a {TITLE_ID} disc", disc.display()));
    }
    // the trees the title writes (tools/run.sh makes the same)
    for d in ["dev_hdd0/game", "dev_hdd0/home/00000001/savedata", "dev_hdd1/cache"] {
        fs::create_dir_all(play.join(d)).map_err(|e| format!("{}: {e}", play.display()))?;
    }
    let vfs = vfs_for(&play, &disc);
    if cfg!(unix) {
        link_vfs(&vfs, &disc)?;
    }
    let report_dir = if diagnostic.unwrap_or(false) { Some(report::create(&reports_dir(), &saves::stamp(now_secs()))?) } else { None };
    let log_path = match &report_dir {
        Some(d) => d.join(report::LOG),
        None => play.join("last.log"),
    };
    let log = fs::File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    let log2 = log.try_clone().map_err(|e| e.to_string())?;

    // the saves as they are now, before the game can touch them
    let (backup, backup_error) = match saves::backup_before_play(&save_locations(&s), s.save_backups_keep, now_secs()) {
        Ok(b) => (b.map(|b| b.id), None),
        Err(e) => (None, Some(e)),
    };

    let argv = launch_argv(&s, &b)?;
    // the player's controls file follows the mouse settings (a failure
    // leaves the file as it is: the variables still carry them)
    let _ = sync_input_map(&input_map_path(), &s);
    let input = pick_input_map(&input_map_path(), Some(&root));
    let mut env = launch_env(&s, &vfs, &play, game_icon(&s).as_deref(), &input);
    if let Some(d) = &report_dir {
        env.extend(report::env(d)); // after the extra variables: the report's paths win
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&play)
        .env("PATH", tool_path())
        .env_remove("PS3RECOMP_METAL_HEADLESS")
        .env_remove("PS3RECOMP_HOST_SURFACES")
        .env_remove("PS3RECOMP_WINDOW_STATE")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log2);
    for (k, v) in &env {
        cmd.env(k, v);
    }
    own_process_group(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", argv[0]))?;
    *slot = Some(child.id());
    drop(slot);
    *state.last_log.lock().unwrap() = Some(log_path.clone());
    let started = now_secs();
    if let Some(d) = &report_dir {
        *state.report.lock().unwrap() = Some((d.clone(), true));
        session_event(d, "start", &format!("pid={}", child.id()));
        let (d2, b2, disc2, now) = (d.clone(), b.clone(), disc.clone(), started);
        let env2 = env.clone();
        let argv2 = argv.clone();
        thread::spawn(move || {
            let _ = fs::write(d2.join(report::SYSTEM), system_info(&b2, &disc2, &env2, &argv2, now));
        });
        spawn_ticker(app.clone(), d.clone());
    }
    let a = app.clone();
    let session = report_dir.clone();
    thread::spawn(move || {
        let status = child.wait();
        let code = status.as_ref().ok().and_then(|s| s.code()).unwrap_or(-1);
        let st = a.state::<AppState>();
        *st.game.lock().unwrap() = None;
        if let Some((_, live)) = st.report.lock().unwrap().as_mut() {
            *live = false;
        }
        let _ = a.emit("game-exit", code);
        let debug = session.is_some();
        if let Some(d) = session {
            session_exit(&d, status.ok(), started);
            let _ = a.emit("report-changed", ());
        }
        if code == 0 && !debug && *st.exit_with_game.lock().unwrap() {
            a.exit(0);
        }
    });
    Ok(PlayStarted { log: s_(&log_path), backup, backup_error, report: report_dir.map(|d| s_(&d)), mounted })
}

/// Stop: SIGTERM to what Play started. The port takes it as the console's
/// Quit Game (REQUEST_EXITGAME, then exit after PS3_QUIT_GRACE_MS, 3 s by
/// default, never during a save); if the group is still there 8 s later it
/// gets SIGKILL. Only this launcher's own process group, never by name.
#[tauri::command]
fn stop_game(app: AppHandle, state: State<'_, AppState>) {
    if let Some(pid) = *state.game.lock().unwrap() {
        if let Some((d, true)) = state.report.lock().unwrap().clone() {
            session_event(&d, "stop", "");
        }
        kill_tree(pid, false);
        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_secs(8));
            if *app.state::<AppState>().game.lock().unwrap() == Some(pid) {
                kill_tree(pid, true);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    builds: Option<Builds>,
    disc: DiscInfo,
    icon: Option<String>,
    background: Option<String>,
    last_save: Option<SaveInfo>,
    game_running: bool,
    /// An AfroSamuraiRecomp this launcher did not start (tools/run.sh).
    other_game_running: bool,
    rebuild_running: bool,
    play_dir: String,
    data_root: String,
    host_supported: bool,
    /// The port's build for this system is untested (Windows).
    experimental: bool,
    os: String,
    app_bundle: Option<String>,
    finder_icon: bool,
    version: String,
    /// FEATURE_MARKERS found in the builds.
    features: Vec<String>,
}

#[tauri::command]
async fn get_status(app: AppHandle, state: State<'_, AppState>) -> Result<Status, String> {
    let s = load_settings_file();
    let game_running = state.game.lock().unwrap().is_some();
    let rebuild_running = state.rebuild.lock().unwrap().is_some();
    let app2 = app.clone();
    let st = tauri::async_runtime::spawn_blocking(move || {
        let builds = port_root(&s).map(|r| resolve_builds(&r));
        let features = builds.as_ref().map(|b| build_features(b, &app2.state::<AppState>().features)).unwrap_or_default();
        let (icon, pic1) = disc_art(&s);
        let play = play_dir(&s);
        sync_app_icon(&app2);
        let bundle = app_icon::app_bundle().filter(|_| cfg!(target_os = "macos"));
        Status {
            builds,
            disc: disc_info(&s),
            icon: icon.and_then(|p| data_url(&p, "image/png")),
            background: pic1.and_then(|p| data_url(&p, "image/png")),
            last_save: last_save(&s),
            game_running,
            other_game_running: !game_running && process_running(GAME_EXE),
            rebuild_running,
            play_dir: self::s(&play),
            data_root: self::s(&data_root()),
            host_supported: host_supported(),
            experimental: false,
            os: std::env::consts::OS.into(),
            finder_icon: bundle.as_deref().is_some_and(app_icon::has_finder_icon),
            app_bundle: bundle.map(|b| self::s(&b)),
            version: env!("CARGO_PKG_VERSION").into(),
            features,
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(st)
}

// ---------------------------------------------------------------------------
// What the build supports
// ---------------------------------------------------------------------------

/// Environment variables whose names in a build's executable say that it has
/// a feature the settings drive. A build made before the feature landed lacks
/// the name; the window then labels (or, for the optional game patches,
/// hides) the setting rather than pass a variable nothing reads.
const FEATURE_MARKERS: &[&str] = &[
    "PS3RECOMP_FULLSCREEN", // the window's size
    "PS3_WINDOW_MODE",      // the window's modes, the screen, vsync
    "PS3_AA",               // the present pass: AA, sharpen, vibrance, contrast
    "PS3_GRAIN",            // vignette, grain and the colour LUT
    "PS3_RES_SCALE",        // internal resolution
    "PS3_UPSCALE",          // MetalFX
    "PS3_AUDIO_VOLUME",     // volume, mute and output device
    "PS3_INPUT_MAP",        // bindings, input mode, mouse camera, rumble
    "PS3RECOMP_MSL_CACHE",  // the shader translation cache
    "SPURS_JC_JOBLIST_PAR", // the skinning jobs on four threads
    "PS3_QUIT_GRACE_MS",    // Stop and closing the window quit the game cleanly
];

/// The markers found in `data` (a NUL-terminated C string in the binary).
fn markers_in(data: &[u8], found: &mut std::collections::BTreeSet<String>) {
    for m in FEATURE_MARKERS {
        if found.contains(*m) {
            continue;
        }
        let mut pat = m.as_bytes().to_vec();
        pat.push(0);
        if memchr::memmem::find(data, &pat).is_some() {
            found.insert((*m).to_string());
        }
    }
}

type FeatureCache = BTreeMap<String, (u64, u64, Vec<String>)>;

/// The markers in the executable of `b`, cached by path, size and mtime:
/// the build is about 370 MB.
fn build_features(b: &Builds, cache: &Mutex<FeatureCache>) -> Vec<String> {
    let mut all = std::collections::BTreeSet::new();
    for p in [&b.exe].into_iter().flatten() {
        let meta = fs::metadata(p).ok();
        let key = (meta.as_ref().map_or(0, |m| m.len()), meta.as_ref().map_or(0, |m| mtime_of(m)));
        let hit = cache.lock().unwrap().get(p).filter(|c| (c.0, c.1) == key).map(|c| c.2.clone());
        let found = match hit {
            Some(v) => v,
            None => {
                let mut set = std::collections::BTreeSet::new();
                if let Ok(data) = fs::read(p) {
                    markers_in(&data, &mut set);
                }
                let v: Vec<String> = set.into_iter().collect();
                cache.lock().unwrap().insert(p.clone(), (key.0, key.1, v.clone()));
                v
            }
        };
        all.extend(found);
    }
    all.into_iter().collect()
}

fn mtime_of(m: &fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Controls, screens
// ---------------------------------------------------------------------------

/// The controls for the Controls page: the full layout in effect (the port's
/// with the player's file on top), the port's own for "Restore defaults",
/// and where the player's file is.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputMapView {
    path: String,
    exists: bool,
    bindings: Bindings,
    defaults: Bindings,
}

fn input_map_view(path: &Path) -> InputMapView {
    let text = fs::read_to_string(path);
    InputMapView {
        path: s(path),
        exists: text.is_ok(),
        bindings: input_map::full(&text.map(|t| input_map::parse(&t).bindings).unwrap_or_default()),
        defaults: input_map::port_defaults(),
    }
}

#[tauri::command]
fn get_input_map() -> InputMapView {
    input_map_view(&input_map_path())
}

#[tauri::command]
fn save_input_map(bindings: Bindings) -> Result<String, String> {
    let p = input_map_path();
    write_input_map(&p, &bindings, &load_settings_file())?;
    Ok(s(&p))
}

/// "Restore defaults": the player's file goes (kept as input.toml.bak), so
/// the port's own applies again. With the mouse camera on a file is still
/// needed (`sync_input_map`): it is written anew with the port's layout.
fn reset_input_map_at(path: &Path, s: &Settings) -> Result<(), String> {
    if path.is_file() {
        fs::rename(path, path.with_extension("toml.bak")).map_err(|e| e.to_string())?;
    }
    sync_input_map(path, s)
}

#[tauri::command]
fn reset_input_map() -> Result<InputMapView, String> {
    let p = input_map_path();
    reset_input_map_at(&p, &load_settings_file())?;
    Ok(input_map_view(&p))
}

/// The names in the JSON array the screen probe prints.
fn parse_screens(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json.trim()).unwrap_or_default()
}

/// The screens in the system's order (NSScreen.screens: the first has the
/// menu bar), by the names the game window's PS3_DISPLAY matches.
#[tauri::command]
async fn list_screens() -> Vec<String> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    tauri::async_runtime::spawn_blocking(|| {
        command_stdout(Command::new("/usr/bin/osascript").args([
            "-l",
            "JavaScript",
            "-e",
            "ObjC.import('AppKit'); JSON.stringify($.NSScreen.screens.js.map(s => s.localizedName.js))",
        ]))
        .map(|s| parse_screens(&s))
        .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

/// The sound outputs `system_profiler SPAudioDataType -json` lists (devices
/// with output channels), by the names PS3_AUDIO_DEVICE matches; the
/// system's default output first.
fn parse_audio_outputs(json: &str) -> Vec<String> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<(bool, String)> = Vec::new();
    for group in v["SPAudioDataType"].as_array().into_iter().flatten() {
        for it in group["_items"].as_array().into_iter().flatten() {
            let name = it["_name"].as_str().unwrap_or("").trim();
            if name.is_empty() || name.contains(['\n', '\r']) || it.get("coreaudio_device_output").is_none() {
                continue;
            }
            if out.iter().any(|(_, n)| n == name) {
                continue;
            }
            out.push((it["coreaudio_default_audio_output_device"].as_str() == Some("spaudio_yes"), name.to_string()));
        }
    }
    out.sort_by_key(|(d, _)| !*d);
    out.into_iter().map(|(_, n)| n).collect()
}

/// The sound outputs the Mac has now (see parse_audio_outputs).
#[tauri::command]
async fn list_audio_outputs() -> Vec<String> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    tauri::async_runtime::spawn_blocking(|| {
        command_stdout(Command::new("/usr/sbin/system_profiler").args(["SPAudioDataType", "-json"]))
            .map(|s| parse_audio_outputs(&s))
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Settings, the port folder, the disc
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_settings() -> Settings {
    load_settings_file()
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    write_settings(&settings)?;
    let _ = sync_input_map(&input_map_path(), &settings); // Play does it again
    sync_app_icon(&app);
    Ok(())
}

#[tauri::command]
async fn find_port_dirs() -> Vec<Builds> {
    tauri::async_runtime::spawn_blocking(find_port_dirs_now).await.unwrap_or_default()
}

#[tauri::command]
fn builds_at(path: String) -> Builds {
    resolve_builds(Path::new(path.trim()))
}

#[tauri::command]
async fn disc_status() -> DiscInfo {
    tauri::async_runtime::spawn_blocking(|| disc_info(&load_settings_file())).await.unwrap_or_default()
}

/// Attach the image and remember it (and its mount point) as the disc.
#[tauri::command]
async fn mount_image(path: String) -> Result<DiscInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = attach_image(Path::new(path.trim()))?;
        let mut st = load_settings_file();
        st.disc_image = path.trim().to_string();
        st.disc_path = s(&root);
        write_settings(&st)?;
        Ok(disc_info(&st))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Eject the disc when it is a mounted image (refused while the game runs).
#[tauri::command]
async fn eject_disc(state: State<'_, AppState>) -> Result<DiscInfo, String> {
    if state.game.lock().unwrap().is_some() || process_running(GAME_EXE) {
        return Err("the game is running: close it first".into());
    }
    tauri::async_runtime::spawn_blocking(|| {
        let st = load_settings_file();
        let root = find_disc(&st).ok_or("no disc mounted")?;
        if cfg!(windows) {
            let letter = drive_letter(&root).ok_or_else(|| format!("{}: not a mounted disc image", root.display()))?;
            detach_windows(letter)?;
            return Ok(disc_info(&st));
        }
        if !root.starts_with("/Volumes") || image_of(&root).is_none() {
            return Err(format!("{}: not a mounted disc image", root.display()));
        }
        let out = Command::new("/usr/bin/hdiutil").arg("detach").arg(&root).stdin(Stdio::null()).output().map_err(|e| format!("hdiutil: {e}"))?;
        if !out.status.success() {
            return Err(format!("hdiutil detach: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(disc_info(&st))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// The Dock and Finder icons from the player's disc (app_icon.rs)
// ---------------------------------------------------------------------------

fn app_icon_cache() -> PathBuf {
    data_root().join(app_icon::CACHE_NAME)
}

/// The running game's Dock icon (PS3_APP_ICON): the square icon drawn from
/// the player's own ICON0.PNG when the disc is there, else this launcher's
/// own icon (the packaged app's icon.icns). A bare executable would show the
/// generic one.
fn game_icon(s: &Settings) -> Option<PathBuf> {
    app_icon::refresh(&app_icon_cache(), disc_art(s).0.as_deref())
        .or_else(|| app_icon::app_bundle().map(|b| b.join("Contents/Resources/icon.icns")).filter(|p| p.is_file()))
}

fn composed_icon(s: &Settings) -> Option<PathBuf> {
    if !s.game_icon {
        return None;
    }
    app_icon::refresh(&app_icon_cache(), disc_art(s).0.as_deref())
}

fn sync_app_icon(app: &AppHandle) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        let icon = composed_icon(&load_settings_file());
        let key = app_icon::key(icon.as_deref());
        let state = app.state::<AppState>();
        let mut cur = state.dock_icon.lock().unwrap();
        if *cur != key {
            *cur = key;
            app_icon::apply_dock_icon(&app, icon);
        }
    });
}

#[tauri::command]
async fn finder_icon(app: AppHandle, set: bool) -> Result<bool, String> {
    if !cfg!(target_os = "macos") {
        return Err("the Finder icon is macOS only".into());
    }
    let bundle = app_icon::app_bundle().ok_or("the launcher is not running from an .app (development build)")?;
    let icon = if set {
        let st = load_settings_file();
        Some(app_icon::refresh(&app_icon_cache(), disc_art(&st).0.as_deref()).ok_or("no ICON0.PNG from your disc yet")?)
    } else {
        None
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let b = bundle.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(app_icon::set_finder_icon(&b, icon.as_deref()));
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())??;
    Ok(app_icon::has_finder_icon(&bundle))
}

#[tauri::command]
fn take_play_request(state: State<'_, AppState>) -> bool {
    std::mem::take(&mut *state.play_request.lock().unwrap())
}

#[tauri::command]
fn take_start_view(state: State<'_, AppState>) -> Option<String> {
    state.start_view.lock().unwrap().take()
}

/// Home > Status > "Play shortcut on the Desktop" (play_shortcut.rs).
#[tauri::command]
async fn create_play_shortcut(app: AppHandle) -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("the Play shortcut is macOS only".into());
    }
    let bundle = app_icon::app_bundle().ok_or("the launcher is not running from an .app (development build)")?;
    let icns = play_shortcut::bundle_icns(&bundle);
    let path = play_shortcut::write(&home().join("Desktop"), &app.config().identifier, icns.as_deref())?;
    if let Some(icon) = composed_icon(&load_settings_file()) {
        let (tx, rx) = std::sync::mpsc::channel();
        let p = path.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(app_icon::set_finder_icon(&p, Some(&icon)));
        })
        .map_err(|e| e.to_string())?;
        if let Ok(Err(e)) = rx.recv() {
            eprintln!("[launcher] Play shortcut icon: {e}");
        }
    }
    Ok(s(&path))
}

// ---------------------------------------------------------------------------
// Add to Steam (steam.rs, steam_art.rs)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamStatus {
    available: bool,
    installed: bool,
    running: bool,
    users: usize,
    added: usize,
}

fn steam_dir() -> PathBuf {
    data_root().join("steam")
}

fn steam_target() -> steam::Target {
    steam::Target::new(&play_shortcut::executable(&steam_dir().join(play_shortcut::NAME)))
}

fn steam_available() -> bool {
    cfg!(target_os = "macos") && app_icon::app_bundle().is_some()
}

fn steam_art(st: &Settings) -> Option<steam::Art> {
    let (icon0, pic1) = disc_art(st);
    let icon0 = app_icon::decode_png(&fs::read(icon0?).ok()?).ok()?;
    let pic1 = pic1.and_then(|p| fs::read(p).ok()).and_then(|b| app_icon::decode_png(&b).ok());
    let imgs = steam_art::draw(&icon0, pic1.as_ref());
    let enc = |i: &app_icon::Rgba| app_icon::encode_png(i).ok();
    Some([enc(&imgs.portrait)?, enc(&imgs.landscape)?, enc(&imgs.hero)?, enc(&imgs.icon)?])
}

#[tauri::command]
async fn steam_status() -> SteamStatus {
    tauri::async_runtime::spawn_blocking(|| {
        let users = steam::users(&steam::root(&home()));
        let t = steam_target();
        SteamStatus {
            available: steam_available(),
            installed: !users.is_empty(),
            running: steam::running(),
            users: users.len(),
            added: users.iter().filter(|u| steam::user_has(u, &t)).count(),
        }
    })
    .await
    .unwrap_or(SteamStatus { available: false, installed: false, running: false, users: 0, added: 0 })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamAdded {
    users: usize,
    art: bool,
}

#[tauri::command]
async fn steam_add(app: AppHandle) -> Result<SteamAdded, String> {
    if !steam_available() {
        return Err("Add to Steam needs the packaged app on macOS".into());
    }
    let id = app.config().identifier.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let users = steam::users(&steam::root(&home()));
        if users.is_empty() {
            return Err("Steam is not installed here, or nobody has signed in to it yet".to_string());
        }
        if steam::running() {
            return Err("steam-running".to_string());
        }
        let icns = app_icon::app_bundle().and_then(|b| play_shortcut::bundle_icns(&b));
        fs::create_dir_all(steam_dir()).map_err(|e| e.to_string())?;
        play_shortcut::write_kind(&steam_dir(), &id, icns.as_deref(), play_shortcut::Kind::Steam)?;
        let art = steam_art(&load_settings_file());
        let t = steam_target();
        for u in &users {
            steam::add(u, &t, art.as_ref())?;
        }
        Ok(SteamAdded { users: users.len(), art: art.is_some() })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn steam_remove(app: AppHandle) -> Result<usize, String> {
    let id = app.config().identifier.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if steam::running() {
            return Err("steam-running".to_string());
        }
        let t = steam_target();
        let mut n = 0;
        for u in &steam::users(&steam::root(&home())) {
            n += steam::remove(u, &t)? as usize;
        }
        let bundle = steam_dir().join(play_shortcut::NAME);
        if play_shortcut::is_ours(&bundle, &id, play_shortcut::Kind::Steam) {
            fs::remove_dir_all(&bundle).map_err(|e| format!("{}: {e}", bundle.display()))?;
        }
        Ok(n)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn steam_quit() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(steam::quit).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Requirements and the rebuild
// ---------------------------------------------------------------------------

/// One line of Advanced > Install / rebuild > "What this computer needs".
/// `level`: "ok", "warn", "fail", "info" (not a problem, only to know) or
/// "optional" (absent, and not needed). The window shows `key` (an i18n key
/// under `prereq.msg.`, with `vars`) and falls back to the English
/// `message`; `path` is shown on a line of its own, shortened, with the whole
/// path as a tooltip; `optional` marks a tool that is only a fallback.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CheckItem {
    level: String,
    name: String,
    message: String,
    key: String,
    vars: BTreeMap<String, String>,
    path: Option<String>,
    optional: bool,
}

impl CheckItem {
    fn new(level: &str, name: &str, key: &str, message: impl Into<String>) -> Self {
        CheckItem { level: level.into(), name: name.into(), key: key.into(), message: message.into(), ..Default::default() }
    }
    /// A check whose detail is only a path.
    fn at(level: &str, name: &str, path: &Path) -> Self {
        CheckItem::new(level, name, "", s(path)).path(path)
    }
    fn var(mut self, k: &str, v: impl ToString) -> Self {
        self.vars.insert(k.into(), v.to_string());
        self
    }
    fn path(mut self, p: &Path) -> Self {
        self.path = Some(s(p));
        self
    }
    fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Prereqs {
    supported: bool,
    clt: bool,
    checks: Vec<CheckItem>,
    ok: bool,
    /// Homebrew has every formula the build needs.
    deps: bool,
    /// Homebrew is installed (brew), and the formulae it still lacks.
    brew: bool,
    brew_missing: Vec<String>,
    /// The port's sources (a folder with tools/rebuild.sh), when there are any;
    /// "Install the game" downloads them otherwise.
    sources: Option<String>,
    /// The game's EBOOT is decrypted already (the disc is then only needed
    /// to play).
    elfs: bool,
    disc: bool,
    builds_ok: bool,
    free_gb: Option<u64>,
    need_gb: u64,
}

/// What the rebuild and the game need, by Homebrew formula: (formula, what
/// shows it is installed). The README's `brew install` line.
const BREW_NEEDS: &[(&str, &[&str])] = &[
    ("cmake", &["bin/cmake"]),
    ("ninja", &["bin/ninja"]),
    ("sdl2", &["lib/libSDL2.dylib"]),
    ("glslang", &["bin/glslangValidator", "bin/glslang"]),
    ("spirv-cross", &["bin/spirv-cross"]),
    ("spirv-tools", &["bin/spirv-opt"]),
    ("ffmpeg", &["lib/libavcodec.dylib"]),
    ("python@3.12", &["bin/python3.12"]),
];

fn find_rpcs3() -> Option<PathBuf> {
    let places = if cfg!(windows) {
        let pf = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Program Files"));
        vec![home().join("rpcs3/rpcs3.exe"), PathBuf::from("C:\\rpcs3\\rpcs3.exe"), pf.join("RPCS3/rpcs3.exe")]
    } else {
        vec![home().join("Applications/RPCS3.app/Contents/MacOS/rpcs3"), PathBuf::from("/Applications/RPCS3.app/Contents/MacOS/rpcs3")]
    };
    places.into_iter().find(|p| p.is_file()).or_else(|| find_in_path("rpcs3"))
}

fn free_gb(p: &Path) -> Option<u64> {
    if cfg!(windows) {
        return None;
    }
    let out = command_stdout(Command::new("/bin/df").arg("-k").arg(p))?;
    let kb: u64 = out.lines().nth(1)?.split_whitespace().nth(3)?.parse().ok()?;
    Some(kb / (1024 * 1024))
}

/// The formulae Homebrew does not have yet (all of them without Homebrew).
fn brew_missing() -> Vec<String> {
    let prefix = brew_prefix();
    let have = find_brew().is_some();
    BREW_NEEDS.iter().filter(|(_, marks)| !have || !marks.iter().any(|m| prefix.join(m).exists())).map(|(f, _)| f.to_string()).collect()
}

/// The toolkit checkout tools/env.sh uses: PS3RECOMP_DIR, else
/// ../ps3recomp-afro next to the port.
fn toolkit_dir(root: &Path) -> PathBuf {
    std::env::var_os("PS3RECOMP_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| root.join("../ps3recomp-afro"))
}

/// The disk space a full install still needs here: none once the builds are there.
fn need_gb(b: Option<&Builds>) -> u64 {
    if b.is_some_and(|b| b.ok) {
        0
    } else {
        INSTALL_NEED_GB
    }
}

#[tauri::command]
async fn check_prereqs() -> Prereqs {
    tauri::async_runtime::spawn_blocking(|| {
        let st = load_settings_file();
        let mut p = Prereqs { supported: host_supported(), clt: clt_installed(), ..Default::default() };
        let mut add = |c: CheckItem| p.checks.push(c);
        if !p.supported {
            return p;
        }
        add(if p.clt {
            CheckItem::new("ok", "xcode-clt", "cltOk", "installed")
        } else {
            CheckItem::new("fail", "xcode-clt", "cltMissing", "not installed (Apple's installer: the compiler and git)")
        });
        p.brew = find_brew().is_some();
        p.brew_missing = brew_missing();
        p.deps = p.brew_missing.is_empty();
        add(if !p.brew {
            CheckItem::new("fail", "brew", "brewMissing", "Homebrew is not installed (brew.sh): the build tools come from it")
        } else if p.deps {
            CheckItem::new("ok", "deps", "depsBrew", "Homebrew's").path(&brew_prefix())
        } else {
            let m = p.brew_missing.join(" ");
            CheckItem::new("warn", "deps", "depsMissing", format!("missing: {m} (Install the build tools)")).var("missing", m)
        });
        let root = port_root(&st).filter(|r| r.join("tools/rebuild.sh").is_file() || resolve_builds(r).ok);
        let b = root.as_deref().map(resolve_builds);
        let elfs = b.as_ref().is_some_and(|b| b.eboot_elf.is_some());
        p.elfs = elfs;
        let disc = find_disc(&st);
        p.disc = disc.is_some();
        add(match disc {
            Some(d) => CheckItem::at("ok", "disc", &d),
            None if elfs => CheckItem::new("warn", "disc", "discForPlay", "not mounted (needed to play)"),
            None => CheckItem::new("fail", "disc", "discMissing", "not mounted (needed to install and to play)"),
        });
        match &b {
            Some(b) if b.rebuild_sh.is_some() => {
                let root = Path::new(&b.root);
                p.sources = Some(b.root.clone());
                let tk = toolkit_dir(root);
                add(if tk.join(".git").exists() {
                    CheckItem::at("ok", "toolkit", &tk.canonicalize().unwrap_or(tk))
                } else {
                    CheckItem::new("warn", "toolkit", "toolkitLater", "not set up yet (the setup step does it)")
                });
                let venv = root.join(".venv");
                add(if venv.join("bin/python").is_file() { CheckItem::at("ok", "venv", &venv) } else { CheckItem::new("warn", "venv", "venvLater", "not made yet (the setup step makes it)") });
                let keys = keys_path();
                add(if keys.is_file() {
                    CheckItem::at("ok", "keys", &keys)
                } else if elfs {
                    CheckItem::new("optional", "keys", "keysNotNeeded", "not needed: the EBOOT is decrypted already").optional()
                } else {
                    CheckItem::new("warn", "keys", "keysLater", "not downloaded yet (the setup step gets them from RPCS3's public source; internet needed once)")
                });
            }
            // a build that can be played but not rebuilt from here
            Some(_) => add(CheckItem::new("info", "rebuild", "rebuildElsewhere", "use the afro-samurai-recomp checkout to rebuild")),
            None => add(CheckItem::new("warn", "rebuild", "noSources", "no port folder yet: Install the game downloads the sources").path(&src_dir())),
        }
        p.need_gb = need_gb(b.as_ref());
        let measured = root.clone().unwrap_or_else(data_root);
        let at = if measured.exists() { measured } else { home() };
        p.free_gb = free_gb(&at);
        if let Some(gb) = p.free_gb {
            add(if p.need_gb == 0 {
                CheckItem::new("ok", "disk", "diskEnough", format!("{gb} GB free: enough")).var("free", gb)
            } else if gb >= p.need_gb {
                CheckItem::new("ok", "disk", "diskNeed", format!("{gb} GB free; the install needs about {} GB", p.need_gb)).var("free", gb).var("need", p.need_gb)
            } else {
                CheckItem::new("warn", "disk", "diskLow", format!("only {gb} GB free; the install needs about {} GB", p.need_gb)).var("free", gb).var("need", p.need_gb)
            });
        }
        if let Some(b) = &b {
            p.builds_ok = b.ok;
            add(if b.ok {
                CheckItem::at("ok", "builds", Path::new(&b.root))
            } else {
                CheckItem::new("warn", "builds", "buildsMissing", format!("missing: {}", b.missing.join(", "))).var("missing", b.missing.join(", "))
            });
        }
        p.ok = !p.checks.iter().any(|c| c.level == "fail");
        p
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
fn install_clt() -> Result<(), String> {
    Command::new("xcode-select")
        .arg("--install")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("xcode-select: {e}"))
}

/// Stream a child's stdout and stderr to the window as `<event>-line`, and
/// its exit code as `<event>-exit`; `slot` holds its pid while it runs.
fn stream_child(app: &AppHandle, child: std::process::Child, event: &'static str, which: fn(&AppState) -> &Mutex<Option<u32>>) {
    stream_child_logged(app, child, event, which, None)
}

/// stream_child, each line also written to `log` (the install's log file).
fn stream_child_logged(app: &AppHandle, mut child: std::process::Child, event: &'static str, which: fn(&AppState) -> &Mutex<Option<u32>>, log: Option<fs::File>) {
    use std::io::Write;
    let log = std::sync::Arc::new(Mutex::new(log));
    let (stdout, stderr) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let (a1, l1) = (app.clone(), log.clone());
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(f) = l1.lock().unwrap().as_mut() {
                let _ = writeln!(f, "{line}");
            }
            let _ = a1.emit(&format!("{event}-line"), line);
        }
    });
    let (a2, l2) = (app.clone(), log);
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(f) = l2.lock().unwrap().as_mut() {
                let _ = writeln!(f, "{line}");
            }
            let _ = a2.emit(&format!("{event}-line"), line);
        }
    });
    let a3 = app.clone();
    thread::spawn(move || {
        let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        *which(&a3.state::<AppState>()).lock().unwrap() = None;
        let _ = a3.emit(&format!("{event}-exit"), code);
    });
}

/// Advanced > Install / rebuild: "Install the build tools", i.e.
/// `brew install` of the formulae Homebrew lacks, streamed as setup-line /
/// setup-exit. Without Homebrew: "homebrew" (the window links brew.sh).
#[tauri::command]
fn install_deps(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    require_supported_host()?;
    if !clt_installed() {
        return Err("install the Xcode Command Line Tools first".into());
    }
    let brew = find_brew().ok_or("homebrew")?;
    let missing = brew_missing();
    if missing.is_empty() {
        return Err("Homebrew has every build tool already".into());
    }
    let mut slot = state.setup.lock().unwrap();
    if slot.is_some() {
        return Err("an install is already running".into());
    }
    let mut cmd = Command::new(&brew);
    cmd.arg("install")
        .args(&missing)
        .env("PATH", tool_path())
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .env("NONINTERACTIVE", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    own_process_group(&mut cmd);
    let child = cmd.spawn().map_err(|e| format!("brew: {e}"))?;
    *slot = Some(child.id());
    drop(slot);
    let _ = app.emit("setup-line", format!("$ brew install {}", missing.join(" ")));
    stream_child(&app, child, "setup", |s| &s.setup);
    Ok(())
}

#[tauri::command]
fn cancel_setup(state: State<'_, AppState>) {
    if let Some(pid) = *state.setup.lock().unwrap() {
        kill_tree(pid, cfg!(windows));
    }
}

const STEPS: &[&str] = &["setup", "stage", "decrypt", "analyse", "lift", "spu", "build"];

/// The rebuild, as a zsh script run in the port folder: tools/setup.sh (the
/// toolkit, the venv and the keys) when it is asked for or the venv is not
/// there yet, then tools/rebuild.sh with the other chosen steps. A rebuild
/// started from here may recompile every lifted chunk (BUILD_ALLOW_FULL=1);
/// tools/build.sh's memory, disk and one-build-at-a-time guards still apply.
fn rebuild_script(steps: &[String]) -> String {
    let want: Vec<&str> = STEPS.iter().copied().filter(|s| steps.is_empty() || steps.iter().any(|x| x == s)).collect();
    let rest: Vec<&str> = want.iter().copied().filter(|s| *s != "setup").collect();
    let mut sc = String::from("set -e\n");
    if want.contains(&"setup") {
        sc.push_str("echo '==> setup'\ntools/setup.sh\n");
    } else if !rest.is_empty() {
        sc.push_str("if [ ! -x .venv/bin/python ]; then\n  echo '==> setup'\n  tools/setup.sh\nfi\n");
    }
    if !rest.is_empty() {
        sc.push_str(&format!("BUILD_ALLOW_FULL=1 exec tools/rebuild.sh {}\n", rest.join(" ")));
    } else {
        sc.push_str("echo '==> done'\n");
    }
    sc
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildRequest {
    steps: Vec<String>,
    jobs: Option<u32>,
}

#[tauri::command]
fn start_rebuild(app: AppHandle, state: State<'_, AppState>, req: RebuildRequest) -> Result<(), String> {
    require_supported_host()?;
    refuse_while_updating(&state)?;
    if let Some(bad) = req.steps.iter().find(|s| !STEPS.contains(&s.as_str())) {
        return Err(format!("{bad}: not a step ({})", STEPS.join(" ")));
    }
    if state.game.lock().unwrap().is_some() {
        return Err("the game is running: close it before rebuilding".into());
    }
    let mut slot = state.rebuild.lock().unwrap();
    if slot.is_some() {
        return Err("a rebuild is already running".into());
    }
    let st = load_settings_file();
    let root = port_root(&st).ok_or("no port folder (pick the afro-samurai-recomp checkout)")?;
    if !root.join("tools/rebuild.sh").is_file() {
        return Err(format!("{}: no tools/rebuild.sh (pick the afro-samurai-recomp checkout)", root.display()));
    }
    let mut cmd = Command::new("/usr/bin/nice");
    cmd.args(["-n", "10", "/bin/zsh", "-c", &rebuild_script(&req.steps)]);
    cmd.current_dir(&root)
        .env("PATH", tool_path())
        .env("TERM", "dumb")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // the stage step copies the EBOOT from the launcher's disc (tools/mount.sh
    // finds it mounted there and mounts nothing)
    if let Some(d) = find_disc(&st) {
        cmd.env("AFRO_MOUNT", d);
    }
    if let Some(j) = req.jobs.filter(|j| *j > 0) {
        cmd.env("JOBS", j.to_string());
    }
    own_process_group(&mut cmd);
    let child = cmd.spawn().map_err(|e| format!("zsh: {e}"))?;
    *slot = Some(child.id());
    drop(slot);
    let _ = app.emit("rebuild-line", format!("$ cd {} && tools/rebuild.sh {}", root.display(), if req.steps.is_empty() { STEPS.join(" ") } else { req.steps.join(" ") }));
    stream_child(&app, child, "rebuild", |s| &s.rebuild);
    Ok(())
}

#[tauri::command]
fn cancel_rebuild(state: State<'_, AppState>) {
    if let Some(pid) = *state.rebuild.lock().unwrap() {
        kill_tree(pid, cfg!(windows));
    }
}

/// "Install the game" (Home), as one zsh script whose stages are announced as
/// "==> <stage>" lines (src/progress.ts): the sources when there is no
/// checkout (`root` None: cloned into `src`, at the tag of this launcher's
/// version when there is one, on the default branch so Updates can pull),
/// then tools/install.sh (setup, stage, decrypt, analyse, lift, spu, build),
/// which skips what an earlier run finished. A checkout from before
/// tools/install.sh gets the full rebuild.
fn install_script(root: Option<&Path>, src: &Path, url: &str, tag: &str) -> String {
    let mut sc = String::from("set -e\n");
    let dir = match root {
        Some(r) => s(r),
        None => {
            let q = shell_quote(&s(src));
            sc.push_str("echo '==> sources (the port, from GitHub)'\n");
            sc.push_str(&format!("SRC={q}\n"));
            sc.push_str("if [ ! -f \"$SRC/tools/install.sh\" ]; then\n");
            sc.push_str("  rm -rf \"$SRC.partial\"\n  mkdir -p \"${SRC:h}\"\n");
            sc.push_str(&format!(
                "  git clone --quiet {} \"$SRC.partial\" || {{ echo 'ERROR: could not download the port'\"'\"'s sources. Check the internet connection and try again.' >&2; exit 1; }}\n",
                shell_quote(url)
            ));
            sc.push_str(&format!(
                "  br=$(git -C \"$SRC.partial\" rev-parse --abbrev-ref HEAD)\n  git -C \"$SRC.partial\" checkout --quiet -B \"$br\" {t} 2>/dev/null && echo \"  at {t}\" || echo '  (no tag {t}: the newest sources)'\n",
                t = shell_quote(tag)
            ));
            sc.push_str("  rm -rf \"$SRC\"\n  mv \"$SRC.partial\" \"$SRC\"\nfi\n");
            sc.push_str("cd \"$SRC\"\nexec tools/install.sh\n");
            return sc;
        }
    };
    sc.push_str(&format!("cd {}\n", shell_quote(&dir)));
    if Path::new(&dir).join("tools/install.sh").is_file() {
        sc.push_str("exec tools/install.sh\n");
    } else {
        sc.push_str(rebuild_script(&[]).trim_start_matches("set -e\n"));
    }
    sc
}

/// "Install the game": everything from the disc to Play, in one go (see
/// install_script). Errors the window explains itself: "clt" (the Command
/// Line Tools are missing), "no-disc". Returns the port folder it works in.
#[tauri::command]
fn start_install(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    require_supported_host()?;
    refuse_while_updating(&state)?;
    if !cfg!(target_os = "macos") {
        return Err("the install is for macOS".into());
    }
    if state.game.lock().unwrap().is_some() {
        return Err("the game is running: close it first".into());
    }
    let mut slot = state.rebuild.lock().unwrap();
    if slot.is_some() {
        return Err("an install or a rebuild is already running".into());
    }
    if !clt_installed() {
        return Err("clt".into());
    }
    let mut st = load_settings_file();
    let root = port_root(&st).filter(|r| r.join("tools/rebuild.sh").is_file());
    if root.is_none() && !st.port_dir.trim().is_empty() {
        // a port folder set by hand that cannot be rebuilt: the install's own
        // sources take over (found on their own once the setting is empty)
        st.port_dir.clear();
        write_settings(&st)?;
    }
    let b = root.as_deref().map(resolve_builds);
    let elfs = b.as_ref().is_some_and(|b| b.eboot_elf.is_some());
    let disc = match find_disc(&st) {
        Some(d) => Some(d),
        None if !st.disc_image.trim().is_empty() => attach_image(Path::new(st.disc_image.trim())).ok(),
        None => None,
    };
    if disc.is_none() && !elfs {
        return Err("no-disc".into());
    }
    let data = data_root();
    fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
    let script = install_script(root.as_deref(), &src_dir(), &sources_url(), &format!("v{}", env!("CARGO_PKG_VERSION")));
    let mut cmd = Command::new("/usr/bin/nice");
    cmd.args(["-n", "10", "/bin/zsh", "-c", &script])
        .current_dir(&data)
        .env("PATH", tool_path())
        .env("TERM", "dumb")
        .env("GIT_TERMINAL_PROMPT", "0")
        // the scripts find their checkout and its toolkit from where they are
        .env_remove("AFRO_ROOT")
        .env_remove("PS3RECOMP_DIR")
        .env_remove("AFRO_KEYS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = &disc {
        cmd.env("AFRO_MOUNT", d);
    }
    if st.jobs > 0 {
        cmd.env("JOBS", st.jobs.to_string());
    }
    own_process_group(&mut cmd);
    let log = fs::File::create(install_log_path()).ok();
    let child = cmd.spawn().map_err(|e| format!("zsh: {e}"))?;
    *slot = Some(child.id());
    drop(slot);
    let at = root.unwrap_or_else(src_dir);
    let first = format!("$ tools/install.sh   # in {}", at.display());
    if let Some(mut f) = log.as_ref().and_then(|f| f.try_clone().ok()) {
        use std::io::Write;
        let _ = writeln!(f, "{first}");
    }
    let _ = app.emit("rebuild-line", first);
    stream_child_logged(&app, child, "rebuild", |s| &s.rebuild, log);
    Ok(s(&at))
}

// ---------------------------------------------------------------------------
// Updates (update.rs)
// ---------------------------------------------------------------------------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    sources: update::Sources,
    /// The newest commit touching the port is newer than the EBOOT build.
    rebuild_needed: bool,
    built_at: u64,
    /// The launcher (latest.json, the install, "updated from").
    app: update::AppUpdate,
}

#[tauri::command]
async fn check_updates(app: AppHandle, online: bool) -> UpdateInfo {
    tauri::async_runtime::spawn_blocking(move || {
        let st = load_settings_file();
        let root = port_root(&st);
        let b = root.as_deref().map(resolve_builds);
        let sources = root.as_deref().map(|r| update::sources(r, online)).unwrap_or_default();
        let built_at = b.as_ref().map_or(0, |b| b.built_at);
        let rebuild_needed = built_at > 0 && sources.sources_time > built_at;
        let app = update::check_app(&app, online, built_at);
        UpdateInfo { sources, rebuild_needed, built_at, app }
    })
    .await
    .unwrap_or_default()
}

/// "Get the new sources": git pull --ff-only in the port folder, streamed as
/// rebuild-line / rebuild-exit (the Install tab's log).
#[tauri::command]
fn pull_sources(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    refuse_while_updating(&state)?;
    let mut slot = state.rebuild.lock().unwrap();
    if slot.is_some() {
        return Err("a rebuild is running".into());
    }
    let root = port_root(&load_settings_file()).ok_or("no port folder")?;
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&root)
        .args(["pull", "--ff-only"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("PATH", tool_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    own_process_group(&mut cmd);
    let child = cmd.spawn().map_err(|e| format!("git: {e}"))?;
    *slot = Some(child.id());
    drop(slot);
    let _ = app.emit("rebuild-line", format!("$ git -C {} pull --ff-only", root.display()));
    stream_child(&app, child, "rebuild", |s| &s.rebuild);
    Ok(())
}

// ---------------------------------------------------------------------------
// Links, files, the log
// ---------------------------------------------------------------------------

const URL_ALLOWED: &[&str] = &["https://rpcs3.net/", "https://brew.sh/", "https://github.com/erikomis/afro-samurai-recomp", "https://github.com/sp00nznet/ps3recomp"];

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !URL_ALLOWED.iter().any(|p| url.starts_with(p)) || url.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err(format!("{url}: not a link this app opens"));
    }
    if cfg!(windows) {
        return Command::new("rundll32").arg("url.dll,FileProtocolHandler").arg(&url).spawn().map(|_| ()).map_err(|e| e.to_string());
    }
    Command::new("open").arg(&url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Explorer: open a folder, or select a file in its folder (Windows).
#[cfg(windows)]
fn reveal_windows(p: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("explorer");
    if p.is_dir() {
        cmd.arg(p);
    } else if p.exists() {
        // raw: explorer parses /select, itself; a '"' (never in a Windows
        // path) could not be quoted, so it is refused
        let t = p.display().to_string();
        if t.contains('"') {
            return Err(format!("{t}: not a path"));
        }
        cmd.raw_arg(format!("/select,\"{t}\""));
    } else {
        cmd.arg(p.parent().unwrap_or(p));
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Show a folder or file in Finder (never opens an .app: that would run it).
#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    #[cfg(windows)]
    {
        reveal_windows(&p)
    }
    #[cfg(not(windows))]
    reveal_unix(p)
}

#[cfg(not(windows))]
fn reveal_unix(p: PathBuf) -> Result<(), String> {
    if !p.exists() {
        let parent = p.parent().map(Path::to_path_buf).unwrap_or(p);
        return Command::new("open").arg(&parent).spawn().map(|_| ()).map_err(|e| e.to_string());
    }
    if p.is_dir() && p.extension().is_none_or(|e| e != "app") {
        Command::new("open").arg(&p).spawn().map(|_| ()).map_err(|e| e.to_string())
    } else {
        Command::new("open").arg("-R").arg(&p).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
}

fn game_log_path(state: &State<'_, AppState>) -> PathBuf {
    state.last_log.lock().unwrap().clone().unwrap_or_else(|| play_dir(&load_settings_file()).join("last.log"))
}

fn tail_text(data: &[u8], cut: bool) -> String {
    let start = if cut { data.iter().position(|&b| b == b'\n').map_or(0, |i| i + 1) } else { 0 };
    String::from_utf8_lossy(&data[start..]).into_owned()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLog {
    path: String,
    exists: bool,
    size: u64,
    modified: u64,
    text: String,
    truncated: bool,
}

#[tauri::command]
fn read_game_log(state: State<'_, AppState>, max_bytes: Option<u64>) -> Result<GameLog, String> {
    let path = game_log_path(&state);
    let max = max_bytes.unwrap_or(512 * 1024).clamp(4 * 1024, 8 * 1024 * 1024);
    let mut out = GameLog { path: s(&path), exists: false, size: 0, modified: 0, text: String::new(), truncated: false };
    let Ok(mut f) = fs::File::open(&path) else { return Ok(out) };
    let meta = f.metadata().map_err(|e| e.to_string())?;
    out.exists = true;
    out.size = meta.len();
    out.modified = mtime(&path);
    out.truncated = meta.len() > max;
    if out.truncated {
        f.seek(SeekFrom::Start(meta.len() - max)).map_err(|e| e.to_string())?;
    }
    let mut buf = Vec::with_capacity(max.min(meta.len()) as usize);
    f.take(max).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    out.text = tail_text(&buf, out.truncated);
    Ok(out)
}

#[tauri::command]
fn log_paths(state: State<'_, AppState>) -> BTreeMap<String, String> {
    let st = load_settings_file();
    let mut m = BTreeMap::new();
    m.insert("game".into(), s(&game_log_path(&state)));
    m.insert("play".into(), s(&play_dir(&st)));
    m.insert("saves".into(), s(&save_locations(&st).port_savedata));
    m.insert("data".into(), s(&data_root()));
    m.insert("install".into(), s(&install_log_path()));
    m
}

// ---------------------------------------------------------------------------
// Shader cache
// ---------------------------------------------------------------------------

fn dir_usage(dir: &Path) -> (u64, u64) {
    let (mut bytes, mut files) = (0u64, 0u64);
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            match e.metadata() {
                Ok(m) if m.is_dir() => {
                    let (b, f) = dir_usage(&e.path());
                    bytes += b;
                    files += f;
                }
                Ok(m) => {
                    bytes += m.len();
                    files += 1;
                }
                Err(_) => {}
            }
        }
    }
    (bytes, files)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderCacheInfo {
    path: Option<String>,
    bytes: u64,
    files: u64,
}

fn shader_cache_info_now() -> ShaderCacheInfo {
    match shader_cache_dir(&load_settings_file()) {
        Some(d) => {
            let (bytes, files) = dir_usage(&d);
            ShaderCacheInfo { path: Some(s(&d)), bytes, files }
        }
        None => ShaderCacheInfo { path: None, bytes: 0, files: 0 },
    }
}

#[tauri::command]
fn shader_cache_info() -> ShaderCacheInfo {
    shader_cache_info_now()
}

#[tauri::command]
fn shader_cache_clear(state: State<'_, AppState>) -> Result<ShaderCacheInfo, String> {
    if state.game.lock().unwrap().is_some() || process_running(GAME_EXE) {
        return Err("the game is running: close it before clearing the shader cache".into());
    }
    if let Some(d) = shader_cache_dir(&load_settings_file()) {
        if d.exists() {
            fs::remove_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
    }
    Ok(shader_cache_info_now())
}

// ---------------------------------------------------------------------------
// Debug mode (report.rs)
// ---------------------------------------------------------------------------

fn session_event(dir: &Path, kind: &str, detail: &str) {
    let ms = now_ms();
    let _ = report::event(dir, ms, &utc_text(ms / 1000), kind, detail);
}

fn spawn_ticker(app: AppHandle, dir: PathBuf) {
    thread::spawn(move || loop {
        thread::sleep(std::time::Duration::from_secs(2));
        let live = matches!(app.state::<AppState>().report.lock().unwrap().as_ref(), Some((d, true)) if *d == dir);
        if !live {
            break;
        }
        session_event(&dir, "tick", "");
    });
}

fn session_exit(dir: &Path, status: Option<std::process::ExitStatus>, started: u64) {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.and_then(|s| s.signal())
    };
    #[cfg(not(unix))]
    let signal: Option<i32> = None;
    let code = status.and_then(|s| s.code());
    let detail = match (code, signal) {
        (Some(c), _) => format!("code={c}"),
        (None, Some(sig)) => format!("signal={sig}"),
        _ => "unknown".into(),
    };
    session_event(dir, "exit", &detail);
    let stopped = fs::read_to_string(dir.join(report::EVENTS)).is_ok_and(|t| t.contains("\tstop\t"));
    let crashed = signal.is_some() || code.is_some_and(|c| c != 0);
    if !crashed || stopped || !cfg!(target_os = "macos") {
        return;
    }
    let diag = home().join("Library/Logs/DiagnosticReports");
    for _ in 0..20 {
        if let Some(p) = report::find_crash_report(&diag, &[GAME_EXE], started) {
            if fs::copy(&p, dir.join(report::CRASH)).is_ok() {
                session_event(dir, "crash-report", &format!("file={}", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
            }
            return;
        }
        thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn command_line(program: &str, args: &[&str]) -> String {
    command_stdout(Command::new(program).args(args)).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// UTC "YYYY-MM-DD HH:MM:SS".
fn utc_text(secs: u64) -> String {
    let st = saves::stamp(secs);
    format!("{}-{}-{} {}:{}:{}", &st[0..4], &st[4..6], &st[6..8], &st[9..11], &st[11..13], &st[13..15])
}

fn system_info(b: &Builds, disc: &Path, env: &[(String, String)], argv: &[String], now: u64) -> String {
    let mut t = String::new();
    let mut line = |k: &str, v: &str| {
        if !v.is_empty() {
            t.push_str(&format!("{k}: {v}\n"));
        }
    };
    line("report", &utc_text(now));
    line("launcher", env!("CARGO_PKG_VERSION"));
    line("os", &format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
    if cfg!(target_os = "macos") {
        line("macos", &format!("{} ({})", command_line("sw_vers", &["-productVersion"]), command_line("sw_vers", &["-buildVersion"])));
        line("model", &command_line("sysctl", &["-n", "hw.model"]));
        line("cpu", &command_line("sysctl", &["-n", "machdep.cpu.brand_string"]));
        let mem = command_line("sysctl", &["-n", "hw.memsize"]).parse::<u64>().unwrap_or(0);
        if mem > 0 {
            line("memory", &format!("{} GB", mem >> 30));
        }
        let gpus = command_stdout(Command::new("/usr/sbin/system_profiler").args(["SPDisplaysDataType", "-json"]))
            .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
            .map(|v| {
                v["SPDisplaysDataType"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|g| g["sppci_model"].as_str().map(String::from))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        line("gpu", &gpus);
    }
    let v = read_sfo(&disc.join("PS3_GAME/PARAM.SFO")).unwrap_or_default();
    line("disc", &format!("{} {} {}", v.get("TITLE_ID").cloned().unwrap_or_default(), v.get("TITLE").cloned().unwrap_or_default(), v.get("APP_VER").cloned().unwrap_or_default()));
    line("port folder", &b.root);
    line("built", &utc_text(b.built_at));
    line("commit", &command_line("git", &["-C", &b.root, "rev-parse", "--short", "HEAD"]));
    t.push_str("\ncommand:\n  ");
    t.push_str(&argv.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "));
    t.push_str("\n\nenvironment:\n");
    for (k, v) in env {
        t.push_str(&format!("  {k}={}\n", shell_quote(v)));
    }
    let home = s(&home());
    if home.len() > 1 {
        t = t.replace(&home, "~");
    }
    t
}

fn current_report(state: &State<'_, AppState>) -> Option<(PathBuf, bool)> {
    let r = state.report.lock().unwrap().clone();
    r.or_else(|| report::latest(&reports_dir()).map(|d| (d, false)))
}

#[tauri::command]
fn report_info(state: State<'_, AppState>) -> Option<report::Info> {
    current_report(&state).map(|(d, live)| report::info(&d, live))
}

#[tauri::command]
fn report_capture(state: State<'_, AppState>) -> Result<report::Info, String> {
    let game = state.game.lock().unwrap().is_some();
    match state.report.lock().unwrap().clone() {
        Some((d, true)) if game => {
            let ms = now_ms();
            report::capture(&d, &utc_text(ms / 1000), ms)?;
            Ok(report::info(&d, true))
        }
        _ => Err("the game was not started in debug mode".into()),
    }
}

fn report_dir_of(state: &State<'_, AppState>, id: Option<&str>) -> Result<(PathBuf, bool), String> {
    let cur = current_report(state);
    match id {
        Some(id) => {
            let d = report::by_id(&reports_dir(), id)?;
            let live = cur.is_some_and(|(c, live)| live && c == d);
            Ok((d, live))
        }
        None => cur.ok_or_else(|| "no report yet: start the game in debug mode first".into()),
    }
}

#[tauri::command]
async fn report_list(state: State<'_, AppState>) -> Result<Vec<report::Info>, String> {
    let cur = current_report(&state);
    tauri::async_runtime::spawn_blocking(move || {
        report::list(&reports_dir(), 40)
            .into_iter()
            .map(|d| {
                let live = cur.as_ref().is_some_and(|(c, live)| *live && *c == d);
                report::info(&d, live)
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn report_triage(state: State<'_, AppState>, id: String) -> Result<report::Info, String> {
    let (dir, live) = report_dir_of(&state, Some(&id))?;
    tauri::async_runtime::spawn_blocking(move || {
        report::summarise(&dir)?;
        Ok(report::info(&dir, live))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn report_zip(state: State<'_, AppState>, id: Option<String>, notes: Option<String>) -> Result<report::Info, String> {
    let (dir, live) = report_dir_of(&state, id.as_deref())?;
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(n) = notes.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            fs::write(dir.join(report::NOTES), format!("{n}\n")).map_err(|e| e.to_string())?;
        }
        let _ = report::summarise(&dir);
        report::pack(&dir, &report::zip_path(&dir), &s(&home()))?;
        Ok(report::info(&dir, live))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn report_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let (_, live) = report_dir_of(&state, Some(&id))?;
    if live {
        return Err("that session is still running".into());
    }
    report::delete(&reports_dir(), &id)
}

// ---------------------------------------------------------------------------
// Saves (saves.rs)
// ---------------------------------------------------------------------------

fn game_busy(state: &AppState) -> bool {
    state.game.lock().unwrap().is_some() || process_running(GAME_EXE)
}

fn with_saves<T>(state: &State<'_, AppState>, f: impl FnOnce(&saves::Locations) -> Result<T, String>) -> Result<T, String> {
    let game = state.game.lock().unwrap();
    if game.is_some() || process_running(GAME_EXE) {
        return Err("the game is running: close it before changing saves".into());
    }
    let r = f(&save_locations(&load_settings_file()));
    drop(game);
    r
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavesOverview {
    port: Vec<Slot>,
    rpcs3: Vec<Slot>,
    rpcs3_users: Vec<String>,
    rpcs3_user: String,
    backups: Vec<BackupMeta>,
    port_dir: String,
    rpcs3_dir: String,
    backups_dir: String,
    game_running: bool,
    rpcs3_running: bool,
}

#[tauri::command]
async fn saves_overview(state: State<'_, AppState>, user: Option<String>) -> Result<SavesOverview, String> {
    let running = game_busy(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let loc = save_locations(&load_settings_file());
        let users = saves::rpcs3_users(&loc);
        let user = user.filter(|u| users.contains(u)).or_else(|| users.first().cloned()).unwrap_or_else(|| saves::PORT_USER.into());
        SavesOverview {
            port: saves::list_slots(&loc, Side::Port, "", true),
            rpcs3: if users.is_empty() { Vec::new() } else { saves::list_slots(&loc, Side::Rpcs3, &user, true) },
            rpcs3_users: users,
            rpcs3_user: user.clone(),
            backups: saves::list_backups(&loc),
            port_dir: s(&loc.port_savedata),
            rpcs3_dir: s(&loc.rpcs3_savedata(&user)),
            backups_dir: s(&loc.backups),
            game_running: running,
            rpcs3_running: process_running("rpcs3") || process_running("RPCS3"),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn saves_backup(state: State<'_, AppState>, side: Side, user: String, slot: Option<String>) -> Result<Option<BackupMeta>, String> {
    with_saves(&state, |loc| {
        let one = slot.map(|s| vec![s]);
        saves::create_backup(loc, side, &user, one.as_deref(), "manual", now_secs())
    })
}

#[tauri::command]
async fn saves_restore(state: State<'_, AppState>, id: String, slot: Option<String>) -> Result<saves::Report, String> {
    with_saves(&state, |loc| saves::restore_backup(loc, &id, slot.as_deref(), now_secs()))
}

#[tauri::command]
async fn saves_delete_backup(state: State<'_, AppState>, id: String) -> Result<(), String> {
    with_saves(&state, |loc| saves::delete_backup(loc, &id))
}

#[tauri::command]
async fn saves_copy(state: State<'_, AppState>, from: Side, to: Side, user: String, slot: String) -> Result<saves::Report, String> {
    with_saves(&state, |loc| saves::copy_slot(loc, from, to, &user, &slot, now_secs()))
}

#[tauri::command]
async fn saves_delete_slot(state: State<'_, AppState>, side: Side, user: String, slot: String) -> Result<BackupMeta, String> {
    with_saves(&state, |loc| saves::delete_slot(loc, side, &user, &slot, now_secs()))
}

#[tauri::command]
async fn saves_export_zip(state: State<'_, AppState>, slots: Option<Vec<String>>, dest: String) -> Result<String, String> {
    let mut dest = PathBuf::from(dest.trim());
    if dest.as_os_str().is_empty() {
        return Err("no file chosen".into());
    }
    if !dest.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        dest.as_mut_os_string().push(".zip");
    }
    with_saves(&state, |loc| saves::export_zip(loc, slots.as_deref(), &dest, now_secs()))?;
    Ok(s(&dest))
}

#[tauri::command]
async fn saves_import_zip(state: State<'_, AppState>, path: String) -> Result<saves::Report, String> {
    with_saves(&state, |loc| saves::import_zip(loc, Path::new(path.trim()), now_secs()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    // the updater plugin needs a public key in tauri.conf.json (update::SIGNED_UPDATES)
    let builder = if update::SIGNED_UPDATES { builder.plugin(tauri_plugin_updater::Builder::new().build()) } else { builder };
    builder
        .manage(AppState::default())
        .setup(|app| {
            sync_app_icon(app.handle());
            let args: Vec<String> = std::env::args().collect();
            let st = app.state::<AppState>();
            *st.play_request.lock().unwrap() = play_shortcut::requested(args.iter().cloned());
            *st.exit_with_game.lock().unwrap() = play_shortcut::exit_requested(&args);
            *st.start_view.lock().unwrap() = play_shortcut::start_view(&args);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_input_map,
            save_input_map,
            reset_input_map,
            list_screens,
            list_audio_outputs,
            get_status,
            find_port_dirs,
            builds_at,
            disc_status,
            mount_image,
            eject_disc,
            check_updates,
            pull_sources,
            update::install_update,
            update::restart_app,
            check_prereqs,
            install_clt,
            install_deps,
            start_install,
            cancel_setup,
            open_url,
            start_rebuild,
            cancel_rebuild,
            launch_preview,
            play,
            stop_game,
            reveal,
            log_paths,
            read_game_log,
            shader_cache_info,
            shader_cache_clear,
            saves_overview,
            saves_backup,
            saves_restore,
            saves_delete_backup,
            saves_copy,
            saves_delete_slot,
            saves_export_zip,
            saves_import_zip,
            report_info,
            report_list,
            report_triage,
            report_capture,
            report_zip,
            report_delete,
            finder_icon,
            take_play_request,
            take_start_view,
            create_play_shortcut,
            steam_status,
            steam_add,
            steam_remove,
            steam_quit
        ])
        .build(tauri::generate_context!())
        .expect("error while running the launcher")
        .run(|app, ev| {
            // Quitting stops an install or a rebuild (its own process group,
            // by pid); "Install the game" continues where it stopped next time.
            if let tauri::RunEvent::Exit = ev {
                let st = app.state::<AppState>();
                for slot in [&st.rebuild, &st.setup] {
                    if let Some(pid) = *slot.lock().unwrap() {
                        kill_tree(pid, false);
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_outputs_from_system_profiler() {
        let json = r#"{"SPAudioDataType":[{"_items":[
            {"_name":"Alto-falantes (MacBook Pro)","coreaudio_device_output":2},
            {"_name":"Microfone (MacBook Pro)","coreaudio_device_input":1},
            {"_name":"LG TV","coreaudio_device_output":2,"coreaudio_default_audio_output_device":"spaudio_yes"},
            {"_name":"DualSense Wireless Controller","coreaudio_device_output":4},
            {"_name":"DualSense Wireless Controller","coreaudio_device_input":2}]}]}"#;
        assert_eq!(parse_audio_outputs(json), vec!["LG TV", "Alto-falantes (MacBook Pro)", "DualSense Wireless Controller"]);
        assert!(parse_audio_outputs("not json").is_empty());
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rl-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn env_lines() {
        let v = parse_env_lines("# c\nA=1\nexport B=\"x y\"\nbad key=2\n\nC=");
        assert_eq!(v, vec![("A".into(), "1".into()), ("B".into(), "x y".into()), ("C".into(), "".into())]);
    }

    #[test]
    fn plist_values() {
        let xml = "<dict><key>image-path</key>\n\t<string>/a/b &amp; c.iso</string><key>system-entities</key><array>\
                   <dict><key>mount-point</key><string>/Volumes/AFRO_SAMURAI</string></dict></array></dict>";
        assert_eq!(plist_strings(xml, "image-path"), vec!["/a/b & c.iso".to_string()]);
        assert_eq!(plist_strings(xml, "mount-point"), vec!["/Volumes/AFRO_SAMURAI".to_string()]);
    }

    #[test]
    fn builds_and_disc_layout() {
        let root = tmp("builds");
        let b = resolve_builds(&root);
        assert!(!b.ok && b.missing.len() == 2 && b.rebuild_sh.is_none());
        touch(&root.join("build/BLUS30264/AfroSamuraiRecomp"));
        assert_eq!(resolve_builds(&root).missing, vec!["game/EBOOT.elf".to_string()]);
        touch(&root.join("game/EBOOT.elf"));
        touch(&root.join("tools/rebuild.sh"));
        let b = resolve_builds(&root);
        assert!(b.ok && b.rebuild_sh.is_some() && b.built_at > 0, "{:?}", b.missing);

        let mut st = Settings::default();
        st.keep_awake = false;
        let argv = launch_argv(&st, &b).unwrap();
        assert_eq!(argv, vec![s(&root.join("build/BLUS30264/AfroSamuraiRecomp")), s(&root.join("game/EBOOT.elf"))]);

        let disc = root.join("disc");
        fs::create_dir_all(disc.join("PS3_GAME")).unwrap();
        assert!(disc_root_of(&disc).is_none());
        fs::write(disc.join("PS3_GAME/PARAM.SFO"), saves::psf_write(&[])).unwrap();
        assert_eq!(disc_root_of(&disc), Some(disc.clone()));
        assert_eq!(disc_root_of(&disc.join("PS3_GAME")), Some(disc.clone()));
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_vfs_links_the_disc_like_run_sh() {
        let t = tmp("vfs");
        let disc = t.join("AFRO_SAMURAI");
        fs::create_dir_all(disc.join("PS3_GAME/USRDIR")).unwrap();
        fs::write(disc.join("PS3_DISC.SFB"), b"x").unwrap();
        let play = t.join("play");
        let vfs = vfs_root(&play);
        assert_eq!(vfs, play.join("dev_hdd0/disc/BLUS30264"));
        link_vfs(&vfs, &disc).unwrap();
        assert_eq!(fs::read_link(vfs.join("PS3_GAME")).unwrap(), disc.join("PS3_GAME"));
        assert_eq!(fs::read_link(vfs.join("PS3_DISC.SFB")).unwrap(), disc.join("PS3_DISC.SFB"));
        // another disc mount point: the links follow it
        let other = t.join("AFRO_SAMURAI 1");
        fs::create_dir_all(other.join("PS3_GAME")).unwrap();
        link_vfs(&vfs, &other).unwrap();
        assert_eq!(fs::read_link(vfs.join("PS3_GAME")).unwrap(), other.join("PS3_GAME"));
        assert!(fs::symlink_metadata(vfs.join("PS3_DISC.SFB")).is_err(), "a stale link to a file the disc lacks goes");
        // a real folder in the way is never touched
        let in_way = t.join("p2");
        fs::create_dir_all(vfs_root(&in_way).join("PS3_GAME")).unwrap();
        assert!(link_vfs(&vfs_root(&in_way), &disc).is_err());
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn launch_env_maps_the_settings() {
        let none = InputEnv::default();
        let env = |st: &Settings| -> BTreeMap<String, String> { launch_env(st, Path::new("/v"), Path::new("/p"), None, &none).into_iter().collect() };
        let mut st = Settings::default();
        st.video = false; // no FFmpeg lookup
        st.shader_cache = false;
        let e = env(&st);
        // what tools/run.sh sets
        assert_eq!(e["PS3_VFS_ROOT"], "/v");
        assert_eq!(e["PS3_HDD0_ROOT"], "/p/dev_hdd0");
        assert_eq!(e["PS3_HDD1_ROOT"], "/p/dev_hdd1");
        assert_eq!(e["PS3_GAMEDATA_ROOT"], "/p");
        assert_eq!(e["PS3_BOOT_PATH"], "/dev_bdvd/PS3_GAME/USRDIR/EBOOT.BIN");
        assert_eq!(e["PS3_TITLE"], "Afro Samurai");
        assert_eq!(e["AUDIO_ELASTIC"], "1");
        assert_eq!(e["PS3_VERBOSE"], "0");
        assert_eq!(e["PS3RECOMP_FFMPEG"], "0");
        // the mouse camera is the one default that differs from the runtime's
        assert_eq!(e["PS3_MOUSE"], "0");
        // the defaults pass nothing else
        for k in [
            "PS3RECOMP_METAL_WINDOW", "PS3RECOMP_FULLSCREEN", "PS3_WINDOW_MODE", "PS3_WINDOW_SIZE", "PS3_DISPLAY", "PS3_VSYNC",
            "PS3_RES_SCALE", "PS3_UPSCALE", "PS3_AA", "PS3_SHARPEN", "PS3_ANISO", "PS3_DISPLAY_GAMMA", "PS3_VIBRANCE", "PS3_CONTRAST",
            "PS3_VIGNETTE", "PS3_GRAIN", "PS3_LUT", "PS3_LUT_STRENGTH", "PS3_INPUT_MAP", "PS3_INPUT_MODE", "PAD_NO_KEYBOARD",
            "PS3_MOUSE_SENS", "PS3_MOUSE_INVERT_X", "PS3_MOUSE_INVERT_Y", "PS3_MOUSE_DEADZONE", "PS3_MOUSE_CAPTURE", "PS3_RUMBLE",
            "PS3_RUMBLE_STRENGTH", "PS3_MUTE", "PS3_AUDIO_MUTE", "PS3_AUDIO_VOLUME", "PS3_AUDIO_DEVICE", "PS3_AUDIO_DOWNMIX",
            "PS3RECOMP_METAL_FPS", "PS3RECOMP_MSL_CACHE", "SPURS_JC_JOBLIST_PAR", "PS3_QOS", "PS3_FRAMECLOCK_US", "PS3_LANG",
            "PS3_APP_ICON",
        ] {
            assert!(!e.contains_key(k), "{k} passed with the defaults");
        }
        let keys: Vec<&str> = e.keys().map(String::as_str).filter(|k| *k != "PS3_DEV_FLASH").collect(); // RPCS3's firmware, when installed
        assert_eq!(keys, ["AUDIO_ELASTIC", "PS3RECOMP_FFMPEG", "PS3_BOOT_PATH", "PS3_GAMEDATA_ROOT", "PS3_HDD0_ROOT", "PS3_HDD1_ROOT", "PS3_MOUSE", "PS3_TITLE", "PS3_VERBOSE", "PS3_VFS_ROOT"]);

        st.window_size = "1920x1080".into();
        st.window_mode = "fullscreen".into();
        st.aa = "smaa".into();
        st.sharpen = 0.5;
        st.aniso = 32;
        st.gamma_override = true;
        st.gamma = 9.0;
        st.vibrance = -0.25;
        st.contrast = 1.2;
        st.input_mode = "controller".into();
        st.audio_mute = true;
        st.audio_downmix = true;
        st.show_fps = true;
        st.shader_cache = true;
        st.parallel_jobs = false;
        st.high_priority = false;
        st.frame_clock_us = 50;
        st.game_lang = "ja".into();
        st.quiet_log = false;
        st.extra_env = "PS3_AA=fxaa\nX=1".into();
        let v = launch_env(&st, Path::new("/v"), Path::new("/p"), Some(Path::new("/i.png")), &none);
        let e: BTreeMap<String, String> = v.iter().cloned().collect();
        assert_eq!(e["PS3_APP_ICON"], "/i.png");
        assert_eq!(e["PS3RECOMP_METAL_WINDOW"], "1920x1080");
        assert_eq!(e["PS3_WINDOW_MODE"], "fullscreen");
        assert!(!e.contains_key("PS3RECOMP_FULLSCREEN"));
        assert_eq!(e["PS3_SHARPEN"], "0.50");
        assert_eq!(e["PS3_ANISO"], "16");
        assert_eq!(e["PS3_DISPLAY_GAMMA"], "1.50");
        assert_eq!(e["PS3_VIBRANCE"], "-0.25");
        assert_eq!(e["PS3_CONTRAST"], "1.20");
        assert_eq!(e["PS3_INPUT_MODE"], "controller");
        assert!(!e.contains_key("PAD_NO_KEYBOARD"));
        assert_eq!(e["PS3_AUDIO_MUTE"], "1");
        assert!(!e.contains_key("PS3_MUTE"));
        assert_eq!(e["PS3_AUDIO_DOWNMIX"], "itu");
        assert_eq!(e["PS3RECOMP_METAL_FPS"], "1");
        assert!(e["PS3RECOMP_MSL_CACHE"].ends_with("afro-samurai-recomp/shader-cache"));
        assert_eq!(e["SPURS_JC_JOBLIST_PAR"], "0");
        assert_eq!(e["PS3_QOS"], "0");
        assert_eq!(e["PS3_FRAMECLOCK_US"], "250");
        assert_eq!(e["PS3_LANG"], "ja");
        assert_eq!(e["PS3_VERBOSE"], "1");
        // the extra variables come last, so they win
        assert_eq!(e["PS3_AA"], "fxaa");
        assert_eq!(v.last().unwrap(), &("X".to_string(), "1".to_string()));
        // a bad window size or language is not passed
        st.window_size = "10x10".into();
        st.game_lang = "a b".into();
        let e = env(&st);
        assert!(!e.contains_key("PS3RECOMP_METAL_WINDOW") && !e.contains_key("PS3_LANG"));
    }

    /// The window modes, the screen, vsync, the internal resolution, MetalFX,
    /// the filters, the input mode, the mouse, rumble and the audio options:
    /// each from its default (nothing passed) to a change.
    #[test]
    fn launch_env_maps_the_new_settings() {
        let env = |st: &Settings, input: &InputEnv| -> BTreeMap<String, String> {
            launch_env(st, Path::new("/v"), Path::new("/p"), None, input).into_iter().collect()
        };
        let mut st = Settings::default();
        st.window_mode = "borderless".into();
        st.window_size = "1920x1080".into();
        st.display = "DELL U2720Q".into();
        st.vsync = false;
        st.res_scale = "1.5".into();
        st.upscale = "metalfx".into();
        st.vignette = 0.5;
        st.grain = 0.1;
        st.lut_path = "/luts/warm.png".into();
        st.lut_strength = 0.6;
        st.input_mode = "keyboard".into();
        st.mouse = true;
        st.mouse_sens = 1.5;
        st.mouse_invert_y = true;
        st.mouse_deadzone = 10;
        st.mouse_capture = false;
        st.rumble_strength = 150;
        st.audio_volume = 40;
        st.audio_device = "AirPods".into();
        // a file without [mouse] (hand-written): only what differs from the
        // runtime's mouse defaults
        let input = InputEnv { path: Some("/data/input.toml".into()), has_mouse_section: false };
        let e = env(&st, &input);
        assert_eq!(e["PS3_WINDOW_MODE"], "borderless");
        assert_eq!(e["PS3RECOMP_METAL_WINDOW"], "1920x1080");
        assert!(!e.contains_key("PS3_WINDOW_SIZE"), "the size stays PS3RECOMP_METAL_WINDOW");
        assert_eq!(e["PS3_DISPLAY"], "DELL U2720Q");
        assert_eq!(e["PS3_VSYNC"], "0");
        assert_eq!(e["PS3_RES_SCALE"], "1.5");
        assert_eq!(e["PS3_UPSCALE"], "metalfx");
        assert_eq!(e["PS3_VIGNETTE"], "0.50");
        assert_eq!(e["PS3_GRAIN"], "0.10");
        assert_eq!(e["PS3_LUT"], "/luts/warm.png");
        assert_eq!(e["PS3_LUT_STRENGTH"], "0.60");
        assert_eq!(e["PS3_INPUT_MAP"], "/data/input.toml");
        assert_eq!(e["PS3_INPUT_MODE"], "keyboard");
        assert!(!e.contains_key("PS3_MOUSE"));
        assert_eq!(e["PS3_MOUSE_SENS"], "1.50");
        assert_eq!(e["PS3_MOUSE_INVERT_Y"], "1");
        assert!(!e.contains_key("PS3_MOUSE_INVERT_X"));
        assert_eq!(e["PS3_MOUSE_DEADZONE"], "10");
        assert_eq!(e["PS3_MOUSE_CAPTURE"], "0");
        assert!(!e.contains_key("PS3_RUMBLE"));
        assert_eq!(e["PS3_RUMBLE_STRENGTH"], "150");
        assert_eq!(e["PS3_AUDIO_VOLUME"], "40");
        assert_eq!(e["PS3_AUDIO_DEVICE"], "AirPods");
        assert!(!e.contains_key("PS3_AUDIO_MUTE"));
        // muted: the mute alone (any value mutes); a device name with a line
        // break is not passed
        st.audio_mute = true;
        st.audio_device = " a\nb ".into();
        let e = env(&st, &input);
        assert_eq!(e["PS3_AUDIO_MUTE"], "1");
        assert!(!e.contains_key("PS3_AUDIO_VOLUME") && !e.contains_key("PS3_AUDIO_DEVICE") && !e.contains_key("PS3_MUTE"));

        // the mouse camera on with a file that has a [mouse] (the port's,
        // camera = false, and every file the launcher writes): every mouse
        // setting is passed, so the launcher's stand
        let mut st = Settings::default();
        st.mouse = true;
        let e = env(&st, &InputEnv { path: Some("/x.toml".into()), has_mouse_section: true });
        assert_eq!(e["PS3_INPUT_MAP"], "/x.toml");
        assert_eq!(e["PS3_MOUSE"], "1");
        assert_eq!(e["PS3_MOUSE_SENS"], "1.00");
        assert_eq!(e["PS3_MOUSE_INVERT_X"], "0");
        assert_eq!(e["PS3_MOUSE_INVERT_Y"], "0");
        assert_eq!(e["PS3_MOUSE_DEADZONE"], "30");
        assert_eq!(e["PS3_MOUSE_CAPTURE"], "1");
        // on with no file at all: the runtime's defaults already
        let e = env(&st, &InputEnv::default());
        for k in ["PS3_INPUT_MAP", "PS3_MOUSE", "PS3_MOUSE_SENS", "PS3_MOUSE_INVERT_X", "PS3_MOUSE_INVERT_Y", "PS3_MOUSE_DEADZONE", "PS3_MOUSE_CAPTURE"] {
            assert!(!e.contains_key(k), "{k}");
        }
        // mouse and rumble off: nothing else of theirs; values out of range
        // or unknown pass nothing
        st.mouse = false;
        st.mouse_sens = 2.0;
        st.rumble = false;
        st.rumble_strength = 50;
        st.window_mode = "windowed".into();
        st.input_mode = "both".into();
        st.aa = "msaa".into();
        st.res_scale = "1".into();
        st.upscale = "fsr".into();
        st.display = "a\nb".into();
        st.lut_strength = 0.5; // no LUT file: no strength either
        st.audio_volume = 100;
        let e = env(&st, &InputEnv { path: Some("/x.toml".into()), has_mouse_section: true });
        assert_eq!(e["PS3_MOUSE"], "0");
        assert_eq!(e["PS3_RUMBLE"], "0");
        for k in [
            "PS3_MOUSE_SENS", "PS3_MOUSE_CAPTURE", "PS3_RUMBLE_STRENGTH", "PS3_WINDOW_MODE", "PS3_INPUT_MODE", "PS3_AA", "PS3_RES_SCALE",
            "PS3_UPSCALE", "PS3_DISPLAY", "PS3_LUT", "PS3_LUT_STRENGTH", "PS3_AUDIO_VOLUME",
        ] {
            assert!(!e.contains_key(k), "{k}");
        }
        // out-of-range values are clamped
        st.mouse = true;
        st.mouse_sens = 99.0;
        st.mouse_deadzone = 500;
        st.rumble = true;
        st.rumble_strength = 900;
        st.vignette = 3.0;
        let e = env(&st, &InputEnv::default());
        assert_eq!((e["PS3_MOUSE_SENS"].as_str(), e["PS3_MOUSE_DEADZONE"].as_str(), e["PS3_RUMBLE_STRENGTH"].as_str(), e["PS3_VIGNETTE"].as_str()), ("10.00", "120", "200", "1.00"));
    }

    #[test]
    fn res_scale_values() {
        assert_eq!(res_scale_value(""), None);
        assert_eq!(res_scale_value("1"), None);
        assert_eq!(res_scale_value("1.5").as_deref(), Some("1.5"));
        assert_eq!(res_scale_value("2").as_deref(), Some("2"));
        assert_eq!(res_scale_value("3").as_deref(), Some("3"));
        assert_eq!(res_scale_value("2.6").as_deref(), Some("2.5"));
        assert_eq!(res_scale_value("5"), None);
        assert_eq!(res_scale_value("1080p"), None);
    }

    #[test]
    fn screens_from_the_probe() {
        assert_eq!(parse_screens("[\"Built-in Retina Display\",\"DELL U2720Q\"]\n"), vec!["Built-in Retina Display", "DELL U2720Q"]);
        assert!(parse_screens("oops").is_empty());
    }

    /// Play always passes a controls file when there is one: the player's
    /// first, else the port checkout's port/input.toml.
    #[test]
    fn the_input_map_play_passes() {
        let d = tmp("pick");
        let player = d.join("data/input.toml");
        let root = d.join("repo");
        // neither: nothing to pass
        assert_eq!(pick_input_map(&player, Some(&root)), InputEnv::default());
        assert_eq!(pick_input_map(&player, None), InputEnv::default());
        // the port's own (it has a [mouse])
        fs::create_dir_all(root.join("port")).unwrap();
        fs::write(root.join("port/input.toml"), "[keyboard]\ncross = \"Z\"\n\n[mouse]\ncamera = false\n").unwrap();
        let port = InputEnv { path: Some(s(&root.join("port/input.toml"))), has_mouse_section: true };
        assert_eq!(pick_input_map(&player, Some(&root)), port);
        // the player's wins once it exists, with or without a port folder
        fs::create_dir_all(player.parent().unwrap()).unwrap();
        fs::write(&player, "[keyboard]\ncross = \"Space\"\n").unwrap();
        let own = InputEnv { path: Some(s(&player)), has_mouse_section: false };
        assert_eq!(pick_input_map(&player, Some(&root)), own);
        assert_eq!(pick_input_map(&player, None), own);
        // "Restore defaults": the port's again, the player's kept as .bak
        reset_input_map_at(&player, &Settings::default()).unwrap();
        assert!(!player.exists() && player.with_extension("toml.bak").is_file());
        assert_eq!(pick_input_map(&player, Some(&root)), port);
        reset_input_map_at(&player, &Settings::default()).unwrap(); // nothing to do
        fs::remove_dir_all(&d).unwrap();
    }

    /// The player's file is the full layout: the port's defaults with the
    /// changes on top, every control listed, and [mouse] from the settings.
    #[test]
    fn input_map_file_is_written_in_full() {
        let d = tmp("input");
        let p = d.join("sub/input.toml");
        let mut st = Settings::default();
        let mut b = Bindings::default();
        b.keyboard.insert("cross".into(), vec!["Space".into()]);
        b.keyboard.insert("r1".into(), vec![]);
        b.mouse_buttons.insert("triangle".into(), vec!["WheelUp".into(), "WheelDown".into()]);
        write_input_map(&p, &b, &st).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.starts_with("# Afro Samurai recomp: controls"));
        assert!(text.contains("[keyboard]\ncross    = \"Space\"\ncircle   = \"X\"\nsquare   = \"A\"\ntriangle = \"S\"\nl1       = \"Q\"\nr1       = \"\"\n"));
        assert!(text.contains("ls_up    = \"Up\"\n") && text.contains("rs_right = \"\"\n"));
        assert!(text.contains("[mouse_buttons]\ncross    = \"\"\ncircle   = \"\"\nsquare   = \"\"\ntriangle = \"WheelUp, WheelDown\"\n"));
        assert!(text.contains("[controller]\ncross    = \"a\"\n"));
        assert!(text.contains("[mouse]\ncamera      = false\nsensitivity = 1.00\ninvert_x    = false\ninvert_y    = false\ndeadzone    = 30\ncapture     = true\n"));
        let f = input_map::parse(&text);
        assert_eq!(f.bindings, input_map::full(&b));
        for map in [&f.bindings.keyboard, &f.bindings.mouse_buttons, &f.bindings.controller] {
            assert_eq!(map.len(), input_map::CONTROLS.len());
        }
        // the page reads the same layout back, with the port's for the reset
        let v = input_map_view(&p);
        assert!(v.exists && v.bindings == input_map::full(&b) && v.defaults == input_map::port_defaults());
        let none = input_map_view(&d.join("none.toml"));
        assert!(!none.exists && none.bindings == input_map::port_defaults());
        // a section the launcher does not know survives the next save; a
        // hand-written [mouse] gives way to the settings
        fs::write(&p, format!("{}\n[future]\nx = 1\n", text.replace("camera      = false", "camera = true"))).unwrap();
        b.keyboard.insert("r1".into(), vec!["F".into()]);
        st.mouse = true;
        st.mouse_sens = 1.5;
        write_input_map(&p, &b, &st).unwrap();
        let f = input_map::parse(&fs::read_to_string(&p).unwrap());
        assert_eq!(f.bindings.keyboard["r1"], vec!["F"]);
        assert_eq!(f.mouse, Some(mouse_section(&st)));
        assert_eq!(f.other, "[future]\nx = 1\n");
        // names that could break the file are refused, and the file stays
        b.keyboard.insert("cross".into(), vec!["Q\"\n[x]".into()]);
        assert!(write_input_map(&p, &b, &st).is_err());
        assert_eq!(input_map::parse(&fs::read_to_string(&p).unwrap()).bindings.keyboard["cross"], vec!["Space"]);
        fs::remove_dir_all(&d).unwrap();
    }

    /// The player's file follows the mouse settings.
    #[test]
    fn the_input_map_follows_the_mouse_settings() {
        let d = tmp("sync");
        let p = d.join("input.toml");
        let mut st = Settings::default();
        // mouse off, no file: none is made (the port's file applies)
        sync_input_map(&p, &st).unwrap();
        assert!(!p.exists());
        // mouse on: the port's layout is written, mouse buttons unbound
        st.mouse = true;
        st.mouse_invert_y = true;
        sync_input_map(&p, &st).unwrap();
        let f = input_map::parse(&fs::read_to_string(&p).unwrap());
        assert_eq!(f.bindings, input_map::port_defaults());
        assert_eq!(f.mouse, Some(input_map::Mouse { camera: true, sensitivity: 1.0, invert_x: false, invert_y: true, deadzone: 30, capture: true }));
        // nothing changed: the file is left alone
        let before = fs::read_to_string(&p).unwrap();
        fs::write(&p, format!("{before}# mine\n")).unwrap();
        sync_input_map(&p, &st).unwrap();
        assert!(fs::read_to_string(&p).unwrap().ends_with("# mine\n"));
        // a setting changed: [mouse] is rewritten, the bindings stay
        let mut b = Bindings::default();
        b.keyboard.insert("cross".into(), vec!["Space".into()]);
        write_input_map(&p, &b, &st).unwrap();
        st.mouse_sens = 2.004;
        st.mouse = false;
        sync_input_map(&p, &st).unwrap();
        let f = input_map::parse(&fs::read_to_string(&p).unwrap());
        assert_eq!(f.bindings.keyboard["cross"], vec!["Space"]);
        assert_eq!(f.mouse.as_ref().map(|m| (m.camera, m.sensitivity)), Some((false, 2.0)));
        // "Restore defaults" with the mouse on writes the port's layout anew
        st.mouse = true;
        reset_input_map_at(&p, &st).unwrap();
        assert!(p.with_extension("toml.bak").is_file());
        assert_eq!(input_map::parse(&fs::read_to_string(&p).unwrap()).bindings, input_map::port_defaults());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_shader_cache_follows_the_extra_variable() {
        let mut st = Settings::default();
        assert!(shader_cache_dir(&st).unwrap().ends_with("shader-cache"));
        st.extra_env = "PS3RECOMP_MSL_CACHE=/elsewhere".into();
        assert_eq!(shader_cache_dir(&st), Some(PathBuf::from("/elsewhere")));
        st.extra_env = "PS3RECOMP_MSL_CACHE=0".into();
        assert_eq!(shader_cache_dir(&st), None);
    }

    #[test]
    fn saves_live_in_the_play_folder() {
        let mut st = Settings::default();
        st.play_dir = "/repo/rpcs3".into();
        assert_eq!(save_locations(&st).port_savedata, PathBuf::from("/repo/rpcs3/dev_hdd0/home/00000001/savedata"));
        st.extra_env = "PS3_GAMEDATA_ROOT=/g".into();
        assert_eq!(save_locations(&st).port_savedata, PathBuf::from("/g/dev_hdd0/home/00000001/savedata"));
    }

    #[test]
    fn old_or_partial_settings_files_still_load() {
        let st = parse_settings(r#"{"uiLang":"en","aa":"fxaa","windowMode":"borderless","unknown":3}"#).unwrap();
        assert_eq!((st.ui_lang.as_str(), st.aa.as_str(), st.window_mode.as_str()), ("en", "fxaa", "borderless"));
        assert!(st.parallel_jobs && st.high_priority && st.shader_cache && st.auto_mount);
        // the new settings at their defaults
        assert!(st.vsync && !st.mouse && st.mouse_capture && st.rumble && !st.audio_mute);
        assert_eq!((st.mouse_sens, st.mouse_deadzone, st.rumble_strength, st.audio_volume, st.lut_strength), (1.0, 30, 100, 100, 1.0));
        assert_eq!(st.input_mode, "both");
        assert!(st.display.is_empty() && st.res_scale.is_empty() && st.upscale.is_empty() && st.lut_path.is_empty() && st.audio_device.is_empty());
        assert!(parse_settings("not json").is_none() && parse_settings(r#"{"aniso":"x"}"#).is_none());
    }

    /// The two switches the window mode and the input mode replaced.
    #[test]
    fn old_switches_become_the_new_modes() {
        let old = parse_settings(r#"{"fullscreen":true,"keyboard":false,"audioMute":true}"#).unwrap();
        assert_eq!((old.window_mode.as_str(), old.input_mode.as_str()), ("fullscreen", "controller"));
        let e: BTreeMap<String, String> = launch_env(&old, Path::new("/v"), Path::new("/p"), None, &InputEnv::default()).into_iter().collect();
        assert_eq!((e["PS3_WINDOW_MODE"].as_str(), e["PS3_INPUT_MODE"].as_str(), e["PS3_AUDIO_MUTE"].as_str()), ("fullscreen", "controller", "1"));
        // at their old defaults they change nothing
        let old = parse_settings(r#"{"fullscreen":false,"keyboard":true}"#).unwrap();
        assert_eq!((old.window_mode.as_str(), old.input_mode.as_str()), ("windowed", "both"));
        // a file that has the new settings keeps them, whatever the old say
        let new = parse_settings(r#"{"fullscreen":true,"windowMode":"windowed","keyboard":false,"inputMode":"keyboard"}"#).unwrap();
        assert_eq!((new.window_mode.as_str(), new.input_mode.as_str()), ("windowed", "keyboard"));
        // what the launcher writes has no trace of the old switches
        let text = serde_json::to_string(&parse_settings(r#"{"fullscreen":true}"#).unwrap()).unwrap();
        assert!(text.contains(r#""windowMode":"fullscreen""#) && !text.contains(r#""fullscreen":"#) && !text.contains(r#""keyboard":"#));
    }

    #[test]
    fn feature_markers_in_a_build() {
        let mut found = std::collections::BTreeSet::new();
        markers_in(b"..PS3_AA\0..SPURS_JC_JOBLIST_PAR\0..PS3RECOMP_FULLSCREENX\0", &mut found);
        assert_eq!(found.into_iter().collect::<Vec<_>>(), vec!["PS3_AA".to_string(), "SPURS_JC_JOBLIST_PAR".to_string()]);
        // the options that came with the window modes, the filters, the
        // audio options and the controls file; a longer name is not a match
        let mut found = std::collections::BTreeSet::new();
        markers_in(b"PS3_RES_SCALE\0PS3_UPSCALE\0PS3_WINDOW_MODE\0PS3_GRAIN\0PS3_AUDIO_VOLUME\0PS3_INPUT_MAP\0PS3_INPUT_MAPS\0", &mut found);
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["PS3_AUDIO_VOLUME", "PS3_GRAIN", "PS3_INPUT_MAP", "PS3_RES_SCALE", "PS3_UPSCALE", "PS3_WINDOW_MODE"]);
        for m in ["PS3_RES_SCALE", "PS3_UPSCALE", "PS3_WINDOW_MODE", "PS3_GRAIN", "PS3_AUDIO_VOLUME", "PS3_INPUT_MAP"] {
            assert!(FEATURE_MARKERS.contains(&m), "{m}");
        }
    }

    #[test]
    fn windows_drive_roots_and_quoting() {
        assert_eq!(drive_letter(Path::new("e:\\")), Some('E'));
        assert_eq!(drive_letter(Path::new("E:")), Some('E'));
        assert_eq!(drive_letter(Path::new("/Volumes/E")), None);
        assert_eq!(shell_quote("a b'c"), "'a b'\\''c'");
        assert_eq!(shell_quote("/a/b"), "/a/b");
    }

    #[test]
    fn window_sizes() {
        assert!(valid_window_size("1280x720"));
        assert!(valid_window_size("2560x1440"));
        assert!(!valid_window_size(""));
        assert!(!valid_window_size("10x10"));
        assert!(!valid_window_size("1280x720x1"));
    }

    #[test]
    fn rebuild_script_steps() {
        let all = rebuild_script(&[]);
        assert_eq!(all, "set -e\necho '==> setup'\ntools/setup.sh\nBUILD_ALLOW_FULL=1 exec tools/rebuild.sh stage decrypt analyse lift spu build\n");
        let some = rebuild_script(&["build".into()]);
        assert!(some.contains("if [ ! -x .venv/bin/python ]; then") && some.ends_with("BUILD_ALLOW_FULL=1 exec tools/rebuild.sh build\n"));
        let setup = rebuild_script(&["setup".into()]);
        assert!(setup.contains("tools/setup.sh\n") && setup.ends_with("==> done'\n"));
    }

    #[test]
    fn install_script_clones_or_reuses() {
        let t = tmp("install-script");
        // no checkout: clone into src at the tag, then tools/install.sh
        let src = t.join("Application Support/src");
        let sc = install_script(None, &src, "https://example.org/r.git", "v1.2.3");
        assert!(sc.contains("==> sources"));
        assert!(sc.contains(&format!("SRC='{}'\n", src.display())));
        assert!(sc.contains("git clone --quiet https://example.org/r.git \"$SRC.partial\""));
        assert!(sc.contains("checkout --quiet -B \"$br\" v1.2.3"));
        assert!(sc.ends_with("cd \"$SRC\"\nexec tools/install.sh\n"));
        // a checkout from before tools/install.sh: the full rebuild
        let repo = t.join("repo");
        touch(&repo.join("tools/rebuild.sh"));
        let old = install_script(Some(&repo), &src, "u", "v1");
        assert!(!old.contains("==> sources") && old.contains("exec tools/rebuild.sh stage decrypt analyse lift spu build"));
        touch(&repo.join("tools/install.sh"));
        let new = install_script(Some(&repo), &src, "u", "v1");
        assert_eq!(new, format!("set -e\ncd {}\nexec tools/install.sh\n", shell_quote(&s(&repo))));
        let _ = fs::remove_dir_all(&t);
    }

    #[test]
    fn need_gb_is_zero_once_built() {
        assert_eq!(need_gb(None), INSTALL_NEED_GB);
        let b = Builds { ok: true, ..Default::default() };
        assert_eq!(need_gb(Some(&b)), 0);
    }
}

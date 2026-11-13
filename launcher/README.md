# Launcher (Tauri)

A desktop app that takes the player from their own Afro Samurai disc
(BLUS30264) to "Play" without a terminal: it finds the port's build and the
disc, mounts the disc image, keeps the game's settings, runs the install or
the rebuild with a progress bar, and starts the game the way `tools/run.sh`
does. It ships no game code or files; the cover art on the home screen is
read from the player's mounted disc (`PS3_GAME/ICON0.PNG`, `PIC1.PNG`).

It is modelled on the launchers of the
[inFAMOUS 2](https://github.com/erikomis/infamous2-recomp/tree/main/launcher)
and [Resistance](https://github.com/erikomis/resistance-recomp/tree/main/launcher)
ports and keeps their look, their layout and most of their code (Play
shortcut, Add to Steam, the Dock icon from the player's copy, debug mode, the
saves manager, the log viewer), adapted to this port and to what its runtime
offers. Made for Apple-silicon macOS, like the game.

## What the player does

1. **Home > Install the game** (shown until the game is built): choose the
   disc image (`.iso`) and press the button. The app checks the free disk
   space (6 GB asked), opens Apple's Command Line Tools installer when they
   are missing (`xcode-select --install`) and waits for it, installs
   Homebrew's build tools when some are missing (`brew install cmake ninja
   sdl2 glslang spirv-cross spirv-tools ffmpeg python@3.12`; without Homebrew
   it opens brew.sh), clones the sources into
   `~/Library/Application Support/afro-samurai-recomp/src` when there is no
   checkout (at the tag `v<launcher version>` when it exists, on the default
   branch so Updates can pull; `AFRO_SOURCES_URL` overrides the repository),
   then runs `tools/install.sh`: `tools/setup.sh` (the toolkit, the venv, the
   disc SELF keys), stage, decrypt, analyse, lift, SPU, build. One progress
   bar with the stage's name and a time estimate; **Cancel**; on a failure a
   plain-language reason by stage with **Copy log**, **Show the log file**
   (`<data>/install.log`) and **Report a problem**. Pressing it again after a
   quit or a failure continues where it stopped: analyse, lift and SPU run
   only when their inputs changed, the build is incremental. Quitting the app
   stops the install (its own process group, by pid).
2. **Game > Disc**: **Mount the disc image…** picks the `.iso` and attaches
   it read-only (`hdiutil attach -readonly`); the image is remembered and
   mounted again on Play when it is not (setting "Mount the image on Play").
   A folder holding `PS3_GAME/` works too, and without either the first
   BLUS30264 volume under `/Volumes` is used.
3. **Play** on Home.

## Screens

**Settings are saved the moment they change** (`launcher.json`) and apply the
next time the game starts. Every setting is passed only when it differs from
the port's default: with the defaults, Play starts the game exactly as
`tools/run.sh` does. A section whose feature the build lacks (the build's
executable is searched for the variable's name) says to rebuild instead.

- **Home / Início**: a hero with the player's own disc art when it is
  mounted, else a plain styled title; "BLUS30264 · version 01.00" and the
  build date; **Play** (Stop while the game runs; **Capture this frame** in
  debug mode). A bar for whatever blocks Play, with its fix: no build (to
  Install / rebuild), no disc (mount an image or pick a folder), a disc that
  is not BLUS30264. A bar when Updates has news. Cards: **Quick settings**
  (window size, fullscreen, fps counter, keep the Mac awake), **Last save**,
  **Status** (state, disc, build, play folder; links to the log, Updates,
  **Play shortcut on the Desktop**, **Add to Steam**).
- **Graphics / Gráficos**: the window: the display mode (window, macOS's
  fullscreen or borderless: `PS3_WINDOW_MODE=fullscreen|borderless`; Home's
  quick "Fullscreen" switches between the window and fullscreen), its size
  (`PS3RECOMP_METAL_WINDOW`; the picture keeps 16:9 with black bars), the
  screen (`PS3_DISPLAY`, the names from `NSScreen`), vsync (`PS3_VSYNC=0`
  when off), "Your disc's icon in the Dock" (and "Use as the app's Finder
  icon"); image quality: a profile, the internal resolution
  (`PS3_RES_SCALE` 1.5, 2 or 3; experimental), MetalFX upscaling
  (`PS3_UPSCALE=metalfx`), FXAA or SMAA (`PS3_AA`), sharpening
  (`PS3_SHARPEN`), anisotropic filtering (`PS3_ANISO`), a gamma override
  (`PS3_DISPLAY_GAMMA`); filters: vibrance and contrast (`PS3_VIBRANCE`,
  `PS3_CONTRAST`), vignette and film grain (`PS3_VIGNETTE`, `PS3_GRAIN`), a
  colour LUT file in ReShade's PNG layout and its strength (`PS3_LUT`,
  `PS3_LUT_STRENGTH`). The profiles: **Original (PS3)** is everything off,
  **Balanced** FXAA, 4× and 20% sharpening at the native resolution,
  **Quality** SMAA, 16×, 35% sharpening, internal resolution 1.5 and MetalFX.
- **Performance / Desempenho**: the **in-game menu** (on; off passes
  `PS3_OVERLAY=0`) and its performance panel from the start
  (`PS3_OVERLAY_HUD=1`). With it on, Play passes the menu's accent
  (`PS3_OVERLAY_ACCENT`, the port's red), the launcher's language
  (`PS3_OVERLAY_LANG`), `PS3_OVERLAY_INI=<data>/overlay.ini` (what the
  player changes in the menu; an option the launcher passes wins over it)
  and `PS3_INPUT_MAP_SAVE=<data>/input.toml` (a binding changed in the menu
  becomes the player's controls file, the one the Controls page edits).
  Then the skinning jobs on four threads (on, as
  `port/main.cpp` sets it; off passes `SPURS_JC_JOBLIST_PAR=0`), high
  priority for the game's threads (on; off passes `PS3_QOS=0`), the shader
  translation cache (`PS3RECOMP_MSL_CACHE=<data>/shader-cache`, on, with its
  size and **Clear**), the frame clock (`PS3_FRAMECLOCK_US`), the frame-rate
  counter (`PS3RECOMP_METAL_FPS=1`), keep the Mac awake (`caffeinate -i`, on).
- **Game / Jogo**: the disc (path, the image it is mounted from, mount,
  folder, find, eject, mount on Play); the console language (`PS3_LANG`,
  empty = the Mac's); movies (`PS3RECOMP_FFMPEG=0` when off, and the
  `PS3RECOMP_FFMPEG_AVCODEC` / `_AVUTIL` paths, Homebrew's by default: the
  runtime dlopens them by name, which a GUI app's search path misses); audio:
  the volume (`PS3_AUDIO_VOLUME`, passed below 100), mute
  (`PS3_AUDIO_MUTE=1`), the output device, picked from the
  outputs the Mac has now (`PS3_AUDIO_DEVICE`) and the ITU stereo fold-down
  (`PS3_AUDIO_DOWNMIX=itu`); the play folder; the **saves manager** (below).
- **Controls / Controles**: the bindings, one row per PS3 control and a
  column each for the keyboard, the mouse buttons and the controller: click
  a cell, then press the key, mouse button (or scroll) or controller button;
  × unbinds, Esc cancels, and a controller the web view does not see can be
  bound from a list. Pressing anything lights the rows it drives, and the
  controller test shows the pad's name and both sticks. The default is the
  port's own layout, `port/input.toml`: the keyboard as a pad (Z ✕, X ○,
  A □, S △, Q L1, W R1, 1 L2, 2 R2, Return START, Tab SELECT, the arrows on
  both the d-pad and the left stick, nothing on L3, R3 and the right stick),
  no mouse buttons, SDL's standard mapping for a controller. A change is
  written at once to `<data>/input.toml` (`PS3_INPUT_MAP`; format in the
  toolkit's `libs/input/pad_input_map.h`), always as the full layout: every
  control of the three sections (`""` = unbound) and a `[mouse]` section
  from the mouse settings, because a control the file leaves out would get
  the runtime's first-person default, not the port's. **Restore defaults**
  moves that file to `input.toml.bak`, so the port's applies again. Below
  the table: what drives the game (`PS3_INPUT_MODE=controller|keyboard`),
  the mouse camera (off in this port; `PS3_MOUSE`, `_SENS`, `_INVERT_Y`,
  `_INVERT_X`, `_DEADZONE`, `_CAPTURE`; the mouse buttons count only while
  it is on), controller vibration (`PS3_RUMBLE=0`,
  `PS3_RUMBLE_STRENGTH`) and the **DualSense**: the adaptive triggers'
  resistance (`PS3_PAD_ADAPTIVE`, 0 = free), the light bar (`PS3_PAD_LIGHT`,
  the port's gold by default; off = the controller's own colour) with its
  red glow on hits (`PS3_PAD_LIGHT_HIT=0`), and the gyro on the camera
  (`PS3_PAD_GYRO`, off by default; `_MODE`, `_INVERT_Y`).
- **Advanced / Avançado**, five tabs:
  - **Install / rebuild**: 1. the requirements (the Command Line Tools,
    Homebrew and its build tools, with **Install the build tools**; the disc,
    the toolkit, the venv, the keys, the disk space, the build); 2. the port
    folder (below); 3. the rebuild: the steps (setup, stage, decrypt,
    analyse, lift, SPU, build; setup also runs whenever the venv is missing),
    JOBS, a progress bar with a time estimate, the step list and the log;
    **Cancel** stops it and everything it started. A rebuild started here may
    compile every lifted chunk (`BUILD_ALLOW_FULL=1`); `tools/build.sh` still
    runs one build at a time on this Mac and stops on low memory or disk.
  - **Updates**: the port checkout (branch, commit, `git fetch`, commits
    behind its upstream, **Get the new sources** = `git pull --ff-only`,
    refused with local changes) and "the sources changed after the build"
    (the newest commit touching `port/`, `patches/` or `tools/` against the
    build's time) with **Rebuild the game**; the launcher itself (see
    [App updates](#app-updates)). Once per session on start when **Check for
    updates when the app opens** is on; `AFRO_NO_UPDATE_CHECK=1` turns it off.
  - **Game log**: the end of `last.log` (or the debug session's log), at
    most 512 KB, followed every 2 s, a filter, warnings and errors only,
    copy, show in Finder.
  - **Debug mode**: see below.
  - **Environment**: extra variables (passed last, so they win; the main
    README's environment table), quiet log (`PS3_VERBOSE=0`), the equivalent
    command line, Restore defaults.
- UI in Brazilian Portuguese (default) and English (sidebar), English as the
  fallback for a missing string.

## How it fits together

```
launcher (Rust, src-tauri/src/lib.rs)
 ├─ Install → zsh in <data>: [git clone → <data>/src,] tools/install.sh
 │            (setup, stage, decrypt … build; resumable; its output also in
 │            <data>/install.log)
 ├─ Rebuild → zsh in the port folder: [tools/setup.sh,]
 │            BUILD_ALLOW_FULL=1 tools/rebuild.sh <steps>   ("==> step" lines: src/progress.ts)
 └─ Play    → cd <play folder> &&
              [caffeinate -i] build/BLUS30264/AfroSamuraiRecomp game/EBOOT.elf  > last.log 2>&1
```

- **The port folder** is a checkout with `build/BLUS30264/AfroSamuraiRecomp`
  and `game/EBOOT.elf`: this repository after `tools/rebuild.sh`. Empty
  setting = found on its own: `AFRO_ROOT`, the repository the launcher was
  built from, `<data>/src` (where Install the game clones the sources), then
  any folder one level under `~/Desktop`, `~/Developer`, `~/Documents`,
  `~/Projects`, `~/src`, `~/code`, `~/git` or `~` that has
  `build/BLUS30264`, complete builds and the newest first. Its toolkit is
  `../ps3recomp-afro` next to it (`PS3RECOMP_DIR`), as `tools/env.sh` has it.
- **Play's environment** is `tools/run.sh`'s: the disc's `PS3_GAME` and
  `PS3_DISC.SFB` linked into `<play>/dev_hdd0/disc/BLUS30264`
  (`PS3_VFS_ROOT`; the links follow the disc, a real file in the way is
  never touched), `PS3_HDD0_ROOT` / `PS3_HDD1_ROOT` (`<play>/dev_hdd0`,
  `dev_hdd1`), `PS3_GAMEDATA_ROOT=<play>`, `PS3_BOOT_PATH`, `PS3_TITLE`,
  `AUDIO_ELASTIC=1`, RPCS3's `dev_flash` as `PS3_DEV_FLASH` when it is
  installed (read-only), the FFmpeg paths, and `PS3_APP_ICON`, the running
  game's Dock icon: the square one drawn from the player's own `ICON0.PNG`
  (the same as "Your disc's icon in the Dock"), else this app's icon, so the
  game never shows the generic executable icon; then only the settings that
  differ from the runtime's defaults, then the extra variables. The controls
  file is the one setting always passed: `PS3_INPUT_MAP` is the player's
  `<data>/input.toml` when it exists, else the port folder's
  `port/input.toml` (as `tools/run.sh`), since without a file the runtime
  falls back to the toolkit's first-person layout. The mouse camera is on in
  the runtime and off here, so off always passes `PS3_MOUSE=0`; on passes
  `PS3_MOUSE=1` and every mouse variable (they win over the file's
  `[mouse]`), and the player's file is written if there is none yet (the
  port's layout, mouse buttons unbound) and kept in step with the mouse
  settings. A `launcher.json` from before these options still loads:
  `fullscreen: true` becomes the fullscreen display mode, `keyboard: false`
  the "controller only" input mode. It drops an
  inherited `PS3RECOMP_METAL_HEADLESS`, `PS3RECOMP_HOST_SURFACES` and
  `PS3RECOMP_WINDOW_STATE`. A GUI app starts with a bare `PATH`: Homebrew's
  folders go in front.
- **Stop** sends SIGTERM to the process group Play started: the port takes
  it as the console's Quit Game (`REQUEST_EXITGAME`, then exit after
  `PS3_QUIT_GRACE_MS`, 3 s by default, never in the middle of a save), and
  SIGKILL goes to that group 8 s later if it is still there. Processes are
  never looked up by name to be signalled; an `AfroSamuraiRecomp` started
  elsewhere (`tools/run.sh`) is only shown ("running, started outside the
  launcher") and keeps the saves manager read-only.
- Settings live in
  `~/Library/Application Support/afro-samurai-recomp/launcher.json`, next to
  the default play folder and the disc keys `tools/setup.sh` writes
  (`keys/`). `tools/run.sh` keeps its saves in the repository's `rpcs3/`
  folder instead: choose that folder as the play folder to share them.

## Saves

`src-tauri/src/saves.rs`. Afro Samurai keeps one cellSaveData slot, a folder
named after the title, `BLUS30264` (PARAM.SFO, ICON0.PNG, PIC1.PNG and the
game's `AFRO_0.SAV`), under `<play>/dev_hdd0/home/00000001/savedata` in the
port and RPCS3's `dev_hdd0/home/<user>/savedata`.

Both sides are listed side by side, with "same as / differs from the other
side". Back up one or all, copy either way, delete, export to `.zip`, import
a `.zip` (an RPCS3 `savedata` zipped works), restore a backup. Backups go to
`<data>/save-backups/<UTC time>-<side>-<reason>/` with `backup.json`; one is
made before every Play (skipped when nothing changed; the newest N kept,
default 10, 0 = off) and before every restore, copy, import and delete. A
slot is built next to the old one and swapped in by renaming. The game's
files are copied byte for byte; crossing sides converts only the metadata:
the port's `PARAM.PFD` (the SECUREFILE names, one per line) becomes RPCS3's
`*<file>` keys and `RPCS3_BLIST` in PARAM.SFO, and back (RPCS3's own
PARAM.SFO is kept when it already has the slot). Everything refuses while
the game runs.

## Debug mode

`src-tauri/src/report.rs`, Advanced > Debug mode. **Play in debug mode**
starts the game as Play does, with its log and dumps in a new folder
`<data>/reports/<UTC time>/`:

| Variable | What it adds |
|---|---|
| `PS3RECOMP_METAL_FRAME_DUMP=<dir>/frame.ppm` | the whole frame every 120 presents |
| `PS3RECOMP_METAL_FPS=1` | the frame rate and frame times (`[RSX FPS]` lines) |
| `LV2_ERRDBG=1` | each failing syscall, once per (number, error) |
| `AUDIO_RATE=1` | every 5 s: mixed audio vs real time |
| `PS3_VERBOSE=1` | the runtime's per-event lines |

**Capture this frame** keeps the latest whole frame as `shots/shot<n>.ppm`;
`events.tsv` indexes the session (start, a tick every 2 s with the log's
size, captures, stop, exit), `system.txt` has the Mac, macOS, GPU, the disc,
the build and the command and variables. After a crash the macOS crash
report (`AfroSamuraiRecomp-*.ips`) is copied as `crash.ips`. **Analyse**
groups the log's error and warning lines by message (numbers masked), most
frequent first, with the frame rate from the `[RSX FPS]` lines
(`triage.md`, `triage.json`). **Create .zip** packs the folder (PPM as PNG,
the home folder's path as `~`, the optional description as `notes.txt`) to
attach to an issue. The draw engine's per-pass capture
(`RSX_ENGINE_PASS_DUMP`) has no trigger file in this toolkit, so it stays a
terminal tool.

## The Play shortcut and Add to Steam

As in the inFAMOUS 2 launcher (`play_shortcut.rs`, `steam.rs`, `vdf.rs`,
`steam_art.rs`): `--play` on the command line starts the game once the
window is up (`open -b io.github.erikomis.afrosamurairecomp --args --play`);
Home > Status > **Play shortcut on the Desktop** writes `~/Desktop/Play Afro
Samurai.app` running that line. **Add to Steam** (Steam installed, the
packaged app) writes a non-Steam entry "Afro Samurai" into every account's
`shortcuts.vdf` (backed up first, other entries untouched; Steam must be
closed, the app offers to quit it), with library art drawn at that moment
from the player's own `ICON0.PNG` and `PIC1.PNG`; the entry runs `--play
--exit-with-game`, so Steam sees the game exactly as long as it runs.
**Remove from Steam** takes it out again.

## Development

Needs Node 20+, Rust (stable) and the Xcode Command Line Tools.

```sh
cd launcher
npm install
npm run tauri dev        # the app, with the frontend hot-reloaded
npm run tauri build      # .app and .dmg in src-tauri/target/release/bundle/
npm run typecheck        # frontend only
(cd src-tauri && cargo test)
```

Or from the repository root: `tools/rebuild.sh launcher` builds the `.app`
into `build/launcher/` (`CARGO_TARGET_DIR` is honoured).

`tools/rebuild.sh launcher` builds without Cargo's default stripping
(`CARGO_PROFILE_RELEASE_STRIP=false` and `..._BUILD_OVERRIDE_STRIP=false`,
as the Resistance port does): on macOS 27 with Rust 1.94, rustc cannot load
some stripped proc-macro libraries ("can't find crate for `tauri_macros`" or
`phf_macros`). Do the same for a hand-run `tauri build`.

The screenshots come from the built frontend in a plain browser with a fake
backend: `npm run build && npx vite preview --port 4173`, then load the page
with `scripts/ui-mock.js` injected before it (e.g. Playwright's
`page.addInitScript({ path: "scripts/ui-mock.js" })`, at 1280×800);
`window.__UI_MOCK__ = { noBuilds: true }`, `{ noDisc: true }`,
`{ running: true }` or `{ lang: "en" }` in an earlier init script shows
those states.

`scripts/check_bundle.py <.app|.dmg>` fails when a bundle carries anything
from the game (names, PS3 file headers, PowerPC/SPU ELFs, big files). The
build is not code-signed: right-click the app and choose Open the first time
(Gatekeeper).

## App updates

The self-update code of the inFAMOUS 2 launcher is here
(`src-tauri/src/update.rs`, `src/update.ts`), switched off by
`update::SIGNED_UPDATES = false` until the project has an updater signing
key: the app still looks for a newer release (`latest.json` on the
`updater` release, then GitHub's latest release) and shows it with a link
to its page, but does not register the updater plugin (it refuses to start
without a public key) or install anything.

To turn it on: make a key once with `npx tauri signer generate -w <file>`
(keep the password), put the public key, the two endpoints of
`update::ENDPOINTS` and `"requireSignedVersion": true` under
`plugins.updater` in `src-tauri/tauri.conf.json`, set `SIGNED_UPDATES` to
true (the `update::tests` check the config), and keep the private key and
its password as the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` for a release build with
`--config src-tauri/tauri.updater.conf.json`; `scripts/updater_json.py`
writes the manifest (`python3 scripts/test_updater_json.py` tests it).

The look follows the game's own colours: ink-black panels, paper-white text,
blood red as the accent and gold for running states (`src/styles.css`). The
icon is an original placeholder in the same colours, a red sun and a katana
(`scripts/make_icon.py`, then `npx tauri icon src-tauri/icons/app-icon.png`),
no game art or logo. The font is Oswald (SIL Open Font License,
`src/assets/fonts/`).

## Not done (from the inFAMOUS 2 and Resistance launchers)

- **Texture packs**, and the picture options beyond the ones above (**tone
  curve, clarity, built-in LUT presets**): this port's runtime (upstream
  ps3recomp v0.19 plus `patches/`) has no such options yet.
- **Trophies**: the disc has no `PS3_GAME/TROPDIR`.
- **A bundled port**: the app carries no sources; Install the game clones
  them and builds from that checkout.
- Windows and Linux: the game is macOS-only.

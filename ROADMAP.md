# Roadmap

🌐 **English** · [Português (BR)](ROADMAP.pt-BR.md)

Status of the native Afro Samurai (`BLUS30264`) port to macOS/arm64 and what
comes next. Same layout as the inFAMOUS 2 and Resistance roadmaps.
`[x]` = done and verified; `[ ]` = to do; "(in progress)" = being worked on.

## Status at a glance (2026-11-11)

- **Gameplay renders correctly:** boot → menus → New Game or Continue → the
  3D loading screen → the opening cutscene and the combat tutorial, with
  Afro, Ninja Ninja and the village textured and lit, played in a window
  with a controller without a crash. RPCS3 on the same disc was the visual
  reference.
- **Frame pacing fixed:** gameplay runs at 30 frames a second like the PS3
  and RPCS3 (29.5-30.1 measured over 260 s, headless New Game run to the
  combat tutorial; it was ~45-48). The title paces itself with libgcm's
  prepare-flip + flip-immediate scheme; the toolkit's HLE now returns the
  real buffer id from `cellGcmSetPrepareFlip`, runs the flip handler at the
  flip and blocks the RSX on `SEMAPHORE_ACQUIRE` (250 ms bound).
- **In-game audio fixed:** it was slow, smeared and unintelligible in
  windowed play. FMOD's 18 codec/DSP plugins (SPU images 2-19) were run by
  the SPU interpreter (84% of the mixer thread) because the mixer copies
  their text to a different local-store address than the one they were
  lifted at; `tools/lift_fmod_plugins.sh` (run by `rebuild.sh spu`) now
  lifts them as relocatable overlays. Confirmed by ear.
- **Broken textures, two causes fixed:** alpha-to-coverage (`0x1D7C`) was
  unsupported and is now emulated as an alpha test at 0.5; and the draw
  engine mixed the title's 2D blit methods (NV3089, subchannel 5) into the
  3D register file, wiping the alpha-test state, so grass and foliage cards
  drew as dark rectangles. Only 3D subchannels reach the engine now
  (`GCM_ENGINE_3D_SUBCH=1`, the port default).
- **Faster play (2026-11-11):** 26.1 -> 28.9 fps over the same 60 s of a
  headless walk, p95 64 -> 42 ms, p99 83 -> 57 ms, frames over 50 ms 12.4%
  -> 2.0%. The skinning JOBLIST runs on four threads instead of one
  (`SPURS_JC_JOBLIST_PAR=4`), and the FIFO drain spends less CPU per draw.
  Shader translations are kept across runs, closing the window or Ctrl+C
  quits cleanly, and a 10-minute run held its memory steady.
- **Current blocker:** none known. The final look at the courtyard grass is
  pending, and other broken textures may remain unreported.
- **Next:** confirm the grass by eye, look for other rendering errors while
  playing further than the first minutes, and take the last 2% of slow
  frames (the FIFO drain and the skinning jobs).
- **Toolkit:** upstream ps3recomp v0.19.0 (`ae92407b`) plus this port's
  generic, env-gated changes, as one patch
  (`patches/ps3recomp-ae92407b-afro.patch`).

## Done

- [x] The disc inventoried ([`docs/game-manifest.md`](docs/game-manifest.md)):
  `BLUS30264` v01.00, no game update, the engine (Namco Bandai's NTG, not
  UE3), the middleware (Bink, FMOD Ex, NavPower, EDGE), 156 imports from 12
  firmware libraries, 20 embedded SPU images, no PRX of its own.
- [x] `EBOOT.BIN` decrypted without RPCS3 (`tools/decrypt_self.py`, a disc
  SELF with `key_rev 0x0001`).
- [x] EBOOT analysed and lifted (12,869 OPD functions, 32,951 emitted, 16
  translation units); builds and links on macOS/arm64 with Metal.
- [x] The 20 embedded SPU ELFs lifted (`tools/lift_spu.sh`): EDGE ZLIB,
  the FMOD Ex SPURS mixer and 18 FMOD codec/DSP plugins.
- [x] The scripts: `rebuild.sh`, `build.sh` with memory and disk guards (two
  kernel panics from unguarded builds on 2026-11-01), `run.sh` with the ISO
  mounted read-only and the HDD folders kept inside the repository,
  `probe.sh` for headless runs with frame dumps and scripted buttons.
- [x] `sys_mempool` implemented (`try_allocate` / `allocate` / `free` /
  `destroy`, big-endian counts, generation-tagged handles): the game creates
  about 3,800 pools in two minutes.
- [x] The tracked memory mapper (`PS3_MMAPPER_TRACK=1`): a page-owner map,
  real map / unmap / free and next-fit per reservation; the cursor no longer
  overlaps the game's heap pages and its pools (the `memset` over live list
  nodes, then the `abort()`).
- [x] Six ascending address reservations, 512 MB + 5 × 256 MB
  (`PS3_MAPPER_WIDE=1`): two used to come back `ENOMEM`, and the game routes
  `free()` by reservation range.
- [x] FMOD's SPU plugin overlays run interpreted where no lifted body exists
  (`SPU_INTERP_UNLIFTED`; now lifted, see the audio item below): the event thread no longer waits for good on the
  mixer while holding the lwmutex the main thread needs.
- [x] First save: a negative stat-callback result answers `ERROR_CBRESULT`,
  as firmware does (`SAVEDATA_CBRESULT_STRICT=1`); the "save data appears to
  be damaged" stop is gone, and `AFRO_0.SAV` is written and loaded back.
- [x] `cellKbRead` (NID `0xFF0A21B7`) answered by `cellKbGetData`; the game
  no longer reads stack garbage as key events.
- [x] `_cellSpursLFQueuePushBody` on the canonical `CellSyncLFQueue` line,
  and the LF-queue lost wake-up in `WAIT_SIGNAL` (both from the Resistance 2
  tree).
- [x] Past the old main-thread null read in `func_002D13B8` (the global
  registry at `[r30-0x7FA0]+4`): the boot now goes on to the terrain loader,
  the save worker, audio and the logo movie.
- [x] The temporary `[afro-memset]` trace taken back out of the lifted code.
- [x] The black frames at boot: the movies open. Bink reads the `cellFs`
  descriptor the game hands it with the raw lv2 syscalls; lv2 read, lseek and
  fstat now fall back to the `cellFs` table.
- [x] The logo and credit movies (`bng.bik`, `two_headbands.bik`,
  `afro_credit.bik`) and the main menu over `background_movie.bik`.
- [x] `PAD_SCRIPT` reaches the game: START through the title, CROSS through
  the menu to New Game.
- [x] The SPU skinning chain: its job binaries dumped (`SPU_DUMP_MISS`) and
  lifted (`tools/lift_jobs.sh`); the walker waits on a closed guard instead
  of giving up after 20 s (`SPURS_JG_WAIT_FOREVER`); only the jobs' own
  events posted (`SPURS_JC_EVENT_FROM_JOB`, the depth-1 queue the render
  thread waits on); the older job-context fields the job reads
  (`SPURS_JOB_CTX_V1`, the halt on its guard and the PUT over the game's
  code); DMA PUTs into the read-only segment dropped.
- [x] The `AFRO_TEXT_RO=1` diagnostic: the game's code read-only after load,
  so a host write into it stops with its caller (it caught the skinning job's
  PUT).
- [x] Jump-table case targets lifted as functions (`tools/augment_functions.py`,
  `tools/jumptables.txt`): the message dispatcher's lost case `0x1F079C` and
  New Game spinning on a NULL; no "unresolved indirect call" in four runs.
- [x] New Game reaches the 3D loading screen.
- [x] The in-game audio: FMOD's 18 codec/DSP plugins lifted as relocatable
  overlays (`tools/lift_fmod_plugins.sh`, run by `rebuild.sh spu`) instead of
  interpreted; the interpreter went from 979 of 1162 mixer-thread samples to
  none, and it sounds right by ear (`SPU_RELOC_OFF=1` for A/B).
- [x] Alpha-to-coverage (`0x1D7C`) emulated as an alpha test at 0.5
  (`PS3RECOMP_ENG_A2C`): grass tufts render as blades, not rectangles.
- [x] 2D blit methods no longer reach the 3D register file
  (`GCM_ENGINE_3D_SUBCH=1`): the alpha-test state survives the title's
  NV3089 blits.

## Next: play it through, and make it fast

### Blockers

- [x] **Black, then exploded geometry after the loading screen.** Three
  causes: vertex programs decompiled as if they started at slot 0 (branching
  programs looped 16,384 times per vertex); the SPURS chain walker skipped
  JOBLIST, so the 5 vertex-skinning jobs per frame never ran and the draws
  read another writer's leftovers from the ring; and the skinning jobs' second
  input pointed at the cache buffer in the older job context. All three fixed
  (2026-11-07).
- [ ] Watch the logs for more "unresolved indirect call" targets and add
  their tables to `tools/jumptables.txt` (or let the scan find them). None
  in a 10-minute run into the village (2026-11-11).
- [x] The two known skinning-chain jobs extracted from the EBOOT at rebuild
  time (`tools/extract_jobs.py`), verified byte-for-byte against runtime dumps.

### The menus, for real

- [ ] The legal and studio screens, the title screen and the menus checked
  against the PS3 (every option, Treasures).
- [ ] Input: a controller through the toolkit's SDL pad, then keyboard and
  mouse.
- [ ] Menu sound and music (FMOD on SPURS), in sync with the movies.

### Into the first fight

- [ ] New Game → the opening cutscene → the first level, controllable.
- [ ] The cel-shaded look and the outlines checked against the PS3: shaders,
  blending, depth/stencil, render targets.
- [ ] Streaming from `ps3.psarc` with EDGE ZLIB on the SPUs, without stalls.

## What is missing to be 100% playable

### Gameplay

- [ ] The whole story played through, level by level: bosses, QTEs, deaths,
  checkpoints, the ending.
- [ ] Saves across sessions (checkpoint, quit, Continue).

### Graphics

- [x] Grass and foliage cut-outs: alpha-to-coverage emulated, and the 2D
  blits kept out of the 3D alpha-test state.
- [ ] The grass in the courtyard confirmed by eye in a windowed session.
- [ ] Look for other broken textures; test whether the texture cache's
  full hash only every 8 frames leaves streamed textures stale
  (`PS3RECOMP_ENG_TEXHASH_FULL=1`, unconfirmed).
- [ ] Every rendering bug found in play fixed, each compared with a capture
  from the PS3 in the same scene.

### Performance

- [x] The flip-pacing fix: real prepare-flip ids, the flip handler run at
  the flip, the RSX waiting on the display buffer (`GCM_PREPAREFLIP_FIFO=2`,
  `GCM_FLIPCB_ONTICK=1`, `GCM_SEMA_ACQUIRE=1`). Gameplay at 29.5-30.1 frames
  a second over 260 s, no acquire timed out.
- [x] The in-game audio checked by ear: it sounds right (see the FMOD
  plugin fix above). A comparison against RPCS3 and the optional
  `PS3_AUDIO_DOWNMIX=itu` fold-down are left for later.
- [x] Frame times measured: `tools/frametimes.py` reads a `PS3_FTRACE`
  timeline and prints p50/p95/p99 and the share of slow frames.
- [x] The skinning JOBLIST on four threads (`SPURS_JC_JOBLIST_PAR=4`): it
  kept one host thread 94% busy and the render thread waited on it.
- [x] Less CPU per draw in the FIFO drain (direct-map vertex dedupe, a
  `CMP32` loop, a four-lane texture hash): 17.8 -> 14.3 ms a frame.
- [ ] A steady 30 fps with even frame pacing (the game's target on PS3)
  (in progress): 28.9 fps, p95 42 ms, p99 57 ms, 2% of frames over 50 ms
  over 60 s of a headless walk. Next costs: the drain's CPU vertex fetch
  and the skinning job itself; streaming not profiled yet.
- [x] The shader translation cache kept across runs (`run.sh` set a
  variable nothing read; now `PS3RECOMP_MSL_CACHE=cache/msl`): the worst
  frame of the first minutes 481 -> 261 ms.
- [ ] Background pipeline builds from the other ports, so a first-seen
  shader does not hitch even once.

### Stability

- [ ] Long sessions without a hang or a leak (the game churns thousands of
  mempools; watch that the tracked mapper does not run out of room). Ten
  minutes headless: memory steady at ~600 MB from the fourth minute on;
  an hour of real play is the next check.
- [x] A clean shutdown: closing the window, Ctrl+C and SIGTERM quit like
  Cmd+Q and Quit Game (REQUEST_EXITGAME, exit 0 after
  `PS3_QUIT_GRACE_MS`, never during a save); before, a closed window left
  the game running.

### Rebuild

- [x] The toolkit changes exported as one patch,
  `patches/ps3recomp-ae92407b-afro.patch` (checked to apply to a clean
  `ae92407b`).
- [x] `tools/setup_toolkit.sh` clones v0.19.0 and applies the patch;
  `tools/export_toolkit_patch.sh` turns a toolkit folder back into it
  (new files included).
- [x] Self-contained: `tools/setup.sh` makes its own venv (`.venv`) and
  `tools/fetch_keys.py` writes the APP keys from RPCS3's pinned
  `key_vault.cpp` (the EBOOT it decrypts is byte for byte the old one).

## After it is playable (improvements)

The Enhanced Edition ideas from
[`afro_samurai_ps3_recomp_enhanced.md`](afro_samurai_ps3_recomp_enhanced.md),
each as an option, with the original behaviour kept as the default. Each
complaint from the 2009 reviews has to be reproduced on this build first:
a port bug and a bug of the original game are different things.

### Graphics and resolution

- [ ] Internal resolution: 1080p and 1440p first, 4K only if performance
  allows.
- [ ] SMAA / FXAA, anisotropic filtering and sharpening, without spoiling
  the cel shading and the outlines.
- [ ] Ultrawide, if the camera and the HUD allow it.

### Frame rate

- [ ] 60 fps as an experimental mode, only after mapping what is tied to the
  frame rate (physics, animation, scripts, timers, QTEs, the camera).

### Camera and controls

- [ ] A better camera: distance, collision, no losing Afro behind geometry.
- [ ] Independent X/Y inversion and sensitivity; deadzones and remapping
  for DualSense, DualShock 4 and Xbox.
- [ ] Jump and movement response (input buffering, tolerances).

### Game design (optional)

- [ ] Extra checkpoints that restore health, enemies, scripts and flags
  without softlocks.
- [ ] An optional boss balance (telegraphs, damage, timing).
- [ ] A move list and combo feedback; skippable cutscenes where the state
  after them is safe to set up.

### App to install and play (launcher)

- [x] The desktop launcher from the inFAMOUS 2 and Resistance ports, in the
  game's colours (`launcher/`, 2026-11-11): one-button install from the disc
  (`tools/install.sh`, resumable), the settings this runtime has, saves
  manager, debug mode, Play shortcut, Add to Steam, Stop that quits cleanly,
  the running game's Dock icon from the player's disc (`PS3_APP_ICON`).
- [x] The other launchers' options, in the runtime and in the app
  (2026-11-11): internal resolution (`PS3_RES_SCALE`, experimental: 1.5x
  checked in play, -3% frame rate), MetalFX, vignette, grain and LUTs, window
  modes, screen and vsync, volume and output device, remapping
  (`port/input.toml` keeps this port's layout as the default), mouse camera,
  rumble.
- [ ] Play-test them by hand: 2x and 3x, borderless and a second screen, the
  mouse camera, remapped keys, rumble strength, the volume by ear.
- [ ] Texture packs (`PS3_MODS_DIR`), the one option still missing.
- [ ] Signed in-place app updates (needs an updater key; `launcher/README.md`).

### Other platforms

- [ ] Windows and Linux, after macOS is playable.

### Out of scope for now

- Online features, trophies, and texture or asset packs that would need
  someone else's content.

## Giving back

- [ ] Send the generic toolkit fixes upstream to
  [sp00nznet/ps3recomp](https://github.com/sp00nznet/ps3recomp), each with a
  test: `sys_mempool`, the tracked mapper and the ascending reservations,
  `ERROR_CBRESULT` for a negative stat callback, `cellKbRead`, lv2 read /
  lseek / fstat on a `cellFs` descriptor, the job-chain walker's guard wait
  and job-sent events, the older job-context fields, DMA PUTs into the
  read-only segment dropped.
- [ ] The jump-table case split (`augment_functions.py`) as a lifter option
  upstream, so every port gets it.
- [x] Its own git repository (`afro-samurai-recomp`): the port sources, the
  toolkit patch, README, the license and a `.gitignore` that blocks game
  data, firmware, keys and saves.
- [x] `CREDITS.md` (en + pt-BR).
- [x] The repository published on GitHub:
  [erikomis/afro-samurai-recomp](https://github.com/erikomis/afro-samurai-recomp).

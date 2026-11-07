# Roadmap

🌐 **English** · [Português (BR)](ROADMAP.pt-BR.md)

Status of the native Afro Samurai (`BLUS30264`) port to macOS/arm64 and what
comes next. Same layout as the inFAMOUS 2 and Resistance roadmaps.
`[x]` = done and verified; `[ ]` = to do; "(in progress)" = being worked on.

## Status at a glance (2026-11-07)

- **Gameplay renders correctly:** boot → menus → New Game or Continue → the
  3D loading screen → the opening cutscene and the combat tutorial, with
  Afro, Ninja Ninja and the village textured and lit, played in a window
  with a controller without a crash. RPCS3 on the same disc was the visual
  reference.
- **Next:** the frame rate, and playing further than the first minutes.
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
  (`SPU_INTERP_UNLIFTED`): the event thread no longer waits for good on the
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
  their tables to `tools/jumptables.txt` (or let the scan find them).
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

- [ ] Every rendering bug found in play fixed, each compared with a capture
  from the PS3 in the same scene.

### Performance

- [ ] A steady 30 fps with even frame pacing (the game's target on PS3),
  measured with p95/p99 frame times; profile CPU, SPU, GPU, shader builds and
  streaming.
- [ ] The shader translation cache and background pipeline builds from the
  other ports, so first-seen shaders do not hitch.

### Stability

- [ ] Long sessions without a hang or a leak (the game churns thousands of
  mempools; watch that the tracked mapper does not run out of room).
- [ ] A clean shutdown.

### Rebuild

- [x] The toolkit changes exported as one patch,
  `patches/ps3recomp-ae92407b-afro.patch` (checked to apply to a clean
  `ae92407b`).
- [ ] `tools/setup_toolkit.sh` to clone v0.19.0 and apply the patch, and
  `tools/export_toolkit_patch.sh` to turn a toolkit folder back into it.
- [ ] Self-contained: its own key fetcher and Python venv instead of the
  Deadpool port's.

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

- [ ] The desktop launcher from the Resistance ports: one-click install
  from the disc, settings, saves, play without a terminal.

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
- [ ] `CREDITS.md`, and the repository published on GitHub.

# Afro Samurai — static recompilation for PC (macOS)

🌐 **English** · [Português (BR)](README.pt-BR.md)

A native port of **Afro Samurai** (PS3, disc `BLUS30264`, v01.00) to
**macOS/arm64**, built with static recompilation: the game's PowerPC and SPU
code is translated to C/C++ and compiled into a native executable, with
graphics on Metal. It is not an emulator.

**It reaches gameplay, and the game renders correctly.** From the boot
through the logo movies, the main menu and New Game (or Continue), the first
level plays in a window with a controller: Afro, Ninja Ninja and the village
are drawn textured and lit, and the opening cutscene and the combat tutorial
run. It has been played in a window without a crash; it runs about 1.5x too
fast and the in-game audio sounds wrong, which is what is being fixed now. See [Status](#status).

| | |
| --- | --- |
| [<img src="docs/screenshots/03-gameplay-village.png" width="420" alt="Afro from behind, sword drawn, in the village at dusk">](docs/screenshots/03-gameplay-village.png)<br>Gameplay: Afro in the village | [<img src="docs/screenshots/04-ninja-ninja.png" width="420" alt="Ninja Ninja leaning against a tree by a lit lantern">](docs/screenshots/04-ninja-ninja.png)<br>Ninja Ninja |
| [<img src="docs/screenshots/01-main-menu.png" width="420" alt="Main menu: Continue, New Game, Options, Treasures">](docs/screenshots/01-main-menu.png)<br>The main menu | [<img src="docs/screenshots/02-loading.png" width="420" alt="The 3D loading screen after New Game">](docs/screenshots/02-loading.png)<br>New Game: the loading screen |

Built on [ps3recomp](https://github.com/sp00nznet/ps3recomp), with the layout
and the tools of the author's other ports
([inFAMOUS 2](https://github.com/erikomis/infamous2-recomp),
[Resistance: Fall of Man](https://github.com/erikomis/resistance-recomp),
[Resistance 2](https://github.com/erikomis/resistance2-recomp)).

> [!WARNING]
> **Early work in progress.** The first level is playable, but only its
> start has been played, it runs about 1.5x too fast, and the in-game audio
> sounds wrong. There are no releases: you build the game yourself from your own
> disc. Apple-silicon macOS only. What is missing is in
> [ROADMAP.md](ROADMAP.md).

> **This repository contains no game data, no firmware, no Sony keys and
> nothing translated from them.** Everyone generates all of that locally from
> their own legitimate copy. A fan project, not affiliated with Bandai Namco
> Entertainment, Sony Interactive Entertainment or GONZO.

## Status

As of 2026-11-07: boot → heaps and static constructors → SPURS and the
game's threads → the save is loaded → audio up → the logo and credit movies
(`bng.bik`, `two_headbands.bik`, `afro_credit.bik`) → the main menu over
`background_movie.bik` → New Game or Continue → the 3D loading screen → the
opening cutscene and the combat tutorial, rendered correctly, controlled
with a pad (Xbox Series X and DualSense through SDL). RPCS3 running the same
disc was the reference for what the scene should look like.

The two fixes that took the game from exploded geometry to a correct scene
(2026-11-07):

- **JOBLIST in the SPURS job chains.** The renderer laps its skinning chain
  every frame as [JOB palette] → [JOBLIST of 5 vertex-skinning jobs] → [JOB
  lap notify]. The HLE chain walker treated JOBLIST as a no-op, so the
  skinning jobs never ran and about 100 colour draws a frame read another
  writer's leftovers from the RSX-visible ring: giant stretched triangles.
  The walker now runs a JOBLIST's jobs (`SPURS_JC_JOBLIST=0` skips it).
- **One job-context pointer per input.** Those skinning jobs take two DMA
  inputs, and the older job context they read lists one LS pointer per
  input and then per cache buffer; input 1 pointed at the cache, so the
  characters were skinned from the wrong data (shards in cutscenes).

Earlier the same day, the vertex programs' start slot: branch targets in RSX
vertex microcode are absolute, and decompiling every program as if it
started at slot 0 left branching programs looping 16,384 times per vertex
(0.4–3 s of GPU per frame).

What it took, with the cause of each:

- **Memory pools:** the game creates and destroys thousands of small
  `sys_mempool`s (about 3,800 in two minutes; 16-, 32- and 64-byte classes).
  `sysPrxForUser` now implements `try_allocate` / `allocate` / `free` /
  `destroy`, writes the counts big-endian, and hands out generation-tagged
  handles so a destroyed pool's handle is never reused by mistake
  (`SYS_MEMPOOL_LOG=1` traces it).
- **The memory mapper:** the game places its own 1 MB heap pages with
  `sys_mmapper_map_memory` next to those 256 KB pools. The untracked cursor
  overlapped the two (a block array was `memset` over live list nodes), then
  ran out of address space and the game called `abort()`. The mapper now
  keeps a page-owner map with real map / unmap / free and a next-fit search
  per reservation (`PS3_MMAPPER_TRACK=1`, on by default).
- **Six ascending reservations:** the game reserves 512 MB plus 5 × 256 MB
  of address space, and routes `free()` by which reservation range a pointer
  falls in, so they must come back in ascending order. Two came back
  `ENOMEM`. With `PS3_MAPPER_WIDE=1` (on by default) they ascend and may
  enclose the runtime's own windows, which the tracked mapper never hands out.
- **FMOD's SPU plugins:** the FMOD Ex mixer task loads its DSP and codec
  plugins as overlays and branches into them (LS `0x25A18`). With no lifted
  code there the job ended, and FMOD's event thread then waited for good on
  the mixer's event flag while holding the lwmutex the main thread needed.
  `SPU_INTERP_UNLIFTED` (on by default) interprets what has no lifted body, as
  the Deadpool port does.
- **First save:** with no save yet, the game's stat callback answers
  `ERR_NODATA`; firmware turns that into `ERROR_CBRESULT`, which is how the
  game learns to create one. The toolkit answered `ERROR_NODATA`, and the game
  said its save data "appears to be damaged" and stopped.
  `SAVEDATA_CBRESULT_STRICT=1` (on by default) answers as firmware does; the
  game now writes and reads back `AFRO_0.SAV`.
- **`cellKbRead`:** the game polls the keyboard every frame through
  `cellKbRead` (NID `0xFF0A21B7`, the SDK's name for `cellKbGetData`). It was
  unresolved, the generic path answered `CELL_OK` without writing anything,
  and the game read whatever the stack held as key events.
- **SPURS lock-free queues:** `_cellSpursLFQueuePushBody` is implemented on
  the canonical 128-byte `CellSyncLFQueue` line the SPU consumer reads, and a
  task that parks in `WAIT_SIGNAL` while its queue already holds entries
  resumes instead of sleeping (both ported from the Resistance 2 tree;
  `SPURS_LFQ_NOWAKE=1` turns the second off).
- **The movies (black screen at boot):** the game opens each movie with
  `cellFsOpen` and hands the descriptor to Bink, which seeks and reads it
  with the raw lv2 syscalls (818/802). Those keep their own descriptor table
  and knew nothing of a `cellFs` one, so every movie failed to open and the
  boot sat on black frames after `bng.bik`. lv2 read, lseek and fstat now fall
  back to the `cellFs` table.
- **The skinning job chain (frozen after New Game):** the render thread
  waits for an event from the NTG SPU skinning chain, and nothing walked it.
  The job binaries are raw blobs, not SPU ELFs, so they were never lifted
  ("dispatch MISS"); `tools/lift_jobs.sh` lifts them. The chain is kicked at
  boot and sits on its guard until a game starts, and the walker's 20 s
  give-up ended it (`SPURS_JG_WAIT_FOREVER=1`). The ring's last job sends the
  one event per lap the render thread waits for, on a queue of depth 1, and
  a synthetic event per job filled it first (`SPURS_JC_EVENT_FROM_JOB=1`).
  And the job reads its buffer counts and DMA tag past the 48-byte job
  context (an older layout); unset, it halted on its own guard, or PUT its
  whole local store over the game's code at `0x10000`
  (`SPURS_JOB_CTX_V1=1`). A DMA PUT into the executable's read-only segment
  is now dropped, as it faults on hardware.
- **Jump tables (New Game spinning on a NULL):** the lifter resolves a
  `switch` only over the cases inside the function it lifted, and cases past
  where it cut a large function went through the indirect-call fallback,
  which needs a lifted function at the case address. The engine's message
  dispatcher (`func_001EA288`, 4,191 cases) lost case `0x1F079C` that way.
  `tools/augment_functions.py` splits the function list at every case target
  of the tables in `tools/jumptables.txt` before lifting.

Known issues, being worked on:

- **Runs about 1.5x too fast, and the in-game audio sounds wrong.** In
  gameplay the title prepares ~45-48 frames a second in headless runs; the
  PS3 (and RPCS3) run it at 30, so the game logic runs ~1.5x real speed.
  The audio is likely the same pacing problem, not yet confirmed. Cause
  found, fix in progress: the title paces itself with libgcm's prepare-flip
  plus flip-immediate scheme (the buffer id returned by
  `cellGcmSetPrepareFlip`, a flip handler that frees the display buffer the
  RSX waits on), and the toolkit's HLE completed the flip at call time, so
  nothing held the render loop (see the ROADMAP).
- **Only the start of the first level has been played**; the rest of the
  game is untested.
- **Keyboard and mouse** have not been tested; a controller works.
- One headless run stalled once while loading the level (after
  `LoadRenderThreadProc` exited); not reproduced since, not investigated.

## How it works

1. **Decrypt** your disc's `EBOOT.BIN` (a disc SELF, `key_rev 0x0001`, type
   APP) into `game/EBOOT.elf` with `tools/decrypt_self.py` and your own key
   file. RPCS3 is not needed.
2. **Analyse and lift** it with the toolkit's `ppu_loader.py` and
   `ppu_lifter.py`: 12,869 functions in the OPD table, 32,951 emitted by the
   lifter, as C++ in `recompiled/BLUS30264/` (16 translation units). Before
   lifting, `tools/augment_functions.py` adds the case targets of the jump
   tables in `tools/jumptables.txt` to the function list.
3. **SPU code:** the 20 SPU ELFs embedded in the EBOOT are cut out by the
   toolkit's `extract_spu_images.py` and lifted by `tools/lift_spu.sh` into
   `port/spu/`: EDGE ZLIB inflate (`spu_0000`), the FMOD Ex SPURS mixer
   (`spu_0001`, lifted with every call's return as an entry, so the plugins
   return into lifted code) and 18 FMOD codec and DSP plugins
   (`spu_0002`–`spu_0019`). The SPURS job binaries of the skinning chain are
   not ELFs: `tools/extract_jobs.py` extracts them from the EBOOT and
   `tools/lift_jobs.sh` lifts them (the 17,552-byte skinning job `8B2B4776E3A079D8` and the
   112-byte job beside it, `37DA975A7C220171`).
4. **Build** with CMake/Ninja against the toolkit: the runtime (LV2
   syscalls, HLE firmware libraries, SPURS, the Metal RSX backend) plus the
   lifted code, into `build/BLUS30264/AfroSamuraiRecomp`.

The game is Namco Bandai's own **NTG** engine (paths `nhi/ntg/...`), not
Unreal Engine 3: scripts and configuration in `.ini` and `.ntg` files inside
`ps3.psarc` (2.7 GB, all of the game's content), Bink movies, FMOD Ex audio,
NavPower for AI navigation and EDGE 1.2.3.0. It has no PRX of its own and
imports 156 firmware functions from 12 libraries; it saves through
`cellSaveData` and keeps a cache in `/dev_hdd0/game/ntg_cache` and
`/dev_hdd1`. The full inventory is in
[`docs/game-manifest.md`](docs/game-manifest.md).

The toolkit is upstream ps3recomp **v0.19.0** (`ae92407b`) plus this port's
changes (the memory pools, the tracked mapper, the first-save answer,
`cellKbRead`, the SPURS lock-free queue), which are generic and env-gated,
kept as one patch:
[`patches/ps3recomp-ae92407b-afro.patch`](patches/). For now the build uses
a checkout with the patch applied next to this repository
(`../ps3recomp-afro`); a script that clones the toolkit and applies it is on
the [roadmap](ROADMAP.md).

## Building from source

### What you need

- An Apple-silicon Mac (macOS with Metal). Each lifted chunk is tens of MB
  of C++ and clang takes 3–4 GB on one; `tools/build.sh` keeps the build to
  two compiles at a time and watches memory and disk.
- Your own **Afro Samurai disc, `BLUS30264`**, dumped to an ISO. Put it
  (or a link to its folder) at `game_iso/`; it is mounted read-only and never
  written to.
- A key file for the disc SELF (`AFRO_KEYS`; for now the one the Deadpool
  port's `tools/fetch_keys.py` writes).
- The ps3recomp toolkit at `../ps3recomp-afro` (`PS3RECOMP_DIR` changes
  it): upstream `ae92407b` with `patches/ps3recomp-ae92407b-afro.patch`
  applied (`git apply`).
- Xcode Command Line Tools and Homebrew:

```sh
brew install cmake ninja sdl2 glslang spirv-cross spirv-tools ffmpeg python@3.12
```

### Building

```sh
tools/rebuild.sh            # stage, decrypt, analyse, lift, spu, build
```

`tools/rebuild.sh` runs these steps (run any of them alone by name, e.g.
`tools/rebuild.sh spu build`):

| Step | What it does |
| --- | --- |
| `stage` | mounts the ISO read-only (`tools/mount.sh`) and copies `EBOOT.BIN` and `PARAM.SFO` into `game/` |
| `decrypt` | `tools/decrypt_self.py` with `$AFRO_KEYS` → `game/EBOOT.elf` |
| `analyse` | `ppu_loader.py` → `analysis/BLUS30264/` |
| `lift` | `ppu_lifter.py` → `recompiled/BLUS30264/` (`CHUNKS`, default 12; `JOBS` limits parallelism) |
| `spu` | `extract_spu_images.py` and `tools/lift_spu.sh` → `port/spu/` |
| `build` | CMake (once) + `tools/build.sh` → `build/BLUS30264/AfroSamuraiRecomp` |

The `spu` step also extracts the two known raw skinning-chain jobs directly
from the decrypted EBOOT, validates their SHA-256 and runtime fingerprints,
and lifts them. No capture run is needed. To run this alone, use
`tools/lift_jobs.sh`. Additional jobs can still be captured with `SPU_DUMP_MISS`.

The first build compiles a lot of generated code and takes a while. Always
build through `tools/build.sh`: it refuses to start while another Ninja is
running (also another port's), stops if memory runs low (exit 99) and asks
for `BUILD_ALLOW_FULL=1` before rebuilding many lifted chunks at once
(`tools/build.sh --check` says what would be rebuilt; `BUILD_JOBS` sets the
parallel compiles, default 2). The tools need Python 3.10 or newer
(`PY=<python>` picks one). Paths are in [`tools/env.sh`](tools/env.sh).

## Playing

There is nothing to play past the menus yet. To run it:

```sh
tools/run.sh
```

It mounts the ISO, links the disc's `PS3_GAME` into the VFS root and keeps
`/dev_hdd0` and `/dev_hdd1` in this repository's own `rpcs3/` folder, never
in RPCS3's install, so a test run cannot touch your RPCS3 saves. Extra
arguments are environment overrides: `tools/run.sh PS3_VERBOSE=1`.

### Environment

| Variable | Effect |
| --- | --- |
| `PS3RECOMP_DIR` | the toolkit checkout (default `../ps3recomp-afro`) |
| `AFRO_KEYS` | the key file `decrypt_self.py` reads |
| `AFRO_ISO_DIR`, `AFRO_MOUNT` | where the ISO is (`game_iso/`) and where it is mounted (`/Volumes/AFRO_SAMURAI`) |
| `PS3_HDD0_ROOT`, `PS3_HDD1_ROOT` | the game's HDD folders (`rpcs3/dev_hdd0`, `rpcs3/dev_hdd1`) |
| `PS3_VERBOSE=1` | the full runtime log (off in `run.sh`) |
| `PS3_QOS=0` | leaves macOS's default thread priority (by default the game's threads run as user-interactive, off the efficiency cores) |
| `PS3_MMAPPER_TRACK=0` | the memory mapper back to the untracked cursor (brings back the `abort()`) |
| `PS3_MAPPER_WIDE=0` | the six reservations back to the narrow window (two `ENOMEM`) |
| `PS3_MMAPPER_FIRSTFIT=1` / `PS3_MMAPPER_ZERO=1` | first-fit instead of next-fit (measured worse) / zero pages on map |
| `SAVEDATA_CBRESULT_STRICT=0` | `ERR_NODATA` back to `ERROR_NODATA` (the "damaged save" stop) |
| `SPU_INTERP_UNLIFTED` | interpret SPU code with no lifted body (set by `port/main.cpp`; no off switch yet) |
| `SPURS_LFQ_NOWAKE=1` | turns off the LF-queue wake fix (for comparison) |
| `SPURS_JG_WAIT_FOREVER=0` | the job-chain walker gives up on a closed guard after 20 s again (the skinning chain dies before New Game) |
| `SPURS_JC_EVENT_FROM_JOB=0` | a synthetic event per job again (the render thread's event is dropped) |
| `SPURS_JOB_CTX_V1` | fills the older job-context fields the skinning job reads (set by `port/main.cpp`; no off switch yet) |
| `SPU_DUMP_MISS=<dir>` | writes every SPU job binary with no lifted code to `<dir>` (for `tools/lift_jobs.sh`) |
| `SYS_MEMPOOL_LOG=1`, `PS3_MEMTRACE=1`, `PS3_LFQ_LOG=<n>` | trace mempools / address reservations / the first n LF-queue pushes |
| `AFRO_TEXT_RO=1` | makes the game's code read-only after load, so a host write into it (an HLE copy, an SPU DMA, a file read) stops with its caller; `AFRO_TEXT_RO_CONT=1` reports each writer once and goes on |
| `AFRO_WRAP=<program>` | runs the game under a wrapper (a debugger script) |

The seven defaults above (`PS3_MMAPPER_TRACK`, `PS3_MAPPER_WIDE`,
`SPU_INTERP_UNLIFTED`, `SAVEDATA_CBRESULT_STRICT` and the three `SPURS_*`
job switches) are set in `afro_defaults()` in
[`port/main.cpp`](port/main.cpp) unless they are already in the environment.

### Diagnostics

`tools/probe.sh <label> <seconds> [VAR=value ...]` runs the build headless
with a time limit and writes `cache/probe/<label>.log` and a frame dump,
`<label>.ppm`, every 120 presents. `PROBE_SNAP=<s>` keeps a copy of the
latest dump every `s` seconds, so a run shows a sequence rather than its last
frame. Buttons come from a script, `PAD_SCRIPT="<sec>:<mask>,..."` (START
`0x0008`, UP `0x0010`, DOWN `0x0040`, CROSS `0x4000`, CIRCLE `0x2000`, SQUARE
`0x8000`, TRIANGLE `0x1000`; `PAD_CLOCK=flips` counts flips instead of
seconds). It stops only the process it started.

```sh
PROBE_SNAP=5 tools/probe.sh boot 80
```

Into New Game, as tested (START every second from 85 to 100 s through the
movies and the title, then CROSS every 2 s from 101 to 145 s through the
menu; the timing can miss, run it again if it does):

```sh
B=""; for t in $(seq 85 100); do B="$B$t:0x0008,"; done
for t in $(seq 101 2 145); do B="$B$t:0x4000,"; done
PROBE_SNAP=10 tools/probe.sh newgame 240 PAD_SCRIPT="${B%,}" SPU_DUMP_MISS=$PWD/cache/jobs
```

A debugger does not launch the game cleanly; attaching `lldb` to the
running process does, and guest memory is read through the `vm_base` symbol
(big-endian).

## Layout

| Path | What it is |
| --- | --- |
| [`port/`](port/) | The port: `main.cpp` (boot, guest thread, `afro_defaults()`), `stubs.cpp` (PRX hook and overrides, empty for now), `CMakeLists.txt`, `spu/` (written by `tools/lift_spu.sh`: the lifted SPU code and `spu_workloads.c`, never committed) |
| [`tools/`](tools/) | `rebuild.sh`, `build.sh`, `run.sh`, `probe.sh`, `mount.sh`, `env.sh`, `decrypt_self.py`, `lift_spu.sh`, `lift_jobs.sh`, `augment_functions.py` and `jumptables.txt` (the jump tables whose cases are lifted as functions) |
| [`docs/`](docs/) | `game-manifest.md`: the disc, the executable, the engine, the imports and the SPU images; `screenshots/` |
| [`patches/`](patches/) | The toolkit patch over ps3recomp v0.19.0 (`ae92407b`) |
| [`ROADMAP.md`](ROADMAP.md) | Status and next steps |
| `afro_samurai_ps3_recomp_enhanced.md` | The original plan (Portuguese), with the Enhanced Edition ideas |

`game/`, `analysis/`, `recompiled/`, `build/`, `cache/` and `rpcs3/` are
generated from your disc and never committed.

## License

Code in this repository: MIT ([LICENSE](LICENSE)). The toolkit is MIT by its authors. Afro
Samurai © Bandai Namco Entertainment, published by Namco Bandai Games and
based on the work of Takashi Okazaki and GONZO. This project is not
affiliated with any of them.

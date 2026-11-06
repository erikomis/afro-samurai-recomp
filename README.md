# Afro Samurai — static recompilation for PC (macOS)

🌐 **English** · [Português (BR)](README.pt-BR.md)

A native port of **Afro Samurai** (PS3, disc `BLUS30264`, v01.00) to
**macOS/arm64**, built with static recompilation: the game's PowerPC and SPU
code is translated to C/C++ and compiled into a native executable, with
graphics on Metal. It is not an emulator.

**It boots, but the screen stays black.** The game gets through its start-up
(memory heaps, SPURS with 5 SPUs, the file and render threads, the terrain
loader), loads its save, starts the audio (FMOD Ex on SPURS) and opens the
Bandai Namco logo movie (`bng.bik`). From there it presents frames steadily,
about 54 a second with 10 draws each, for as long as it is left running, but
every frame is black. Nothing past that point has been seen yet; see
[Status](#status).

Built on [ps3recomp](https://github.com/sp00nznet/ps3recomp), with the layout
and the tools of the author's other ports
([inFAMOUS 2](https://github.com/erikomis/infamous2-recomp),
[Resistance: Fall of Man](https://github.com/erikomis/resistance-recomp),
[Resistance 2](https://github.com/erikomis/resistance2-recomp)).

> [!WARNING]
> **Very early work in progress.** It does not show a picture yet, let
> alone a menu or gameplay. There are no releases: you build the game yourself
> from your own disc. Apple-silicon macOS only. What is missing is in
> [ROADMAP.md](ROADMAP.md).

> **This repository contains no game data, no firmware, no Sony keys and
> nothing translated from them.** Everyone generates all of that locally from
> their own legitimate copy. A fan project, not affiliated with Bandai Namco
> Entertainment, Sony Interactive Entertainment or GONZO.

## Status

As of 2026-11-06: boot → heaps and static constructors → SPURS and the
game's threads → the save is loaded → audio up → the logo movie is opened →
a steady stream of black frames. A headless 80-second run ends with the game
still presenting; no crash and no hang.

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

Known issues, being worked on:

- **Black frames.** The game draws (10 draws a frame into 11 surfaces) and
  flips, but the image is black. The logo movie is opened and nothing shows;
  it is not known yet whether the Bink frames are not being decoded, not
  uploaded, or drawn and lost in the backend.
- **Input** has not been tested; there is no controller mapping of the
  port's own.

## How it works

1. **Decrypt** your disc's `EBOOT.BIN` (a disc SELF, `key_rev 0x0001`, type
   APP) into `game/EBOOT.elf` with `tools/decrypt_self.py` and your own key
   file. RPCS3 is not needed.
2. **Analyse and lift** it with the toolkit's `ppu_loader.py` and
   `ppu_lifter.py`: 12,869 functions in the OPD table, 32,951 emitted by the
   lifter, as C++ in `recompiled/BLUS30264/` (16 translation units).
3. **SPU code:** the 20 SPU ELFs embedded in the EBOOT are cut out by the
   toolkit's `extract_spu_images.py` and lifted by `tools/lift_spu.sh` into
   `port/spu/`: EDGE ZLIB inflate (`spu_0000`), the FMOD Ex SPURS mixer
   (`spu_0001`) and 18 FMOD codec and DSP plugins (`spu_0002`–`spu_0019`).
   The SPU skinning job (`ntg/graphics/render/src/ps3/spu_skinning`) is not
   among them; it is expected as raw bytes, as in Resistance 2.
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

The first build compiles a lot of generated code and takes a while. Always
build through `tools/build.sh`: it refuses to start while another Ninja is
running (also another port's), stops if memory runs low (exit 99) and asks
for `BUILD_ALLOW_FULL=1` before rebuilding many lifted chunks at once
(`tools/build.sh --check` says what would be rebuilt; `BUILD_JOBS` sets the
parallel compiles, default 2). The tools need Python 3.10 or newer
(`PY=<python>` picks one). Paths are in [`tools/env.sh`](tools/env.sh).

## Playing

There is nothing to play yet. To run it:

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
| `SYS_MEMPOOL_LOG=1`, `PS3_MEMTRACE=1`, `PS3_LFQ_LOG=<n>` | trace mempools / address reservations / the first n LF-queue pushes |
| `AFRO_WRAP=<program>` | runs the game under a wrapper (a debugger script) |

The four defaults above are set in `afro_defaults()` in
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

A debugger does not launch the game cleanly; attaching `lldb` to the
running process does, and guest memory is read through the `vm_base` symbol
(big-endian).

## Layout

| Path | What it is |
| --- | --- |
| [`port/`](port/) | The port: `main.cpp` (boot, guest thread, `afro_defaults()`), `stubs.cpp` (PRX hook and overrides, empty for now), `CMakeLists.txt`, `spu/` (written by `tools/lift_spu.sh`: the lifted SPU code and `spu_workloads.c`, never committed) |
| [`tools/`](tools/) | `rebuild.sh`, `build.sh`, `run.sh`, `probe.sh`, `mount.sh`, `env.sh`, `decrypt_self.py`, `lift_spu.sh` |
| [`docs/`](docs/) | `game-manifest.md`: the disc, the executable, the engine, the imports and the SPU images |
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

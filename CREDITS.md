# Credits

🌐 **English** · [Português (BR)](CREDITS.pt-BR.md)

This port only exists because of the work below. Thank you all.

## Toolkit

- **[ps3recomp](https://github.com/sp00nznet/ps3recomp)** — Ned Heller
  ([@sp00nznet](https://github.com/sp00nznet)) and
  [contributors](https://github.com/sp00nznet/ps3recomp/blob/master/CONTRIBUTORS.md).
  The runtime, the HLE libraries, the PPU and SPU lifters, the Metal backend
  and the project template this port's `main.cpp`, `stubs.cpp` and
  `CMakeLists.txt` start from. MIT. Built on v0.19.0 (`ae92407b`); this
  port's changes are in [`patches/`](patches/).

## Other ports of the author

- [Resistance 2](https://github.com/erikomis/resistance2-recomp) — the
  `_cellSpursLFQueuePushBody` fix and the LF-queue lost wake-up in
  `WAIT_SIGNAL`.
- [Deadpool](https://github.com/erikomis/deadpool-recomp) — the key
  fetcher `tools/fetch_keys.py` is cut down from its own, and its port ran
  FMOD's SPU overlays through the interpreter first.
- [inFAMOUS 2](https://github.com/erikomis/infamous2-recomp),
  [Resistance: Fall of Man](https://github.com/erikomis/resistance-recomp) and
  PES 2013 — the repository layout, `tools/build.sh` with its memory and disk
  guards, and the roadmap format. Their toolkits also gave this port the
  DualSense code (adaptive triggers, light bar, gyro) and the in-game menu
  and HUD.

## Tools

- [RPCS3](https://github.com/RPCS3/rpcs3) — the visual reference for every
  scene, and the SELF keys: `tools/fetch_keys.py` reads them from its public
  `key_vault.cpp` at a pinned commit, only to decrypt your own disc; no key
  is stored in this repository. Its `Crypto/unself.cpp` was read, not
  copied, for `tools/decrypt_self.py`.
- [sp00nznet/twistedmetal](https://github.com/sp00nznet/twistedmetal) —
  `tools/decrypt_self.py` (MIT), the base of this port's decryptor.
- [Dear ImGui](https://github.com/ocornut/imgui) v1.92.9b — Omar Cornut
  and contributors, MIT. The in-game menu and HUD are drawn with it; its
  core and Metal renderer are part of the toolkit patch
  (`third_party/imgui`, unmodified, with its licence).
- [PS3 Developer Wiki](https://www.psdevwiki.com/ps3/) — the SELF, GCM and
  SPURS documentation.
- [SDL2](https://www.libsdl.org/), [glslang](https://github.com/KhronosGroup/glslang),
  [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross),
  [SPIRV-Tools](https://github.com/KhronosGroup/SPIRV-Tools),
  [Capstone](https://www.capstone-engine.org/),
  [pycryptodome](https://www.pycryptodome.org/), CMake, Ninja.

## Game

Afro Samurai © Bandai Namco Entertainment, published in 2009 by Namco
Bandai Games, based on the work of Takashi Okazaki and GONZO. This fan
project is not affiliated with or endorsed by any of them or by Sony
Interactive Entertainment, and contains no game or firmware data. To use it
you need your own copy of the game.

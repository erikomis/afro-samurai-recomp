# Créditos

🌐 [English](CREDITS.md) · **Português (BR)**

Este port só existe por causa do trabalho abaixo. Obrigado a todos.

## Toolkit

- **[ps3recomp](https://github.com/sp00nznet/ps3recomp)** — Ned Heller
  ([@sp00nznet](https://github.com/sp00nznet)) e
  [colaboradores](https://github.com/sp00nznet/ps3recomp/blob/master/CONTRIBUTORS.md).
  O runtime, as bibliotecas HLE, os tradutores de PPU e SPU, o backend Metal
  e o modelo de projeto de onde saem o `main.cpp`, o `stubs.cpp` e o
  `CMakeLists.txt` deste port. MIT. Baseado na v0.19.0 (`ae92407b`); as
  mudanças deste port estão em [`patches/`](patches/).

## Outros ports do autor

- [Resistance 2](https://github.com/erikomis/resistance2-recomp) — a
  correção do `_cellSpursLFQueuePushBody` e o despertar perdido da fila LF
  em `WAIT_SIGNAL`.
- [Deadpool](https://github.com/erikomis/deadpool-recomp) — o
  `tools/fetch_keys.py` é uma versão reduzida do dele, e foi lá que os
  overlays SPU do FMOD rodaram pelo interpretador primeiro.
- [inFAMOUS 2](https://github.com/erikomis/infamous2-recomp),
  [Resistance: Fall of Man](https://github.com/erikomis/resistance-recomp) e
  PES 2013 — a organização do repositório, o `tools/build.sh` com as
  proteções de memória e disco, e o formato do roadmap.

## Ferramentas

- [RPCS3](https://github.com/RPCS3/rpcs3) — a referência visual de cada
  cena, e as chaves do SELF: o `tools/fetch_keys.py` as lê do
  `key_vault.cpp` público dele num commit fixo, só para descriptografar o seu
  próprio disco; nenhuma chave fica neste repositório. O `Crypto/unself.cpp`
  dele foi lido, não copiado, para o `tools/decrypt_self.py`.
- [sp00nznet/twistedmetal](https://github.com/sp00nznet/twistedmetal) — o
  `tools/decrypt_self.py` (MIT), base do descriptografador deste port.
- [PS3 Developer Wiki](https://www.psdevwiki.com/ps3/) — a documentação de
  SELF, GCM e SPURS.
- [SDL2](https://www.libsdl.org/), [glslang](https://github.com/KhronosGroup/glslang),
  [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross),
  [SPIRV-Tools](https://github.com/KhronosGroup/SPIRV-Tools),
  [Capstone](https://www.capstone-engine.org/),
  [pycryptodome](https://www.pycryptodome.org/), CMake, Ninja.

## Jogo

Afro Samurai © Bandai Namco Entertainment, publicado em 2009 pela Namco
Bandai Games, baseado na obra de Takashi Okazaki e da GONZO. Este projeto de
fã não tem ligação com nenhum deles nem com a Sony Interactive
Entertainment, nem é endossado por eles, e não contém dados do jogo ou do
firmware. Para usá-lo você precisa da sua própria cópia do jogo.

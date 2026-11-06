# Afro Samurai (PS3) — manifesto do jogo

Fase 1 do plano (`afro_samurai_ps3_recomp_enhanced.md`): inventário do dump.

## Identificação

| Campo | Valor |
|---|---|
| Title ID | BLUS30264 (EUA, En/Fr) |
| Versão (APP_VER / VERSION) | 01.00 / 01.00 |
| Firmware mínimo (PS3_SYSTEM_VER) | 02.52 |
| Imagem | `Afro Samurai (USA) (En,Fr).iso`, 3,8 GB, ISO já descriptografado |
| Atualização do jogo | nenhuma instalada; `PS3_UPDATE/PS3UPDAT.PUP` é firmware, não patch |

## Executável

| Item | Valor |
|---|---|
| `PS3_GAME/USRDIR/EBOOT.BIN` | SELF v2, key_rev 0x0001, tipo APP (disco), sha256 `0a3dceca52850d0c7a4a51ddcdbee8065b6b7f454ca04eb0fdb22ae56d55e662` |
| `game/EBOOT.elf` (descriptografado) | 9.842.544 bytes, sha256 `1660911545d35e7c4f2c66be9f1e1a3891e9707e5a9abb59ff065bc62c07de57` |
| Entry OPD / TOC | 0x008DEDC8 → código 0x00010230, TOC 0x008FFFE0 |
| Segmentos | texto 0x10000 (0x88DBC8), dados 0x8A0000 (0xD0BA0, memsz 0x2ED9F0) |
| Funções (OPD) | 12.869; o lifter emitiu 32.951 funções em 16 chunks |
| PRX próprios | nenhum (só bibliotecas de firmware via HLE) |

## Motor e middleware

- Motor próprio da Namco Bandai ("NTG", caminhos `nhi/ntg/...`), **não** Unreal Engine 3.
- Scripts e configuração em `.ini` e `.ntg` dentro de `ps3.psarc`.
- Vídeos Bink (`common/movies/*.bik`), com threads "Bink Audio" e "Bink IO".
- Áudio FMOD Ex (mixer em SPURS + plugins DSP/codecs em SPU).
- NavPower (navegação de IA), EDGE 1.2.3.0 (ZLIB em SPU).
- Skinning em SPU via SPURS (`ntg/graphics/render/src/ps3/spu_skinning`): o binário do job não está entre os ELFs SPU embutidos; esperar um job cru.

## Imports de firmware (156 em 12 bibliotecas)

sysPrxForUser 32, cellSpurs 27, cellGcmSys 19, sys_net 17, sys_io 15, sys_fs 15,
cellSysutil 11, cellAudio 10, cellRtc 4, cellSysmodule 3, cellGame 2, cellNetCtl 1.

Sem cellSaveData, cellResc ou cellSync. Os saves parecem ir por arquivo em
`/dev_hdd0/game/ntg_gameout` e o cache em `/dev_hdd0/game/ntg_cache` / `/dev_hdd1`.

## Imagens SPU embutidas (20)

| Imagem | Conteúdo |
|---|---|
| spu_0000 @0x840600 | EDGE ZLIB inflate (tarefa SPURS) |
| spu_0001 @0x849D80 | FMOD Ex mixer SPURS (`fmodex_spurs.elf`) |
| spu_0002–0019 | plugins FMOD: codecs adpcm/raw/mpeg, DSP chorus, compressor, distortion, echo, flange, highpass, lowpass(2/simple), normalize, oscillator, parameq, pitchshift, sfxreverb |

## Dados do disco

- `PS3_GAME/USRDIR/ps3.psarc` — 2,7 GB, todo o conteúdo do jogo.
- `PS3_GAME/USRDIR/common/movies` — cutscenes e fundos de menu em Bink.
- `PS3_GAME/USRDIR/common/stringtables`, `subtitles`.

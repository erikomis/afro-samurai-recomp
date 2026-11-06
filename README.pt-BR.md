# Afro Samurai — recompilação estática para PC (macOS)

🌐 [English](README.md) · **Português (BR)**

Port nativo do **Afro Samurai** (PS3, disco `BLUS30264`, v01.00) para
**macOS/arm64**, feito com recompilação estática: o código PowerPC e SPU do
jogo é traduzido para C/C++ e compilado como um executável nativo, com
gráficos em Metal. Não é um emulador.

**Ele dá boot, mas a tela fica preta.** O jogo passa pela inicialização
(heaps de memória, SPURS com 5 SPUs, as threads de arquivo e de render, o
carregador de terreno), carrega o save, liga o áudio (FMOD Ex no SPURS) e
abre o vídeo do logo da Bandai Namco (`bng.bik`). Dali em diante apresenta
quadros sem parar, uns 54 por segundo com 10 draws cada, pelo tempo que for
deixado rodando, mas todos os quadros são pretos. Nada depois desse ponto foi
visto ainda; veja [Estado](#estado).

Construído sobre o [ps3recomp](https://github.com/sp00nznet/ps3recomp), com a
organização e as ferramentas dos outros ports do mesmo autor
([inFAMOUS 2](https://github.com/erikomis/infamous2-recomp),
[Resistance: Fall of Man](https://github.com/erikomis/resistance-recomp),
[Resistance 2](https://github.com/erikomis/resistance2-recomp)).

> [!WARNING]
> **Trabalho em andamento, bem no começo.** Ainda não mostra imagem, muito
> menos menu ou gameplay. Não há releases: você mesmo compila o jogo a partir
> do seu disco. Só macOS com Apple silicon. O que falta está no
> [ROADMAP.pt-BR.md](ROADMAP.pt-BR.md).

> **Este repositório não contém dados do jogo, firmware, chaves da Sony nem
> nada traduzido a partir deles.** Cada um gera tudo isso localmente a partir
> da própria cópia legítima. Projeto de fã, sem afiliação com a Bandai Namco
> Entertainment, a Sony Interactive Entertainment ou a GONZO.

## Estado

Em 06/11/2026: boot → heaps e construtores estáticos → SPURS e as threads do
jogo → o save é carregado → áudio ligado → o vídeo do logo é aberto → um
fluxo constante de quadros pretos. Um teste headless de 80 segundos termina
com o jogo ainda apresentando quadros; sem crash e sem travar.

O que foi preciso, com a causa de cada um:

- **Pools de memória:** o jogo cria e destrói milhares de `sys_mempool`
  pequenos (uns 3.800 em dois minutos; classes de 16, 32 e 64 bytes). O
  `sysPrxForUser` agora implementa `try_allocate` / `allocate` / `free` /
  `destroy`, escreve as contagens em big-endian e entrega handles marcados com
  geração, para que o handle de um pool destruído nunca seja reaproveitado por
  engano (`SYS_MEMPOOL_LOG=1` rastreia).
- **O mapeador de memória:** o jogo coloca as próprias páginas de heap de
  1 MB com `sys_mmapper_map_memory` ao lado desses pools de 256 KB. O cursor
  sem controle sobrepunha os dois (um array de blocos levava `memset` por
  cima de nós de lista vivos), depois ficava sem espaço de endereços e o jogo
  chamava `abort()`. O mapeador agora mantém um mapa de dono por página, com
  map / unmap / free de verdade e busca next-fit por reserva
  (`PS3_MMAPPER_TRACK=1`, ligado por padrão).
- **Seis reservas em ordem crescente:** o jogo reserva 512 MB mais
  5 × 256 MB de espaço de endereços e decide o `free()` pela faixa de reserva
  em que o ponteiro cai, então elas precisam vir em ordem crescente. Duas
  voltavam `ENOMEM`. Com `PS3_MAPPER_WIDE=1` (ligado por padrão) elas sobem
  e podem englobar as janelas do próprio runtime, que o mapeador com controle
  nunca entrega.
- **Os plugins SPU do FMOD:** a tarefa do mixer do FMOD Ex carrega seus
  plugins de DSP e codec como overlays e pula para dentro deles (LS
  `0x25A18`). Sem código traduzido ali o job terminava, e a thread de eventos
  do FMOD esperava para sempre pela event flag do mixer segurando o lwmutex
  de que a thread principal precisava. `SPU_INTERP_UNLIFTED` (ligado por
  padrão) interpreta o que não tem corpo traduzido, como no port do Deadpool.
- **Primeiro save:** sem save ainda, o callback de stat do jogo responde
  `ERR_NODATA`; o firmware transforma isso em `ERROR_CBRESULT`, que é como o
  jogo sabe que deve criar um. O toolkit respondia `ERROR_NODATA`, e o jogo
  dizia que os dados salvos "parecem estar danificados" e parava.
  `SAVEDATA_CBRESULT_STRICT=1` (ligado por padrão) responde como o firmware;
  agora o jogo grava e lê de volta o `AFRO_0.SAV`.
- **`cellKbRead`:** o jogo consulta o teclado a cada quadro por
  `cellKbRead` (NID `0xFF0A21B7`, o nome do SDK para `cellKbGetData`). Não
  estava resolvido, o caminho genérico respondia `CELL_OK` sem escrever nada,
  e o jogo lia o que estivesse na pilha como eventos de tecla.
- **Filas lock-free do SPURS:** `_cellSpursLFQueuePushBody` foi
  implementado sobre a linha canônica de 128 bytes da `CellSyncLFQueue` que o
  consumidor SPU lê, e uma tarefa que estaciona em `WAIT_SIGNAL` com a fila já
  tendo entradas continua em vez de dormir (os dois trazidos da árvore do
  Resistance 2; `SPURS_LFQ_NOWAKE=1` desliga o segundo).

Problemas conhecidos, sendo resolvidos:

- **Quadros pretos.** O jogo desenha (10 draws por quadro em 11
  superfícies) e faz flip, mas a imagem é preta. O vídeo do logo é aberto e
  nada aparece; ainda não se sabe se os quadros do Bink não estão sendo
  decodificados, não estão sendo enviados, ou se são desenhados e se perdem
  no backend.
- **Entrada** ainda não foi testada; o port não tem mapeamento de controle
  próprio.

## Como funciona

1. **Descriptografar** o `EBOOT.BIN` do seu disco (um SELF de disco,
   `key_rev 0x0001`, tipo APP) em `game/EBOOT.elf` com o
   `tools/decrypt_self.py` e o seu próprio arquivo de chaves. Não precisa do
   RPCS3.
2. **Analisar e traduzir** com o `ppu_loader.py` e o `ppu_lifter.py` do
   toolkit: 12.869 funções na tabela OPD, 32.951 emitidas pelo lifter, como
   C++ em `recompiled/BLUS30264/` (16 unidades de tradução).
3. **Código SPU:** os 20 ELFs SPU embutidos no EBOOT são recortados pelo
   `extract_spu_images.py` do toolkit e traduzidos pelo `tools/lift_spu.sh`
   em `port/spu/`: EDGE ZLIB inflate (`spu_0000`), o mixer SPURS do FMOD Ex
   (`spu_0001`) e 18 plugins de codec e DSP do FMOD (`spu_0002`–`spu_0019`).
   O job de skinning em SPU (`ntg/graphics/render/src/ps3/spu_skinning`) não
   está entre eles; espera-se que esteja em bytes crus, como no Resistance 2.
4. **Compilar** com CMake/Ninja contra o toolkit: o runtime (syscalls do
   LV2, bibliotecas de firmware em HLE, SPURS, o backend RSX em Metal) mais o
   código traduzido, em `build/BLUS30264/AfroSamuraiRecomp`.

O jogo usa o motor próprio da Namco Bandai, o **NTG** (caminhos
`nhi/ntg/...`), não a Unreal Engine 3: scripts e configuração em arquivos
`.ini` e `.ntg` dentro do `ps3.psarc` (2,7 GB, todo o conteúdo do jogo),
vídeos Bink, áudio FMOD Ex, NavPower para a navegação da IA e EDGE 1.2.3.0.
Não tem PRX próprio e importa 156 funções de firmware de 12 bibliotecas;
salva pelo `cellSaveData` e mantém um cache em `/dev_hdd0/game/ntg_cache` e
em `/dev_hdd1`. O inventário completo está em
[`docs/game-manifest.md`](docs/game-manifest.md).

O toolkit é o ps3recomp **v0.19.0** do upstream (`ae92407b`) mais as
mudanças deste port (os pools de memória, o mapeador com controle, a
resposta do primeiro save, `cellKbRead`, a fila lock-free do SPURS), que são
genéricas e controladas por variáveis de ambiente, guardadas num patch só:
[`patches/ps3recomp-ae92407b-afro.patch`](patches/). Por enquanto o build
usa um checkout com o patch aplicado ao lado deste repositório
(`../ps3recomp-afro`); um script que clona o toolkit e aplica o patch está no
[roadmap](ROADMAP.pt-BR.md).

## Compilando a partir do código

### O que você precisa

- Um Mac com Apple silicon (macOS com Metal). Cada pedaço traduzido tem
  dezenas de MB de C++ e o clang usa 3–4 GB em um; o `tools/build.sh` limita
  a compilação a duas de cada vez e vigia a memória e o disco.
- O seu próprio **disco do Afro Samurai, `BLUS30264`**, em ISO. Coloque-o
  (ou um link para a pasta dele) em `game_iso/`; ele é montado só para
  leitura e nunca é alterado.
- Um arquivo de chaves para o SELF do disco (`AFRO_KEYS`; por enquanto o que
  o `tools/fetch_keys.py` do port do Deadpool gera).
- O toolkit ps3recomp em `../ps3recomp-afro` (`PS3RECOMP_DIR` muda isso):
  o `ae92407b` do upstream com o `patches/ps3recomp-ae92407b-afro.patch`
  aplicado (`git apply`).
- Xcode Command Line Tools e Homebrew:

```sh
brew install cmake ninja sdl2 glslang spirv-cross spirv-tools ffmpeg python@3.12
```

### Compilando

```sh
tools/rebuild.sh            # stage, decrypt, analyse, lift, spu, build
```

O `tools/rebuild.sh` roda estes passos (rode qualquer um sozinho pelo nome,
por exemplo `tools/rebuild.sh spu build`):

| Passo | O que faz |
| --- | --- |
| `stage` | monta a ISO só para leitura (`tools/mount.sh`) e copia o `EBOOT.BIN` e o `PARAM.SFO` para `game/` |
| `decrypt` | `tools/decrypt_self.py` com `$AFRO_KEYS` → `game/EBOOT.elf` |
| `analyse` | `ppu_loader.py` → `analysis/BLUS30264/` |
| `lift` | `ppu_lifter.py` → `recompiled/BLUS30264/` (`CHUNKS`, padrão 12; `JOBS` limita o paralelismo) |
| `spu` | `extract_spu_images.py` e `tools/lift_spu.sh` → `port/spu/` |
| `build` | CMake (uma vez) + `tools/build.sh` → `build/BLUS30264/AfroSamuraiRecomp` |

A primeira compilação passa por muito código gerado e demora. Compile
sempre pelo `tools/build.sh`: ele se recusa a começar enquanto outro Ninja
estiver rodando (inclusive o de outro port), para se a memória ficar baixa
(código de saída 99) e pede `BUILD_ALLOW_FULL=1` antes de recompilar muitos
pedaços traduzidos de uma vez (`tools/build.sh --check` diz o que seria
recompilado; `BUILD_JOBS` define as compilações em paralelo, padrão 2). As
ferramentas precisam de Python 3.10 ou mais novo (`PY=<python>` escolhe um).
Os caminhos estão no [`tools/env.sh`](tools/env.sh).

## Jogando

Ainda não há nada para jogar. Para rodar:

```sh
tools/run.sh
```

Ele monta a ISO, liga o `PS3_GAME` do disco à raiz do VFS e mantém o
`/dev_hdd0` e o `/dev_hdd1` na pasta `rpcs3/` deste repositório, nunca na
instalação do RPCS3, então um teste não mexe nos seus saves do RPCS3.
Argumentos a mais são variáveis de ambiente: `tools/run.sh PS3_VERBOSE=1`.

### Ambiente

| Variável | Efeito |
| --- | --- |
| `PS3RECOMP_DIR` | o checkout do toolkit (padrão `../ps3recomp-afro`) |
| `AFRO_KEYS` | o arquivo de chaves que o `decrypt_self.py` lê |
| `AFRO_ISO_DIR`, `AFRO_MOUNT` | onde está a ISO (`game_iso/`) e onde ela é montada (`/Volumes/AFRO_SAMURAI`) |
| `PS3_HDD0_ROOT`, `PS3_HDD1_ROOT` | as pastas de HDD do jogo (`rpcs3/dev_hdd0`, `rpcs3/dev_hdd1`) |
| `PS3_VERBOSE=1` | o log completo do runtime (desligado no `run.sh`) |
| `PS3_QOS=0` | deixa a prioridade de thread padrão do macOS (por padrão as threads do jogo rodam como user-interactive, fora dos núcleos de eficiência) |
| `PS3_MMAPPER_TRACK=0` | o mapeador de memória volta ao cursor sem controle (traz o `abort()` de volta) |
| `PS3_MAPPER_WIDE=0` | as seis reservas voltam à janela estreita (dois `ENOMEM`) |
| `PS3_MMAPPER_FIRSTFIT=1` / `PS3_MMAPPER_ZERO=1` | first-fit em vez de next-fit (medido pior) / zerar as páginas ao mapear |
| `SAVEDATA_CBRESULT_STRICT=0` | `ERR_NODATA` volta a ser `ERROR_NODATA` (a parada do "save danificado") |
| `SPU_INTERP_UNLIFTED` | interpreta o código SPU sem corpo traduzido (definida pelo `port/main.cpp`; ainda sem como desligar) |
| `SPURS_LFQ_NOWAKE=1` | desliga a correção de despertar da fila LF (para comparação) |
| `SYS_MEMPOOL_LOG=1`, `PS3_MEMTRACE=1`, `PS3_LFQ_LOG=<n>` | rastreia mempools / reservas de endereço / os n primeiros pushes na fila LF |
| `AFRO_WRAP=<programa>` | roda o jogo dentro de um wrapper (um script de depurador) |

Os quatro padrões acima são definidos em `afro_defaults()` no
[`port/main.cpp`](port/main.cpp), a não ser que já estejam no ambiente.

### Diagnóstico

`tools/probe.sh <rótulo> <segundos> [VAR=valor ...]` roda o build headless
com limite de tempo e grava `cache/probe/<rótulo>.log` e um dump de quadro,
`<rótulo>.ppm`, a cada 120 apresentações. `PROBE_SNAP=<s>` guarda uma cópia
do dump mais recente a cada `s` segundos, para o teste mostrar uma sequência
e não só o último quadro. Os botões vêm de um script,
`PAD_SCRIPT="<seg>:<máscara>,..."` (START `0x0008`, UP `0x0010`, DOWN
`0x0040`, CROSS `0x4000`, CIRCLE `0x2000`, SQUARE `0x8000`, TRIANGLE
`0x1000`; `PAD_CLOCK=flips` conta flips em vez de segundos). Ele só encerra o
processo que ele mesmo iniciou.

```sh
PROBE_SNAP=5 tools/probe.sh boot 80
```

Iniciar o jogo pelo depurador não funciona direito; anexar o `lldb` ao
processo já rodando funciona, e a memória do guest é lida pelo símbolo
`vm_base` (big-endian).

## Organização

| Caminho | O que é |
| --- | --- |
| [`port/`](port/) | O port: `main.cpp` (boot, thread do guest, `afro_defaults()`), `stubs.cpp` (hook de PRX e substituições, vazio por enquanto), `CMakeLists.txt`, `spu/` (gerado pelo `tools/lift_spu.sh`: o código SPU traduzido e o `spu_workloads.c`, nunca vão para o repositório) |
| [`tools/`](tools/) | `rebuild.sh`, `build.sh`, `run.sh`, `probe.sh`, `mount.sh`, `env.sh`, `decrypt_self.py`, `lift_spu.sh` |
| [`docs/`](docs/) | `game-manifest.md`: o disco, o executável, o motor, os imports e as imagens SPU |
| [`patches/`](patches/) | O patch do toolkit sobre o ps3recomp v0.19.0 (`ae92407b`) |
| [`ROADMAP.pt-BR.md`](ROADMAP.pt-BR.md) | Estado e próximos passos |
| `afro_samurai_ps3_recomp_enhanced.md` | O plano original, com as ideias da Enhanced Edition |

`game/`, `analysis/`, `recompiled/`, `build/`, `cache/` e `rpcs3/` são
gerados a partir do seu disco e nunca vão para o repositório.

## Licença

Código deste repositório: MIT ([LICENSE](LICENSE)). O toolkit é MIT, dos seus autores. Afro
Samurai © Bandai Namco Entertainment, publicado pela Namco Bandai Games e
baseado na obra de Takashi Okazaki e da GONZO. Este projeto não tem
afiliação com nenhum deles.

# Roadmap

🌐 [English](ROADMAP.md) · **Português (BR)**

Estado do port nativo do Afro Samurai (`BLUS30264`) para macOS/arm64 e o que
vem a seguir. Mesma organização dos roadmaps do inFAMOUS 2 e dos Resistance.
`[x]` = feito e verificado; `[ ]` = a fazer; "(em andamento)" = sendo feito.

## Resumo (11/11/2026)

- **A gameplay é desenhada corretamente:** boot → menus → New Game ou
  Continue → a tela 3D de carregamento → a cutscene de abertura e o tutorial
  de combate, com o Afro, o Ninja Ninja e a vila com textura e iluminação,
  jogado numa janela com controle sem crash. O RPCS3 com o mesmo disco foi a
  referência visual.
- **Ritmo de quadros corrigido:** a gameplay roda a 30 quadros por segundo
  como o PS3 e o RPCS3 (29,5-30,1 medidos em 260 s, teste headless de New
  Game até o tutorial de combate; eram ~45-48). O título dita o próprio
  ritmo com o esquema prepare-flip + flip-immediate da libgcm; o HLE do
  toolkit agora devolve o id real do buffer em `cellGcmSetPrepareFlip`,
  roda o flip handler no flip e bloqueia o RSX no `SEMAPHORE_ACQUIRE`
  (limite de 250 ms).
- **Áudio dentro do jogo corrigido:** estava lento, borrado e
  ininteligível em jogo em janela. Os 18 plugins de codec/DSP do FMOD
  (imagens SPU 2-19) rodavam no interpretador de SPU (84% da thread do
  mixer) porque o mixer copia o texto deles para um endereço de local store
  diferente daquele em que foram traduzidos; o `tools/lift_fmod_plugins.sh`
  (rodado pelo `rebuild.sh spu`) agora os traduz como overlays relocáveis.
  Confirmado de ouvido.
- **Texturas quebradas, duas causas corrigidas:** o alpha-to-coverage
  (`0x1D7C`) não era suportado e agora é emulado como alpha test em 0,5; e o
  motor de desenho misturava os métodos de blit 2D do título (NV3089,
  subcanal 5) no arquivo de registradores 3D, apagando o estado do alpha
  test, então cartões de grama e folhagem saíam como retângulos escuros.
  Agora só os subcanais 3D chegam ao motor (`GCM_ENGINE_3D_SUBCH=1`, padrão
  do port).
- **Bloqueio atual:** nenhum conhecido. A conferência final da grama do
  pátio está pendente, e outras texturas quebradas podem restar sem
  reporte.
- **Próximo:** confirmar a grama a olho, procurar outros erros de
  renderização jogando além dos primeiros minutos.
- **Toolkit:** ps3recomp v0.19.0 do upstream (`ae92407b`) mais as mudanças
  genéricas deste port, controladas por variáveis de ambiente, num patch só
  (`patches/ps3recomp-ae92407b-afro.patch`).

## Feito

- [x] O disco inventariado ([`docs/game-manifest.md`](docs/game-manifest.md)):
  `BLUS30264` v01.00, sem atualização do jogo, o motor (o NTG da Namco
  Bandai, não a UE3), o middleware (Bink, FMOD Ex, NavPower, EDGE), 156
  imports de 12 bibliotecas de firmware, 20 imagens SPU embutidas, nenhum PRX
  próprio.
- [x] `EBOOT.BIN` descriptografado sem o RPCS3 (`tools/decrypt_self.py`, um
  SELF de disco com `key_rev 0x0001`).
- [x] EBOOT analisado e traduzido (12.869 funções OPD, 32.951 emitidas, 16
  unidades de tradução); compila e linka no macOS/arm64 com Metal.
- [x] Os 20 ELFs SPU embutidos traduzidos (`tools/lift_spu.sh`): EDGE ZLIB,
  o mixer SPURS do FMOD Ex e 18 plugins de codec/DSP do FMOD.
- [x] Os scripts: `rebuild.sh`, `build.sh` com proteção de memória e disco
  (dois kernel panics por builds sem controle em 01/11/2026), `run.sh` com a
  ISO montada só para leitura e as pastas de HDD dentro do repositório,
  `probe.sh` para testes headless com dumps de quadro e botões por script.
- [x] `sys_mempool` implementado (`try_allocate` / `allocate` / `free` /
  `destroy`, contagens em big-endian, handles marcados com geração): o jogo
  cria uns 3.800 pools em dois minutos.
- [x] O mapeador de memória com controle (`PS3_MMAPPER_TRACK=1`): mapa de
  dono por página, map / unmap / free de verdade e next-fit por reserva; o
  cursor não sobrepõe mais as páginas de heap do jogo e os pools dele (o
  `memset` por cima de nós de lista vivos, depois o `abort()`).
- [x] Seis reservas de endereço em ordem crescente, 512 MB + 5 × 256 MB
  (`PS3_MAPPER_WIDE=1`): duas voltavam `ENOMEM`, e o jogo decide o `free()`
  pela faixa de reserva.
- [x] Os overlays de plugin SPU do FMOD rodam interpretados onde não há corpo
  traduzido (`SPU_INTERP_UNLIFTED`; hoje traduzidos, veja o item de áudio
  abaixo): a thread de eventos não espera mais para
  sempre pelo mixer segurando o lwmutex de que a thread principal precisa.
- [x] Primeiro save: um resultado negativo do callback de stat responde
  `ERROR_CBRESULT`, como o firmware (`SAVEDATA_CBRESULT_STRICT=1`); a parada
  do "os dados salvos parecem estar danificados" acabou, e o `AFRO_0.SAV` é
  gravado e lido de volta.
- [x] `cellKbRead` (NID `0xFF0A21B7`) respondido pelo `cellKbGetData`; o jogo
  não lê mais lixo da pilha como eventos de tecla.
- [x] `_cellSpursLFQueuePushBody` sobre a linha canônica da
  `CellSyncLFQueue`, e o despertar perdido da fila LF no `WAIT_SIGNAL` (os dois
  da árvore do Resistance 2).
- [x] Passou da antiga leitura de nulo na thread principal em
  `func_002D13B8` (o registro global em `[r30-0x7FA0]+4`): o boot agora segue
  para o carregador de terreno, o worker de save, o áudio e o vídeo do logo.
- [x] O trace temporário `[afro-memset]` retirado do código traduzido.
- [x] Os quadros pretos no boot: os vídeos abrem. O Bink lê o descritor do
  `cellFs` que o jogo entrega a ele pelas syscalls cruas do lv2; agora read,
  lseek e fstat do lv2 recorrem à tabela do `cellFs`.
- [x] Os vídeos de logo e de créditos (`bng.bik`, `two_headbands.bik`,
  `afro_credit.bik`) e o menu principal sobre o `background_movie.bik`.
- [x] O `PAD_SCRIPT` chega ao jogo: START pela tela de título, CROSS pelo
  menu até o New Game.
- [x] A cadeia de skinning em SPU: os binários dos jobs gravados
  (`SPU_DUMP_MISS`) e traduzidos (`tools/lift_jobs.sh`); o percorredor espera
  num guard fechado em vez de desistir depois de 20 s
  (`SPURS_JG_WAIT_FOREVER`); só os eventos dos próprios jobs são postados
  (`SPURS_JC_EVENT_FROM_JOB`, a fila de profundidade 1 que a thread de render
  espera); os campos do contexto de job antigo que o job lê
  (`SPURS_JOB_CTX_V1`, a parada no guard e o PUT por cima do código do jogo);
  PUTs de DMA no segmento só-leitura descartados.
- [x] O diagnóstico `AFRO_TEXT_RO=1`: o código do jogo só-leitura depois do
  carregamento, para uma escrita do host nele parar mostrando quem chamou
  (pegou o PUT do job de skinning).
- [x] Alvos das tabelas de salto traduzidos como funções
  (`tools/augment_functions.py`, `tools/jumptables.txt`): o caso perdido
  `0x1F079C` do despachante de mensagens e o New Game girando num NULL;
  nenhum "unresolved indirect call" em quatro testes.
- [x] O New Game chega à tela 3D de carregamento.
- [x] O áudio dentro do jogo: os 18 plugins de codec/DSP do FMOD traduzidos
  como overlays relocáveis (`tools/lift_fmod_plugins.sh`, rodado pelo
  `rebuild.sh spu`) em vez de interpretados; o interpretador foi de 979 de
  1162 amostras da thread do mixer para nenhuma, e soa certo de ouvido
  (`SPU_RELOC_OFF=1` para A/B).
- [x] Alpha-to-coverage (`0x1D7C`) emulado como alpha test em 0,5
  (`PS3RECOMP_ENG_A2C`): tufos de grama saem como lâminas, não retângulos.
- [x] Os métodos de blit 2D não chegam mais ao arquivo de registradores 3D
  (`GCM_ENGINE_3D_SUBCH=1`): o estado do alpha test sobrevive aos blits
  NV3089 do título.

## Próximos passos: jogar até o fim, e deixar rápido

### Bloqueios

- [x] **Preto, depois geometria explodida, depois da tela de carregamento.**
  Três causas: vertex programs traduzidos como se começassem no slot 0 (os
  programas com salto davam 16.384 voltas por vértice); o percorredor de
  cadeias do SPURS pulava o JOBLIST, então os 5 jobs de skinning de vértices
  por quadro nunca rodavam e os draws liam sobras de outro escritor no anel; e
  a segunda entrada dos jobs de skinning apontava para o buffer de cache no
  contexto de job antigo. As três corrigidas (07/11/2026).
- [ ] Vigiar os logs por mais alvos de "unresolved indirect call" e
  acrescentar as tabelas deles ao `tools/jumptables.txt` (ou deixar a busca
  achá-las).
- [x] Os dois jobs conhecidos da cadeia de skinning extraídos do EBOOT no
  rebuild (`tools/extract_jobs.py`), conferidos byte a byte com dumps de execução.

### Os menus, de verdade

- [ ] As telas legais e dos estúdios, a tela de título e os menus conferidos
  com o PS3 (todas as opções, Treasures).
- [ ] Entrada: um controle pelo pad SDL do toolkit, depois teclado e mouse.
- [ ] Som e música dos menus (FMOD no SPURS), em sincronia com os vídeos.

### Até a primeira luta

- [ ] New Game → a cutscene de abertura → a primeira fase, controlável.
- [ ] O visual cel-shaded e os contornos conferidos com o PS3: shaders,
  blending, depth/stencil, render targets.
- [ ] Streaming do `ps3.psarc` com o EDGE ZLIB nos SPUs, sem engasgos.

## O que falta para ficar 100% jogável

### Gameplay

- [ ] A história inteira jogada, fase por fase: chefes, QTEs, mortes,
  checkpoints, o final.
- [ ] Saves entre sessões (checkpoint, sair, Continuar).

### Gráficos

- [x] Recortes de grama e folhagem: alpha-to-coverage emulado, e os blits
  2D mantidos fora do estado de alpha test do 3D.
- [ ] A grama do pátio confirmada a olho numa sessão em janela.
- [ ] Procurar outras texturas quebradas; testar se o hash completo do cache
  de texturas só a cada 8 quadros deixa texturas em streaming desatualizadas
  (`PS3RECOMP_ENG_TEXHASH_FULL=1`, não confirmado).
- [ ] Cada bug de renderização encontrado jogando corrigido, cada um
  comparado com uma captura do PS3 na mesma cena.

### Desempenho

- [x] A correção do ritmo de flip: ids reais do prepare-flip, o flip handler
  rodando no flip, o RSX esperando o buffer de exibição
  (`GCM_PREPAREFLIP_FIFO=2`, `GCM_FLIPCB_ONTICK=1`, `GCM_SEMA_ACQUIRE=1`).
  Gameplay a 29,5-30,1 quadros por segundo em 260 s, nenhum acquire estourou
  o limite.
- [x] O áudio dentro do jogo conferido de ouvido: soa certo (veja a
  correção dos plugins do FMOD acima). A comparação com o RPCS3 e o
  rebaixamento opcional `PS3_AUDIO_DOWNMIX=itu` ficam para depois.
- [ ] 30 fps estáveis com frame pacing regular (o alvo do jogo no PS3; a média
  já foi atingida), medidos com tempos de quadro p95/p99; perfilar CPU, SPU, GPU, compilação de
  shaders e streaming.
- [ ] O cache de tradução de shaders e a compilação de pipelines em segundo
  plano dos outros ports, para shaders vistos pela primeira vez não
  engasgarem.

### Estabilidade

- [ ] Sessões longas sem travar e sem vazamento (o jogo cria e destrói
  milhares de mempools; vigiar se o mapeador com controle não fica sem
  espaço).
- [ ] Um encerramento limpo.

### Rebuild

- [x] As mudanças do toolkit exportadas como um patch só,
  `patches/ps3recomp-ae92407b-afro.patch` (conferido que aplica num
  `ae92407b` limpo).
- [ ] Um `tools/setup_toolkit.sh` que clona a v0.19.0 e aplica o patch, e um
  `tools/export_toolkit_patch.sh` que transforma uma pasta do toolkit de volta
  no patch.
- [ ] Independente: o próprio buscador de chaves e o próprio venv de Python,
  em vez dos do port do Deadpool.

## Depois de jogável (melhorias)

As ideias da Enhanced Edition do
[`afro_samurai_ps3_recomp_enhanced.md`](afro_samurai_ps3_recomp_enhanced.md),
cada uma como opção, com o comportamento original mantido como padrão. Cada
crítica das análises de 2009 precisa ser reproduzida neste build primeiro:
bug do port e bug do jogo original são coisas diferentes.

### Gráficos e resolução

- [ ] Resolução interna: 1080p e 1440p primeiro, 4K só se o desempenho
  permitir.
- [ ] SMAA / FXAA, filtragem anisotrópica e nitidez, sem estragar o cel
  shading e os contornos.
- [ ] Ultrawide, se a câmera e o HUD permitirem.

### Taxa de quadros

- [ ] 60 fps como modo experimental, só depois de mapear o que depende da
  taxa de quadros (física, animação, scripts, temporizadores, QTEs, a
  câmera).

### Câmera e controles

- [ ] Uma câmera melhor: distância, colisão, sem perder o Afro atrás da
  geometria.
- [ ] Inversão X/Y independente e sensibilidade; deadzones e remapeamento
  para DualSense, DualShock 4 e Xbox.
- [ ] Resposta de pulo e movimento (buffer de entrada, tolerâncias).

### Design do jogo (opcional)

- [ ] Checkpoints extras que restaurem vida, inimigos, scripts e flags sem
  softlocks.
- [ ] Um balanceamento opcional dos chefes (avisos de ataque, dano, timing).
- [ ] Uma lista de golpes e feedback de combos; cutscenes puláveis onde o
  estado depois delas for seguro de montar.

### App para instalar e jogar (launcher)

- [ ] O launcher de desktop dos ports do Resistance: instalação em um clique
  a partir do disco, configurações, saves, jogar sem terminal.

### Outras plataformas

- [ ] Windows e Linux, depois que o macOS estiver jogável.

### Fora do escopo por enquanto

- Recursos online, troféus, e pacotes de texturas ou assets que dependam de
  conteúdo de terceiros.

## Devolvendo ao upstream

- [ ] Mandar as correções genéricas do toolkit para o
  [sp00nznet/ps3recomp](https://github.com/sp00nznet/ps3recomp), cada uma com
  um teste: `sys_mempool`, o mapeador com controle e as reservas em ordem
  crescente, `ERROR_CBRESULT` para um callback de stat negativo, `cellKbRead`,
  read / lseek / fstat do lv2 num descritor do `cellFs`, a espera no guard e
  os eventos mandados pelos jobs no percorredor da cadeia, os campos do
  contexto de job antigo, PUTs de DMA no segmento só-leitura descartados.
- [ ] A divisão nos casos das tabelas de salto (`augment_functions.py`) como
  opção do lifter no upstream, para todo port ganhar.
- [x] O próprio repositório git (`afro-samurai-recomp`): os fontes do port, o
  patch do toolkit, README, a licença e um `.gitignore` que bloqueia dados do
  jogo, firmware, chaves e saves.
- [ ] `CREDITS.md`, e o repositório publicado no GitHub.

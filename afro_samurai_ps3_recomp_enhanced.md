# Afro Samurai (PS3) — Plano de Recompilação Nativa e Enhanced Edition
usando ps3recomp
**Plataforma de origem:** PlayStation 3 (2009)  
**Destino:** macOS Apple Silicon, com renderização Metal  
**Base técnica proposta:** PS3Recomp, runtime PS3/LV2, backend RSX/Metal e SDL para controles  
**Status:** planejamento; compatibilidade e viabilidade ainda precisam ser verificadas no executável específico.

## 1. Objetivo

Recompilar estaticamente a versão de PS3 de **Afro Samurai** para execução nativa no macOS, preservar o estilo artístico e a identidade do combate e, após alcançar gameplay completo, implementar correções opcionais para problemas do jogo original e aprimoramentos técnicos.

**Princípio:** separar sempre os bugs introduzidos pelo port (renderização, SPU, sincronização, áudio) dos defeitos e limitações do jogo de 2009 (câmera, checkpoints, balanceamento). A recompilação por si só não corrige automaticamente nenhum deles.

## 2. Problemas históricos a investigar e corrigir

As críticas de época mencionaram problemas de câmera, controle/plataforma, desempenho, repetição e frustrações de combate. A severidade e a presença de cada problema devem ser verificadas na versão PS3 escolhida, com testes reproduzíveis; a lista abaixo é um backlog de investigação, não um inventário de bugs já comprovados no port.

| Área | Problema relatado / hipótese | Correção proposta | Dificuldade estimada |
|---|---|---|---|
| Câmera | Câmera instável, lenta ou obstruída | Câmera livre, colisão e distância melhoradas | Alta |
| Controles | Preferência por eixos de câmera não atendida | Inversão X/Y independente e sensibilidade ajustável | Baixa–média |
| Desempenho | Quedas de FPS, inclusive em cenas exigentes | Perfil de CPU/GPU, frame pacing e otimização | Alta |
| Plataforma | Saltos e movimentação imprecisos | Ajuste de resposta, buffers de entrada e tolerâncias | Alta |
| Checkpoints | Repetição excessiva após derrotas | Pontos de reinício adicionais, com integridade de estado | Média–alta |
| Chefes | Ataques ou janelas de reação frustrantes | Ajustes opcionais de telegraph, dano e timing | Alta |
| Combate | Encontros repetitivos | Rebalanceamento opcional e variedade de encontros | Muito alta |
| Progressão | Pouca clareza sobre golpes e combos | Lista de movimentos e feedback de desbloqueio | Média |
| Cutscenes | Repetição de cenas | Investigar opção segura de pular cenas | Média |
| Áudio | Falhas eventuais de efeitos/mixagem | Diagnóstico de canais, buffers e eventos de áudio | Média |

### Critérios de aceite para as correções

- **Câmera:** sem atravessar geometria nem ocultar o personagem em arenas de teste; opções X/Y persistidas.
- **Controles:** suporte SDL para DualSense, DualShock 4 e Xbox, com mapeamento e deadzones configuráveis.
- **Checkpoints:** retorno consistente de vida, inimigos, scripts, câmera, inventário e flags de progresso; evitar softlocks.
- **Chefes:** alterações opcionais, com modo original preservado; validar todas as fases e transições.
- **Áudio:** verificar sincronismo com vídeo, efeitos, música e dublagem em cenas longas.
- **Cutscenes:** pular apenas quando a inicialização do estado posterior for segura.

## 3. Melhorias gráficas e desempenho no macOS

### Resolução e imagem

- Testar renderização interna em 1080p e 1440p antes de considerar 4K.
- Implementar ajuste de escala de renderização e saída em tela cheia/janela.
- Investigar SMAA, FXAA, filtragem anisotrópica e nitidez, preservando o cel shading e os contornos.
- Evitar filtros que destruam a estética desenhada à mão.

### Fidelidade do renderer

- Validar texturas, transparências, blending, depth/stencil, sombras, partículas e pós-processamento.
- Comparar capturas do port com uma referência legítima da versão PS3 nas mesmas cenas.
- Diagnosticar problemas por draw call, shader, textura e estado RSX, sem depender apenas de logs de erro.

### FPS

1. Alcançar **30 FPS estáveis** com frame pacing consistente e sem regressões visuais.
2. Instrumentar CPU, SPU, GPU, compilação de shaders, streaming e tempos p95/p99.
3. Investigar **60 FPS como modo experimental**, somente após mapear vínculos de física, animação, scripts, temporizadores, QTEs e câmera com o frame rate.
4. Manter o modo de 30 FPS como referência funcional até comprovar que 60 FPS não altera a lógica.

> **Importante:** 60 FPS e 4K são metas condicionais, não capacidades garantidas pelo PS3Recomp nem pelo hardware.

## 4. Fases técnicas do projeto

### Fase 1 — Inventário e validação do jogo

- Confirmar **Title ID, região, versão, hashes e atualizações** do dump próprio.
- Identificar EBOOT, módulos PRX, bibliotecas, SPU, formatos de assets, vídeos e sistema de saves.
- Mapear a sequência de boot e as dependências do sistema PS3.
- Criar uma matriz de cenas de referência: boot, menu, cutscene, tutorial, primeira luta, chefe e salvamento.
- Registrar problemas originais separadamente dos novos problemas do port.

**Entrega:** manifesto de build e matriz de compatibilidade inicial.

### Fase 2 — Primeiro boot nativo

- Configurar pipeline de recompilação PPU e integração ao runtime.
- Resolver chamadas LV2 e serviços necessários.
- Inicializar RSX/Metal, áudio, entrada e gerenciamento de memória.
- Atingir tela inicial e menus sem travamentos.
- Instrumentar logs, dumps de estado, traces e detecção de freeze.

**Entrega:** boot reproduzível no Apple Silicon.

### Fase 3 — Primeira cena jogável

- Corrigir shaders, texturas, depth, render targets e sincronização PPU/SPU.
- Implementar controles SDL e testar combate, câmera e menus.
- Corrigir vídeos, áudio e transições entre cenas.
- Validar carregamento, streaming e saves.

**Entrega:** tutorial ou primeira fase jogável.

### Fase 4 — Campanha completa

- Testar fase por fase, chefes, QTEs, mortes, checkpoints e finais.
- Investigar memory leaks, deadlocks, corrupção de saves e regressões de rendering.
- Medir FPS e uso de CPU/GPU em áreas pesadas.
- Criar testes automatizados onde viável, com checkpoints reproduzíveis e hashes de estado.

**Entrega:** campanha concluível com 30 FPS como alvo, sujeito à viabilidade medida.

### Fase 5 — Enhanced Edition

- Implementar patches opcionais de câmera, checkpoints e acessibilidade.
- Corrigir problemas de movimento e resposta de entrada.
- Oferecer balanceamento alternativo para chefes e encontros, preservando a experiência original.
- Adicionar menu de combos, configurações e melhorias visuais.
- Testar novamente toda a campanha com cada combinação relevante de patches.

**Entrega:** build aprimorada com modo original e modo enhanced.

## 5. Prioridades

### Essenciais

- [ ] Boot nativo no macOS arm64
- [ ] Renderização fiel no Metal
- [ ] Gameplay completo e saves confiáveis
- [ ] 30 FPS estáveis e frame pacing
- [ ] Câmera corrigida e eixos configuráveis
- [ ] Checkpoints menos frustrantes, sem quebrar scripts
- [ ] Controles SDL configuráveis

### Importantes

- [ ] Melhor resposta de movimento e saltos
- [ ] Melhor feedback de combos
- [ ] Balanceamento opcional dos chefes
- [ ] Cutscenes puláveis com segurança
- [ ] Diagnóstico e correção de áudio

### Opcionais / experimentais

- [ ] Modo 60 FPS após auditoria de lógica
- [ ] 1440p/4K conforme desempenho
- [ ] SMAA e opções de filtragem
- [ ] Ajustes em encontros e variedade de inimigos
- [ ] Melhorias de texturas quando houver assets adequados e permissão de uso

## 6. Organização sugerida do repositório

```text
afro-samurai-recomp/
├── README.md
├── docs/
│   ├── compatibility.md
│   ├── game-manifest.md
│   ├── original-issues.md
│   ├── renderer-diagnostics.md
│   ├── performance.md
│   └── test-matrix.md
├── patches/
│   ├── camera/
│   ├── checkpoints/
│   ├── gameplay/
│   └── ui/
├── mac/
│   ├── build.sh
│   └── play.sh
├── port/
│   ├── runtime/
│   ├── renderer/
│   └── input/
└── tests/
```

Esta é uma estrutura proposta, não uma descrição de arquivos existentes.

## 7. Estratégia de testes e regressão

Para cada problema:

1. Registrar **versão do jogo, cena, passos para reproduzir e comportamento esperado**.
2. Capturar logs, FPS, p95/p99, estado relevante e imagens comparativas.
3. Isolar se o defeito existe no PS3 original ou foi introduzido pelo port.
4. Corrigir a menor camada possível: runtime, renderer, entrada, patch de lógica ou assets.
5. Testar com patch ligado/desligado e repetir em cenas relacionadas.
6. Atualizar documentação e manter um teste de regressão.

## 8. Riscos e limitações

- O jogo pode depender de recursos específicos de PPU, SPU, RSX ou middleware ainda não suportados pelo runtime atual.
- A engenharia reversa da câmera, checkpoints e scripts pode ser mais difícil do que a recompilação inicial.
- Alterações em física e temporização podem quebrar QTEs, chefes e animações.
- Mods de texturas e assets precisam respeitar direitos de uso; o projeto deve exigir os arquivos do jogo obtidos legitimamente pelo próprio usuário e não redistribuir conteúdo protegido.
- O esforço e o prazo só podem ser estimados com confiança após o inventário e o primeiro boot.

## 9. Primeiro marco recomendado

**Meta inicial:** executar a versão PS3 de Afro Samurai nativamente no M4 Pro, com Metal, controles SDL, menus e primeira luta renderizados corretamente. Em seguida, estabilizar 30 FPS e investigar a câmera. Só depois iniciar o pacote de correções históricas.

## 10. Referências para pesquisa

Consultar análises de época e confrontar suas críticas com testes práticos da versão PS3:

- GameSpot — *Afro Samurai Review*.
- Game Informer — cobertura e análise de *Afro Samurai* (2009).
- Push Square — análise de *Afro Samurai* para PS3.
- PSX Extreme — análise histórica de *Afro Samurai*.

> Estas referências são pontos de partida para verificação. Não substituem testes do executável e não significam que todos os problemas tenham sido reproduzidos.

# AntiVector — Índice Principal (fonte de verdade)

Projeto: vetorizador automático raster→vetor, qualidade de referência Vector Magic. Estado inicial: workspace vazio (2026‑10‑28). Nenhum código existe.

## Ordem de leitura (obrigatória)

| # | Documento | Conteúdo |
|---|-----------|----------|
| 1 | [../AGENTS.md](../AGENTS.md) | Regras operacionais para o implementador (Codex) |
| 2 | [01_PRODUCT.md](01_PRODUCT.md) | Visão, escopo, requisitos, fluxos, critérios de sucesso |
| 3 | [02_RESEARCH_DECISIONS.md](02_RESEARCH_DECISIONS.md) | Pesquisa, alternativas, escolhas, licenças, riscos |
| 4 | [03_ARCHITECTURE.md](03_ARCHITECTURE.md) | Componentes, repositório, contratos de dados, erros, ADRs |
| 5 | [04_ENGINE.md](04_ENGINE.md) | Pipeline de vetorização completo e parâmetros |
| 6 | [05_UI_UX.md](05_UI_UX.md) | Telas, controles, estados |
| 7 | [07_QUALITY_VALIDATION.md](07_QUALITY_VALIDATION.md) | Corpus, métricas, regressão, benchmarks |
| 8 | [06_IMPLEMENTATION_PLAN.md](06_IMPLEMENTATION_PLAN.md) | Fases, tarefas, critérios de aceite, DoD |
| 9 | [PROGRESS.md](PROGRESS.md) | Registro vivo de progresso (atualizado pelo implementador) |
| 10 | [08_CLI.md](08_CLI.md) | Uso da CLI e opções de exportação implementadas em F6 |

## Ordem de execução
Seguir as fases de [06_IMPLEMENTATION_PLAN.md](06_IMPLEMENTATION_PLAN.md) estritamente: F0 → F1 → … → F9. Nenhuma fase começa sem os critérios de aceite da anterior passarem.

## Convenções de status de decisão
- **[DEFINITIVO]** — não alterar sem novo ADR justificado em `03_ARCHITECTURE.md §9`.
- **[HIPÓTESE]** — valor/abordagem inicial; validar com o harness de qualidade (F3+).
- **[EXPERIMENTO]** — exige comparação A/B documentada antes de fixar.

## Glossário
- **Região**: conjunto conexo (4‑conexo) de pixels com o mesmo rótulo após segmentação.
- **Aresta compartilhada (edge)**: fronteira entre exatamente duas regiões (ou região e exterior), entre dois **junções**.
- **Junção**: vértice do grid onde ≥3 regiões se encontram (ou ponto de início de um laço fechado).
- **Grafo planar de fronteiras (BoundaryGraph)**: estrutura com regiões, arestas e junções; base de todo o ajuste de curvas.
- **Nós**: pontos âncora de Bézier no SVG final.

# AGENTS.md — Regras para o implementador

1. Fonte de verdade: `docs/` (comece por `docs/00_INDEX.md`). Em conflito, vale: ADR mais recente > 04_ENGINE > 03_ARCHITECTURE > demais.
2. Implemente fase a fase conforme `docs/06_IMPLEMENTATION_PLAN.md`. Não avance sem passar os critérios de aceite da fase.
3. Ao concluir cada tarefa/fase: atualize `docs/PROGRESS.md` (data, tarefa, status, resultados reais de testes/métricas, desvios).
4. Itens [DEFINITIVO] só mudam com novo ADR em `docs/03_ARCHITECTURE.md §9` contendo motivo, evidência e impacto.
5. Itens [HIPÓTESE]/[EXPERIMENTO]: decida com dados do harness (`cargo run -p av-bench`) e registre resultados.
6. Nunca invente resultados de testes ou benchmarks. Registre apenas números medidos.
7. Comandos de verificação obrigatórios antes de declarar uma fase pronta:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - (a partir de F7) `pnpm -C app lint && pnpm -C app test && pnpm -C app build`
8. Proibido: linkar código GPL/AGPL (ex.: potrace) no produto; copiar código ou assets do Vector Magic.
9. Prossiga autonomamente; só interrompa por bloqueio real (ambiente, decisão impossível de tomar com os docs).

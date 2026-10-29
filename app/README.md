# AntiVector desktop

React 18/TypeScript, Zustand e Tauri 2. O motor permanece em Rust; o frontend
recebe caminhos vetoriais e eventos com identificadores de sessão/requisição.

Na raiz do repositório, com Node 22, pnpm 11.25 e Rust stable:

```text
pnpm -C app install --frozen-lockfile
pnpm -C app tauri dev
pnpm -C app lint
pnpm -C app test
pnpm -C app build
pnpm -C app tauri build --no-bundle
```

O último comando gera o executável offline em target/release. Tauri também
exige as bibliotecas de desenvolvimento nativas da plataforma, instaladas
explicitamente na matriz de CI. O modo dev usa localhost:1420; o produto
empacotado carrega recursos internos sem servidor HTTP.

Abra uma imagem e aguarde a vetorização automática. Compare nas quatro vistas,
ajuste os parâmetros e exporte pelo diálogo nativo. Escape cancela mantendo o
último resultado concluído; Ctrl+O abre e Ctrl+E exporta. A roda amplia e o
arraste move as vistas juntas. Imagens acima de 16 MP oferecem redução antes
de continuar, até o limite de entrada de 64 MP.

QA local opcional: compilar com VITE_AV_MEASURE_UI=1 e iniciar o executável com
AV_UI_METRICS_PATH apontando para um JSONL local. Registra PerformanceObserver
longtask, duração de desenho e visibilidade; sem essas variáveis, não grava
métricas. O teste Vitest mantém seu cache temporário em target/vitest-tmp para
compatibilidade com o sandbox Windows.

Estado das fases e evidências reais em ../docs/PROGRESS.md. Edição de regiões,
histórico e projetos pertencem à F8; acessibilidade/i18n/release à F9.

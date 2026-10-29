# F9 — Reprodução e gates de release

F9 permanece em andamento até todos os critérios de06/01 passarem. Não publicar
um release como aprovado a partir de um build ou de uma avaliação parcial.

## Corpus e qualidade

`av-bench gen` conserva o corpus mínimo histórico de34 entradas para testes
rápidos. O corpus versionado completo contém73 entradas:
20synthetic,15logo,10lineart,10illustration,5pixelart,5photo e8edge.

Para reproduzir o completo, executar `./scripts/fetch-photo-corpus.ps1` uma vez
com rede e `cargo run -p av-bench --release -- gen --full`. Fontes fotográficas
originais, hashes SHA256 oficiais e evidência de licença ficam em
`corpus/sources/scikit-image-v0.25.2/`. O gerador usa thumbnails≤384px para as
fotos; essas imagens não são referências de desempenho. Não há conteúdo externo
no aplicativo distribuído.

```text
cargo run -p av-bench --release -- run --config corpus --label f9-complete --check-baseline corpus/baselines/metrics.json
cargo run -p av-bench --release -- baseline vtracer --config corpus --label f9-reference
cargo run -p av-bench --release -- gate-quality bench-out/f9-complete/metrics.json bench-out/f9-reference/metrics.json
cargo run -p av-bench --release -- visual f9-complete f9-reference --label 2026-10-29-final
cargo run -p av-bench --release -- score-visual 2026-10-29-final bench-out/visual/2026-10-29-final.json --candidate f9-complete
```

`--config corpus` usa o preset da categoria; synthetic/edge/logo mantêm logo.
Cada linha guarda os parâmetros efetivamente usados. O arquivo corrompido tem
`expected_decode_error:true`: a execução exige erro recuperável e registra
`input_checks`, sem fabricar métricas ou imagem vetorizada para bytes inválidos.
As outras72 entradas têm render/fidelidade/cobertura na resolução de entrada,
incluindo64MP. Métricas de imagens byte-idênticas dispensam os buffers gaussianos
SSIM, mas continuam contando a cobertura real em todos os pixels.

A avaliação humana contém72pares válidos, com lados/ordem embaralhados e chave
separada. Somente o usuário preenche as escolhas. O JSON precisa conter todos os
pares e o score deve ser calculado contra a chave correspondente. Não regenerar
o rótulo enquanto o usuário estiver preenchendo a página.
O scorer exige rótulo/seed/quantidade corretos, todos os pares respondidos uma
vez e escolhas A/B/tie válidas; respostas parciais não produzem aceite.

## Desempenho, robustez e UI

```text
cargo run -p av-bench --release -- rnf --label f9-rnf-final
cargo flamegraph -p av-bench -- profile corpus/baselines/performance/f9/before/illustration-4mp-32.png
```

RNF usa cinco processos novos por referência,8workers Rayon e mediana de tempo
do raster já decodificado até SVG, incluindo detecção. Memória local Windows:
PeakWorkingSet64 do SO, pico vitalício do processo novo; consulta após o trecho
cronometrado. Limites conservadores decimais:1000/4000ms e40bytes/pixel+200MB.
As cenas geométricas originais e todos os resultados individuais são preservados;
não equivalem a uma garantia de pior caso para qualquer foto ou histórico.
Executar sem fuzz, builds ou outras cargas pesadas concorrentes.

O comando `profile` repete a referência cinco vezes e é portátil; tempos sob
amostragem não são evidência RNF. Windows local exigiu token administrador para
o backend de flamegraph; a coleta alternativa está no jobCPU flamegraph do CI.

Fuzz: instruções e limites em[fuzz/README.md](../fuzz/README.md). A política de
licenças da ferramenta é independente, conforme ADR-014. Logs/resultados reais
ficam em `corpus/baselines/robustness/f9/`.

Para QA nativa offline, construir com `VITE_AV_MEASURE_UI=1` e iniciar o binário
com `AV_UI_METRICS_PATH` apontando para um JSONL local. A Performance API mede
longtasks≥50ms, maior desenho Canvas e visibilidade; não há telemetria. Rust
debug e frontend production são identificados no relatório e não provam o
tempo release RNF01/02.

## Instaladores e avisos

CI gera MSI/DMG/AppImage como artefatos de workflow, sem publicar release.
`scripts/check-installers.mjs` exige o formato da plataforma, arquivo não vazio,
tamanho≤25.000.000bytes e registra SHA256. Falhas de tamanho não são dispensadas.

Windows usa WebView2 já instalado no sistema (`webviewInstallMode:skip`), para
o instalador não baixar runtime. A aplicação usa o WebView do SO; informar o
pré-requisito na distribuição. Não há runtime de fuzz/Ghostscript/vtracer no
produto. Assinatura de instaladores não está configurada; artefatos de CI não
são uma publicação assinada.

`node scripts/third-party-notices.mjs` reconstrói os avisos de dependências
Cargo do desktop e JavaScript de produção, com hashes dos textos e URLs dos
fontes originais. Textos ausentes no pacote publicado ficam em `third-party/`,
com proveniência versionada. A ausência de qualquer texto falha a geração.
O script de coleta usa os commits publicados; os antigos subpacotes winapi0.4.0
sem metadados VCS são conferidos na revisão do acompanhante winapi0.3.9, que
ainda declara essas mesmas versões. Nenhum fonte de terceiro foi modificado.
Os avisos e o manifest são recursos empacotados em `notices/`.

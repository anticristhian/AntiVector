# F9 — Reprodução e gates de release

F9 permanece em andamento até todos os critérios de06/01 passarem. Não publicar
um release como aprovado a partir de um build ou de uma avaliação parcial.

## Primeiro candidato — resultados preservados (2026-10-29)

| Gate | Resultado registrado | Evidência |
|---|---|---|
| Corpus/cobertura |72 válidas sem gaps;1 corrupção rejeitada |`corpus/baselines/quality/f9/metrics.json` |
| Fidelidade/compactação |Limites absolutos aprovados;20/25 logos/ilustrações vencem ou empatam em nós e fidelidade |`corpus/baselines/quality/f9/` |
| RNF01/02 |Logo1MP:282,902ms;ilustração4MP:997,3239ms (mediana de5) |`corpus/baselines/performance/f9/final/metrics.json` |
| RNF03 |Picos67.817.472/235.687.936bytes nas duas referências |Mesmo relatório RNF |
| UI |Abertura4MP:zero longtasks≥50ms,drawmáximo0,5ms;frontendproduction/Rustdebug |`corpus/baselines/session/f9/ui-final.jsonl` |
| Fuzz |90.151 execuções/3.601s,sem crash/panic |`corpus/baselines/robustness/f9/fuzz.txt` |
| PerfilCPU |23.038 eventos perf;quantização32,28% do peso amostrado |`corpus/baselines/performance/f9/profile-ci.json` e SVG |
| Instaladores |MSI5.099.520bytes,DMG4.764.463bytes,AppImage11.282.936bytes;smokeLinux aprovado;matriz final verde |Relatórios `installer-*-final.json` em `corpus/baselines/performance/f9/`;`corpus/baselines/quality/f9/ci.json` |
| Preferência humana≥80% |REPROVADO:25vitórias/19empates/28derrotas,44/72=61,11% |`corpus/baselines/quality/f9/human-2026-10-29/score.json` |

Esses resultados têm os limites de método descritos nas seções seguintes.
Não substituem gates pendentes nem garantem pior caso em qualquer entrada.

A primeira avaliação humana terminou abaixo do limiar. F9 continua aberta para
melhoria de qualidade; não falta preencher essa avaliação. As respostas e a
chave originais são preservadas, sem regenerar o rótulo. Uma alteração de saída
não herda os votos antigos como se tivesse sido avaliada.

Diagnóstico reproduzível de composição AA (sem mudar o produto):
`cargo run -p av-bench --release --example opaque_matte_probe`.
O experimento usa os SVGs de `bench-out/f9-complete`, mede seis casos e salva em
`bench-out/f9-opaque-matte-probe`; a cobertura vem do SVG original, sem o fundo
experimental. Melhorar essas métricas não aprova a preferência humana nem a
integração com os demais formatos. Resultados/detalhes em PROGRESS.

## Corpus e qualidade

### Candidato de correção após a primeira avaliação

O candidato atual combina composição opaca e ajuste circular isolado (ADR-017),
ambos opt-in e com default false. O ajuste circular preserva todas as junções
reais e só permite deslocar a costura artificial de um laço circular isolado.
Não altera elipses, pixel-art ou contornos com junções reais. Reprodução:

```text
cargo run -p av-bench --release -- run --config corpus --opaque-canvas --circular-loops --label f9-circles-candidate --check-baseline corpus/baselines/metrics.json
cargo run -p av-bench --release -- compare f9-complete f9-circles-candidate --check
cargo run -p av-bench --release -- gate-quality bench-out/f9-circles-candidate/metrics.json bench-out/f9-reference/metrics.json
cargo run -p av-bench --release -- gate-curves bench-out/f9-circles-candidate/metrics.json bench-out/f9-reference/metrics.json
cargo run -p av-bench --release -- visual f9-circles-candidate f9-reference --label 2026-10-29-revision2 --seed 107
cargo run -p av-bench --release -- score-visual 2026-10-29-revision2 bench-out/visual/2026-10-29-revision2.json --candidate f9-circles-candidate
cargo run -p av-bench --release -- rnf --opaque-canvas --circular-loops --label f9-revision2-rnf
```

A página revision2 já existe e foi avaliada; não regenerar. Score humano:
43vitórias/16empates/13derrotas,59/72=81,944444%,aprovado. Par7 transcrito
como empate por escolha explícita do usuário;original parcial e proveniência
preservados em quality/f9/human-2026-10-29-revision2/. Resultados numéricos:22/25 (88%) no
gate de logos/ilustrações;Hausdorff médio0,421196691556px,corner recall
0,988095238095,18/20 synthetic (90%),zero gaps. Evidências em
`corpus/baselines/quality/f9/revision2/`. A tabela inicial descreve o produto
anterior; o estado deste candidato é:

| Gate do candidato corrigido | Estado medido |
|---|---|
| Regressão |34 históricas e72 completas aprovadas,zero gaps |
| Qualidade/geometria |22/25 logos/ilustrações e18/20 synthetic;limites preservados |
| Verificações |75 testes Rust em cada SO;17 Vitest;fmt/clippy/lint/build/WASM/licenças aprovados |
| CI |Success nos5jobs do run40968541146,SHAbe26f37;ci.json em revision2 |
| Instaladores |MSI5.107.712bytes,DMG4.772.847bytes,AppImage11.291.128bytes;hashes ZIP/arquivo conferidos e smokeLinux aprovado |
| Fuzz combinado |ASAN75.053execuções/3.601s,exit0,sem crash/panic;robustness/f9/revision2 |
| RNF01–03 combinado |Logo371,8948ms/67.887.104bytes;ilustração1.397,4143ms/235.745.280bytes;5processos novos,8workers,aprovado |
| QA nativa da correção |Após worker de bitmap:abertura/revetorização4MP com0longtasks≥50ms,drawmáximo0,4ms;primeira ocorrência60ms preservada |
| Preferência humana≥80% |APROVADO:43vitórias/16empates/13derrotas,59/72=81,944444%;score/proveniência preservados |

ADR-018 adota `experimental.opaque_canvas_overlap` e
`experimental.circular_loops` como true nas novas configurações,presets e
composição SVG;false explícito mantém o controle anterior. Campos ausentes em
objetos experimental antigos desserializam false,preservando projetos.
Novo run sem flags produziu72SVGs byte-idênticos aos avaliados;prova em
quality/f9/adoption/default-identity.json. Não preencher votos pelo implementador.
CI desta promoção ainda precisa concluir;runbe26f37 descreve o candidato anterior.

Controle na mesma sessão,com ambos flags desligados:logo353,3196ms e
ilustração1.532,2126ms. Variações do candidato+5,2573%/−8,7976%,abaixo do
limite de regressão15%;ambos passam as metas RNF. Dados completos,parâmetros e
hashes das entradas em performance/f9/revision2/rnf-*.json. Essas medições
excluem decodificação e UI e não garantem pior caso para qualquer entrada.

O experimento de composição isolada abaixo continua preservado como histórico.

A correção de composição está em `experimental.opaque_canvas_overlap`,false
por padrão. O harness pode ligá-la explicitamente com `--opaque-canvas`:

```text
cargo run -p av-bench --release -- run --config corpus --opaque-canvas --label f9-composition-candidate --check-baseline corpus/baselines/quality/f9/metrics.json
cargo run -p av-bench --release -- compare f9-complete f9-composition-candidate --check
cargo run -p av-bench --release -- gate-quality bench-out/f9-composition-candidate/metrics.json bench-out/f9-reference/metrics.json
cargo run -p av-bench --release -- visual f9-composition-candidate f9-reference --label 2026-10-29-revision1 --seed 73
cargo run -p av-bench --release -- score-visual 2026-10-29-revision1 bench-out/visual/2026-10-29-revision1.json --candidate f9-composition-candidate
cargo run -p av-bench --release -- rnf --opaque-canvas --label f9-revision1-rnf
```

Não regenerar rótulos existentes:o gerador recusa qualquer diretório/JSON de
avaliação já existente. A página nova mostra o Original decodificado e vetores
A/B com zoom real100/400,rolagem sincronizada e rascunho local quando disponível.
O JSON exportado continua obrigatório. Comentários dos geradores são removidos
sem reescrever coordenadas;igualdade do render em resolução de fonte é exigida.
`node scripts/check-visual-page.mjs` testa apenas a avaliação sintética de3pares
gerada por cargo test -p av-bench;nunca deve preencher a avaliação humana real.

Na CLI do produto,usar params.json com `experimental.opaque_canvas_overlap:true`.
Projetos preservam esse campo;na UI,prévia/exportação/SSIM estimado seguem o
parâmetro da revisão. O exemplo opaque_canvas_ab salva um projeto opt-in de QA
em bench-out/f9-opaque-canvas-ab/photo_chelsea-optin.avproj. Sem opt-in,o produto
mantém a composição anterior até os gates,incluindo a revisão humana.

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

Linux: ADR-015 conserva GTK3/WebKitGTK4.1 no SO. A baseline testada é Ubuntu24.04;
instalar `libwebkit2gtk-4.1-0`, `libgtk-3-0t64`, `libxdo3` e
`libayatana-appindicator3-1`. Em uma sessão desktop normal, marcar o AppImage
como executável e abrir. Para executar sem depender de montagem FUSE:
`APPIMAGE_EXTRACT_AND_RUN=1 ./AntiVector_0.1.0_amd64-system-webview.AppImage`.
Esse modo usa extração temporária local, sem rede. Não há garantia para todas
as distribuições Linux ou versões anteriores à baseline.

`scripts/lean-appimage.sh` preserva o AppImage grande fora do diretório de
distribuição, cria o contêiner menor e inclui fontes/avisos do bootstrap separado.
`scripts/smoke-appimage.sh` exige evento real de página carregada sob Xvfb/D-Bus;
o timeout30s/exit124 encerra a janela deliberadamente e não substitui o marcador.
O workflow diagnóstico reutiliza um artefato identificado por run-id, para
investigar o ambiente sem reconstruir o produto. Ele não substitui o build e
as verificações da matriz principal. Log/marcador/tamanho são artefatos do CI.

`node scripts/third-party-notices.mjs` reconstrói os avisos de dependências
Cargo do desktop e JavaScript de produção, com hashes dos textos e URLs dos
fontes originais. Textos ausentes no pacote publicado ficam em `third-party/`,
com proveniência versionada. A ausência de qualquer texto falha a geração.
O script de coleta usa os commits publicados; os antigos subpacotes winapi0.4.0
sem metadados VCS são conferidos na revisão do acompanhante winapi0.3.9, que
ainda declara essas mesmas versões. Nenhum fonte de terceiro foi modificado.
Os avisos e o manifest são recursos empacotados em `notices/`.

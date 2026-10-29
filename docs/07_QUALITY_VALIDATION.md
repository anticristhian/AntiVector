# 07 — Qualidade e Validação

## 1. Corpus (`corpus/`)
`corpus/manifest.json`: `[{id, file, category, source, license, expected_colors?, notes}]`. Só imagens de domínio público/CC0/CC‑BY ou **geradas sinteticamente** pelo `av-bench gen` (determinístico).
| Categoria | Mín. | Exemplos |
|---|---|---|
| synthetic | 20 | círculos, quadrados rotacionados, estrelas, texto renderizado, anéis concêntricos, junções T/Y, linhas de 1 px, com/sem AA, com ruído JPEG q=60 (geradas, com ground‑truth SVG) |
| logo | 15 | logos planos 2–12 cores, PNG e JPEG |
| lineart | 10 | desenhos P&B, quadrinhos, assinaturas escaneadas |
| illustration | 10 | clipart, ícones, ilustrações flat com AA |
| pixelart | 5 | sprites |
| photo | 5 | retratos/paisagens (posterização) |
| edge | 8 | 1×1, 1×N, totalmente transparente, uniforme, 64 MP, PNG 16‑bit, GIF animado, arquivo corrompido |
Fontes sugeridas: Wikimedia Commons (PD/CC0), OpenClipart (CC0), Kenney (CC0 pixel art). Registrar licença no manifest.

## 2. Métricas (`av-metrics`)
Renderizar o SVG gerado com resvg na resolução original (AA ligado para fidelidade; desligado para teste de frestas).
| Métrica | Definição | Uso |
|---|---|---|
| ΔE00 médio / p95 | CIEDE2000 por pixel opaco | Fidelidade de cor |
| SSIM | Luminância, janela gaussiana 11, σ 1,5 | Estrutura |
| Gap pixels | Render sem AA sobre magenta: nº pixels magenta onde original é opaco | Deve ser 0 |
| Nós | Soma de pontos âncora de todos os paths | Compacidade |
| Paths | nº de `<path>` | Compacidade |
| Hausdorff (synthetic) | Distância entre contornos do SVG e do ground truth (amostrados a 0,25 px) | Precisão geométrica |
| Corner recall (synthetic) | Cantos do GT encontrados a ≤ 1 px / total | Preservação de cantos |
| Tempo por estágio, pico de memória | `tracing` + medição de RSS (crate `memory-stats`, MIT/Apache) | RNF01–03 |
| Tamanho SVG | bytes | Saída |
Score composto para ranking A/B: `Q = SSIM − 0,02·ΔE00médio − 0,05·log10(nós)` [HIPÓTESE; validar contra avaliação visual].

## 3. Harness (`av-bench`)
- `av-bench gen` — gera synthetic + GT.
- `av-bench run --config <preset|params.json> --label <nome>` — roda corpus, salva `bench-out/<label>/{svgs, renders, metrics.json}`.
- `av-bench baseline vtracer` — roda vtracer (crate) com nº de cores equivalente.
- `av-bench compare A B` — relatório HTML com tabela por imagem, deltas, miniaturas lado a lado e mapas de diferença; destaca pioras > limites §4.
- `av-bench perf` — 5 repetições, mediana, reporta por estágio.
- `av-bench gate-quality candidate/metrics.json reference/metrics.json` — critérios F5 no corpus atual; exige paletas/cores equivalentes e nós+SSIM+ΔE00 pelo menos equivalentes em ≥80% de logo/illustration.
- `av-bench visual A B --label AAAA-MM-DD-f5 --seed 42` — pares anônimos, ordem/lados embaralhados, HTML com zoom100/400%, chave separada e JSON sem respostas. O botão salva escolhas humanas; copiar o download para `bench-out/visual/<label>.json`.

Em F5, vtracer recebe a mesma segmentação após resolução AA/merge e compactação
da paleta, não uma paleta com cores eliminadas pelo AntiVector. Configuração,
escala, stacking e parâmetros resolvidos por imagem constam nas métricas.

## 4. Regressão
- `corpus/baselines/metrics.json` versionado. CI (job `quality`, corpus mínimo synthetic+logo) falha se para qualquer imagem: SSIM cair > 0,005, ΔE00 subir > 0,3, nós subir > 5%, gap > 0, ou tempo subir > 15% (perf só local/nightly).
- Melhorias atualizam baseline com commit dedicado "bench: update baseline" incluindo o relatório compare.
- Snapshots `insta` para 10 SVGs sintéticos (mudança exige revisão consciente).

## 5. Validação de exportação
- SVG: re‑parse com usvg sem erros; render idêntico (SSIM ≥ 0,999) ao render interno; validar estrutura (viewBox, width/height, sem NaN, precisão decimal).
- PDF: parse estrutural com `lopdf` (MIT, dev‑dependency); checar contagem de paths; conferência visual manual por marco.
- EPS: validação estrutural (cabeçalho DSC, BoundingBox); visual manual com Ghostscript **local** (não distribuído).
- DXF: parse com crate `dxf` (MIT) e contagem de entidades.

F6: `cargo test -p av-export --test formats` percorre o manifest inteiro e grava
os cinco formatos e `summary.json` em `target/f6-export-tests/`. PNG e SVG têm
render RGBA idêntico. PDF é reaberto com lopdf, EPS tem DSC/BoundingBox verificados,
DXF é reaberto com dxf e deve conter uma POLYLINE por fronteira compartilhada.
Há snapshots adicionais de EPS, DXF e stream vetorial PDF. O teste de subdivisão
DXF amostra 10.001 pontos de uma cúbica com inflexão contra a polilinha.

Conferência visual local: Poppler `pdftoppm -scale-to-x W -scale-to-y H -png
-singlefile entrada.pdf saída` e Ghostscript externo `-dSAFER -dBATCH -dNOPAUSE
-sDEVICE=pngalpha -dGraphicsAlphaBits=4 -r96 -dEPSCrop`. W/H são os pixels da fonte;
o modo somente `-r96` do Poppler 26.07 arredondou 1px para2px em dois edge cases,
apesar de MediaBox=0,75pt correto. Ghostscript não integra dependências/produto.
`av-bench compare-renders target/f6-export-tests <pasta-renders> --label <nome>`
salva SSIM/ΔE00 por imagem; diferenças entre AA de renderizadores não substituem
o gap test sem AA nem a avaliação visual humana cega de F5/F9.

## 6. Avaliação visual cega (por marco F5, F9)
Script do harness gera página HTML com pares (A/B embaralhados) de AntiVector × vtracer (e, se disponível localmente, potrace/Inkscape) a 100% e 400%. Avaliador humano escolhe melhor/empate; resultados salvos em `bench-out/visual/<data>.json`. O implementador **não** preenche esta avaliação; registra como pendente em PROGRESS.md para o usuário.

## 7. Diagnóstico de resultados inferiores
Ordem de investigação quando uma imagem piora/fica abaixo do vtracer:
1. Ver mapa de diferença → erro concentrado em bordas (ajuste) ou em áreas (cor/segmentação)?
2. Área: inspecionar dump do LabelMap (`--dump-stages` salva PNG de cada estágio) → quantização (nº de cores, snap) ou AA.
3. Bordas: vista Contorno → cantos perdidos (alphamax/corner threshold) ou excesso de nós (tolerância/DP).
4. Confete: estatística de regiões por área → `min_region_area`, merge cost.
5. Registrar caso no manifest com `notes` e adicionar ao corpus de regressão.

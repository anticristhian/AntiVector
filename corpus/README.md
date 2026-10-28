# Corpus inicial

Gerado deterministicamente por `cargo run -p av-bench --release -- gen`.
São 20 casos synthetic com SVG de referência, cinco logos geométricos originais
e quatro casos edge. As imagens não reproduzem marcas ou arte de terceiros.
Os assets gerados são disponibilizados como CC0-1.0; fontes e licenças constam
do manifest. O corpus completo de 07_QUALITY_VALIDATION.md ainda é tarefa de F9.

`baselines/metrics.json` contém o AntiVector poligonal inicial;
`baselines/vtracer.json`, o vtracer 0.6.5 com a mesma paleta pré-quantizada,
precisão 8 bits, speckle=0, layer_difference=0, modo cutout.
Essa configuração documenta uma comparação com nº de cores equivalente;
não é alegada como a melhor configuração possível do concorrente.

`baselines/experiments/` preserva as métricas, cinco repetições por variante,
calibração do detector e relatórios HTML com assets relativos. Os tempos são
medições locais release em Windows x86_64, sujeitos a ruído; não são metas RNF
validadas. RSS é amostrado a cada 2ms e inclui o processo inteiro.

Comandos principais:

```text
cargo run -p av-bench --release -- run --config logo --label candidate
cargo run -p av-bench --release -- baseline vtracer --label reference
cargo run -p av-bench --release -- compare reference candidate
cargo run -p av-bench --release -- perf --config logo --label perf
cargo run -p av-bench --release -- run --label regression --check-baseline corpus/baselines/metrics.json
```

Os gaps são medidos pela cobertura alfa de um render sem AA (resvg CrispEdges),
equivalente a detectar fundo magenta vazando, mas sem confundir cores magenta
legítimas da imagem com frestas. Fidelidade usa render AA e composição sobre
branco. SSIM usa luminância sRGB normalizada, janela gaussiana 11, sigma 1,5,
com replicação das bordas para suportar inclusive imagens 1×1.

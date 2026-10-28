# 02 — Pesquisa e Decisões Técnicas

> Legenda de evidência: **(C)** comprovado por documentação/literatura; **(H)** hipótese a validar experimentalmente.

## 1. Métodos de vetorização investigados
| Método | Como funciona | Pontos fortes | Limitações |
|---|---|---|---|
| Potrace (Selinger 2003) | Bitmap 1‑bit → caminhos de pixel → polígono ótimo (penalidade) → Bézier com detecção de cantos (parâmetro alphamax) | Curvas excelentes, poucos nós (C) | Só binário; cor exige camadas empilhadas → frestas/sobreposição; GPL‑2 (C) |
| VTracer (VisionCortex) | Clustering hierárquico de cor → caminhos de cluster → remoção de escadas → splice + ajuste de curvas; empilhamento | Rápido O(n), cor, MIT (C) | Curvas por região (não compartilhadas) → micro‑frestas/desalinhamento; mais nós; ruído em AA (C/H) |
| Autotrace | Traçado por cor, centerline | Centerline | Qualidade inferior, manutenção fraca, GPL (C) |
| Inkscape Trace Bitmap | Potrace por camadas | Integrado | Mesmos limites do Potrace (C) |
| Vector Magic (proprietário) | Segmentação perceptual com remoção de anti‑aliasing; regiões com fronteiras compartilhadas; ajuste com otimização global; edição pós‑segmentação (C funcional, H interno) | Sem frestas, poucos nós, cantos corretos | Fechado, pago |
| Depixelização Kopf‑Lischinski 2011 | Grafo de similaridade de pixels, resolução de ambiguidades diagonais, B‑splines | Ótimo para pixel art (C) | Escala mal para fotos/grandes |
| DiffVG / LIVE / métodos neurais | Otimização diferenciável de Bézier vs. imagem | Alta fidelidade (C) | Lento (s‑min/imagem), GPU, instável na topologia (C) |
| Segmentação SLIC/Felzenszwalb/Mean‑shift | Superpixels/grafos | Boas regiões em fotos (C) | Precisa de pós‑mesclagem |

## 2. Desafios de qualidade identificados
1. **Frestas** entre regiões adjacentes quando cada região é ajustada independentemente (VTracer, Potrace multicor).
2. **Anti‑aliasing (AA)**: pixels de borda com cores intermediárias geram "halos" e micro‑regiões.
3. **Cantos vs. curvas**: escadas de pixel se confundem com cantos reais; ângulo puro não basta, é preciso escala.
4. **Excesso de nós**: ajuste local guloso sem otimização global.
5. **Detalhes finos**: linhas de 1–2 px somem com filtros agressivos.
6. **Junções em T/Y**: pontos onde 3+ regiões se encontram devem ser fixos e compartilhados.
7. **Ruído JPEG**: blocos 8×8 e ringing criam confete.

## 3. Estratégia escolhida (resumo) [DEFINITIVO]
**Motor próprio em Rust, baseado em grafo planar de fronteiras compartilhadas**:
pré‑processamento edge‑preserving → quantização perceptual (OKLab) com resolução de AA → rotulagem + mesclagem de regiões pequenas → extração do grafo de fronteiras no grid de vértices de pixel → por **aresta compartilhada** (não por região): polígono ótimo estilo Potrace (reimplementado a partir do artigo, não do código GPL) + detecção de cantos multi‑escala → ajuste Bézier com minimização global de nós (programação dinâmica) → montagem das regiões a partir das arestas ajustadas → exportação.
Cada aresta é ajustada **uma vez** e usada (invertida) pelas duas regiões vizinhas ⇒ frestas são impossíveis por construção.

Justificativa: combina a qualidade de curva do Potrace (C), a capacidade multicor do VTracer (C) e a topologia sem frestas observada no Vector Magic. IA não entra no caminho crítico v1 (custo/benefício ruim, ver §5); fica prevista como plugin experimental opcional de pré‑processamento (upscaling/denoise) pós‑v1.

## 4. Linguagens e plataforma
| Opção | Prós | Contras | Decisão |
|---|---|---|---|
| **Rust** (motor) | Desempenho nativo, memória segura, rayon, compila para WASM, crates maduros (image, kurbo) | Curva de aprendizado | **Escolhido** [DEFINITIVO] |
| C++ + OpenCV | OpenCV completo | Segurança de memória, build complexo, binário grande | Rejeitado |
| Python + NumPy | Prototipagem rápida | Lento, distribuição ruim | Rejeitado (inclusive no harness: um só toolchain Rust) |
| Go | Simples | Ecossistema gráfico fraco, sem SIMD fácil | Rejeitado |

| UI | Prós | Contras | Decisão |
|---|---|---|---|
| **Tauri 2 + React + TypeScript + Vite** | Binário pequeno (WebView do SO), Rust nativo no backend, UI moderna, canvas/SVG nativos | Diferenças entre WebViews | **Escolhido** [DEFINITIVO] |
| Electron | Maduro | 100+ MB, Rust só via N‑API | Rejeitado |
| egui/iced (Rust puro) | Um só idioma | UX profissional mais trabalhosa, texto/acessibilidade fracos | Rejeitado |
| Qt | Profissional | Licença LGPL/comercial, C++ | Rejeitado |
| Web pura (WASM) | Sem instalação | Memória/threads limitados p/ 64 MP | Adiado; motor permanece compilável a `wasm32` (ADR‑003) |

## 5. Uso de IA — avaliação
- Segmentação neural (SAM etc.): modelos de centenas de MB, fronteiras imprecisas em nível de pixel; não melhora logos. **Rejeitado v1** (H: pode ajudar fotos).
- Super‑resolução (Real‑ESRGAN) antes de vetorizar JPEGs pequenos: ganho plausível (H). **Experimento pós‑v1** como plugin ONNX opcional.

## 6. Bibliotecas selecionadas
| Crate/pacote | Uso | Licença |
|---|---|---|
| `image` | Decodificação PNG/JPEG/BMP/GIF/TIFF/WebP | MIT/Apache‑2.0 |
| `rayon` | Paralelismo de dados | MIT/Apache‑2.0 |
| `kurbo` | Primitivas Bézier, avaliação, bbox, distâncias | MIT/Apache‑2.0 |
| `resvg` + `usvg` + `tiny-skia` | Rasterizar SVG para métricas e pré‑visualização PNG | Apache‑2.0/MIT (verificar versão no `cargo deny`) |
| `serde`, `serde_json` | Contratos, presets, projeto | MIT/Apache‑2.0 |
| `thiserror`, `anyhow`(só bins) | Erros | MIT/Apache‑2.0 |
| `clap` | CLI | MIT/Apache‑2.0 |
| `pdf-writer` | Exportação PDF | MIT/Apache‑2.0 |
| `tracing` | Logs/timings por estágio | MIT |
| `criterion` | Microbenchmarks | MIT/Apache‑2.0 |
| `insta` | Snapshot tests | Apache‑2.0 |
| `proptest` | Testes de propriedade | MIT/Apache‑2.0 |
| `vtracer` (somente `av-bench`, dev) | Baseline comparativo | MIT |
| Front: `react`, `zustand`, `vite`, `vitest`, `@tauri-apps/api` | UI | MIT |

Implementação própria (não há crate adequada): quantização OKLab, rotulagem, grafo de fronteiras, polígono ótimo, ajuste Bézier, EPS/DXF writers, SSIM/ΔE00.

**Excluídos**: potrace (GPL), autotrace (GPL), OpenCV (peso, build; licença ok Apache‑2 mas desnecessário).
Potrace pode ser invocado como **executável externo opcional** apenas no harness de benchmark local, nunca distribuído.

`cargo-deny` com allowlist de licenças é obrigatório no CI (F0).

## 7. Riscos
| Risco | Prob. | Impacto | Mitigação |
|---|---|---|---|
| Qualidade de segmentação em fotos/AA insuficiente | Alta | Alto | Estágio de AA dedicado; harness A/B; parâmetros por modo |
| Polígono ótimo O(n²) lento em arestas longas | Média | Médio | Janela máxima de busca (k ≤ 200 vértices), divisão em junções/cantos, paralelismo por aresta |
| Diferenças de WebView entre SO (render SVG grande) | Média | Médio | Render de preview via canvas com Path2D por região + tiles; fallback PNG gerado por resvg |
| Determinismo com paralelismo | Média | Médio | Ordenação estável por ID antes de reduções; sem HashMap iterado na saída (usar BTreeMap/Vec) |
| Escopo de UI crescer | Média | Médio | UI só após motor estável (F6) |
| Métricas não refletirem percepção | Média | Médio | Avaliação visual cega obrigatória por marco |

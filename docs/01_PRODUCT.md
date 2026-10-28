# 01 — Visão e Requisitos do Produto

## 1. Visão
AntiVector é um aplicativo desktop (Windows, macOS, Linux) + CLI que converte imagens raster em SVG/PDF/EPS editáveis com qualidade comparável ao Vector Magic: regiões sem frestas, curvas suaves, cantos preservados e poucos nós. Motor próprio em Rust, offline, sem dependência de nuvem.

## 2. Escopo
### Dentro (v1.0)
- Entrada: PNG, JPEG, BMP, GIF (1º quadro), TIFF, WebP. Até 64 MP (limite configurável).
- Modos: **Logo/Arte** (paleta reduzida, bordas nítidas), **Ilustração** (mais cores, anti‑aliasing), **Foto‑estilizada** (posterização controlada), **Preto&Branco/Linha** (binário).
- Detecção automática de modo e nº de cores (sobreponível pelo usuário).
- Transparência: alfa preservado como região "transparente" (não exportada).
- Exportação: SVG 1.1 (default), PDF, EPS, DXF (só contornos), PNG re‑rasterizado.
- Edição pós‑vetorização: mesclar/remover cores da paleta, editar cor, remover regiões pequenas, borracha de região (clique remove/mescla região com vizinha), ajuste de suavidade/detalhe com re‑ajuste incremental.
- Visualização: lado a lado, sobreposição com slider, modo contorno (wireframe com nós), zoom até 6400%.
- CLI headless com os mesmos parâmetros (presets JSON).

### Fora (v1.0)
- Gradientes vetoriais, vetorização de centerline/stroke (planejada v1.1, ver 04_ENGINE §12), IA generativa, edição livre de Bézier nó‑a‑nó (v1.1), versão web (arquitetura permite via WASM, ver ADR‑003).

## 3. Requisitos funcionais
| ID | Requisito |
|----|-----------|
| RF01 | Importar por arrastar‑soltar, diálogo ou colar da área de transferência |
| RF02 | Detectar automaticamente tipo de imagem e sugerir modo + nº de cores |
| RF03 | Vetorizar com progresso por estágio e cancelamento a qualquer momento |
| RF04 | Pré‑visualização progressiva (resultado de baixa resolução em < 1 s para imagens ≤ 4 MP) |
| RF05 | Ajustar parâmetros (detalhe, suavidade, cantos, nº de cores, ruído mínimo) e re‑vetorizar reaproveitando estágios em cache |
| RF06 | Editar paleta: mesclar, excluir, recolorir, travar cor |
| RF07 | Ferramentas de região: remover, mesclar com vizinha, preencher |
| RF08 | Comparar original × vetor (lado a lado, slider, diferença) |
| RF09 | Exportar SVG/PDF/EPS/DXF/PNG com opções (agrupamento por cor, empilhamento vs. recorte) |
| RF10 | Desfazer/refazer (≥ 100 níveis) para todas as edições |
| RF11 | CLI: `antivector in.png -o out.svg --preset logo --colors 8` e lote por diretório |
| RF12 | Salvar/abrir projeto `.avproj` (imagem + parâmetros + edições) |
| RF13 | Exibir estatísticas: nº de caminhos, nós, cores, fidelidade estimada |

## 4. Requisitos não funcionais
| ID | Requisito | Meta |
|----|-----------|------|
| RNF01 | Desempenho logo 1 MP, 8 cores | ≤ 1,0 s (CPU 8 núcleos ref., release) |
| RNF02 | Desempenho ilustração 4 MP, 32 cores | ≤ 4 s |
| RNF03 | Memória pico | ≤ 40 bytes/pixel + 200 MB |
| RNF04 | UI responsiva | nunca bloquear thread de UI > 50 ms |
| RNF05 | Determinismo | mesma entrada+parâmetros ⇒ SVG byte‑idêntico (independente de nº de threads) |
| RNF06 | Robustez | nenhuma entrada (corrompida, 1×1, 64 MP, toda transparente) causa panic |
| RNF07 | Offline/privacidade | zero rede |
| RNF08 | Licenciamento | apenas dependências MIT/Apache‑2.0/BSD/Zlib/MPL‑2.0 (arquivo‑nível) |
| RNF09 | Instalador | ≤ 25 MB |
| RNF10 | Acessibilidade | navegação por teclado, contraste AA |

## 5. Fluxos principais
1. **Rápido**: abrir imagem → auto‑detecção → resultado → Exportar SVG.
2. **Refinado**: abrir → escolher modo → ajustar cores/detalhe → editar paleta → remover ruído com borracha → comparar → exportar.
3. **Lote (CLI)**: `antivector batch ./in -o ./out --preset illustration`.

## 6. Critérios de sucesso (v1.0)
Medidos pelo harness ([07_QUALITY_VALIDATION.md](07_QUALITY_VALIDATION.md)):
- Zero frestas entre regiões (gap test = 0 pixels de fundo vazando) em 100% do corpus.
- Fidelidade: ΔE00 médio ≤ 3,0 e SSIM ≥ 0,95 em categorias Logo e Linha; ≥ 0,90 em Ilustração.
- Nós: ≤ vtracer (mesmos nº de cores) com fidelidade igual ou superior em ≥ 80% das imagens Logo/Ilustração.
- Avaliação visual cega (07 §6) — preferido ou empatado vs. vtracer em ≥ 80% dos pares.
- Desempenho RNF01–RNF03 atendidos.

# 05 — Interface e Experiência do Usuário

Tecnologia: React 18 + TypeScript + Zustand + Vite dentro de Tauri 2. Idiomas: pt‑BR e en (i18n via JSON simples). Tema claro/escuro.

## 1. Telas
### 1.1 Início (sem imagem)
Área central de arrastar‑soltar; botões "Abrir imagem" (Ctrl+O), "Colar" (Ctrl+V), lista de recentes (máx 10, miniaturas). Formatos suportados exibidos.

### 1.2 Espaço de trabalho
```
┌ Barra superior: Abrir | Salvar projeto | Desfazer/Refazer | Exportar ▾ ───────────────┐
├ Painel esquerdo (280px)        │ Canvas central                       │ Painel direito│
│ Modo: [Auto|Logo|Ilustr.|Foto| │ Vistas: Lado a lado | Slider |      │ Paleta        │
│        Linha|Pixel]            │ Vetor | Contorno | Diferença         │ (swatches,    │
│ Cores: Auto ☐ / slider 2–64    │ zoom 1%–6400%, pan (espaço+arrasto)  │  mesclar,     │
│ Detalhe  ───●───               │                                      │  travar,      │
│ Suavidade ───●───              │                                      │  excluir)     │
│ Cantos    ───●───              │                                      │ Estatísticas  │
│ Ruído     ───●───              │                                      │ (paths, nós,  │
│ [Avançado ▾] parâmetros 04 §11 │                                      │ cores, SSIM)  │
│ [Vetorizar] (auto ao mudar)    │ Barra de ferramentas: Mover | Borracha│              │
│                                │ de região | Conta‑gotas | Mesclar     │              │
└ Barra de status: estágio + progresso + tempo + cancelar ─────────────────────────────┘
```

## 2. Comportamentos
- Ao abrir: chama `open_image`, aplica detecção, inicia vetorização automaticamente.
- Sliders: re‑vetorização com debounce de 250 ms; job anterior cancelado. Resultado antigo permanece visível com indicador "atualizando".
- Pré‑visualização progressiva: primeiro processar versão reduzida (≤ 1 MP) e exibir; depois resolução total substitui [HIPÓTESE de valor UX; implementar em F7].
- Vista Contorno: paths com stroke 1px de tela, nós como quadrados 4px (cantos) e círculos (suaves); limitar a 50k nós visíveis (acima: aviso e apenas strokes).
- Borracha de região: hover destaca região (hit test via `isPointInPath` no Path2D da shape); clique ⇒ `edit{MergeRegion{region, into: auto}}`; Shift+clique ⇒ remover (tornar transparente).
- Paleta: arrastar swatch sobre outro ⇒ mesclar; clique ⇒ seletor de cor; cadeado ⇒ travar (vai para `palette_lock`).
- Desfazer/refazer: Ctrl+Z / Ctrl+Shift+Z; cobre parâmetros e edições.
- Exportar: diálogo com formato, agrupamento por cor, modo empilhamento (ADR‑006), escala, precisão decimal (default 2), "hairline seal".

## 3. Renderização
- Canvas 2D com Path2D por shape construído das strings `d` recebidas; cache de Path2D por versão do doc.
- Original: `ImageBitmap`; pixelado (`imageSmoothingEnabled=false`) acima de 400%.
- Diferença: render do vetor em canvas offscreen + composição `difference`.

## 4. Estados
| Estado | UI |
|---|---|
| Vazio | Tela inicial |
| Carregando imagem | Spinner central + nome do arquivo |
| Vetorizando | Barra de status com estágio (ex.: "Ajustando curvas 63%"), botão Cancelar (Esc) |
| Pronto | Estatísticas atualizadas |
| Cancelado | Mantém resultado anterior; toast "Cancelado" |
| Erro | Banner vermelho com mensagem do código (03 §7) e ação (tentar novamente / reduzir imagem) |
| Exportando | Modal com progresso; sucesso ⇒ toast com "Abrir pasta" |
| Imagem grande (> 16 MP) | Aviso prévio com opção de reduzir |

## 5. Acessibilidade e atalhos
Todos os controles com rótulo ARIA e foco visível; atalhos: Ctrl+O, Ctrl+S, Ctrl+E (exportar), 1–5 (vistas), +/−/0 (zoom), E (borracha), Esc (cancelar).

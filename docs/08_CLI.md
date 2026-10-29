# CLI e exportação (F6)

O executável `antivector` funciona offline. Exemplos:

```text
cargo run -p av-cli -- vectorize corpus/images/logo_original_1.png -o logo.svg
antivector in.png -o out.pdf --preset logo --colors 8
antivector vectorize in.png -o out.eps --params params.json
antivector vectorize in.png -o out.svg --precision 4 --group-by-color
antivector vectorize in.png -o out.svg --stacking exact --dump-stages stages
antivector batch ./in -o ./out --preset illustration --format svg
antivector info in.png --max-pixels 64000000
```

Formatos: svg/pdf/eps/dxf/png, inferidos pela extensão; `--format` sobrepõe essa
inferência. Presets: logo/illustration/photo/lineart/pixelart; `--params` lê Params
v1 completo e preserva overrides, `--colors` tem precedência sobre o arquivo.
`--max-pixels` limita decodificação (default64MP). PNG rasteriza na resolução
original com alfa; PDF/EPS preservam paths editáveis e usam96px=72pt. DXF contém
apenas contornos R12 em unidades de pixel (Y para cima), com erro de aproximação
≤0,1px; uma fronteira compartilhada não é duplicada.

SVG aceita precisão0..8 (default2). `--group-by-color` combina subpaths por RGB
e seleciona recorte exato; combiná-lo explicitamente com `--stacking overlap`
é erro de parâmetros, pois agrupamento reordena pintura. Sem agrupamento,
sobreposição opaca segura continua sendo o default de ADR-010.

Lote percorre arquivos diretamente no diretório, em ordem determinística,
reconhecendo PNG/JPEG/BMP/GIF/TIFF/WebP. Subdiretórios e outros arquivos são
ignorados. Cada saída usa o stem da entrada e a extensão de `--format` (default
svg). Stems colidentes são rejeitados antes da conversão, também em Unix, para
evitar comportamento diferente no Windows. Diretórios de entrada/saída devem
diferir. Falhas individuais são relatadas e não impedem as próximas imagens;
ao final há `batch: succeeded=N failed=N`. Uma saída não pode substituir a
própria imagem de entrada. Diretórios pais de saída individual devem existir.

Códigos:0 sucesso;1 lote com falhas;2 parâmetros/limites;3 decodificação;
4 leitura/escrita;10 invariante interna. `info` imprime dimensões e has_alpha.
As estatísticas de conversão contam a representação emitida, incluindo
agrupamento e omissão de furos (DXF: entidades/vértices, sem cores de preenchimento).

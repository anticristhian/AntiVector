# Robustez local

O alvo `decode_pipeline` alterna bytes codificados, rasters RGBA estruturados e
projetos ZIP. Exercita os seis decodificadores habilitados, parâmetros válidos,
vetorização e exportação SVG/PNG. Limite de raster para o fuzz:65.536pixels;
o caso64MP é uma verificação separada do harness. Erros de entrada são esperados;
panics, invariantes e falhas do sanitizador são falhas reais.

Instalar nightly e cargo-fuzz. Windows exige MSVC e C++ AddressSanitizer;
adicionar `bin/Hostx64/x64` do MSVC ao PATH do processo.

```powershell
./scripts/seed-fuzz.ps1
$env:RAYON_NUM_THREADS='1'
cargo +nightly fuzz run decode_pipeline -- -max_total_time=3600 -max_len=65536 -rss_limit_mb=2048
cargo deny --manifest-path fuzz/Cargo.toml --config fuzz/deny.toml check licenses
```

O tempo é do libFuzzer, após a compilação. Preservar log, seeds e qualquer
reproducer antes de corrigir falhas; repetir uma hora sem crash após correções.
Dependências de fuzz ficam neste workspace separado, fora do produto.

Referência: https://rust-fuzz.github.io/book/cargo-fuzz/windows/setup.html

Rodada F9 registrada:90.151execuções em3.601s,exit0,nenhum artefato de crash.
Seeds iniciais:PNG,projeto eRGBA; máximo de entrada configurado65.536bytes,
limite efetivamente explorado informado pelo libFuzzer3.193bytes. O alvo permite
os seis codecs, mas esse log não comprova cobertura uniforme de todos eles.

# AntiVector

Motor offline em Rust para converter raster em vetores multicor, usando um grafo
de fronteiras compartilhadas. Exporta SVG, PDF, EPS, DXF R12 e PNG. Aplicativo
desktop Tauri/React implementado na F7; estado de aceite e evidências em
[docs/PROGRESS.md](docs/PROGRESS.md).

```text
cargo run -p av-cli --release -- corpus/images/logo_original_1.png -o logo.svg
cargo run -p av-cli --release -- vectorize in.png -o out.pdf --preset logo --colors 8
cargo run -p av-cli --release -- batch ./in -o ./out --preset illustration --format svg
cargo run -p av-cli -- info in.png
```

Entrada PNG/JPEG/BMP/GIF/TIFF/WebP, até64MP. Presets logo, illustration, photo,
lineart e pixelart; parâmetros JSON e diagnósticos por estágio. Veja
[uso da CLI](docs/08_CLI.md) para opções, composição, precisão e códigos de saída.

O [aplicativo desktop](app/README.md) usa Tauri/React, com processamento offline,
cache por estágio, comparação sincronizada, cancelamento e exportação nativa.

Requer Rust stable com rustfmt/clippy e alvo wasm32-unknown-unknown. Verificações:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses
cargo check -p av-core --target wasm32-unknown-unknown
cargo run -p av-bench --release -- run --label local --check-baseline corpus/baselines/metrics.json
```

O harness inclui corpus geométrico original CC0 e fotos CC0/domínio público com
proveniência, métricas, comparação vtracer isolada do produto e avaliação visual
cega. O corpus completo contém73 entradas; medições de desempenho e robustez
estão registradas. F0–F9 concluídas:avaliação humana59/72(81,94%),CI final nos
três sistemas e instaladores≤25MB aprovados em2026-10-30. Reprodução em
[docs/09_RELEASE_VALIDATION.md](docs/09_RELEASE_VALIDATION.md). Decisões, arquitetura e fases em
[docs/00_INDEX.md](docs/00_INDEX.md); regras do implementador em [AGENTS.md](AGENTS.md).

# Solwired Raster

App desktop (Rust + [rune](https://github.com/anomalyco/rune)) para o omp-dsh: o [oh-my-pi](https://www.npmjs.com/package/@oh-my-pi/pi-coding-agent) rodando com o system prompt, as ferramentas e a cadeia de contexto do dsh.

- spec: `docs/SPEC.md` · decisões: `docs/DECISIONS.md`
- runtime: `runtime/` (`bun install` instala o omp; `runtime/dsh/` é a extensão)
- agentes: `agents/*.md`

```bash
cd runtime && bun install && cd ..
cargo run --release -- ~/projects/algum-projeto
```

Headless: `raster --snap out.png --scene demo`.

Licença MIT. Fontes: JetBrains Mono e Pixelify Sans (OFL, `assets/fonts`).

# decisões

## D1 nome (12/11/2026)
Nome oficial Solwired Raster; no código, binário e repo: `raster`. Licença MIT.

## D2 UI em rune (12/11/2026)
rune (anomalyco, MIT, pré-release, wgpu + winit, renderer próprio). Fixado num rev do branch `dev`; winit e wgpu-hal via `[patch.crates-io]` apontando para o mesmo rev, como o rune exige. Risco: API instável; mitigação: rev fixo.

## D3 backend é o RPC do omp (12/11/2026)
O app não reimplementa o agente: cada sessão é `omp --mode rpc` (protocolo JSON documentado em `@oh-my-pi/pi-coding-agent/src/modes/rpc/wire`). Isso dá de graça modelos, thinking, abort, `ask` como UI request, eventos de mensagem e ferramenta. A extensão `dsh.ts` (omp-dsh, validada em benchmark) dá prompt e ferramentas do dsh, incluindo subagentes como processos com caixa de entrada em arquivo.

## D4 login do Claude sem rotação (12/11/2026)
Raster lê o access token atual de `~/.claude/.credentials.json` e passa como `ANTHROPIC_OAUTH_TOKEN` a cada processo que sobe. Não importa o refresh token no omp: se o omp renovasse, a rotação invalidaria o login do Claude Code. Quando o token vence, o próximo spawn relê o arquivo (o Claude Code renova sozinho ao ser usado).

## D5 poda de contexto no estilo DCP (12/11/2026)
`runtime/dsh/dcp.ts` reescreve só o payload da sessão raiz: faixas comprimidas pela ferramenta `compress` viram resumo, chamadas idênticas antigas perdem a saída e entradas grandes de erros antigos são esvaziadas. O histórico do omp não muda. Nudges em `<raster-reminder>`: contexto ≥100k a cada 5 requests, ≥50k no turno do usuário, a cada 15 iterações; e o lembrete de vault (`RASTER_VAULT_NUDGE=1`). Os blocos ficam em `~/.raster/ompdsh/dcp-<sessão>.json` e são descartados se a primeira mensagem mudar (compactação do omp).

## D6 vault por MCP do omp (12/11/2026)
O basic-memory entra por `~/.raster/agent/mcp.json`. Por isso a sessão raiz sobe sem `--no-extensions`, que também desliga a descoberta de MCP. O `shapePayload` mantém só as ferramentas do dsh mais as `mcp__*`, então extensões descobertas não vazam para o modelo. Subagentes não recebem MCP.

## D7 lista de modelos estática; Codex pelo login do omp (12/11/2026)
`get_available_models` estoura o limite de transporte do RPC, então a lista fica em `MODELS` (src/main.rs). O login do ChatGPT/Codex usa o OAuth `openai-codex` do próprio omp, disparado pela sessão utilitária em configs; não foi testado ponta a ponta.

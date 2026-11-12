# decisões

## D1 nome (12/11/2026)
Nome oficial Solwired Raster; no código, binário e repo: `raster`. Licença MIT.

## D2 UI em rune (12/11/2026)
rune (anomalyco, MIT, pré-release, wgpu + winit, renderer próprio). Fixado num rev do branch `dev`; winit e wgpu-hal via `[patch.crates-io]` apontando para o mesmo rev, como o rune exige. Risco: API instável; mitigação: rev fixo.

## D3 backend é o RPC do omp (12/11/2026)
O app não reimplementa o agente: cada sessão é `omp --mode rpc` (protocolo JSON documentado em `@oh-my-pi/pi-coding-agent/src/modes/rpc/wire`). Isso dá de graça modelos, thinking, abort, `ask` como UI request, eventos de mensagem e ferramenta. A extensão `dsh.ts` (omp-dsh, validada em benchmark) dá prompt e ferramentas do dsh, incluindo subagentes como processos com caixa de entrada em arquivo.

## D4 login do Claude sem rotação (12/11/2026)
Raster lê o access token atual de `~/.claude/.credentials.json` e passa como `ANTHROPIC_OAUTH_TOKEN` a cada processo que sobe. Não importa o refresh token no omp: se o omp renovasse, a rotação invalidaria o login do Claude Code. Quando o token vence, o próximo spawn relê o arquivo (o Claude Code renova sozinho ao ser usado).

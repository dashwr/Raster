# Solwired Raster — spec

Raster é um app desktop (Rust + [rune](https://github.com/anomalyco/rune)) que embrulha o omp-dsh: o omp (oh-my-pi) rodando com o system prompt, as ferramentas e a cadeia de contexto do dsh. Está para o omp-dsh como o Claude Desktop está para o Claude Code CLI.

## R1 runtime

- R1.1 cada sessão deve ser um processo `omp --mode rpc` com a extensão `runtime/dsh/dsh.ts`, falando JSON por stdin/stdout.
- R1.2 subagente deve ser um processo omp mínimo (recebe, pensa, mexe em arquivo, usa só as tools/MCPs que a mãe liberar, responde); a mãe escolhe modelo e tools e enxerga se ele está pensando, ocioso ou morto.
- R1.3 agente invocado por agente é subagente e não invoca ninguém (profundidade 1).
- R1.4 encerrar sessão mata o processo dela; mandar mensagem a uma sessão encerrada deve re-subir o processo e reabrir a sessão.
- R1.5 todo comando bash dos agentes passa pelo `rtk`.

## R2 agentes

| agente | papel | modelo padrão |
|---|---|---|
| chat | conversa, tira dúvida, pesquisa; plano sem forçar tudo a virar plano; anota no vault | opus |
| code | padrão; prompt do dsh; mensagens mínimas | opus |
| search | prompt enxuto; trabalho braçal; resposta rápida e precisa, sem ressalva defensiva nem "não encontrei" circular | haiku |
| test | teste e triagem com boas práticas; usa o produto como o público-alvo; roda e lê tudo, não altera código, abre GitHub issues | sonnet |

- R2.1 cada agente é um `.md` em `agents/` com persona, regras e output style, montado sobre o prompt do dsh (`runtime/dsh/system.txt`).
- R2.2 `Center.md` descreve o ambiente (SO, pacotes, kernel, specs) e entra em toda sessão.
- R2.3 qualquer agente pode invocar qualquer agente com modelo escolhido; cada agente tem modelo padrão.
- R2.4 sessões (não subagentes) conversam entre si por uma ferramenta; subagente só recebe mensagem da mãe.

## R3 memória

- R3.1 basic-memory MCP (vault): anote novidades; dúvida → busque no vault; não achou → web search.
- R3.2 poda e compressão de contexto no modelo do DCP compress, com nudging que também lembra de ler/gravar no vault.

## R4 provedores

- R4.1 conexão direta com os provedores, sem proxy.
- R4.2 Claude OAuth reaproveitando o login do Claude Code da máquina; Codex OAuth.
- R4.3 lista de modelos com nomes amigáveis; card de effort com slider.

## R5 interface

- R5.1 três colunas: sidebar esquerda (marca + versão, nova sessão, busca, árvore pasta → sessões, configs), transcrição ao centro, painel direito (swarm de subagentes, comandos em background com preview da saída).
- R5.2 indicador por sessão: braille animado trabalhando; `⠿ → ?` pontilhado quando pergunta ou pede permissão. Filtro ativas/todas.
- R5.3 transcrição: mensagem do usuário em caixa com borda; resposta em markdown solto com cara de terminal; chips de arquivo; grupo recolhível "thinking… · used N tools" com braille.
- R5.4 composer: pasta do projeto e "modelo · effort" acima; `+` (anexar, mencionar MCP) e ícone colorido do agente à esquerda; anel/barra de contexto e ícone do provedor à direita.
- R5.5 swarm: grade de avatares pixelados pré-gerados por subagente com status estático (trabalhando, pensando, ocioso); encerrado sai.
- R5.6 permissões e questions inline em bloco na conversa; questions com alternativas selecionáveis via ferramenta.
- R5.7 embeds: imagens, tabelas, markdown, yaml, diagramas renderizados.
- R5.8 menu de contexto (botão direito) em mensagens e sessões; encerrar sessão.
- R5.9 visual: pixel art preto e branco surrealista com dithering (referência: tema pixelized do aerographene), aceleração de hardware, efeitos tipo ripple.

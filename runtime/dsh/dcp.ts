// Poda dinâmica de contexto no modelo do DCP (Tarquinen/opencode-dynamic-context-pruning), para o
// payload Anthropic que o dsh monta. O histórico da sessão não muda: a cada request as faixas
// comprimidas são trocadas pelo resumo, chamadas idênticas antigas perdem a saída e entradas de
// chamadas com erro antigas são esvaziadas. Nudges entram como texto no fim da última mensagem.
import * as fs from "node:fs";

export type Block = { id: string; start: number; end: number; topic: string; summary: string };

const MIN = Number(process.env.RASTER_DCP_MIN ?? 50_000);
const MAX = Number(process.env.RASTER_DCP_MAX ?? 100_000);
const FREQ = Number(process.env.RASTER_DCP_FREQ ?? 5);
const ITER = Number(process.env.RASTER_DCP_ITER ?? 15);
const DEDUP = process.env.RASTER_DCP_DEDUP !== "0";
const VAULT_FIRST = process.env.RASTER_VAULT_FIRST !== "0";
const PURGE_TURNS = 4;
const DEDUP_SKIP = new Set(["write", "edit", "todo_write", "compress", "subagent", "subagent_fork", "send_message", "ask_user", "create_goal", "update_goal"]);

export class Dcp {
	blocks: Block[] = [];
	anchor = "";
	requests = 0;
	lastIds: number[] = [];
	constructor(private file: string) {
		try {
			const saved = JSON.parse(fs.readFileSync(file, "utf8"));
			this.blocks = saved.blocks ?? [];
			this.anchor = saved.anchor ?? "";
		} catch {}
	}
	save() {
		try {
			fs.writeFileSync(this.file, JSON.stringify({ anchor: this.anchor, blocks: this.blocks }));
		} catch {}
	}

	/** Aplica blocos, dedup, purge e ids; devolve as mensagens para o request. */
	apply(raw: any[], vaultNudge: boolean): any[] {
		this.requests++;
		// se o histórico foi reescrito (compactação do omp), os índices não valem mais
		const anchor = JSON.stringify(raw[0]?.content ?? "").slice(0, 400);
		if (anchor !== this.anchor) {
			if (this.anchor) this.blocks = [];
			this.anchor = anchor;
			this.save();
		}
		this.blocks = this.blocks.filter(b => b.end < raw.length);
		const msgs = structuredClone(raw);
		if (DEDUP) dedup(msgs);
		purgeErrors(msgs);
		// ids nas mensagens de usuário (posição no histórico bruto; estável enquanto ele só cresce)
		this.lastIds = [];
		msgs.forEach((m, i) => {
			if (m.role !== "user") return;
			this.lastIds.push(i);
			const tag = `[m${String(i).padStart(4, "0")}]`;
			if (typeof m.content === "string") m.content = `${tag} ${m.content}`;
			else m.content = [...m.content, { type: "text", text: tag }];
		});
		// faixas comprimidas, da mais externa para a interna (blocos aninhados já foram absorvidos)
		let out: any[] = [];
		for (let i = 0; i < msgs.length; i++) {
			const b = this.blocks.find(x => x.start === i);
			if (b) {
				out.push({ role: "user", content: `[${b.id}] compressed context — ${b.topic}\n\n${b.summary}` });
				i = b.end;
				continue;
			}
			out.push(msgs[i]);
		}
		out = mergeUsers(out);
		const tokens = Math.round(JSON.stringify(out).length / 4);
		const nudge = this.nudge(raw, tokens, vaultNudge);
		if (nudge) {
			const last = out.at(-1);
			if (typeof last.content === "string") last.content = [{ type: "text", text: last.content }];
			last.content.push({ type: "text", text: nudge });
		}
		return out;
	}

	private nudge(raw: any[], tokens: number, vault: boolean): string | undefined {
		const parts: string[] = [];
		const sinceUser = raw.length - 1 - lastHumanIndex(raw);
		if (tokens >= MAX && this.requests % FREQ === 0) {
			parts.push("CRITICAL: context is at or beyond the configured limit. Use the `compress` tool now on older, resolved ranges (not the newest active messages) before continuing; keep user intent verbatim.");
		} else if (tokens >= MIN && sinceUser === 0) {
			parts.push("Evaluate the conversation for compressible ranges. If messages are cleanly closed and unlikely to be needed again, use `compress` on them. Keep active context uncompressed.");
		} else if (tokens >= MIN && sinceUser >= ITER && sinceUser % ITER === 0) {
			parts.push("You've been iterating for a while since the last user message. If a closed portion is unlikely to be referenced soon (e.g. finished research before implementation), `compress` it now.");
		}
		if (vault && sinceUser === 0) {
			parts.push(`Memory: if this turn produced something durable (a decision, a verified procedure, a bug cause, a user preference), record it in the vault (basic-memory).${VAULT_FIRST ? " When unsure about the user or their projects, search the vault first; if it is not there, search the web." : ""}`);
		}
		return parts.length ? `<raster-reminder>\n${parts.join("\n\n")}\n</raster-reminder>` : undefined;
	}

	/** Executa a ferramenta `compress`. */
	compress(a: any): string {
		const ranges: any[] = Array.isArray(a.content) ? a.content : [];
		if (!ranges.length) throw new Error("compress needs content: [{ startId, endId, summary }]");
		const lastHuman = this.lastIds.at(-1) ?? 0;
		let n = 0;
		const made: string[] = [];
		for (const r of ranges) {
			let start = parseId(r.startId, this.blocks, "start");
			const end = parseId(r.endId, this.blocks, "end");
			if (start === undefined || end === undefined) throw new Error(`unknown id in ${r.startId}..${r.endId}; use only ids visible in context (mNNNN or bN)`);
			if (start > end) throw new Error(`startId ${r.startId} comes after endId ${r.endId}`);
			if (end >= lastHuman) throw new Error("the range reaches the latest user message; keep the active context uncompressed");
			// um resultado de ferramenta no início precisa da chamada que o gerou
			if (start > 0) start -= 1;
			const id = `b${Math.max(0, ...this.blocks.map(b => Number(b.id.slice(1)))) + 1}`;
			this.blocks = this.blocks.filter(b => b.start < start || b.end > end);
			this.blocks.push({ id, start, end, topic: String(a.topic ?? "context"), summary: String(r.summary ?? "") });
			this.blocks.sort((x, y) => x.start - y.start);
			n += end - start + 1;
			made.push(id);
		}
		this.save();
		return `Compressed ${n} messages into ${made.join(", ")}.`;
	}
}

function parseId(id: unknown, blocks: Block[], side: "start" | "end"): number | undefined {
	const s = String(id ?? "").replace(/[[\]()]/g, "");
	const m = /^m(\d+)$/.exec(s);
	if (m) return Number(m[1]);
	const b = blocks.find(x => x.id === s);
	return b ? (side === "start" ? b.start : b.end) : undefined;
}

function lastHumanIndex(msgs: any[]): number {
	for (let i = msgs.length - 1; i >= 0; i--) {
		const m = msgs[i];
		if (m.role !== "user") continue;
		if (typeof m.content === "string") return i;
		if (!m.content.some((b: any) => b.type === "tool_result")) return i;
	}
	return 0;
}

function mergeUsers(msgs: any[]): any[] {
	const out: any[] = [];
	for (const m of msgs) {
		const prev = out.at(-1);
		if (prev && prev.role === "user" && m.role === "user") {
			const asBlocks = (c: any) => (typeof c === "string" ? [{ type: "text", text: c }] : c);
			prev.content = [...asBlocks(prev.content), ...asBlocks(m.content)];
		} else out.push(m);
	}
	return out;
}

/** Chamadas idênticas (mesma ferramenta e argumentos): só a última mantém a saída. */
function dedup(msgs: any[]) {
	const seen = new Map<string, string>();
	const keyOf = new Map<string, string>();
	for (let i = msgs.length - 1; i >= 0; i--) {
		const m = msgs[i];
		if (m.role !== "assistant" || !Array.isArray(m.content)) continue;
		for (const b of m.content) {
			if (b.type !== "tool_use" || DEDUP_SKIP.has(b.name)) continue;
			const k = `${b.name}:${JSON.stringify(b.input)}`;
			if (seen.has(k)) keyOf.set(b.id, k);
			else seen.set(k, b.id);
		}
	}
	if (!keyOf.size) return;
	for (const m of msgs) {
		if (m.role !== "user" || !Array.isArray(m.content)) continue;
		for (const b of m.content) {
			if (b.type === "tool_result" && keyOf.has(b.tool_use_id)) b.content = "[output superseded by a later identical call]";
		}
	}
}

/** Entradas grandes de chamadas que deram erro, depois de PURGE_TURNS turnos, viram um marcador. */
function purgeErrors(msgs: any[]) {
	const errored = new Set<string>();
	let turns = 0;
	for (let i = msgs.length - 1; i >= 0; i--) {
		const m = msgs[i];
		if (m.role === "assistant") turns++;
		if (turns <= PURGE_TURNS || m.role !== "user" || !Array.isArray(m.content)) continue;
		for (const b of m.content) if (b.type === "tool_result" && b.is_error) errored.add(b.tool_use_id);
	}
	if (!errored.size) return;
	for (const m of msgs) {
		if (m.role !== "assistant" || !Array.isArray(m.content)) continue;
		for (const b of m.content) {
			if (b.type === "tool_use" && errored.has(b.id) && JSON.stringify(b.input).length > 400) b.input = { purged: "input of an old failed call removed" };
		}
	}
}

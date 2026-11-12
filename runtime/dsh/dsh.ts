// omp-dsh: roda o omp com o system prompt, as ferramentas e a cadeia de contexto do dsh 0.2.2-rc.1.
// Carregar com: omp --no-tools --no-context-files --no-skills --no-rules --no-extensions -e dsh.ts
// Env: OMPDSH_MODEL (seletor do omp, ex. hb/claude-sonnet-5-5); internos: OMPDSH_DEPTH, OMPDSH_AGENT_ID,
// OMPDSH_PARENT_ID, OMPDSH_ROOT (diretório de estado compartilhado), OMPDSH_FORK (arquivo com mensagens semente).
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { Dcp } from "./dcp";

const HERE = path.dirname(new URL(import.meta.url).pathname);
const EXT = path.join(HERE, "dsh.ts");
const MODEL_SEL = process.env.OMPDSH_MODEL ?? "";
const MODEL_ID = MODEL_SEL.split("/").pop() ?? "";
// Raster: RASTER_PROMPT_FILES (separados por ":") são anexados ao system do dsh (agente + Center.md)
const SYSTEM = [fs.readFileSync(path.join(HERE, "system.txt"), "utf8").replace("{{model}}", MODEL_ID),
	...(process.env.RASTER_PROMPT_FILES ?? "").split(":").filter(Boolean).map(f => fs.readFileSync(f, "utf8").trim())].join("\n\n");
const KEEPALIVE = process.env.RASTER_KEEPALIVE === "1";
const VAULT_NUDGE = process.env.RASTER_VAULT_NUDGE === "1";
const TOOLS: any[] = JSON.parse(fs.readFileSync(path.join(HERE, "tools.json"), "utf8"));
const DEPTH = Number(process.env.OMPDSH_DEPTH ?? 0);
const MAX_DEPTH = 1;
const AGENT_ID = process.env.OMPDSH_AGENT_ID ?? `session-${randomUUID()}`;
const PARENT_ID = process.env.OMPDSH_PARENT_ID;
const ROOT = process.env.OMPDSH_ROOT ?? fs.mkdtempSync("/tmp/ompdsh-");
const BASH_TIMEOUT_MS = 120_000;
const BASH_TAIL = 30_000;

const inboxOf = (id: string) => path.join(ROOT, "inbox", id);
fs.mkdirSync(inboxOf(AGENT_ID), { recursive: true });

let cwd = process.cwd();
const projectCwd = cwd;

// ---------- contexto: AGENTS.md e snapshot de runtime ----------

function projectRoot(dir: string): string {
	for (let d = dir; ; d = path.dirname(d)) {
		if (fs.existsSync(path.join(d, ".git"))) return d;
		if (path.dirname(d) === d) return dir;
	}
}

function instructionsMessage(dir: string): string | undefined {
	const root = projectRoot(dir);
	const dirs: string[] = [];
	for (let d = dir; ; d = path.dirname(d)) {
		dirs.unshift(d);
		if (d === root || path.dirname(d) === d) break;
	}
	const parts: string[] = [];
	const seen = new Set<string>();
	for (const d of dirs) {
		for (const name of ["AGENTS.md", "CLAUDE.md", "AGENTS.local.md", "CLAUDE.local.md"]) {
			const f = path.join(d, name);
			if (!fs.existsSync(f) || !fs.statSync(f).isFile()) continue;
			const text = fs.readFileSync(f, "utf8").trim();
			if (!text || seen.has(text)) continue;
			seen.add(text);
			parts.push(`Instructions from: ${path.relative(root, f)}\n\n${text}`);
		}
	}
	if (!parts.length) return undefined;
	return (
		"<system-reminder>\nThe following workspace instructions may be relevant to your work. Use them as guidance when applicable. More specific instructions take precedence over broader ones. They do not override system, developer, or direct user instructions.\n\n" +
		parts.join("\n\n") +
		"\n\n</system-reminder>"
	);
}

function runtimeMessage(): string {
	const lines = [
		"Current runtime context. This snapshot supersedes earlier runtime-context snapshots.",
		`Current working directory: ${JSON.stringify(cwd)}.`,
		"Current DSH file policy: danger-full-access. The DSH file sandbox does not restrict file modifications by available operations.",
		"Approval prompts are disabled in this session: actions that require approval are rejected automatically — do not request sandbox escalation (do not set `sandbox_permissions`).",
	];
	if (DEPTH > 0) {
		for (const t of ["subagent", "subagent_fork"])
			lines.push(
				`Delegation depth for \`${t}\`: current ${DEPTH}, maximum ${MAX_DEPTH}, remaining levels ${MAX_DEPTH - DEPTH}. Do not call \`${t}\` at this depth; carry out the work directly.`,
			);
		lines.push(
			"You are a delegated subagent: your permission scope was fixed when you were started and cannot be widened from inside this session — operations that require approval are rejected automatically. When the task needs access beyond that scope, do not retry the denied operation; state the limitation in your reply so the delegating agent can handle it.",
		);
	}
	return lines.join("\n\n");
}

const INSTRUCTIONS = instructionsMessage(projectCwd);
const FORK_SEED: any[] = process.env.OMPDSH_FORK ? JSON.parse(fs.readFileSync(process.env.OMPDSH_FORK, "utf8")) : [];
let lastMessages: any[] = [];
let runtimeSnapshots: string[] = [runtimeMessage()];

const OMP_REMINDER = /^<system-reminder>\nToday:[\s\S]*?<\/system-reminder>\n?/;

function stripPrefix(name: string) {
	return name.startsWith("_") ? name.slice(1) : name;
}

function shapePayload(p: any): any {
	if (!Array.isArray(p?.messages) || !Array.isArray(p?.tools) || p.tools.length === 0) return undefined;
	// OAuth da assinatura Claude exige os blocos de identidade/billing que o omp põe no system
	const keep = (Array.isArray(p.system) ? p.system : [])
		.filter((b: any) => typeof b?.text === "string" && /^You are Claude Code|billing/i.test(b.text))
		.map(({ cache_control: _, ...b }: any) => b);
	p.system = [...keep, { type: "text", text: SYSTEM, cache_control: { type: "ephemeral" } }];
	// Raster: tools MCP (basic-memory) registradas pelo omp continuam visíveis na raiz
	const mcp = DEPTH === 0 ? p.tools.filter((t: any) => typeof t?.name === "string" && stripPrefix(t.name).startsWith("mcp__")).map(({ cache_control: _, ...t }: any) => ({ ...t, name: stripPrefix(t.name) })) : [];
	p.tools = [...activeTools(), ...mcp];
	delete p.tool_choice;
	p.thinking = { budget_tokens: 2048, display: "summarized", type: "enabled" };
	p.max_tokens = 128000;
	for (const m of p.messages) {
		if (!Array.isArray(m.content)) continue;
		for (const b of m.content) if (b.type === "tool_use" && typeof b.name === "string") b.name = stripPrefix(b.name);
	}
	// Raster: poda dinâmica (DCP) só na raiz; filhos são curtos
	if (DEPTH === 0) p.messages = dcp().apply(p.messages, VAULT_NUDGE);
	const first = p.messages[0];
	if (first?.role === "user") {
		if (typeof first.content === "string") first.content = first.content.replace(OMP_REMINDER, "");
		else
			first.content = first.content
				.map((b: any) => (b.type === "text" ? { ...b, text: b.text.replace(OMP_REMINDER, "") } : b))
				.filter((b: any) => b.type !== "text" || b.text.length > 0);
	}
	const inject = [...(INSTRUCTIONS ? [INSTRUCTIONS] : []), ...runtimeSnapshots].map(text => ({ role: "user", content: text }));
	const rest = p.messages.slice(1);
	p.messages = [...FORK_SEED, first, ...inject, ...rest].map(normalize);
	lastMessages = structuredClone(p.messages);
	// como o dsh: um único breakpoint de cache, no último bloco da última mensagem
	const last = p.messages.at(-1);
	if (typeof last.content === "string") last.content = [{ type: "text", text: last.content }];
	last.content.at(-1).cache_control = { type: "ephemeral" };
	delete p.context_management;
	delete p.metadata;
	return p;
}

// sem cache_control do omp; mensagem de usuário com um só bloco de texto vira string, como o dsh envia
function normalize(m: any): any {
	if (typeof m.content === "string") return { role: m.role, content: m.content };
	const content = m.content.map(({ cache_control: _, ...b }: any) => b);
	if (m.role === "user" && content.length === 1 && content[0].type === "text") return { role: m.role, content: content[0].text };
	return { role: m.role, content };
}

let dcpState: Dcp | undefined;
function dcp(): Dcp {
	return (dcpState ??= new Dcp(path.join(ROOT, `dcp-${AGENT_ID}.json`)));
}

// Raster: subagente só vê as ferramentas básicas e as que a mãe liberou; raiz vê todas
function activeTools(): any[] {
	return ALLOWED ? TOOLS.filter(t => ALLOWED.has(t.name)) : TOOLS;
}

// Raster: todo bash passa pelo rtk quando há reescrita conhecida
function rtkRewrite(cmd: string): string {
	if (process.env.RASTER_RTK === "0") return cmd;
	try {
		const r = spawnSync("rtk", ["rewrite", cmd], { encoding: "utf8", timeout: 2000 });
		const out = r.status === 0 ? r.stdout.trim() : "";
		return out || cmd;
	} catch {
		return cmd;
	}
}

// ---------- jobs em segundo plano ----------

type Job = { id: string; kind: string; proc?: ChildProcess; out: string; read: number; status: "running" | "finished" | "killed"; exit?: number | null; done: Promise<void> };
const jobs = new Map<string, Job>();
let jobSeq = 0;

function formatBash(stdout: string, stderr: string, code: number | null): string {
	let s = stdout;
	if (stderr) s += `${s && !s.endsWith("\n") ? "\n" : ""}[stderr]\n${stderr}`;
	if (code !== 0) s += `${s && !s.endsWith("\n") ? "\n" : ""}[exit code: ${code ?? "signal"}]`;
	if (s.length > BASH_TAIL) {
		const f = path.join(ROOT, `bash-${randomUUID()}.txt`);
		fs.writeFileSync(f, s);
		s = `(Output truncated to its tail; full output saved to ${f})\n${s.slice(-BASH_TAIL)}`;
	}
	return s;
}

// ---------- subagentes ----------

type Child = { id: string; label: string; status: "running" | "inactive"; proc?: ChildProcess; sessionDir: string; final: string; sentBeforeFinish: boolean; runs: number; model?: string; effort?: string; agent?: string; tools?: string[] };
const children = new Map<string, Child>();
const pending: string[] = [];
let busy = false;

// raiz roda em --mode rpc (processo vive entre turnos): notificação com agente ocioso abre turno novo,
// como no dsh; sem nada pendente, a raiz encerra o processo
function notify(pi: ExtensionAPI, text: string) {
	if (busy) pi.sendUserMessage(text, { deliverAs: "aside" });
	else if (DEPTH === 0) {
		busy = true;
		pi.sendUserMessage(text);
	} else pending.push(text);
}

function maybeExit(pi: ExtensionAPI) {
	if (DEPTH !== 0 || busy) return;
	const running = [...children.values()].some(c => c.status === "running") || [...jobs.values()].some(j => j.status === "running");
	if (running) return;
	if (goal?.status === "active" && (!goal.max_goal_rounds || goal.rounds < goal.max_goal_rounds)) {
		goal.rounds++;
		notify(pi, `Continue working toward the active goal: ${goal.objective}`);
		return;
	}
	if (!KEEPALIVE) setTimeout(() => !busy && process.exit(0), 200);
}

// Raster: nomes amigáveis de modelo aceitos pelo subagent
const MODEL_ALIASES: Record<string, string> = {
	opus: "anthropic/claude-opus-5-5", sonnet: "anthropic/claude-sonnet-5-5", haiku: "anthropic/claude-haiku-5-5", fable: "anthropic/claude-fable-5-1",
};
const AGENT_MODELS: Record<string, string> = JSON.parse(process.env.RASTER_AGENT_MODELS ?? "{}");
const BASIC_TOOLS = ["bash", "read", "write", "edit", "glob", "grep", "read_image", "send_message", "todo_write", "working_directory"];
const ALLOWED = process.env.RASTER_ALLOWED_TOOLS ? new Set([...BASIC_TOOLS, ...process.env.RASTER_ALLOWED_TOOLS.split(",").filter(Boolean)]) : undefined;
function resolveModel(m?: string, agent?: string): string {
	const want = m?.trim().toLowerCase();
	if (want) return MODEL_ALIASES[want] ?? (want.includes("/") ? m!.trim() : `anthropic/${m!.trim()}`);
	return (agent && AGENT_MODELS[agent]) || MODEL_SEL;
}

function ompArgs(extra: string[], prompt: string, c?: Child) {
	return [
		"-p", "--no-title", "--model", c?.model ?? MODEL_SEL, "--thinking", c?.effort ?? "low", "--auto-approve", "--mode", "json",
		"--no-tools", "--no-context-files", "--no-skills", "--no-rules", "--no-extensions", "-e", EXT, ...extra, prompt,
	];
}

function runChild(pi: ExtensionAPI, c: Child, prompt: string, env: Record<string, string>, cdir: string) {
	c.status = "running";
	c.runs++;
	writeSwarm();
	c.sentBeforeFinish = false;
	const extra = ["--session-dir", c.sessionDir, ...(c.runs > 1 ? ["-c"] : [])];
	const agentEnv: Record<string, string> = {};
	if (c.agent && process.env.RASTER_AGENTS_DIR) {
		agentEnv.RASTER_PROMPT_FILES = [path.join(process.env.RASTER_AGENTS_DIR, `${c.agent}.md`), process.env.RASTER_CENTER ?? ""].filter(Boolean).join(":");
	}
	agentEnv.RASTER_ALLOWED_TOOLS = (c.tools ?? []).join(",");
	agentEnv.OMPDSH_MODEL = c.model ?? MODEL_SEL;
	const proc = spawn(process.env.RASTER_OMP ?? "omp", ompArgs(extra, prompt, c), {
		cwd: cdir,
		env: { ...process.env, ...agentEnv, ...env, OMPDSH_DEPTH: String(DEPTH + 1), OMPDSH_AGENT_ID: c.id, OMPDSH_PARENT_ID: AGENT_ID, OMPDSH_ROOT: ROOT },
		stdio: ["ignore", "pipe", "pipe"],
	});
	c.proc = proc;
	const log = fs.createWriteStream(path.join(ROOT, `child-${c.id}.jsonl`), { flags: "a" });
	let buf = "";
	proc.stdout!.on("data", (d: Buffer) => {
		log.write(d);
		buf += d.toString();
		let i: number;
		while ((i = buf.indexOf("\n")) >= 0) {
			const line = buf.slice(0, i);
			buf = buf.slice(i + 1);
			try {
				const e = JSON.parse(line);
				if (e.type === "message_end" && e.message?.role === "assistant") {
					const t = (e.message.content ?? []).filter((b: any) => b.type === "text").map((b: any) => b.text).join("\n");
					if (t) c.final = t;
				}
			} catch {}
		}
	});
	proc.stderr!.on("data", (d: Buffer) => log.write(d));
	return new Promise<void>(resolve =>
		proc.on("close", () => {
			log.end();
			c.status = "inactive";
			c.proc = undefined;
			writeSwarm();
			resolve();
		}),
	);
}

function childPrompt(id: string, prompt: string) {
	return `${prompt}Your parent agent id is "${AGENT_ID}". Before you finish, send your result to that agent with send_message({ agent_id: "${AGENT_ID}", message: "<self-contained result>" }). The parent shares your workspace but does not automatically receive your transcript, tool output, or reasoning. Send earlier messages as well when a finding changes what the parent should do next; sending a message does not end your turn.`;
}

function startChild(pi: ExtensionAPI, args: any, fork: boolean): string {
	if (DEPTH >= MAX_DEPTH) throw new Error(`Delegation depth limit reached: current ${DEPTH}, maximum ${MAX_DEPTH}. Carry out the work directly.`);
	const id = randomUUID();
	const agent = typeof args.agent === "string" ? args.agent : undefined;
	const c: Child = {
		id, label: args.description ?? "", status: "running", sessionDir: path.join(ROOT, "sessions", id), final: "", sentBeforeFinish: false, runs: 0,
		agent, model: resolveModel(args.model, agent), effort: args.effort, tools: Array.isArray(args.tools) ? args.tools : [],
	};
	children.set(id, c);
	writeSwarm();
	fs.mkdirSync(inboxOf(id), { recursive: true });
	const env: Record<string, string> = {};
	if (fork) {
		// semente: turnos completos (antes da última mensagem humana em curso)
		let cut = 0;
		lastMessages.forEach((m, i) => {
			if (m.role === "user" && typeof m.content === "string" && i > 0 && !m.content.startsWith("<system-reminder>") && !m.content.startsWith("Current runtime context.")) cut = i;
		});
		const f = path.join(ROOT, `fork-${id}.json`);
		fs.writeFileSync(f, JSON.stringify(lastMessages.slice(0, cut)));
		env.OMPDSH_FORK = f;
	}
	const cdir = args.cwd ? path.resolve(cwd, args.cwd) : cwd;
	runChild(pi, c, childPrompt(id, args.prompt), env, cdir).then(() => finished(pi, c));
	return `started subagent ${id}`;
}

function finished(pi: ExtensionAPI, c: Child) {
	pollInbox(pi);
	notify(
		pi,
		c.sentBeforeFinish
			? `Background subagent ${c.id} finished and will do no further work unless you send it more.It sent you a message just before finishing, so its closing message is omitted.`
			: `Background subagent ${c.id} finished and will do no further work unless you send it more. Its closing message:\n${c.final}`,
	);
}

// caixa de entrada: mensagens de filhos (para o pai) ou do pai (para este filho)
function pollInbox(pi: ExtensionAPI) {
	const dir = inboxOf(AGENT_ID);
	for (const f of fs.readdirSync(dir).sort()) {
		const p = path.join(dir, f);
		let m: any;
		try {
			m = JSON.parse(fs.readFileSync(p, "utf8"));
		} catch {
			continue;
		}
		fs.unlinkSync(p);
		const c = children.get(m.from);
		if (c) c.sentBeforeFinish = true;
		notify(pi, `Agent ${m.from} sent a message: ${m.message}`);
	}
}

// Raster: estado do swarm desta sessão, lido pelo app (pensando, ferramenta, morto)
function writeSwarm() {
	if (DEPTH !== 0) return;
	const list = [...children.values()].map(c => ({ id: c.id, label: c.label, status: c.status, agent: c.agent, model: c.model, pid: c.proc?.pid }));
	try {
		fs.writeFileSync(path.join(ROOT, `swarm-${AGENT_ID}.json`), JSON.stringify(list));
	} catch {}
}

function rasterSessions(): any[] {
	try {
		return JSON.parse(fs.readFileSync(path.join(ROOT, "raster-sessions.json"), "utf8"));
	} catch {
		return [];
	}
}

function deliver(to: string, message: string) {
	const f = path.join(inboxOf(to), `${Date.now()}-${randomUUID()}.json`);
	fs.writeFileSync(`${f}.tmp`, JSON.stringify({ from: AGENT_ID, message }));
	fs.renameSync(`${f}.tmp`, f);
}

// ---------- metas ----------
let goal: { id: string; revision: number; objective: string; status: string; max_goal_rounds?: number; rounds: number } | undefined;

// ---------- workflow ----------
async function runWorkflow(pi: ExtensionAPI, a: any): Promise<string> {
	const logs: string[] = [];
	const agent = async (prompt: string, opts: any = {}) => {
		if (DEPTH >= MAX_DEPTH) throw new Error("workflow agent(): delegation depth limit reached");
		const id = randomUUID();
		const c: Child = { id, label: opts.label ?? "workflow", status: "running", sessionDir: path.join(ROOT, "sessions", id), final: "", sentBeforeFinish: false, runs: 0 };
		fs.mkdirSync(inboxOf(id), { recursive: true });
		const p = opts.schema ? `${prompt}\n\nReply with only a JSON value matching this JSON Schema:\n${JSON.stringify(opts.schema)}` : prompt;
		await runChild(pi, c, p, {}, cwd);
		if (!opts.schema) return c.final || null;
		try {
			return JSON.parse(c.final.replace(/^```(?:json)?\n?|```$/g, "").trim());
		} catch {
			return null;
		}
	};
	const parallel = (thunks: (() => any)[]) => Promise.all(thunks.map(t => Promise.resolve().then(t).catch(() => null)));
	const pipeline = (items: any[], ...stages: ((prev: any, item: any, i: number) => any)[]) =>
		Promise.all(
			items.map(async (item, i) => {
				let prev: any = item;
				for (const s of stages) {
					try {
						prev = await s(prev, item, i);
					} catch {
						return null;
					}
				}
				return prev;
			}),
		);
	const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor;
	const fn = new AsyncFunction("agent", "parallel", "pipeline", "phase", "log", "args", a.script);
	const result = await fn(agent, parallel, pipeline, (t: string) => logs.push(`## ${t}`), (m: string) => logs.push(m), a.args ?? {});
	return JSON.stringify(result ?? null, null, 1);
}

// ---------- ferramentas ----------

const text = (t: string) => ({ content: [{ type: "text" as const, text: t }], details: {} });
const abs = (p: string) => path.resolve(cwd, p);

function rg(args: string[], dir: string): Promise<string> {
	return new Promise(resolve => {
		const p = spawn("rg", args, { cwd: dir });
		let o = "";
		p.stdout.on("data", d => (o += d));
		p.on("close", () => resolve(o));
		p.on("error", () => resolve(""));
	});
}

export default function ompDsh(pi: ExtensionAPI) {
	const z = pi.zod;
	const anyArgs = z.object({}).passthrough();

	const impl: Record<string, (a: any, signal?: AbortSignal, ctx?: any) => Promise<any>> = {
		async bash(a) {
			const dir = a.workdir ? abs(a.workdir) : cwd;
			const proc = spawn("bash", ["-c", rtkRewrite(a.command)], { cwd: dir, stdio: ["ignore", "pipe", "pipe"] });
			let out = "", err = "";
			proc.stdout.on("data", d => (out += d));
			proc.stderr.on("data", d => (err += d));
			const done = new Promise<number | null>(r => proc.on("close", c => r(c)));
			const toJob = () => {
				const id = `job-${++jobSeq}`;
				const j: Job = { id, kind: "bash", proc, out: "", read: 0, status: "running", done: Promise.resolve() };
				const sync = () => (j.out = formatBash(out, err, j.exit ?? 0));
				j.done = done.then(c => {
					j.exit = c;
					if (j.status === "running") j.status = "finished";
					j.out = formatBash(out, err, c);
					notify(pi, `Background job ${id} (${a.description ?? "bash"}) finished with exit code ${c}. Collect it with job_output.`);
				});
				proc.stdout.on("data", sync);
				jobs.set(id, j);
				return id;
			};
			if (a.run_in_background) return text(`started background job ${toJob()}`);
			const limit = Math.min(a.timeoutMs ?? BASH_TIMEOUT_MS, 600_000);
			const r = await Promise.race([done, new Promise<"t">(res => setTimeout(() => res("t"), limit))]);
			if (r === "t") return text(`Command exceeded ${limit} ms and moved to the background as job ${toJob()}.`);
			return text(formatBash(out, err, r));
		},
		async read(a) {
			const f = abs(a.file_path);
			if (!fs.existsSync(f)) throw new Error(`File not found: ${f}`);
			if (fs.statSync(f).isDirectory()) {
				const entries = fs.readdirSync(f, { withFileTypes: true }).map(e => e.name + (e.isDirectory() ? "/" : "")).sort();
				return text(`<path>${f}</path>\n<type>directory</type>\n<entries>\n${entries.join("\n")}\n</entries>`);
			}
			const lines = fs.readFileSync(f, "utf8").split("\n");
			if (lines.at(-1) === "") lines.pop();
			const off = Math.max(1, a.offset ?? 1), lim = a.limit ?? 2000;
			const slice = lines.slice(off - 1, off - 1 + lim);
			const end = off - 1 + slice.length;
			const tail = end < lines.length ? `(File has more lines. Use 'offset' parameter to read beyond line ${end})` : `(End of file - total ${lines.length} lines)`;
			return text(`<path>${f}</path>\n<type>file</type>\n<content>\n${slice.map((l, i) => `${off + i}: ${l}`).join("\n")}\n\n${tail}\n</content>`);
		},
		async read_image(a) {
			const f = abs(a.file_path);
			const ext = path.extname(f).slice(1).toLowerCase();
			const mime = ({ png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp", gif: "image/gif" } as any)[ext];
			if (!mime) throw new Error(`Unsupported image type: ${f}`);
			return { content: [{ type: "image" as const, data: fs.readFileSync(f).toString("base64"), mimeType: mime }], details: {} };
		},
		async write(a) {
			const f = abs(a.file_path);
			const existed = fs.existsSync(f);
			fs.mkdirSync(path.dirname(f), { recursive: true });
			fs.writeFileSync(f, a.content);
			return text(`<path>${f}</path>\n<type>file</type>\n<content>\n${existed ? "Updated file" : "Created file"}\n</content>`);
		},
		async edit(a) {
			const f = abs(a.file_path);
			if (!fs.existsSync(f)) throw new Error(`File not found: ${f}`);
			const s = fs.readFileSync(f, "utf8");
			const n = s.split(a.old_string).length - 1;
			if (!a.old_string || n === 0) throw new Error(`old_string not found in ${f}`);
			if (n > 1 && !a.replace_all) throw new Error(`old_string appears ${n} times in ${f}; make it unique or set replace_all`);
			fs.writeFileSync(f, a.replace_all ? s.split(a.old_string).join(a.new_string) : s.replace(a.old_string, () => a.new_string));
			return text(`The file ${f} has been updated successfully.`);
		},
		async glob(a) {
			const dir = a.path ? abs(a.path) : cwd;
			const pat = a.pattern.includes("/") ? a.pattern : `**/${a.pattern}`;
			const files = (await rg(["--files", "--hidden", "--no-ignore", "-g", pat, "-g", "!.git"], dir)).split("\n").filter(Boolean).map(f => path.join(dir, f));
			files.sort((x, y) => fs.statSync(y).mtimeMs - fs.statSync(x).mtimeMs);
			if (!files.length) return text("No files found");
			if (files.length <= 100) return text(files.join("\n"));
			const save = path.join(ROOT, `glob-${randomUUID()}.txt`);
			fs.writeFileSync(save, files.join("\n"));
			return text(`${files.slice(0, 100).join("\n")}\n(Showing 100 of ${files.length} files; complete list saved to ${save})`);
		},
		async grep(a) {
			const target = a.path ? abs(a.path) : cwd;
			const args = ["-n", "--hidden", "--no-heading", "--with-filename", "-g", "!.git", ...(a.include ? ["-g", a.include] : []), "-e", a.pattern, target];
			const lines = (await rg(args, cwd)).split("\n").filter(Boolean);
			if (!lines.length) return text("No matches found");
			const by = new Map<string, string[]>();
			for (const l of lines.slice(0, 250)) {
				const m = l.match(/^(.*?):(\d+):(.*)$/);
				if (!m) continue;
				by.set(m[1], [...(by.get(m[1]) ?? []), `  Line ${m[2]}: ${m[3]}`]);
			}
			let out = `Found ${lines.length} matches\n` + [...by].map(([f, ls]) => `${f}:\n${ls.join("\n")}`).join("\n\n");
			if (lines.length > 250) {
				const save = path.join(ROOT, `grep-${randomUUID()}.txt`);
				fs.writeFileSync(save, lines.join("\n"));
				out += `\n(Showing 250 of ${lines.length} matches; complete list saved to ${save})`;
			}
			return text(out);
		},
		async job_output(a) {
			const j = jobs.get(a.job_id);
			if (!j) throw new Error(`Unknown job: ${a.job_id}`);
			if (a.wait && j.status === "running") await Promise.race([j.done, new Promise(r => setTimeout(r, Math.min(a.timeout_ms ?? 300_000, 600_000)))]);
			const chunk = j.out.slice(j.read);
			j.read = j.out.length;
			return text(`[${j.id} ${j.status}${j.exit !== undefined ? `, exit code ${j.exit}` : ""}]\n${chunk}`);
		},
		async job_list() {
			return text([...jobs.values()].map(j => `${j.id} [${j.status}] ${j.kind}`).join("\n") || "No background jobs");
		},
		async job_kill(a) {
			const j = jobs.get(a.job_id);
			if (!j) throw new Error(`Unknown job: ${a.job_id}`);
			j.status = "killed";
			j.proc?.kill("SIGTERM");
			return text(`kill requested for job ${j.id}`);
		},
		async todo_write(a) {
			return text(a.todos.map((t: any) => `[${t.status}] ${t.content}`).join("\n"));
		},
		async web_fetch(a) {
			const r = await fetch(a.url);
			let t = await r.text();
			if ((r.headers.get("content-type") ?? "").includes("html")) t = t.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/gi, "").replace(/<[^>]+>/g, " ").replace(/\s+\n/g, "\n");
			return text(t.slice(0, 100_000));
		},
		async working_directory(a) {
			if (a.cd) {
				const d = abs(a.cd);
				if (!fs.existsSync(d) || !fs.statSync(d).isDirectory()) throw new Error(`Not a directory: ${d}`);
				cwd = d;
				runtimeSnapshots.push(runtimeMessage());
			}
			return text(`Current working directory: ${JSON.stringify(cwd)}.`);
		},
		async skill(a) {
			throw new Error(`Unknown skill: ${a.name}. The session skill catalog is empty.`);
		},
		async subagent(a) {
			return text(startChild(pi, a, false));
		},
		async subagent_fork(a) {
			return text(startChild(pi, a, true));
		},
		async send_message(a) {
			if (DEPTH === 0 && typeof a.agent_id === "string" && a.agent_id.startsWith("session-") && a.agent_id !== AGENT_ID) {
				fs.mkdirSync(inboxOf(a.agent_id), { recursive: true });
				deliver(a.agent_id, a.message);
				return text(`message delivered to session ${a.agent_id}`);
			}
			if (a.agent_id === PARENT_ID) {
				deliver(PARENT_ID!, a.message);
				return text(`message delivered to agent ${a.agent_id}`);
			}
			const c = children.get(a.agent_id);
			if (!c) throw new Error(`Unknown agent: ${a.agent_id}`);
			if (c.status === "running") deliver(c.id, a.message);
			else runChild(pi, c, a.message, {}, cwd).then(() => finished(pi, c));
			return text(`message delivered to agent ${a.agent_id}`);
		},
		async list_agents(a) {
			const kids = [...children.values()].map(c => `${c.id} [${c.status}] — ${c.label}${c.agent ? ` (${c.agent}, ${c.model})` : ""}`);
			if (a?.scope === "sessions" && DEPTH === 0) {
				const ss = rasterSessions().filter((x: any) => x.agent_id !== AGENT_ID).map((x: any) => `${x.agent_id} [${x.status}] — ${x.agent}: ${x.title}`);
				return text(ss.join("\n") || "No other sessions");
			}
			return text(kids.join("\n") || "No subagents");
		},
		async interrupt_agent(a) {
			const c = children.get(a.agent_id);
			if (!c) throw new Error(`Unknown agent: ${a.agent_id}`);
			c.proc?.kill("SIGINT");
			return text(`interrupt requested for agent ${a.agent_id}`);
		},
		async create_goal(a) {
			goal = { id: randomUUID(), revision: 1, objective: a.objective, status: "active", max_goal_rounds: a.max_goal_rounds, rounds: 0 };
			return text(JSON.stringify(goal));
		},
		async get_goal() {
			return text(goal ? JSON.stringify(goal) : "No goal");
		},
		async update_goal(a) {
			if (!goal || goal.id !== a.goal_id) throw new Error("Unknown goal_id");
			if (goal.revision !== a.revision) throw new Error(`Stale revision: current ${goal.revision}`);
			goal.revision++;
			if (a.action === "edit") Object.assign(goal, a.objective ? { objective: a.objective } : {}, a.max_goal_rounds ? { max_goal_rounds: a.max_goal_rounds } : {});
			else goal.status = { pause: "paused", resume: "active", complete: "complete", blocked: "blocked" }[a.action as string] ?? goal.status;
			return text(JSON.stringify(goal));
		},
		async workflow(a) {
			return text(await runWorkflow(pi, a));
		},
		async compress(a) {
			return text(dcp().compress(a));
		},
		// Raster: perguntas com alternativas, desenhadas como bloco interativo no app
		async ask_user(a, _signal, ctx) {
			const qs = (Array.isArray(a.questions) ? a.questions : []).map((q: any, i: number) => ({
				id: String(q.id ?? `q${i + 1}`),
				question: String(q.question ?? ""),
				options: (q.options ?? []).map((o: any) => (typeof o === "string" ? { label: o } : { label: String(o.label), description: o.description })),
				multi: !!q.multi,
				recommended: typeof q.recommended === "number" ? q.recommended : undefined,
			}));
			if (!qs.length) throw new Error("ask_user needs at least one question");
			const ui = ctx?.ui;
			if (ui?.askDialog) {
				const r = await ui.askDialog(qs, {});
				if (!r) return text("The user dismissed the question without answering.");
				if (r.kind === "chat") return text("The user wants to discuss this instead of choosing; ask in plain text.");
				return text(r.results.map((x: any) => `${x.question}\n→ ${[...x.selectedOptions, ...(x.customInput ? [x.customInput] : [])].join(", ") || "(no answer)"}`).join("\n\n"));
			}
			if (!ui?.select) throw new Error("No interactive user is attached; decide with the best default and say so.");
			const out: string[] = [];
			for (const q of qs) out.push(`${q.question}\n→ ${(await ui.select(q.question, q.options)) ?? "(no answer)"}`);
			return text(out.join("\n\n"));
		},
	};

	for (const t of TOOLS) {
		const run = impl[t.name];
		if (!run) throw new Error(`omp-dsh: tool without implementation: ${t.name}`);
		pi.registerTool({
			name: t.name,
			label: t.name,
			description: t.description,
			parameters: anyArgs,
			loadMode: "essential",
			async execute(_id: string, params: any, signal: AbortSignal | undefined, _onUpdate: unknown, ctx: any) {
				return run(params, signal, ctx);
			},
		} as any);
	}

	// tools MCP chegam depois do session_start (descoberta assíncrona); ressincroniza a cada turno
	const syncTools = async () => {
		const mcp = DEPTH === 0 ? pi.getAllTools().map((t: any) => t.name).filter((n: string) => n.startsWith("mcp__")) : [];
		if (process.env.RASTER_DEBUG_TOOLS) console.error("raster tools", pi.getAllTools().length, mcp.join(","));
		await pi.setActiveTools([...activeTools().map(t => t.name), ...mcp]);
	};
	pi.on("session_start", syncTools);
	pi.on("before_agent_start", syncTools);
	pi.on("agent_start", () => {
		busy = true;
	});
	pi.on("agent_end", () => {
		busy = false;
		setTimeout(() => maybeExit(pi), 300);
	});
	setInterval(() => pollInbox(pi), 300).unref();

	pi.on("before_provider_request", (ev: any) => shapePayload(structuredClone(ev.payload)));
}

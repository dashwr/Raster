import { expect, test } from "bun:test";
import * as os from "node:os";
import * as path from "node:path";
import { Dcp } from "./dcp";

const tmp = () => path.join(os.tmpdir(), `dcp-${Math.random()}.json`);
const convo = () => [
	{ role: "user", content: "pergunta inicial" },
	{ role: "assistant", content: [{ type: "tool_use", id: "a", name: "read", input: { path: "x" } }] },
	{ role: "user", content: [{ type: "tool_result", tool_use_id: "a", content: "x v1" }] },
	{ role: "assistant", content: [{ type: "tool_use", id: "b", name: "read", input: { path: "x" } }] },
	{ role: "user", content: [{ type: "tool_result", tool_use_id: "b", content: "x v2" }] },
	{ role: "assistant", content: [{ type: "text", text: "ok" }] },
	{ role: "user", content: "segunda pergunta" },
];

test("ids nas mensagens de usuário e dedup mantém só a última saída", () => {
	const d = new Dcp(tmp());
	const out = d.apply(convo(), false);
	expect(out[0].content).toBe("[m0000] pergunta inicial");
	expect(out[2].content[0].content).toBe("[output superseded by a later identical call]");
	expect(out[4].content[0].content).toBe("x v2");
	expect(d.lastIds).toEqual([0, 2, 4, 6]);
});

test("compress troca a faixa pelo resumo e protege a mensagem ativa", () => {
	const d = new Dcp(tmp());
	d.apply(convo(), false);
	expect(() => d.compress({ topic: "t", content: [{ startId: "m0002", endId: "m0006", summary: "s" }] })).toThrow();
	expect(d.compress({ topic: "leitura de x", content: [{ startId: "m0002", endId: "m0004", summary: "x foi lido duas vezes" }] })).toContain("b1");
	const out = d.apply(convo(), false);
	// a faixa começa na chamada (m0001) para não deixar resultado órfão
	expect(out.length).toBe(3);
	expect(JSON.stringify(out[0].content)).toContain("pergunta inicial");
	expect(JSON.stringify(out[0].content)).toContain("[b1] compressed context");
	expect(out[1].role).toBe("assistant");
	expect(out.at(-1).content).toBe("[m0006] segunda pergunta");
});

test("histórico reescrito descarta os blocos", () => {
	const d = new Dcp(tmp());
	d.apply(convo(), false);
	d.compress({ topic: "t", content: [{ startId: "m0002", endId: "m0004", summary: "s" }] });
	const other = convo();
	other[0] = { role: "user", content: "resumo da compactação do omp" };
	d.apply(other, false);
	expect(d.blocks.length).toBe(0);
});

test("nudge de memória entra no começo do turno", () => {
	const d = new Dcp(tmp());
	const out = d.apply(convo(), true);
	expect(JSON.stringify(out.at(-1).content)).toContain("vault");
});

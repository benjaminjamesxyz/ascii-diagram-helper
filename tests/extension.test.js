import assert from "node:assert";
import asciiDiagramExtension from "../extensions/ascii-diagram.ts";

console.log("Running Pi Extension tests...");

let registeredTool = null;
let registeredCommand = null;
let registeredTransformer = null;

const sentMessages = [];
const mockPi = {
	registerTool: (tool) => {
		registeredTool = tool;
	},
	registerCommand: (name, cmd) => {
		registeredCommand = { name, cmd };
	},
	registerMarkdownTransformer: (fn) => {
		registeredTransformer = fn;
	},
	sendUserMessage: (msg) => {
		sentMessages.push(msg);
	},
};

asciiDiagramExtension(mockPi);

assert.ok(registeredTool, "Tool must be registered");
assert.strictEqual(registeredTool.name, "draw_diagram");
assert.ok(registeredCommand, "Command must be registered");
assert.strictEqual(registeredCommand.name, "diagram");
assert.ok(registeredTransformer, "Markdown transformer must be registered");

// Test Flowchart execution
const fcResult = await registeredTool.execute(
	"test-call-1",
	{
		dsl: "graph TD\n  Client[Client] -->|Request| Server[API Server]",
		style: "rounded",
	},
	null,
	null,
	{ cwd: process.cwd() },
);

assert.ok(fcResult.content[0].text.includes("Client"));
assert.ok(fcResult.content[0].text.includes("API Server"));
assert.ok(fcResult.content[0].text.includes("Request"));
assert.ok(fcResult.content[0].text.includes("▼"));

// Test Sequence diagram execution
const seqResult = await registeredTool.execute(
	"test-call-2",
	{
		dsl: "sequenceDiagram\n  A -> B: Ping\n  B --> A: Pong",
		style: "sharp",
	},
	null,
	null,
	{ cwd: process.cwd() },
);

assert.ok(seqResult.content[0].text.includes("A"));
assert.ok(seqResult.content[0].text.includes("B"));
assert.ok(seqResult.content[0].text.includes("Ping"));
assert.ok(seqResult.content[0].text.includes("Pong"));

// Test Table JSON execution
const tblResult = await registeredTool.execute(
	"test-call-3",
	{
		spec: JSON.stringify({
			type: "table",
			style: "rounded",
			headers: ["Key", "Value"],
			rows: [["Status", "Active"]],
		}),
	},
	null,
	null,
	{ cwd: process.cwd() },
);

assert.ok(tblResult.content[0].text.includes("Status"));
assert.ok(tblResult.content[0].text.includes("Active"));

// Test Markdown Transformer auto-rendering Mermaid blocks
const sampleMarkdown = `Interrupt sequence:

\`\`\`mermaid
flowchart TB
A["CPSR/PSTATE to SPSR<br/>2. Saves return PC to LR/ELR<br/>3. Masks IRQs (Sets I-bit = 1)<br/>4. Jumps to Vector Base Address (VBAR)"] --> VEC["Exception Vector Table (IRQ Offset)"]
\`\`\`
\`Mermaid diagram not rendered: dropped, expected a link: "CPSR/PSTATE to SPSR<br/>2. Saves return PC to LR/ELR<br/>3. Masks IRQs (Sets I-bit = 1)<br/>4. Jumps to Vector Base Address (VBAR)" --> VEC["Exception Vector Table (IRQ Offset)"]" (+1 more)\`

End of section.`;

const transformed = registeredTransformer(sampleMarkdown, {
	messageType: "assistant",
	isStreaming: false,
	availableWidth: 80,
});

assert.ok(transformed.includes("```text"), "Must convert to ```text code block");
assert.ok(transformed.includes("CPSR/PSTATE to SPSR"), "Must contain node label text");
assert.ok(transformed.includes("Exception Vector Table"), "Must contain target node text");
assert.ok(!transformed.includes("Mermaid diagram not rendered"), "Must strip grok-mermaid warning");

// Test /diagram --color flag parsing (colored output requested)
const notifications = [];
const ctxMock = {
	cwd: process.cwd(),
	ui: { notify: (msg, level) => notifications.push({ msg, level }) },
};
await registeredCommand.cmd.handler("--color graph TD; A --> B", ctxMock);
assert.ok(sentMessages.length > 0, "diagram message sent");
assert.ok(sentMessages.at(-1).includes("A"), "diagram content present");

// Test /diagram plain mode (no flag → no --color arg → binary auto = plain when piped)
await registeredCommand.cmd.handler("graph TD; C --> D", ctxMock);
assert.ok(sentMessages.at(-1).includes("C"), "plain diagram sent");

// Test /diagram --style pass-through (README-advertised; DOC-03): pure 7-bit ascii render
await registeredCommand.cmd.handler("--style ascii graph TD; A --> B", ctxMock);
assert.ok(
	notifications.at(-1).level === "info",
	"--style ascii render must succeed",
);
assert.ok(/^\|\s+A\s+\|$/m.test(sentMessages.at(-1)), "ascii box border present");
assert.ok(
	!/[┌┐└┘├┤╭╮╰╯]/.test(sentMessages.at(-1)),
	"ascii style must not leak Unicode box glyphs",
);

// Test --style with the README /diagram example form (ds shorthand trailing input)
await registeredCommand.cmd.handler("--style ascii ds tree 8 3", ctxMock);
assert.ok(
	notifications.at(-1).level === "info",
	"--style ascii ds tree must succeed",
);
assert.ok(sentMessages.at(-1).includes("+"), "ascii ds tree box present");

// Test leading flags in any order (position-independent flag loop)
await registeredCommand.cmd.handler("--color --style ascii graph TD; E --> F", ctxMock);
assert.ok(
	notifications.at(-1).level === "info",
	"--color --style order must not matter",
);
const cleanMsg = sentMessages.at(-1).replace(/\x1b\[[0-9;]*m/g, "");
assert.ok(/^\|\s+E\s+\|$/m.test(cleanMsg), "both flags applied to render");
assert.ok(sentMessages.at(-1).includes("\x1b["), "color escape codes present when --color requested");

// Test --example keeps working alongside --style, order-free
await registeredCommand.cmd.handler("--style ascii --example tree", ctxMock);
assert.ok(
	notifications.at(-1).level === "info",
	"--style before --example must succeed",
);
assert.ok(
	sentMessages.at(-1).includes("|--"),
	"example tree rendered in ascii style",
);

// Test missing --style value → actionable error, no render
const msgsBeforeFlagErr = sentMessages.length;
await registeredCommand.cmd.handler("--style", ctxMock);
assert.ok(
	notifications.at(-1).level === "error" &&
		notifications.at(-1).msg.includes("rounded"),
	"missing --style value must list valid styles",
);
assert.ok(
	sentMessages.length === msgsBeforeFlagErr,
	"no diagram sent on flag error",
);

// Test leftover text after --example <type> → error, not silent drop
await registeredCommand.cmd.handler("--example sequence extra text", ctxMock);
assert.ok(
	notifications.at(-1).level === "error" &&
		notifications.at(-1).msg.includes("--example"),
	"leftover text after --example must be reported",
);

// Test EPIPE guard (CLI-05): binary that exits before consuming stdin must
// surface as a rejected render, never crash the host with unhandled EPIPE
{
	const prevBin = process.env.ASCII_DIAGRAM_BIN;
	process.env.ASCII_DIAGRAM_BIN = "/bin/false"; // exists, exits 1 without reading stdin
	try {
		await registeredCommand.cmd.handler("x".repeat(200 * 1024), ctxMock);
	} finally {
		if (prevBin === undefined) delete process.env.ASCII_DIAGRAM_BIN;
		else process.env.ASCII_DIAGRAM_BIN = prevBin;
	}
}
assert.ok(
	notifications.at(-1).level === "error" &&
		/exited with code 1/.test(notifications.at(-1).msg),
	"early-exit binary must surface as render failure, not crash",
);
// Test color preservation under NO_COLOR (color override workflow)
{
	const styledMermaid = "```mermaid\ngraph TD\n  classDef hot stroke:red\n  A --> B\n  class A hot\n```";
	const colorTransformed = registeredTransformer(styledMermaid, {
		messageType: "assistant",
		isStreaming: false,
		availableWidth: 80,
	});
	assert.ok(colorTransformed.includes("\x1b[31m"), "Transformer must preserve red ANSI border");

	await registeredCommand.cmd.handler("--color graph TD; A --> B; classDef hot stroke:red; class A hot", ctxMock);
	assert.ok(sentMessages.at(-1).includes("\x1b[31m"), "Command --color must emit red ANSI escapes");

	const toolColorRes = await registeredTool.execute("call-color", {
		dsl: "graph TD; A --> B; classDef hot stroke:red; class A hot",
		color: true,
	}, null, null, { cwd: process.cwd() });
	assert.ok(toolColorRes.content[0].text.includes("\x1b[31m"), "draw_diagram must emit red ANSI escapes when color: true");
}

console.log("All Pi Extension tests passed successfully!");

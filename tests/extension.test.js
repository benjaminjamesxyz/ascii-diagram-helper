import assert from "node:assert";
import asciiDiagramExtension from "../extensions/ascii-diagram.ts";
import { stripTerminalSequences, visibleWidth } from "@earendil-works/pi-tui";

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

// JSON labels never opt into Mermaid directives, including CLI-normalized BOM
// and whitespace. Exercise both tool parameters and explicit plain suppression.
const renderDiagram = (params) =>
	registeredTool.execute("regression", params, null, null, { cwd: process.cwd() });

// Direction cannot turn native input into a flowchart, even behind BOM/space.
const nativeInputs = [
	"ds linkedlist 10 20",
	"ds tree 8 3 10",
	"ds btree 10,20 / 3,5 12,15 25,30",
	"ds array a b",
	"tree\n  root\n    child",
	"stack\n  a\n  b",
	"table\nKey | Value\na | b",
	'{"type":"datastructure","kind":"graph","nodes":["a","b"],"edges":[["a","b"]]}',
	"ds doublylinkedlist 10 20",
	"ds linkedlist left->right end",
	"tree\n  root\n    left->right",
	"stack\n  left->right\n  end",
	"table\nKey | Value\narrow | left->right",
];
for (const style of ["rounded", "sharp", "double", "heavy", "ascii"]) {
	for (const input of nativeInputs) {
		const plain = await renderDiagram({ dsl: input, style, color: false });
		for (const prefix of ["", " \n\uFEFF \n"]) {
			const directed = await renderDiagram({
				dsl: prefix + input, direction: "LR", style, color: false,
			});
			assert.strictEqual(directed.details.diagram, plain.details.diagram, `${style}: ${input}`);
		}
	}
	const dllSpec = {
		type: "datastructure", kind: "doublylinkedlist", nodes: [10, 20],
	};
	const dllDsl = await renderDiagram({ dsl: "ds doublylinkedlist 10 20", style, color: false });
	const dllJson = await renderDiagram({ spec: JSON.stringify(dllSpec), direction: "LR", style, color: false });
	assert.strictEqual(dllJson.details.diagram, dllDsl.details.diagram);
	const customDll = await renderDiagram({
		spec: JSON.stringify({ ...dllSpec, head_label: "start", tail_label: "finish" }),
		direction: "LR", style, color: false,
	});
	assert.ok(customDll.details.diagram.includes("start"));
	assert.ok(customDll.details.diagram.includes("finish"));

	const vertical = await renderDiagram({ dsl: "A --> B", style, color: false });
	const horizontal = await renderDiagram({ dsl: " \n\uFEFF A --> B", direction: "LR", style, color: false });
	const verticalRows = vertical.details.diagram.split("\n");
	const horizontalRows = horizontal.details.diagram.split("\n");
	assert.ok(verticalRows.findIndex((row) => row.includes("A")) < verticalRows.findIndex((row) => row.includes("B")));
	assert.ok(horizontalRows.some((row) => row.includes("A") && row.indexOf("B") > row.indexOf("A")));
	assert.ok(horizontalRows.length < verticalRows.length, "LR changes actual flowchart geometry");
	for (const input of ["graph TD; A --> B", "flowchart TB\nA --> B", "sequenceDiagram\nA -> B: Ping"]) {
		const plain = await renderDiagram({ dsl: input, style, color: false });
		const directed = await renderDiagram({ dsl: ` \n\uFEFF ${input}`, direction: "LR", style, color: false });
		assert.strictEqual(directed.details.diagram, plain.details.diagram, "explicit headers stay authoritative");
	}
}
const graphJson = ` \n\uFEFF ${JSON.stringify({
	type: "architecture",
	title: "Dual-core flowchart architecture",
	containers: [{
		id: "domain",
		title: "graph domain",
		color: "green",
		items: [{ id: "core", name: "graph processor", color: "red" }],
	}],
	connections: [],
})} \n`;
for (const parameter of ["spec", "dsl"]) {
	const colored = await renderDiagram({ [parameter]: graphJson, color: true });
	const plain = await renderDiagram({ [parameter]: graphJson, color: false });
	assert.ok(colored.details.diagram.includes("graph processor"));
	assert.ok(colored.details.diagram.includes("flowchart architecture"));
	assert.ok(colored.details.diagram.includes("\x1b[31m"));
	assert.ok(!plain.details.diagram.includes("\x1b["));
	assert.strictEqual(
		stripTerminalSequences(colored.details.diagram),
		plain.details.diagram,
		"JSON color changes only ANSI styling, not diagram contents",
	);
}

const jsonFlowchart = await renderDiagram({
	spec: JSON.stringify({
		type: "flowchart",
		nodes: [{ id: "A", label: "graph label", color: "red" }],
		edges: [],
	}),
	color: true,
});
assert.ok(jsonFlowchart.details.diagram.includes("graph label"));
assert.ok(jsonFlowchart.details.diagram.includes("\x1b[31m"));
const jsonTable = await renderDiagram({
	spec: JSON.stringify({ type: "table", headers: ["graph"], rows: [["flowchart"]] }),
});
assert.ok(jsonTable.details.diagram.includes("graph"));
assert.ok(jsonTable.details.diagram.includes("flowchart"));

// Real non-flowchart parsers must not receive appended classDef/linkStyle
// directives just because a label happens to mention graph or flowchart.
for (const dsl of [
	"sequenceDiagram\n  A -> B: graph flowchart",
	"tree\n  graph\n    flowchart",
	"stack\n  graph\n  flowchart",
	"table\nKey | Value\ngraph | flowchart",
]) {
	const colored = await renderDiagram({ dsl, color: true });
	const plain = await renderDiagram({ dsl, color: false });
	assert.ok(plain.details.diagram.includes("graph"));
	assert.ok(plain.details.diagram.includes("flowchart"));
	assert.strictEqual(
		stripTerminalSequences(colored.details.diagram),
		plain.details.diagram,
		"non-flowchart output must be unchanged by automatic flowchart styling",
	);
	const transformedLabel = registeredTransformer(`\`\`\`mermaid\n${dsl}\n\`\`\``, {
		messageType: "assistant",
		isStreaming: false,
		availableWidth: 100,
	});
	assert.ok(transformedLabel.includes(colored.details.diagram));
}

// Both genuine header forms retain cyan nodes/blue edges; classes still win.
for (const header of ["graph TD", "flowchart LR"]) {
	const colored = await renderDiagram({ dsl: `${header}; A --> B` });
	const plain = await renderDiagram({ dsl: `${header}; A --> B`, color: false });
	assert.ok(colored.details.diagram.includes("\x1b[36m"), "default cyan nodes");
	assert.ok(colored.details.diagram.includes("\x1b[34m"), "default blue edges");
	assert.strictEqual(stripTerminalSequences(colored.details.diagram), plain.details.diagram);
}
const styledPlain = await renderDiagram({
	dsl: "graph TD; A --> B; classDef hot stroke:red; class A hot",
	color: false,
});
assert.ok(!styledPlain.details.diagram.includes("\x1b["), "false suppresses user class colors");
const directStyleDsl = "graph TD; A --> B; style A stroke:red";
const directStylePlain = await renderDiagram({ dsl: directStyleDsl, color: false });
const directStyleColor = await renderDiagram({ dsl: directStyleDsl, color: true });
assert.ok(!directStylePlain.details.diagram.includes("\x1b["), "false suppresses direct node styles");
assert.ok(directStyleColor.details.diagram.includes("\x1b[31m"), "user node style retains red");
assert.strictEqual(stripTerminalSequences(directStyleColor.details.diagram), directStylePlain.details.diagram);

// Frame geometry is measured in display cells, not ANSI bytes or UTF-16 units.
const frameTheme = {
	fg: (role, text) => `\x1b[${role === "accent" ? 35 : 37}m${text}\x1b[0m`,
};
for (const value of ["x", "界".repeat(36)]) {
	for (const color of [false, true]) {
		const result = await renderDiagram({
			spec: JSON.stringify({
				type: "table",
				headers: ["Label"],
				rows: [[value]],
				color: "cyan",
			}),
			color,
		});
		const diagramLines = result.details.diagram.split("\n");
		const expectedWidth = Math.max(13, ...diagramLines.map((line) => visibleWidth(line) + 2));
		const framed = registeredTool.renderResult(result, { expanded: false }, frameTheme)
			.render(expectedWidth + 20)
			.map((line) => stripTerminalSequences(line).trimEnd());
		assert.strictEqual(visibleWidth(framed[0]), expectedWidth, "header fits diagram display width");
		assert.strictEqual(visibleWidth(framed.at(-1)), expectedWidth, "footer matches header");
		assert.strictEqual(framed.length, diagramLines.length + 2, "frame adds no wrapped diagram rows");
		for (let i = 0; i < diagramLines.length; i++) {
			assert.strictEqual(framed[i + 1], `│ ${stripTerminalSequences(diagramLines[i])}`.trimEnd());
		}
	}
}

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
assert.ok(!sentMessages.at(-1).includes("\x1b["), "unstyled command stays plain when piped");

// Help is handled locally even with other leading flags, not sent to a
// renderer. An executable that always fails makes accidental invocation fail.
{
	const prevBin = process.env.ASCII_DIAGRAM_BIN;
	const messagesBeforeHelp = sentMessages.length;
	process.env.ASCII_DIAGRAM_BIN = "/bin/false";
	try {
		for (const args of [
			"--help",
			"-h",
			"--color --style ascii --help",
			"--style sharp --color -h",
			"--example tree --color -h",
			"--help graph TD; A --> B",
		]) {
			const notificationsBeforeHelp = notifications.length;
			await registeredCommand.cmd.handler(args, ctxMock);
			assert.strictEqual(notifications.length, notificationsBeforeHelp + 1);
			assert.strictEqual(notifications.at(-1).level, "info", "help must not run a failing binary");
			assert.strictEqual(sentMessages.length, messagesBeforeHelp, "help sends no diagram message");
		}
	} finally {
		if (prevBin === undefined) delete process.env.ASCII_DIAGRAM_BIN;
		else process.env.ASCII_DIAGRAM_BIN = prevBin;
	}
}

for (const dsl of [
	'graph TD; A["--help graph"] --> B["-h flowchart"]',
	"table\nKey | Value\ngraph | flowchart",
	graphJson,
]) {
	const beforeCommand = sentMessages.length;
	await registeredCommand.cmd.handler(`--color ${dsl}`, ctxMock);
	assert.strictEqual(notifications.at(-1).level, "info", "colored command renders genuine input kind");
	assert.strictEqual(sentMessages.length, beforeCommand + 1, "DSL help labels still render");
	assert.ok(sentMessages.at(-1).includes("graph"));
	assert.ok(sentMessages.at(-1).includes("flowchart"));
}

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

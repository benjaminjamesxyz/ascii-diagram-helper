import assert from "node:assert";
import asciiDiagramExtension from "../extensions/ascii-diagram.ts";

console.log("Running Pi Extension tests...");

let registeredTool = null;
let registeredCommand = null;
let registeredTransformer = null;

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

console.log("All Pi Extension tests passed successfully!");

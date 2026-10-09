/**
 * ASCII & Unicode Diagram Helper Extension for Pi AI Agent
 *
 * Provides the `draw_diagram` tool and `/diagram` command so the agent
 * can produce perfectly aligned terminal diagrams (flowcharts, sequence diagrams,
 * architecture containers, trees, stacks) without broken boxes or jagged lines.
 */

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { Text, visibleWidth } from "@earendil-works/pi-tui";
import { Type } from "typebox";
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export default function asciiDiagramExtension(pi: ExtensionAPI) {
	// Locate the compiled Rust binary
	function findBinary(cwd: string): string {
		if (
			process.env.ASCII_DIAGRAM_BIN &&
			existsSync(process.env.ASCII_DIAGRAM_BIN)
		) {
			return process.env.ASCII_DIAGRAM_BIN;
		}

		// Check relative to current file
		let currentDir = "";
		try {
			if (typeof __dirname !== "undefined") {
				currentDir = __dirname;
			} else if (import.meta.url) {
				currentDir = dirname(fileURLToPath(import.meta.url));
			}
		} catch {
			// ignore
		}

		const homeDir = process.env.HOME || "";
		const candidates = [
			join(currentDir, "..", "bin", "ascii-diagram"),
			join(currentDir, "..", "target", "release", "ascii-diagram"),
			join(currentDir, "..", "target", "debug", "ascii-diagram"),
			join(homeDir, ".local", "bin", "ascii-diagram"),
			resolve(cwd, "bin", "ascii-diagram"),
			resolve(cwd, "target", "release", "ascii-diagram"),
			resolve(cwd, "target", "debug", "ascii-diagram"),
			"ascii-diagram",
		];

		for (const candidate of candidates) {
			if (existsSync(candidate)) {
				return candidate;
			}
		}

		return "ascii-diagram";
	}
	function withAutoColor(dsl: string): string {
		const isFlowchart = /^\s*(?:graph|flowchart)(?=\s|;|$)/.test(dsl);
		if (isFlowchart && !dsl.includes("classDef")) {
			return `${dsl}\nclassDef default stroke:cyan\nlinkStyle default stroke:blue`;
		}
		return dsl;
	}

	function runDiagramBinary(
		binPath: string,
		input: string,
		args: string[],
		signal?: AbortSignal,
	): Promise<string> {
		return new Promise((resolvePromise, rejectPromise) => {
			const child = spawn(binPath, args, {
				stdio: ["pipe", "pipe", "pipe"],
			});

			if (signal) {
				signal.addEventListener("abort", () => {
					child.kill();
					rejectPromise(new Error("Diagram execution cancelled"));
				});
			}

			let stdout = "";
			let stderr = "";

			child.stdout.on("data", (chunk: Buffer) => {
				stdout += chunk.toString("utf8");
			});

			child.stderr.on("data", (chunk: Buffer) => {
				stderr += chunk.toString("utf8");
			});

			child.on("error", (err) => {
				rejectPromise(
					new Error(
						`Failed to execute ascii-diagram binary at '${binPath}': ${err.message}. Run 'cargo build --release' to compile it.`,
					),
				);
			});

			child.on("close", (code) => {
				if (code === 0) {
					resolvePromise(stdout.trimEnd());
				} else {
					rejectPromise(
						new Error(stderr.trim() || `ascii-diagram exited with code ${code}`),
					);
				}
			});

			// If the binary exits before consuming stdin (bad args, crash, a
			// non-diagram binary on ASCII_DIAGRAM_BIN), the pending write
			// emits EPIPE on stdin. Swallow it: the `close` handler above
			// rejects with the real exit status. Without this handler the
			// unhandled 'error' event crashes the whole extension host.
			child.stdin.on("error", () => {});

			child.stdin.write(input);
			child.stdin.end();
		});
	}

	// Register `draw_diagram` tool
	pi.registerTool({
		name: "draw_diagram",
		label: "ASCII Diagram",
		// Own framing: skips Pi's toolSuccessBg panel paint, which would sit
		// as a colored slab behind the diagram's own colors
		renderShell: "self",
		description:
			"Draws clean, perfectly aligned ASCII and Unicode diagrams (flowcharts, sequence diagrams, architecture boxes, trees, stacks, data structures like binary trees and B-trees) without broken lines or misaligned borders. Accepts Mermaid DSL or JSON specification.",
		promptSnippet:
			"Draw clean, aligned ASCII/Unicode diagrams without broken boxes or lines",
		promptGuidelines: [
			"Use draw_diagram whenever explaining architectures, workflows, processes, sequence interactions, data models, or trees to guarantee proper box alignment and unbroken connector lines.",
			"Pass Mermaid syntax in the `dsl` parameter (e.g., 'graph TD\n A --> B' or 'sequenceDiagram\n Client -> Server: Request') or a JSON diagram object in `spec`.",
			"Do NOT attempt to manually hand-draw ASCII/Unicode box art in conversational text because variable character widths and token streaming corrupt box borders and connectors.",
		],
		parameters: Type.Object({
			dsl: Type.Optional(
				Type.String({
					description:
						"Diagram DSL: Mermaid flowchart ('graph TD' / 'flowchart LR'), sequence ('sequenceDiagram'), tree, stack, table, or five native shorthands: ds tree, ds btree, ds linkedlist, ds doublylinkedlist, ds array. Shorthand values are whitespace tokens; use JSON for complex labels.",
				}),
			),
			spec: Type.Optional(
				Type.String({
					description:
						"JSON DiagramSpec: flowchart, sequence, architecture, tree, table, stack, or datastructure. Eight native kinds: tree, btree, linkedlist, doublylinkedlist, array, queue, heap, graph. Example: {\"type\":\"datastructure\",\"kind\":\"doublylinkedlist\",\"nodes\":[10,20],\"head_label\":\"head\",\"tail_label\":\"tail\"}. Queue uses nodes; heap uses values in supplied level order (no heapify). No native deque.",
				}),
			),
			style: Type.Optional(
				StringEnum(["rounded", "sharp", "double", "heavy", "ascii"] as const, {
					description:
						"Border and line style: 'rounded' (default Unicode curved corners), 'sharp' (sharp box borders), 'double' (double lines), 'heavy' (bold lines), or 'ascii' (7-bit safe ASCII + - |).",
				}),
			),
			direction: Type.Optional(
				StringEnum(["TB", "LR"] as const, {
					description:
						"Headerless flowchart orientation: 'TB' (Top-to-Bottom) or 'LR' (Left-to-Right). Explicit headers take precedence; native DSL and JSON are unchanged.",
				}),
			),
			color: Type.Optional(
				Type.Boolean({
					description:
						"Render node/edge emphasis colors (from classDef/class/style stroke:red or #hex) as ANSI colors. Default: true. Set false for plain output.",
				}),
			),
		}),

		async execute(toolCallId, params, signal, _onUpdate, ctx) {
			const binPath = findBinary(ctx.cwd);
			let inputContent = "";

			if (params.dsl) {
				inputContent = params.dsl;
				// Only headerless arrow syntax opts into flowchart orientation.
				const trimmed = inputContent.trimStart();
				if (
					params.direction &&
					!/^(?:\{|graph|flowchart|sequenceDiagram|tree|stack|memory|table|datastructure|\||ds(?:\s|$))/.test(trimmed) &&
					/->/.test(trimmed) &&
					!/->>|<->|^\s*(?:participant |actor |- )/m.test(trimmed)
				) {
					inputContent = `graph ${params.direction}\n${trimmed}`;
				}
			} else if (params.spec) {
				inputContent = params.spec;
			} else {
				throw new Error(
					"Either 'dsl' or 'spec' parameter must be provided to draw_diagram.",
				);
			}

			const args: string[] = [];
			if (params.style) {
				args.push("--style", params.style);
			}
			// Colors on by default in the TUI; false explicitly suppresses even
			// diagram-defined styles, rather than leaving the binary in auto.
			if (params.color !== false) {
				args.push("--color", "always");
				inputContent = withAutoColor(inputContent);
			} else {
				args.push("--color", "never");
			}

			try {
				const rendered = await runDiagramBinary(
					binPath,
					inputContent,
					args,
					signal,
				);
				return {
					content: [{ type: "text", text: `\`\`\`text\n${rendered}\n\`\`\`` }],
					details: {
						diagram: rendered,
						style: params.style ?? "rounded",
					},
				};
			} catch (err: any) {
				throw new Error(`Diagram rendering failed: ${err.message}`);
			}
		},

		renderCall(args, theme) {
			const style = args?.style ?? "rounded";
			const snippet = args?.dsl
				? args.dsl.split("\n")[0]?.slice(0, 30)
				: "json-spec";
			return new Text(
				`${theme.fg("toolTitle", theme.bold("draw_diagram"))} ${theme.fg("muted", `(${style})`)} ${theme.fg("dim", snippet)}`,
				0,
				0,
			);
		},

		renderResult(result, { expanded: _expanded }, theme) {
			const details = (result.details ?? {}) as {
				error?: string;
				diagram?: string;
			};
			if (details.error) {
				return new Text(
					theme.fg("error", `✗ Diagram Error: ${details.error}`),
					0,
					0,
				);
			}
			const diagram = details.diagram || "";
			if (!diagram) {
				return new Text(theme.fg("dim", "(no diagram)"), 0, 0);
			}

			// Render diagram in a clean subtle frame sized in terminal columns.
			const lines = diagram.split("\n");
			let frameWidth = visibleWidth("╭── Diagram ─");
			for (const line of lines) {
				frameWidth = Math.max(frameWidth, visibleWidth(line) + 2);
			}
			for (let i = 0; i < lines.length; i++) {
				const line = lines[i];
				// ANSI-colored lines pass through raw — wrapping them in the
				// theme text color would break at the diagram's internal
				// resets and leave two-tone artifacts on themed terminals.
				lines[i] = /\x1b\[/.test(line)
					? `${theme.fg("accent", "│")} ${line}`
					: `${theme.fg("accent", "│")} ${theme.fg("text", line)}`;
			}
			lines.unshift(
				theme.fg("accent", `╭── Diagram ${"─".repeat(frameWidth - 12)}`),
			);
			lines.push(theme.fg("accent", `╰${"─".repeat(frameWidth - 1)}`));

			return new Text(lines.join("\n"), 0, 0);
		},
	});

	// Register interactive `/diagram` command
	pi.registerCommand("diagram", {
		description: "Render an ASCII/Unicode diagram directly in the terminal",
		handler: async (args, ctx) => {
			const trimmedArgs = args?.trim() ?? "";
			const usage =
				"Usage: /diagram [--color] [--style rounded|sharp|double|heavy|ascii] [--example <type>] <mermaid-dsl>";

			if (!trimmedArgs) {
				ctx.ui.notify(
					usage,
					"info",
				);
				return;
			}

			let cmdArgs: string[] = [];
			let input = trimmedArgs;

			// Leading flags, in any order; everything after the first
			// non-flag token is the DSL. Style values are validated by the
			// binary (clap lists valid values on a typo) — pass-through keeps
			// one source of truth.
			for (;;) {
				if (/^(?:--help|-h)(?=\s|$)/.test(input)) {
					ctx.ui.notify(usage, "info");
					return;
				}

				let m = input.match(/^--color(?=\s|$)/);
				if (m) {
					// Explicit --color flag: forces ANSI colors even if NO_COLOR is set (no-color.org precedence)
					cmdArgs.push("--color", "always");
					input = input.slice(m[0].length).trim();
					continue;
				}

				m = input.match(/^--style(?=\s|$)(?:\s+(?!-)(\S+))?/);
				if (m) {
					if (!m[1]) {
						ctx.ui.notify(
							"--style requires a value: rounded, sharp, double, heavy, or ascii",
							"error",
						);
						return;
					}
					cmdArgs.push("--style", m[1]);
					input = input.slice(m[0].length).trim();
					continue;
				}

				m = input.match(/^--example(?=\s|$)(?:\s+(?!-)(\S+))?/);
				if (m) {
					cmdArgs.push("example", m[1] ?? "flowchart");
					input = input.slice(m[0].length).trim();
					continue;
				}

				break;
			}

			if (cmdArgs.includes("example") && input) {
				ctx.ui.notify(
					`'--example <type>' takes no diagram text; unexpected: '${input}'`,
					"error",
				);
				return;
			}

			try {
				let dslToRender = input;
				if (cmdArgs.includes("--color")) {
					dslToRender = withAutoColor(input);
				}
				const rendered = await runDiagramBinary(
					findBinary(ctx.cwd),
					dslToRender,
					cmdArgs,
				);
				ctx.ui.notify("Diagram rendered successfully", "info");
				// Print diagram to session
				pi.sendUserMessage(`\`\`\`text\n${rendered}\n\`\`\``);
			} catch (e: any) {
				ctx.ui.notify(`Failed to render diagram: ${e.message}`, "error");
			}
		},
	});

	function transformMermaidInMarkdown(markdown: string, binPath: string): string {
		return markdown.replace(
			/```(?:mermaid)\r?\n([\s\S]*?)```(?:\r?\n`Mermaid diagram not rendered:[^`\n]*`\s*)?/gi,
			(match, dsl) => {
				const trimmed = dsl.trim();
				if (!trimmed) return match;
				try {
					const dslToRender = withAutoColor(trimmed);
					const args = ["--color", "always", "dsl", dslToRender];
					const res = spawnSync(binPath, args, {
						encoding: "utf8",
						timeout: 3000,
					});
					if (res.status === 0 && res.stdout.trim()) {
						return `\`\`\`text\n${res.stdout.trimEnd()}\n\`\`\`\n`;
					}
				} catch {
					// Keep original on error
				}
				return match;
			},
		);
	}

	// In omp, intercept assistant messages and rewrite any Mermaid blocks to ascii-diagram output with color
	if (typeof pi.on === "function") {
		pi.on("assistant_message", async (event) => {
			const binPath = findBinary(process.cwd());
			let changed = false;
			const newContent = event.message.content.map((block) => {
				if (block.type === "text" && block.text.includes("```mermaid")) {
					const replaced = transformMermaidInMarkdown(block.text, binPath);
					if (replaced !== block.text) {
						changed = true;
						return { ...block, text: replaced };
					}
				}
				return block;
			});
			if (changed) {
				return { content: newContent };
			}
		});
	}

	// In upstream Pi, use registerMarkdownTransformer if present
	if (typeof pi.registerMarkdownTransformer === "function") {
		pi.registerMarkdownTransformer((markdown, context) => {
			if (context.isStreaming || context.messageType === "assistant-thinking") {
				return markdown;
			}
			const binPath = findBinary(process.cwd());
			return transformMermaidInMarkdown(markdown, binPath);
		});
	}
}

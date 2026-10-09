/**
 * ASCII & Unicode Diagram Helper Extension for Pi AI Agent
 *
 * Provides the `draw_diagram` tool and `/diagram` command so the agent
 * can produce perfectly aligned terminal diagrams (flowcharts, sequence diagrams,
 * architecture containers, trees, stacks) without broken boxes or jagged lines.
 */

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { Text } from "@earendil-works/pi-tui";
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
		const isFlowchart = dsl.includes("graph") || dsl.includes("flowchart");
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
						"Diagram DSL string: supports Mermaid flowchart ('graph TD' / 'flowchart LR'), sequence ('sequenceDiagram'), tree, or stack syntax. Data structures (binary tree, B-tree) are JSON-only via `spec`.",
				}),
			),
			spec: Type.Optional(
				Type.String({
					description:
						"JSON diagram specification matching the DiagramSpec schema (flowchart, sequence, architecture, tree, table, stack, datastructure — binary tree / B-tree, e.g. {\"type\":\"datastructure\",\"kind\":\"tree\",\"values\":[\"8\",\"3\",\"10\"]}).",
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
						"Flowchart orientation: 'TB' (Top-to-Bottom) or 'LR' (Left-to-Right).",
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
				// If direction specified and not in DSL, prepend if flowchart
				if (
					params.direction &&
					!inputContent.includes("graph ") &&
					!inputContent.includes("flowchart ") &&
					!inputContent.includes("sequenceDiagram")
				) {
					inputContent = `graph ${params.direction}\n${inputContent}`;
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
			// Colors on by default in the TUI (binary auto-detects TTY, but we are
			// piping — force always unless the agent opted out or NO_COLOR is set)
			if (params.color !== false) {
				args.push("--color", "always");
				inputContent = withAutoColor(inputContent);
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

			// Render diagram in a clean subtle frame
			const lines = [
				theme.fg("accent", "╭── Diagram ──────────────────────────────────────"),
				...diagram
					.split("\n")
					.map((line) =>
						// ANSI-colored lines pass through raw — wrapping them in the
						// theme text color would break at the diagram's internal
						// resets and leave two-tone artifacts on themed terminals
						/\x1b\[/.test(line)
							? `${theme.fg("accent", "│")} ${line}`
							: `${theme.fg("accent", "│")} ${theme.fg("text", line)}`,
					),
				theme.fg("accent", "╰─────────────────────────────────────────────────"),
			];

			return new Text(lines.join("\n"), 0, 0);
		},
	});

	// Register interactive `/diagram` command
	pi.registerCommand("diagram", {
		description: "Render an ASCII/Unicode diagram directly in the terminal",
		handler: async (args, ctx) => {
			const binPath = findBinary(ctx.cwd);
			const trimmedArgs = args?.trim() ?? "";

			if (!trimmedArgs) {
				ctx.ui.notify(
					"Usage: /diagram [--color] [--style rounded|sharp|double|heavy|ascii] [--example <type>] <mermaid-dsl>",
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
					if (input) {
						ctx.ui.notify(
							`'--example <type>' takes no diagram text; unexpected: '${input}'`,
							"error",
						);
						return;
					}
					continue;
				}

				break;
			}

			try {
				let dslToRender = input;
				if (cmdArgs.includes("--color")) {
					dslToRender = withAutoColor(input);
				}
				const rendered = await runDiagramBinary(binPath, dslToRender, cmdArgs);
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

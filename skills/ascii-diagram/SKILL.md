---
name: ascii-diagram
description: Generate clean, perfectly aligned ASCII and Unicode diagrams (flowcharts, sequence diagrams, architecture boxes, trees, stacks) using the Rust ascii-diagram engine instead of hand-drawing.
---

# ASCII & Unicode Diagram Helper for Pi

When explaining architectures, system designs, workflows, or sequence interactions in terminal environments, LLM models often attempt to generate raw box drawing art in chat text. This frequently breaks due to:

- Multi-byte Unicode character width discrepancies
- Inconsistent box widths and jagged borders
- Misaligned connector routing and arrowhead placement
- Token generation errors during column spacing

This skill guides you to use the `draw_diagram` tool (or `ascii-diagram` CLI) to render mathematically aligned, non-broken terminal diagrams.

## How to Call `draw_diagram`

### 1. Flowcharts & Graphs

Pass Mermaid-style syntax in `dsl`:

```json
{
  "dsl": "graph TD\n  Client[Web App] -->|HTTPS| Gateway[API Gateway]\n  Gateway --> Auth[Auth Service]\n  Gateway --> Orders[Order Service]\n  Orders --> DB[(PostgreSQL)]",
  "style": "rounded"
}
```

Available shapes:

- `[Text]`: Box / Rectangle
- `(Text)`: Rounded corner box
- `{Text}`: Diamond / Decision node
- `[(Text)]`: Cylinder / Database node
- `([Text])`: Stadium / Pill node
- `[[Text]]`: Subroutine box

Supported directions:

- `TB` (or `TD`): Top to Bottom
- `LR`: Left to Right

### 2. Sequence Diagrams

Pass sequence DSL in `dsl`:

```json
{
  "dsl": "sequenceDiagram\n  participant Client\n  participant Gateway as API Gateway\n  participant DB as Database\n  Client -> Gateway: POST /items\n  Gateway -> DB: INSERT item\n  DB --> Gateway: OK (id: 42)\n  Gateway --> Client: 201 Created",
  "style": "rounded"
}
```

Arrows:

- `->` or `->>`: Synchronous call (`─────►`)
- `-->` or `-->>`: Asynchronous / Return message (`┄ ┄ ┄►`)
- `<->`: Bidirectional interaction (`◄────►`)

### 3. Architecture & Nested Container Diagrams

Pass JSON specification in `spec`:

```json
{
  "spec": "{\"type\":\"architecture\",\"title\":\"Cloud Infrastructure\",\"containers\":[{\"id\":\"vpc\",\"title\":\"VPC: 10.0.0.0/16\",\"layout\":\"row\",\"items\":[{\"id\":\"pub\",\"name\":\"Public Subnet\",\"properties\":[[\"CIDR\",\"10.0.1.0/24\"],[\"Gateway\",\"NAT\"]]},{\"id\":\"priv\",\"name\":\"Private Subnet\",\"properties\":[[\"CIDR\",\"10.0.2.0/24\"],[\"Service\",\"K8s Nodes\"]]}]}],\"connections\":[{\"from\":\"pub\",\"to\":\"priv\",\"label\":\"Internal\"}]}"
}
```

### 4. Hierarchy / Directory Trees

Pass indented lines or bullet points:

```json
{
  "dsl": "tree\n  src/\n    main.rs (CLI entry)\n    canvas.rs (Grid engine)\n    flowchart.rs (Sugiyama layout)\n  Cargo.toml"
}
```

### 5. Memory & Protocol Stacks

Pass stack lines:

```json
{
  "dsl": "stack\n  0xFFFF: Kernel Space\n  0xC000: Stack (grows down)\n  Heap (grows up)\n  0x0000: Text / Code"
}
```

### 6. Register Bitfields & Pinout Tables

Pass markdown table syntax:

```json
{
  "dsl": "table\n| Bit | Field | Access | Reset | Description |\n|---|---|---|---|---|\n| 31:16 | RESERVED | RO | 0x0000 | Reserved |\n| 15:8  | PRESCALER| RW | 0x00   | Timer prescaler |\n| 0     | ENABLE   | RW | 0x0    | Counter enable |"
}
```

## Styling Options

- `rounded` (Default): Smooth curved corners (`╭ ╮ ╰ ╯ ─ │`)
- `sharp`: Sharp box-drawing characters (`┌ ┐ └ ┘ ─ │`)
- `double`: Double lines (`╔ ╗ ╚ ╝ ═ ║`)
- `heavy`: Bold heavy lines (`┏ ┓ ┗ ┛ ━ ┃`)
- `ascii`: Pure 7-bit ASCII (`+ - | > v`) - safe for all terminals and strict markdown renderers

## CLI Usage

You can also run the binary directly via bash:

```bash
./bin/ascii-diagram dsl "graph TD; A --> B"
./bin/ascii-diagram --style ascii example sequence
./bin/ascii-diagram --color always dsl "graph TD; A --> B; classDef hot stroke:red; class A hot"
```

Pi `/diagram` command accepts `--color` to force ANSI colors in the rendered fence
(e.g. `/diagram --color graph TD; A --> B`); default is plain for transcript/copy safety.

## Terminal Display & Width Guidelines (Avoid Diagram Wrapping)

In terminal environments (TUI), lines wider than the user's terminal window (typically **80–120 columns**) wrap around to the next row. When an ASCII or Unicode diagram wraps, all vertical lifelines, arrows, and borders become misaligned and unreadable visual noise.

To ensure diagrams always display cleanly in any terminal:

1. **Target Width $\le 100$ Columns**:
   - Keep node labels concise (e.g. 1–3 words or break lines with `\n`).
   - For sequence diagrams, use concise participant aliases (e.g., `participant CPU as CPU Core`) and limit to 3–4 participants per diagram.
2. **Decompose Complex / Wide Architectures**:
   - If a system has many parallel branches (e.g., 4–5 Bottom Half mechanisms), **do not place them all in a single horizontal rank**.
   - Instead, split into two or three focused diagrams:
     - **Diagram 1 (Flow / High-Level Pipeline)**: End-to-end overview (e.g. HW $\to$ CPU $\to$ Top Half $\to$ Bottom Half Dispatch $\to$ User Space).
     - **Diagram 2 (Deep Dive / Branch Details)**: Focused comparison or detailed subsystem breakdown (e.g. Softirq vs Tasklet, or Workqueue vs Threaded IRQ).
3. **Automatic Rendering in Pi**:
   - The Pi extension automatically intercepts ```` ```mermaid ```` code blocks and renders them with `ascii-diagram`.
   - If a session does not render automatically, run `/reload` in that session to ensure the global package is loaded.

## Supported Syntax & Limitations

- **Edges**: solid `-->`, dashed `-.->`, no arrow `---`, bidirectional `<-->` / `<==>`, reverse `<--`, thick `==>` / `<==>` (heavy line weight); labels via `-->|label|` or `-- "label" -->`
- **Shapes**: `[box]`, `(rounded)`, `((circle))`, `[[subprocess]]`, `{diamond}`, `([stadium])`, `[(database)]`
- Mermaid `subgraph id [Title]` ... `end` renders as a labeled group box (nesting supported; members = nodes first declared inside). Per-subgraph `direction` is ignored
- **Colors**: `classDef`/`class`/`style` `stroke:<name|#hex>` colorizes node borders + edge lines; `linkStyle N stroke:<color>` colorizes edges. Named colors: `red green yellow blue magenta cyan white black grey gray orange purple brown bright*`. Hex: `#ff8800` / `#f80`. Label text stays terminal-default — colors emphasize structure without hurting text readability. Emits ANSI 16 (theme-adaptive) for names, truecolor for hex
- `classDef` / `class` / `style` / `linkStyle`: only `stroke-dasharray` (dashed) and `stroke:<color>` are applied; `fill`, `stroke-width` and other props are ignored
- **Tips**: prefer `TB` for cascades with multiple feedback loops; declare same-rank branch targets left-to-right in escape order so loop-back channels stay clear.

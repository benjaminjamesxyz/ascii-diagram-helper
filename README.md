# ASCII Diagram Helper for Pi AI Agent

A high-precision, Rust-powered ASCII & Unicode diagramming tool and Pi extension designed to solve the common issue where LLMs break terminal diagrams (jagged box borders, misaligned connector lines, font width anomalies, and broken ASCII art).

---

## Why LLM-Generated Diagrams Break in Terminals

When Large Language Models attempt to draw box-drawing character diagrams directly as text tokens:

1. **Multi-byte character width misalignment**: Unicode box characters (`─`, `│`, `┌`, `┐`, `└`, `┘`, `┼`, `►`) take 3 bytes in UTF-8, but 1 terminal cell. Variable tokenization causes the LLM to miscount character columns across lines.
2. **Autoregressive limitations**: Drawing a 2D box requires the closing vertical bar `│` on line N to be aligned with line 1 before the model knows the length of words on lines 2..N-1.
3. **Complex routing**: Orthogonal bends (Manhattan routing), branching (`┬`, `┴`), crossing (`┼`), and parallel lifelines require global geometric coordination that token-by-token generation cannot guarantee.

### The Solution

Instead of forcing the LLM to output character coordinates, the LLM provides **Mermaid DSL** (which models already generate with near-100% syntactic precision) or **declarative JSON**. The Rust engine:

- Computes exact topological layering (Sugiyama layout algorithm)
- Dynamically allocates cell grid space using `unicode-width`
- Reroutes connectors orthogonally with smart junction resolution (`─` meeting `│` automatically becomes `┬`, `┴`, `├`, `┤`, or `┼`)
- Renders character-perfect Unicode or 7-bit ASCII diagrams with zero skew

---

## Features

- **Flowcharts & Graphs (`flowchart` / `graph`)**:
  - Directions: `TB` (Top to Bottom) and `LR` (Left to Right)
  - Shapes: Box `[text]`, Rounded `(text)`, Diamond `{text}`, Database `[(text)]`, Subprocess `[[text]]`, Stadium `([text])`
  - Subgraphs: `subgraph id [Title]` ... `end` render as labeled group boxes (nesting supported); members are nodes first declared inside the block; per-subgraph `direction TB|LR|RL|BT` is applied when the subgraph is edge-isolated from the rest of the diagram (rendered in its own orientation as a self-contained block, Mermaid parity) — otherwise the global direction wins
  - Orthogonal routing with smart junction merging (`┬`, `┴`, `┼`, `├`, `┤`)
  - Shared jump tracks: multi-rank edges from one source (watchdog feeds, debug taps) share a single routing channel instead of overlapping full-width runs
  - Staggered bend bands: overlapping fan-out/fan-in trunks from sibling nodes get separate band rows, preventing adjacent junction characters
  - Chained edges (`A --> B --> C`), edge labels (`-->|label|`), dotted lines (`-.->`), loops
  - Edge styles: solid (`-->`), dashed (`-.->`), no arrow (`---`), bidirectional (`<-->`, `<==>`, `<-.->`), reverse (`<--`), thick (`==>`, `<==>` — heavy glyphs `━ ┃` in Unicode styles, `=` in ASCII style)
  - Style directives: `classDef`, `class`, `style` and `linkStyle` are parsed — `stroke-dasharray` maps to dashed node borders / dashed edges, and `stroke:<color>` maps to border/line emphasis colors. Fill and stroke *widths* are ignored
- **Sequence Diagrams (`sequenceDiagram`)**:
  - Synchronous calls (`->`, `->>`), asynchronous messages (`-->`, `-->>`), bidirectional (`<->`)
  - Self loops (`A -> A: msg`)
  - Control-flow frames: `alt`/`else`, `opt`, `loop`, `par`/`and`, `critical`, `break` — rendered as labeled group boxes with branch dividers
  - Dynamic lifeline column spacing preventing label overflow
- **Architecture & Container Diagrams**:
  - Hierarchical nesting, `row` and `column` layouts
  - Component property badges (`Port: 8080`, `Protocol: gRPC`)
  - Corridor routing: connections spanning multiple containers route through the right-margin corridor instead of slicing through unrelated containers
- **Hierarchy & Directory Trees**:
  - Classic `├──`, `└──`, `│` formatting with annotations
- **Memory & Protocol Stacks**:
  - Memory layouts with address offsets (`0xFFFF`) and growth direction indicators
- **Multiple Styling Modes**:
  - `rounded`: `╭ ─ ╮ │ │ ╰ ─ ╯` (modern smooth terminal look)
  - `sharp`: `┌ ─ ┐ │ │ └ ─ ┘` (classic box-drawing)
  - `double`: `╔ ═ ╗ ║ ║ ╚ ═ ╝` (double lines)
  - `heavy`: `┏ ━ ┓ ┃ ┃ ┗ ━ ┛` (bold lines)
  - `ascii`: `+ - + | | + - +` (pure 7-bit ASCII safe for all terminals)

### Known Limitations

- `classDef` / `class` / `style` / `linkStyle` `fill` colors and `stroke-width` are ignored — label text stays terminal-default by design (readability); only `stroke-dasharray` (dashed) and `stroke:<color>` (border/line color) are applied
- Subgraphs render as group boxes; `style <subgraph-id> stroke:<color>` colorizes the group border (per-subgraph `direction` applies only to edge-isolated subgraphs — subgraphs with edges crossing their border use the global direction)
- Tree / table / stack colors are JSON-only (their DSL has no color syntax): `root/child.color` on tree nodes (branch glyphs, inherited by subtree), `color` on table (grid lines), `color` per stack layer (layer borders)

### Tips for Dense Graphs

Cross-branch edges (watchdog kicks, config broadcasts, debug taps) work correctly at any density but get visually busy past a threshold. Keeping them rare keeps diagrams crisp:

- **Limit cross-branch edges per source to ≤ 2.** Beyond that, every edge still renders (shared corridor track per source), but dash runs cross more bands and gain `┼` crossings
- **Prefer one fan-out over many hops**: `WDG -.-> A` + `WDG -.-> B` renders cleaner than routing a kick through intermediate tasks
- **Split by concern**: put watchdog/telemetry/config wiring in a companion diagram instead of overlaying it on the main dataflow — the task graph stays readable and the cross-branch view gets its own clear picture
- **Use dashed style (`-.->`) for supervisory edges** — visually separates control-plane from data-plane at a glance

---

## Installation

The extension and skill are pure TypeScript/markdown, but the diagram engine is a compiled Rust binary. Install order matters: **build the binary first**, then install the Pi package so the extension can find it.

### 1. Build the binary

Requires [Rust](https://rustup.rs) and Node.js 20+:

```bash
git clone https://github.com/benjaminjamesxyz/ascii-diagram-helper.git
cd ascii-diagram-helper
npm install
npm run build
```

Then put the binary on the extension's search path — `~/.local/bin` is the standard choice:

```bash
mkdir -p ~/.local/bin
cp bin/ascii-diagram ~/.local/bin/
```

(Ensure `~/.local/bin` is on your `PATH`. Alternatively, set `ASCII_DIAGRAM_BIN=/full/path/to/ascii-diagram` in your environment.)

Every later `npm run build` automatically refreshes `~/.local/bin/ascii-diagram` (best-effort — skipped with a note if the directory cannot be created), so the installed copy never goes stale.

### 2. Install the Pi package

```bash
pi install git:github.com/benjaminjamesxyz/ascii-diagram-helper
```

Verify with `pi list`. The extension auto-discovers the binary in this order:

1. `ASCII_DIAGRAM_BIN` environment variable
2. `bin/` or `target/{release,debug}/` inside the installed package
3. `~/.local/bin/ascii-diagram`
4. `bin/` or `target/{release,debug}/` relative to the current working directory
5. `ascii-diagram` on `PATH`

---

## Pi AI Agent Integration

### 1. `draw_diagram` Tool

Registered into Pi's tool registry. The agent uses this tool whenever it needs to explain a concept visually:

```json
{
  "dsl": "graph TD\n  Client[Web App] -->|HTTPS| Gateway[API Gateway]\n  Gateway --> Auth[Auth Service]\n  Gateway --> Orders[Order Service]\n  Orders --> DB[(PostgreSQL)]",
  "style": "rounded"
}
```

Output received by Pi:

```text
             ╭─────────╮
             │ Web App │
             ╰─────────╯
                  │
                  │ HTTPS
                  │
                  ▼
         ╭────────────────╮
         │ Cloudflare CDN │
         ╰────────────────╯
                  │
                  ▼
           ╭─────────────╮
           │ API Gateway │
           ╰─────────────╯
                  │
        ╭─────────┴─────────╮
        │                   │
        ▼                   ▼
╭──────────────╮    ╭───────────────╮
│ Auth Service │    │ Order Service │
╰──────────────╯    ╰───────────────╯
                            │
                  ╭─────────╯
                  │
                  ▼
           ╭────────────╮
           ├────────────┤
           │ PostgreSQL │
           ╰────────────╯
```

### 2. Interactive `/diagram` Command

Users can render diagrams interactively inside Pi's TUI:

```bash
/diagram graph TD; Client --> Server
/diagram --example sequence
/diagram --example stack
```

### 3. Prompt Guidelines & Progressive Disclosure Skill

The bundled skill (`skills/ascii-diagram/SKILL.md`) instructs the agent to reach for `draw_diagram` instead of raw ASCII art when generating diagrams.

---

## CLI Usage

The compiled Rust binary can also be run directly from the command line:

```bash
# Render inline Mermaid DSL
./bin/ascii-diagram dsl "graph LR; Client --> Server --> DB"

# Render from stdin with ASCII styling
echo "graph TD; A --> B" | ./bin/ascii-diagram --style ascii

# Render sequence diagram example
./bin/ascii-diagram example sequence

# Output wrapped in markdown code fence
./bin/ascii-diagram --markdown example flowchart
```

---

## Testing & Building

```bash
# Run both Rust and Pi Extension tests
npm test

# Build release binary
npm run build
```

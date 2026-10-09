---
name: ascii-diagram-qa
description: Systematic QA testing of ascii-diagram diagram types — per-type probe matrix, structured findings, and reviewer audit protocol distilled from the Sprint v0.9.0 hardening loop. Use when the user asks to "test the diagrams", "QA ascii-diagram", "regression test diagram types", or to verify a diagram type / render fix before release.
---

# ascii-diagram QA Tester Skill

Reusable methodology for testing the `ascii-diagram` engine (binary `./bin/ascii-diagram`,
7 diagram types: flowchart, sequence, architecture, tree, stack, table, datastructure).
Distilled from the Sprint v0.9.0 multi-agent hardening loop (50 findings, 7 types,
3 cycles to convergence). Run it before any release gate or after touching a
renderer/parser.

Work from the repo root. Never trust a prior report — re-run everything.

## 1. Per-Type Probe Matrix

Run every probe for **each diagram type**. Record a finding whenever output violates
alignment, encoding, exit-code, or error-message expectations.

### 1.1 Valid specs — three densities

| Probe | Purpose |
|---|---|
| **Minimal** | Smallest legal spec (1 node / 2 participants / 1 row). Catches empty-cell and single-element crashes. |
| **Typical** | The `example <type>` spec. Baseline correctness. |
| **Dense** | Max realistic load: 8+ nodes, multi-hop edges, long chains, wide tables, deep trees. Catches routing collisions, truncation, layout blowups. |

### 1.2 Malformed input

- Truncated JSON, missing required fields, unknown `kind`/`type`
- DSL syntax errors: unterminated node bracket, `A -->` with no target, stray pipes in `ds btree`
- Native graph endpoints that reference undeclared nodes must error, naming the node; Mermaid flowcharts auto-declare endpoints.
- Empty `nodes`/`values` arrays that the type requires non-empty (must give a **clear Err**, not a panic or blank render)

Expected: non-zero exit, actionable error message naming the problem. Never a panic,
never a silently empty diagram.

### 1.3 Edge cases

| Probe | Expected |
|---|---|
| Empty input (`""` / whitespace-only / no lines) | Graceful: either a clear error or a minimal valid frame — never a panic |
| Single element (1 node, 1 row, 1 value) | No off-by-one in borders, indices, or pointer cells |
| Long labels (40–80 chars, longer than terminal width) | Box borders stay aligned; native cells grow horizontally, never wrap |
| CJK labels (e.g. `データベース`, `数据库`) | **Display-width** (double-width) padding — borders must align when printed, not byte/char-counted |
| Emoji labels (e.g. `🚀 deploy`) | Same width-accounting rules as CJK; no ragged right edges |
| Whitespace-only labels, leading/trailing spaces | Native labels quote edge whitespace reversibly; never trim raw identity |

### 1.4 Styles — all 5 values, border-alignment check

For every probe render: `--style rounded` (default), `sharp`, `double`, `heavy`, `ascii`.

Alignment checks per style:
- All box top/bottom borders equal width; corners match the style's glyph set
  (`╭╮╰╯` / `┌┐└┘` / `╔╗╚╝` / `┏┓┗┛` / `+-`); no glyph leakage across styles
- Mixed-weight diagrams: heavy/`stroke-width` levels still resolve junction glyphs
  correctly within the chosen style
- `ascii` style: geometry and markers must be 7-bit ASCII; user Unicode labels remain unchanged. Check ASCII-only fixtures separately from Unicode label fixtures.
- Arrowheads and junctions (`┼`, `┬`, `►`) consistent within one style

### 1.5 Output modes

- `--markdown`: output wrapped in a safe fenced code block; inner content byte-identical to the plain render, with a newline after the closing fence
- `--color always` / `--color never` / default auto: with `never` the output must contain **zero ANSI escape bytes** (`grep -c $'\033'` = 0); with `always`, SGR appears exactly around intended glyphs
- `NO_COLOR=1` suppresses unstyled auto output; explicitly styled input activates auto color by design. Explicit `--color always` forces color; `--color never` suppresses even explicit styling. Tool `color:false` must suppress ANSI too — verify each path rather than treating all auto inputs alike.
- Colored vs plain render of the same spec must differ **only** in escape bytes (strip SGR, compare)

### 1.6 Input modes — all four paths, same result

The same DSL must render byte-identical (modulo flags) through:

1. **Inline**: `./bin/ascii-diagram dsl '<dsl>'` (and trailing-arg form)
2. **File**: `./bin/ascii-diagram render spec.dsl`
3. **Stdin**: `echo '<dsl>' | ./bin/ascii-diagram render -` (and bare no-subcommand stdin)
4. **Example**: `./bin/ascii-diagram example <type>`

Also verify: file path that does not exist → clear `file not found` error (not a silent
render of the path as DSL); `\n`/`\t` escapes unescape identically in inline and file modes.

### 1.7 Native data-structure acceptance

Exercise all eight JSON `datastructure` kinds: `tree`, `btree`, `linkedlist`,
`doublylinkedlist`, `array`, `queue`, `heap`, `graph`. The five shorthands are
`ds tree`, `ds btree`, `ds linkedlist`, `ds doublylinkedlist`, and `ds array`.
Use JSON for complex labels; shorthand tokenization does not support quoting.
Run each focused probe in all five styles against the source-built candidate.

| Probe | Required behavior |
|---|---|
| Binary `root` with only `left` versus only `right`, mixed label widths | Distinct geometry; child stays on its side; equal depths share rows without phantom nodes |
| `ds tree 9007199254740993 9007199254740992 0` | Descending left chain; exact integral comparison preserves labels; also cover negative, signed/zero-padded equals, duplicates, decimals, and lexical fallback |
| `ds btree 40 \| 20 \| 10 30` | Match explicit JSON 40 → 20 → (10,30); pipes define levels, not a flattened cell stream |
| `ds btree 40 \| 10 20 50` | Error names level 2; no diagram stdout; partial child sets remain valid |
| Colored one-/two-/three-key B-tree cells | SGR-stripped output equals plain output, including frame width and connector positions |
| Heap `values:[1,2]`, `[1,3,2,6,4,5]`, and ten nodes | Real binary tree with left first child and equal-depth ranks; preserve supplied order, no heapify |
| DLL one/three nodes, duplicates, numeric/empty/`NULL` values | Separate ordered prev/value/next cells, two null ends, bidirectional links; singleton shares head/tail |
| DLL `head_label`/`tail_label` omitted, customized, empty, and escaped | Defaults `head`/`tail`; explicit empty stays supplied; endpoint text obeys reversible display |
| DLL absent/empty/null/nonsequence `nodes`; bare `ds doublylinkedlist` | Clear error, never panic; JSON and DSL single-token cases agree; `example doublylinkedlist` works |
| Queue custom `front_label`/`rear_label` | Only text changes; arrows stay rightward; `example queue` works; `kind:"deque"` rejects |
| Graph duplicate `nodes:["a","a"]` or `[1,"1"]` | Reject after numeric conversion; error identifies duplicate |
| Graph repeated edges, self-loops, whitespace-distinct keys | Preserve raw identity, edge order/multiplicity, and distinct displayed labels |

For every native cell and head/tail/front/rear endpoint, test LF, CR, tab, ESC,
DEL/C1, quotes, backslashes, edge whitespace, empty strings, CJK, and emoji.
Decode quoted displays to recover original labels; actual LF versus literal `\n`
must differ. Controls must not create physical rows or terminal commands.
Ordinary printable labels stay unchanged. Measure displayed widths once:
untitled array has four rows, singly linked list/queue three, two-source graph
seven. Inspect index 10/100 alignment and connector attachment, not snapshots alone.
Test inline/file/stdin parity and SGR-stripped color parity.
Hierarchy `type:"tree"` is different: multiline names/annotations retain physical
continuation lines, ancestor gutters, indentation, and inherited colors.

Capture the actual extension registration and execute the selected fresh binary.
Compare with/without `direction:"LR"` for `ds linkedlist 10 20`, `ds tree 8 3 10`,
`ds btree 10,20 / 3,5 12,15 25,30`, `ds array a b`, hierarchy `tree`, `stack`,
`table`, graph JSON-in-`dsl`, and native DLL via both `dsl` and `spec`.
The slash fixture checks dispatch parity only, not pipe-level correctness.
Repeat with leading whitespace/BOM and labels containing arrows.
All native outputs must remain identical. Positive control: headerless `A --> B`
is vertical by default and horizontal with LR; explicit graph/flowchart headers
and sequence dispatch remain authoritative. Check geometry, not success alone.
Registered metadata must list eight JSON kinds/five shorthands and DLL endpoint
fields; no `ds queue`, `ds heap`, `ds graph`, or native deque claims.


## 2. Findings Format

One structured entry per finding. No prose-only reports.

```
ID: SEQ-07                      # <TYPE>-<n>, unique, stable across cycles
Severity: blocker|major|minor|nit
Category: alignment|crash|error-message|encoding|color|style|regression|ux
Repro: ./bin/ascii-diagram --style double dsl 'sequenceDiagram
A -> B: データ'                 # EXACT command, copy-paste runnable
Expected: lifelines align under participant headers in all styles
Observed: double-style lifeline drifts 1 column right of the header center
Notes: only double style; rounded/sharp/heavy correct
```

Severity rubric:

| Severity | Meaning |
|---|---|
| **blocker** | Panic, hang, or output so broken it is unusable (misaligned everything, data loss) |
| **major** | Visible defect in a normal use case: wrong alignment, wrong arrow, missing element, misleading error |
| **minor** | Cosmetic or rare-input defect: single style off-by-one, edge-case padding, suboptimal error wording |
| **nit** | Polish: naming, redundant blank lines, doc inconsistencies |

Rules: repro must be minimal (strip everything not needed to trigger); one defect per
finding; state which styles/modes the defect does and does not reproduce in.

## 3. Reviewer Audit Protocol

Tester findings are claims. The reviewer independently verifies:

1. **Re-run every repro** from every finding, exactly as written, in a clean checkout
   of the branch under test. Never trust the tester's observed output.
2. **Reject false positives with written reasons.** A rejection must cite: actual output
   observed, the spec/README line that defines the behavior (or "by design, documented
   in Known Limitations"), and why the expected-observed mismatch does not hold.
   v0.9.0 precedent: 2 of 52 cycle-1 findings were rejected this way.
3. **Check severity against the rubric** (Section 2). Downgrade/upgrade with justification.
4. **Hunt what the tester missed** on the touched code paths: adjacent inputs, the other
   4 styles, the other input modes for every fix. v0.9.0 cycle 2 found 8 follow-up
   findings this way.
5. **Verify the fix, not the claim**: re-run the original repro AND a regression probe
   around it (same construct, one step simpler and one step denser).

**Verdict = satisfied** requires ALL of:
- Zero open blocker / major / minor findings (nits may be deferred to backlog)
- Every repro re-run personally; every rejection reasoned in writing
- Gates green: `cargo test` + `cargo clippy --all-targets` (7 pre-existing baseline
  warnings tolerated) + `cargo fmt --check` on the branch
- Any fixed type re-tested at least once after the fix lands (regression pass)

Loop: test → review → fix → re-test, capped at 3 cycles per type. At the cap, unresolved
majors go to README Known Limitations, not silently dropped.

## 4. Gates & Environment Context

- Binary: `./bin/ascii-diagram` (Rust, `cargo build`); releases sync via `npm run sync:pi`
- Per-branch gate: `cargo test` (193+ tests), `cargo clippy --all-targets`
  (baseline: 7 pre-existing warnings — new warnings are findings), `cargo fmt --check`
- Integration sweep: `cmp` master build vs integration build across all 7 types × 5
  styles — any unintended byte diff is a regression finding
- Findings that are "by design" still need a documented home: README Known Limitations
  entry or an explicit spec note — never silence

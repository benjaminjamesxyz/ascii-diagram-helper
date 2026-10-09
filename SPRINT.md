# Sprint v0.12.1 — Firmware Review Fixes

**Status:** Implementation and integrated verification complete.
**Integration branch:** `sprint/v0.12.1`; base `93fcae5`.
**Coordination:** four GPT implementation workers in isolated worktrees; Scrum Master maintained the deduplicated common ledger at `local://sprint-v0.12.1-ledger.md`. Parent owned merges, final gates, runtime acceptance and release synchronization. No upstream push.

## Fix ledger

| Owner | Review IDs | Implemented behavior | Commits |
|---|---|---|---|
| FlowDev | ALIGN-01/STRESS-01, ALIGN-03, ALIGN-04, ROUTE-01/STRESS-02 | Recursive whole-group packing, member/nesting padding, odd-width decision ingress, intact titles with connected crossing detours; LR groups pack on Y without changing X/ranks. | `81d8d31`, `cc139d27`, `a116d54` |
| ArchDev + parent | ALIGN-02, ALIGN-05/ROUTE-02 | Common top-level frame width without widening nested containers; sibling corridor clearance; route-first label placement retains all three firmware connection labels; legal terminal bends and joined source turns. | `9f802df`, `79ec81e`, `c63c604` |
| CliDev | MODES-01, MODES-02/CLI-OMP-03 | Closing markdown fence ends in LF; options may follow bare files/quoted DSL; literal `--` and argument-error exits preserved. Color help now describes the intentional explicit-style auto behavior. | `ea07415` |
| ExtDev | CLI-OMP-01, CLI-OMP-04, CLI-OMP-05 | Header-anchored Mermaid auto-color guard preserves JSON; tool `color:false` sends `--color never`; local command help; ANSI/CJK-aware result framing. | `31db238` |

Runtime verification caught two follow-ups beyond the initially green suite: overlapping README LR groups and an unjoined narrow architecture source turn. Both are fixed and covered by actual render checks. The incidental moved-child ordering assertion was removed, retaining the original orientation/non-origin geometry invariants (`6f531d5`). The obsolete production `find_subgraph` helper was removed after LR cutover; the remaining test locates its top-level fixture groups directly.

## Intentional behavior

- CLI-OMP-02: explicit diagram colors activate CLI auto color even when piped or with `NO_COLOR`; unstyled auto honors `NO_COLOR`. Explicit `never` suppresses styles; `always` forces color. No precedence reversal.
- COLOR-01: extended names such as `orange` legitimately use truecolor.
- MODES-03: four-space-indented backticks cannot close a CommonMark fence.
- ALIGN-06: sibling node widths remain content-sized; only outer architecture frames are normalized.

## Observed verification

- `cargo test`: **308 passed** (285 library, 23 CLI); targeted source-connectivity and group-containment regressions also passed.
- `cargo clippy --all-targets`: passed with only the three existing warnings (`int_plus_one`, `module_inception`, `too_many_arguments`). `cargo fmt --check`: passed. Release build succeeded.
- Extension behavior suite passed with `ASCII_DIAGRAM_BIN` pointing to the current integrated release, not stale `bin/`.
- Firmware Spec A/B × five styles: complete group titles and all three architecture connection labels retained; colored output is byte-identical after stripping SGR; ASCII variants are 7-bit. Final ASCII sizes: **A 54 rows × 181 columns**, **B 28 × 83**.
- Seven input paths × two specs × five styles: **70 byte-identical runs**. Ten markdown bodies match plain output and close with LF; trailing flags, literal `--`, exit-2 option errors and intended color precedence exercised.
- All **35** built-in example/style renders succeeded; **25** unaffected renderer/style outputs stayed byte-identical to the local v0.12.0 binary.
- Exact README LR input and four-/eight-motor fan-outs exercised in all five styles; narrow column/nested-column/row routes preserve neighboring properties and labels.
- Real installed `pi-tui` Markdown/Text components exercised tool JSON, explicit color suppression, command, transformer and assistant-message output. CJK/ANSI frame geometry and color fidelity passed. This is component-level runtime evidence, **not a live user-session reload claim**.

README flowchart/architecture examples were regenerated from the release CLI; CLI/color/width contracts and bundled skills updated. Wide firmware views may still wrap in narrow terminals; split them into focused diagrams. No automatic fit-to-terminal feature was added.

---

# Sprint v0.12.0 — Post-QA Hardening: 88-Defect Fix Campaign

**Goal:** Fix every verified defect from the 2026-10-09 QA campaign
(`local://qa-board.md`: 94 raw findings → 88 unique after 1 rejection and
dup consolidation; 1 blocker, 21 major, 43 minor, 23 nit), then close the
version drift the v0.11.0 docs sprint left behind (titles said v0.11.0,
manifests and `--version` stayed 0.10.0) by releasing as **v0.12.0**.
**Method:** 7 parallel fix agents with exclusive file ownership
(`local://fix-board.md`), one regression test per fix, followed by a
reviewer + re-tester cycle (cap 3 loops) against the rebuilt release binary.

## Fix slices (cycle 1)

| Owner | Scope | Representative fixes |
|---|---|---|
| fix-parser | `src/parser.rs` | SEQ-W-01 blocker (multibyte byte-slice panic), tree annotation/bullet/root rules, dangling-edge + empty-label edges, sequence message drop, escaped-pipe table cells, prose fallback → hard error |
| fix-main | `src/main.rs` | `example <unknown>` errors (was silent flowchart), exit paths, ERR-01/02, JSON gate |
| fix-leaf | `src/tree.rs`, `src/table.rs`, `src/stack.rs` | tree style-aware connectors (OUT-02/TREE-02), stack/table glyph defects |
| fix-schema | `src/schema.rs`, `src/architecture.rs`, `src/datastructure.rs` | blank render on empty containers, input echo on malformed JSON, routing overwrites |
| fix-flow | `src/flowchart/**` | dense LR/TB border piercing, subgraph nesting/borders, `<br/>`/`\n` multi-line labels + edge labels |
| fix-canvas | `src/canvas.rs`, `src/sequence.rs`, `src/color.rs` | display-width fixes: NFD combining marks, tab expansion, emoji clusters (ZWJ/skin-tone), bidi stripping |
| fix-docs | README, SPRINT, manifests, skills, extension | version sync → 0.12.0, `/diagram --style` pass-through, EPIPE stdin guard in `runDiagramBinary`, by-design behaviors documented in README Known Limitations, `npm run sync:pi` |

## Documentation of by-design behavior (README Known Limitations)

New rows: implicit node auto-declaration (Mermaid parity), empty `[]` label →
id fallback + warning, dangling-edge hard error, tree trailing `/` plain text,
sequence labels never wrap (no `--width`), Unicode width residuals
(`U+FE0F`, legacy-terminal emoji drift, zero-width stripping, combining-mark
cap), unknown JSON fields ignored (`color`, not `stroke`, on connections).

## Integration

1. Reviewer + re-tester wave against the release rebuild; `bin/` synced by
   coordinator before re-tests (repo `bin/` is stale during fix cycle)
2. Full gate: `cargo test` + `cargo clippy --all-targets` (baseline only) +
   `cargo fmt --check` + `node --experimental-strip-types tests/extension.test.js`
3. Version bump `Cargo.toml` + `package.json` + package-lock → `0.12.0` (done in cycle 1)
4. `npm run sync:pi`; note: `~/.omp/plugins/node_modules/ascii-diagram-helper`
   is NOT covered by `sync:pi` — it predates the `prepare` script and carries
   no binary; reinstall that copy (or re-link) manually
5. Merge, tag `v0.12.0`

**OUTCOME:** pending cycle 2 (reviewers + re-testers).

---

# Sprint v0.11.0 — Human-First Documentation & Showcase Overhaul

**Goal:** Redesign `README.md` to be a human-friendly visual showcase.
Deliver immediate visual feedback, comprehensive diagram cards, complete
syntax examples for all 7 types + 7 data structure kinds, clear CLI and Pi
agent instructions, clean limitations and dense-graph tips.
**Synchronized Workflow:**
1. Technical Writer: Drafts `README.md` in worktree `asii-docs-v11`. Every diagram
   example must be generated by the actual `./bin/ascii-diagram` binary.
2. Reviewer: Audits draft against accuracy, syntax, layout, and human readability;
   re-executes all snippets to ensure exact byte match.
3. Approver: Verifies `npm test` gate, checks final formatting, commits and prepares
   release/merge.

---

# Sprint v0.10.0 — Backlog Features: Data-Structure Family Expansion

**OUTCOME (cycle 2, converged):** All 4 backlog deliverables implemented
and verified by independent tester→reviewer pairs:
1. `feat/ds-kinds`: queue (single-cell boxes with front/rear labels, deque support),
   heap (complete binary tree with 2048 node count guard), graph (adjacency
   list buckets with self-loop support), CLI example arms, and styles.
2. `feat/ds-shorthand2`: `ds linkedlist` and `ds array` shorthand forms with
   updated `DS_DSL_HINT` and error messaging.
3. `docs/limitations`: v0.9.0 reviewer-rejected findings compiled into README
   Known Limitations (NO_COLOR precedence, single-pipe fallback, lone color,
   paren-first trees, parser header-only policy).
4. `skill/qa-checklist`: reusable `ascii-diagram-qa` skill with 7-type probe matrix,
   findings format, reviewer audit protocol, and gates context.
All 4 reviewer verdicts `satisfied` (cycle 2 resolved blocker B1 heap count bound).
214 tests green (up from 193), clippy baseline-only (7 warnings), cargo fmt clean.
Released as v0.10.0 pending approval.

**Goal:** clear the feature backlog: three new data-structure kinds (queue,
heap, graph), `ds linkedlist` / `ds array` shorthand, README Known
Limitations for the v0.9.0 rejected findings, and the tester checklist
promoted to a reusable QA skill.
**Integration branch:** `sprint/v0.10.0` (from `master`, post-v0.9.0)
**Merge order:** A → B → C → D → version bump → `master`, tag `v0.10.0`
**Test gate per branch:** `cargo test` + `cargo clippy --all-targets` (7
pre-existing baseline warnings tolerated) + `cargo fmt --check`.

## Parallel-work contract

One `git worktree` per branch. File ownership is disjoint; README and
skills/ are single-owner branches. Features are pre-specified below —
devs implement directly, then per-deliverable tester→reviewer pairs verify
(fix rounds capped at 2) before the release gate.

| Branch | Files owned | Conflicts with |
|---|---|---|
| `feat/ds-kinds` | `src/schema.rs` (DataStructure region), `src/datastructure.rs`, `src/main.rs` (example arms only), tests | none (sole schema/datastructure owner this sprint) |
| `feat/ds-shorthand2` | `src/parser.rs` ONLY inside `parse_datastructure_dsl` + `DS_DSL_HINT`, ds parser tests | none |
| `docs/limitations` | `README.md` | none |
| `skill/qa-checklist` | `skills/ascii-diagram-qa/**` (new) | none |

## Branch A — `feat/ds-kinds` — queue, heap, graph kinds
Pre-specified (follow existing datastructure patterns — see linkedlist/array
render fns):
1. **queue**: `{"kind":"queue","nodes":["a","b","c"],"front_label":"front",
   "rear_label":"rear"}` (labels optional, defaults `front`/`rear`; serves
   deque when both ends labeled). Single row of single-cell boxes; arrow
   into the first box from the front label, arrow from the last box to the
   rear label. Empty `nodes` → clear Err.
2. **heap**: `{"kind":"heap","values":[...]}` — complete binary tree from
   `values` by index (children `2i+1`, `2i+2`), rendered through the
   existing tree layout (build `DsNode` tree programmatically, reuse
   `render_node`). Deep inputs (> 2048 nodes) hit the existing depth guard.
3. **graph**: `{"kind":"graph","nodes":["a","b"],"edges":[["a","b"],["a","c"]]}`
   — adjacency-bucket layout: one row of source node boxes; each source has
   an arrow to a chain of neighbor boxes (duplicated per bucket — textbook
   adjacency list). Unknown edge endpoints → clear `Err` naming them.
   Self-loops render as a loop-back arrow to the same box (or an explicit
   `(self)` cell — pick simpler, document).
4. `example queue|heap|graph` arms in main.rs + README datastructure bullet
   (README edits confined to this branch's own bullet append).
**Accept:** all three kinds render per spec in unicode + ascii styles; unit
+ render tests per kind; existing 193 tests stay green.

## Branch B — `feat/ds-shorthand2` — `ds linkedlist` / `ds array`
Extend `parse_datastructure_dsl`: `ds linkedlist 10 20 30` → kind linkedlist
`nodes`; `ds array a b c` → kind array `values`. Optional trailing
`@Title`? — no, keep minimal. Update `DS_DSL_HINT` to list all four forms
(`ds tree|btree|linkedlist|array ...`). Tests per form incl. malformed.
**Files:** `src/parser.rs` (parse_datastructure_dsl region only), tests.

## Branch C — `docs/limitations` — README Known Limitations
Compile the v0.9.0 reviewer-rejected findings + deferred nits into README
Known Limitations: NO_COLOR vs explicit `--color always` precedence (flag
wins, no-color.org compliant); `A | B; no pipe here` single-pipe dsl falls
to tree; lone `color:` directive passes through; tree `(y) x` paren-first
renders empty-name node [superseded v0.12.0: headerless flat prose now
errors ("unrecognized diagram input", CLI-04); as tree input — via `tree`
header or an indented headerless tree — `(y) x` renders verbatim as the
node name]; header-only diagram policy per type; BT/RL and
subgraph-direction notes (already present — keep). Prose only, NO code.
**Files:** `README.md`.

## Branch D — `skill/qa-checklist` — tester checklist as reusable skill
New `skills/ascii-diagram-qa/SKILL.md` (+ frontmatter: name, description
with trigger phrases) encoding the v0.9.0 tester methodology: per-type probe
matrix (valid/malformed/edge/CJK-emoji/5 styles/markdown/color modes/input
modes/example), findings format (id, severity, repro, expected, observed),
reviewer audit protocol (re-run repros, reject false positives with reasons,
severity rubric, satisfaction verdict). Distilled from the sprint tester
prompts; no code.
**Files:** `skills/ascii-diagram-qa/**`.

## Integration & loop
1. Merge A → B → C → D into `sprint/v0.10.0` (`--no-ff`); full gate
2. Per-deliverable tester → reviewer pairs (4 pairs) verify acceptance
   criteria + regression on the integration build
3. Fix rounds capped at 2; then gate report → **human approval** → bump
   `0.10.0`, `npm run sync:pi`, merge `--no-ff` to `master`, tag `v0.10.0`

## Post-sprint backlog (not this sprint)
- `--strict` flag (opt out of lenient sequence frame auto-commit)
- `Option<BoxStyle>` specs (true explicit-style precedence)
- Renderer warnings channel (Vec<String> instead of eprintln)
- BT/RL mirrored layouts; mixed-direction clusters; global crossing minimization
- `ds queue|heap|graph` shorthand (needs Branch A kinds first)

---

# Sprint v0.9.0 — Ultimate Test: Multi-Agent Hardening Loop

**OUTCOME (cycle 3, converged):** 50 findings accepted across 7 types
(2 blockers, 15 majors, ~25 minors, 8 nits); 48 fixed + verified by
per-type tester→reviewer loops, 2 rejected as false positives (cycle 1),
8 cycle-2 follow-up findings found and fixed. All 6 re-tested types
verdict `satisfied` (stack clean both cycles). 193 tests green,
clippy baseline-only, fmt clean. Released as v0.9.0 pending approval.

**Goal:** test-driven quality sprint across all 7 diagram types. Tester →
reviewer → engineer → developer per type, looping (hard cap 3 cycles) until
every reviewer is satisfied (zero open blocker/major/minor). No new features —
fixes only. Release `v0.9.0` gated on human approval.
**Integration branch:** `sprint/v0.9.0` (from `master`)
**Test gate per branch:** `cargo test` + `cargo clippy --all-targets` (7
pre-existing baseline warnings tolerated) + `cargo fmt --check`.

## Cycle policy
- Cycle = test → review → design → implement → integrate → re-test changed
  types. Loop until all reviewer verdicts `satisfied`; HARD CAP 3 cycles.
- Findings require exact repro commands; reviewers re-run repros and reject
  false positives with reasons. Agents re-verify; reports are never trusted
  blindly.

## File ownership (shared files batched at integration)

| Branch | Owns | Shared-file policy |
|---|---|---|
| `fix/flowchart` | `src/flowchart.rs`, `src/flowchart/*`, flowchart tests | parser.rs/canvas.rs hunks → batched |
| `fix/sequence` | `src/sequence.rs`, sequence tests | parser.rs hunks → batched |
| `fix/architecture` | `src/architecture.rs`, arch tests | schema.rs hunks → batched |
| `fix/tree` | `src/tree.rs`, tree tests | — |
| `fix/table` | `src/table.rs`, table tests | parser.rs hunks → batched |
| `fix/stack` | `src/stack.rs`, stack tests | parser.rs hunks → batched |
| `fix/datastructure` | `src/datastructure.rs`, ds tests | parser.rs/schema.rs hunks → batched |
| integration (scrum master) | `src/parser.rs`, `src/canvas.rs`, `src/schema.rs`, `src/main.rs`, `src/lib.rs`, `README.md`, `SPRINT.md` | applied as one `fix/shared` batch |

Developers whose design requires shared-file hunks note them in the branch
report; the scrum master applies them post-merge to avoid N-way conflicts.

## Integration
1. Merge fix branches in table order → `sprint/v0.9.0` (`--no-ff`)
2. Apply batched shared-file hunks, full gate + master-vs-integration render
   `cmp` sweep across all 7 types × 5 styles
3. Re-test changed types (next cycle) until reviewers satisfied or cap
4. Gate report → **human approval** → bump `0.9.0`, `npm run sync:pi`, merge
   `--no-ff` to `master`, tag `v0.9.0`

## Post-sprint backlog (not this sprint)
- Findings rejected as "by design" get README Known Limitations entries
- Tester checklist promoted to a reusable `skills/` QA skill

---

# Sprint v0.8.0 — Data-Structure Diagrams Expansion

**Goal:** round out the `type:"datastructure"` family (tree/btree landed in
v0.7.x): classic linear structures, low-friction DSL shorthand, node emphasis
colors.
**Integration branch:** `sprint/v0.8.0` (from `master`)
**Merge order:** A → C → B → version bump → `master`, tag `v0.8.0`
**Test gate per branch:** `cargo test` + `cargo clippy --all-targets` clean.

## Parallel-work contract

One `git worktree` per branch (`git worktree add ../asii-ds-<x> feat/<branch>
master`). A branch may only edit files it owns. README edits are append-one-
bullet at the end of the datastructure Features section; the integrator
resolves trivial adjacent-line conflicts. An integration reviewer performs a
merge dry-run on `sprint/v0.8.0` before anything touches `master`.

| Branch | Files owned | Conflict risk |
|---|---|---|
| `feat/ds-list-array` | `src/schema.rs` (DsKind + new spec fields), `src/datastructure.rs` (new render fns + dispatch arms), tests | schema.rs/datastructure.rs overlap with C — A is additive; rebase on C if hunks collide |
| `feat/ds-dsl-shorthand` | `src/parser.rs` (`ds` dispatch + `parse_datastructure_dsl`), tests | low |
| `feat/ds-node-colors` | `src/schema.rs` (DsNode.color), `src/datastructure.rs` (paint threading), `src/lib.rs` (call-site only), tests | schema.rs/datastructure.rs overlap with A |

## Branch A — `feat/ds-list-array` — linked list + array kinds
**Problem:** datastructure covers only tree/btree. Classic linear structures
need the same textbook rendering.
**Fix:** add `DsKind::LinkedList` and `DsKind::Array`:
- `linkedlist`: new spec field `nodes: Vec<String>` + optional `head_label`
  (default `"head"`). Single-row left→right layout: head label, arrow into the
  first node box (two cells: value │ pointer `●`), theme arrow glyph between
  boxes, last pointer cell shows `∅` (ASCII fallback `X` or `NULL`).
- `array`: render `values: Vec<String>` as cells in one boxed row with
  separators; centered index ruler row (`0 1 2 …`) above the cells.
**Accept:** `{"type":"datastructure","kind":"linkedlist","nodes":["10","20","30"]}`
renders head→chain→NULL; `{"kind":"array","values":["a","b","c"]}` renders
ruler + cells. Unit tests for both kinds incl. ascii fallback. Gate green.
**Files:** `src/schema.rs`, `src/datastructure.rs`, tests.

## Branch B — `feat/ds-dsl-shorthand` — `ds` DSL for datastructure
**Problem:** datastructure diagrams are JSON-only; quick sketches need less
friction (`ds tree 8 3 10` currently errors with a JSON hint).
**Fix:** `parse_datastructure_dsl` in parser.rs, dispatched on first token
`ds` (replace the current JSON-only error path):
- `ds tree <v...>` → kind tree, `values` (BST build) — `ds tree 8 3 10 1 6`
- `ds btree <rootkeys> | <next-level cells> | ...` — pipe-separated levels,
  comma-separated keys/cells, children assigned level-order (BFS). Example:
  `ds btree 10,20 | 3,5 12,15 25,30`. Permissive arity: fewer children than
  keys+1 renders as-is (same as JSON path).
Malformed input → actionable error naming the expected syntax.
**Accept:** `ascii-diagram dsl 'ds tree 8 3 10 1'` output byte-identical to
the equivalent JSON spec render; `ds btree 10,20 | 3,5 12,15 25,30` renders
the 3-node btree. Parser tests incl. malformed inputs. Gate green.
**Files:** `src/parser.rs`, tests.

## Branch C — `feat/ds-node-colors` — per-node color support
**Problem:** other diagram types support node colors; datastructure nodes are
mono, so path/route highlighting in dense trees is impossible.
**Fix:** `DsNode.color: Option<Color>` (JSON). Thread `colored: bool` through
`DataStructureRenderer::render(colored)`; paint a node's border glyphs (and
the connector glyphs its subtree owns) with the nearest colored ancestor,
overridable per node — same semantics as `TreeRenderer`'s `paint` + color
inheritance. Label text stays default.
**Accept:** colored render emits SGR around colored node glyphs; plain render
is byte-identical to current output (zero ANSI). Tests both modes. Gate green.
**Files:** `src/schema.rs`, `src/datastructure.rs`, tests.

## Integration
1. Reviewer merge dry-run: `sprint/v0.8.0` ← A → C → B (`--no-ff`), gate run
2. Apply review findings, then merge `sprint/v0.8.0` → `master` (--no-ff)
3. Bump `Cargo.toml` + `package.json` → `0.8.0`, `npm run sync:pi`, tag `v0.8.0`

## Post-sprint backlog (not this sprint)
- `ds linkedlist` / `ds array` shorthand (needs Branch A kinds first)
- Queue/deque kind (front/rear markers), heap kind (array-as-tree dual view)
- Graph (adjacency) kind: bucket row + neighbor chains

---

# Sprint v0.6.0 — Clear the 5 Remaining Minor Items

**Goal:** zero undocumented limitations in README Known Limitations.
**Integration branch:** `sprint/v0.6.0` (from `main`)
**Merge order:** A → B → C → D → E → version bump → main, tag `v0.6.0`
**Test gate per branch:** `cargo test` green + `npm test` green before merge.

## Parallel-work contract

Each branch owns a disjoint file set — multiple agents can work simultaneously
via `git worktree add ../asii-<name> feat/<name>`. No branch may edit files
owned by another *active* branch. README edits are centralized in Branch E to
avoid merge conflicts.

| Branch | Files owned | Conflicts with |
|---|---|---|
| `feat/linkstyle-fill` | `src/parser.rs` (linkStyle arm only) | — |
| `feat/stroke-width-levels` | `src/schema.rs`, `src/canvas.rs` (draw_box_inner), `src/theme.rs`, `src/parser.rs` (prop_thick), test literals | rebase on A if parser.rs hunk overlaps |
| `feat/crossing-junctions` | `src/canvas.rs` (dash stamp paths), `src/theme.rs` | rebase on B (both touch canvas.rs — different functions) |
| `feat/dense-track-routing` | `src/flowchart/tb.rs` | — |
| `docs/subgraph-direction` | `src/flowchart/tests.rs`, `README.md` | — |

## Branch A — `linkStyle fill` → edge color alias
**Problem:** `linkStyle N fill:red` silently ignored.
**Fix:** alias `fill` to edge line color (Mermaid links have no fill; terminal
edge color = stroke). Same behavior as `stroke:`.
**Accept:** `linkStyle 0 fill:red` + colored render → red edge SGR. Test.
**Files:** `src/parser.rs` (linkStyle arm), one test.

## Branch B — graduated `stroke-width` levels
**Problem:** `stroke-width` binary (≥2px heavy, else default). No weight above heavy.
**Fix:** `border_level`: `0` normal, `1` heavy `┏━┓` (≥2px), `2` double `╔═╗` (≥3px).
`Canvas::draw_box_inner(thick: bool)` → `weight: u8`; render resolves glyph set
per weight (heavy path exists via `cell.thick`; double path via
`BoxStyle::Double` glyph table — add `cell.double` or map weight→thick flag
extension).
**Accept:** 2px → `┏━┓`; 3px → `╔═╗`; 1px → default. Parse + render tests.
**Files:** schema (field type), parser (`prop_thick` → `prop_border_level`),
canvas, theme, ~21 test literals (mechanical), README line.

## Branch C — crossing junction resolution
**Problem:** dashed horizontal run stamps `╌` over a solid vertical line cell —
vertical stroke lost at crossing (renders `╌` instead of `┼`).
**Fix:** in dashed line stamp paths, when target cell already `is_line` with
perpendicular conn, **merge conn** instead of overwriting the glyph — existing
`resolve_line_glyph` then renders `┼` (or heavy `╬` when either side thick).
Same treatment for dashed vline × solid hline.
**Accept:** dense graph: dash run crossing a solid fan-out trunk shows `┼`,
vertical strokes continuous above+below. Test.
**Files:** `src/canvas.rs`, maybe `src/theme.rs`, one render test.

## Branch D — dense routing: track-side selection
**Problem:** shared jump track always right margin — left-heavy feeds (targets
mostly left of source) drag long horizontal runs across more bands.
**Fix:** pick track side (left corridor vs right corridor) by majority of
target centers relative to source; mirror the existing right-corridor logic.
Keep ≤2-per-source guidance; this reduces run length + band crossings for the
common left-feed case.
**Accept:** left-heavy dense graph: track on left, fewer crossing glyphs vs
before (assert track column < source column). Test + before/after manual check.
**Files:** `src/flowchart/tb.rs` (jump_tracks block), one test.

## Branch E — nested isolated subgraph direction + docs
**Problem:** "direction requires isolation" boundary needs verification of the
nested case (isolated child inside a non-isolated parent) + honest docs.
**Fix:** verify nested-child move works (collect_sgs recurses unmoved
subtrees); add tests; README: boundary table — what applies when.
**Accept:** test: isolated child in non-isolated parent renders in own
orientation; README boundary documented.
**Files:** `src/flowchart/tests.rs`, `README.md`.

## Integration
1. Merge order A → B → C → D → E into `sprint/v0.6.0` (no-ff, one merge commit each)
2. Full gate: `npm test`
3. Bump `package.json` + `Cargo.toml` → `0.6.0`
4. `npm run sync:pi`
5. Merge `sprint/v0.6.0` → `main` (--no-ff), tag `v0.6.0`

## Post-sprint backlog (not this sprint)
- Global crossing minimization in router (full dense-graph solution)
- Mixed-direction cluster layout (supernode approach)
- Horizontal-band `┼` between two dashed runs (both-dashed crossing glyph)

---

# Sprint v0.7.0 — Backlog Sprint

**Goal:** clear the v0.6.0 post-sprint backlog (scoped versions).
**Integration branch:** `sprint/v0.7.0` — merge order A → B → C → bump → `master`, tag `v0.7.0`.

| Branch | Files owned | Scope |
|---|---|---|
| `feat/dashed-crossing-glyph` | `src/canvas.rs` | Dash × dash crossing: single crossing cell becomes a solid cross (pattern sacrificed at one cell, both strokes continuous). Small. |
| `feat/barycenter-reorder` | `src/flowchart.rs`, `src/flowchart/tb.rs`, `src/flowchart/lr.rs` | Sugiyama crossing reduction: reorder nodes within layers by neighbor barycenter, 2 down/up sweeps, deterministic tiebreak. Medium — biggest visual win for dense graphs. |
| `feat/supernode-clusters` | `src/flowchart.rs`, `src/flowchart/tb.rs` | Non-isolated subgraph with own `direction` → cluster block + phantom node in layout, external edges re-target to phantom. Timeboxed: fallback = current isolation behavior. High risk. |

Acceptance: `cargo test` + `npm test` green per branch; before/after visual check for B and C.

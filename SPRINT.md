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
| `feat/ds-node-colors` | `src/schema.rs` (DsNode.color), `src/datastructure.rs` (paint threading), tests | schema.rs/datastructure.rs overlap with A |

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

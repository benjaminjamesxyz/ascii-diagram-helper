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

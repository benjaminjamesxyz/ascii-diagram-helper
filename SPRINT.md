# Release v0.13.0 — Native Data Structures

**Distribution:** annotated upstream Git tag `v0.13.0`, following the existing source/tag release process. GitHub supplies source ZIP/tar archives from the tag; no npm registry publication or separately uploaded binary is implied. Version metadata in Cargo/npm manifests and lockfiles is `0.13.0`.

This minor pre-1.0 release adds native doubly linked lists and closes all ten data-structure sprint improvements below. Rounded Unicode remains the default; explicit styles remain authoritative.

**Compatibility:** binary tree/heap geometry and special-label presentation change intentionally. B-tree shorthand rejects declared-level overflow; graph input rejects duplicate identifiers. Rust `DataStructureSpec` struct literals need the new optional `tail_label` or a default struct update. Decimal comparison and JSON number coercion remain unchanged.

The sprint's acceptance results below refer to the pre-version-bump candidate. Historical release and sprint evidence is retained, not relabeled as fresh execution.

## Versioned release verification

- Cargo manifest/lockfile, npm manifest/lockfile/root package, and CLI runtime agree on `0.13.0`.
- `cargo build --release --locked`, `cargo test --locked` (**338 passed**), `cargo fmt --check`, and `cargo clippy --all-targets --locked` passed. Clippy retains only the three pre-existing warnings (`int_plus_one`, `module_inception`, `too_many_arguments`).
- The extension suite passed against the rebuilt release binary.
- Release CLI smoke verified all eight exact README native diagrams, forty native-kind/style renders, default rounded Unicode, explicit ASCII preservation, and sparse B-tree DSL/JSON topology equality.
- `npm pack --ignore-scripts` validated the local package contents: 33 files including the native binary, Rust sources, extension, bundled skills, tests, and documentation. This packaging check is not npm publication.
- Repository/release binaries share SHA-256 `6f209ccb64b43e5689db24aaf9bd98f726af5c144dc8f02c5f88eafc847e05aa`.

Upstream inspection found annotated tags but no GitHub Release objects or release workflow. Git SSH access is available; GitHub CLI has no authenticated host. Publication therefore uses the established source/tag channel, not an unverified GitHub Release asset upload. Installed plugins and running sessions are outside this rollout.

---

# Data-structure correctness sprint — 2026-10-09

**Status:** Complete. All ten board items passed parent integration gates and independent final source review/runtime acceptance. No unresolved board findings.

## Goal and coordination

Improve semantic fidelity and label safety across native data structures, add native doubly linked lists, and preserve the correct renderer through the extension.

- Eight parallel QA agents exercised tree/hierarchy, B-tree, singly linked list, the doubly linked use case, array, queue, heap, and graph.
- Three independent finding reviewers reproduced tree-family, linear, and graph/integration defects. Their own runs covered 100 tree-family CLI probes, 168 linear CLI probes, and 40 graph CLI plus 90 fresh-extension calls. These are baseline defect-verification counts, not post-fix passes.
- ScrumMaster consolidated the reviewed findings into this ten-item sprint with no deferred rows. Shared label defects count once, not once per structure.
- Four parallel implementation tracks had exclusive files: NativeRenderer (`src/datastructure.rs`), ParserSchema (`src/parser.rs`, `src/schema.rs`, `src/main.rs`), Hierarchy (`src/tree.rs`), and ToolIntegration (extension, extension tests, bundled skills). Parent owned integration, README, this board, and the repository binary.
- The implementation sprint did not bump versions, commit, push, publish packages, install plugins, or reload sessions. The subsequent v0.13.0 upstream source/tag rollout is recorded above.

## Reviewed improvement board

All ten rows are **closed**. Each has both implementation evidence and independent acceptance below; none closed solely from a developer receipt.

| ID | Priority | Verified baseline problem | Implemented outcome | Track |
|---|---|---|---|---|
| DS-01 | P1 | Unary binary children lose left/right identity; partial heaps compress one child's rank. | Side-correct unary placement and consistent binary ranks without phantom subtrees. | NativeRenderer |
| DS-02 | P1 | Adjacent integral labels above 2^53 compare equal through f64 and create wrong BST topology. | Exact sign/magnitude integral comparison before existing decimal/lexical fallback. | NativeRenderer |
| DS-03 | P1 | B-tree shorthand flattens pipe levels and silently reparents cells. | Each level consumes only the previous frontier; numbered overflow errors replace silent reparenting. | ParserSchema |
| DS-04 | P1 | Colored multi-key separators inflate measured box width. | Width calculations exclude ANSI; colored/plain geometry agrees. | NativeRenderer |
| DS-05 | P1 | Controls break native frames/endpoints; whitespace graph keys become visually identical. | One reversible quoted display policy; all control bytes escape, widths use displayed text, raw identity stays intact. | NativeRenderer |
| DS-06 | P1 | Duplicate graph identifiers split one logical vertex across contradictory adjacency rows. | Named duplicate-key error after numeric coercion; repeated edges and self-loops remain valid. | NativeRenderer |
| DS-07 | P2 feature | Doubly linked lists require a generic flowchart workaround. | Native `doublylinkedlist` JSON, shorthand, and example; prev/value/next cells, head/tail, bidirectional links, and null ends. | NativeRenderer + ParserSchema |
| DS-08 | P2 | Multiline hierarchy labels lose ancestor/sibling gutters. | Physical continuation lines retain nested and last-child prefixes, tabs, and inherited colors. | Hierarchy |
| DS-09 | P1 | A flowchart direction option rejects or silently reinterprets native DSL/JSON. | Header injection applies only to headerless arrow input; explicit diagram kinds retain their renderer. | ToolIntegration |
| DS-10 | P2/P3 | Tool metadata claims JSON-only data structures; queue wording claims unsupported deque behavior. | Eight native JSON kinds/five shorthands documented; native DLL/display/parser contracts and refreshed examples; queue claims corrected. | ToolIntegration + parent |

### Reproduction and acceptance fixtures

Use `./bin/ascii-diagram --style STYLE --color never dsl 'INPUT'` for these inputs, with `STYLE` set to each of rounded, sharp, double, heavy, and ascii. JSON control escapes below are input data, not raw terminal controls.

| Board | Minimal input or scenario | Required result |
|---|---|---|
| DS-01 | `{"type":"datastructure","kind":"tree","root":{"value":"P","left":{"value":"C"}}}` versus `right` | C appears strictly left/right respectively. Heap `[1,3,2,6,4,5]` puts 6/4/5 on one rank. |
| DS-02 | `ds tree 9007199254740993 9007199254740992 0` | Descending left chain, not two root children. Negative/huge/signed/zero-prefixed labels retain exact integral order. |
| DS-03 | `ds btree 40 \| 20 \| 10 30`; `ds btree 40 \| 10 20 50` | First matches explicit 40-to-20-to-10/30 JSON; second exits nonzero naming level 2. |
| DS-04 | `{"type":"datastructure","kind":"btree","btree_root":{"keys":[10,20],"color":"red"}}` | `--color always` stripped of SGR equals `--color never`; each root box row is 11 columns. |
| DS-05 | `{"type":"datastructure","kind":"linkedlist","nodes":["a\nb","a\\nb"]}` | Distinct reversible quoted labels in intact three-row frames. Repeat across all native kinds and endpoints with CR/tab/ESC/DEL/C1, quotes, edge whitespace, and Unicode. |
| DS-06 | `{"type":"datastructure","kind":"graph","nodes":["a","a","b"],"edges":[["a","b"]]}` | Error names duplicate a. Numeric/string collision `[1,"1"]` also errors; unique raw whitespace keys stay distinct. |
| DS-07 | `ds doublylinkedlist 10`; `ds doublylinkedlist 10 20 30` | Two null endpoint pointers; singleton has head/tail on one node, three nodes have four non-null pointers and both link directions. |
| DS-08 | `{"type":"tree","root":{"name":"root","children":[{"name":"one\ntwo"},{"name":"last"}]}}` | Continuation `two` retains the pending-sibling gutter, not column zero. |
| DS-09 | Fresh `draw_diagram` with `dsl:"ds linkedlist 10 20",direction:"LR",color:false` | Same output as omitted direction; tree/stack/table/JSON and new DLL also preserve kind. Headerless `A --> B` still honors LR. |
| DS-10 | Fresh registered tool metadata; native guide; `example doublylinkedlist`; `example queue` | Capability descriptions match real accepted inputs; native deque remains unsupported and is not advertised. |

The shared label fix deliberately changes presentation of special labels. Controls, backslashes, quotes, and edge whitespace get JSON-compatible quoted escapes. Printable Unicode and raw comparison/lookup values are preserved. Native cells remain single-line; hierarchy labels retain physical multiline rendering. Exact numeric comparison applies to integral tokens, not arbitrary decimal arithmetic. JSON numeric coercion remains unchanged; use string labels for larger exact numbers.

## Parent integration verification

- `cargo build --release --locked` passed; the resulting binary was copied to repository `bin/ascii-diagram`.
- Final `cargo test --locked`: **338 passed** (316 library, 22 CLI; no doctests), after review removed two incidental example tests.
- `cargo fmt --check` passed after the centralized formatting pass.
- `cargo clippy --all-targets --locked` passed with only the three existing warnings: `int_plus_one`, `module_inception`, `too_many_arguments`.
- Existing extension suite passed against the new release binary.
- Actual CLI smoke passed **130 board assertions** across all five styles, including topology, integral ordering, B-tree levels, colored widths, special labels across all eight kinds, graph errors, DLL pointers, and hierarchy continuation.
- Fresh extension smoke passed **160 dispatch/orientation/CLI/frame checks**, including real `Text` result rendering, and the `/diagram` native DLL command.
- Compared 60 built-in example/style outputs with the frozen source baseline: **55 unchanged**; the five heap style outputs changed intentionally for side/rank fidelity. All 30 non-data-structure example/style outputs remained identical.
- README BST, heap, and native DLL examples were captured from the new binary, not hand-drawn.

## Final independent verification

Three independent reviewers and three runtime testers assessed the integrated candidate in parallel. All assigned criteria passed.

| Acceptance track | Board coverage | Personally observed evidence | Verdict |
|---|---|---|---|
| FinalNativeReview | DS-01/02/04/05/06/07 and queue wording | Complete native production/test source review; no runtime claims. | PASS |
| FinalContractReview | DS-03/07 API/08/09 and help | Parser/schema/hierarchy/extension consumer traces and regression-test review. One low-value test finding closed by removal. | PASS |
| FinalTreeTester | DS-01/02/03/04/08; tree/B-tree/heap portion of DS-05 | 755 candidate CLI calls, 6 unchanged-baseline calls, 601 semantic assertions; all five styles. Includes 90-digit integer topology, 96-node chain attachment/ranks, and a 2,050-value depth-guard error. | PASS |
| FinalLinearTester | Linear portion of DS-05, DS-07, queue/DLL docs | 634 candidate CLI calls, 60 unchanged-baseline comparisons, 2,629 assertions; all five styles. Includes every C0/DEL/C1 control, index 100, endpoint preservation, pointer fields, errors, and five input modes. | PASS |
| FinalGraphToolTester | Graph portion of DS-05, DS-06, DS-07 integration, DS-09/10 | 150 captured candidate CLI calls plus 5 baseline calls; 655 fresh tool executions, 5 real Text frames, 10 command executions, 1,442 assertions. | PASS |
| FinalDocsReview | DS-10 and documented cross-board contracts | 281 candidate CLI probes; 38 exact comparisons, including all eight README output blocks. | PASS |

The source reviewers found no remaining production defect. Contract review rejected a new fixed-example forwarding test; the parent removed it and the touched registry-only nonempty test rather than pinning copied output. Meaningful topology, pointer, error, identity, and geometry regressions remain. Final Rust tests, formatting, and clippy passed after those test-only removals; production code and the reviewed runtime binary were unchanged.

Runtime evidence from the three family testers jointly covers every DS-05 native consumer; no family-level PASS was treated as global coverage by itself. Graph identity probes independently decoded source/neighbor cells, preserved duplicate edges/self-loops, and rejected duplicate/coerced identifiers. Fresh extension probes covered all eight JSON kinds, native DSL, both direction values, BOM/leading whitespace, explicit headers, command output, and real result frames.

All runtime tracks verified the pre-version-bump candidate SHA-256 `ba898fc7fa430ab02aaf3d3eb627da41165a8887742d89f55ff9a12afefde495`; its repository and release-build binaries matched. Exact session-local captures accompany each track report; the fixtures above and permanent behavior tests retain reproducible acceptance in the repository.

## Intentional limits

No heapification, strict B-tree balancing, arbitrary pointer graphs, native deque, queue/heap/graph shorthand, variable-height native cells, terminal-width clamp, or Unicode transliteration was added. Side-faithful binary chains grow linearly in width and may wrap in narrow terminals. Existing resource guards remain. Fresh component-level extension verification is not a live user-session reload claim.

Historical release records follow unchanged.

---

# Release v0.12.1 — Firmware Renderer, CLI, and Extension Polish

**Status:** Verified v0.12.1 patch release. Rust/npm metadata, documentation, and the runtime version agree.
**Release references:** `master` and annotated tag `v0.12.1`. Integration branch: `sprint/firmware-polish-20261009`; implementation checkpoint `c01b7fe`; upstream base `93fcae5` (v0.12.0).

## Release summary

- **Layout and labels:** recursive group packing/padding, title detours, decision ingress, and LR separation; compact title shelves; deferred multiline flowchart labels bounded to their route corridors; architecture frame widths, column-label width reservation, sibling clearance, label margins, and joined source turns.
- **Color:** unstyled redraws preserve stroke ownership/color, explicit colors win at shared wire cells, node borders retain their own color, and architecture property dividers receive complete component-border color.
- **CLI and extension:** trailing options after bare input, LF-terminated markdown fences, JSON-safe default coloring, explicit `color:false` in freshly loaded extensions, command help, and ANSI/CJK-aware result framing.
- **Verification:** final release checks are recorded below; prior implementation-stage results remain separately identified.
- **Compactness and limits:** the original flight-control diagram fell from 54 × 181 to 54 × 158 rows × columns; architecture stayed 28 × 83. No terminal-width cap, truncation, or automatic fit-to-terminal wrapping was added.
- **Session limitation:** the already-loaded live wrapper ignored `color:false`; the freshly loaded installed extension and CLI `--color never` honored suppression. A session reload remains unverified. Renderer changes do not establish that the stale wrapper was refreshed.

For the source update/build, installed Pi synchronization, separate omp installation, and session reload steps, see [README.md](README.md#3-update-an-existing-installation).

## Final release checks (2026-10-09)

- All five Rust/npm version entries agree on `0.12.1`; the release binary reports `ascii-diagram 0.12.1`.
- `cargo test --locked`: **322 tests passed**, including the targeted column-label regression. `cargo fmt --check`, `cargo clippy --all-targets --locked`, and `cargo build --release --locked` passed. Clippy reports only the three existing warnings: `int_plus_one`, `module_inception`, and `too_many_arguments`.
- The extension suite passed against the rebuilt release binary. A separate fresh-extension smoke exercised both original firmware diagrams in colored/plain modes, real `Text` result frames, command/help, Mermaid transformation, and JSON input containing `graph`/`flowchart` text.
- Both original firmware specifications passed all five styles: full title/node/property/edge-label retention, blank label margins, ANSI/plain parity, 7-bit ASCII, and LF-terminated markdown fences. All **70** input-path renders were byte-identical; `Target Angle` remained between its source and target.
- All **35** built-in example/style outputs matched the installed visual-polish baseline. Raw CLI dimensions remain **54 × 158** for the flight-control diagram and **28 × 83** for memory/bus architecture.
- Live colored `draw_diagram` output for both original specifications exactly matched the verified native renderer. Fresh-extension plain output passed; a reload of the already-running user session remains unverified.
- `npm pack --dry-run --ignore-scripts --json` included the required sources, documentation, extension, and skills in **33 files**, excluding build/dependency/index caches. This checks package contents; no npm registry publication is claimed.
- All **six** paired README input/rendered-output showcases matched the release CLI. The tree showcase's stale sharp final corner was corrected to the actual rounded-style output.
- Independent release review found a new narrow-column label-loss regression. The actual three-container probe dropped `Internal link` in all five styles before repair. Column measurement now reserves display-width-aware space for the label, route, blank margins, and frame. The regression and **30** original/nested/CJK/long-label style probes pass, retaining each label exactly once with endpoint locality and ANSI/plain parity. The full gates, 70 original firmware input paths, and 35 example/style comparisons were rerun after this repair.
- Final repository/user/Pi/cache/omp binaries share SHA-256 `cbea0c62d304fcd2cbac8716e55964dda549f6fc26ab2f85f77faef375ceb8a9`. The installed live `draw_diagram` also retains the previously missing `Internal link` and exactly matches the corrected release binary.

## Historical implementation stages

The sections below preserve the validation counts, commits, local binary hashes, and synchronization observations from each implementation stage. Their “no upstream push” statements describe those stages, not a separate current publication status. Session-only coordination artifacts are not required release references.

### Initial firmware review fixes

**Stage status:** Implementation and integrated verification completed before the color and visual-polish follow-ups.
**Stage integration branch:** `sprint/v0.12.1`; base `93fcae5`.
**Coordination:** four GPT implementation workers in isolated worktrees; Scrum Master maintained a session-local deduplicated ledger. Parent owned merges, stage gates, runtime acceptance and local synchronization. No upstream push at this stage.

### Initial fix ledger

| Owner | Review IDs | Implemented behavior | Commits |
|---|---|---|---|
| FlowDev | ALIGN-01/STRESS-01, ALIGN-03, ALIGN-04, ROUTE-01/STRESS-02 | Recursive whole-group packing, member/nesting padding, odd-width decision ingress, intact titles with connected crossing detours; LR groups pack on Y without changing X/ranks. | `81d8d31`, `cc139d27`, `a116d54` |
| ArchDev + parent | ALIGN-02, ALIGN-05/ROUTE-02 | Common top-level frame width without widening nested containers; sibling corridor clearance; route-first label placement retains all three firmware connection labels; legal terminal bends and joined source turns. | `9f802df`, `79ec81e`, `c63c604` |
| CliDev | MODES-01, MODES-02/CLI-OMP-03 | Closing markdown fence ends in LF; options may follow bare files/quoted DSL; literal `--` and argument-error exits preserved. Color help now describes the intentional explicit-style auto behavior. | `ea07415` |
| ExtDev | CLI-OMP-01, CLI-OMP-04, CLI-OMP-05 | Header-anchored Mermaid auto-color guard preserves JSON; tool `color:false` sends `--color never`; local command help; ANSI/CJK-aware result framing. | `31db238` |

Runtime verification caught two follow-ups beyond the initially green suite: overlapping README LR groups and an unjoined narrow architecture source turn. Both are fixed and covered by actual render checks. The incidental moved-child ordering assertion was removed, retaining the original orientation/non-origin geometry invariants (`6f531d5`). The obsolete production `find_subgraph` helper was removed after LR cutover; the remaining test locates its top-level fixture groups directly.

### Intentional behavior

- CLI-OMP-02: explicit diagram colors activate CLI auto color even when piped or with `NO_COLOR`; unstyled auto honors `NO_COLOR`. Explicit `never` suppresses styles; `always` forces color. No precedence reversal.
- COLOR-01: extended names such as `orange` legitimately use truecolor.
- MODES-03: four-space-indented backticks cannot close a CommonMark fence.
- ALIGN-06: sibling node widths remain content-sized; only outer architecture frames are normalized.

### Initial observed verification

- `cargo test`: **308 passed** (285 library, 23 CLI); targeted source-connectivity and group-containment regressions also passed.
- `cargo clippy --all-targets`: passed with only the three existing warnings (`int_plus_one`, `module_inception`, `too_many_arguments`). `cargo fmt --check`: passed. Release build succeeded.
- Extension behavior suite passed with `ASCII_DIAGRAM_BIN` pointing to the current integrated release, not stale `bin/`.
- Firmware Spec A/B × five styles: complete group titles and all three architecture connection labels retained; colored output is byte-identical after stripping SGR; ASCII variants are 7-bit. Final ASCII sizes: **A 54 rows × 181 columns**, **B 28 × 83**.
- Seven input paths × two specs × five styles: **70 byte-identical runs**. Ten markdown bodies match plain output and close with LF; trailing flags, literal `--`, exit-2 option errors and intended color precedence exercised.
- All **35** built-in example/style renders succeeded; **25** unaffected renderer/style outputs stayed byte-identical to the local v0.12.0 binary.
- Exact README LR input and four-/eight-motor fan-outs exercised in all five styles; narrow column/nested-column/row routes preserve neighboring properties and labels.
- Real installed `pi-tui` Markdown/Text components exercised tool JSON, explicit color suppression, command, transformer and assistant-message output. CJK/ANSI frame geometry and color fidelity passed. This is component-level runtime evidence, **not a live user-session reload claim**.

README flowchart/architecture examples were regenerated from the release CLI; CLI/color/width contracts and bundled skills updated. Wide firmware views may still wrap in narrow terminals; split them into focused diagrams. No automatic fit-to-terminal feature was added.

**Initial release synchronization:** repository, `~/.local/bin`, Pi binary/cache and omp plugin binary copies shared SHA-256 `bfdaf3d3bf650694ae55bbe01bbb248044a1f6f8c3fba97d005d858cf710efbc`. Repository/user/Pi/omp engines reported v0.12.1 and rendered actual firmware Spec B identically. Installed omp extension also passed the real-component smoke using native binary discovery with no `ASCII_DIAGRAM_BIN` override. No upstream push was performed.

### Color fidelity follow-up

- Fixed partial stroke coloring reported after the firmware review: unstyled connector/arrow redraws preserve existing color, explicit colors take precedence at shared wire cells, and node borders retain their own color.
- Architecture property dividers now use the component border color; flowchart title bypasses inherit the crossing edge's color rather than the group color. No geometry or CLI color-precedence changes.
- Four behavior regressions failed before the fix and pass afterward. Full Rust suite: **312 passed**. Clippy retains the three existing warnings; formatting and release build pass.
- Actual firmware A/B renders pass in all five styles: plain geometry unchanged, SGR-stripped colored/plain parity, and 7-bit ASCII. All **35** example/style geometries remain unchanged. All **20** firmware component/style property dividers have complete stroke color; their labels remain terminal-default.
- Extension behavior suite passed after installation synchronization. Live `draw_diagram` output for both original firmware specs exactly matches the corrected rounded, colored native renders.
- Corrected repository/user/Pi/omp binary copies share SHA-256 `ac2099548e77470bebf4ba42bc9fd0537ddce044a35f05b7060dadb314d5bfe7`. This follow-up is local; no upstream push was performed.

### Parallel visual polish follow-up

**Stage branch:** `sprint/firmware-polish-20261009`; color-preserving baseline `299df10`. Three implementation agents worked in isolated worktrees; the parent maintained a session-local coordination board, integrated commits, and ran this stage's verification.

| Track | Owner | Change | Commits |
|---|---|---|---|
| Architecture labels | ArchPolish | Bounded label search reserves one blank display cell at each end without moving components or routes | `525aa4c` |
| Flowchart labels | FlowLabels | Defer labels until routes/nodes/groups are drawn; preserve complete multiline blocks, stroke clearance, and source/target corridor locality | `b366572`, `32a1e69` |
| Group compactness | CompactGroups | Extend title shelves only to their required end; choose the earliest blocking anchor deterministically without allocating a sorted copy | `6d56efc` |

The first render pass caught a locality regression despite green tests: `Target Angle` moved above its source, and a skip-edge label moved near an unrelated arrow. Corridor bounds now keep those labels with their own connections; new endpoint-locality regressions and actual renders pass.

| Unchanged firmware input | Before | After |
|---|---|---|
| Flight-control flowchart | 54 rows × 181 columns | 54 rows × 158 columns |
| Memory/bus architecture | 28 rows × 83 columns | 28 rows × 83 columns |

- **321 Rust tests passed**; current-release extension suite passed; formatting passed. Clippy retains only the three existing warnings.
- Original A/B × five styles preserve all labels/titles/node text, blank connection-label margins, and ANSI/plain parity. All 20 component/style property dividers retain complete border color.
- Crowded TB/LR, multiline, CJK/nested, self-loop, and four/eight-motor scenarios pass in all five styles. Twenty architecture column/nested/row/CJK renders retain components, properties, and complete labels.
- All 35 built-in example/style outputs remain byte-identical to the color-fix baseline. Five independent runs per style produce identical original-firmware flowchart output.
- README's LR showcase was regenerated from the release CLI. The complete firmware flowchart still needs 158 columns; no truncation, terminal-width cap, or automatic wrapping was introduced. No upstream push.
- Installed repository/user/Pi binary/cache and omp copies share SHA-256 `4d87f284d5010b7688b1a48450ccbceb3f7f189e3b79da775017faf48151d5e9`; changed package sources and documentation are synchronized. Live colored `draw_diagram` renders for both original specs exactly match the corrected native renderer.
- Session caveat: the live tool still emits ANSI for styled input with `color:false`, while a freshly loaded installed omp extension honors suppression with native binary discovery. Reported through automated tool QA. Reload the session to refresh its tool wrapper; that reload has not been verified. CLI `--color never` remains verified in all five styles.

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

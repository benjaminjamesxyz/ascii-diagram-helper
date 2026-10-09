use crate::canvas::{Canvas, CellRole, Direction, Rect, display_width};
use crate::color::Color;
use crate::schema::{EdgeSpec, FlowchartSpec, LayoutDirection, NodeShape, NodeSpec, SubgraphSpec};
use crate::theme::{BoxStyle, Theme};
use std::collections::{HashMap, HashSet, VecDeque};
use unicode_width::UnicodeWidthStr;

mod edges;
mod lr;
mod tb;
#[cfg(test)]
mod tests;

use edges::{dfs_find_cycles, edge_arrow_heads, edge_hline, edge_vline};

pub(super) struct LayoutNode {
    label_lines: Vec<String>,
    shape: NodeShape,
    /// Propagated from `NodeSpec::dashed_border` (Mermaid `stroke-dasharray`)
    dashed_border: bool,
    /// Propagated from `NodeSpec::color` (Mermaid `stroke:<name|hex>`)
    color: Option<Color>,
    /// Propagated from `NodeSpec::fill_color` (Mermaid `fill:<name|hex>`) —
    /// label text color
    fill_color: Option<Color>,
    /// Propagated from `NodeSpec::border_level` (`stroke-width` weight)
    border_level: u8,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rank: usize,
}

pub struct FlowchartRenderer<'a> {
    spec: &'a FlowchartSpec,
    theme: Theme,
}

/// Edge labels are painted only after every route, node, and group border.
struct PendingLabel {
    lines: Vec<String>,
    x: usize,
    y: usize,
    centered: bool,
    up: bool,
    band: Option<Rect>,
}

impl PendingLabel {
    fn left(lines: Vec<String>, x: usize, y: usize) -> Self {
        Self {
            lines,
            x,
            y,
            centered: false,
            up: false,
            band: None,
        }
    }

    fn centered(lines: Vec<String>, x: usize, y: usize, up: bool) -> Self {
        Self {
            lines,
            x,
            y,
            centered: true,
            up,
            band: None,
        }
    }

    fn within(mut self, band: Rect) -> Self {
        self.band = Some(band);
        self
    }

    fn draw(self, renderer: &FlowchartRenderer<'_>, canvas: &mut Canvas) {
        if self.centered {
            renderer.draw_stacked_label(
                canvas, &self.lines, self.x, self.y, self.up, self.band,
            );
        } else {
            FlowchartRenderer::draw_stacked_label_left(
                canvas, &self.lines, self.x, self.y, true, self.band,
            );
        }
    }
}

/// A subgraph with its own `direction` that is edge-isolated from the rest of
/// the diagram: rendered independently in its own orientation and pasted as a
/// pre-composed block. Mermaid applies subgraph `direction` under the same
/// restriction (disconnected clusters only).
/// How a rendered direction-cluster block is placed on the main canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BlockMode {
    /// Edge-isolated cluster: pasted below the main graph
    Below,
    /// Non-isolated cluster: replaces its members as a phantom node in the
    /// layout; block pasted at the phantom's position
    AtPhantom,
}

#[derive(Clone)]
pub(super) struct IsolatedBlock {
    /// Subgraph id this block was rendered from
    sg_id: String,
    /// Placement mode
    mode: BlockMode,
    /// For `AtPhantom`: id of the phantom node standing in for this cluster
    phantom_id: String,
    /// All recursive member ids of the cluster
    member_ids: Vec<String>,
    /// Owned-spec index of the phantom node (AtPhantom, assigned on reindex)
    phantom_idx: Option<usize>,
    /// Rendered content lines (no trailing blanks)
    lines: Vec<String>,
    width: usize,
    height: usize,
    /// Paste origin on the main canvas, assigned during layout
    origin_x: usize,
    origin_y: usize,
}

/// Everything `render_tb` / `render_lr` need to lay out a diagram containing
/// isolated-direction subgraph blocks.
pub(super) struct Blocks {
    items: Vec<IsolatedBlock>,
    /// Indices into `spec.nodes` of ALL (recursive) members of BELOW-mode
    /// moved subgraphs (phantom members are removed from the spec instead)
    member_indices: HashSet<usize>,
    /// Member ids of below-mode subgraphs — used to recompute
    /// `member_indices` when the spec is rewritten for phantom clusters
    pub(super) isolated_member_ids: Vec<String>,
    /// Owned-spec indices of phantom nodes (skip drawing; they carry the
    /// pasted cluster block instead)
    pub(super) phantom_indices: HashSet<usize>,
}

/// Prefix of the phantom node standing in for a supernode cluster.
pub(super) const PHANTOM_PREFIX: &str = "__sg_";

/// Subgraph group rectangles (layout-locked once nodes are positioned) plus
/// the set of their perimeter cells. Shared by the containment push
/// (FC-SUB-01), border-row dodging (FC-SUB-04) and edge routing.
pub(super) struct GroupGeo {
    /// `(rect, subgraph id)` for every drawn group box
    pub rects: Vec<(Rect, String)>,
    /// Perimeter cells of every rect
    #[allow(dead_code)]
    pub cells: HashSet<(usize, usize)>,
}

impl GroupGeo {
    #[allow(dead_code)]
    pub(super) fn border_row_blocked(&self, y: usize, x1: usize, x2: usize) -> bool {
        let (lo, hi) = (x1.min(x2), x1.max(x2));
        self.cells
            .iter()
            .any(|&(cx, cy)| cy == y && cx >= lo && cx <= hi)
    }
}

impl Blocks {
    /// No moved clusters — every node participates in the main layout.
    #[allow(dead_code)]
    pub(super) fn empty() -> Blocks {
        Blocks {
            items: Vec::new(),
            member_indices: HashSet::new(),
            isolated_member_ids: Vec::new(),
            phantom_indices: HashSet::new(),
        }
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn has_supernodes(&self) -> bool {
        self.items.iter().any(|b| b.mode == BlockMode::AtPhantom)
    }

    fn rect_for(&self, sg_id: &str) -> Option<(usize, usize, usize, usize)> {
        // Assigned during layout; stored alongside by render_tb/render_lr
        self.items
            .iter()
            .find(|b| b.sg_id == sg_id)
            .map(|b| (b.origin_x, b.origin_y, b.width, b.height))
    }

    /// Recomputes index-based fields against a rewritten (phantom-bearing)
    /// spec: below-mode members keep their ids; phantoms are new nodes.
    fn reindex_for(&self, renderer: &FlowchartRenderer<'_>) -> Blocks {
        let idx = renderer.index_of();
        let mut member_indices = HashSet::new();
        for id in &self.isolated_member_ids {
            if let Some(&i) = idx.get(id.as_str()) {
                member_indices.insert(i);
            }
        }
        let mut phantom_indices = HashSet::new();
        for b in &self.items {
            if b.mode == BlockMode::AtPhantom
                && let Some(&i) = idx.get(b.phantom_id.as_str())
            {
                phantom_indices.insert(i);
            }
        }
        Blocks {
            items: self
                .items
                .iter()
                .map(|b| {
                    let mut b = b.clone();
                    if b.mode == BlockMode::AtPhantom
                        && let Some(&i) = idx.get(b.phantom_id.as_str())
                    {
                        b.phantom_idx = Some(i);
                    }
                    b
                })
                .collect(),
            member_indices,
            isolated_member_ids: self.isolated_member_ids.clone(),
            phantom_indices,
        }
    }
}

/// Recursive member ids of a subgraph, including nested subgraph members.
/// The parser assigns each node to exactly one subgraph, so ids are unique.
fn member_ids(sg: &SubgraphSpec) -> Vec<String> {
    let mut out = sg.nodes.clone();
    for child in &sg.subgraphs {
        out.extend(member_ids(child));
    }
    out
}

impl<'a> FlowchartRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a FlowchartSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[must_use]
    pub fn render(&self, colored: bool) -> String {
        if self.spec.nodes.is_empty() {
            return String::new();
        }

        let blocks = self.collect_blocks(colored);
        if blocks.has_supernodes() {
            // Non-isolated direction clusters: members collapse into phantom
            // nodes; render against the rewritten spec
            let owned = self.supernode_spec(&blocks);
            let sub = FlowchartRenderer::new(&owned, self.theme.clone());
            let blocks = blocks.reindex_for(&sub);
            sub.render_dispatch(colored, blocks)
        } else {
            self.render_dispatch(colored, blocks)
        }
    }

    fn render_dispatch(&self, colored: bool, mut blocks: Blocks) -> String {
        let is_lr = matches!(
            self.spec.direction,
            LayoutDirection::LR | LayoutDirection::RL
        );
        if is_lr {
            self.render_lr(colored, &mut blocks)
        } else {
            self.render_tb(colored, &mut blocks)
        }
    }

    /// Rewrites the spec for supernode clusters: below-mode members stay,
    /// each AtPhantom cluster's members are replaced by a single phantom node,
    /// internal cluster edges are dropped, external edge endpoints are
    /// re-targeted to the phantom id.
    fn supernode_spec(&self, blocks: &Blocks) -> FlowchartSpec {
        let mut in_supernode: HashSet<&str> = HashSet::new();
        let mut phantom_of: HashMap<&str, String> = HashMap::new();
        for b in &blocks.items {
            if b.mode == BlockMode::AtPhantom {
                for id in &b.member_ids {
                    in_supernode.insert(id.as_str());
                    phantom_of.insert(id.as_str(), format!("{PHANTOM_PREFIX}{}", b.sg_id));
                }
            }
        }
        let mut nodes: Vec<NodeSpec> = Vec::new();
        for n in &self.spec.nodes {
            if let Some(pid) = phantom_of.get(n.id.as_str()) {
                // Emit one phantom per cluster (first member seen)
                if !nodes.iter().any(|m| &m.id == pid) {
                    nodes.push(NodeSpec {
                        id: pid.clone(),
                        label: String::new(),
                        shape: NodeShape::Box,
                        dashed_border: false,
                        color: None,
                        fill_color: None,
                        border_level: 0,
                        lines: Vec::new(),
                    });
                }
            } else {
                nodes.push(n.clone());
            }
        }
        let edges = self
            .spec
            .edges
            .iter()
            .filter(|e| {
                // drop internal cluster edges
                !(in_supernode.contains(e.from.as_str())
                    && in_supernode.contains(e.to.as_str())
                    && phantom_of.get(e.from.as_str()) == phantom_of.get(e.to.as_str()))
            })
            .map(|e| {
                let mut e = e.clone();
                if let Some(pid) = phantom_of.get(e.from.as_str()) {
                    e.from = pid.clone();
                }
                if let Some(pid) = phantom_of.get(e.to.as_str()) {
                    e.to = pid.clone();
                }
                e
            })
            .collect();
        FlowchartSpec {
            direction: self.spec.direction,
            style: self.spec.style,
            title: self.spec.title.clone(),
            nodes,
            edges,
            subgraphs: self.spec.subgraphs.clone(),
        }
    }

    /// Finds subgraphs with an explicit `direction` that differs from the
    /// global direction and renders each recursively as a standalone block.
    /// Edge-isolated clusters paste below the graph (`Below`); non-isolated
    /// clusters become phantom-backed supernodes (`AtPhantom`). Nested
    /// subgraphs of a moved subgraph are handled inside the recursive render,
    /// not visited again.
    fn collect_blocks(&self, colored: bool) -> Blocks {
        let idx = self.index_of();
        let mut items = Vec::new();
        let mut member_indices = HashSet::new();
        let mut isolated_member_ids = Vec::new();
        self.collect_sgs(
            &self.spec.subgraphs,
            &idx,
            colored,
            &mut items,
            &mut member_indices,
            &mut isolated_member_ids,
        );
        Blocks {
            items,
            member_indices,
            isolated_member_ids,
            phantom_indices: HashSet::new(),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "internal collector; threading state through recursion reads clearest"
    )]
    fn collect_sgs(
        &self,
        sgs: &[SubgraphSpec],
        idx: &HashMap<&str, usize>,
        colored: bool,
        out: &mut Vec<IsolatedBlock>,
        member_indices: &mut HashSet<usize>,
        isolated_member_ids: &mut Vec<String>,
    ) {
        for sg in sgs {
            let wants_move = sg.direction.is_some_and(|d| d != self.spec.direction);
            let mut moved = false;
            if wants_move {
                let members = member_ids(sg);
                // Both modes render the cluster recursively; isolation only
                // picks the placement strategy
                let mode = if Self::is_isolated(self.spec, &members) {
                    BlockMode::Below
                } else {
                    BlockMode::AtPhantom
                };
                {
                    let idset: HashSet<&str> = members.iter().map(String::as_str).collect();
                    let sub = FlowchartSpec {
                        direction: sg.direction.unwrap(),
                        style: self.spec.style,
                        title: None,
                        nodes: self
                            .spec
                            .nodes
                            .iter()
                            .filter(|n| idset.contains(n.id.as_str()))
                            .cloned()
                            .collect(),
                        edges: self
                            .spec
                            .edges
                            .iter()
                            .filter(|e| {
                                idset.contains(e.from.as_str()) && idset.contains(e.to.as_str())
                            })
                            .cloned()
                            .collect(),
                        subgraphs: sg.subgraphs.clone(),
                    };
                    let rendered = FlowchartRenderer::new(&sub, self.theme.clone()).render(colored);
                    let mut lines: Vec<String> = rendered.lines().map(String::from).collect();
                    while lines.last().is_some_and(|l| l.trim().is_empty()) {
                        lines.pop();
                    }
                    let height = lines.len().max(1);
                    let width = lines
                        .iter()
                        .map(|l| UnicodeWidthStr::width(l.as_str()))
                        .max()
                        .unwrap_or(1);
                    if std::env::var("DBG_SG").is_ok() {
                        let dir_dbg = sub.direction;
                        eprintln!(
                            "BLOCK sg={} mode={:?} sub_dir={:?} {}x{} members={:?} nodes_in_sub={}",
                            sg.id,
                            mode,
                            dir_dbg,
                            width,
                            height,
                            members,
                            sub.nodes.len()
                        );
                        eprintln!("{rendered}");
                    }
                    if mode == BlockMode::Below {
                        for id in &members {
                            if let Some(&i) = idx.get(id.as_str()) {
                                member_indices.insert(i);
                            }
                            isolated_member_ids.push(id.clone());
                        }
                    }
                    out.push(IsolatedBlock {
                        sg_id: sg.id.clone(),
                        mode,
                        phantom_id: format!("{PHANTOM_PREFIX}{}", sg.id),
                        member_ids: members,
                        phantom_idx: None,
                        lines,
                        width,
                        height,
                        origin_x: 0,
                        origin_y: 0,
                    });
                    moved = true;
                }
            }
            if !moved {
                // Children of a moved subgraph are handled by the recursive
                // render; only unmoved subgraphs can still host moved children
                self.collect_sgs(
                    &sg.subgraphs,
                    idx,
                    colored,
                    out,
                    member_indices,
                    isolated_member_ids,
                );
            }
        }
    }

    /// True when no edge connects a member of `members` to a node outside it.
    fn is_isolated(spec: &FlowchartSpec, members: &[String]) -> bool {
        let set: HashSet<&str> = members.iter().map(String::as_str).collect();
        spec.edges.iter().all(|e| {
            let f = set.contains(e.from.as_str());
            let t = set.contains(e.to.as_str());
            f == t
        })
    }

    fn prepare_nodes(&self, blocks: &Blocks) -> Vec<LayoutNode> {
        let mut nodes = Vec::with_capacity(self.spec.nodes.len());

        for node in &self.spec.nodes {
            // Multi-line labels: parser-filled `lines` when present (split on
            // `\n` / `<br/>` / physical newlines), else fall back to splitting
            // `label` itself on `\n`
            let label = if node.label.is_empty() && node.lines.is_empty() {
                Some(node.id.clone())
            } else {
                None
            };
            let source_lines: Vec<String> = if !node.lines.is_empty() {
                node.lines.clone()
            } else {
                let base = label.unwrap_or_else(|| node.label.clone());
                base.split('\n').map(str::to_string).collect()
            };

            // Wrap text nicely if long
            let mut lines: Vec<String> = Vec::new();
            for part in &source_lines {
                let trimmed_part = part.trim();
                if trimmed_part.len() > 36 {
                    for wrapped in textwrap::wrap(trimmed_part, 32) {
                        lines.push(wrapped.into_owned());
                    }
                } else {
                    lines.push(trimmed_part.to_string());
                }
            }

            let max_text_w = lines
                .iter()
                .map(|l| UnicodeWidthStr::width(l.as_str()))
                .max()
                .unwrap_or(4);

            let (width, height) = match node.shape {
                NodeShape::Diamond => {
                    // Custom continuous decision block:
                    // Solid continuous box-drawing borders with an embedded diamond badge.
                    // Never breaks across terminal fonts or character aspect ratios.
                    let w = (max_text_w + 6).max(10);
                    // An odd width puts the padded diamond badge and the
                    // incoming/outgoing anchor on the same display column.
                    let w = w | 1;
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Database => {
                    // Cylinder silhouette: paren side walls plus one cell of
                    // padding on each side, and a wall row above and below
                    // the label under the curved caps
                    let w = (max_text_w + 6).max(10);
                    let h = lines.len() + 4;
                    (w, h)
                }
                NodeShape::Subprocess => {
                    // Subprocess has double vertical side borders
                    let w = (max_text_w + 6).max(10);
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Circle => {
                    // Circular node / summing junction with circular badge
                    let w = (max_text_w + 6).max(8);
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Stadium => {
                    let w = (max_text_w + 6).max(8);
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Hexagon | NodeShape::DoubleCircle => {
                    // Badge glyph embedded in the top border needs extra width
                    let w = (max_text_w + 6).max(8);
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Parallelogram | NodeShape::ParallelogramAlt => {
                    let w = (max_text_w + 6).max(8);
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Trapezoid | NodeShape::TrapezoidAlt => {
                    // 4-char badge `/__\` in the top border
                    let w = (max_text_w + 8).max(10);
                    let h = lines.len() + 2;
                    (w, h)
                }
                _ => {
                    let w = (max_text_w + 4).max(6);
                    // Restore padding parity after the min-width bump: an even
                    // box around odd text can never center (`| A  |`); widening
                    // to odd width keeps the border padding symmetric (FC-TB-05)
                    let w = w + (w - max_text_w) % 2;
                    let h = lines.len() + 2;
                    (w, h)
                }
            };

            nodes.push(LayoutNode {
                label_lines: lines,
                shape: node.shape,
                dashed_border: node.dashed_border,
                color: node.color,
                fill_color: node.fill_color,
                border_level: node.border_level,
                width,
                height,
                x: 0,
                y: 0,
                rank: 0,
            });
        }

        // Phantom cluster nodes carry the pasted block's real dimensions
        // (their label is empty, so text-based sizing would be wrong)
        for (i, spec_node) in self.spec.nodes.iter().enumerate() {
            if let Some(b) = blocks
                .items
                .iter()
                .find(|b| b.mode == BlockMode::AtPhantom && b.phantom_id == spec_node.id)
            {
                nodes[i].width = b.width.max(6);
                nodes[i].height = b.height.max(2);
            }
        }

        nodes
    }

    /// Node id -> index into the spec's node list. Built once, borrowed, never cloned.
    fn index_of(&self) -> HashMap<&str, usize> {
        self.spec
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect()
    }

    /// Dense fan-outs need a contiguous title shelf clear of member anchors.
    fn group_title_right(
        sg: &SubgraphSpec,
        left: usize,
        right: usize,
        nodes: &[LayoutNode],
        idx: &HashMap<&str, usize>,
    ) -> usize {
        let label_width = UnicodeWidthStr::width(sg.title.as_deref().unwrap_or(&sg.id)) + 2;
        let right = right.max(left + label_width + 3);
        let mut x = left + 2;
        while x + label_width <= right {
            let blocker = idx.iter().find_map(|(&id, &i)| {
                let cx = nodes[i].x + nodes[i].width / 2;
                (Self::subgraph_contains_node(sg, id) && cx >= x && cx < x + label_width)
                    .then_some(cx)
            });
            let Some(cx) = blocker else {
                return right;
            };
            x = cx + 1;
        }
        right + label_width + 2
    }

    /// Group boxes as `(rect, subgraph id, title, color)` in declaration
    /// order — shared by `draw_subgraphs` and the layout-time geometry pass.
    pub(super) fn collect_group_rects(
        &self,
        nodes: &[LayoutNode],
        idx: &HashMap<&str, usize>,
        blocks: &Blocks,
    ) -> Vec<(Rect, String, String, Option<Color>)> {
        fn group_rect(
            sg: &SubgraphSpec,
            nodes: &[LayoutNode],
            idx: &HashMap<&str, usize>,
            pad: (usize, usize, usize),
            blocks: &Blocks,
        ) -> Option<Rect> {
            let (pad_x, pad_top, pad_bottom) = pad;
            let mut x0 = usize::MAX;
            let mut y0 = usize::MAX;
            let mut x1 = 0usize;
            let mut y1 = 0usize;
            for id in &sg.nodes {
                if let Some(&i) = idx.get(id.as_str()) {
                    let n = &nodes[i];
                    x0 = x0.min(n.x);
                    y0 = y0.min(n.y);
                    x1 = x1.max(n.x + n.width - 1);
                    y1 = y1.max(n.y + n.height - 1);
                }
            }
            for child in &sg.subgraphs {
                // Moved children render as pasted blocks elsewhere; their
                // member coords are phantom and would inflate this box
                if blocks.rect_for(&child.id).is_some() {
                    continue;
                }
                if let Some(r) = group_rect(child, nodes, idx, pad, blocks) {
                    x0 = x0.min(r.x);
                    y0 = y0.min(r.y);
                    x1 = x1.max(r.x + r.width - 1);
                    y1 = y1.max(r.y + r.height - 1);
                }
            }
            if x0 == usize::MAX {
                return None;
            }
            let bx = x0.saturating_sub(pad_x);
            let by = y0.saturating_sub(pad_top);
            let mut br = x1.saturating_add(pad_x);
            let bb = y1.saturating_add(pad_bottom);
            br = FlowchartRenderer::group_title_right(sg, bx, br, nodes, idx);
            Some(Rect::new(bx, by, br - bx + 1, bb - by + 1))
        }

        fn collect(
            sgs: &[SubgraphSpec],
            nodes: &[LayoutNode],
            idx: &HashMap<&str, usize>,
            pad: (usize, usize, usize),
            blocks: &Blocks,
            out: &mut Vec<(Rect, String, String, Option<Color>)>,
        ) {
            for sg in sgs {
                if let Some((ox, oy, w, h)) = blocks.rect_for(&sg.id) {
                    // Moved subgraph: box wraps the pasted block; nested boxes
                    // are already baked into the block's own render
                    let (pad_x, pad_top, pad_bottom) = pad;
                    let title = sg.title.clone().unwrap_or_else(|| sg.id.clone());
                    let bx = ox.saturating_sub(pad_x);
                    let by = oy.saturating_sub(pad_top);
                    let mut br = ox + w + pad_x;
                    let bb = oy + h + pad_bottom;
                    br = FlowchartRenderer::group_title_right(sg, bx, br, nodes, idx);
                    out.push((
                        Rect::new(bx, by, br - bx + 1, bb - by + 1),
                        sg.id.clone(),
                        title,
                        sg.color,
                    ));
                    continue;
                }
                if let Some(r) = group_rect(sg, nodes, idx, pad, blocks) {
                    out.push((
                        r,
                        sg.id.clone(),
                        sg.title.clone().unwrap_or_else(|| sg.id.clone()),
                        sg.color,
                    ));
                }
                collect(&sg.subgraphs, nodes, idx, pad, blocks, out);
            }
        }

        let mut groups = Vec::new();
        collect(
            &self.spec.subgraphs,
            nodes,
            idx,
            (2usize, 2usize, 1usize),
            blocks,
            &mut groups,
        );
        groups
    }

    /// Layout-time geometry of all group boxes: rects plus perimeter cells.
    pub(super) fn compute_group_geo(
        &self,
        nodes: &[LayoutNode],
        idx: &HashMap<&str, usize>,
        blocks: &Blocks,
    ) -> GroupGeo {
        let rects_src = self.collect_group_rects(nodes, idx, blocks);
        let mut cells = HashSet::new();
        for (r, _, _, _) in &rects_src {
            let right = r.x + r.width - 1;
            let bottom = r.y + r.height - 1;
            for x in r.x..=right {
                cells.insert((x, r.y));
                cells.insert((x, bottom));
            }
            for y in r.y..=bottom {
                cells.insert((r.x, y));
                cells.insert((right, y));
            }
        }
        GroupGeo {
            rects: rects_src.into_iter().map(|(r, id, _, _)| (r, id)).collect(),
            cells,
        }
    }

    /// True when `node_id` belongs to `sg` or any nested (non-moved) child —
    /// such nodes legitimately sit inside `sg`'s group box.
    fn subgraph_contains_node(sg: &SubgraphSpec, node_id: &str) -> bool {
        fn rec(sg: &SubgraphSpec, node_id: &str) -> bool {
            sg.nodes.iter().any(|n| n == node_id) || sg.subgraphs.iter().any(|c| rec(c, node_id))
        }
        if let Some(cluster) = node_id.strip_prefix(PHANTOM_PREFIX) {
            // A phantom stands for a whole moved cluster: inside `sg` iff the
            // cluster is `sg` itself or a descendant of it
            if sg.id == cluster {
                return true;
            }
            fn rec_id(sg: &SubgraphSpec, id: &str) -> bool {
                sg.id == id || sg.subgraphs.iter().any(|c| rec_id(c, id))
            }
            return rec_id(sg, cluster);
        }
        rec(sg, node_id)
    }

    /// LR containment entry: preserve whole-group geometry across ranks,
    /// rather than pushing only whichever member intersects a stale rect.
    pub(super) fn push_nodes_out_of_groups(
        &self,
        nodes: &mut [LayoutNode],
        layers: &[Vec<usize>],
        geo: &GroupGeo,
        horizontal: bool,
        gap: usize,
    ) {
        if geo.rects.is_empty() {
            return;
        }
        let idx = self.index_of();
        let mut active = vec![false; nodes.len()];
        for layer in layers {
            for &i in layer {
                active[i] = true;
            }
        }
        let mut blocks = Blocks::empty();
        blocks.member_indices.extend(
            active
                .iter()
                .enumerate()
                .filter_map(|(i, &present)| (!present).then_some(i)),
        );
        // LR calls this before applying its final diagram margin. Reserve
        // the same nesting headroom while measuring compound rectangles,
        // then remove it so the caller can apply the final margin once.
        let margin = 2 * self.max_subgraph_depth() + 1;
        for node in nodes.iter_mut() {
            node.x += margin;
            node.y += margin;
        }
        self.separate_groups(nodes, &idx, &blocks, horizontal, gap);
        for node in nodes {
            node.x -= margin;
            node.y -= margin;
        }
    }

    /// Pack sibling groups as indivisible units, moving every member rank
    /// together. Packing individual ranks can widen a group back across the
    /// sibling that was just moved out of it.
    #[allow(clippy::too_many_lines, reason = "recursive compound group packing")]
    fn separate_groups(
        &self,
        nodes: &mut [LayoutNode],
        idx: &HashMap<&str, usize>,
        blocks: &Blocks,
        horizontal: bool,
        gap: usize,
    ) {
        if self.spec.subgraphs.is_empty() {
            return;
        }

        struct Unit {
            rect: Rect,
            members: Vec<usize>,
        }

        fn pack(
            mut units: Vec<Unit>,
            nodes: &mut [LayoutNode],
            horizontal: bool,
            gap: usize,
        ) -> Vec<Unit> {
            units.sort_by_key(|u| {
                if horizontal {
                    (u.rect.x, u.rect.y)
                } else {
                    (u.rect.y, u.rect.x)
                }
            });
            for i in 0..units.len() {
                let current = units[i].rect;
                let mut position = if horizontal { current.x } else { current.y };
                for previous in &units[..i] {
                    let r = previous.rect;
                    if horizontal {
                        if current.y < r.y + r.height && r.y < current.y + current.height {
                            position = position.max(r.x + r.width + gap);
                        }
                    } else if current.x < r.x + r.width && r.x < current.x + current.width {
                        position = position.max(r.y + r.height + gap);
                    }
                }
                let shift = position - if horizontal { current.x } else { current.y };
                for &member in &units[i].members {
                    if horizontal {
                        nodes[member].x += shift;
                    } else {
                        nodes[member].y += shift;
                    }
                }
                if horizontal {
                    units[i].rect.x = position;
                } else {
                    units[i].rect.y = position;
                }
            }
            units
        }

        fn group(
            sg: &SubgraphSpec,
            nodes: &mut [LayoutNode],
            idx: &HashMap<&str, usize>,
            blocks: &Blocks,
            horizontal: bool,
            gap: usize,
        ) -> Option<Unit> {
            let block = blocks.items.iter().find(|b| b.sg_id == sg.id);
            if block.is_some_and(|b| b.mode != BlockMode::AtPhantom) {
                return None;
            }
            let phantom = block
                .and_then(|b| idx.get(b.phantom_id.as_str()).copied())
                .or_else(|| {
                    idx.iter().find_map(|(&id, &i)| {
                        (id.strip_prefix(PHANTOM_PREFIX) == Some(sg.id.as_str())).then_some(i)
                    })
                });
            if let Some(i) = phantom {
                let n = &nodes[i];
                return Some(Unit {
                    rect: Rect::new(
                        n.x.saturating_sub(2),
                        n.y.saturating_sub(2),
                        FlowchartRenderer::group_title_right(
                            sg,
                            n.x.saturating_sub(2),
                            n.x + n.width + 2,
                            nodes,
                            idx,
                        ) + 1
                            - n.x.saturating_sub(2),
                        n.height + 4,
                    ),
                    members: vec![i],
                });
            }
            let mut units = Vec::new();
            for child in &sg.subgraphs {
                if let Some(unit) = group(child, nodes, idx, blocks, horizontal, gap) {
                    units.push(unit);
                }
            }
            for id in &sg.nodes {
                if let Some(&i) = idx.get(id.as_str())
                    && !blocks.member_indices.contains(&i)
                {
                    let n = &nodes[i];
                    units.push(Unit {
                        rect: Rect::new(n.x, n.y, n.width, n.height),
                        members: vec![i],
                    });
                }
            }
            let units = pack(units, nodes, horizontal, gap);
            let first = units.first()?;
            let mut left = first.rect.x;
            let mut top = first.rect.y;
            let mut right = first.rect.right();
            let mut bottom = first.rect.bottom();
            let mut members = Vec::new();
            for unit in units {
                left = left.min(unit.rect.x);
                top = top.min(unit.rect.y);
                right = right.max(unit.rect.right());
                bottom = bottom.max(unit.rect.bottom());
                members.extend(unit.members);
            }
            let x = left.saturating_sub(2);
            let y = top.saturating_sub(2);
            Some(Unit {
                rect: Rect::new(
                    x,
                    y,
                    FlowchartRenderer::group_title_right(sg, x, right + 2, nodes, idx) + 1 - x,
                    bottom + 2 - y,
                ),
                members,
            })
        }

        let mut units = Vec::new();
        let mut grouped = vec![false; nodes.len()];
        for sg in &self.spec.subgraphs {
            if let Some(unit) = group(sg, nodes, idx, blocks, horizontal, gap) {
                for &member in &unit.members {
                    grouped[member] = true;
                }
                units.push(unit);
            }
        }
        for (i, n) in nodes.iter().enumerate() {
            if !grouped[i] && !blocks.member_indices.contains(&i) {
                units.push(Unit {
                    rect: Rect::new(n.x, n.y, n.width, n.height),
                    members: vec![i],
                });
            }
        }
        drop(pack(units, nodes, horizontal, gap));
    }

    /// Deepest subgraph nesting level (1 for a flat top-level subgraph). Also
    /// the maximum border-chain extent: a member at nesting depth `d` has
    /// `d` group border rows below its box and `2*d` above (pad per level).
    pub(super) fn max_subgraph_depth(&self) -> usize {
        fn rec(sgs: &[SubgraphSpec]) -> usize {
            sgs.iter()
                .map(|sg| rec(&sg.subgraphs) + 1)
                .max()
                .unwrap_or(0)
        }
        rec(&self.spec.subgraphs)
    }

    /// Draws subgraph grouping boxes: bounding box of member nodes (and nested
    /// group rects) expanded by padding, with the title embedded in the top
    /// border. Drawn after nodes so borders land on empty cells; edges crossing
    /// a border render as junctions.
    #[allow(
        clippy::too_many_lines,
        reason = "group borders and title crossing detours"
    )]
    pub(super) fn draw_subgraphs(
        &self,
        canvas: &mut Canvas,
        nodes: &[LayoutNode],
        idx: &HashMap<&str, usize>,
        blocks: &Blocks,
    ) {
        let mut groups = self.collect_group_rects(nodes, idx, blocks);
        // Outer boxes first so nested borders layer cleanly
        groups.sort_by_key(|(r, _, _, _)| std::cmp::Reverse(r.width * r.height));

        for (r, _, title, color) in groups {
            let right = r.x + r.width - 1;
            let bottom = r.y + r.height - 1;
            let label = format!(" {title} ");
            let label_w = UnicodeWidthStr::width(label.as_str());
            if r.width > label_w + 2 {
                let title_x = (r.x + 2..=right - label_w)
                    .find(|&x| {
                        (x..x + label_w).all(|cx| {
                            canvas.get_cell(cx, r.y).is_none_or(|c| {
                                !c.conn.north
                                    && !c.conn.south
                                    && c.role != CellRole::Arrow
                                    && !(c.role == CellRole::Text && c.ch != ' ')
                            })
                        })
                    })
                    .unwrap_or(r.x + 2);
                // External corridors can still cross the reserved title
                // shelf. Jog around the entire label, keeping both the title
                // and the incoming/outgoing wire intact.
                let bypass = title_x + label_w;
                let horizontal_crossing = (title_x..bypass).find_map(|x| {
                    canvas.get_cell(x, r.y).and_then(|c| {
                        (c.is_line && (c.conn.east || c.conn.west)).then_some(c.color)
                    })
                });
                if let Some(edge_color) = horizontal_crossing {
                    canvas.set_pen(edge_color);
                    canvas.draw_hline(title_x - 1, bypass, r.y + 1);
                    canvas.draw_vline(title_x - 1, r.y, r.y + 1);
                    canvas.draw_vline(bypass, r.y, r.y + 1);
                }
                for x in title_x..bypass {
                    let Some(cell) = canvas.get_cell(x, r.y).copied() else {
                        continue;
                    };
                    if (cell.conn.north || cell.conn.south) && r.y > 0 {
                        canvas.set_pen(cell.color);
                        if cell.conn.north {
                            canvas.draw_hline(x, bypass, r.y - 1);
                        }
                        if cell.conn.south {
                            canvas.draw_hline(x, bypass, r.y + 1);
                        }
                        canvas.draw_vline(bypass, r.y - 1, r.y + 1);
                    }
                }
                canvas.set_pen(color);
                canvas.draw_text(title_x, r.y, &label);
            }
            canvas.set_pen(color);
            canvas.draw_hline(r.x, right, r.y);
            canvas.draw_hline(r.x, right, bottom);
            canvas.draw_vline(r.x, r.y, bottom);
            canvas.draw_vline(right, r.y, bottom);
            canvas.draw_corner(
                r.x,
                r.y,
                crate::canvas::LineConn {
                    south: true,
                    east: true,
                    ..Default::default()
                },
            );
            canvas.draw_corner(
                right,
                r.y,
                crate::canvas::LineConn {
                    south: true,
                    west: true,
                    ..Default::default()
                },
            );
            canvas.draw_corner(
                r.x,
                bottom,
                crate::canvas::LineConn {
                    north: true,
                    east: true,
                    ..Default::default()
                },
            );
            canvas.draw_corner(
                right,
                bottom,
                crate::canvas::LineConn {
                    north: true,
                    west: true,
                    ..Default::default()
                },
            );
            canvas.set_pen(None);
        }
    }

    /// True when a down-arrowhead already occupies (`x`, `y`). Bend and jump
    /// edges reuse a nearby arrow column so a target fed from two sides shows
    /// one converging arrowhead instead of adjacent `▼▼` pairs.
    fn arrow_down_at(&self, canvas: &Canvas, x: usize, y: usize) -> bool {
        canvas
            .get_cell(x, y)
            .is_some_and(|c| c.ch == self.theme.arrow_down())
    }

    /// Rectangles can converge on a nearby arrowhead; decisions must always
    /// enter at their diamond badge, never at a neighboring border cell.
    fn drop_x_for(&self, canvas: &Canvas, v: &LayoutNode) -> usize {
        let v_cx = v.x + v.width / 2;
        let v_top = v.y - 1;
        if v.shape == NodeShape::Diamond {
            return v_cx;
        }
        if self.arrow_down_at(canvas, v_cx, v_top) {
            return v_cx;
        }
        if self.arrow_down_at(canvas, v_cx + 1, v_top) {
            return v_cx + 1;
        }
        if v_cx > 0 && self.arrow_down_at(canvas, v_cx - 1, v_top) {
            return v_cx - 1;
        }
        v_cx
    }

    /// Sugiyama crossing reduction: reorders nodes within each layer by the
    /// barycenter (average ordinal) of their neighbors in the adjacent layer.
    /// Two down + up sweep rounds; stable sort — ties keep current order.
    /// Only rank-adjacent layers are considered; multi-rank jumps route via
    /// corridors and don't participate.
    pub(super) fn reduce_crossings(&self, layers: &mut [Vec<usize>], idx: &HashMap<&str, usize>) {
        if layers.len() < 2 {
            return;
        }
        // node index -> (layer, ordinal)
        let mut pos: HashMap<usize, (usize, usize)> = HashMap::new();
        for (r, layer) in layers.iter().enumerate() {
            for (o, &i) in layer.iter().enumerate() {
                pos.insert(i, (r, o));
            }
        }
        // undirected adjacency (self-loops excluded)
        let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
        for e in &self.spec.edges {
            if let (Some(&ui), Some(&vi)) = (idx.get(e.from.as_str()), idx.get(e.to.as_str()))
                && ui != vi
            {
                adj.entry(ui).or_default().push(vi);
                adj.entry(vi).or_default().push(ui);
            }
        }

        // Average ordinal of `node`'s neighbors located in layer `toward`.
        // NaN when the node has no neighbors there (keeps current order).
        let barycenter =
            |node: usize, toward: usize, pos: &HashMap<usize, (usize, usize)>| -> f64 {
                let mut sum = 0.0f64;
                let mut n = 0usize;
                if let Some(nbrs) = adj.get(&node) {
                    for &nb in nbrs {
                        if let Some(&(lr, ord)) = pos.get(&nb)
                            && lr == toward
                        {
                            sum += ord as f64;
                            n += 1;
                        }
                    }
                }
                if n == 0 { f64::NAN } else { sum / n as f64 }
            };

        // Reorder layer `r` by neighbor barycenter in layer `toward`
        let sweep = |layers: &mut [Vec<usize>],
                     pos: &mut HashMap<usize, (usize, usize)>,
                     r: usize,
                     toward: usize| {
            let mut keyed: Vec<(usize, f64, usize)> = layers[r]
                .iter()
                .enumerate()
                .map(|(o, &i)| (i, barycenter(i, toward, pos), o))
                .collect();
            keyed.sort_by(|a, b| {
                let (ba, bb) = (a.1, b.1);
                let ord = if ba.is_nan() && bb.is_nan() {
                    std::cmp::Ordering::Equal
                } else if ba.is_nan() {
                    std::cmp::Ordering::Greater
                } else if bb.is_nan() {
                    std::cmp::Ordering::Less
                } else {
                    ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
                };
                ord.then(a.2.cmp(&b.2))
            });
            layers[r] = keyed.into_iter().map(|(i, _, _)| i).collect();
            for (o, &i) in layers[r].iter().enumerate() {
                pos.insert(i, (r, o));
            }
        };

        for _ in 0..2 {
            for r in 1..layers.len() {
                sweep(layers, &mut pos, r, r - 1);
            }
            for r in (0..layers.len().saturating_sub(1)).rev() {
                sweep(layers, &mut pos, r, r + 1);
            }
        }
    }

    fn assign_ranks(
        &self,
        nodes: &mut [LayoutNode],
        idx: &HashMap<&str, usize>,
    ) -> Vec<Vec<usize>> {
        let n = nodes.len();
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        for edge in &self.spec.edges {
            if let (Some(&u), Some(&v)) = (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
                && u != v
            {
                adj[u].push(v);
                in_degree[v] += 1;
            }
        }

        // Detect back-edges (cycles/feedback loops) using DFS so that layering operates on a strict DAG
        let mut color = vec![0u8; n]; // 0 = unvisited, 1 = visiting, 2 = visited
        let mut back_edges: HashSet<(usize, usize)> = HashSet::new();

        // Start DFS from in-degree 0 nodes (natural inputs/sources)
        for i in 0..n {
            if in_degree[i] == 0 && color[i] == 0 {
                dfs_find_cycles(i, &adj, &mut color, &mut back_edges);
            }
        }
        // Then any remaining unvisited nodes
        for i in 0..n {
            if color[i] == 0 {
                dfs_find_cycles(i, &adj, &mut color, &mut back_edges);
            }
        }

        // Build DAG without back-edges
        let mut dag_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut dag_in_deg = vec![0usize; n];
        for u in 0..n {
            for &v in &adj[u] {
                if !back_edges.contains(&(u, v)) {
                    dag_adj[u].push(v);
                    dag_in_deg[v] += 1;
                }
            }
        }

        // Topological longest-path layering on the DAG
        let mut queue: VecDeque<usize> = (0..n).filter(|&i| dag_in_deg[i] == 0).collect();

        while let Some(u) = queue.pop_front() {
            let u_rank = nodes[u].rank;
            for &v in &dag_adj[u] {
                if u_rank + 1 > nodes[v].rank {
                    nodes[v].rank = u_rank + 1;
                }
                let deg = &mut dag_in_deg[v];
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(v);
                }
            }
        }

        // Group by rank, preserving declaration order within ranks
        let max_rank = nodes.iter().map(|nd| nd.rank).max().unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
        for (i, nd) in nodes.iter().enumerate() {
            layers[nd.rank].push(i);
        }

        layers
    }

    /// Splits an edge label into stacked display lines: explicit `\n` (from
    /// the parser's `lines` population or `clean_label`) plus a defensive
    /// `<br/>` split. Empty segments are dropped.
    fn edge_label_lines(&self, edge: &EdgeSpec) -> Vec<String> {
        let raw: Vec<String> = if !edge.lines.is_empty() {
            edge.lines.clone()
        } else {
            edge.label
                .as_deref()
                .map(|l| l.split('\n').map(str::to_string).collect())
                .unwrap_or_default()
        };
        raw.iter()
            .flat_map(|l| l.split("<br/>"))
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Draws stacked edge-label lines centered at `cx`. With `up`, the block
    /// grows upward from `base_y` (labels above the edge); otherwise downward.
    fn draw_stacked_label(
        &self,
        canvas: &mut Canvas,
        lines: &[String],
        cx: usize,
        base_y: usize,
        up: bool,
        band: Option<Rect>,
    ) {
        let y = if up {
            base_y.saturating_sub(lines.len().saturating_sub(1))
        } else {
            base_y
        };
        Self::draw_label_block(canvas, lines, cx, y, true, band);
    }

    /// Draws stacked edge-label lines left-aligned at `x` (self-loops and
    /// loop-back tracks, where centering would straddle the loop glyphs).
    fn draw_stacked_label_left(
        canvas: &mut Canvas,
        lines: &[String],
        x: usize,
        base_y: usize,
        down: bool,
        band: Option<Rect>,
    ) {
        let y = if down {
            base_y
        } else {
            base_y.saturating_sub(lines.len().saturating_sub(1))
        };
        Self::draw_label_block(canvas, lines, x, y, false, band);
    }

    /// Move a complete label to the nearest clear position, never its individual
    /// lines. Blank columns on either side protect both strokes and wide glyphs.
    fn draw_label_block(
        canvas: &mut Canvas,
        lines: &[String],
        preferred_x: usize,
        preferred_y: usize,
        centered: bool,
        band: Option<Rect>,
    ) {
        if lines.is_empty() {
            return;
        }
        let width = lines
            .iter()
            .map(|line| display_width(line))
            .max()
            .unwrap_or(0);
        let preferred_x = if centered {
            preferred_x.saturating_sub(width / 2)
        } else {
            preferred_x
        };
        let clear = |x: usize, y: usize| {
            // Adjacent-rank labels belong between their endpoint boxes, not
            // above the source (TB) or before it (LR). If a multiline label
            // exceeds the band, enlarge only that minimum text extent; the
            // unbounded cross-axis still supplies a lossless clear position.
            if let Some(band) = band
                && (x < band.x
                    || y < band.y
                    || x + width > band.x.saturating_add(band.width.max(width))
                    || y + lines.len() > band.y.saturating_add(band.height.max(lines.len())))
            {
                return false;
            }
            let left = x.saturating_sub(1);
            let right = x + width;
            let bottom = y + lines.len() - 1;
            canvas.obstacles.iter().all(|obstacle| {
                right < obstacle.x
                    || left > obstacle.right()
                    || bottom < obstacle.y
                    || y > obstacle.bottom()
            }) && (y..=bottom).all(|row| {
                (left..=right).all(|column| {
                    canvas.get_cell(column, row).is_none_or(|cell| {
                        cell.ch == ' '
                            && !cell.is_line
                            && !cell.is_continuation
                            && !matches!(cell.role, CellRole::Border | CellRole::Arrow)
                    })
                })
            })
        };
        // Manhattan rings stay near the requested route anchor. Prefer
        // horizontal movement on ties, keeping labels in their edge's band.
        // A position beyond the finite canvas is always clear.
        let mut distance = 0;
        let (x, y) = 'search: loop {
            for dy in 0..=distance {
                let dx = distance - dy;
                let xs = [
                    Some(preferred_x + dx),
                    preferred_x.checked_sub(dx).filter(|_| dx > 0),
                ];
                let ys = [
                    preferred_y.checked_sub(dy),
                    (dy > 0).then_some(preferred_y + dy),
                ];
                for y in ys.into_iter().flatten() {
                    for x in xs.into_iter().flatten() {
                        if clear(x, y) {
                            break 'search (x, y);
                        }
                    }
                }
            }
            distance += 1;
        };
        for (i, line) in lines.iter().enumerate() {
            let offset = if centered {
                width / 2 - display_width(line) / 2
            } else {
                0
            };
            canvas.draw_text(x + offset, y + i, line);
        }
        canvas.add_obstacle(Rect::new(x, y, width, lines.len()));
    }

    /// Draws a self-referencing edge (`A --> A`) as a rectangular arc off the
    /// right wall of the box, re-entering one row lower.
    fn draw_self_loop(
        &self,
        canvas: &mut Canvas,
        edge: &EdgeSpec,
        u: &LayoutNode,
        labels: &mut Vec<PendingLabel>,
    ) {
        let y0 = u.y + u.height / 2;
        let y1 = u.y + u.height - 1;
        if y1 <= y0 {
            return;
        }
        let x0 = u.x + u.width;
        let x1 = x0 + 3;

        edge_hline(canvas, edge, x0, x1, y0, &self.theme);
        edge_vline(canvas, edge, x1, y0, y1, &self.theme);
        edge_hline(canvas, edge, x0, x1, y1, &self.theme);
        edge_arrow_heads(
            canvas,
            edge,
            (x0, y1, Direction::Left),
            (x0, y0, Direction::Right),
            &self.theme,
        );

        // Preferred position leaves one blank cell after the loop's corner
        // glyph so the label never abuts it (FC-EDGE-08)
        let lines = self.edge_label_lines(edge);
        if !lines.is_empty() {
            labels.push(
                PendingLabel::left(lines, x1 + 2, y0)
                    .within(Rect::new(x1 + 2, y0, usize::MAX, y1 - y0 + 1)),
            );
        }
    }

    #[allow(clippy::too_many_lines, reason = "one branch per node shape")]
    fn draw_node(&self, canvas: &mut Canvas, node: &LayoutNode) {
        let is_ascii = self.theme.box_style == BoxStyle::Ascii;
        canvas.set_pen(node.color);
        // Mermaid `class`/`style` `stroke-dasharray` → dashed border;
        // `stroke-width` → weighted border glyphs (box edges only — dashed
        // and weighted are mutually exclusive, dashed wins)
        let node_box = |canvas: &mut Canvas, theme: &Theme, title: Option<&str>| {
            if node.dashed_border {
                canvas.draw_dashed_box(node.x, node.y, node.width, node.height, theme, title);
            } else if node.border_level > 0 {
                canvas.draw_weighted_box(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    theme,
                    title,
                    node.border_level,
                );
            } else {
                canvas.draw_box(node.x, node.y, node.width, node.height, theme, title);
            }
        };
        // Mermaid `fill:<color>` → label text color (ANSI foreground)
        macro_rules! draw_label {
            ($canvas:expr, $lines:expr, $text_start_y:expr, $offset_x:expr) => {
                if node.fill_color.is_some() {
                    $canvas.set_text_pen(node.fill_color);
                }
                for (i, line) in $lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w + $offset_x.1 {
                        (node.width - line_w) / 2
                    } else {
                        $offset_x.0
                    };
                    $canvas.draw_text(node.x + offset_x, $text_start_y + i, line);
                }
                if node.fill_color.is_some() {
                    $canvas.set_text_pen(None);
                }
            };
        }
        match node.shape {
            NodeShape::Diamond => {
                if is_ascii && !node.dashed_border {
                    // True diamond silhouette in the documented 7-bit glyph
                    // set (`/ \ < > ^ v`) — the unicode decision box would
                    // leak `=`/`#`/`<?>` foreign glyphs into ascii (FC-TB-02)
                    canvas.draw_diamond(node.x, node.y, node.width, node.height, &self.theme);
                } else {
                    canvas.draw_decision_box(
                        node.x,
                        node.y,
                        node.width,
                        node.height,
                        &self.theme,
                        Some("◇"),
                        node.dashed_border,
                    );
                }
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 2));
            }
            NodeShape::Circle => {
                // Circular summing junction / comparator with circular indicator badge
                let circle_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Rounded)
                };
                let badge = if is_ascii { "(o)" } else { "○" };
                node_box(canvas, &circle_theme, Some(badge));
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
            NodeShape::Hexagon => {
                // Preparation / condition: sharp box with hexagon badge
                let badge = if is_ascii { "<h>" } else { "⬡" };
                node_box(canvas, &self.theme, Some(badge));
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
            NodeShape::DoubleCircle => {
                // Start / end point: rounded box with bullseye badge
                let circle_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Rounded)
                };
                let badge = if is_ascii { "(oo)" } else { "◎" };
                node_box(canvas, &circle_theme, Some(badge));
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
            NodeShape::Parallelogram | NodeShape::ParallelogramAlt => {
                // Input / output: sharp box with parallelogram badge
                let sub_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Sharp)
                };
                let badge = if is_ascii { "/_/" } else { "▱" };
                node_box(canvas, &sub_theme, Some(badge));
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
            NodeShape::Trapezoid | NodeShape::TrapezoidAlt => {
                // Manual input / operation: sharp box with trapezoid badge
                let sub_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Sharp)
                };
                let badge = "/__\\";
                node_box(canvas, &sub_theme, Some(badge));
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
            NodeShape::Subprocess => {
                // Double vertical side borders for complex components / plant dynamics
                let sub_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Sharp)
                };
                node_box(canvas, &sub_theme, None);
                let right = node.x + node.width - 1;
                let bottom = node.y + node.height - 1;

                if is_ascii {
                    if node.width >= 6 {
                        for r in (node.y + 1)..bottom {
                            canvas.put_char(node.x + 1, r, '|');
                            canvas.put_char(right - 1, r, '|');
                        }
                    }
                } else {
                    // Corners from the glyph tables too (keep the weighted
                    // corner-skip: draw_weighted_box already resolves heavy/
                    // double corners at render time)
                    if node.border_level == 0 {
                        canvas.put_char(node.x, node.y, self.theme.top_left_corner());
                        canvas.put_char(right, node.y, self.theme.top_right_corner());
                        canvas.put_char(node.x, bottom, self.theme.bottom_left_corner());
                        canvas.put_char(right, bottom, self.theme.bottom_right_corner());
                    }

                    if node.width >= 6 {
                        // Border-aware tees and inner walls so weighted
                        // (stroke-width) subprocesses stay one glyph family
                        let (tee_down, tee_up, wall) = if node.border_level > 0 {
                            (
                                self.theme.thick_tee_down(),
                                self.theme.thick_tee_up(),
                                self.theme.thick_vertical_line(),
                            )
                        } else {
                            (
                                self.theme.tee_down(),
                                self.theme.tee_up(),
                                self.theme.vertical_line(),
                            )
                        };
                        let left_inner_x = node.x + 1;
                        let right_inner_x = right - 1;
                        canvas.put_char(left_inner_x, node.y, tee_down);
                        canvas.put_char(left_inner_x, bottom, tee_up);
                        canvas.put_char(right_inner_x, node.y, tee_down);
                        canvas.put_char(right_inner_x, bottom, tee_up);
                        for r in (node.y + 1)..bottom {
                            canvas.put_char(left_inner_x, r, wall);
                            canvas.put_char(right_inner_x, r, wall);
                        }
                    }
                }
                draw_label!(canvas, node.label_lines, node.y + 1, (2, 4));
            }
            NodeShape::Database => {
                // True cylinder silhouette: theme box supplies the curved top
                // arc (╭─╮) and bottom arc (╰─╯); paren side walls `( … )`
                // replace the flat side borders. No mid divider — that read
                // as a generic banded box instead of a cylinder.
                node_box(canvas, &self.theme, None);
                let right = node.x + node.width - 1;
                let bottom = node.y + node.height - 1;
                for r in (node.y + 1)..bottom {
                    canvas.put_char_with_role(node.x, r, '(', CellRole::Border);
                    canvas.put_char_with_role(right, r, ')', CellRole::Border);
                }
                draw_label!(canvas, node.label_lines, node.y + 2, (1, 0));
            }
            NodeShape::Box => {
                // Plain rectangular block; the whole frame — corners, edges,
                // weighted and dashed variants — resolves from the output
                // theme's glyph tables at render time, so every style stays
                // one pure family (rounded ╭──╮, heavy ┏━┓┃, double ╔═╗║,
                // sharp ┌─┐│, ascii +-+|)
                node_box(canvas, &self.theme, None);
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
            _ => {
                // Rounded / Stadium
                let rounded_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Rounded)
                };
                node_box(canvas, &rounded_theme, None);
                draw_label!(canvas, node.label_lines, node.y + 1, (1, 0));
            }
        }
        canvas.set_pen(None);
    }
}

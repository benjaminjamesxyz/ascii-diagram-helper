use crate::canvas::{Canvas, CellRole, Direction, Rect};
use crate::color::Color;
use crate::schema::{EdgeSpec, FlowchartSpec, LayoutDirection, NodeShape, SubgraphSpec};
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
    /// Propagated from `NodeSpec::thick_border` (`stroke-width:>=2`)
    thick_border: bool,
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

/// A subgraph with its own `direction` that is edge-isolated from the rest of
/// the diagram: rendered independently in its own orientation and pasted as a
/// pre-composed block. Mermaid applies subgraph `direction` under the same
/// restriction (disconnected clusters only).
pub(super) struct IsolatedBlock {
    /// Subgraph id this block was rendered from
    sg_id: String,
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
    /// Indices into `spec.nodes` of ALL (recursive) members of moved subgraphs
    member_indices: HashSet<usize>,
}

impl Blocks {
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn rect_for(&self, sg_id: &str) -> Option<(usize, usize, usize, usize)> {
        // Assigned during layout; stored alongside by render_tb/render_lr
        self.items
            .iter()
            .find(|b| b.sg_id == sg_id)
            .map(|b| (b.origin_x, b.origin_y, b.width, b.height))
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

        let is_lr = matches!(
            self.spec.direction,
            LayoutDirection::LR | LayoutDirection::RL
        );
        let mut blocks = self.collect_isolated_blocks(colored);
        if is_lr {
            self.render_lr(colored, &mut blocks)
        } else {
            self.render_tb(colored, &mut blocks)
        }
    }

    /// Finds subgraphs with an explicit `direction` that differs from the
    /// global direction AND whose members have no edges to nodes outside the
    /// subgraph. Each such subgraph is rendered recursively as a standalone
    /// diagram (its own direction applies inside). Nested subgraphs of a moved
    /// subgraph are handled inside the recursive render, not visited again.
    fn collect_isolated_blocks(&self, colored: bool) -> Blocks {
        let idx = self.index_of();
        let mut items = Vec::new();
        let mut member_indices = HashSet::new();
        self.collect_sgs(
            &self.spec.subgraphs,
            &idx,
            colored,
            &mut items,
            &mut member_indices,
        );
        Blocks {
            items,
            member_indices,
        }
    }

    fn collect_sgs(
        &self,
        sgs: &[SubgraphSpec],
        idx: &HashMap<&str, usize>,
        colored: bool,
        out: &mut Vec<IsolatedBlock>,
        member_indices: &mut HashSet<usize>,
    ) {
        for sg in sgs {
            let wants_move = sg.direction.is_some_and(|d| d != self.spec.direction);
            let mut moved = false;
            if wants_move {
                let members = member_ids(sg);
                if Self::is_isolated(self.spec, &members) {
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
                    for id in &members {
                        if let Some(&i) = idx.get(id.as_str()) {
                            member_indices.insert(i);
                        }
                    }
                    out.push(IsolatedBlock {
                        sg_id: sg.id.clone(),
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
                self.collect_sgs(&sg.subgraphs, idx, colored, out, member_indices);
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

    fn prepare_nodes(&self) -> Vec<LayoutNode> {
        let mut nodes = Vec::with_capacity(self.spec.nodes.len());

        for node in &self.spec.nodes {
            let label = if node.label.is_empty() {
                &node.id
            } else {
                &node.label
            };

            // Wrap text nicely if long
            let mut lines: Vec<String> = Vec::new();
            for part in label.split('\n') {
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
                    let h = lines.len() + 2;
                    (w, h)
                }
                NodeShape::Database => {
                    let w = (max_text_w + 4).max(8);
                    let h = lines.len() + 3; // extra row for cylinder header
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
                thick_border: node.thick_border,
                width,
                height,
                x: 0,
                y: 0,
                rank: 0,
            });
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

    /// Draws subgraph grouping boxes: bounding box of member nodes (and nested
    /// group rects) expanded by padding, with the title embedded in the top
    /// border. Drawn after nodes so borders land on empty cells; edges crossing
    /// a border render as junctions.
    pub(super) fn draw_subgraphs(
        &self,
        canvas: &mut Canvas,
        nodes: &[LayoutNode],
        idx: &HashMap<&str, usize>,
        blocks: &Blocks,
    ) {
        let (pad_x, pad_top, pad_bottom) = (2usize, 2usize, 1usize);

        fn group_rect(
            sg: &SubgraphSpec,
            nodes: &[LayoutNode],
            idx: &HashMap<&str, usize>,
            pad: (usize, usize, usize),
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
                if let Some(r) = group_rect(child, nodes, idx, pad) {
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
            // Widen the box so the title fits on the top border
            let title = sg.title.clone().unwrap_or_else(|| sg.id.clone());
            let min_w = UnicodeWidthStr::width(format!(" {title} ").as_str()) + 4;
            if br - bx + 1 < min_w {
                br = bx + min_w - 1;
            }
            Some(Rect::new(bx, by, br - bx + 1, bb - by + 1))
        }

        fn collect(
            sgs: &[SubgraphSpec],
            nodes: &[LayoutNode],
            idx: &HashMap<&str, usize>,
            pad: (usize, usize, usize),
            blocks: &Blocks,
            out: &mut Vec<(Rect, String, Option<Color>)>,
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
                    let min_w = UnicodeWidthStr::width(format!(" {title} ").as_str()) + 4;
                    if br - bx + 1 < min_w {
                        br = bx + min_w - 1;
                    }
                    out.push((
                        Rect::new(bx, by, br - bx + 1, bb - by + 1),
                        title,
                        sg.color,
                    ));
                    continue;
                }
                if let Some(r) = group_rect(sg, nodes, idx, pad) {
                    out.push((
                        r,
                        sg.title.clone().unwrap_or_else(|| sg.id.clone()),
                        sg.color,
                    ));
                }
                collect(&sg.subgraphs, nodes, idx, pad, blocks, out);
            }
        }

        let mut groups: Vec<(Rect, String, Option<Color>)> = Vec::new();
        collect(
            &self.spec.subgraphs,
            nodes,
            idx,
            (pad_x, pad_top, pad_bottom),
            blocks,
            &mut groups,
        );
        // Outer boxes first so nested borders layer cleanly
        groups.sort_by_key(|(r, _, _)| std::cmp::Reverse(r.width * r.height));

        for (r, title, color) in groups {
            let right = r.x + r.width - 1;
            let bottom = r.y + r.height - 1;
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
            let label = format!(" {title} ");
            let label_w = UnicodeWidthStr::width(label.as_str());
            if r.width > label_w + 2 {
                canvas.draw_text(r.x + 2, r.y, &label);
            }
            canvas.set_pen(None);
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

    /// Draws a self-referencing edge (`A --> A`) as a rectangular arc off the
    /// right wall of the box, re-entering one row lower.
    pub(super) fn draw_self_loop(&self, canvas: &mut Canvas, edge: &EdgeSpec, u: &LayoutNode) {
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

        if let Some(ref lbl) = edge.label {
            canvas.draw_text_safe(x1 + 1, y0, lbl);
        }
    }

    #[allow(clippy::too_many_lines, reason = "one branch per node shape")]
    fn draw_node(&self, canvas: &mut Canvas, node: &LayoutNode) {
        let is_ascii = self.theme.box_style == BoxStyle::Ascii;
        canvas.set_pen(node.color);
        // Mermaid `class`/`style` `stroke-dasharray` → dashed border;
        // `stroke-width:>=2` → heavy border glyphs (box edges only — dashed
        // and thick are mutually exclusive, dashed wins)
        let node_box = |canvas: &mut Canvas, theme: &Theme, title: Option<&str>| {
            if node.dashed_border {
                canvas.draw_dashed_box(node.x, node.y, node.width, node.height, theme, title);
            } else if node.thick_border {
                canvas.draw_thick_box(node.x, node.y, node.width, node.height, theme, title);
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
                canvas.draw_decision_box(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    &self.theme,
                    Some("◇"),
                    node.dashed_border,
                );
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
                    if !node.thick_border {
                        canvas.put_char(node.x, node.y, '┌');
                        canvas.put_char(right, node.y, '┐');
                        canvas.put_char(node.x, bottom, '└');
                        canvas.put_char(right, bottom, '┘');
                    }

                    if node.width >= 6 {
                        let left_inner_x = node.x + 1;
                        let right_inner_x = right - 1;
                        canvas.put_char(left_inner_x, node.y, '┬');
                        canvas.put_char(left_inner_x, bottom, '┴');
                        canvas.put_char(right_inner_x, node.y, '┬');
                        canvas.put_char(right_inner_x, bottom, '┴');
                        for r in (node.y + 1)..bottom {
                            canvas.put_char(left_inner_x, r, '│');
                            canvas.put_char(right_inner_x, r, '│');
                        }
                    }
                }
                draw_label!(canvas, node.label_lines, node.y + 1, (2, 4));
            }
            NodeShape::Database => {
                node_box(canvas, &self.theme, None);
                // Cylinder separator line
                if node.height >= 3 {
                    canvas.draw_hline(node.x, node.x + node.width - 1, node.y + 1);
                    canvas.draw_corner(
                        node.x,
                        node.y + 1,
                        crate::canvas::LineConn {
                            north: true,
                            south: true,
                            east: true,
                            west: false,
                        },
                    );
                    canvas.draw_corner(
                        node.x + node.width - 1,
                        node.y + 1,
                        crate::canvas::LineConn {
                            north: true,
                            south: true,
                            west: true,
                            east: false,
                        },
                    );
                }
                draw_label!(canvas, node.label_lines, node.y + 2, (1, 0));
            }
            NodeShape::Box => {
                // Sharp rectangular technical block
                let box_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Sharp)
                };
                node_box(canvas, &box_theme, None);
                // Literal sharp corners force the block look in any theme;
                // thick boxes resolve corners from the heavy glyph table
                if !is_ascii && !node.thick_border {
                    let right = node.x + node.width - 1;
                    let bottom = node.y + node.height - 1;
                    // Border role so the pen color stamps the corners
                    canvas.put_char_with_role(node.x, node.y, '┌', CellRole::Border);
                    canvas.put_char_with_role(right, node.y, '┐', CellRole::Border);
                    canvas.put_char_with_role(node.x, bottom, '└', CellRole::Border);
                    canvas.put_char_with_role(right, bottom, '┘', CellRole::Border);
                }
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

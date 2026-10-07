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
        if is_lr {
            self.render_lr(colored)
        } else {
            self.render_tb(colored)
        }
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
            out: &mut Vec<(Rect, String, Option<Color>)>,
        ) {
            for sg in sgs {
                if let Some(r) = group_rect(sg, nodes, idx, pad) {
                    out.push((
                        r,
                        sg.title.clone().unwrap_or_else(|| sg.id.clone()),
                        sg.color,
                    ));
                }
                collect(&sg.subgraphs, nodes, idx, pad, out);
            }
        }

        let mut groups: Vec<(Rect, String, Option<Color>)> = Vec::new();
        collect(
            &self.spec.subgraphs,
            nodes,
            idx,
            (pad_x, pad_top, pad_bottom),
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
        // Mermaid `class`/`style` `stroke-dasharray` → dashed border
        let node_box = |canvas: &mut Canvas, theme: &Theme, title: Option<&str>| {
            if node.dashed_border {
                canvas.draw_dashed_box(node.x, node.y, node.width, node.height, theme, title);
            } else {
                canvas.draw_box(node.x, node.y, node.width, node.height, theme, title);
            }
        };
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
                let text_start_y = node.y + 1;
                for (i, line) in node.label_lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w + 2 {
                        (node.width - line_w) / 2
                    } else {
                        1
                    };
                    canvas.draw_text(node.x + offset_x, text_start_y + i, line);
                }
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
                let text_start_y = node.y + 1;
                for (i, line) in node.label_lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w {
                        (node.width - line_w) / 2
                    } else {
                        1
                    };
                    canvas.draw_text(node.x + offset_x, text_start_y + i, line);
                }
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
                    canvas.put_char(node.x, node.y, '┌');
                    canvas.put_char(right, node.y, '┐');
                    canvas.put_char(node.x, bottom, '└');
                    canvas.put_char(right, bottom, '┘');

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
                let text_start_y = node.y + 1;
                for (i, line) in node.label_lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w + 4 {
                        (node.width - line_w) / 2
                    } else {
                        2
                    };
                    canvas.draw_text(node.x + offset_x, text_start_y + i, line);
                }
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
                let text_start_y = node.y + 2;
                for (i, line) in node.label_lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w {
                        (node.width - line_w) / 2
                    } else {
                        1
                    };
                    canvas.draw_text(node.x + offset_x, text_start_y + i, line);
                }
            }
            NodeShape::Box => {
                // Sharp rectangular technical block
                let box_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Sharp)
                };
                node_box(canvas, &box_theme, None);
                if !is_ascii {
                    let right = node.x + node.width - 1;
                    let bottom = node.y + node.height - 1;
                    // Border role so the pen color stamps the corners
                    canvas.put_char_with_role(node.x, node.y, '┌', CellRole::Border);
                    canvas.put_char_with_role(right, node.y, '┐', CellRole::Border);
                    canvas.put_char_with_role(node.x, bottom, '└', CellRole::Border);
                    canvas.put_char_with_role(right, bottom, '┘', CellRole::Border);
                }
                let text_start_y = node.y + 1;
                for (i, line) in node.label_lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w {
                        (node.width - line_w) / 2
                    } else {
                        1
                    };
                    canvas.draw_text(node.x + offset_x, text_start_y + i, line);
                }
            }
            _ => {
                // Rounded / Stadium
                let rounded_theme = if is_ascii {
                    Theme::ascii()
                } else {
                    Theme::new(BoxStyle::Rounded)
                };
                node_box(canvas, &rounded_theme, None);
                let text_start_y = node.y + 1;
                for (i, line) in node.label_lines.iter().enumerate() {
                    let line_w = UnicodeWidthStr::width(line.as_str());
                    let offset_x = if node.width > line_w {
                        (node.width - line_w) / 2
                    } else {
                        1
                    };
                    canvas.draw_text(node.x + offset_x, text_start_y + i, line);
                }
            }
        }
        canvas.set_pen(None);
    }
}

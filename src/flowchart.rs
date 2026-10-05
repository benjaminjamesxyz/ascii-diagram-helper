use crate::canvas::{Canvas, Direction};
use crate::schema::{ArrowDirection, EdgeSpec, FlowchartSpec, LayoutDirection, NodeShape};
use crate::theme::{BoxStyle, Theme};
use std::collections::{HashMap, HashSet, VecDeque};
use unicode_width::UnicodeWidthStr;

/// Finds a clear row for a loop-back segment spanning `from_x..to_x`.
/// With `down`, routes below boxes that block the direct path (source side);
/// otherwise above them (target side). Returns `y` unchanged when clear.
fn clear_route_y(canvas: &Canvas, from_x: usize, to_x: usize, y: usize, down: bool) -> usize {
    let mut y_out = y;
    for obs in &canvas.obstacles {
        if obs.x < to_x && obs.right() > from_x && obs.y <= y && y <= obs.bottom() {
            if down {
                y_out = y_out.max(obs.bottom() + 1);
            } else {
                y_out = y_out.min(obs.y.saturating_sub(1));
            }
        }
    }
    y_out
}

fn edge_hline(canvas: &mut Canvas, edge: &EdgeSpec, x1: usize, x2: usize, y: usize, theme: &Theme) {
    if edge.dashed {
        canvas.draw_dashed_hline(x1, x2, y, theme);
    } else {
        canvas.draw_hline(x1, x2, y);
    }
}

fn edge_vline(canvas: &mut Canvas, edge: &EdgeSpec, x: usize, y1: usize, y2: usize, theme: &Theme) {
    if edge.dashed {
        canvas.draw_dashed_vline(x, y1, y2, theme);
    } else {
        canvas.draw_vline(x, y1, y2);
    }
}

/// Draws arrowheads per `edge.arrow`: Forward → target end only, Both → both
/// ends, None → none.
fn edge_arrow_heads(
    canvas: &mut Canvas,
    edge: &EdgeSpec,
    t: (usize, usize, Direction),
    s: (usize, usize, Direction),
    theme: &Theme,
) {
    match edge.arrow {
        ArrowDirection::None => {}
        ArrowDirection::Both => {
            canvas.draw_arrow(t.0, t.1, t.2, theme);
            canvas.draw_arrow(s.0, s.1, s.2, theme);
        }
        ArrowDirection::Back => canvas.draw_arrow(s.0, s.1, s.2, theme),
        ArrowDirection::Forward => canvas.draw_arrow(t.0, t.1, t.2, theme),
    }
}

#[derive(Clone, Debug)]
struct LayoutNode {
    label_lines: Vec<String>,
    shape: NodeShape,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rank: usize,
}

fn dfs_find_cycles(
    u: usize,
    adj: &[Vec<usize>],
    color: &mut [u8],
    back_edges: &mut HashSet<(usize, usize)>,
) {
    color[u] = 1;
    for &v in &adj[u] {
        match color[v] {
            1 => {
                // Back-edge detected!
                back_edges.insert((u, v));
            }
            0 => dfs_find_cycles(v, adj, color, back_edges),
            _ => {}
        }
    }
    color[u] = 2;
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
    pub fn render(&self) -> String {
        if self.spec.nodes.is_empty() {
            return String::new();
        }

        match self.spec.direction {
            LayoutDirection::LR | LayoutDirection::RL => self.render_lr(),
            _ => self.render_tb(),
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

    #[allow(
        clippy::too_many_lines,
        clippy::single_match_else,
        reason = "linear layout pass; rank dispatch reads clearest as if/else-if"
    )]
    fn render_tb(&self) -> String {
        let mut nodes = self.prepare_nodes();
        let idx = self.index_of();
        let layers = self.assign_ranks(&mut nodes, &idx);

        let vertical_gap = 4;
        let horizontal_gap = 4;

        let start_y = if self.spec.title.is_some() { 2 } else { 0 };
        let mut current_y = start_y;

        // Position nodes in each layer
        for layer in &layers {
            let mut layer_max_h = 0;
            let mut current_x = 0;

            for &i in layer {
                let node = &mut nodes[i];
                node.x = current_x;
                node.y = current_y;
                current_x += node.width + horizontal_gap;
                if node.height > layer_max_h {
                    layer_max_h = node.height;
                }
            }

            current_y += layer_max_h + vertical_gap;
        }

        // Center layers horizontally relative to the widest layer
        let mut layer_widths = Vec::new();
        for layer in &layers {
            if let Some(&last_i) = layer.last() {
                let last = &nodes[last_i];
                layer_widths.push(last.x + last.width);
            } else {
                layer_widths.push(0);
            }
        }

        let max_w = *layer_widths.iter().max().unwrap_or(&20);
        let diagram_cx = max_w / 2;

        for (r, layer) in layers.iter().enumerate() {
            if layer.len() == 1 {
                let ni = layer[0];
                let w = nodes[ni].width;
                let ideal_x = diagram_cx.saturating_sub(w / 2);
                nodes[ni].x = ideal_x;
            } else {
                let lw = layer_widths[r];
                if lw < max_w {
                    let offset = (max_w - lw) / 2;
                    for &i in layer {
                        nodes[i].x += offset;
                    }
                }
            }
        }

        let mut canvas = Canvas::new(max_w + 10, current_y + 4);

        // Draw title if present
        if let Some(ref title) = self.spec.title {
            let title_w = UnicodeWidthStr::width(title.as_str());
            let title_x = if max_w > title_w {
                (max_w - title_w) / 2
            } else {
                0
            };
            canvas.draw_text(title_x, 0, title);
        }

        // Register node obstacles for collision detection
        for node in &nodes {
            canvas.add_obstacle(crate::canvas::Rect::new(
                node.x,
                node.y,
                node.width,
                node.height,
            ));
        }

        // Track outgoing and incoming edges
        let mut outgoing_counts: HashMap<String, usize> = HashMap::new();
        let mut incoming_counts: HashMap<String, usize> = HashMap::new();
        for edge in &self.spec.edges {
            *outgoing_counts.entry(edge.from.clone()).or_insert(0) += 1;
            *incoming_counts.entry(edge.to.clone()).or_insert(0) += 1;
        }

        let mut loop_track_x = max_w + 3;
        let mut multi_jump_track_x = max_w + 3;

        // Bend edges to the next rank share one band row (mid_y). When two
        // parents' horizontal spans overlap there, their trunk lines merge and
        // junction chars end up adjacent (`┴┬`). Detect overlapping spans and
        // stagger the second band down a row.
        let mut band_offsets: HashMap<String, usize> = HashMap::new();
        {
            let mut bands: Vec<(usize, usize, usize, String)> = Vec::new();
            for (ui, node_u) in nodes.iter().enumerate() {
                let u_bottom = node_u.y + node_u.height - 1;
                let u_cx = node_u.x + node_u.width / 2;
                let mut lo = u_cx;
                let mut hi = u_cx;
                let mut v_top = None;
                let mut has_bend = false;
                for (vi, node_v) in nodes.iter().enumerate() {
                    if vi == ui || node_v.rank != node_u.rank + 1 {
                        continue;
                    }
                    // Only spans actually connected by an edge
                    let connected = self
                        .spec
                        .edges
                        .iter()
                        .any(|e| {
                            idx.get(e.from.as_str()) == Some(&ui)
                                && idx.get(e.to.as_str()) == Some(&vi)
                        });
                    if !connected {
                        continue;
                    }
                    let cx = node_v.x + node_v.width / 2;
                    if cx != u_cx {
                        has_bend = true;
                    }
                    lo = lo.min(cx);
                    hi = hi.max(cx);
                    v_top = Some(node_v.y);
                }
                if has_bend && let Some(vt) = v_top {
                    let mid_y = u_bottom + (vt - u_bottom) / 2;
                    bands.push((mid_y, lo, hi, self.spec.nodes[ui].id.clone()));
                }
            }
            bands.sort_by_key(|(_, lo, hi, _)| (*lo, *hi));
            let mut placed: Vec<(usize, usize, usize)> = Vec::new();
            for (mid_y, lo, hi, id) in bands {
                let mut y = mid_y;
                while placed
                    .iter()
                    .any(|&(py, plo, phi)| y == py && lo <= phi && hi >= plo)
                {
                    y += 1;
                }
                band_offsets.insert(id, y - mid_y);
                placed.push((y, lo, hi));
            }
        }

        // Draw edges FIRST so boxes can render over or cleanly merge with them
        for edge in &self.spec.edges {
            if let (Some(&ui), Some(&vi)) = (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
            {
                let u = &nodes[ui];
                let v = &nodes[vi];
                if ui == vi {
                    self.draw_self_loop(&mut canvas, edge, u);
                    continue;
                }
                let u_cx = u.x + u.width / 2;
                let u_bottom = u.y + u.height - 1;
                let v_cx = v.x + v.width / 2;
                let v_top = v.y;

                if v.rank > u.rank {
                    // Forward edge
                    if v.rank == u.rank + 1 {
                        let is_branching =
                            outgoing_counts.get(&edge.from).copied().unwrap_or(0) > 1;
                        let is_merging = incoming_counts.get(&edge.to).copied().unwrap_or(0) > 1;
                        let is_aligned = u_cx == v_cx
                            || (u_cx.abs_diff(v_cx) <= 1
                                && u_cx > v.x + 1
                                && u_cx < v.x + v.width - 2
                                && v_cx > u.x + 1
                                && v_cx < u.x + u.width - 2);

                        if is_aligned {
                            let line_x = if is_merging && !is_branching {
                                v_cx
                            } else {
                                u_cx
                            };
                            // Straight line down
                            edge_vline(
                                &mut canvas,
                                edge,
                                line_x,
                                u_bottom + 1,
                                v_top - 1,
                                &self.theme,
                            );
                            edge_arrow_heads(
                                &mut canvas,
                                edge,
                                (line_x, v_top - 1, Direction::Down),
                                (line_x, u_bottom + 1, Direction::Up),
                                &self.theme,
                            );

                            if let Some(ref lbl) = edge.label {
                                let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                                let (label_x, label_y) = if is_branching {
                                    // Place on the left side of the vertical line so it doesn't collide with right branches
                                    (line_x.saturating_sub(lbl_w + 1), u_bottom + 1)
                                } else {
                                    (line_x + 2, usize::midpoint(u_bottom, v_top))
                                };
                                canvas.draw_text_safe(label_x, label_y, lbl);
                            }
                        } else {
                            // Orthogonal bend (Manhattan)
                            let mut mid_y = u_bottom + (v_top - u_bottom) / 2;
                            if let Some(&off) = band_offsets.get(&edge.from) {
                                mid_y = (mid_y + off).min(v_top.saturating_sub(1));
                            }
                            edge_vline(&mut canvas, edge, u_cx, u_bottom + 1, mid_y, &self.theme);
                            edge_hline(&mut canvas, edge, u_cx, v_cx, mid_y, &self.theme);
                            edge_vline(&mut canvas, edge, v_cx, mid_y, v_top - 1, &self.theme);
                            edge_arrow_heads(
                                &mut canvas,
                                edge,
                                (v_cx, v_top - 1, Direction::Down),
                                (u_cx, u_bottom + 1, Direction::Up),
                                &self.theme,
                            );

                            if let Some(ref lbl) = edge.label {
                                let label_w = UnicodeWidthStr::width(lbl.as_str());
                                let label_x = if v_cx > u_cx {
                                    u_cx + 2 + (v_cx - u_cx - 2).saturating_sub(label_w) / 2
                                } else {
                                    let ideal_x = v_cx
                                        + 2
                                        + (u_cx.saturating_sub(v_cx + 2)).saturating_sub(label_w)
                                            / 2;
                                    ideal_x.min(u_cx.saturating_sub(label_w + 1))
                                };
                                let label_y = if mid_y > u_bottom + 1 {
                                    mid_y - 1
                                } else {
                                    mid_y
                                }
                                .clamp(u_bottom + 1, v_top.saturating_sub(1));
                                canvas.draw_text_safe(label_x, label_y, lbl);
                            }
                        }
                    } else {
                        // Multi-rank jump: route around intermediate layers through gaps
                        let top_gap_y = u_bottom + 2;
                        let bottom_gap_y = v_top.saturating_sub(2);

                        let mut max_bound_x = u.x + u.width;
                        for layer in &layers[(u.rank + 1)..v.rank] {
                            for &ni in layer {
                                max_bound_x = max_bound_x.max(nodes[ni].x + nodes[ni].width);
                            }
                        }
                        let route_x = (max_bound_x + 3).max(multi_jump_track_x);
                        multi_jump_track_x = route_x + 4;

                        edge_vline(
                            &mut canvas,
                            edge,
                            u_cx,
                            u_bottom + 1,
                            top_gap_y,
                            &self.theme,
                        );
                        edge_hline(&mut canvas, edge, u_cx, route_x, top_gap_y, &self.theme);
                        edge_vline(
                            &mut canvas,
                            edge,
                            route_x,
                            top_gap_y,
                            bottom_gap_y,
                            &self.theme,
                        );
                        edge_hline(&mut canvas, edge, route_x, v_cx, bottom_gap_y, &self.theme);
                        edge_vline(
                            &mut canvas,
                            edge,
                            v_cx,
                            bottom_gap_y,
                            v_top - 1,
                            &self.theme,
                        );
                        edge_arrow_heads(
                            &mut canvas,
                            edge,
                            (v_cx, v_top - 1, Direction::Down),
                            (u_cx, u_bottom + 1, Direction::Up),
                            &self.theme,
                        );

                        if let Some(ref lbl) = edge.label {
                            let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                            let label_x = if route_x > u_cx {
                                // Center along the horizontal segment between u_cx and route_x
                                u_cx + 2 + (route_x - u_cx - 2).saturating_sub(lbl_w) / 2
                            } else {
                                route_x + 1 + (u_cx - route_x - 1).saturating_sub(lbl_w) / 2
                            };
                            canvas.draw_text_safe(label_x, top_gap_y.saturating_sub(1), lbl);
                        }
                    }
                } else if u.rank == v.rank {
                    // Side-by-side in same rank
                    if u.x < v.x {
                        let y = u.y + u.height / 2;
                        let start_x = u.x + u.width;
                        let end_x = v.x.saturating_sub(1);
                        edge_hline(&mut canvas, edge, start_x, end_x, y, &self.theme);
                        edge_arrow_heads(
                            &mut canvas,
                            edge,
                            (end_x, y, Direction::Right),
                            (start_x, y, Direction::Left),
                            &self.theme,
                        );
                        if let Some(ref lbl) = edge.label {
                            canvas.draw_text_safe(start_x + 1, y.saturating_sub(1), lbl);
                        }
                    } else {
                        let y = u.y + u.height / 2;
                        let start_x = v.x + v.width;
                        let end_x = u.x.saturating_sub(1);
                        edge_hline(&mut canvas, edge, start_x, end_x, y, &self.theme);
                        edge_arrow_heads(
                            &mut canvas,
                            edge,
                            (start_x, y, Direction::Left),
                            (end_x, y, Direction::Right),
                            &self.theme,
                        );
                        if let Some(ref lbl) = edge.label {
                            canvas.draw_text_safe(start_x + 1, y.saturating_sub(1), lbl);
                        }
                    }
                } else {
                    // Loop back edge
                    let loop_x = loop_track_x;
                    let u_center_y = u.y + u.height / 2;
                    let v_center_y = v.y + v.height / 2;

                    // Route around same-rank boxes blocking the direct path
                    let u_exit_y = clear_route_y(&canvas, u.x + u.width, loop_x, u_center_y, true);
                    let v_entry_y =
                        clear_route_y(&canvas, v.x + v.width + 1, loop_x, v_center_y, false);

                    edge_hline(
                        &mut canvas,
                        edge,
                        u.x + u.width,
                        loop_x,
                        u_exit_y,
                        &self.theme,
                    );
                    if u_exit_y != u_center_y {
                        edge_vline(
                            &mut canvas,
                            edge,
                            u.x + u.width,
                            u_center_y,
                            u_exit_y,
                            &self.theme,
                        );
                    }
                    edge_vline(
                        &mut canvas,
                        edge,
                        loop_x,
                        v_entry_y.min(v_center_y),
                        u_exit_y,
                        &self.theme,
                    );
                    if v_entry_y != v_center_y {
                        edge_vline(
                            &mut canvas,
                            edge,
                            v.x + v.width + 1,
                            v_entry_y,
                            v_center_y,
                            &self.theme,
                        );
                    }
                    edge_hline(
                        &mut canvas,
                        edge,
                        v.x + v.width + 1,
                        loop_x,
                        v_entry_y,
                        &self.theme,
                    );
                    edge_arrow_heads(
                        &mut canvas,
                        edge,
                        (v.x + v.width, v_center_y, Direction::Left),
                        (u.x + u.width, u_center_y, Direction::Right),
                        &self.theme,
                    );

                    if let Some(ref lbl) = edge.label {
                        let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                        canvas.draw_text_safe(
                            loop_x + 1,
                            usize::midpoint(u_center_y, v_center_y),
                            lbl,
                        );
                        loop_track_x += lbl_w + 3;
                    } else {
                        loop_track_x += 4;
                    }
                }
            }
        }

        // Draw nodes
        for node in &nodes {
            self.draw_node(&mut canvas, node);
        }

        canvas.render(&self.theme)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "linear layout pass; splitting would thread a wide context"
    )]
    fn render_lr(&self) -> String {
        let mut nodes = self.prepare_nodes();
        let idx = self.index_of();
        let layers = self.assign_ranks(&mut nodes, &idx);

        let vertical_gap = 2;
        let start_y = if self.spec.title.is_some() { 2 } else { 0 };
        let mut current_x = 0;

        // Position nodes along X across layers using tailored gaps between consecutive layers
        for (r, layer) in layers.iter().enumerate() {
            let mut layer_max_w = 0;
            let mut current_y = start_y;

            for &i in layer {
                let node = &mut nodes[i];
                node.x = current_x;
                node.y = current_y;
                current_y += node.height + vertical_gap;
                if node.width > layer_max_w {
                    layer_max_w = node.width;
                }
            }

            // Tailored gap to the next layer based on edge labels between layer r and r + 1
            let mut max_lbl_len = 0;
            if r + 1 < layers.len() {
                for edge in &self.spec.edges {
                    if let (Some(&ui), Some(&vi)) =
                        (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
                    {
                        let u = &nodes[ui];
                        let v = &nodes[vi];
                        if u.rank == r
                            && v.rank == r + 1
                            && let Some(ref lbl) = edge.label
                        {
                            max_lbl_len = max_lbl_len.max(UnicodeWidthStr::width(lbl.as_str()));
                        }
                    }
                }
            }
            let gap = if max_lbl_len > 0 {
                (max_lbl_len + 4).max(6)
            } else {
                5
            };

            current_x += layer_max_w + gap;
        }

        // Vertically center nodes in each layer relative to tallest layer
        let mut layer_heights = Vec::new();
        for layer in &layers {
            if let Some(&last_i) = layer.last() {
                let last = &nodes[last_i];
                layer_heights.push(last.y + last.height);
            } else {
                layer_heights.push(0);
            }
        }

        let max_h = *layer_heights.iter().max().unwrap_or(&10);
        let diagram_cy = max_h / 2;

        for (r, layer) in layers.iter().enumerate() {
            if layer.len() == 1 {
                let ni = layer[0];
                let h = nodes[ni].height;
                let ideal_y = diagram_cy.saturating_sub(h / 2);
                nodes[ni].y = ideal_y;
            } else {
                let lh = layer_heights[r];
                if lh < max_h {
                    let offset = (max_h - lh) / 2;
                    for &i in layer {
                        nodes[i].y += offset;
                    }
                }
            }
        }

        let mut canvas = Canvas::new(current_x + 6, max_h + 4);

        if let Some(ref title) = self.spec.title {
            canvas.draw_text(0, 0, title);
        }

        // Register node obstacles for collision detection
        for node in &nodes {
            canvas.add_obstacle(crate::canvas::Rect::new(
                node.x,
                node.y,
                node.width,
                node.height,
            ));
        }

        let mut loop_track_y = max_h + 2;

        // Draw edges
        for edge in &self.spec.edges {
            if let (Some(&ui), Some(&vi)) = (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
            {
                let u = &nodes[ui];
                let v = &nodes[vi];
                if ui == vi {
                    self.draw_self_loop(&mut canvas, edge, u);
                    continue;
                }
                let u_right = u.x + u.width - 1;
                let u_cy = u.y + u.height / 2;
                let v_left = v.x;
                let v_cy = v.y + v.height / 2;

                if v.rank > u.rank {
                    if u_cy == v_cy {
                        // Straight horizontal line
                        edge_hline(
                            &mut canvas,
                            edge,
                            u_right + 1,
                            v_left - 1,
                            u_cy,
                            &self.theme,
                        );
                        edge_arrow_heads(
                            &mut canvas,
                            edge,
                            (v_left - 1, u_cy, Direction::Right),
                            (u_right + 1, u_cy, Direction::Left),
                            &self.theme,
                        );

                        if let Some(ref lbl) = edge.label {
                            let label_w = UnicodeWidthStr::width(lbl.as_str());
                            let mid = (u_right + 1 + v_left - 1) / 2;
                            let label_x = if mid > label_w / 2 {
                                mid - label_w / 2
                            } else {
                                u_right + 2
                            };
                            let label_y = u_cy.saturating_sub(1);
                            canvas.draw_text_safe(label_x, label_y, lbl);
                        }
                    } else {
                        // Orthogonal bend in the corridor between layers
                        let u_layer_right = layers[u.rank]
                            .iter()
                            .map(|&ni| nodes[ni].x + nodes[ni].width - 1)
                            .max()
                            .unwrap_or(u_right);
                        let mid_x = (u_layer_right + (v_left.saturating_sub(u_layer_right)) / 2)
                            .max(u_right + 1);
                        edge_hline(&mut canvas, edge, u_right + 1, mid_x, u_cy, &self.theme);
                        edge_vline(&mut canvas, edge, mid_x, u_cy, v_cy, &self.theme);
                        edge_hline(&mut canvas, edge, mid_x, v_left - 1, v_cy, &self.theme);
                        edge_arrow_heads(
                            &mut canvas,
                            edge,
                            (v_left - 1, v_cy, Direction::Right),
                            (u_right + 1, u_cy, Direction::Left),
                            &self.theme,
                        );

                        if let Some(ref lbl) = edge.label {
                            let label_w = UnicodeWidthStr::width(lbl.as_str());
                            let label_x = if mid_x > label_w / 2 {
                                mid_x - label_w / 2
                            } else {
                                mid_x + 1
                            };
                            let label_y = if v_cy > 0 { v_cy - 1 } else { v_cy };
                            canvas.draw_text_safe(label_x, label_y, lbl);
                        }
                    }
                } else {
                    // Back-edge loop
                    let loop_y = loop_track_y;
                    edge_vline(&mut canvas, edge, u_right, u_cy + 1, loop_y, &self.theme);
                    edge_hline(&mut canvas, edge, v_left, u_right, loop_y, &self.theme);
                    edge_vline(&mut canvas, edge, v_left, v_cy + 1, loop_y, &self.theme);
                    edge_arrow_heads(
                        &mut canvas,
                        edge,
                        (v_left, v_cy + 1, Direction::Up),
                        (u_right, u_cy + 1, Direction::Down),
                        &self.theme,
                    );

                    if let Some(ref lbl) = edge.label {
                        let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                        let mid_x = usize::midpoint(v_left, u_right);
                        let lbl_x = if mid_x > lbl_w / 2 {
                            mid_x - lbl_w / 2
                        } else {
                            v_left + 1
                        };
                        canvas.draw_text_safe(lbl_x, loop_y + 1, lbl);
                        loop_track_y += 3;
                    } else {
                        loop_track_y += 2;
                    }
                }
            }
        }

        // Draw nodes
        for node in &nodes {
            self.draw_node(&mut canvas, node);
        }

        canvas.render(&self.theme)
    }

    /// Draws a self-referencing edge (`A --> A`) as a rectangular arc off the
    /// right wall of the box, re-entering one row lower.
    fn draw_self_loop(&self, canvas: &mut Canvas, edge: &EdgeSpec, u: &LayoutNode) {
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
        match node.shape {
            NodeShape::Diamond => {
                canvas.draw_decision_box(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    &self.theme,
                    Some("◇"),
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
                canvas.draw_box(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    &circle_theme,
                    Some(badge),
                );
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
                canvas.draw_box(node.x, node.y, node.width, node.height, &sub_theme, None);
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
                canvas.draw_box(node.x, node.y, node.width, node.height, &self.theme, None);
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
                canvas.draw_box(node.x, node.y, node.width, node.height, &box_theme, None);
                if !is_ascii {
                    let right = node.x + node.width - 1;
                    let bottom = node.y + node.height - 1;
                    canvas.put_char(node.x, node.y, '┌');
                    canvas.put_char(right, node.y, '┐');
                    canvas.put_char(node.x, bottom, '└');
                    canvas.put_char(right, bottom, '┘');
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
                canvas.draw_box(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    &rounded_theme,
                    None,
                );
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn test_simple_flowchart_tb() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Client".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Server".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: Some("HTTP".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
            }],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let result = renderer.render();
        assert!(result.contains("Client"));
        assert!(result.contains("Server"));
        assert!(result.contains("HTTP"));
        assert!(result.contains("▼"));
    }

    #[test]
    fn test_simple_flowchart_lr() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::LR,
            style: BoxStyle::Ascii,
            title: None,
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
            }],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::ascii());
        let result = renderer.render();
        assert!(result.contains("Start"));
        assert!(result.contains("End"));
        assert!(result.contains('>'));
    }

    #[test]
    fn test_diamond_decision_flowchart() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Is Valid?".to_string(),
                    shape: NodeShape::Diamond,
                },
                NodeSpec {
                    id: "C".to_string(),
                    label: "Proceed".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    label: Some("Yes".to_string()),
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
            ],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let result = renderer.render();
        assert!(result.contains("Is Valid?"));
        assert!(result.contains("◇"));
        assert!(result.contains("╔"));
        assert!(result.contains("║"));
        assert!(result.contains("Proceed"));
    }

    #[test]
    fn test_feedback_loop_ranking_and_rendering() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            nodes: vec![
                NodeSpec {
                    id: "Ref".to_string(),
                    label: "Target".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "Sum".to_string(),
                    label: "Error".to_string(),
                    shape: NodeShape::Circle,
                },
                NodeSpec {
                    id: "PID".to_string(),
                    label: "Controller".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "Plant".to_string(),
                    label: "Motor".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "Ref".to_string(),
                    to: "Sum".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "Sum".to_string(),
                    to: "PID".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "PID".to_string(),
                    to: "Plant".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "Plant".to_string(),
                    to: "Sum".to_string(),
                    label: Some("Feedback".to_string()),
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
            ],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let mut nodes = renderer.prepare_nodes();
        let idx = renderer.index_of();
        let layers = renderer.assign_ranks(&mut nodes, &idx);

        // Verify ranks are ordered: Ref (0) -> Sum (1) -> PID (2) -> Plant (3)
        assert_eq!(nodes[idx["Ref"]].rank, 0);
        assert_eq!(nodes[idx["Sum"]].rank, 1);
        assert_eq!(nodes[idx["PID"]].rank, 2);
        assert_eq!(nodes[idx["Plant"]].rank, 3);
        assert_eq!(layers.len(), 4);

        let result = renderer.render();
        assert!(result.contains("Target"));
        assert!(result.contains("Error"));
        assert!(result.contains("Controller"));
        assert!(result.contains("Motor"));
        assert!(result.contains("Feedback"));
    }
}

#[cfg(test)]
mod self_loop_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::Theme;

    fn spec(direction: LayoutDirection) -> FlowchartSpec {
        FlowchartSpec {
            direction,
            style: BoxStyle::Rounded,
            title: None,
            nodes: vec![NodeSpec {
                id: "A".to_string(),
                label: "Box".to_string(),
                shape: NodeShape::Box,
            }],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "A".to_string(),
                label: Some("retry".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
            }],
        }
    }

    #[test]
    fn test_self_loop_tb() {
        let spec = spec(LayoutDirection::TB);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render();
        assert!(out.contains("Box"));
        assert!(out.contains("retry"), "self-loop label must render");
        assert!(out.contains('◄'), "re-entry arrowhead must render");
    }

    #[test]
    fn test_self_loop_lr() {
        let spec = spec(LayoutDirection::LR);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render();
        assert!(out.contains("Box"));
        assert!(out.contains("retry"));
        assert!(out.contains('◄'));
    }

    /// Regression: `A --> A` at x=0 used to panic with `attempt to subtract
    /// with overflow` in the same-rank branch.
    #[test]
    fn test_self_loop_at_origin_no_panic() {
        let spec = spec(LayoutDirection::TB);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render();
        assert!(out.contains("Box"));
    }
}

use super::edges::{clear_route_y, edge_arrow_heads, edge_hline, edge_vline};
use super::{BlockMode, Blocks, FlowchartRenderer, PendingLabel};
use crate::canvas::{Canvas, Direction, display_width};
use crate::schema::SubgraphSpec;
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

/// Smallest node rank inside `sg` (recursing into non-moved child
/// subgraphs); `None` when the subgraph has no ranked members. Moved child
/// blocks render elsewhere — their phantom coords would inflate the rank
/// set, mirroring `group_rect`'s skip.
fn subgraph_min_rank(
    sg: &SubgraphSpec,
    nodes: &[super::LayoutNode],
    idx: &HashMap<&str, usize>,
    blocks: &Blocks,
) -> Option<usize> {
    let mut r = usize::MAX;
    for id in &sg.nodes {
        if let Some(&i) = idx.get(id.as_str()) {
            r = r.min(nodes[i].rank);
        }
    }
    for child in &sg.subgraphs {
        if blocks.rect_for(&child.id).is_some() {
            continue;
        }
        if let Some(cr) = subgraph_min_rank(child, nodes, idx, blocks) {
            r = r.min(cr);
        }
    }
    (r != usize::MAX).then_some(r)
}

/// Widens bands crossed by a nested group's border chains (FC-SUB-02/04): a
/// member at nesting depth `d` inside `sg` carries `2*d` rows of title
/// borders above its box and `d` rows of bottom borders below it. Keep the
/// band's midpoint trunk above the complete incoming title-border chain.
fn bump_band_chains(
    sg: &SubgraphSpec,
    nodes: &[super::LayoutNode],
    idx: &HashMap<&str, usize>,
    blocks: &Blocks,
    band_gap: &mut [usize],
    depth: usize,
) {
    for id in &sg.nodes {
        if let Some(&i) = idx.get(id.as_str()) {
            let r = nodes[i].rank;
            if depth >= 2 {
                if r >= 1 {
                    band_gap[r - 1] = band_gap[r - 1].max(4 * depth + 2);
                }
                if r + 1 < band_gap.len() {
                    band_gap[r] = band_gap[r].max(depth + 2);
                }
            }
        }
    }
    for child in &sg.subgraphs {
        if blocks.rect_for(&child.id).is_some() {
            continue;
        }
        bump_band_chains(child, nodes, idx, blocks, band_gap, depth + 1);
    }
}

impl<'a> FlowchartRenderer<'a> {
    #[allow(
        clippy::too_many_lines,
        clippy::single_match_else,
        reason = "linear layout pass; rank dispatch reads clearest as if/else-if"
    )]
    /// Render a top-to-bottom flowchart. Body extracted verbatim from the
    /// pre-split monolith; see mod.rs for layout pre-passes.
    pub(super) fn render_tb(&self, colored: bool, blocks: &mut Blocks) -> String {
        let mut nodes = self.prepare_nodes(blocks);
        let idx = self.index_of();
        let layers = self.assign_ranks(&mut nodes, &idx);

        // Members of moved (isolated-direction) subgraphs are laid out inside
        // their own block render; keep empty layers so rank indexing stays aligned
        let mut layers: Vec<Vec<usize>> = layers
            .into_iter()
            .map(|l| {
                l.into_iter()
                    .filter(|i| !blocks.member_indices.contains(i))
                    .collect()
            })
            .collect();
        // Sugiyama crossing reduction within layers (dense graphs)
        self.reduce_crossings(&mut layers, &idx);

        let horizontal_gap = 4;

        // Per-band vertical gap: a plain single-edge unlabeled band compacts
        // to one │ row plus the ▼ row. Anything needing more room keeps it:
        // edge labels (a spare label row), multiple rank-adjacent edges
        // (bend/band-stagger rows), multi-rank jumps (top/bottom route runs
        // at `u_bottom + 2` / `v_top - 2`), and subgraph group boxes (title
        // border rows above their top members).
        let mut band_gap = vec![2usize; layers.len().max(1)];
        let mut band_edges = vec![0usize; layers.len().max(1)];
        for edge in &self.spec.edges {
            if let (Some(&ui), Some(&vi)) = (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
            {
                let (ur, vr) = (nodes[ui].rank, nodes[vi].rank);
                if vr > ur + 1 {
                    // Multi-rank jump: every band it crosses needs route room
                    let gap = if edge.label.is_some() { 4 } else { 3 };
                    for b in band_gap.iter_mut().skip(ur).take(vr - ur - 1) {
                        *b = (*b).max(gap);
                    }
                } else if vr > ur {
                    band_edges[ur] += 1;
                    if edge.label.is_some() {
                        band_gap[ur] = band_gap[ur].max(4);
                    }
                    if edge.thick || edge.dashed {
                        // Weighted/dashed strokes need a full trunk row to stay
                        // visible when the trunk jogs (vline → hline → vline);
                        // a 2-row band erases the vertical stroke entirely
                        band_gap[ur] = band_gap[ur].max(3);
                    }
                }
            }
        }
        // Two or more rank-adjacent edges in one band: orthogonal bends and
        // band staggering assume the roomy geometry (mid-band hline plus a
        // stagger row clear of the ▼ row)
        for (b, cnt) in band_edges.iter().enumerate() {
            if *cnt > 1 {
                band_gap[b] = band_gap[b].max(4);
            }
        }
        // Subgraph group boxes draw their title border `pad_top` rows above
        // the top member; that border must clear the band's trunk rows
        for sg in &self.spec.subgraphs {
            bump_band_chains(sg, &nodes, &idx, blocks, &mut band_gap, 1);
            if let Some(r) = subgraph_min_rank(sg, &nodes, &idx, blocks)
                && r >= 1
            {
                band_gap[r - 1] = band_gap[r - 1].max(6);
            }
        }

        let start_y = if self.spec.title.is_some() { 2 } else { 0 };
        let mut current_y = start_y;

        // Position nodes in each layer
        for (r, layer) in layers.iter().enumerate() {
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

            current_y += layer_max_h + band_gap.get(r).copied().unwrap_or(2);
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

        // Leave room for subgraph group padding at the diagram edge
        let sg_depth = self.max_subgraph_depth();
        let sg_margin = if sg_depth == 0 { 0 } else { 2 * sg_depth + 1 };
        if sg_margin > 0 {
            for node in &mut nodes {
                node.x += sg_margin;
                node.y += sg_margin;
            }
        }

        // Separate whole groups before any edge anchoring or block placement.
        // Every member rank moves together, preserving nested containment.
        self.separate_groups(&mut nodes, &idx, blocks, true, 2);
        let max_w = nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| !blocks.member_indices.contains(i))
            .map(|(_, node)| node.x + node.width)
            .max()
            .unwrap_or(20);

        // Paste isolated-direction subgraph blocks below the main graph.
        // AtPhantom clusters sit at their phantom node's laid-out position
        for b in &mut blocks.items {
            if b.mode == BlockMode::AtPhantom
                && let Some(&pi) = idx.get(b.phantom_id.as_str())
            {
                b.origin_x = nodes[pi].x;
                b.origin_y = nodes[pi].y;
            }
        }
        // Below-mode clusters stack below the main graph; origin_y leaves 2
        // rows above the content for the group-box title border drawn later
        // by draw_subgraphs.
        let mut cursor_y = current_y + 1 + sg_margin;
        for b in &mut blocks.items {
            if b.mode != BlockMode::Below {
                continue;
            }
            b.origin_x = 2;
            b.origin_y = cursor_y + 2;
            cursor_y = b.origin_y + b.height + 4;
        }
        let blocks_extent = blocks
            .items
            .iter()
            .filter(|b| b.mode == BlockMode::Below)
            .map(|b| b.origin_y + b.height + 4)
            .max()
            .unwrap_or(0);
        let groups = self.collect_group_rects(&nodes, &idx, blocks);
        let group_w = groups
            .iter()
            .map(|(r, _, _, _)| r.x + r.width)
            .max()
            .unwrap_or(0);
        let group_h = groups
            .iter()
            .map(|(r, _, _, _)| r.y + r.height)
            .max()
            .unwrap_or(0);
        let canvas_w = (max_w + 10 + sg_margin)
            .max(group_w + 2)
            .max(blocks.items.iter().map(|b| b.width + 6).max().unwrap_or(0));
        let canvas_h = (current_y + 4 + sg_margin)
            .max(blocks_extent)
            .max(group_h + 2);

        let mut canvas = Canvas::new(canvas_w, canvas_h);

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

        // Paste isolated-direction subgraph blocks (pre-composed in their own
        // orientation by the recursive render)
        for b in &blocks.items {
            for (dy, line) in b.lines.iter().enumerate() {
                canvas.draw_text(b.origin_x, b.origin_y + dy, line);
            }
        }

        // Register node obstacles for collision detection (moved blocks have
        // no external edges — their region needs no protection)
        for (i, node) in nodes.iter().enumerate() {
            if blocks.member_indices.contains(&i) {
                continue;
            }
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

        let mut loop_track_x = max_w + 3 + sg_margin;
        let mut multi_jump_track_x = max_w + 3 + sg_margin;
        // Left-corridor cursor: usize::MAX until the first left track claims it,
        // then subsequent left tracks stack leftward (floored at 0)
        let mut multi_jump_track_left_x = usize::MAX;

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
                    let connected = self.spec.edges.iter().any(|e| {
                        idx.get(e.from.as_str()) == Some(&ui) && idx.get(e.to.as_str()) == Some(&vi)
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

        // Group multi-rank jumps that share a source (watchdog feeds, debug
        // taps). Each group gets ONE shared vertical track and ONE top run
        // instead of N overlapping full-width horizontal runs.
        let mut jump_tracks: HashMap<String, (usize, usize)> = HashMap::new();
        let mut jump_led: HashSet<String> = HashSet::new();
        {
            let mut members_by_src: HashMap<String, Vec<usize>> = HashMap::new();
            for (ei, edge) in self.spec.edges.iter().enumerate() {
                if let (Some(&ui), Some(&vi)) =
                    (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
                    && nodes[vi].rank > nodes[ui].rank + 1
                {
                    members_by_src
                        .entry(edge.from.clone())
                        .or_default()
                        .push(ei);
                }
            }
            for (src, members) in members_by_src {
                if members.len() < 2 {
                    continue;
                }
                let Some(&sui) = idx.get(src.as_str()) else {
                    continue;
                };
                let su = &nodes[sui];
                let su_cx = su.x + su.width / 2;
                let mut max_bound_x = su.x + su.width;
                let mut min_bound_x = su.x;
                let mut depth_y = 0;
                let (mut left_targets, mut right_targets) = (0usize, 0usize);
                for &ei in &members {
                    let edge = &self.spec.edges[ei];
                    let Some(&vi) = idx.get(edge.to.as_str()) else {
                        continue;
                    };
                    let v = &nodes[vi];
                    // Corridor side by target majority: fewer/shorter
                    // horizontal runs and fewer band crossings
                    if v.x + v.width / 2 < su_cx {
                        left_targets += 1;
                    } else {
                        right_targets += 1;
                    }
                    for layer in &layers[(su.rank + 1)..v.rank] {
                        for &ni in layer {
                            max_bound_x = max_bound_x.max(nodes[ni].x + nodes[ni].width);
                            min_bound_x = min_bound_x.min(nodes[ni].x);
                        }
                    }
                    depth_y = depth_y.max(v.y.saturating_sub(2));
                }
                let track = if left_targets > right_targets {
                    // Left corridor: clear intermediate nodes' left edges;
                    // multiple left sources stack leftward, floored at 0
                    let candidate = if min_bound_x == su.x {
                        su.x.saturating_sub(3)
                    } else {
                        min_bound_x.saturating_sub(3)
                    };
                    let t = candidate.min(multi_jump_track_left_x.saturating_sub(4));
                    multi_jump_track_left_x = t;
                    t
                } else {
                    let track = (max_bound_x + 3).max(multi_jump_track_x);
                    multi_jump_track_x = track + 4;
                    track
                };
                jump_tracks.insert(src, (track, depth_y));
            }
        }

        let mut labels = Vec::new();
        // Draw edges FIRST so boxes can render over or cleanly merge with them
        // (edges between members of moved blocks are baked into those blocks)
        for edge in &self.spec.edges {
            if let (Some(&ui), Some(&vi)) = (idx.get(edge.from.as_str()), idx.get(edge.to.as_str()))
            {
                if blocks.member_indices.contains(&ui) || blocks.member_indices.contains(&vi) {
                    continue;
                }
                let u = &nodes[ui];
                let v = &nodes[vi];
                if ui == vi {
                    self.draw_self_loop(&mut canvas, edge, u, &mut labels);
                    continue;
                }
                let u_cx = u.x + u.width / 2;
                let u_bottom = u.y + u.height - 1;
                let v_cx = v.x + v.width / 2;
                let v_top = v.y;
                let lines = self.edge_label_lines(edge);
                let label_width = lines
                    .iter()
                    .map(|line| display_width(line))
                    .max()
                    .unwrap_or(0);

                if v.rank > u.rank {
                    // Forward edge
                    if v.rank == u.rank + 1 {
                        let is_branching =
                            outgoing_counts.get(&edge.from).copied().unwrap_or(0) > 1;
                        let is_aligned = u_cx == v_cx
                            || (u_cx.abs_diff(v_cx) <= 1
                                && u_cx > v.x + 1
                                && u_cx < v.x + v.width - 2
                                && v_cx > u.x + 1
                                && v_cx < u.x + u.width - 2);

                        if is_aligned {
                            // Draw at the target's center column so every
                            // edge entering the same target converges on one
                            // arrowhead column. is_aligned guarantees v_cx is
                            // within both boxes' spans (± 1 near-align), so
                            // the line still touches the source bottom.
                            let line_x = v_cx;
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

                            if !lines.is_empty() {
                                let lbl_w = label_width;
                                let (label_x, label_y) = if is_branching {
                                    // Place on the left side of the vertical line so it doesn't collide with right branches
                                    (line_x.saturating_sub(lbl_w + 1), u_bottom + 1)
                                } else {
                                    (line_x + 2, usize::midpoint(u_bottom, v_top))
                                };
                                labels.push(PendingLabel::left(lines, label_x, label_y));
                            }
                        } else {
                            // Orthogonal bend (Manhattan)
                            let mut mid_y = u_bottom + (v_top - u_bottom) / 2;
                            if let Some(&off) = band_offsets.get(&edge.from) {
                                mid_y = (mid_y + off).min(v_top.saturating_sub(1));
                            }
                            // Reuse a nearby arrowhead column so a target fed
                            // from two sides converges into one arrowhead
                            let drop_x = self.drop_x_for(&canvas, v);
                            edge_vline(&mut canvas, edge, u_cx, u_bottom + 1, mid_y, &self.theme);
                            edge_hline(&mut canvas, edge, u_cx, drop_x, mid_y, &self.theme);
                            edge_vline(&mut canvas, edge, drop_x, mid_y, v_top - 1, &self.theme);
                            edge_arrow_heads(
                                &mut canvas,
                                edge,
                                (drop_x, v_top - 1, Direction::Down),
                                (u_cx, u_bottom + 1, Direction::Up),
                                &self.theme,
                            );

                            if !lines.is_empty() {
                                let label_w = label_width;
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
                                labels.push(PendingLabel::left(lines, label_x, label_y));
                            }
                        }
                    } else {
                        // Multi-rank jump: route around intermediate layers through gaps.
                        // When several jumps share a source, the lead edge draws one
                        // shared track + top run; members only add their drop.
                        let top_gap_y = u_bottom + 2;
                        let bottom_gap_y = v_top.saturating_sub(2);
                        if let Some(&(track_x, depth_y)) = jump_tracks.get(edge.from.as_str()) {
                            let lead = jump_led.insert(edge.from.clone());
                            if lead {
                                edge_vline(
                                    &mut canvas,
                                    edge,
                                    u_cx,
                                    u_bottom + 1,
                                    top_gap_y,
                                    &self.theme,
                                );
                                edge_hline(
                                    &mut canvas,
                                    edge,
                                    u_cx,
                                    track_x,
                                    top_gap_y,
                                    &self.theme,
                                );
                                edge_vline(
                                    &mut canvas,
                                    edge,
                                    track_x,
                                    top_gap_y,
                                    depth_y,
                                    &self.theme,
                                );
                                edge_arrow_heads(
                                    &mut canvas,
                                    edge,
                                    (v_cx, v_top - 1, Direction::Down),
                                    (u_cx, u_bottom + 1, Direction::Up),
                                    &self.theme,
                                );
                            }
                            // Per-target drop from the shared track — reuse a
                            // nearby arrowhead column when one already lands
                            // at the target top (dense supervisory feeds)
                            let drop_x = self.drop_x_for(&canvas, v);
                            edge_hline(
                                &mut canvas,
                                edge,
                                track_x,
                                drop_x,
                                bottom_gap_y,
                                &self.theme,
                            );
                            if bottom_gap_y < v_top - 1 {
                                edge_vline(
                                    &mut canvas,
                                    edge,
                                    drop_x,
                                    bottom_gap_y,
                                    v_top - 1,
                                    &self.theme,
                                );
                            }
                            edge_arrow_heads(
                                &mut canvas,
                                edge,
                                (drop_x, v_top - 1, Direction::Down),
                                (drop_x, bottom_gap_y, Direction::Up),
                                &self.theme,
                            );
                            if !lines.is_empty() {
                                let lbl_w = label_width;
                                let label_x = usize::midpoint(track_x, v_cx)
                                    .saturating_sub(lbl_w / 2)
                                    .max(v_cx.min(track_x) + 1);
                                labels.push(PendingLabel::left(
                                    lines,
                                    label_x,
                                    bottom_gap_y.saturating_sub(1),
                                ));
                            }
                        } else {
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

                            if !lines.is_empty() {
                                let lbl_w = label_width;
                                let label_x = if route_x > u_cx {
                                    // Center along the horizontal segment between u_cx and route_x
                                    u_cx + 2 + (route_x - u_cx - 2).saturating_sub(lbl_w) / 2
                                } else {
                                    route_x + 1 + (u_cx - route_x - 1).saturating_sub(lbl_w) / 2
                                };
                                labels.push(PendingLabel::left(
                                    lines,
                                    label_x,
                                    top_gap_y.saturating_sub(1),
                                ));
                            }
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
                        if !lines.is_empty() {
                            labels.push(PendingLabel::left(
                                lines,
                                start_x + 1,
                                y.saturating_sub(1),
                            ));
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
                        if !lines.is_empty() {
                            labels.push(PendingLabel::left(
                                lines,
                                start_x + 1,
                                y.saturating_sub(1),
                            ));
                        }
                    }
                } else {
                    // Loop back edge
                    let loop_x = loop_track_x;
                    let u_center_y = u.y + u.height / 2;
                    let v_center_y = v.y + v.height / 2;
                    let u_bottom = u.y + u.height - 1;

                    // Route around same-rank boxes blocking the direct path
                    let u_exit_y = clear_route_y(&canvas, u.x + u.width, loop_x, u_center_y, true);
                    let v_entry_y =
                        clear_route_y(&canvas, v.x + v.width + 1, loop_x, v_center_y, false);

                    if u_exit_y == u_center_y {
                        edge_hline(
                            &mut canvas,
                            edge,
                            u.x + u.width,
                            loop_x,
                            u_exit_y,
                            &self.theme,
                        );
                    } else {
                        // Blocked at the exit row: leave through the source's
                        // bottom border (perpendicular feed, like any rank
                        // edge) and run the detour BELOW the blockers. A side
                        // drop would hug the border (`│ Box │┆`).
                        let stub_x = (u.x + u.width).saturating_sub(2).max(u.x.saturating_add(1));
                        edge_vline(
                            &mut canvas,
                            edge,
                            stub_x,
                            u_bottom + 1,
                            u_exit_y,
                            &self.theme,
                        );
                        edge_hline(&mut canvas, edge, stub_x, loop_x, u_exit_y, &self.theme);
                    }
                    edge_vline(
                        &mut canvas,
                        edge,
                        loop_x,
                        v_entry_y.min(v_center_y),
                        u_exit_y,
                        &self.theme,
                    );
                    if v_entry_y == v_center_y {
                        edge_hline(
                            &mut canvas,
                            edge,
                            v.x + v.width + 1,
                            loop_x,
                            v_entry_y,
                            &self.theme,
                        );
                    } else {
                        // Entry blocked above: drop into the target from a
                        // channel kept one clear cell off the target's right
                        // border, never flush against it.
                        let channel_x = v.x + v.width + 2;
                        edge_vline(
                            &mut canvas,
                            edge,
                            channel_x,
                            v_entry_y,
                            v_center_y,
                            &self.theme,
                        );
                        edge_hline(
                            &mut canvas,
                            edge,
                            v.x + v.width + 1,
                            channel_x,
                            v_center_y,
                            &self.theme,
                        );
                        edge_hline(&mut canvas, edge, channel_x, loop_x, v_entry_y, &self.theme);
                    }
                    edge_arrow_heads(
                        &mut canvas,
                        edge,
                        (v.x + v.width, v_center_y, Direction::Left),
                        (u.x + u.width, u_center_y, Direction::Right),
                        &self.theme,
                    );

                    if !lines.is_empty() {
                        labels.push(PendingLabel::left(
                            lines,
                            loop_x + 1,
                            usize::midpoint(u_center_y, v_center_y),
                        ));
                        loop_track_x += label_width + 3;
                    } else {
                        loop_track_x += 4;
                    }
                }
            }
        }

        // Draw nodes (moved blocks already contain their member boxes;
        // phantoms carry the pasted block instead of a box)
        for (i, node) in nodes.iter().enumerate() {
            if blocks.member_indices.contains(&i) || blocks.phantom_indices.contains(&i) {
                continue;
            }
            self.draw_node(&mut canvas, node);
        }

        // Draw subgraph grouping boxes on top of empty cells
        self.draw_subgraphs(&mut canvas, &nodes, &idx, blocks);

        for label in labels {
            label.draw(self, &mut canvas);
        }

        canvas.render_impl(&self.theme, colored)
    }
}

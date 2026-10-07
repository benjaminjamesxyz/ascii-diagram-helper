use super::Blocks;
use super::FlowchartRenderer;
use super::edges::{clear_route_y, edge_arrow_heads, edge_hline, edge_vline};
use crate::canvas::{Canvas, Direction};
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

impl<'a> FlowchartRenderer<'a> {
    #[allow(
        clippy::too_many_lines,
        clippy::single_match_else,
        reason = "linear layout pass; rank dispatch reads clearest as if/else-if"
    )]
    /// Render a top-to-bottom flowchart. Body extracted verbatim from the
    /// pre-split monolith; see mod.rs for layout pre-passes.
    pub(super) fn render_tb(&self, colored: bool, blocks: &mut Blocks) -> String {
        let mut nodes = self.prepare_nodes();
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

        // Leave room for subgraph group padding at the diagram edge
        let sg_margin = if self.spec.subgraphs.is_empty() { 0 } else { 3 };
        if sg_margin > 0 {
            for node in &mut nodes {
                node.x += sg_margin;
                node.y += sg_margin;
            }
        }

        // Paste isolated-direction subgraph blocks below the main graph.
        // origin_y leaves 2 rows above the content for the group-box title
        // border drawn later by draw_subgraphs.
        let mut cursor_y = current_y + 1;
        for b in &mut blocks.items {
            b.origin_x = 2;
            b.origin_y = cursor_y + 2;
            cursor_y = b.origin_y + b.height + 4;
        }
        let blocks_extent = if blocks.is_empty() { 0 } else { cursor_y };
        let canvas_w = (max_w + 10 + sg_margin)
            .max(blocks.items.iter().map(|b| b.width + 6).max().unwrap_or(0));
        let canvas_h = (current_y + 4 + sg_margin).max(blocks_extent);

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
                            // Reuse a nearby arrowhead column so a target fed
                            // from two sides converges into one arrowhead
                            let drop_x = self.drop_x_for(&canvas, v_cx, v_top - 1);
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
                            let drop_x = self.drop_x_for(&canvas, v_cx, v_top - 1);
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
                            if let Some(ref lbl) = edge.label {
                                let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                                let label_x = usize::midpoint(track_x, v_cx)
                                    .saturating_sub(lbl_w / 2)
                                    .max(v_cx.min(track_x) + 1);
                                canvas.draw_text_safe(label_x, bottom_gap_y.saturating_sub(1), lbl);
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

        // Draw nodes (moved blocks already contain their member boxes)
        for (i, node) in nodes.iter().enumerate() {
            if blocks.member_indices.contains(&i) {
                continue;
            }
            self.draw_node(&mut canvas, node);
        }

        // Draw subgraph grouping boxes on top of empty cells
        self.draw_subgraphs(&mut canvas, &nodes, &idx, blocks);

        canvas.render_impl(&self.theme, colored)
    }
}

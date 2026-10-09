use super::edges::{
    clear_column, detour_row, edge_arrow_heads, edge_hline, edge_vline, hspan_blocked,
};
use super::{BlockMode, Blocks, FlowchartRenderer, LayoutNode, PendingLabel};
use crate::canvas::{Canvas, Direction, Rect};
use unicode_width::UnicodeWidthStr;

/// Picks the back-edge attach column on `n`'s bottom border: the box's center
/// column when the descent to `below_y` is unobstructed, otherwise the
/// nearest inner column with a clear descent; falls back to the right border
/// column (legacy attach) when every inner column is blocked. Keeps clear of
/// the corner columns so loop-backs never destroy them (FC-LR-01/02).
fn attach_col(canvas: &Canvas, n: &LayoutNode, below_y: usize) -> usize {
    let left = n.x + 1;
    let right = n.x + n.width.saturating_sub(2);
    let center = n.x + n.width / 2;
    if left < right {
        let (ya, yb) = (n.y + n.height, below_y);
        for d in 0..=(right - left) {
            if center + d <= right && !super::edges::vspan_blocked(canvas, center + d, ya, yb) {
                return center + d;
            }
            if d > 0
                && center >= d
                && center - d >= left
                && !super::edges::vspan_blocked(canvas, center - d, ya, yb)
            {
                return center - d;
            }
        }
    }
    n.x + n.width - 1
}

impl<'a> FlowchartRenderer<'a> {
    #[allow(
        clippy::too_many_lines,
        reason = "linear layout pass; splitting would thread a wide context"
    )]
    pub(super) fn render_lr(&self, colored: bool, blocks: &mut Blocks) -> String {
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

        let vertical_gap = if self.max_subgraph_depth() >= 2 {
            // Nested group borders stack `depth` rows below the deepest
            // member; stacked layer-mates need to clear that chain
            self.max_subgraph_depth() + 1
        } else {
            2
        };
        let max_label_headroom = self
            .spec
            .edges
            .iter()
            .map(|e| self.edge_label_lines(e).len())
            .max()
            .unwrap_or(0);
        let start_y = if self.spec.title.is_some() {
            2 + max_label_headroom
        } else {
            max_label_headroom
        };
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

        // Containment pass (FC-SUB-01): push non-member nodes out of every
        // group box before the group padding margin is applied
        let geo = self.compute_group_geo(&nodes, &idx, blocks);
        self.push_nodes_out_of_groups(&mut nodes, &layers, &geo, false, vertical_gap);

        // Leave room for subgraph group padding at the diagram edge; nested
        // groups stack their title borders above members (2 rows per level)
        let sg_depth = self.max_subgraph_depth();
        let sg_margin = if sg_depth == 0 {
            0
        } else {
            3usize.max(2 * sg_depth + 1)
        };
        if sg_margin > 0 {
            for node in &mut nodes {
                node.x += sg_margin;
                node.y += sg_margin;
            }
        }

        // Extent after the push may exceed the pre-centering value; back-edge
        // loop tracks and the canvas floor must clear nested group borders
        let max_h = nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| !blocks.member_indices.contains(i))
            .map(|(_, n)| n.y + n.height)
            .max()
            .unwrap_or(10);

        // AtPhantom clusters sit at their phantom node's laid-out position
        for b in &mut blocks.items {
            if b.mode == BlockMode::AtPhantom
                && let Some(&pi) = idx.get(b.phantom_id.as_str())
            {
                b.origin_x = nodes[pi].x;
                b.origin_y = nodes[pi].y;
            }
        }
        // Below-mode clusters stack below the main graph
        let mut cursor_y = max_h + 3 + sg_margin;
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
        let canvas_w = (current_x + 6 + sg_margin)
            .max(blocks.items.iter().map(|b| b.width + 6).max().unwrap_or(0));
        let canvas_h = (max_h + 4 + sg_margin + sg_depth).max(blocks_extent);

        let mut canvas = Canvas::new(canvas_w, canvas_h);

        if let Some(ref title) = self.spec.title {
            canvas.draw_text(0, 0, title);
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
            // Phantoms DO register an obstacle (see tb.rs)
            canvas.add_obstacle(crate::canvas::Rect::new(
                node.x,
                node.y,
                node.width,
                node.height,
            ));
        }

        // Loop tracks start below the deepest content row; nested group
        // borders extend `sg_depth` rows further down — clear them too
        let mut loop_track_y = max_h + 2usize.max(sg_depth + 1) + sg_margin;

        let mut labels = Vec::new();
        // Draw edges (edges between members of moved blocks are baked into
        // those blocks)
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
                let u_right = u.x + u.width - 1;
                let u_cy = u.y + u.height / 2;
                let v_left = v.x;
                let v_cy = v.y + v.height / 2;

                if v.rank > u.rank {
                    let label_band = Rect::new(
                        u_right + 1,
                        0,
                        v_left.saturating_sub(u_right + 1),
                        usize::MAX,
                    );
                    if u_cy == v_cy {
                        let xs = u_right + 1;
                        let xe = v_left - 1;
                        // A same-row skip edge must not pierce an
                        // intermediate node: detour around the blocker
                        // (FC-LR-03 / FC-EDGE-03)
                        let detour = if xs < xe && hspan_blocked(&canvas, xs, xe, u_cy) {
                            detour_row(&canvas, xs, xe, u_cy)
                        } else {
                            None
                        };
                        if let Some(dy) = detour {
                            let jx = xs + 1;
                            let ex = xe - 1;
                            edge_hline(&mut canvas, edge, xs, jx, u_cy, &self.theme);
                            edge_vline(&mut canvas, edge, jx, dy, u_cy, &self.theme);
                            edge_hline(&mut canvas, edge, jx, ex, dy, &self.theme);
                            edge_vline(&mut canvas, edge, ex, dy, v_cy, &self.theme);
                            edge_hline(&mut canvas, edge, ex, xe, v_cy, &self.theme);
                            edge_arrow_heads(
                                &mut canvas,
                                edge,
                                (v_left - 1, u_cy, Direction::Right),
                                (u_right + 1, u_cy, Direction::Left),
                                &self.theme,
                            );

                            let lines = self.edge_label_lines(edge);
                            if !lines.is_empty() {
                                let mid = usize::midpoint(jx, ex);
                                let (label_y, up) = if dy > u_cy {
                                    (dy + 1, false)
                                } else {
                                    (dy.saturating_sub(1), true)
                                };
                                labels.push(
                                    PendingLabel::centered(lines, mid, label_y, up)
                                        .within(label_band),
                                );
                            }
                        } else {
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

                            let lines = self.edge_label_lines(edge);
                            if !lines.is_empty() {
                                let mid = usize::midpoint(u_right + 1, v_left - 1);
                                labels.push(
                                    PendingLabel::centered(lines, mid, u_cy - 1, true)
                                        .within(label_band),
                                );
                            }
                        }
                    } else {
                        // Orthogonal bend in the corridor between layers
                        let u_layer_right = layers[u.rank]
                            .iter()
                            .map(|&ni| nodes[ni].x + nodes[ni].width - 1)
                            .max()
                            .unwrap_or(u_right);
                        let preferred_mid = (u_layer_right
                            + (v_left.saturating_sub(u_layer_right)) / 2)
                            .max(u_right + 1);
                        // Bend corridors must not cross intermediate boxes:
                        // scan for a clear column, else route beyond all
                        // intermediate content (FC-EDGE-03)
                        let mid_x = if v_left >= u_right + 3 {
                            let (ya, yb) = (u_cy.min(v_cy), u_cy.max(v_cy));
                            clear_column(&canvas, u_right + 2, v_left - 2, ya, yb, preferred_mid)
                                .unwrap_or_else(|| {
                                    let mut max_bound = u_layer_right;
                                    for (r, layer) in layers.iter().enumerate().skip(u.rank + 1) {
                                        if r > v.rank {
                                            break;
                                        }
                                        for &ni in layer {
                                            if ni != vi {
                                                max_bound =
                                                    max_bound.max(nodes[ni].x + nodes[ni].width);
                                            }
                                        }
                                    }
                                    max_bound + 3
                                })
                        } else {
                            preferred_mid
                        };
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

                        let lines = self.edge_label_lines(edge);
                        if !lines.is_empty() {
                            let label_y = if v_cy > 0 { v_cy - 1 } else { v_cy };
                            labels.push(
                                PendingLabel::centered(lines, mid_x, label_y, true)
                                    .within(label_band),
                            );
                        }
                    }
                } else {
                    // Back-edge loop: attach through the bottom borders at
                    // offset, non-corner columns so the corner glyphs survive
                    // (FC-LR-01/02, FC-EDGE-04)
                    let loop_y = loop_track_y;
                    let u_bottom = u.y + u.height - 1;
                    let v_bottom = v.y + v.height - 1;
                    let mut u_ax = attach_col(&canvas, u, loop_y);
                    let v_ax = attach_col(&canvas, v, loop_y);
                    // Keep the two attach columns distinct when the boxes
                    // stack in one column range (same-rank loop)
                    if u_ax == v_ax {
                        u_ax = (u_ax + 1).min(u.x + u.width.saturating_sub(2)).max(u.x + 1);
                    }
                    // Solid strokes merge into the border as a tee; dashed
                    // strokes start below the border (a dashed stamp would
                    // erase the border glyph instead of merging)
                    let u_start = if edge.dashed { u_bottom + 1 } else { u_bottom };
                    let v_start = if edge.dashed { v_bottom + 1 } else { v_bottom };
                    edge_vline(&mut canvas, edge, u_ax, u_start, loop_y, &self.theme);
                    edge_hline(&mut canvas, edge, v_ax, u_ax, loop_y, &self.theme);
                    edge_vline(&mut canvas, edge, v_ax, v_start, loop_y, &self.theme);
                    edge_arrow_heads(
                        &mut canvas,
                        edge,
                        (v_ax, v_bottom, Direction::Up),
                        (u_ax, u_bottom, Direction::Down),
                        &self.theme,
                    );

                    let lines = self.edge_label_lines(edge);
                    if !lines.is_empty() {
                        let mid_x = usize::midpoint(v_ax, u_ax);
                        labels.push(
                            PendingLabel::centered(lines, mid_x, loop_y + 1, false).within(
                                Rect::new(
                                    v_ax.min(u_ax),
                                    loop_y + 1,
                                    v_ax.abs_diff(u_ax) + 1,
                                    usize::MAX,
                                ),
                            ),
                        );
                        loop_track_y += 3;
                    } else {
                        loop_track_y += 2;
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

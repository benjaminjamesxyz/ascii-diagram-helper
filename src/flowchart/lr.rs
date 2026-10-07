use super::FlowchartRenderer;
use super::edges::{edge_arrow_heads, edge_hline, edge_vline};
use crate::canvas::{Canvas, Direction};
use unicode_width::UnicodeWidthStr;

impl<'a> FlowchartRenderer<'a> {
    #[allow(
        clippy::too_many_lines,
        reason = "linear layout pass; splitting would thread a wide context"
    )]
    pub(super) fn render_lr(&self, colored: bool) -> String {
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

        // Leave room for subgraph group padding at the diagram edge
        let sg_margin = if self.spec.subgraphs.is_empty() { 0 } else { 3 };
        if sg_margin > 0 {
            for node in &mut nodes {
                node.x += sg_margin;
                node.y += sg_margin;
            }
        }

        let mut canvas = Canvas::new(current_x + 6 + sg_margin, max_h + 4 + sg_margin);

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

        let mut loop_track_y = max_h + 2 + sg_margin;

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

        // Draw subgraph grouping boxes on top of empty cells
        self.draw_subgraphs(&mut canvas, &nodes, &idx);

        canvas.render_impl(&self.theme, colored)
    }
}

use crate::canvas::{Canvas, Direction};
use crate::schema::{SeqMessageType, SequenceSpec};
use crate::theme::{BoxStyle, Theme};
use unicode_width::UnicodeWidthStr;

pub struct SequenceRenderer<'a> {
    spec: &'a SequenceSpec,
    theme: Theme,
}

impl<'a> SequenceRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a SequenceSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[must_use]
    #[allow(
        clippy::too_many_lines,
        clippy::single_match_else,
        reason = "linear layout pass; direction dispatch reads clearest as if/else-if"
    )]
    pub fn render(&self, colored: bool) -> String {
        if self.spec.participants.is_empty() {
            return String::new();
        }

        let num_p = self.spec.participants.len();
        let mut p_labels: Vec<Vec<String>> = Vec::new();
        let mut p_widths = Vec::new();
        let mut max_lines = 1;

        for p in &self.spec.participants {
            let label = p.label.as_deref().unwrap_or(&p.id);
            let lines: Vec<String> = label.lines().map(|s| s.trim().to_string()).collect();
            let lines = if lines.is_empty() {
                vec![label.to_string()]
            } else {
                lines
            };
            let max_w = lines
                .iter()
                .map(|l| UnicodeWidthStr::width(l.as_str()))
                .max()
                .unwrap_or(4);
            let w = max_w + 4; // 2 padding + 2 borders
            max_lines = max_lines.max(lines.len());
            p_labels.push(lines);
            p_widths.push(w.max(8));
        }

        // Map participant ID to index
        let p_index: std::collections::HashMap<String, usize> = self
            .spec
            .participants
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();

        // Calculate minimum distance between adjacent participants based on messages
        let mut min_gap = vec![4; num_p.saturating_sub(1)];

        for msg in &self.spec.messages {
            if let (Some(&from_idx), Some(&to_idx)) = (p_index.get(&msg.from), p_index.get(&msg.to))
            {
                if from_idx != to_idx {
                    let (left, right) = if from_idx < to_idx {
                        (from_idx, to_idx)
                    } else {
                        (to_idx, from_idx)
                    };
                    let span = right - left;
                    let msg_w = UnicodeWidthStr::width(msg.label.as_str()) + 4;
                    // Distribute across the span
                    let per_gap = msg_w.div_ceil(span);
                    for gap in min_gap.iter_mut().take(right).skip(left) {
                        if per_gap > *gap {
                            *gap = per_gap;
                        }
                    }
                } else {
                    let lbl_w = UnicodeWidthStr::width(msg.label.as_str());
                    let needed_gap = 5 + 2 + lbl_w + 2;
                    if from_idx == num_p - 1 && from_idx > 0 {
                        if needed_gap > min_gap[from_idx - 1] {
                            min_gap[from_idx - 1] = needed_gap;
                        }
                    } else if from_idx < num_p.saturating_sub(1) && needed_gap > min_gap[from_idx] {
                        min_gap[from_idx] = needed_gap;
                    }
                }
            }
        }

        // Calculate center X coordinates for each participant lifeline
        let mut p_cx = Vec::new();
        let mut current_cx = p_widths[0] / 2;
        p_cx.push(current_cx);

        for i in 0..num_p - 1 {
            let half_curr = p_widths[i] / 2;
            let half_next = p_widths[i + 1] / 2;
            let min_box_dist = half_curr + half_next + 3;
            let needed = min_box_dist.max(min_gap[i]);
            current_cx += needed;
            p_cx.push(current_cx);
        }

        let start_y = if self.spec.title.is_some() { 2 } else { 0 };
        let box_h = max_lines + 2;
        let p_top_y = start_y;
        let p_bottom_y = p_top_y + box_h - 1;

        // Calculate vertical positions of messages, reserving rows for frame
        // borders and branch dividers so frames never collide with messages
        let n_msgs = self.spec.messages.len();
        let mut open_ids: Vec<Vec<usize>> = vec![Vec::new(); n_msgs + 1];
        let mut close_ids: Vec<Vec<usize>> = vec![Vec::new(); n_msgs + 1];
        let mut divider_ids: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n_msgs + 1];
        for (fi, f) in self.spec.frames.iter().enumerate() {
            let start = f.start_step.min(n_msgs);
            let end = f.end_step.clamp(start, n_msgs);
            open_ids[start].push(fi);
            close_ids[end].push(fi);
            for (bi, &bs) in f.branch_steps.iter().enumerate().skip(1) {
                divider_ids[bs.min(n_msgs)].push((fi, bi));
            }
        }

        let mut msg_y = Vec::new();
        let mut frame_tops = vec![0usize; self.spec.frames.len()];
        let mut frame_bottoms = vec![0usize; self.spec.frames.len()];
        let mut divider_ys: Vec<Vec<(usize, usize)>> = vec![Vec::new(); self.spec.frames.len()];
        let mut cur_y = p_bottom_y + 2;

        for i in 0..n_msgs {
            // Frames closing after the previous message
            for &fi in &close_ids[i] {
                frame_bottoms[fi] = cur_y;
                cur_y += 1;
            }
            // Frames opening at this message
            for &fi in &open_ids[i] {
                frame_tops[fi] = cur_y;
                cur_y += 1;
            }
            // Branch dividers (else / and) starting at this message
            for &(fi, bi) in &divider_ids[i] {
                divider_ys[fi].push((bi, cur_y));
                cur_y += 1;
            }
            let msg = &self.spec.messages[i];
            let is_self = msg.from == msg.to;
            msg_y.push(cur_y);
            if is_self {
                cur_y += 3; // loop takes extra row
            } else {
                cur_y += 2; // label row + line row
            }
        }
        // Frames closing at the very end
        for &fi in &close_ids[n_msgs] {
            frame_bottoms[fi] = cur_y;
            cur_y += 1;
        }

        let lifeline_end_y = cur_y + 1;
        let bottom_box_y = lifeline_end_y;
        let total_h = bottom_box_y + box_h + 1;
        let total_w =
            p_cx.last().copied().unwrap_or(20) + p_widths.last().copied().unwrap_or(8) / 2 + 4;

        let mut canvas = Canvas::new(total_w, total_h);

        // Draw title
        if let Some(ref title) = self.spec.title {
            let tw = UnicodeWidthStr::width(title.as_str());
            let tx = if total_w > tw { (total_w - tw) / 2 } else { 0 };
            canvas.draw_text(tx, 0, title);
        }

        // Draw lifelines (participant color)
        for (i, &cx) in p_cx.iter().enumerate() {
            canvas.set_pen(self.spec.participants[i].color);
            canvas.draw_vline(cx, p_bottom_y, bottom_box_y);
        }
        canvas.set_pen(None);

        // Draw control-flow frames (alt/opt/loop/par) behind messages
        if !self.spec.frames.is_empty() {
            let fx = p_cx[0].saturating_sub(p_widths[0] / 2);
            let fr = p_cx[num_p - 1] + p_widths[num_p - 1] / 2;
            let fw = fr.saturating_sub(fx) + 1;
            let is_ascii = self.theme.box_style == BoxStyle::Ascii;
            let (c_tl, c_tr, c_bl, c_br, c_tee_l, c_tee_r, _hz, vt) = if is_ascii {
                ('+', '+', '+', '+', '+', '+', '-', '|')
            } else {
                ('┌', '┐', '└', '┘', '├', '┤', '─', '│')
            };
            for (fi, frame) in self.spec.frames.iter().enumerate() {
                let top = frame_tops[fi];
                let bottom = frame_bottoms[fi];
                if bottom <= top || fw < 6 {
                    continue;
                }
                // Top border with label tab
                canvas.put_char(fx, top, c_tl);
                canvas.put_char(fx + fw - 1, top, c_tr);
                canvas.draw_hline(fx + 1, fx + fw - 2, top);
                let head = match frame.branches.first() {
                    Some(c) if c.is_empty() => frame.label.clone(),
                    Some(c) => format!("{} {}", frame.label, c),
                    None => frame.label.clone(),
                };
                let head_w = UnicodeWidthStr::width(head.as_str());
                if head_w + 4 < fw {
                    canvas.put_char(fx + 1, top, ' ');
                    canvas.draw_text(fx + 2, top, &head);
                    canvas.put_char(fx + 2 + head_w, top, ' ');
                }
                // Bottom border
                canvas.put_char(fx, bottom, c_bl);
                canvas.put_char(fx + fw - 1, bottom, c_br);
                canvas.draw_hline(fx + 1, fx + fw - 2, bottom);
                // Side borders
                for y in (top + 1)..bottom {
                    canvas.put_char(fx, y, vt);
                    canvas.put_char(fx + fw - 1, y, vt);
                }
                // Branch dividers
                for (bi, dy) in &divider_ys[fi] {
                    canvas.put_char(fx, *dy, c_tee_l);
                    canvas.put_char(fx + fw - 1, *dy, c_tee_r);
                    canvas.draw_hline(fx + 1, fx + fw - 2, *dy);
                    let dhead = format!(
                        "{} {}",
                        if frame.label == "par" { "and" } else { "else" },
                        frame.branches[*bi]
                    );
                    let dw = UnicodeWidthStr::width(dhead.as_str());
                    if dw + 4 < fw {
                        canvas.put_char(fx + 1, *dy, ' ');
                        canvas.draw_text(fx + 2, *dy, &dhead);
                        canvas.put_char(fx + 2 + dw, *dy, ' ');
                    }
                }
            }
        }

        // Draw top participant boxes
        for (i, &cx) in p_cx.iter().enumerate() {
            let w = p_widths[i];
            let x = cx - w / 2;
            canvas.set_pen(self.spec.participants[i].color);
            canvas.draw_box(x, p_top_y, w, box_h, &self.theme, None);
            canvas.set_pen(None);
            for (line_idx, line) in p_labels[i].iter().enumerate() {
                let lbl_w = UnicodeWidthStr::width(line.as_str());
                let lbl_x = x + (w - lbl_w) / 2;
                canvas.draw_text(lbl_x, p_top_y + 1 + line_idx, line);
            }
        }

        // Draw messages
        for (idx, msg) in self.spec.messages.iter().enumerate() {
            if let (Some(&from_idx), Some(&to_idx)) = (p_index.get(&msg.from), p_index.get(&msg.to))
            {
                let y = msg_y[idx];

                if from_idx == to_idx {
                    // Self message loop
                    let cx = p_cx[from_idx];
                    let loop_w = 5;
                    let is_last = from_idx == num_p - 1 && num_p > 1;

                    if is_last {
                        let lbl_w = UnicodeWidthStr::width(msg.label.as_str());
                        let loop_x = cx.saturating_sub(loop_w);
                        let text_x = loop_x.saturating_sub(lbl_w + 1);
                        canvas.draw_hline(loop_x, cx, y);
                        canvas.draw_vline(loop_x, y, y + 1);
                        canvas.draw_hline(loop_x, cx, y + 1);
                        canvas.draw_arrow(cx - 1, y + 1, Direction::Right, &self.theme);
                        canvas.draw_text(text_x, y, &msg.label);
                    } else {
                        canvas.draw_hline(cx, cx + loop_w, y);
                        canvas.draw_vline(cx + loop_w, y, y + 1);
                        canvas.draw_hline(cx, cx + loop_w, y + 1);
                        canvas.draw_arrow(cx + 1, y + 1, Direction::Left, &self.theme);
                        canvas.draw_text(cx + loop_w + 2, y, &msg.label);
                    }
                } else if from_idx < to_idx {
                    // Left to Right ->
                    let from_x = p_cx[from_idx];
                    let to_x = p_cx[to_idx];
                    let line_y = y + 1;

                    // Label above the line
                    let lbl_w = UnicodeWidthStr::width(msg.label.as_str());
                    let mid_x = from_x + (to_x - from_x).saturating_sub(lbl_w) / 2;
                    canvas.draw_text(mid_x, y, &msg.label);

                    // Line and arrow
                    if msg.message_type == SeqMessageType::Async {
                        canvas.draw_dashed_hline(from_x + 1, to_x - 1, line_y, &self.theme);
                    } else {
                        canvas.draw_hline(from_x, to_x - 1, line_y);
                    }
                    canvas.draw_arrow(to_x - 1, line_y, Direction::Right, &self.theme);

                    if msg.message_type == SeqMessageType::Bidirectional {
                        canvas.draw_arrow(from_x + 1, line_y, Direction::Left, &self.theme);
                    }
                } else {
                    // Right to Left <-
                    let from_x = p_cx[from_idx];
                    let to_x = p_cx[to_idx];
                    let line_y = y + 1;

                    let lbl_w = UnicodeWidthStr::width(msg.label.as_str());
                    let mid_x = to_x + (from_x - to_x).saturating_sub(lbl_w) / 2;
                    canvas.draw_text(mid_x, y, &msg.label);

                    if msg.message_type == SeqMessageType::Async {
                        canvas.draw_dashed_hline(to_x + 1, from_x - 1, line_y, &self.theme);
                    } else {
                        canvas.draw_hline(to_x + 1, from_x, line_y);
                    }
                    canvas.draw_arrow(to_x + 1, line_y, Direction::Left, &self.theme);

                    if msg.message_type == SeqMessageType::Bidirectional {
                        canvas.draw_arrow(from_x - 1, line_y, Direction::Right, &self.theme);
                    }
                }
            }
        }

        // Draw bottom participant boxes
        for (i, &cx) in p_cx.iter().enumerate() {
            let w = p_widths[i];
            let x = cx - w / 2;
            canvas.set_pen(self.spec.participants[i].color);
            canvas.draw_box(x, bottom_box_y, w, box_h, &self.theme, None);
            canvas.set_pen(None);
            for (line_idx, line) in p_labels[i].iter().enumerate() {
                let lbl_w = UnicodeWidthStr::width(line.as_str());
                let lbl_x = x + (w - lbl_w) / 2;
                canvas.draw_text(lbl_x, bottom_box_y + 1 + line_idx, line);
            }
        }

        canvas.render_impl(&self.theme, colored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn test_sequence_diagram() {
        let spec = SequenceSpec {
            style: BoxStyle::Rounded,
            title: Some("Authentication Flow".to_string()),
            participants: vec![
                ParticipantSpec {
                    id: "client".to_string(),
                    label: Some("Client".to_string()),
                    color: None,
                },
                ParticipantSpec {
                    id: "auth".to_string(),
                    label: Some("Auth Service".to_string()),
                    color: None,
                },
                ParticipantSpec {
                    id: "db".to_string(),
                    label: Some("Database".to_string()),
                    color: None,
                },
            ],
            messages: vec![
                SeqMessageSpec {
                    from: "client".to_string(),
                    to: "auth".to_string(),
                    label: "POST /login".to_string(),
                    message_type: SeqMessageType::Sync,
                },
                SeqMessageSpec {
                    from: "auth".to_string(),
                    to: "db".to_string(),
                    label: "Query user".to_string(),
                    message_type: SeqMessageType::Sync,
                },
                SeqMessageSpec {
                    from: "db".to_string(),
                    to: "auth".to_string(),
                    label: "User row".to_string(),
                    message_type: SeqMessageType::Reply,
                },
                SeqMessageSpec {
                    from: "auth".to_string(),
                    to: "client".to_string(),
                    label: "JWT Token".to_string(),
                    message_type: SeqMessageType::Reply,
                },
            ],
            notes: vec![],
            frames: vec![],
        };

        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("Authentication Flow"));
        assert!(out.contains("Client"));
        assert!(out.contains("Auth Service"));
        assert!(out.contains("POST /login"));
        assert!(out.contains("JWT Token"));
        assert!(out.contains("►"));
        assert!(out.contains("◄"));
    }

    #[test]
    fn test_multiline_participant_sequence_diagram() {
        let spec = SequenceSpec {
            style: BoxStyle::Rounded,
            title: None,
            participants: vec![
                ParticipantSpec {
                    id: "A".to_string(),
                    label: Some("Top\nHalf".to_string()),
                    color: None,
                },
                ParticipantSpec {
                    id: "B".to_string(),
                    label: Some("Bottom\nHalf".to_string()),
                    color: None,
                },
            ],
            messages: vec![SeqMessageSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: "dispatch".to_string(),
                message_type: SeqMessageType::Sync,
            }],
            notes: vec![],
            frames: vec![],
        };

        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("Top"));
        assert!(out.contains("Half"));
        assert!(out.contains("dispatch"));
        assert!(out.contains("►"));
    }

    #[test]
    fn test_sequence_alt_and_loop_frames() {
        let spec = SequenceSpec {
            style: BoxStyle::Rounded,
            title: None,
            participants: vec![
                ParticipantSpec {
                    id: "A".to_string(),
                    label: Some("Client".to_string()),
                    color: None,
                },
                ParticipantSpec {
                    id: "B".to_string(),
                    label: Some("Server".to_string()),
                    color: None,
                },
            ],
            messages: vec![
                SeqMessageSpec {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    label: "req".to_string(),
                    message_type: SeqMessageType::Sync,
                },
                SeqMessageSpec {
                    from: "B".to_string(),
                    to: "A".to_string(),
                    label: "ok".to_string(),
                    message_type: SeqMessageType::Reply,
                },
                SeqMessageSpec {
                    from: "B".to_string(),
                    to: "A".to_string(),
                    label: "err".to_string(),
                    message_type: SeqMessageType::Reply,
                },
            ],
            notes: vec![],
            frames: vec![SeqFrameSpec {
                label: "alt".to_string(),
                branches: vec!["valid".to_string(), "invalid".to_string()],
                branch_steps: vec![1, 2],
                start_step: 1,
                end_step: 3,
            }],
        };

        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("alt valid"));
        assert!(out.contains("else invalid"));
        assert!(out.contains('┌'));
        assert!(out.contains('├'));
        assert!(out.contains('└'));
    }
}

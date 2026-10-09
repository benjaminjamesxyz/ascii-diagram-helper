use crate::canvas::{Canvas, Direction, display_width};
use crate::schema::{SeqMessageType, SeqNotePosition, SequenceSpec};
use crate::theme::{BoxStyle, Theme};
use unicode_width::UnicodeWidthChar;

fn truncate_to_width(s: &str, max_w: usize) -> (&str, usize) {
    let mut cur_w = 0;
    let mut end_byte = 0;
    for (i, ch) in s.char_indices() {
        let cw = ch.width().unwrap_or(0);
        if cur_w + cw > max_w {
            break;
        }
        cur_w += cw;
        end_byte = i + ch.len_utf8();
    }
    (&s[..end_byte], cur_w)
}

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
                .map(|l| display_width(l.as_str()))
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
                    // SEQ-05: size gaps by the label that will actually draw —
                    // a blank label contributes nothing
                    let msg_w = display_width(msg.label.trim()) + 4;
                    // Distribute across the span
                    let per_gap = msg_w.div_ceil(span);
                    for gap in min_gap.iter_mut().take(right).skip(left) {
                        if per_gap > *gap {
                            *gap = per_gap;
                        }
                    }
                } else {
                    let lbl_w = display_width(msg.label.trim());
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

        // Notes anchored between message rows (clamped at_step -> note index)
        let mut note_at: Vec<Vec<usize>> = vec![Vec::new(); n_msgs + 1];
        for (ni, n) in self.spec.notes.iter().enumerate() {
            note_at[n.at_step.min(n_msgs)].push(ni);
        }

        let mut msg_y = Vec::new();
        let mut frame_tops = vec![0usize; self.spec.frames.len()];
        let mut frame_bottoms = vec![0usize; self.spec.frames.len()];
        let mut divider_ys: Vec<Vec<(usize, usize)>> = vec![Vec::new(); self.spec.frames.len()];
        let mut note_y = vec![0usize; self.spec.notes.len()];
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
            // Notes anchored at this step: top border + text + bottom border
            for &ni in &note_at[i] {
                note_y[ni] = cur_y;
                cur_y += 3;
            }
            let msg = &self.spec.messages[i];
            let is_self = msg.from == msg.to;
            let has_label = display_width(msg.label.trim()) > 0;
            msg_y.push(cur_y);
            if is_self {
                cur_y += 3; // loop takes extra row
            } else if has_label {
                cur_y += 2; // label row + line row
            } else {
                // SEQ-05: `A -> B:` (empty/blank label) draws the arrow only —
                // no blank label row above it
                cur_y += 1;
            }
        }
        // Trailing dividers (else/and after the last message) allocate above
        // their frame's bottom border — draining closes first would draw the
        // divider below the frame bottom.
        for &(fi, bi) in &divider_ids[n_msgs] {
            divider_ys[fi].push((bi, cur_y));
            cur_y += 1;
        }
        // Notes anchored after the last message
        for &ni in &note_at[n_msgs] {
            note_y[ni] = cur_y;
            cur_y += 3;
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
            let tw = display_width(title.as_str());
            let tx = if total_w > tw { (total_w - tw) / 2 } else { 0 };
            canvas.draw_text(tx, 0, title);
        }

        // Draw lifelines (participant color)
        for (i, &cx) in p_cx.iter().enumerate() {
            canvas.set_pen(self.spec.participants[i].color);
            canvas.draw_vline(cx, p_bottom_y, bottom_box_y);
        }
        canvas.set_pen(None);

        let is_ascii = self.theme.box_style == BoxStyle::Ascii;
        let (c_tl, c_tr, c_bl, c_br, c_tee_l, c_tee_r, _hz, vt) = if is_ascii {
            ('+', '+', '+', '+', '+', '+', '-', '|')
        } else {
            ('┌', '┐', '└', '┘', '├', '┤', '─', '│')
        };

        // Draw control-flow frames (alt/opt/loop/par) behind messages.
        // Draw in reverse commit order: inner frames sit at lower indices, so
        // painting outer frames first lets nested corners overwrite the outer
        // side borders (painter's algorithm restores correct nesting).
        if !self.spec.frames.is_empty() {
            let fx = p_cx[0].saturating_sub(p_widths[0] / 2);
            let fr = p_cx[num_p - 1] + p_widths[num_p - 1] / 2;
            let fw = fr.saturating_sub(fx) + 1;
            for (fi, frame) in self.spec.frames.iter().enumerate().rev() {
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
                let head_w = display_width(head.as_str());
                let avail_w = fw.saturating_sub(4);
                let (head_str, head_w) = if head_w <= avail_w {
                    (head.as_str(), head_w)
                } else {
                    truncate_to_width(&head, avail_w)
                };
                if head_w > 0 {
                    canvas.put_char(fx + 1, top, ' ');
                    canvas.draw_text(fx + 2, top, head_str);
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
                    let div_kw = match frame.label.as_str() {
                        "par" => "and",
                        "critical" => "option",
                        _ => "else",
                    };
                    let dhead = format!("{} {}", div_kw, frame.branches[*bi]);
                    let dw = display_width(dhead.as_str());
                    let (dhead_str, dw) = if dw <= avail_w {
                        (dhead.as_str(), dw)
                    } else {
                        truncate_to_width(&dhead, avail_w)
                    };
                    if dw > 0 {
                        canvas.put_char(fx + 1, *dy, ' ');
                        canvas.draw_text(fx + 2, *dy, dhead_str);
                        canvas.put_char(fx + 2 + dw, *dy, ' ');
                    }
                }
            }
        }

        // Draw notes (annotation boxes over or beside lifelines)
        let fx = p_cx[0].saturating_sub(p_widths[0] / 2);
        let fr = p_cx[num_p - 1] + p_widths[num_p - 1] / 2;
        for (ni, note) in self.spec.notes.iter().enumerate() {
            let idxs: Vec<usize> = note
                .over
                .iter()
                .filter_map(|id| p_index.get(id).copied())
                .collect();
            if idxs.is_empty() {
                continue;
            }
            let text_w = display_width(note.text.as_str());
            let y = note_y[ni];
            let span_lx = p_cx[0].saturating_sub(p_widths[0] / 2);
            let span_rx = p_cx[num_p - 1] + p_widths[num_p - 1] / 2;
            let (mut left_x, mut right_x) = match note.position {
                SeqNotePosition::Over => {
                    let min_i = idxs.iter().min().copied().unwrap_or(0);
                    let max_i = idxs.iter().max().copied().unwrap_or(0);
                    (
                        p_cx[min_i].saturating_sub(p_widths[min_i] / 2),
                        p_cx[max_i] + p_widths[max_i] / 2,
                    )
                }
                SeqNotePosition::RightOf => {
                    // Anchor beside the lifeline: left edge 2 right of cx
                    let lx = p_cx[idxs[0]] + 2;
                    let rx = lx + text_w + 3; // text + 4 padding/borders, inclusive
                    if rx < total_w {
                        (lx, rx)
                    } else if lx + text_w + 1 < total_w {
                        (lx, lx + text_w + 1) // shrink padding if tight
                    } else {
                        (span_lx, span_rx) // clamped: fall back to column span
                    }
                }
                SeqNotePosition::LeftOf => {
                    // Anchor beside the lifeline: right edge 2 left of cx
                    let rx = p_cx[idxs[0]].saturating_sub(2);
                    if rx + 1 >= text_w + 4 {
                        (rx + 1 - (text_w + 4), rx)
                    } else if rx + 1 >= text_w + 2 {
                        (rx + 1 - (text_w + 2), rx) // shrink padding if tight
                    } else {
                        (span_lx, span_rx) // clamped: fall back to column span
                    }
                }
            };
            if !self.spec.frames.is_empty() {
                let in_frame = self.spec.frames.iter().enumerate().any(|(fi, _)| {
                    let top = frame_tops[fi];
                    let bottom = frame_bottoms[fi];
                    bottom > top && y >= top && y + 2 <= bottom
                });
                if in_frame {
                    if left_x <= fx {
                        left_x = fx + 1;
                    }
                    if right_x >= fr {
                        right_x = fr.saturating_sub(1);
                    }
                }
            }
            if right_x <= left_x || right_x - left_x + 1 < 3 {
                continue;
            }
            let y_bot = y + 2;
            canvas.put_char(left_x, y, c_tl);
            canvas.put_char(right_x, y, c_tr);
            canvas.draw_hline(left_x + 1, right_x - 1, y);
            canvas.put_char(left_x, y_bot, c_bl);
            canvas.put_char(right_x, y_bot, c_br);
            canvas.draw_hline(left_x + 1, right_x - 1, y_bot);
            canvas.put_char(left_x, y + 1, vt);
            canvas.put_char(right_x, y + 1, vt);
            let tw = right_x - left_x + 1;
            let tx = left_x + tw.saturating_sub(text_w) / 2;
            canvas.draw_text(tx, y + 1, &note.text);
        }

        // Draw top participant boxes
        for (i, &cx) in p_cx.iter().enumerate() {
            let w = p_widths[i];
            let x = cx - w / 2;
            canvas.set_pen(self.spec.participants[i].color);
            canvas.draw_box(x, p_top_y, w, box_h, &self.theme, None);
            canvas.set_pen(None);
            for (line_idx, line) in p_labels[i].iter().enumerate() {
                let lbl_w = display_width(line.as_str());
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
                    let has_label = display_width(msg.label.trim()) > 0;

                    if is_last {
                        let lbl_w = display_width(msg.label.as_str());
                        let loop_x = cx.saturating_sub(loop_w);
                        let text_x = loop_x.saturating_sub(lbl_w + 1);
                        canvas.draw_hline(loop_x, cx, y);
                        canvas.draw_vline(loop_x, y, y + 1);
                        canvas.draw_hline(loop_x, cx, y + 1);
                        canvas.draw_arrow(cx - 1, y + 1, Direction::Right, &self.theme);
                        if has_label {
                            canvas.draw_text(text_x, y, &msg.label);
                        }
                    } else {
                        canvas.draw_hline(cx, cx + loop_w, y);
                        canvas.draw_vline(cx + loop_w, y, y + 1);
                        canvas.draw_hline(cx, cx + loop_w, y + 1);
                        canvas.draw_arrow(cx + 1, y + 1, Direction::Left, &self.theme);
                        if has_label {
                            canvas.draw_text(cx + loop_w + 2, y, &msg.label);
                        }
                    }
                } else if from_idx < to_idx {
                    // Left to Right ->
                    let from_x = p_cx[from_idx];
                    let to_x = p_cx[to_idx];
                    let has_label = display_width(msg.label.trim()) > 0;
                    let line_y = y + if has_label { 1 } else { 0 };

                    // Label above the line (SEQ-05: skipped entirely when empty)
                    if has_label {
                        let lbl_w = display_width(msg.label.as_str());
                        let mid_x = from_x + (to_x - from_x).saturating_sub(lbl_w) / 2;
                        canvas.draw_text(mid_x, y, &msg.label);
                    }

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
                    let has_label = display_width(msg.label.trim()) > 0;
                    let line_y = y + if has_label { 1 } else { 0 };

                    if has_label {
                        let lbl_w = display_width(msg.label.as_str());
                        let mid_x = to_x + (from_x - to_x).saturating_sub(lbl_w) / 2;
                        canvas.draw_text(mid_x, y, &msg.label);
                    }

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
                let lbl_w = display_width(line.as_str());
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

    #[test]
    fn test_sequence_note_render() {
        let dsl = r"
        sequenceDiagram
          A->>B: step1
          note over A,B: cross note
          A->>B: step2
          note right of B: right note
          note left of A: left note
          note over A: trailing note
        ";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };

        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);

        // (d) render shows text + box glyphs between message rows
        assert!(out.contains("cross note"));
        assert!(out.contains("right note"));
        assert!(out.contains("left note"));
        assert!(out.contains("trailing note"));

        let lines: Vec<&str> = out.lines().collect();
        let idx_step1 = lines.iter().position(|l| l.contains("step1")).unwrap();
        let idx_cross = lines.iter().position(|l| l.contains("cross note")).unwrap();
        let idx_step2 = lines.iter().position(|l| l.contains("step2")).unwrap();
        let idx_trailing = lines
            .iter()
            .position(|l| l.contains("trailing note"))
            .unwrap();

        assert!(idx_step1 < idx_cross, "step1 before cross note");
        assert!(idx_cross < idx_step2, "cross note before step2");
        assert!(idx_step2 < idx_trailing, "step2 before trailing note (f)");

        // (e) JSON spec notes render same as DSL
        let json_dsl = r#"
        {
          "type": "sequence",
          "participants": [{"id": "A"}, {"id": "B"}],
          "messages": [
            {"from": "A", "to": "B", "label": "step1"},
            {"from": "A", "to": "B", "label": "step2"}
          ],
          "notes": [
            {"over": ["A", "B"], "text": "cross note", "at_step": 1, "position": "Over"},
            {"over": ["B"], "text": "right note", "at_step": 2, "position": "RightOf"},
            {"over": ["A"], "text": "left note", "at_step": 2, "position": "LeftOf"},
            {"over": ["A"], "text": "trailing note", "at_step": 2, "position": "Over"}
          ]
        }
        "#;
        let json_spec = crate::parser::parse_dsl_or_json(json_dsl, BoxStyle::Rounded).unwrap();
        let json_out = crate::render_diagram(&json_spec);
        assert_eq!(out, json_out);
    }

    #[test]
    fn test_sequence_unclosed_frame_render() {
        // T-4 (e) unclosed-alt DSL renders header + bottom border
        let dsl = "sequenceDiagram\nalt pending\nA->>B: query";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("alt pending"));
        assert!(out.contains('┌'));
        assert!(out.contains('└'));
    }

    #[test]
    fn test_sequence_trailing_divider_render() {
        // T-5 (a) alt ok / msg / else bad / end -> render contains 'else bad',
        // divider row strictly between last message row and frame bottom
        let dsl = r"
        sequenceDiagram
          A->>B: init
          alt ok
            A->>B: success
          else bad
          end
        ";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("else bad"));

        let lines: Vec<&str> = out.lines().collect();
        let msg_line = lines.iter().position(|l| l.contains("success")).unwrap();
        let div_line = lines.iter().position(|l| l.contains("else bad")).unwrap();
        let bot_line = lines
            .iter()
            .rposition(|l| l.contains('└') && l.contains('┘'))
            .unwrap();

        assert!(msg_line < div_line, "message row above trailing divider");
        assert!(
            div_line < bot_line,
            "trailing divider above frame bottom border"
        );

        // T-5 (b) trailing 'and' in par renders
        let par_dsl = r"
        sequenceDiagram
          par one
            A->>B: task1
          and two
          end
        ";
        let DiagramSpec::Sequence(par_spec) =
            crate::parser::parse_sequence_dsl(par_dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let par_out = SequenceRenderer::new(&par_spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(par_out.contains("and two"));
    }

    #[test]
    fn test_sequence_critical_option_render() {
        // T-6 (a) critical up / msg / option backup / msg / end
        let dsl = r"
        sequenceDiagram
          critical up
            A->>B: live
          option backup
            A->>B: backup
          end
        ";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("critical up"));
        assert!(out.contains("option backup"));
    }

    #[test]
    fn test_sequence_nested_frames_draw_order() {
        // T-7: loop outer > alt inner > opt innermost: both frame labels render,
        // >= two '└' and two '┘' glyphs; inner bottom corner row above outer's
        let dsl = r"
        sequenceDiagram
          loop outer
            alt inner
              opt in
                A->>B: ping
              end
            end
          end
        ";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);

        assert!(out.contains("loop outer"));
        assert!(out.contains("alt inner"));
        assert!(out.contains("opt in"));

        let bl_count = out.chars().filter(|&c| c == '└').count();
        let br_count = out.chars().filter(|&c| c == '┘').count();
        assert!(
            bl_count >= 3,
            "expected at least 3 bottom-left corners, got {bl_count}"
        );
        assert!(
            br_count >= 3,
            "expected at least 3 bottom-right corners, got {br_count}"
        );

        let lines: Vec<&str> = out.lines().collect();
        let bottom_rows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains('└') && l.contains('┘'))
            .map(|(idx, _)| idx)
            .collect();
        assert!(bottom_rows.len() >= 3);
        // Each nested frame bottom is on a distinct or strictly ordered row
        assert!(bottom_rows[0] < bottom_rows[1]);
        assert!(bottom_rows[1] < bottom_rows[2]);
    }

    #[test]
    fn test_sequence_short_single_char_note() {
        // C2-T-1: single-char note 'note right of B: x' must not be dropped by span guard
        let dsl = "sequenceDiagram\nA->>B: msg\nnote right of B: x";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains(" x "), "short note text 'x' rendered");
        assert!(out.contains('┌'), "note box top border rendered");
        assert!(out.contains('└'), "note box bottom border rendered");
    }

    #[test]
    fn test_sequence_frame_label_longer_than_frame_width() {
        // C2-T-2: frame label longer than frame width must be rendered (truncated) rather than dropped
        let dsl = "sequenceDiagram\nbreak connection timed out\n  B->>A: reset\nend";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("break"), "frame label 'break' is present");
        assert!(out.contains('┌'), "frame top border rendered");
        assert!(out.contains('└'), "frame bottom border rendered");
    }

    #[test]
    fn test_sequence_note_in_frame_preserves_frame_borders() {
        // C2-T-3: full-span note over A,B inside a frame must not overwrite frame side borders
        let dsl = "sequenceDiagram\nalt check\n  A->>B: query\n  note over A,B: note text\n  B->>A: resp\nend";
        let DiagramSpec::Sequence(spec) =
            crate::parser::parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap()
        else {
            panic!("Expected sequence");
        };
        let renderer = SequenceRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("note text"));

        // Verify frame side borders are continuous vertical lines on note rows
        let lines: Vec<&str> = out.lines().collect();
        let note_row = lines.iter().position(|l| l.contains("note text")).unwrap();
        for line in &lines[(note_row - 1)..=(note_row + 1)] {
            assert!(
                line.starts_with('│'),
                "row preserves left frame border: {line}"
            );
            assert!(
                line.ends_with('│'),
                "row preserves right frame border: {line}"
            );
        }
    }

    // ---- Cycle-1 regressions: width policy, ascii dashed, empty labels ----

    fn render_seq(dsl: &str, style: BoxStyle) -> String {
        let DiagramSpec::Sequence(spec) = crate::parser::parse_sequence_dsl(dsl, style).unwrap()
        else {
            panic!("Expected sequence");
        };
        SequenceRenderer::new(&spec, Theme::new(style)).render(false)
    }

    /// The row carrying the arrowhead — layout columns must match between two
    /// diagrams whose labels differ only in encodings with equal display width.
    fn arrow_row(out: &str) -> String {
        out.lines()
            .find(|l| l.contains('►'))
            .expect("arrow row present")
            .to_string()
    }

    #[test]
    fn test_sequence_empty_label_suppresses_blank_row() {
        // SEQ-05: `A -> B:` must not emit a blank label row above the arrow
        let unlabeled = render_seq("sequenceDiagram\nA ->> B:", BoxStyle::Rounded);
        let labeled = render_seq("sequenceDiagram\nA ->> B: hi", BoxStyle::Rounded);
        assert_eq!(
            unlabeled.lines().count() + 1,
            labeled.lines().count(),
            "empty label renders exactly one row less:\n{unlabeled}"
        );
        // The arrow sits in the first message row, directly under the boxes
        let arrow_line = unlabeled
            .lines()
            .position(|l| l.contains('►'))
            .expect("arrow present");
        assert!(
            unlabeled
                .lines()
                .take(arrow_line)
                .all(|l| l.contains('│') || l.contains('─')),
            "no gap row between boxes and arrow:\n{unlabeled}"
        );
        // Whitespace-only labels behave the same
        let blank = render_seq("sequenceDiagram\nA ->> B:   ", BoxStyle::Rounded);
        assert_eq!(blank, unlabeled, "blank label == empty label");
    }

    #[test]
    fn test_sequence_ascii_dashed_arrow_distinct_from_solid() {
        // SEQ-02: ascii `-->` must be visibly dashed, not a second solid run
        let solid = render_seq("sequenceDiagram\nA -> B: s", BoxStyle::Ascii);
        let dashed = render_seq("sequenceDiagram\nA --> B: a", BoxStyle::Ascii);
        let solid_row = solid.lines().find(|l| l.contains('>')).unwrap();
        let dashed_row = dashed.lines().find(|l| l.contains('>')).unwrap();
        assert!(solid_row.contains('-'), "solid ascii run: {solid_row:?}");
        assert!(
            dashed_row.contains('.'),
            "ascii async arrow must be dotted: {dashed_row:?}"
        );
        assert!(
            !dashed_row.contains('-'),
            "ascii dashed must not reuse the solid glyph: {dashed_row:?}"
        );
    }

    #[test]
    fn test_sequence_skin_tone_and_zwj_width_match_terminal() {
        // SEQ-W-04: the arrow row of a skin-tone label must line up with an
        // ASCII label of the same collapsed display width
        let emoji = render_seq(
            "sequenceDiagram\nA ->> B: \u{1F44D}\u{1F3FD} ok",
            BoxStyle::Rounded,
        );
        let plain = render_seq("sequenceDiagram\nA ->> B: zz ok", BoxStyle::Rounded);
        assert_eq!(arrow_row(&emoji), arrow_row(&plain));

        let family = render_seq(
            "sequenceDiagram\nA ->> B: \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} fam",
            BoxStyle::Rounded,
        );
        let plain_fam = render_seq("sequenceDiagram\nA ->> B: xx fam", BoxStyle::Rounded);
        assert_eq!(arrow_row(&family), arrow_row(&plain_fam));
    }

    #[test]
    fn test_sequence_nfd_label_keeps_accent_and_width() {
        let nfd = render_seq("sequenceDiagram\nA ->> B: cafe\u{0301}", BoxStyle::Rounded);
        assert!(
            nfd.contains("e\u{0301}"),
            "SEQ-W-02: NFD accent must survive:\n{nfd:?}"
        );
        let ascii = render_seq("sequenceDiagram\nA ->> B: cafe", BoxStyle::Rounded);
        assert_eq!(
            arrow_row(&nfd),
            arrow_row(&ascii),
            "combining mark adds no columns"
        );
    }

    #[test]
    fn test_sequence_tab_in_label_expands_to_spaces() {
        let tabbed = render_seq("sequenceDiagram\nA ->> B: a\tb", BoxStyle::Rounded);
        assert!(
            !tabbed.contains('\t'),
            "no literal tab in output: {tabbed:?}"
        );
        let spaced = render_seq("sequenceDiagram\nA ->> B: a   b", BoxStyle::Rounded);
        assert_eq!(arrow_row(&tabbed), arrow_row(&spaced));
    }

    #[test]
    fn test_sequence_bidi_override_stripped() {
        // ERR-05: U+202E must never reach the rendered output
        let out = render_seq("sequenceDiagram\nA ->> B: \u{202e}abc", BoxStyle::Rounded);
        assert!(
            !out.contains('\u{202e}'),
            "bidi override rendered!: {out:?}"
        );
        let plain = render_seq("sequenceDiagram\nA ->> B: abc", BoxStyle::Rounded);
        assert_eq!(arrow_row(&out), arrow_row(&plain));
    }
}

use crate::canvas::{Canvas, Direction};
use crate::schema::{ArrowDirection, EdgeSpec};
use crate::theme::Theme;
use std::collections::HashSet;

/// True when any registered node obstacle intersects the horizontal span
/// [`x1..=x2`] at row `y`. Used to keep straight corridors from piercing
/// intermediate boxes (FC-LR-03 / FC-EDGE-03).
pub(super) fn hspan_blocked(canvas: &Canvas, x1: usize, x2: usize, y: usize) -> bool {
    let (lo, hi) = (x1.min(x2), x1.max(x2));
    canvas
        .obstacles
        .iter()
        .any(|o| o.y <= y && y <= o.bottom() && o.x <= hi && lo <= o.right())
}

/// True when any registered node obstacle intersects the vertical span
/// [`y1..=y2`] at column `x`.
pub(super) fn vspan_blocked(canvas: &Canvas, x: usize, y1: usize, y2: usize) -> bool {
    let (lo, hi) = (y1.min(y2), y1.max(y2));
    canvas
        .obstacles
        .iter()
        .any(|o| o.x <= x && x <= o.right() && o.y <= hi && lo <= o.bottom())
}

/// Extent of the blockers on the horizontal span [`x1..=x2`] at row `y`:
/// `(topmost blocker top, bottommost blocker bottom)`.
fn hspan_blocker_extent(canvas: &Canvas, x1: usize, x2: usize, y: usize) -> Option<(usize, usize)> {
    let (lo, hi) = (x1.min(x2), x1.max(x2));
    let mut top = usize::MAX;
    let mut bottom = 0usize;
    for o in &canvas.obstacles {
        if o.y <= y && y <= o.bottom() && o.x <= hi && lo <= o.right() {
            top = top.min(o.y);
            bottom = bottom.max(o.bottom());
        }
    }
    (top != usize::MAX).then_some((top, bottom))
}

/// Picks a detour row for a horizontal corridor blocked at `y`: one row above
/// the topmost blocker or one row below the bottommost blocker. A candidate
/// is valid only when the whole jog path (detour hline plus both vertical
/// jogs at `x1+1` / `x2-1`) is obstacle-free. Nearest valid row wins.
pub(super) fn detour_row(canvas: &Canvas, x1: usize, x2: usize, y: usize) -> Option<usize> {
    let (block_top, block_bottom) = hspan_blocker_extent(canvas, x1, x2, y)?;
    let jx = x1 + 1;
    let ex = x2.saturating_sub(1);
    if jx >= ex {
        return None;
    }
    let mut candidates = Vec::with_capacity(2);
    if let Some(above) = block_top.checked_sub(1) {
        candidates.push(above);
    }
    candidates.push(block_bottom + 1);
    candidates
        .into_iter()
        .filter(|&dy| {
            !hspan_blocked(canvas, jx, ex, dy)
                && !vspan_blocked(canvas, jx, dy.min(y), dy.max(y))
                && !vspan_blocked(canvas, ex, dy.min(y), dy.max(y))
        })
        .min_by_key(|&dy| dy.abs_diff(y))
}
/// Nearest column to `preferred` within `lo..=hi` whose vertical span
/// `ya..=yb` is free of node obstacles (FC-EDGE-03: bend corridors must not
/// cross intermediate boxes). Scans outward, right side first.
pub(super) fn clear_column(
    canvas: &Canvas,
    lo: usize,
    hi: usize,
    ya: usize,
    yb: usize,
    preferred: usize,
) -> Option<usize> {
    if lo > hi {
        return None;
    }
    for d in 0..=(hi - lo) {
        let right = preferred + d;
        if right >= lo && right <= hi && !vspan_blocked(canvas, right, ya, yb) {
            return Some(right);
        }
        if d == 0 {
            continue;
        }
        match preferred.checked_sub(d) {
            Some(left) if left >= lo && left <= hi && !vspan_blocked(canvas, left, ya, yb) => {
                return Some(left);
            }
            // `preferred - d` underflowed: no left candidates remain
            None => break,
            _ => {}
        }
    }
    None
}

/// Finds a clear row for a loop-back segment spanning `from_x..to_x`.
/// With `down`, routes below boxes that block the direct path (source side);
/// otherwise above them (target side). Returns `y` unchanged when clear.
pub(super) fn clear_route_y(
    canvas: &Canvas,
    from_x: usize,
    to_x: usize,
    y: usize,
    down: bool,
) -> usize {
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

pub(super) fn edge_hline(
    canvas: &mut Canvas,
    edge: &EdgeSpec,
    x1: usize,
    x2: usize,
    y: usize,
    theme: &Theme,
) {
    canvas.set_pen(edge.color);
    if edge.dashed {
        canvas.draw_dashed_hline(x1, x2, y, theme);
    } else if edge.thick {
        canvas.draw_thick_hline(x1, x2, y);
    } else {
        canvas.draw_hline(x1, x2, y);
    }
}

pub(super) fn edge_vline(
    canvas: &mut Canvas,
    edge: &EdgeSpec,
    x: usize,
    y1: usize,
    y2: usize,
    theme: &Theme,
) {
    canvas.set_pen(edge.color);
    if edge.dashed {
        canvas.draw_dashed_vline(x, y1, y2, theme);
    } else if edge.thick {
        canvas.draw_thick_vline(x, y1, y2);
    } else {
        canvas.draw_vline(x, y1, y2);
    }
}

/// Draws arrowheads per `edge.arrow`: Forward → target end only, Both → both
/// ends, None → none.
pub(super) fn edge_arrow_heads(
    canvas: &mut Canvas,
    edge: &EdgeSpec,
    t: (usize, usize, Direction),
    s: (usize, usize, Direction),
    theme: &Theme,
) {
    canvas.set_pen(edge.color);
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

pub(super) fn dfs_find_cycles(
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

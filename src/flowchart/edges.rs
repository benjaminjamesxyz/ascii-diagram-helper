use crate::canvas::{Canvas, Direction};
use crate::schema::{ArrowDirection, EdgeSpec};
use crate::theme::Theme;
use std::collections::HashSet;

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
    if edge.dashed {
        canvas.draw_dashed_hline(x1, x2, y, theme);
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
    if edge.dashed {
        canvas.draw_dashed_vline(x, y1, y2, theme);
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

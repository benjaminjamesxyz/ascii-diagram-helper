use crate::canvas::{Canvas, CellRole, Direction, LineConn, Rect};
use crate::schema::{
    ArchitectureSpec, ContainerItem, ContainerLayout, ContainerSpec, EdgeSpec, LeafComponent,
};
use crate::theme::Theme;
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
struct BoxBounds {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

pub struct ArchitectureRenderer<'a> {
    spec: &'a ArchitectureSpec,
    theme: Theme,
}

impl<'a> ArchitectureRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a ArchitectureSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    /// Renders the diagram.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the spec has no containers, duplicate IDs, or
    /// connections referencing unknown, self, or container IDs (each named
    /// in the message).
    pub fn render(&self, colored: bool) -> Result<String, String> {
        if self.spec.containers.is_empty() {
            return Err("architecture requires at least one container".to_string());
        }
        let (comp_ids, container_ids) = validate_ids(&self.spec.containers)?;

        let mut comp_bounds: HashMap<String, BoxBounds> = HashMap::new();
        let mut container_rects: Vec<Rect> = Vec::new();

        // Compute layout for each top-level container. With no connections
        // there is nothing to route, so stacked containers keep a small
        // uniform gap instead of reserving blank corridor rows.
        let gap = if self.spec.connections.is_empty() {
            2
        } else {
            4
        };
        let mut top_layouts = Vec::new();
        let start_y = if self.spec.title.is_some() { 2 } else { 0 };
        let mut cur_y = start_y;

        for c in &self.spec.containers {
            let (w, h) = self.measure_container(c);
            top_layouts.push((0, cur_y, w, h));
            cur_y += h + gap;
        }

        let max_w = top_layouts
            .iter()
            .map(|(_, _, w, _)| *w)
            .max()
            .unwrap_or(40);
        // Normalize only the stacked outer frames; nested containers retain
        // their measured widths and their children retain their placement.
        for (_, _, width, _) in &mut top_layouts {
            *width = max_w;
        }
        let total_h = cur_y + 2;

        let mut canvas = Canvas::new(max_w + 4, total_h);

        // Title (whitespace-only titles are trimmed away entirely)
        if let Some(spec_title) = &self.spec.title {
            let title = spec_title.trim();
            if !title.is_empty() {
                let tw = UnicodeWidthStr::width(title);
                let tx = if max_w > tw { (max_w - tw) / 2 } else { 0 };
                canvas.draw_text(tx, 0, title);
            }
        }

        // Draw containers first: fills comp_bounds and lays down walls
        for (i, c) in self.spec.containers.iter().enumerate() {
            let (_, y, w, h) = top_layouts[i];
            self.render_container(
                &mut canvas,
                c,
                Rect::new(0, y, w, h),
                &mut comp_bounds,
                &mut container_rects,
            );
        }

        validate_connections(&self.spec.connections, &comp_ids, &container_ids)?;

        // Corridor routing: leaf boxes are impassable, container borders may
        // only be crossed perpendicular (which paints the junction glyph),
        // and text cells are impassable — so routes run through the free
        // corridors between boxes and never cross item content.
        let grid = RouteGrid::new(
            canvas.width,
            canvas.height,
            &container_rects,
            &comp_bounds,
            &canvas,
        );

        // Route and draw each connection through the corridor grid.
        let mut labeled_routes = Vec::new();
        for conn in &self.spec.connections {
            let (Some(u), Some(v)) = (comp_bounds.get(&conn.from), comp_bounds.get(&conn.to))
            else {
                continue;
            };
            canvas.set_pen(conn.color);
            let u_rect = Rect::new(u.x, u.y, u.width, u.height);
            let v_rect = Rect::new(v.x, v.y, v.width, v.height);
            let Some(route) = grid.route(u_rect, v_rect) else {
                // Degenerate-shape safety net (layout gaps always leave a
                // corridor): fall back to the direct center Z route.
                draw_fallback_route(&mut canvas, u, v, &self.theme);
                continue;
            };
            draw_cells(&mut canvas, &route.cells);
            canvas.draw_corner(
                route.src_anchor.0,
                route.src_anchor.1,
                line_conn_toward(route.src_dir),
            );
            canvas.draw_corner(
                route.dst_anchor.0,
                route.dst_anchor.1,
                line_conn_toward(route.dst_dir),
            );
            let arrow_cell = route.cells[route.cells.len() - 1];
            canvas.draw_arrow(arrow_cell.0, arrow_cell.1, route.arrow, &self.theme);

            if conn.label.is_some() {
                labeled_routes.push((conn, u, v, route));
            }
        }

        // Labels see every routed line, so later connections cannot overwrite
        // earlier labels or make their placement depend on draw order.
        for (conn, u, v, route) in labeled_routes {
            let cw = canvas.width;
            let ch = canvas.height;
            place_route_label(
                &mut canvas,
                &route.cells,
                u,
                v,
                &top_layouts,
                cw,
                ch,
                conn.label.as_deref().unwrap(),
            );
        }

        canvas.set_pen(None);

        Ok(canvas.render_impl(&self.theme, colored))
    }
    fn calculate_row_gap(&self) -> usize {
        // No connections means nothing to route or label between row items:
        // keep them close instead of reserving blank corridor columns.
        if self.spec.connections.is_empty() {
            return 4;
        }
        let max_label_w = self
            .spec
            .connections
            .iter()
            .filter_map(|conn| conn.label.as_deref())
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        (max_label_w + 6).max(16)
    }

    fn measure_leaf(leaf: &LeafComponent) -> (usize, usize) {
        let name = leaf.name.trim();
        let name_w = UnicodeWidthStr::width(name);
        let props_w = leaf
            .properties
            .iter()
            .map(|(k, v)| UnicodeWidthStr::width(format!("{k}: {v}").as_str()))
            .max()
            .unwrap_or(0);

        let w = name_w.max(props_w) + 4; // 2 padding + 2 border
        let h = if leaf.properties.is_empty() {
            3
        } else {
            4 + leaf.properties.len()
        };
        (w.max(14), h)
    }

    fn measure_container(&self, c: &ContainerSpec) -> (usize, usize) {
        let title_w = UnicodeWidthStr::width(c.title.trim()) + 6;

        if c.items.is_empty() {
            return (title_w.max(16), 4);
        }

        let child_sizes: Vec<(usize, usize)> = c
            .items
            .iter()
            .map(|item| match item {
                ContainerItem::Leaf(l) => Self::measure_leaf(l),
                ContainerItem::SubContainer(sub) => self.measure_container(sub),
            })
            .collect();

        match c.layout {
            ContainerLayout::Row => {
                let gap = self.calculate_row_gap();
                let total_w: usize = child_sizes.iter().map(|(w, _)| *w).sum::<usize>()
                    + gap * (child_sizes.len() - 1)
                    + 4;
                let max_h = child_sizes.iter().map(|(_, h)| *h).max().unwrap_or(3) + 4;
                (total_w.max(title_w), max_h)
            }
            ContainerLayout::Column => {
                let gap = 2;
                let max_w = child_sizes.iter().map(|(w, _)| *w).max().unwrap_or(12) + 4;
                let total_h: usize = child_sizes.iter().map(|(_, h)| *h).sum::<usize>()
                    + gap * (child_sizes.len() - 1)
                    + 4;
                (max_w.max(title_w), total_h)
            }
        }
    }

    fn place_label(canvas: &mut Canvas, bounds: Rect, px: usize, py: usize, label: &str) -> bool {
        if let Some((sx, sy)) = canvas.find_safe_text_pos_within(bounds, px, py, label) {
            canvas.draw_text(sx, sy, label);
            return true;
        }
        // The local search can miss a free corridor on a long vertical route.
        // Keep the nearest safe position in the same bounded search window.
        let label_w = UnicodeWidthStr::width(label);
        let mut best = None;
        if label_w <= bounds.width {
            for y in bounds.y..bounds.y + bounds.height {
                for x in bounds.x..=bounds.x + bounds.width - label_w {
                    let distance = x.abs_diff(px) + y.abs_diff(py);
                    if best.is_none_or(|(_, _, d)| distance < d)
                        && canvas.can_place_text(x, y, label)
                    {
                        best = Some((x, y, distance));
                    }
                }
            }
        }
        if let Some((x, y, _)) = best {
            canvas.draw_text(x, y, label);
            return true;
        }
        false
    }

    fn render_container(
        &self,
        canvas: &mut Canvas,
        c: &ContainerSpec,
        area: Rect,
        bounds: &mut HashMap<String, BoxBounds>,
        container_rects: &mut Vec<Rect>,
    ) {
        container_rects.push(area);
        // Draw outer container box (whitespace-only titles render as none)
        canvas.set_pen(c.color);
        canvas.draw_box(
            area.x,
            area.y,
            area.width,
            area.height,
            &self.theme,
            Some(c.title.trim()),
        );
        canvas.obstacles.pop();
        canvas.set_pen(None);

        let gap = match c.layout {
            ContainerLayout::Row => self.calculate_row_gap(),
            ContainerLayout::Column => 2,
        };
        let mut cur_x = area.x + 2;
        let mut cur_y = area.y + 2;

        for item in &c.items {
            match item {
                ContainerItem::Leaf(leaf) => {
                    let (w, h) = Self::measure_leaf(leaf);
                    self.render_leaf(canvas, leaf, cur_x, cur_y, w, h);
                    bounds.insert(
                        leaf.id.clone(),
                        BoxBounds {
                            x: cur_x,
                            y: cur_y,
                            width: w,
                            height: h,
                        },
                    );

                    match c.layout {
                        ContainerLayout::Row => {
                            cur_x += w + gap;
                        }
                        ContainerLayout::Column => {
                            cur_y += h + gap;
                        }
                    }
                }
                ContainerItem::SubContainer(sub) => {
                    let (w, h) = self.measure_container(sub);
                    self.render_container(
                        canvas,
                        sub,
                        Rect::new(cur_x, cur_y, w, h),
                        bounds,
                        container_rects,
                    );

                    match c.layout {
                        ContainerLayout::Row => {
                            cur_x += w + gap;
                        }
                        ContainerLayout::Column => {
                            cur_y += h + gap;
                        }
                    }
                }
            }
        }
    }

    fn render_leaf(
        &self,
        canvas: &mut Canvas,
        leaf: &LeafComponent,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) {
        canvas.set_pen(leaf.color);
        canvas.draw_box(x, y, width, height, &self.theme, None);
        canvas.add_obstacle(Rect::new(x, y, width, height));
        canvas.set_pen(None);

        // Name (whitespace-only names collapse to an empty label)
        let name = leaf.name.trim();
        let name_w = UnicodeWidthStr::width(name);
        let name_x = x + (width.saturating_sub(name_w)) / 2;
        canvas.draw_text(name_x, y + 1, name);

        // Properties with divider if present
        if !leaf.properties.is_empty() && height >= 4 {
            let right = x + width - 1;
            canvas.draw_hline(x + 1, right - 1, y + 2);
            canvas.draw_corner(
                x,
                y + 2,
                crate::canvas::LineConn {
                    north: true,
                    south: true,
                    east: true,
                    west: false,
                },
            );
            canvas.draw_corner(
                right,
                y + 2,
                crate::canvas::LineConn {
                    north: true,
                    south: true,
                    west: true,
                    east: false,
                },
            );

            for (i, (k, v)) in leaf.properties.iter().enumerate() {
                let prop_text = format!("{k}: {v}");
                canvas.draw_text(x + 2, y + 3 + i, &prop_text);
            }
        } else {
            for (i, (k, v)) in leaf.properties.iter().enumerate() {
                let prop_text = format!("{k}: {v}");
                canvas.draw_text(x + 2, y + 2 + i, &prop_text);
            }
        }
    }
}

/// Rejects specs with duplicate IDs up front and returns the component /
/// container ID sets. An ambiguous reference (one ID naming two boxes)
/// would otherwise be dropped silently at routing time.
fn validate_ids(
    containers: &[ContainerSpec],
) -> Result<(HashSet<String>, HashSet<String>), String> {
    fn claim(
        owner: &mut HashMap<String, &'static str>,
        id: &str,
        kind: &'static str,
    ) -> Result<(), String> {
        if let Some(prev) = owner.insert(id.to_string(), kind) {
            return Err(format!(
                "duplicate ID '{id}': already declared as a {prev} and again as a {kind} — \
                 architecture IDs must be unique across containers and components"
            ));
        }
        Ok(())
    }

    fn walk(
        c: &ContainerSpec,
        comps: &mut HashSet<String>,
        conts: &mut HashSet<String>,
        owner: &mut HashMap<String, &'static str>,
    ) -> Result<(), String> {
        claim(owner, &c.id, "container")?;
        conts.insert(c.id.clone());
        for item in &c.items {
            match item {
                ContainerItem::Leaf(l) => {
                    claim(owner, &l.id, "component")?;
                    comps.insert(l.id.clone());
                }
                ContainerItem::SubContainer(sub) => walk(sub, comps, conts, owner)?,
            }
        }
        Ok(())
    }

    let mut comps = HashSet::new();
    let mut conts = HashSet::new();
    let mut owner: HashMap<String, &'static str> = HashMap::new();
    for c in containers {
        walk(c, &mut comps, &mut conts, &mut owner)?;
    }
    Ok((comps, conts))
}

/// Hard-errors on unusable connections instead of warn-and-drop: each
/// problem names the connection index and the offending ID.
fn validate_connections(
    connections: &[EdgeSpec],
    comps: &HashSet<String>,
    conts: &HashSet<String>,
) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    for (i, conn) in connections.iter().enumerate() {
        if conn.from == conn.to {
            problems.push(format!(
                "connection {i} ({} -> {}): self-loop connection on '{}'",
                conn.from, conn.to, conn.from
            ));
            continue;
        }
        for (endpoint, field) in [(&conn.from, "from"), (&conn.to, "to")] {
            if conts.contains(endpoint) {
                problems.push(format!(
                    "connection {i}: endpoint '{endpoint}' ({field}) is a container ID — \
                     architecture connections link components (items), not containers"
                ));
            } else if !comps.contains(endpoint) {
                problems.push(format!(
                    "connection {i}: endpoint '{endpoint}' ({field}) is an unknown component ID"
                ));
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "cannot render architecture connections:\n  - {}",
            problems.join("\n  - ")
        ))
    }
}

fn line_conn_toward(d: Direction) -> LineConn {
    match d {
        Direction::Up => LineConn {
            north: true,
            ..LineConn::default()
        },
        Direction::Down => LineConn {
            south: true,
            ..LineConn::default()
        },
        Direction::Left => LineConn {
            west: true,
            ..LineConn::default()
        },
        Direction::Right => LineConn {
            east: true,
            ..LineConn::default()
        },
    }
}

fn dir_delta(d: Direction) -> (isize, isize) {
    match d {
        Direction::Up => (0, -1),
        Direction::Down => (0, 1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
    }
}

fn delta_to_dir(dx: isize, dy: isize) -> Direction {
    if dx > 0 {
        Direction::Right
    } else if dx < 0 {
        Direction::Left
    } else if dy > 0 {
        Direction::Down
    } else {
        Direction::Up
    }
}

fn step(p: (usize, usize), d: Direction) -> Option<(usize, usize)> {
    let (dx, dy) = dir_delta(d);
    let nx = p.0 as isize + dx;
    let ny = p.1 as isize + dy;
    if nx < 0 || ny < 0 {
        None
    } else {
        Some((nx as usize, ny as usize))
    }
}

fn center_x(r: Rect) -> usize {
    r.x + r.width / 2
}

fn center_y(r: Rect) -> usize {
    r.y + r.height / 2
}

/// Border cells (corners excluded) of `r` on the sides facing the other
/// box, paired with the outward direction. `(dx, dy)` points from `r`
/// toward the other box.
fn facing_anchors(r: Rect, dx: isize, dy: isize) -> Vec<((usize, usize), Direction)> {
    let right = r.x + r.width - 1;
    let bottom = r.y + r.height - 1;
    let mut out = Vec::new();
    if dy > 0 {
        for x in (r.x + 1)..right {
            out.push(((x, bottom), Direction::Down));
        }
    }
    if dy < 0 {
        for x in (r.x + 1)..right {
            out.push(((x, r.y), Direction::Up));
        }
    }
    if dx > 0 {
        for y in (r.y + 1)..bottom {
            out.push(((right, y), Direction::Right));
        }
    }
    if dx < 0 {
        for y in (r.y + 1)..bottom {
            out.push(((r.x, y), Direction::Left));
        }
    }
    out
}

/// A routed connection: corridor cells from the seed beside the source box
/// to the cell beside the target box, the two border anchor cells, and the
/// arrowhead direction (the final move direction, pointing into the
/// target).
struct Routed {
    cells: Vec<(usize, usize)>,
    src_anchor: (usize, usize),
    src_dir: Direction,
    dst_anchor: (usize, usize),
    dst_dir: Direction,
    arrow: Direction,
}

/// Per-cell routing surface. Leaf boxes and any rendered text are
/// impassable; container borders may only be crossed perpendicular to
/// their edge (a vertical move may enter a horizontal border cell), which
/// is what paints the junction glyph where a route pierces a wall.
struct RouteGrid<'a> {
    w: usize,
    h: usize,
    blocked: Vec<bool>,
    hborder: Vec<bool>,
    vborder: Vec<bool>,
    containers: &'a [Rect],
    leaves: &'a HashMap<String, BoxBounds>,
}

impl<'a> RouteGrid<'a> {
    fn new(
        w: usize,
        h: usize,
        containers: &'a [Rect],
        leaves: &'a HashMap<String, BoxBounds>,
        canvas: &Canvas,
    ) -> Self {
        let mut grid = Self {
            w,
            h,
            blocked: vec![false; w * h],
            hborder: vec![false; w * h],
            vborder: vec![false; w * h],
            containers,
            leaves,
        };
        for r in containers {
            grid.mark_container(*r);
        }
        for b in leaves.values() {
            grid.mark_blocked(Rect::new(b.x, b.y, b.width, b.height));
        }
        // Any rendered text/arrow cell (spec title, container titles, item
        // content) is impassable and also drops its border-corridor flag.
        for y in 0..h {
            for x in 0..w {
                if let Some(cell) = canvas.get_cell(x, y)
                    && (cell.role == CellRole::Text
                        || cell.role == CellRole::Arrow
                        || cell.is_continuation)
                {
                    let i = y * w + x;
                    grid.blocked[i] = true;
                    grid.hborder[i] = false;
                    grid.vborder[i] = false;
                }
            }
        }
        grid
    }

    fn mark_blocked(&mut self, r: Rect) {
        for y in r.y..(r.y + r.height) {
            for x in r.x..(r.x + r.width) {
                if x < self.w && y < self.h {
                    self.blocked[y * self.w + x] = true;
                }
            }
        }
    }

    fn mark_container(&mut self, r: Rect) {
        if r.width < 2 || r.height < 2 {
            return;
        }
        let right = r.x + r.width - 1;
        let bottom = r.y + r.height - 1;
        // Corners never carry a route.
        for (cx, cy) in [(r.x, r.y), (right, r.y), (r.x, bottom), (right, bottom)] {
            if cx < self.w && cy < self.h {
                self.blocked[cy * self.w + cx] = true;
            }
        }
        for x in (r.x + 1)..right {
            if x < self.w {
                self.hborder[r.y * self.w + x] = true;
                self.hborder[bottom * self.w + x] = true;
            }
        }
        for y in (r.y + 1)..bottom {
            if y < self.h {
                self.vborder[y * self.w + r.x] = true;
                self.vborder[y * self.w + right] = true;
            }
        }
    }

    /// Directional clearance leaves a blank cell beside unrelated boxes
    /// without blocking perpendicular attachments or container crossings.
    fn mark_clearance(&self, flags: &mut [u8], r: Rect) {
        let left = r.x.saturating_sub(1);
        let right = (r.x + r.width).min(self.w - 1);
        let top = r.y.saturating_sub(1);
        let bottom = (r.y + r.height).min(self.h - 1);
        for x in left..=right {
            if r.y > 0 {
                flags[top * self.w + x] |= 1;
            }
            if r.y + r.height < self.h {
                flags[bottom * self.w + x] |= 1;
            }
        }
        for y in top..=bottom {
            if r.x > 0 {
                flags[y * self.w + left] |= 2;
            }
            if r.x + r.width < self.w {
                flags[y * self.w + right] |= 2;
            }
        }
    }

    /// Whether a move in direction `(dx, dy)` may land on `(x, y)`.
    fn step_ok(&self, x: usize, y: usize, dx: isize, dy: isize) -> bool {
        if x >= self.w || y >= self.h {
            return false;
        }
        let i = y * self.w + x;
        if self.blocked[i] {
            return false;
        }
        // Running along a border would erase/overlap it; only perpendicular
        // crossings are legal.
        if dx != 0 && self.hborder[i] {
            return false;
        }
        if dy != 0 && self.vborder[i] {
            return false;
        }
        true
    }

    /// Shortest corridor path between two component boxes. `None` only if
    /// no legal path exists (layout gaps guarantee one, so this is a
    /// degenerate-shape guard rather than an expected outcome).
    fn route(&self, u: Rect, v: Rect) -> Option<Routed> {
        let dx = center_x(v) as isize - center_x(u) as isize;
        let dy = center_y(v) as isize - center_y(u) as isize;

        // Reuse the visitation buffer for directional clearance bits (1/2);
        // bit 4 records BFS visitation. Endpoint boxes and their containing
        // frames must remain accessible for attachment and wall crossings.
        let mut visited = vec![0_u8; self.w * self.h];
        for b in self.leaves.values() {
            let r = Rect::new(b.x, b.y, b.width, b.height);
            if r != u && r != v {
                self.mark_clearance(&mut visited, r);
            }
        }
        for &r in self.containers {
            if !r.contains_point(center_x(u), center_y(u))
                && !r.contains_point(center_x(v), center_y(v))
            {
                self.mark_clearance(&mut visited, r);
            }
        }

        // (seed cell, source anchor, outward direction)
        let mut seeds = Vec::new();
        for (anchor, dir) in facing_anchors(u, dx, dy) {
            if let Some(seed) = step(anchor, dir) {
                let (ddx, ddy) = dir_delta(dir);
                let mask = if ddx != 0 { 1 } else { 2 };
                if self.step_ok(seed.0, seed.1, ddx, ddy)
                    && visited[seed.1 * self.w + seed.0] & mask == 0
                {
                    seeds.push((seed, anchor, dir));
                }
            }
        }
        // goal cell -> (target anchor, inward direction)
        let mut goals: HashMap<(usize, usize), ((usize, usize), Direction)> = HashMap::new();
        for (anchor, dir) in facing_anchors(v, -dx, -dy) {
            if let Some(goal) = step(anchor, dir) {
                let (ddx, ddy) = dir_delta(dir);
                let mask = if ddx != 0 { 1 } else { 2 };
                if self.step_ok(goal.0, goal.1, ddx, ddy)
                    && visited[goal.1 * self.w + goal.0] & mask == 0
                {
                    goals.insert(goal, (anchor, dir));
                }
            }
        }
        if seeds.is_empty() || goals.is_empty() {
            return None;
        }

        // Multi-source BFS over legal moves.
        let mut prev: Vec<Option<u32>> = vec![None; self.w * self.h];
        let mut roots: HashMap<usize, ((usize, usize), Direction)> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        for (cell, anchor, dir) in seeds {
            let idx = cell.1 * self.w + cell.0;
            visited[idx] |= 4;
            roots.insert(idx, (anchor, dir));
            queue.push_back(idx);
        }
        let found = 'search: {
            while let Some(idx) = queue.pop_front() {
                let (x, y) = (idx % self.w, idx / self.w);
                for dir in [
                    Direction::Up,
                    Direction::Down,
                    Direction::Left,
                    Direction::Right,
                ] {
                    let (ddx, ddy) = dir_delta(dir);
                    let Some((nx, ny)) = step((x, y), dir) else {
                        continue;
                    };
                    if !self.step_ok(x, y, ddx, ddy) || !self.step_ok(nx, ny, ddx, ddy) {
                        continue;
                    }
                    let nidx = ny * self.w + nx;
                    let mask = if ddx != 0 { 1 } else { 2 };
                    if visited[nidx] & 4 != 0
                        || visited[idx] & mask != 0
                        || visited[nidx] & mask != 0
                    {
                        continue;
                    }
                    if let Some(&(_, dst_dir)) = goals.get(&(nx, ny)) {
                        let (gx, gy) = dir_delta(dst_dir);
                        if (ddx, ddy) != (-gx, -gy) {
                            continue;
                        }
                    }
                    visited[nidx] |= 4;
                    prev[nidx] = Some(idx as u32);
                    if let Some(&(dst_anchor, dst_dir)) = goals.get(&(nx, ny)) {
                        break 'search Some((nidx, dst_anchor, dst_dir));
                    }
                    queue.push_back(nidx);
                }
            }
            None
        };
        let (goal_idx, dst_anchor, dst_dir) = found?;

        // Reconstruct seed -> goal.
        let mut cells = Vec::new();
        let mut cur = goal_idx;
        loop {
            cells.push((cur % self.w, cur / self.w));
            let Some(p) = prev[cur] else { break };
            cur = p as usize;
        }
        cells.reverse();
        let (src_anchor, src_dir) = roots[&cur];
        if cells.len() < 2 {
            return None;
        }
        let (x0, y0) = cells[cells.len() - 2];
        let (x1, y1) = cells[cells.len() - 1];
        let arrow = delta_to_dir(x1 as isize - x0 as isize, y1 as isize - y0 as isize);
        Some(Routed {
            cells,
            src_anchor,
            src_dir,
            dst_anchor,
            dst_dir,
            arrow,
        })
    }
}

/// Stamps a routed cell path as line runs (crossings merge into junction
/// glyphs via the canvas conn flags).
fn draw_cells(canvas: &mut Canvas, cells: &[(usize, usize)]) {
    let mut i = 0;
    while i + 1 < cells.len() {
        let (x0, y0) = cells[i];
        let mut j = i + 1;
        if cells[j].0 == x0 {
            while j + 1 < cells.len() && cells[j + 1].0 == x0 {
                j += 1;
            }
            canvas.draw_vline(x0, y0, cells[j].1);
        } else {
            while j + 1 < cells.len() && cells[j + 1].1 == y0 {
                j += 1;
            }
            canvas.draw_hline(x0, cells[j].0, y0);
        }
        i = j;
    }
}

/// Safety net when no corridor path exists: the direct center-to-center Z
/// route (best effort, matches the pre-router geometry).
fn draw_fallback_route(canvas: &mut Canvas, u: &BoxBounds, v: &BoxBounds, theme: &Theme) {
    let u_right = u.x + u.width - 1;
    let u_cy = u.y + u.height / 2;
    let u_cx = u.x + u.width / 2;
    let u_bottom = u.y + u.height - 1;
    let v_left = v.x;
    let v_cx = v.x + v.width / 2;
    let v_cy = v.y + v.height / 2;
    if u_right < v_left {
        let mid_x = u_right + (v_left - u_right) / 2;
        canvas.draw_hline(u_right + 1, mid_x, u_cy);
        canvas.draw_vline(mid_x, u_cy, v_cy);
        canvas.draw_hline(mid_x, v_left - 1, v_cy);
        canvas.draw_arrow(v_left - 1, v_cy, Direction::Right, theme);
    } else if u.y + u.height <= v.y {
        let mid_y = u_bottom + (v.y - u_bottom) / 2;
        canvas.draw_vline(u_cx, u_bottom + 1, mid_y);
        canvas.draw_hline(u_cx, v_cx, mid_y);
        canvas.draw_vline(v_cx, mid_y, v.y.saturating_sub(1));
        canvas.draw_arrow(v_cx, v.y.saturating_sub(1), Direction::Down, theme);
    } else {
        let route_y = u.y + u.height + 1;
        canvas.draw_vline(u_cx, u.y + u.height, route_y);
        canvas.draw_hline(v_cx.min(u_cx), v_cx.max(u_cx), route_y);
        canvas.draw_vline(v_cx, route_y, v.y + v.height);
        canvas.draw_arrow(v_cx, v.y + v.height, Direction::Up, theme);
    }
}

/// Longest straight horizontal run in a routed path: `(y, x1, x2)`.
fn longest_horizontal_run(cells: &[(usize, usize)]) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    let mut i = 0;
    while i < cells.len() {
        let mut j = i;
        while j + 1 < cells.len()
            && cells[j + 1].1 == cells[i].1
            && cells[j + 1].0 == cells[j].0 + 1
        {
            j += 1;
        }
        if j > i {
            let cand = (cells[i].1, cells[i].0, cells[j].0);
            best = match best {
                Some(b) if b.2 - b.1 >= cand.2 - cand.1 => Some(b),
                _ => Some(cand),
            };
        }
        i = j + 1;
    }
    best
}

/// Search window for a route label: the interior of the top-level
/// container holding both endpoints, else a window spanning both endpoint
/// boxes (clamped to the canvas).
fn label_bounds(
    u: &BoxBounds,
    v: &BoxBounds,
    top_layouts: &[(usize, usize, usize, usize)],
    canvas_w: usize,
    canvas_h: usize,
) -> Rect {
    let inside = |b: &BoxBounds, r: Rect| r.contains_point(b.x + b.width / 2, b.y + b.height / 2);
    for &(cx, cy, cw, ch) in top_layouts {
        let r = Rect::new(cx, cy, cw, ch);
        if inside(u, r) && inside(v, r) {
            return Rect::new(cx + 1, cy + 1, cw.saturating_sub(2), ch.saturating_sub(2));
        }
    }
    let min_x = u.x.min(v.x);
    let min_y = u.y.min(v.y);
    let max_x = (u.x + u.width).max(v.x + v.width);
    let max_y = (u.y + u.height).max(v.y + v.height);
    let mut rect = Rect::new(
        min_x.saturating_sub(4),
        min_y.saturating_sub(1),
        max_x - min_x + 9,
        max_y - min_y + 3,
    );
    rect.width = rect.width.min(canvas_w.saturating_sub(rect.x));
    rect.height = rect.height.min(canvas_h.saturating_sub(rect.y));
    rect
}

/// Places a connection label anchored to its route: one row above the
/// longest horizontal run, or beside the path midpoint for straight
/// vertical routes.
#[allow(
    clippy::too_many_arguments,
    reason = "explicit route/context arguments keep the call site readable"
)]
fn place_route_label(
    canvas: &mut Canvas,
    cells: &[(usize, usize)],
    u: &BoxBounds,
    v: &BoxBounds,
    top_layouts: &[(usize, usize, usize, usize)],
    canvas_w: usize,
    canvas_h: usize,
    label: &str,
) {
    let lbl_w = UnicodeWidthStr::width(label);
    let run = longest_horizontal_run(cells);
    let (px, py) = match run {
        Some((ry, rx1, rx2)) => (
            usize::midpoint(rx1, rx2).saturating_sub(lbl_w / 2),
            ry.saturating_sub(1),
        ),
        None => {
            let mid = cells[cells.len() / 2];
            (mid.0.saturating_sub(lbl_w + 2), mid.1)
        }
    };
    let bounds = label_bounds(u, v, top_layouts, canvas_w, canvas_h);
    if ArchitectureRenderer::place_label(canvas, bounds, px, py, label) {
        return;
    }
    if let Some((ry, _, _)) = run {
        ArchitectureRenderer::place_label(
            canvas,
            bounds,
            px,
            (ry + 1).min(canvas_h.saturating_sub(1)),
            label,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    fn firmware_spec() -> ArchitectureSpec {
        serde_json::from_str(
            r#"{
                "type": "architecture",
                "title": "UAV STM32H7 Dual-Core Memory & Bus Architecture",
                "containers": [
                    {
                        "id": "core_domain", "title": "Core Processing Domain", "layout": "row",
                        "items": [
                            {"id": "c0", "name": "Cortex-M7 (480 MHz)",
                             "properties": [["Role", "1 kHz Flight Loop"], ["L1 I/D Cache", "32 KB / 32 KB"], ["TCM", "128 KB DTCM"]]},
                            {"id": "c1", "name": "Cortex-M4 (240 MHz)",
                             "properties": [["Role", "50 Hz Nav & Log"], ["Bus", "AXI / AHB3"], ["SRAM", "64 KB SRAM3"]]}
                        ]
                    },
                    {
                        "id": "mem_domain", "title": "Memory & Bus Interconnect", "layout": "row",
                        "items": [
                            {"id": "dma_sram", "name": "AXI SRAM (384 KB)",
                             "properties": [["Buffer 1", "IMU SPI1 DMA Ring"], ["Buffer 2", "CRSF UART2 DMA"], ["Buffer 3", "Blackbox Cache"]]},
                            {"id": "flash", "name": "QSPI Flash (16 MB)",
                             "properties": [["Chip", "W25Q128JV"], ["Mode", "Quad-SPI XiP"], ["Sectors", "4 KB Subsector"]]}
                        ]
                    }
                ],
                "connections": [
                    {"from": "c0", "to": "dma_sram", "label": "Direct DMA FIFO"},
                    {"from": "c1", "to": "dma_sram", "label": "HSEM IPC Lock"},
                    {"from": "c1", "to": "flash", "label": "Log Flush"}
                ]
            }"#,
        )
        .unwrap()
    }

    fn routing_layout(
        renderer: &ArchitectureRenderer<'_>,
    ) -> (Canvas, HashMap<String, BoxBounds>, Vec<Rect>) {
        let sizes: Vec<_> = renderer
            .spec
            .containers
            .iter()
            .map(|c| renderer.measure_container(c))
            .collect();
        let width = sizes.iter().map(|(w, _)| *w).max().unwrap();
        let height = sizes.iter().map(|(_, h)| h + 4).sum::<usize>() + 4;
        let mut canvas = Canvas::new(width + 4, height);
        let mut bounds = HashMap::new();
        let mut containers = Vec::new();
        let mut y = 2;
        for (c, (_, h)) in renderer.spec.containers.iter().zip(sizes) {
            renderer.render_container(
                &mut canvas,
                c,
                Rect::new(0, y, width, h),
                &mut bounds,
                &mut containers,
            );
            y += h + 4;
        }
        (canvas, bounds, containers)
    }

    fn assert_route_clearance(route: &Routed, unrelated: Rect) {
        for &(x, y) in &route.cells {
            assert!(!unrelated.contains_point(x, y), "route enters sibling box");
        }
        for pair in route.cells.windows(2) {
            let [(x0, y0), (x1, y1)] = pair else {
                unreachable!();
            };
            if x0 == x1 {
                let near_wall =
                    x0.abs_diff(unrelated.x) == 1 || x0.abs_diff(unrelated.right()) == 1;
                let overlaps = (*y0).min(*y1) <= unrelated.bottom() + 1
                    && (*y0).max(*y1) + 1 >= unrelated.y;
                assert!(!near_wall || !overlaps, "vertical route abuts sibling");
            } else {
                let near_wall =
                    y0.abs_diff(unrelated.y) == 1 || y0.abs_diff(unrelated.bottom()) == 1;
                let overlaps = (*x0).min(*x1) <= unrelated.right() + 1
                    && (*x0).max(*x1) + 1 >= unrelated.x;
                assert!(!near_wall || !overlaps, "horizontal route abuts sibling");
            }
        }
        assert_eq!(
            step(route.dst_anchor, route.dst_dir),
            route.cells.last().copied(),
            "route must terminate immediately beside its target anchor"
        );
        let (dx, dy) = dir_delta(route.dst_dir);
        assert_eq!(dir_delta(route.arrow), (-dx, -dy));
    }

    #[test]
    fn test_firmware_outer_widths_labels_and_sibling_clearance() {
        let spec = firmware_spec();
        for style in [
            BoxStyle::Rounded,
            BoxStyle::Sharp,
            BoxStyle::Double,
            BoxStyle::Heavy,
            BoxStyle::Ascii,
        ] {
            let renderer = ArchitectureRenderer::new(&spec, Theme::new(style));
            let width = spec
                .containers
                .iter()
                .map(|c| renderer.measure_container(c).0)
                .max()
                .unwrap();
            let out = renderer.render(false).unwrap();
            for c in &spec.containers {
                let border = out.lines().find(|line| line.contains(&c.title)).unwrap();
                assert_eq!(UnicodeWidthStr::width(border), width, "{out}");
                for item in &c.items {
                    if let ContainerItem::Leaf(leaf) = item {
                        assert!(out.contains(&leaf.name), "{out}");
                        for (key, value) in &leaf.properties {
                            assert!(out.contains(&format!("{key}: {value}")), "{out}");
                        }
                    }
                }
            }
            for conn in &spec.connections {
                assert_eq!(out.matches(conn.label.as_deref().unwrap()).count(), 1, "{out}");
            }
            let (canvas, bounds, containers) = routing_layout(&renderer);
            let grid = RouteGrid::new(canvas.width, canvas.height, &containers, &bounds, &canvas);
            let rect = |id: &str| {
                let b = &bounds[id];
                Rect::new(b.x, b.y, b.width, b.height)
            };
            for conn in &spec.connections {
                let route = grid.route(rect(&conn.from), rect(&conn.to)).unwrap();
                for id in bounds.keys() {
                    if id != &conn.from && id != &conn.to {
                        assert_route_clearance(&route, rect(id));
                    }
                }
            }
        }
    }

    #[test]
    fn test_outer_width_normalization_does_not_widen_nested_column() {
        let spec: ArchitectureSpec = serde_json::from_str(
            r#"{
                "containers": [
                    {"id": "small", "title": "Small", "layout": "column",
                     "items": [{"id": "nested", "title": "Nested", "layout": "column",
                                "items": [{"id": "a", "name": "Alpha"}, {"id": "b", "name": "Beta"}]}]},
                    {"id": "wide", "title": "A much wider neighboring outer container",
                     "items": [{"id": "c", "name": "Gamma"}]}
                ],
                "connections": [{"from": "a", "to": "b", "label": "Local"}]
            }"#,
        )
        .unwrap();
        let renderer = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false).unwrap();
        let max_width = spec
            .containers
            .iter()
            .map(|c| renderer.measure_container(c).0)
            .max()
            .unwrap();
        for c in &spec.containers {
            let border = out.lines().find(|line| line.contains(&c.title)).unwrap();
            assert_eq!(UnicodeWidthStr::width(border), max_width, "{out}");
        }
        let ContainerItem::SubContainer(nested) = &spec.containers[0].items[0] else {
            panic!("nested fixture must contain a container");
        };
        let nested_width = renderer.measure_container(nested).0;
        let border = out.lines().find(|line| line.contains("Nested")).unwrap();
        assert_eq!(border.chars().position(|ch| ch == '╭'), Some(2));
        assert_eq!(
            border.chars().position(|ch| ch == '╮'),
            Some(2 + nested_width - 1)
        );
        assert!(nested_width < max_width);
        assert_eq!(out.matches("Local").count(), 1, "{out}");
    }

    #[test]
    fn test_neighboring_and_nested_route_clearance() {
        for (layout, nested) in [("column", false), ("column", true), ("row", false)] {
            let items = r#"[
                {"id": "a", "name": "Source"},
                {"id": "sibling", "name": "Neighbor", "properties": [["Keep", "intact"]]},
                {"id": "b", "name": "Target"}
            ]"#;
            let items = if nested {
                format!(
                    r#"[{{"id": "inner", "title": "Inner", "layout": "{layout}", "items": {items}}}]"#
                )
            } else {
                items.to_string()
            };
            let spec: ArchitectureSpec = serde_json::from_str(&format!(
                r#"{{
                    "containers": [{{"id": "outer", "title": "Outer", "layout": "{layout}", "items": {items}}}],
                    "connections": [{{"from": "a", "to": "b", "label": "Around"}}]
                }}"#
            ))
            .unwrap();
            let renderer = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
            let (canvas, bounds, containers) = routing_layout(&renderer);
            let grid = RouteGrid::new(canvas.width, canvas.height, &containers, &bounds, &canvas);
            let rect = |id: &str| {
                let b = &bounds[id];
                Rect::new(b.x, b.y, b.width, b.height)
            };
            let route = grid.route(rect("a"), rect("b")).unwrap();
            assert_route_clearance(&route, rect("sibling"));
            let out = renderer.render(false).unwrap();
            for text in ["Source", "Neighbor", "Target", "Keep: intact", "Around"] {
                assert_eq!(out.matches(text).count(), 1, "{out}");
            }
        }
    }

    #[test]
    fn test_architecture_diagram() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Kubernetes Cluster".to_string()),
            containers: vec![ContainerSpec {
                id: "k8s".to_string(),
                title: "Namespace: Production".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "frontend".to_string(),
                        name: "Frontend".to_string(),
                        properties: vec![("Port".to_string(), "80".to_string())],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "backend".to_string(),
                        name: "Backend API".to_string(),
                        properties: vec![("Port".to_string(), "8080".to_string())],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "frontend".to_string(),
                to: "backend".to_string(),
                label: Some("HTTP".to_string()),
                arrow: crate::schema::ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let renderer = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false).unwrap();
        println!("ARCHITECTURE OUTPUT:\n{out}");
        assert!(out.contains("Kubernetes Cluster"));
        assert!(out.contains("Namespace: Production"));
        assert!(out.contains("Frontend"));
        assert!(out.contains("Backend API"));
        assert!(out.contains("Port: 80"));
        assert!(out.contains("►"));
    }

    #[test]
    fn test_lr_label_stays_inside_container() {
        let make_spec = |label: Option<String>| ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Service Mesh".to_string()),
            containers: vec![ContainerSpec {
                id: "c1".to_string(),
                title: "Cluster".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "a".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "b".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "a".to_string(),
                to: "b".to_string(),
                label,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let with_lbl = ArchitectureRenderer::new(
            &make_spec(Some("HTTP".to_string())),
            Theme::new(BoxStyle::Rounded),
        )
        .render(false)
        .unwrap();
        let no_lbl = ArchitectureRenderer::new(&make_spec(None), Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();

        // Label appears exactly once
        assert_eq!(with_lbl.matches("HTTP").count(), 1);

        let lines_with: Vec<&str> = with_lbl.lines().collect();
        let lines_no: Vec<&str> = no_lbl.lines().collect();
        assert_eq!(lines_with.len(), lines_no.len());

        let top_border_idx = lines_with
            .iter()
            .position(|l| l.contains("Cluster"))
            .unwrap();
        let lbl_idx = lines_with.iter().position(|l| l.contains("HTTP")).unwrap();
        let bottom_border_idx = lines_with
            .iter()
            .rposition(|l| l.contains('╰') && l.contains('╯'))
            .unwrap();

        // Label must be strictly inside container bounds
        assert!(lbl_idx > top_border_idx);
        assert!(lbl_idx < bottom_border_idx);

        // Title line untouched
        assert_eq!(lines_with[top_border_idx], lines_no[top_border_idx]);

        // Glyph diff: difference only on the label line
        for (i, (w, n)) in lines_with.iter().zip(lines_no.iter()).enumerate() {
            if i == lbl_idx {
                assert_ne!(w, n);
            } else {
                assert_eq!(w, n, "line {i} differs unexpectedly");
            }
        }
    }

    #[test]
    fn test_label_never_escapes_above_container() {
        let spec_with_lbl = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("T".to_string()),
            containers: vec![ContainerSpec {
                id: "c1".to_string(),
                title: "ColumnContainer".to_string(),
                layout: ContainerLayout::Column,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "alpha".to_string(),
                to: "beta".to_string(),
                label: Some("HTTP".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let mut spec_no_lbl = spec_with_lbl.clone();
        spec_no_lbl.connections[0].label = None;

        let out_with = ArchitectureRenderer::new(&spec_with_lbl, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();
        let out_no = ArchitectureRenderer::new(&spec_no_lbl, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();

        let lines_with: Vec<&str> = out_with.lines().collect();
        let lines_no: Vec<&str> = out_no.lines().collect();

        // (1) Output line 0 == title row of no-label render
        assert_eq!(lines_with[0], lines_no[0]);

        // Find container top border index
        let container_top_idx = lines_with
            .iter()
            .position(|l| l.contains("ColumnContainer"))
            .unwrap();

        // (2) Every label line index > container top border line index (if label rendered)
        // (3) No line above or at container top border contains label text
        for (idx, line) in lines_with.iter().enumerate() {
            if idx <= container_top_idx {
                assert!(
                    !line.contains("HTTP"),
                    "Label found at or above top border on line {idx}"
                );
            }
        }

        // Second spec: stacked containers
        let stacked_spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Stacked".to_string()),
            containers: vec![
                ContainerSpec {
                    id: "c1".to_string(),
                    title: "First".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "a".to_string(),
                        name: "A".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
                ContainerSpec {
                    id: "c2".to_string(),
                    title: "Second".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "b".to_string(),
                        name: "B".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
            ],
            connections: vec![EdgeSpec {
                from: "a".to_string(),
                to: "b".to_string(),
                label: Some("CALL".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let stacked_out = ArchitectureRenderer::new(&stacked_spec, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();
        let first_top_idx = stacked_out
            .lines()
            .position(|l| l.contains("First"))
            .unwrap();
        for (idx, line) in stacked_out.lines().enumerate() {
            if idx <= first_top_idx {
                assert!(!line.contains("CALL"));
            }
        }
    }

    #[test]
    fn test_tb_label_does_not_sever_border() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![ContainerSpec {
                id: "c".to_string(),
                title: "Box".to_string(),
                layout: ContainerLayout::Column,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "alpha".to_string(),
                to: "beta".to_string(),
                label: Some("DATA".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let out = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();
        let lines: Vec<&str> = out.lines().collect();

        // Alpha bottom border has full corner pair
        let alpha_bottom = lines.iter().find(|l| l.contains('╰') && l.contains('╯'));
        assert!(alpha_bottom.is_some());
        assert!(!alpha_bottom.unwrap().contains("DATA"));

        // If label is present, it is at most once and never on a border row
        let lbl_count = out.matches("DATA").count();
        assert!(lbl_count <= 1);
        if let Some(lbl_idx) = lines.iter().position(|l| l.contains("DATA")) {
            let line = lines[lbl_idx];
            assert!(!line.contains('─'));
        }
    }

    #[test]
    fn test_rtl_wrap_label_stays_inside_container() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![ContainerSpec {
                id: "c".to_string(),
                title: "RowContainer".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "beta".to_string(),
                to: "alpha".to_string(),
                label: Some("SYNC".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let out = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();
        let lines: Vec<&str> = out.lines().collect();

        // Container bottom border is the last non-empty line
        let bottom_border_idx = lines
            .iter()
            .rposition(|l| l.contains('╰') && l.contains('╯'))
            .unwrap();

        if let Some(lbl_idx) = lines.iter().position(|l| l.contains("SYNC")) {
            assert!(
                lbl_idx < bottom_border_idx,
                "Label on or below container bottom border"
            );
        }

        // Bottom border has full corner pair and is intact
        let b_line = lines[bottom_border_idx];
        assert!(b_line.contains('╰') && b_line.contains('╯'));
        assert!(!b_line.contains("SYNC"));
    }

    #[test]
    fn test_rtl_label_non_bottom_row_unchanged() {
        // Taller container where Alpha and Beta have a taller sibling Gamma
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![ContainerSpec {
                id: "c".to_string(),
                title: "TallerContainer".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "gamma".to_string(),
                        name: "Gamma".to_string(),
                        properties: vec![
                            ("P1".to_string(), "V1".to_string()),
                            ("P2".to_string(), "V2".to_string()),
                            ("P3".to_string(), "V3".to_string()),
                        ],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "beta".to_string(),
                to: "alpha".to_string(),
                label: Some("SYNC".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
                ..EdgeSpec::default()
            }],
        };

        let out = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap();
        assert!(out.contains("SYNC"));
    }

    #[test]
    fn test_connection_problems_are_hard_errors() {
        let comps: HashSet<String> = ["frontend".to_string(), "backend".to_string()]
            .into_iter()
            .collect();
        let conts: HashSet<String> = ["k8s".to_string()].into_iter().collect();
        let edge = |from: &str, to: &str| EdgeSpec {
            from: from.to_string(),
            to: to.to_string(),
            ..EdgeSpec::default()
        };

        let ok = validate_connections(&[edge("frontend", "backend")], &comps, &conts);
        assert!(ok.is_ok(), "valid connection must pass: {ok:?}");

        let err = validate_connections(
            &[
                edge("frontend", "ghost"),
                edge("frontend", "frontend"),
                edge("frontend", "k8s"),
                edge("ghost", "backend"),
            ],
            &comps,
            &conts,
        )
        .unwrap_err();
        // Each problem names its connection and the offending id
        assert!(
            err.contains("'ghost'") && err.contains("unknown component ID"),
            "{err}"
        );
        assert!(err.contains("self-loop connection on 'frontend'"), "{err}");
        assert!(
            err.contains("'k8s'") && err.contains("container ID"),
            "{err}"
        );
        assert!(
            err.contains("connection 0") && err.contains("connection 1"),
            "{err}"
        );
    }

    #[test]
    fn test_empty_containers_hard_error() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Empty".to_string()),
            containers: vec![],
            connections: vec![],
        };
        let err = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap_err();
        assert!(
            err.contains("architecture requires at least one container"),
            "{err}"
        );
    }

    #[test]
    fn test_duplicate_ids_hard_error() {
        // Leaf vs container id collision (ARCH-E-05 repro shape)
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![
                ContainerSpec {
                    id: "c1".to_string(),
                    title: "A".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "api".to_string(),
                        name: "API".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
                ContainerSpec {
                    id: "api".to_string(),
                    title: "Dup".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![],
                },
            ],
            connections: vec![EdgeSpec {
                from: "api".to_string(),
                to: "c1".to_string(),
                ..EdgeSpec::default()
            }],
        };
        let err = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap_err();
        assert!(err.contains("duplicate ID 'api'"), "{err}");
        assert!(
            err.contains("container") && err.contains("component"),
            "{err}"
        );

        // Duplicate leaf ids across containers are rejected too
        let dup_leaf = ArchitectureSpec {
            containers: vec![
                ContainerSpec {
                    id: "c1".to_string(),
                    title: "A".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "x".to_string(),
                        name: "X1".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
                ContainerSpec {
                    id: "c2".to_string(),
                    title: "B".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "x".to_string(),
                        name: "X2".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
            ],
            ..ArchitectureSpec::default()
        };
        let err = ArchitectureRenderer::new(&dup_leaf, Theme::new(BoxStyle::Rounded))
            .render(false)
            .unwrap_err();
        assert!(err.contains("duplicate ID 'x'"), "{err}");
    }
}

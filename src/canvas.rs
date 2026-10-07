use crate::color::Color;
use crate::theme::Theme;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "north/south/east/west line connections are the domain model"
)]
pub struct LineConn {
    pub north: bool,
    pub south: bool,
    pub east: bool,
    pub west: bool,
}

impl LineConn {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.north && !self.south && !self.east && !self.west
    }

    pub fn merge(&mut self, other: LineConn) {
        self.north |= other.north;
        self.south |= other.south;
        self.east |= other.east;
        self.west |= other.west;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellRole {
    Empty,
    Line,
    Border,
    Text,
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    #[must_use]
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn contains_point(&self, px: usize, py: usize) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    #[must_use]
    pub fn right(&self) -> usize {
        self.x + self.width.saturating_sub(1)
    }

    #[must_use]
    pub fn bottom(&self) -> usize {
        self.y + self.height.saturating_sub(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub is_continuation: bool,
    pub conn: LineConn,
    pub is_line: bool,
    /// Set on cells belonging to a thick (`==>`-style) edge run; picks heavy
    /// glyphs at render time.
    pub thick: bool,
    pub custom_corner: Option<char>,
    pub role: CellRole,
    /// Emphasis color for border/line/arrow cells; `None` on text cells.
    pub color: Option<Color>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            is_continuation: false,
            conn: LineConn::default(),
            is_line: false,
            thick: false,
            custom_corner: None,
            role: CellRole::Empty,
            color: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Canvas {
    /// Row-major flat grid, len == width * height. One allocation, cache-friendly.
    cells: Vec<Cell>,
    pub width: usize,
    pub height: usize,
    pub obstacles: Vec<Rect>,
    /// Current pen color: stamped onto border/line/arrow cells by draw calls;
    /// text cells are never stamped. Set around node/edge drawing.
    pen: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Canvas {
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![Cell::default(); width * height],
            width,
            height,
            obstacles: Vec::new(),
            pen: None,
        }
    }

    /// Sets the pen color for subsequent border/line/arrow writes.
    pub fn set_pen(&mut self, color: Option<Color>) {
        self.pen = color;
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn ensure_capacity(&mut self, x: usize, y: usize) {
        let required_h = y + 1;
        let required_w = x + 1;

        if required_h <= self.height && required_w <= self.width {
            return;
        }

        if required_w > self.width {
            // ponytail: double width on growth — amortizes the O(H*W) stride rebuild;
            // surplus columns are invisible (render trims to content).
            let new_w = required_w.max(self.width.saturating_mul(2));
            let new_h = required_h.max(self.height);
            let mut new_cells = vec![Cell::default(); new_w * new_h];
            for old_y in 0..self.height {
                let src = old_y * self.width;
                let dst = old_y * new_w;
                new_cells[dst..dst + self.width]
                    .copy_from_slice(&self.cells[src..src + self.width]);
            }
            self.cells = new_cells;
            self.width = new_w;
            self.height = new_h;
        } else {
            // Height-only growth: stride unchanged, extend in place.
            self.cells.resize(self.width * required_h, Cell::default());
            self.height = required_h;
        }
    }

    #[must_use]
    pub fn get_cell(&self, x: usize, y: usize) -> Option<&Cell> {
        if y < self.height && x < self.width {
            Some(&self.cells[self.idx(x, y)])
        } else {
            None
        }
    }

    pub fn get_cell_mut(&mut self, x: usize, y: usize) -> &mut Cell {
        self.ensure_capacity(x, y);
        let i = self.idx(x, y);
        &mut self.cells[i]
    }

    pub fn add_obstacle(&mut self, rect: Rect) {
        self.obstacles.push(rect);
    }

    #[must_use]
    pub fn is_point_in_obstacle(&self, x: usize, y: usize) -> bool {
        self.obstacles.iter().any(|r| r.contains_point(x, y))
    }

    pub fn put_char(&mut self, x: usize, y: usize, ch: char) {
        self.put_char_with_role(x, y, ch, CellRole::Text);
    }

    pub fn put_char_with_role(&mut self, x: usize, y: usize, ch: char, role: CellRole) {
        let w = ch.width().unwrap_or(1);
        if w == 0 {
            return;
        }
        self.ensure_capacity(x + w - 1, y);

        let i = self.idx(x, y);
        let cell = &mut self.cells[i];
        cell.ch = ch;
        cell.is_line = role == CellRole::Line;
        cell.is_continuation = false;
        cell.custom_corner = None;
        cell.role = role;
        // Text stays terminal-default; emphasis colors ride on glyphs only
        cell.color = if role == CellRole::Text {
            None
        } else {
            self.pen
        };

        if w > 1 {
            for offset in 1..w {
                let i = self.idx(x + offset, y);
                let cont_cell = &mut self.cells[i];
                cont_cell.ch = ' ';
                cont_cell.is_continuation = true;
                cont_cell.is_line = false;
                cont_cell.role = role;
            }
        }
    }

    pub fn draw_text(&mut self, start_x: usize, y: usize, text: &str) {
        let mut cur_x = start_x;
        for ch in text.chars() {
            let w = ch.width().unwrap_or(1);
            if w == 0 {
                continue;
            }
            self.put_char_with_role(cur_x, y, ch, CellRole::Text);
            cur_x += w;
        }
    }

    /// Checks if a string of text can be safely placed starting at (`start_x`, y)
    /// without colliding with box borders, arrowheads, obstacle interiors, or existing text.
    #[must_use]
    pub fn can_place_text(&self, start_x: usize, y: usize, text: &str) -> bool {
        if y >= self.height {
            return true;
        }
        let text_w = UnicodeWidthStr::width(text);
        let base = y * self.width;
        for offset in 0..text_w {
            let x = start_x + offset;
            if x < self.width {
                let cell = &self.cells[base + x];
                // Do not overwrite borders
                if cell.role == CellRole::Border {
                    return false;
                }
                // Do not overwrite arrowheads
                if cell.role == CellRole::Arrow
                    || matches!(cell.ch, '►' | '◄' | '▼' | '▲' | '>' | '<' | 'v' | '^')
                {
                    return false;
                }
                // Do not overwrite vertical lines or junctions
                if cell.is_line && (cell.conn.north || cell.conn.south)
                    || matches!(cell.ch, '│' | '|' | '║' | '┆' | '┊' | '╎' | '╏')
                {
                    return false;
                }
                // Do not place labels inside horizontal line or dash runs — a label
                // drawn mid-run reads as merged with the crossing edge
                if cell.is_line || matches!(cell.ch, '─' | '-' | '╌' | '┄' | '━' | '═') {
                    return false;
                }
                // Do not overwrite existing non-space text
                if cell.role == CellRole::Text && cell.ch != ' ' {
                    return false;
                }
            }
        }
        // Require at least one blank cell between the label and any neighbouring
        // text so parallel sibling edge labels never render back-to-back
        if y < self.height && text_w > 0 {
            let base = y * self.width;
            if let Some(x) = start_x.checked_sub(1)
                && let Some(cell) = self.cells.get(base + x)
                && cell.role == CellRole::Text
                && cell.ch != ' '
            {
                return false;
            }
            if let Some(cell) = self.cells.get(base + start_x + text_w)
                && cell.role == CellRole::Text
                && cell.ch != ' '
            {
                return false;
            }
        }
        // Obstacle interiors and borders: prevent placing text over nodes (including borders)
        if text_w > 0 {
            let span_end = start_x + text_w - 1;
            for obs in &self.obstacles {
                if y >= obs.y && y <= obs.bottom() && span_end >= obs.x && start_x <= obs.right() {
                    return false;
                }
            }
        }
        true
    }

    /// Finds the closest collision-free position for text around (`preferred_x`, `preferred_y`).
    #[must_use]
    pub fn find_safe_text_pos(
        &self,
        preferred_x: usize,
        preferred_y: usize,
        text: &str,
    ) -> (usize, usize) {
        if self.can_place_text(preferred_x, preferred_y, text) {
            return (preferred_x, preferred_y);
        }

        // Test nearby offsets: 1 row up, 1 row down, side shifts
        let offsets: &[(isize, isize)] = &[
            (0, -1),
            (0, 1),
            (-1, 0),
            (1, 0),
            (-2, 0),
            (2, 0),
            (-3, 0),
            (3, 0),
            (-4, 0),
            (4, 0),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
            (0, -2),
            (0, 2),
            (-2, -1),
            (2, -1),
            (-2, 1),
            (2, 1),
            (-5, 0),
            (5, 0),
            (-6, 0),
            (6, 0),
            (0, -3),
            (0, 3),
            (-2, -2),
            (2, -2),
            (-2, 2),
            (2, 2),
            (-3, -1),
            (3, -1),
            (-3, 1),
            (3, 1),
            (-1, -2),
            (1, -2),
            (-1, 2),
            (1, 2),
            (-7, 0),
            (7, 0),
            (-8, 0),
            (8, 0),
            (0, -4),
            (0, 4),
        ];

        for &(dx, dy) in offsets {
            let Some(test_x) = offset_pos(preferred_x, dx) else {
                continue;
            };
            let Some(test_y) = offset_pos(preferred_y, dy) else {
                continue;
            };
            if self.can_place_text(test_x, test_y, text) {
                return (test_x, test_y);
            }
        }

        (preferred_x, preferred_y)
    }

    /// Places text safely using collision detection, shifting if an obstacle or border is in the way.
    pub fn draw_text_safe(
        &mut self,
        preferred_x: usize,
        preferred_y: usize,
        text: &str,
    ) -> (usize, usize) {
        let (safe_x, safe_y) = self.find_safe_text_pos(preferred_x, preferred_y, text);
        self.draw_text(safe_x, safe_y, text);
        (safe_x, safe_y)
    }

    pub fn draw_hline(&mut self, x1: usize, x2: usize, y: usize) {
        self.draw_hline_thick(x1, x2, y, false);
    }

    /// Thick horizontal line (heavy glyphs at render time).
    pub fn draw_thick_hline(&mut self, x1: usize, x2: usize, y: usize) {
        self.draw_hline_thick(x1, x2, y, true);
    }

    fn draw_hline_thick(&mut self, x1: usize, x2: usize, y: usize, thick: bool) {
        if x1 > x2 {
            return self.draw_hline_thick(x2, x1, y, thick);
        }
        self.ensure_capacity(x2, y);
        let base = y * self.width;

        for x in x1..=x2 {
            // Collision protection: don't overwrite text with a line; keep a
            // one-cell gap on each side so the text never reads as merged
            if self.cells[base + x].role == CellRole::Text && self.cells[base + x].ch != ' ' {
                continue;
            }
            if x > x1 && self.cells[base + x - 1].role == CellRole::Text {
                continue; // pre-gap after text
            }
            if x < x2 && self.cells[base + x + 1].role == CellRole::Text {
                continue; // pre-gap before text
            }
            let cell = &mut self.cells[base + x];
            cell.is_line = true;
            cell.thick = thick;
            if cell.role != CellRole::Border {
                cell.role = CellRole::Line;
                cell.color = self.pen;
            }
            if x > x1 {
                cell.conn.west = true;
            }
            if x < x2 {
                cell.conn.east = true;
            }
        }
    }

    pub fn draw_dashed_hline(&mut self, x1: usize, x2: usize, y: usize, theme: &Theme) {
        if x1 > x2 {
            return self.draw_dashed_hline(x2, x1, y, theme);
        }
        self.ensure_capacity(x2, y);
        let base = y * self.width;

        let dash_char = if theme.box_style == crate::theme::BoxStyle::Ascii {
            '-'
        } else {
            '╌'
        };

        for x in x1..=x2 {
            if self.cells[base + x].role == CellRole::Text && self.cells[base + x].ch != ' ' {
                continue;
            }
            if x > x1 && self.cells[base + x - 1].role == CellRole::Text {
                continue; // pre-gap after text
            }
            if x < x2 && self.cells[base + x + 1].role == CellRole::Text {
                continue; // pre-gap before text
            }
            let cell = &mut self.cells[base + x];
            cell.ch = dash_char;
            cell.is_line = false;
            if cell.role != CellRole::Border {
                cell.color = self.pen;
            }
        }
    }

    /// Vertical dashed line (used by flowchart `-.->` edges).
    pub fn draw_dashed_vline(&mut self, x: usize, y1: usize, y2: usize, theme: &Theme) {
        if y1 > y2 {
            return self.draw_dashed_vline(x, y2, y1, theme);
        }
        self.ensure_capacity(x, y2);

        let dash_char = if theme.box_style == crate::theme::BoxStyle::Ascii {
            '|'
        } else {
            '┆'
        };

        for y in y1..=y2 {
            let cell = &mut self.cells[y * self.width + x];
            if cell.role == CellRole::Text && cell.ch != ' ' {
                continue;
            }
            cell.ch = dash_char;
            cell.is_line = false;
            if cell.role != CellRole::Border {
                cell.color = self.pen;
            }
        }
    }

    pub fn draw_vline(&mut self, x: usize, y1: usize, y2: usize) {
        self.draw_vline_thick(x, y1, y2, false);
    }

    /// Thick vertical line (heavy glyphs at render time).
    pub fn draw_thick_vline(&mut self, x: usize, y1: usize, y2: usize) {
        self.draw_vline_thick(x, y1, y2, true);
    }

    fn draw_vline_thick(&mut self, x: usize, y1: usize, y2: usize, thick: bool) {
        if y1 > y2 {
            return self.draw_vline_thick(x, y2, y1, thick);
        }
        self.ensure_capacity(x, y2);

        for y in y1..=y2 {
            let cell = &mut self.cells[y * self.width + x];
            // Collision protection: don't overwrite text with a line
            if cell.role == CellRole::Text && cell.ch != ' ' {
                continue;
            }
            cell.is_line = true;
            cell.thick = thick;
            if cell.role != CellRole::Border {
                cell.role = CellRole::Line;
                cell.color = self.pen;
            }
            if y > y1 {
                cell.conn.north = true;
            }
            if y < y2 {
                cell.conn.south = true;
            }
        }
    }

    pub fn draw_corner(&mut self, x: usize, y: usize, conn: LineConn) {
        self.ensure_capacity(x, y);
        let i = self.idx(x, y);
        let cell = &mut self.cells[i];
        cell.is_line = true;
        if cell.role != CellRole::Border {
            cell.role = CellRole::Line;
            cell.color = self.pen;
        }
        cell.conn.merge(conn);
    }

    pub fn draw_arrow(&mut self, x: usize, y: usize, dir: Direction, theme: &Theme) {
        let ch = match dir {
            Direction::Right => theme.arrow_right(),
            Direction::Left => theme.arrow_left(),
            Direction::Down => theme.arrow_down(),
            Direction::Up => theme.arrow_up(),
        };
        self.put_char_with_role(x, y, ch, CellRole::Arrow);
    }

    pub fn draw_box(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        theme: &Theme,
        title: Option<&str>,
    ) {
        self.draw_box_inner(x, y, width, height, theme, title, false);
    }

    /// Dashed-border box (Mermaid `class`/`style` with `stroke-dasharray`).
    pub fn draw_dashed_box(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        theme: &Theme,
        title: Option<&str>,
    ) {
        self.draw_box_inner(x, y, width, height, theme, title, true);
    }

    /// Draws a styled rectangle box with optional title and content
    ///
    /// # Panics
    ///
    /// Panics if the box coordinates overflow `usize`, or on allocation failure
    /// when the canvas must grow to fit the box.
    fn draw_box_inner(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        theme: &Theme,
        title: Option<&str>,
        dashed: bool,
    ) {
        if width < 2 || height < 2 {
            return;
        }

        let right = x + width - 1;
        let bottom = y + height - 1;

        self.ensure_capacity(right, bottom);

        // Clear interior of the box
        for row in (y + 1)..bottom {
            let base = row * self.width;
            for col in (x + 1)..right {
                let cell = &mut self.cells[base + col];
                // Full reset: stale color/role from earlier edge stamping would
                // otherwise survive as colored whitespace patches
                *cell = Cell::default();
            }
        }

        // Draw horizontal edges
        if dashed {
            self.draw_dashed_hline(x, right, y, theme);
            self.draw_dashed_hline(x, right, bottom, theme);
        } else {
            self.draw_hline(x, right, y);
            self.draw_hline(x, right, bottom);
        }

        // Draw vertical edges
        if dashed {
            self.draw_dashed_vline(x, y, bottom, theme);
            self.draw_dashed_vline(right, y, bottom, theme);
        } else {
            self.draw_vline(x, y, bottom);
            self.draw_vline(right, y, bottom);
        }

        // Corners
        self.draw_corner(
            x,
            y,
            LineConn {
                south: true,
                east: true,
                ..Default::default()
            },
        );
        self.draw_corner(
            right,
            y,
            LineConn {
                south: true,
                west: true,
                ..Default::default()
            },
        );
        self.draw_corner(
            x,
            bottom,
            LineConn {
                north: true,
                east: true,
                ..Default::default()
            },
        );
        // Register obstacle
        self.add_obstacle(Rect::new(x, y, width, height));

        // Mark borders (pen color wins over earlier edge lines here — border
        // cells were just drawn over the edge approach paths)
        let top_base = y * self.width;
        let bottom_base = bottom * self.width;
        for col in x..=right {
            let cell = &mut self.cells[top_base + col];
            cell.role = CellRole::Border;
            cell.color = self.pen;
            let cell = &mut self.cells[bottom_base + col];
            cell.role = CellRole::Border;
            cell.color = self.pen;
        }
        for row in y..=bottom {
            let cell = &mut self.cells[row * self.width + x];
            cell.role = CellRole::Border;
            cell.color = self.pen;
            let cell = &mut self.cells[row * self.width + right];
            cell.role = CellRole::Border;
            cell.color = self.pen;
        }

        // Title if present
        if let Some(t) = title
            && !t.is_empty()
            && width > 4
        {
            let padded_title = format!(" {t} ");
            let title_cols = UnicodeWidthChar::width(' ').unwrap() * 2
                + t.chars().map(|c| c.width().unwrap_or(1)).sum::<usize>();
            if title_cols < width - 2 {
                let title_x = x + (width - title_cols) / 2;
                self.draw_text(title_x, y, &padded_title);
            }
        }
    }

    /// Draws a diamond / decision block with diagonal corners and side points
    pub fn draw_diamond(&mut self, x: usize, y: usize, width: usize, height: usize, theme: &Theme) {
        if width < 4 || height < 3 {
            return;
        }

        let right = x + width - 1;
        let bottom = y + height - 1;

        self.ensure_capacity(right, bottom);

        // Clear interior of the diamond
        for row in (y + 1)..bottom {
            let base = row * self.width;
            for col in (x + 1)..right {
                let cell = &mut self.cells[base + col];
                // Full reset: stale color/role from earlier edge stamping would
                // otherwise survive as colored whitespace patches
                *cell = Cell::default();
            }
        }

        let (tl, tr, bl, br) = if theme.box_style == crate::theme::BoxStyle::Ascii {
            ('/', '\\', '\\', '/')
        } else {
            ('╱', '╲', '╲', '╱')
        };

        let mid_x = x + width / 2;
        let mid_y = y + height / 2;
        let half_h = height / 2;
        let half_w = width / 2;

        if height >= 5 {
            let (apex_n, apex_s) = if theme.box_style == crate::theme::BoxStyle::Ascii {
                ('^', 'v')
            } else {
                ('▲', '▼')
            };

            // Middle side vertices
            self.put_char(x, mid_y, '<');
            self.put_char(right, mid_y, '>');

            // North apex: place apex if not already occupied by incoming arrow
            let cell_above_is_incoming = y > 0
                && self
                    .get_cell(mid_x, y - 1)
                    .is_some_and(|c| c.ch == '▼' || c.ch == 'v' || c.is_line);

            let top_has_arrow = self
                .get_cell(mid_x, y)
                .is_some_and(|c| c.ch == '▼' || c.ch == 'v' || c.is_line);

            if cell_above_is_incoming {
                let v_line = if theme.box_style == crate::theme::BoxStyle::Ascii {
                    '|'
                } else {
                    '│'
                };
                self.put_char(mid_x, y - 1, v_line);
                let arrow_in = if theme.box_style == crate::theme::BoxStyle::Ascii {
                    'v'
                } else {
                    '▼'
                };
                self.put_char(mid_x, y, arrow_in);
            } else if !top_has_arrow {
                self.put_char(mid_x, y, apex_n);
            }

            // South apex: place apex if not already a line
            let cell_below_is_outgoing = self
                .get_cell(mid_x, bottom + 1)
                .is_some_and(|c| c.is_line || c.ch == '│' || c.ch == '|');

            let bot_has_line = self.get_cell(mid_x, bottom).is_some_and(|c| c.is_line);

            if cell_below_is_outgoing {
                let v_line = if theme.box_style == crate::theme::BoxStyle::Ascii {
                    '|'
                } else {
                    '│'
                };
                self.put_char(mid_x, bottom, v_line);
            } else if !bot_has_line {
                self.put_char(mid_x, bottom, apex_s);
            }

            // Equal-length diagonal sides (integer round-half-up of half_w * r / half_h)
            for r in 1..half_h {
                let dx = (half_w * r + half_h / 2) / half_h;
                let dx = dx.max(1);
                let c_left = mid_x.saturating_sub(dx).max(x + 1);
                let c_right = (mid_x + dx).min(right - 1);

                self.put_char(c_left, y + r, tl);
                self.put_char(c_right, y + r, tr);
                self.put_char(c_left, bottom - r, bl);
                self.put_char(c_right, bottom - r, br);
            }
        } else {
            // Chamfered 3-line fallback
            if right > x + 3 {
                self.draw_hline(x + 2, right - 2, y);
                self.draw_hline(x + 2, right - 2, bottom);
            }
            self.put_char(x + 1, y, tl);
            self.put_char(right - 1, y, tr);
            self.put_char(x + 1, bottom, bl);
            self.put_char(right - 1, bottom, br);
            self.put_char(x, mid_y, '<');
            self.put_char(right, mid_y, '>');
        }
    }

    /// Draws a custom continuous decision block that never breaks across terminal fonts.
    /// Uses unbroken box-drawing glyphs with an embedded decision diamond badge.
    pub fn draw_decision_box(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        theme: &Theme,
        badge: Option<&str>,
        dashed: bool,
    ) {
        if width < 4 || height < 2 {
            return;
        }

        let right = x + width - 1;
        let bottom = y + height - 1;

        self.ensure_capacity(right, bottom);

        // Clear interior
        for row in (y + 1)..bottom {
            let base = row * self.width;
            for col in (x + 1)..right {
                let cell = &mut self.cells[base + col];
                // Full reset: stale color/role from earlier edge stamping would
                // otherwise survive as colored whitespace patches
                *cell = Cell::default();
            }
        }

        let is_ascii = theme.box_style == crate::theme::BoxStyle::Ascii;
        let (tl, tr, bl, br, h_char, v_char) = match (is_ascii, dashed) {
            (true, true) => ('+', '+', '+', '+', '-', '|'),
            (true, false) => ('+', '+', '+', '+', '=', '#'),
            (false, true) => ('╌', '╌', '╌', '╌', '╌', '┆'),
            (false, false) => ('╔', '╗', '╚', '╝', '═', '║'),
        };

        // Draw horizontal edges
        for col in (x + 1)..right {
            self.put_char(col, y, h_char);
            self.put_char(col, bottom, h_char);
        }

        // Draw vertical edges
        for row in (y + 1)..bottom {
            self.put_char(x, row, v_char);
            self.put_char(right, row, v_char);
        }

        // Corners
        self.put_char(x, y, tl);
        self.put_char(right, y, tr);
        self.put_char(x, bottom, bl);
        self.put_char(right, bottom, br);

        // Register obstacle
        self.add_obstacle(Rect::new(x, y, width, height));

        // Mark borders
        let top_base = y * self.width;
        let bottom_base = bottom * self.width;
        for col in x..=right {
            let cell = &mut self.cells[top_base + col];
            cell.role = CellRole::Border;
            cell.color = self.pen;
            let cell = &mut self.cells[bottom_base + col];
            cell.role = CellRole::Border;
            cell.color = self.pen;
        }
        for row in y..=bottom {
            let cell = &mut self.cells[row * self.width + x];
            cell.role = CellRole::Border;
            cell.color = self.pen;
            let cell = &mut self.cells[row * self.width + right];
            cell.role = CellRole::Border;
            cell.color = self.pen;
        }

        // Connectors on boundaries:
        let mid_x = x + width / 2;
        let mid_y = y + height / 2;

        // Bottom connector junction if a line departs downward
        if !is_ascii {
            if self
                .get_cell(mid_x, bottom + 1)
                .is_some_and(|c| c.is_line || c.ch == '│')
            {
                // Border role so the pen color stamps these junction glyphs
                self.put_char_with_role(mid_x, bottom, '╧', CellRole::Border);
            }
            // Left connector if horizontal line touches
            if x > 0
                && self
                    .get_cell(x - 1, mid_y)
                    .is_some_and(|c| c.is_line || c.ch == '─' || c.ch == '►')
            {
                self.put_char_with_role(x, mid_y, '╟', CellRole::Border);
            }
            // Right connector if horizontal line touches
            if self
                .get_cell(right + 1, mid_y)
                .is_some_and(|c| c.is_line || c.ch == '─')
            {
                self.put_char_with_role(right, mid_y, '╢', CellRole::Border);
            }
        }

        // Embed badge on top border if provided
        if let Some(b) = badge {
            let badge_text = if is_ascii {
                String::from(" <?> ")
            } else {
                [" ", b, " "].concat()
            };
            let badge_w = UnicodeWidthStr::width(badge_text.as_str());
            if badge_w < width - 2 {
                let badge_x = x + (width - badge_w) / 2;
                self.draw_text(badge_x, y, &badge_text);
            }
        }
    }

    /// Render canvas to string using theme glyphs (no ANSI color codes).
    #[must_use]
    pub fn render(&self, theme: &Theme) -> String {
        self.render_impl(theme, false)
    }

    /// Render with per-cell emphasis colors as ANSI SGR runs. Escape codes are
    /// emitted after layout — they never affect column math.
    #[must_use]
    pub fn render_colored(&self, theme: &Theme) -> String {
        self.render_impl(theme, true)
    }

    pub fn render_impl(&self, theme: &Theme, colored: bool) -> String {
        if self.cells.is_empty() {
            return String::new();
        }

        // One pass: global bounding box + per-row extents
        let mut max_y = 0;
        let mut max_x = 0;
        let mut row_max_x = vec![0usize; self.height];
        let mut any_content = false;

        for (y, rm) in row_max_x.iter_mut().enumerate() {
            let base = y * self.width;
            for x in 0..self.width {
                let cell = &self.cells[base + x];
                if cell.ch != ' ' || cell.is_line || cell.is_continuation {
                    any_content = true;
                    *rm = x;
                    if x > max_x {
                        max_x = x;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }

        if !any_content
            || (max_x == 0 && max_y == 0 && self.cells[0].ch == ' ' && !self.cells[0].is_line)
        {
            return String::new();
        }

        // Build output directly with a single allocation
        let mut out = String::with_capacity((max_y + 1) * (max_x + 2));
        let mut current: Option<Color> = None;
        for (y, &rx) in row_max_x.iter().enumerate().take(max_y + 1) {
            let limit = rx.min(max_x);
            let base = y * self.width;
            for x in 0..=limit {
                let cell = &self.cells[base + x];
                if cell.is_continuation {
                    continue;
                }
                if colored && cell.color != current {
                    if current.is_some() {
                        out.push_str("\u{1b}[0m");
                    }
                    if let Some(c) = cell.color {
                        out.push_str(&format!("\u{1b}[{}m", c.sgr()));
                    }
                    current = cell.color;
                }
                if cell.is_line && cell.ch == ' ' {
                    out.push(resolve_line_glyph(cell.conn, theme, cell.thick));
                } else {
                    out.push(cell.ch);
                }
            }
            // Close any open color run before trimming/pushing the newline so
            // SGR state never leaks across rows
            if colored && current.is_some() {
                out.push_str("\u{1b}[0m");
                current = None;
            }
            // Trim trailing spaces of this line
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
        }
        // Trim trailing blank lines
        while out.ends_with('\n') {
            out.pop();
        }
        out
    }
}

fn offset_pos(base: usize, off: isize) -> Option<usize> {
    if off >= 0 {
        base.checked_add(off.unsigned_abs())
    } else {
        base.checked_sub(off.unsigned_abs())
    }
}

fn resolve_line_glyph(conn: LineConn, theme: &Theme, thick: bool) -> char {
    let (n, s, e, w) = (conn.north, conn.south, conn.east, conn.west);

    if thick {
        match (n, s, e, w) {
            // 4-way cross
            (true, true, true, true) => theme.thick_cross(),

            // 3-way tees
            (false, true, true, true) => theme.thick_tee_down(),
            (true, false, true, true) => theme.thick_tee_up(),
            (true, true, true, false) => theme.thick_tee_right(),
            (true, true, false, true) => theme.thick_tee_left(),

            // 2-way corners
            (false, true, true, false) => theme.thick_top_left_corner(),
            (false, true, false, true) => theme.thick_top_right_corner(),
            (true, false, true, false) => theme.thick_bottom_left_corner(),
            (true, false, false, true) => theme.thick_bottom_right_corner(),

            // Vertical line and stubs
            (true, true | false, false, false) | (false, true, false, false) => {
                theme.thick_vertical_line()
            }

            // Horizontal line and stubs
            (false, false, true, true | false) | (false, false, false, true) => {
                theme.thick_horizontal_line()
            }

            // Empty / none
            (false, false, false, false) => ' ',
        }
    } else {
        match (n, s, e, w) {
            // 4-way cross
            (true, true, true, true) => theme.cross(),

            // 3-way tees
            (false, true, true, true) => theme.tee_down(),
            (true, false, true, true) => theme.tee_up(),
            (true, true, true, false) => theme.tee_right(),
            (true, true, false, true) => theme.tee_left(),

            // 2-way corners
            (false, true, true, false) => theme.top_left_corner(),
            (false, true, false, true) => theme.top_right_corner(),
            (true, false, true, false) => theme.bottom_left_corner(),
            (true, false, false, true) => theme.bottom_right_corner(),

            // Vertical line and stubs
            (true, true | false, false, false) | (false, true, false, false) => {
                theme.vertical_line()
            }

            // Horizontal line and stubs
            (false, false, true, true | false) | (false, false, false, true) => {
                theme.horizontal_line()
            }

            // Empty / none
            (false, false, false, false) => ' ',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::theme::BoxStyle;

    /// Strips ANSI SGR sequences — used to prove color never shifts layout.
    fn strip_ansi(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn test_pen_colors_borders_not_text() {
        let mut canvas = Canvas::new(10, 5);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.set_pen(Some(Color::Red));
        canvas.draw_box(0, 0, 6, 3, &theme, None);
        canvas.draw_text(1, 1, "Hi");
        canvas.set_pen(None);
        let colored = canvas.render_colored(&theme);
        assert!(
            colored.contains("\u{1b}[31m╭"),
            "border colored: {colored:?}"
        );
        assert!(
            !colored.contains("[31mH"),
            "text never colored: {colored:?}"
        );
    }

    #[test]
    fn test_colored_strips_to_plain() {
        let mut canvas = Canvas::new(12, 5);
        let theme = Theme::new(BoxStyle::Sharp);
        canvas.set_pen(Some(Color::Green));
        canvas.draw_box(0, 0, 8, 3, &theme, None);
        canvas.set_pen(Some(Color::Hex(1, 2, 3)));
        canvas.draw_hline(1, 6, 4);
        canvas.set_pen(None);
        canvas.draw_text(1, 1, "ok");

        let plain = canvas.render(&theme);
        let colored = canvas.render_colored(&theme);
        assert!(!plain.contains('\u{1b}'), "plain render has no ANSI");
        assert!(colored.contains("\u{1b}[32m"), "green border present");
        assert!(colored.contains("\u{1b}[38;2;1;2;3m"), "hex line present");
        assert_eq!(strip_ansi(&colored), plain, "color never shifts layout");
    }

    #[test]
    fn test_edge_color_yields_to_border_at_junction() {
        let mut canvas = Canvas::new(14, 6);
        let theme = Theme::new(BoxStyle::Rounded);
        // Red edge drawn first, then green box on top at the junction cell
        canvas.set_pen(Some(Color::Red));
        canvas.draw_hline(0, 10, 3);
        canvas.set_pen(Some(Color::Green));
        canvas.draw_box(8, 2, 5, 3, &theme, None);
        canvas.set_pen(None);
        let colored = canvas.render_colored(&theme);
        // The box border glyphs (green) must not be recolored by the edge
        let border_line = colored
            .lines()
            .find(|l| l.contains('╭'))
            .expect("border row");
        assert!(
            border_line.contains("[32m"),
            "border stays green: {border_line:?}"
        );
        assert!(
            !border_line.contains("[31m"),
            "no red on border row: {border_line:?}"
        );
    }

    #[test]
    fn test_simple_box() {
        let mut canvas = Canvas::new(10, 5);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_box(0, 0, 6, 3, &theme, None);
        let output = canvas.render(&theme);
        assert_eq!(output, "╭────╮\n│    │\n╰────╯");
    }

    #[test]
    fn test_ascii_box() {
        let mut canvas = Canvas::new(10, 5);
        let theme = Theme::ascii();
        canvas.draw_box(0, 0, 6, 3, &theme, None);
        let output = canvas.render(&theme);
        assert_eq!(output, "+----+\n|    |\n+----+");
    }

    #[test]
    fn test_junction_t_down() {
        let mut canvas = Canvas::new(10, 5);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_hline(0, 4, 0);
        canvas.draw_vline(2, 0, 2);
        let output = canvas.render(&theme);
        assert_eq!(output, "──┬──\n  │\n  │");
    }

    #[test]
    fn test_cross_junction() {
        let mut canvas = Canvas::new(10, 5);
        let theme = Theme::new(BoxStyle::Sharp);
        canvas.draw_hline(0, 4, 2);
        canvas.draw_vline(2, 0, 4);
        let output = canvas.render(&theme);
        assert_eq!(output, "  │\n  │\n──┼──\n  │\n  │");
    }

    #[test]
    fn test_collision_detection_avoids_border() {
        let mut canvas = Canvas::new(15, 10);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_box(2, 2, 8, 4, &theme, None);
        assert!(!canvas.can_place_text(2, 2, "TEST"));
        let (safe_x, safe_y) = canvas.find_safe_text_pos(2, 2, "TEST");
        assert!(safe_y != 2 || safe_x != 2);
        assert!(canvas.can_place_text(safe_x, safe_y, "TEST"));
    }

    #[test]
    fn test_line_does_not_overwrite_text() {
        let mut canvas = Canvas::new(10, 5);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_text(1, 2, "DATA");
        // Vertical line passing through row 2
        canvas.draw_vline(2, 0, 4);
        let output = canvas.render(&theme);
        assert!(output.contains("DATA"));
    }

    #[test]
    fn test_collision_detection_avoids_bottom_border() {
        let mut canvas = Canvas::new(20, 10);
        let theme = Theme::new(BoxStyle::Rounded);
        // Box at x=2, y=2, width=8, height=4 -> bottom is y=5, right is x=9
        canvas.draw_box(2, 2, 8, 4, &theme, None);
        // Bottom border is at row 5
        assert!(!canvas.can_place_text(2, 5, "TEST"));
        assert!(!canvas.can_place_text(5, 5, "TEST"));
        // Top border is at row 2
        assert!(!canvas.can_place_text(2, 2, "TEST"));
        // Right border is at column 9
        assert!(!canvas.can_place_text(9, 3, "TEST"));
        // Left border is at column 2
        assert!(!canvas.can_place_text(2, 3, "TEST"));

        // Unrendered obstacle (e.g. before nodes are drawn) also protected via add_obstacle
        let mut canvas2 = Canvas::new(20, 10);
        canvas2.add_obstacle(Rect::new(2, 2, 8, 4));
        assert!(!canvas2.can_place_text(2, 5, "TEST"));
        assert!(!canvas2.can_place_text(5, 5, "TEST"));
        assert!(!canvas2.can_place_text(2, 2, "TEST"));
    }

    #[test]
    fn test_collision_detection_avoids_dashed_vline() {
        let mut canvas = Canvas::new(20, 10);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_dashed_vline(5, 1, 6, &theme);
        // Placing text at x=4 spanning through x=5 ('B' hits dashed vline)
        assert!(!canvas.can_place_text(4, 3, "ABC"));
    }
}

#[cfg(test)]
mod stride_tests {
    use super::*;
    use crate::theme::BoxStyle;

    /// Regression: height-only growth must keep the row stride. The old code
    /// doubled `width` here without rebuilding, scrambling all pre-growth cells.
    #[test]
    fn test_height_growth_keeps_stride() {
        let mut canvas = Canvas::new(20, 5);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_text(2, 1, "OLD");
        // Force height growth well past the initial height.
        canvas.draw_text(3, 40, "NEW");
        let out = canvas.render(&theme);
        let row1 = out.lines().nth(1).unwrap();
        assert_eq!(
            &row1[2..5],
            "OLD",
            "pre-growth content must stay at its original column after height growth"
        );
        assert!(out.lines().any(|l| l.contains("NEW")));
    }
}

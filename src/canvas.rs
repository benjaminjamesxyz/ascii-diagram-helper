use crate::color::Color;
use crate::theme::Theme;
use unicode_width::UnicodeWidthChar;

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
    /// Combining marks (NFD) attached to `ch`: width-0, emitted directly
    /// after `ch` at render time. At most 2 per cell; a longer mark chain
    /// drops the excess (see `draw_text`).
    pub combining: [Option<char>; 2],
    pub is_continuation: bool,
    pub conn: LineConn,
    pub is_line: bool,
    /// Set on cells belonging to a thick (`==>`-style) edge run; picks heavy
    /// glyphs at render time.
    pub thick: bool,
    /// Set on cells of double-weight (`stroke-width:>=3px`) box borders; picks
    /// double glyphs (`╔═╗`) at render time. Takes precedence over `thick`.
    pub double: bool,
    pub custom_corner: Option<char>,
    pub role: CellRole,
    /// Emphasis color for border/line/arrow cells; `None` on text cells.
    pub color: Option<Color>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            combining: [None, None],
            is_continuation: false,
            conn: LineConn::default(),
            is_line: false,
            thick: false,
            double: false,
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
    /// Optional foreground color for TEXT cells. Unlike `pen`, this colors
    /// label text; used for Mermaid `fill:<color>` node labels. Scoped:
    /// set around label drawing, cleared right after.
    text_pen: Option<Color>,
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
            text_pen: None,
        }
    }

    /// Sets the pen color for subsequent border/line/arrow writes.
    pub fn set_pen(&mut self, color: Option<Color>) {
        self.pen = color;
    }

    /// Sets the foreground color for TEXT cells drawn while active (Mermaid
    /// `fill:<color>`). Scope tightly around label drawing.
    pub fn set_text_pen(&mut self, color: Option<Color>) {
        self.text_pen = color;
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
        let w = if ('\u{1F1E6}'..='\u{1F1FF}').contains(&ch) {
            2
        } else {
            ch.width().unwrap_or(1)
        };
        if w == 0 {
            return;
        }
        self.ensure_capacity(x + w - 1, y);

        let i = self.idx(x, y);
        let cell = &mut self.cells[i];
        cell.ch = ch;
        cell.combining = [None, None];
        cell.is_line = role == CellRole::Line;
        cell.is_continuation = false;
        cell.custom_corner = None;
        cell.role = role;
        // Text stays terminal-default unless a text pen is active
        // (Mermaid `fill:<color>` node labels)
        cell.color = if role == CellRole::Text {
            self.text_pen
        } else {
            self.pen
        };

        if w > 1 {
            for offset in 1..w {
                let i = self.idx(x + offset, y);
                let cont_cell = &mut self.cells[i];
                cont_cell.ch = ' ';
                cont_cell.combining = [None, None];
                cont_cell.is_continuation = true;
                cont_cell.is_line = false;
                cont_cell.role = role;
            }
        }
    }

    /// Tab expansion interval for label text (SEQ-W-03): a literal tab in a
    /// cell would be counted one column here but expands to the terminal's
    /// own tab stop, drifting every border right of it — so tabs never reach
    /// the canvas.
    const TAB_STOP: usize = 4;

    /// Draws label text character by character.
    ///
    /// Width and sanitation policy — keep rule-for-rule in sync with
    /// [`display_width`], which is the layout-math counterpart of this loop:
    /// - combining marks (width 0) attach to the base cell to their left and
    ///   render right after it, so NFD input keeps its diacritics while the
    ///   advance stays the base char's width (SEQ-W-02)
    /// - every other zero-width char (ZWSP U+200B, ZWNJ, LRM/RLM, bidi
    ///   controls U+202A-202E / U+2066-2069) is intentionally dropped and
    ///   never emitted — output carries no invisible formatting or bidi
    ///   override controls (SEQ-W-03, ERR-05)
    /// - tabs expand to spaces on `TAB_STOP`-column stops measured from
    ///   `start_x` (SEQ-W-03)
    /// - emoji presentation sequences collapse the way a modern terminal
    ///   renders them: skin-tone modifiers U+1F3FB-1F3FF and ZWJ U+200D plus
    ///   the component right after it contribute no columns, so `👍🏽` and
    ///   `👨‍👩‍👧` occupy 2 columns (SEQ-W-04); a regional-indicator flag
    ///   pair occupies the same 2 columns with both codepoints preserved
    pub fn draw_text(&mut self, start_x: usize, y: usize, text: &str) {
        let mut cur_x = start_x;
        let mut after_zwj = false;
        let mut prev_ri = false;
        for ch in text.chars() {
            if ch == '\t' {
                let adv = Self::TAB_STOP - ((cur_x - start_x) % Self::TAB_STOP);
                for _ in 0..adv {
                    self.put_char_with_role(cur_x, y, ' ', CellRole::Text);
                    cur_x += 1;
                }
                after_zwj = false;
                prev_ri = false;
                continue;
            }
            let is_ri = ('\u{1F1E6}'..='\u{1F1FF}').contains(&ch);
            let cw = if is_ri { 2 } else { ch.width().unwrap_or(1) };
            if cw == 0 {
                if is_combining_mark(ch) && cur_x > start_x {
                    self.attach_combining(cur_x, y, ch);
                }
                // Zero-width chars that are not combining marks are dropped.
                // A ZWJ additionally arms cluster collapsing below.
                after_zwj = ch == '\u{200D}';
                continue;
            }
            if after_zwj {
                // Component of a ZWJ-joined cluster: shares the base glyph's
                // 2 columns, never drawn separately.
                after_zwj = false;
                continue;
            }
            if ('\u{1F3FB}'..='\u{1F3FF}').contains(&ch) {
                continue; // skin-tone modifier: part of the previous glyph
            }

            if is_ri && prev_ri {
                // Second half of a flag pair: store it in the first RI's
                // continuation cell so both codepoints reach the output (the
                // terminal needs the pair to render the flag) while the pair
                // still occupies exactly the base's 2 columns
                prev_ri = false;
                if cur_x > 0
                    && let Some(cell) = self.cells.get_mut(y * self.width + cur_x - 1)
                    && cell.is_continuation
                {
                    cell.ch = ch;
                }
                continue;
            }
            prev_ri = is_ri;
            self.put_char_with_role(cur_x, y, ch, CellRole::Text);
            cur_x += cw;
        }
    }

    /// Attaches a width-0 combining mark to the base cell left of the pen
    /// position (stepping back over a wide-char continuation cell). At most
    /// `Cell::combining.len()` marks are kept per base; further marks, and
    /// marks with no cell to their left, are dropped.
    fn attach_combining(&mut self, cur_x: usize, y: usize, mark: char) {
        if cur_x == 0 || y >= self.height {
            return;
        }
        let mut mx = cur_x - 1;
        if self.get_cell(mx, y).is_some_and(|c| c.is_continuation) && mx > 0 {
            mx -= 1;
        }
        if let Some(cell) = self.cells.get_mut(y * self.width + mx)
            && let Some(slot) = cell.combining.iter_mut().find(|s| s.is_none())
        {
            *slot = Some(mark);
        }
    }

    /// Checks if a string of text can be safely placed starting at (`start_x`, y)
    /// without colliding with box borders, arrowheads, obstacle interiors, or existing text.
    #[must_use]
    pub fn can_place_text(&self, start_x: usize, y: usize, text: &str) -> bool {
        if y >= self.height {
            return true;
        }
        let text_w = display_width(text);
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
                    || matches!(cell.ch, '│' | '|' | '║' | '┆' | '┊' | '╎' | '╏' | ':')
                {
                    return false;
                }
                // Do not place labels inside horizontal line or dash runs — a label
                // drawn mid-run reads as merged with the crossing edge
                if cell.is_line || matches!(cell.ch, '─' | '-' | '╌' | '┄' | '━' | '═' | '.')
                {
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

    /// Finds the closest collision-free position for text within `bounds` around (`preferred_x`, `preferred_y`).
    /// Returns `None` if no safe position can be found within `bounds`.
    #[must_use]
    pub fn find_safe_text_pos_within(
        &self,
        bounds: Rect,
        preferred_x: usize,
        preferred_y: usize,
        text: &str,
    ) -> Option<(usize, usize)> {
        let text_w = display_width(text);
        let fits_bounds = |x: usize, y: usize| -> bool {
            let max_x = bounds.x.saturating_add(bounds.width);
            let max_y = bounds.y.saturating_add(bounds.height);
            y >= bounds.y && y < max_y && x >= bounds.x && x.saturating_add(text_w) <= max_x
        };

        if fits_bounds(preferred_x, preferred_y)
            && self.can_place_text(preferred_x, preferred_y, text)
        {
            return Some((preferred_x, preferred_y));
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
            if fits_bounds(test_x, test_y) && self.can_place_text(test_x, test_y, text) {
                return Some((test_x, test_y));
            }
        }

        None
    }

    /// Finds the closest collision-free position for text around (`preferred_x`, `preferred_y`).
    #[must_use]
    pub fn find_safe_text_pos(
        &self,
        preferred_x: usize,
        preferred_y: usize,
        text: &str,
    ) -> (usize, usize) {
        self.find_safe_text_pos_within(
            Rect::new(0, 0, usize::MAX, usize::MAX),
            preferred_x,
            preferred_y,
            text,
        )
        .unwrap_or((preferred_x, preferred_y))
    }

    /// Places text safely using collision detection, shifting if an obstacle or border is in the way.
    pub fn draw_text_safe(
        &mut self,
        preferred_x: usize,
        preferred_y: usize,
        text: &str,
    ) -> (usize, usize) {
        let (safe_x, safe_y) = self.find_safe_text_pos(preferred_x, preferred_y, text);
        if self.can_place_text(safe_x, safe_y, text) {
            self.draw_text(safe_x, safe_y, text);
        }
        (safe_x, safe_y)
    }

    pub fn draw_hline(&mut self, x1: usize, x2: usize, y: usize) {
        self.draw_hline_weight(x1, x2, y, 0);
    }

    /// Thick horizontal line (heavy glyphs at render time).
    pub fn draw_thick_hline(&mut self, x1: usize, x2: usize, y: usize) {
        self.draw_hline_weight(x1, x2, y, 1);
    }

    /// Double-weight horizontal line (`stroke-width:>=3px` box borders).
    pub fn draw_double_hline(&mut self, x1: usize, x2: usize, y: usize) {
        self.draw_hline_weight(x1, x2, y, 2);
    }

    fn draw_hline_weight(&mut self, x1: usize, x2: usize, y: usize, weight: u8) {
        if x1 > x2 {
            return self.draw_hline_weight(x2, x1, y, weight);
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
            // A stale dash glyph from an earlier dashed run would suppress
            // junction resolution (render only resolves ' ' line cells)
            if cell.ch == '\u{252e}' || cell.ch == '-' || cell.ch == '.' {
                cell.ch = ' ';
                cell.combining = [None, None];
            }
            cell.is_line = true;
            cell.thick = weight == 1;
            cell.double = weight == 2;
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

        // 7-bit dashed strokes use '.' runs so they stay distinguishable
        // from a solid ascii '-' run (SEQ-02 async arrows, FC-SUB-05 dashed
        // borders). Dots are the ascii dashed family; ':' is its vertical
        // counterpart — both pure 7-bit, neither collides with the solid
        // `+ - |` ascii box set.
        let dash_char = if theme.box_style == crate::theme::BoxStyle::Ascii {
            '.'
        } else {
            '╌'
        };
        // Perpendicular dashed-run glyph: dash × dash crossing resolves to a
        // solid cross cell — one cell loses its dash pattern, both strokes
        // stay continuous
        let vdash_char = if theme.box_style == crate::theme::BoxStyle::Ascii {
            ':'
        } else {
            '┆'
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
            // Crossing: a solid vertical line passes through this dash run —
            // merge into a junction instead of overwriting the stroke; render
            // resolves ┼ from the conn flags
            if cell.is_line && cell.ch == ' ' && (cell.conn.north || cell.conn.south) {
                cell.conn.east = true;
                cell.conn.west = true;
                if cell.role != CellRole::Border {
                    cell.role = CellRole::Line;
                    cell.color = self.pen;
                }
                continue;
            }
            // Crossing: a dashed vertical run passes here — convert to a
            // solid cross cell so neither stroke loses continuity
            if cell.ch == vdash_char {
                cell.ch = ' ';
                cell.is_line = true;
                cell.conn.north = true;
                cell.conn.south = true;
                cell.conn.east = true;
                cell.conn.west = true;
                if cell.role != CellRole::Border {
                    cell.role = CellRole::Line;
                    cell.color = self.pen;
                }
                continue;
            }
            cell.ch = dash_char;
            cell.combining = [None, None];
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
            ':'
        } else {
            '┆'
        };
        // Perpendicular dashed-run glyph: dash × dash crossing resolves to a
        // solid cross cell (see draw_dashed_hline)
        let hdash_char = if theme.box_style == crate::theme::BoxStyle::Ascii {
            '.'
        } else {
            '╌'
        };

        for y in y1..=y2 {
            let cell = &mut self.cells[y * self.width + x];
            if cell.role == CellRole::Text && cell.ch != ' ' {
                continue;
            }
            // Crossing: a solid horizontal line passes through this dash run
            // — merge into a junction instead of overwriting the stroke
            if cell.is_line && cell.ch == ' ' && (cell.conn.east || cell.conn.west) {
                cell.conn.north = true;
                cell.conn.south = true;
                if cell.role != CellRole::Border {
                    cell.role = CellRole::Line;
                    cell.color = self.pen;
                }
                continue;
            }
            // Crossing: a dashed horizontal run passes here — convert to a
            // solid cross cell so neither stroke loses continuity
            if cell.ch == hdash_char {
                cell.ch = ' ';
                cell.is_line = true;
                cell.conn.north = true;
                cell.conn.south = true;
                cell.conn.east = true;
                cell.conn.west = true;
                if cell.role != CellRole::Border {
                    cell.role = CellRole::Line;
                    cell.color = self.pen;
                }
                continue;
            }
            cell.ch = dash_char;
            cell.combining = [None, None];
            cell.is_line = false;
            if cell.role != CellRole::Border {
                cell.color = self.pen;
            }
        }
    }

    pub fn draw_vline(&mut self, x: usize, y1: usize, y2: usize) {
        self.draw_vline_weight(x, y1, y2, 0);
    }

    /// Thick vertical line (heavy glyphs at render time).
    pub fn draw_thick_vline(&mut self, x: usize, y1: usize, y2: usize) {
        self.draw_vline_weight(x, y1, y2, 1);
    }

    /// Double-weight vertical line (`stroke-width:>=3px` box borders).
    pub fn draw_double_vline(&mut self, x: usize, y1: usize, y2: usize) {
        self.draw_vline_weight(x, y1, y2, 2);
    }

    fn draw_vline_weight(&mut self, x: usize, y1: usize, y2: usize, weight: u8) {
        if y1 > y2 {
            return self.draw_vline_weight(x, y2, y1, weight);
        }
        self.ensure_capacity(x, y2);

        for y in y1..=y2 {
            let cell = &mut self.cells[y * self.width + x];
            // Collision protection: don't overwrite text with a line
            if cell.role == CellRole::Text && cell.ch != ' ' {
                continue;
            }
            // Clear stale dash glyphs so junction resolution applies
            if cell.ch == '\u{2546}' || cell.ch == '|' || cell.ch == ':' {
                cell.ch = ' ';
                cell.combining = [None, None];
            }
            cell.is_line = true;
            cell.thick = weight == 1;
            cell.double = weight == 2;
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
        self.draw_box_inner(Rect::new(x, y, width, height), theme, title, false, 0);
    }

    /// Weighted box border (Mermaid `class`/`style` `stroke-width`): `1` =
    /// heavy `┏━┓` (>=2px), `2` = double `╔═╗` (>=3px). Glyphs resolved at
    /// render time like `==>` edge runs.
    #[allow(
        clippy::too_many_arguments,
        reason = "public API keeps explicit x/y/width/height for call-site readability"
    )]
    pub fn draw_weighted_box(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        theme: &Theme,
        title: Option<&str>,
        weight: u8,
    ) {
        self.draw_box_inner(Rect::new(x, y, width, height), theme, title, false, weight);
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
        self.draw_box_inner(Rect::new(x, y, width, height), theme, title, true, 0);
    }

    /// Draws a styled rectangle box with optional title and content
    ///
    /// # Panics
    ///
    /// Panics if the box coordinates overflow `usize`, or on allocation failure
    /// when the canvas must grow to fit the box.
    fn draw_box_inner(
        &mut self,
        rect: Rect,
        theme: &Theme,
        title: Option<&str>,
        dashed: bool,
        weight: u8,
    ) {
        let (x, y) = (rect.x, rect.y);
        let width = rect.width;
        let height = rect.height;
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
        } else if weight == 1 {
            self.draw_thick_hline(x, right, y);
            self.draw_thick_hline(x, right, bottom);
        } else if weight == 2 {
            self.draw_double_hline(x, right, y);
            self.draw_double_hline(x, right, bottom);
        } else {
            self.draw_hline(x, right, y);
            self.draw_hline(x, right, bottom);
        }

        // Draw vertical edges
        if dashed {
            self.draw_dashed_vline(x, y, bottom, theme);
            self.draw_dashed_vline(right, y, bottom, theme);
        } else if weight == 1 {
            self.draw_thick_vline(x, y, bottom);
            self.draw_thick_vline(right, y, bottom);
        } else if weight == 2 {
            self.draw_double_vline(x, y, bottom);
            self.draw_double_vline(right, y, bottom);
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
            let title_cols = 2 + display_width(t);
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
        // Solid borders resolve from the active theme (sharp stays `┌─┐`,
        // double keeps `╔═╗`); dashed borders and 7-bit ascii keep their own
        // glyph families. Ascii dashed uses the dotted family ('.', ':') so a
        // `stroke-dasharray` decision node stays distinct from the solid
        // ascii box (`-`, `|`) and the solid decision box (`=`, `#`).
        let (tl, tr, bl, br, h_char, v_char) = match (is_ascii, dashed) {
            (true, true) => ('+', '+', '+', '+', '.', ':'),
            (true, false) => ('+', '+', '+', '+', '=', '#'),
            (false, true) => ('╌', '╌', '╌', '╌', '╌', '┆'),
            (false, false) => (
                theme.top_left_corner(),
                theme.top_right_corner(),
                theme.bottom_left_corner(),
                theme.bottom_right_corner(),
                theme.horizontal_line(),
                theme.vertical_line(),
            ),
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
                // (theme-resolved: `┬` sharp/rounded, `╦` double, `┻` heavy)
                self.put_char_with_role(mid_x, bottom, theme.tee_down(), CellRole::Border);
            }
            // Left connector if horizontal line touches
            if x > 0
                && self
                    .get_cell(x - 1, mid_y)
                    .is_some_and(|c| c.is_line || c.ch == '─' || c.ch == '►')
            {
                self.put_char_with_role(x, mid_y, theme.tee_right(), CellRole::Border);
            }
            // Right connector if horizontal line touches
            if self
                .get_cell(right + 1, mid_y)
                .is_some_and(|c| c.is_line || c.ch == '─')
            {
                self.put_char_with_role(right, mid_y, theme.tee_left(), CellRole::Border);
            }
        }

        // Embed badge on top border if provided
        if let Some(b) = badge {
            let badge_text = if is_ascii {
                String::from(" <?> ")
            } else {
                [" ", b, " "].concat()
            };
            let badge_w = display_width(&badge_text);
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
        let double_theme = Theme::new(crate::theme::BoxStyle::Double);
        let mut current: Option<Color> = None;
        for (y, &rx) in row_max_x.iter().enumerate().take(max_y + 1) {
            let limit = rx.min(max_x);
            let base = y * self.width;
            for x in 0..=limit {
                let cell = &self.cells[base + x];
                if cell.is_continuation {
                    // Continuation cells are pure column spacers — except when
                    // they carry a sequence tail (second half of a
                    // regional-indicator flag pair): it belongs to the same 2
                    // columns and must reach the output or the terminal gets a
                    // lone half-flag
                    if cell.ch != ' ' {
                        out.push(cell.ch);
                    }
                    continue;
                }
                if colored && cell.color != current {
                    // Close with default-fg only (39m), not full reset (0m):
                    // Pi paints tool output with a theme background, and a
                    // full reset would strip it mid-line, leaving patchy
                    // background bands around colored runs
                    if current.is_some() {
                        out.push_str("\u{1b}[39m");
                    }
                    if let Some(c) = cell.color {
                        out.push_str(&format!("\u{1b}[{}m", c.sgr()));
                    }
                    current = cell.color;
                }
                if cell.is_line && cell.ch == ' ' {
                    let (t, thick) = if cell.double {
                        (&double_theme, false)
                    } else {
                        (theme, cell.thick)
                    };
                    out.push(resolve_line_glyph(cell.conn, t, thick));
                } else {
                    out.push(cell.ch);
                    // NFD combining marks ride on their base cell (SEQ-W-02)
                    for mark in cell.combining.iter().flatten() {
                        out.push(*mark);
                    }
                }
            }
            // Close any open color run before trimming/pushing the newline so
            // SGR state never leaks across rows (default-fg only, see above)
            if colored && current.is_some() {
                out.push_str("\u{1b}[39m");
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

/// Terminal display width of `text` — the layout-math counterpart of
/// [`Canvas::draw_text`], which places glyphs under exactly these rules:
/// - combining marks attach to the preceding base char and add 0 columns
///   (the base keeps its own width), so NFD `café` is 4 columns
/// - every other zero-width char (ZWSP U+200B, ZWNJ, LRM/RLM, bidi controls)
///   is dropped at draw time and counts 0 here
/// - tabs advance to the next 4-column stop measured from the start of
///   `text` (draw_text expands them to spaces on the same stops)
/// - emoji sequences collapse to what a modern terminal renders:
///   U+FE0F/U+FE0E, skin-tone modifiers U+1F3FB-1F3FF, ZWJ U+200D and the
///   component right after a ZWJ all count 0, so `👍🏽` and `👨‍👩‍👧`
///   occupy 2 columns; a regional-indicator flag pair counts 2 total
///
/// Residual limits (see also README Known Limitations): FE0F emoji
/// presentation of a text-default symbol (e.g. `❤️`) still counts 1 while
/// emoji-presentation terminals render 2; terminals that expand ZWJ/skin-tone
/// clusters to multiple cells disagree with the 2-column assumption.
#[must_use]
pub fn display_width(text: &str) -> usize {
    let mut width = 0usize;
    let mut col = 0usize;
    let mut after_zwj = false;
    let mut prev_ri = false;
    for ch in text.chars() {
        if ch == '\t' {
            let adv = Canvas::TAB_STOP - (col % Canvas::TAB_STOP);
            width += adv;
            col += adv;
            after_zwj = false;
            prev_ri = false;
            continue;
        }
        let is_ri = ('\u{1F1E6}'..='\u{1F1FF}').contains(&ch);
        let cw = if is_ri { 2 } else { ch.width().unwrap_or(1) };
        if cw == 0 {
            // Combining marks and other zero-width chars ride for free; a ZWJ
            // arms cluster collapsing for the next visible char.
            after_zwj = ch == '\u{200D}';
            continue;
        }
        if after_zwj {
            after_zwj = false;
            continue; // ZWJ cluster component shares the base glyph's cells
        }
        if ('\u{1F3FB}'..='\u{1F3FF}').contains(&ch) {
            continue; // skin-tone modifier: part of the previous glyph
        }

        if is_ri && prev_ri {
            prev_ri = false;
            continue; // second half of a regional-indicator flag pair
        }
        prev_ri = is_ri;
        width += cw;
        col += cw;
    }
    width
}

/// Combining marks (Unicode Mn/Mc/Me) that [`Canvas::draw_text`] preserves by
/// attaching them to the base cell on their left. The table covers the blocks
/// that occur in real-world diagram labels (Latin/Greek/Cyrillic diacritics,
/// Hebrew, Arabic, Thai/Lao, Indic, Tibetan, Ethiopic, symbol marks and the
/// combining-half forms); a mark outside the table falls back to the
/// zero-width drop policy. Variation selectors are deliberately excluded —
/// they are presentation selectors, not visible marks.
fn is_combining_mark(ch: char) -> bool {
    matches!(ch,
        '\u{0300}'..='\u{036F}' // combining diacritical marks (Latin/Greek/Cyrillic)
        | '\u{0483}'..='\u{0489}' // cyrillic (titlo, palatalization…)
        | '\u{0591}'..='\u{05BD}' | '\u{05BF}' | '\u{05C1}'..='\u{05C2}'
        | '\u{05C4}'..='\u{05C5}' | '\u{05C7}' // hebrew points
        | '\u{0610}'..='\u{061A}' | '\u{064B}'..='\u{065F}' | '\u{0670}'
        | '\u{06D6}'..='\u{06DC}' | '\u{06DF}'..='\u{06E4}' | '\u{06E7}'..='\u{06E8}'
        | '\u{06EA}'..='\u{06ED}' // arabic
        | '\u{0711}' | '\u{0730}'..='\u{074A}' // syriac
        | '\u{07A6}'..='\u{07B0}' // thaana
        | '\u{07EB}'..='\u{07F3}' // nko
        | '\u{0816}'..='\u{0819}' | '\u{081B}'..='\u{0823}' | '\u{0825}'..='\u{0827}'
        | '\u{0829}'..='\u{082D}' | '\u{0859}'..='\u{085B}' | '\u{08D3}'..='\u{08E1}'
        | '\u{093C}' | '\u{0951}'..='\u{0957}' // devanagari nukta/vedic
        | '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}' // thai
        | '\u{0EB1}' | '\u{0EB4}'..='\u{0EBC}' | '\u{0EC8}'..='\u{0ECD}' // lao
        | '\u{0F71}'..='\u{0F84}' | '\u{0F86}'..='\u{0F87}' // tibetan
        | '\u{135D}'..='\u{135F}' // ethiopic
        | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' // diacritics extended/supplement
        | '\u{20D0}'..='\u{20F0}' // combining marks for symbols
        | '\u{2CEF}'..='\u{2CF1}' // coptic
        | '\u{2DE0}'..='\u{2DFF}' // cyrillic extended-A
        | '\u{A66F}'..='\u{A672}' | '\u{A674}'..='\u{A67D}' | '\u{A69E}'..='\u{A69F}'
        | '\u{A6F0}'..='\u{A6F1}' // cyrillic extended-B
        | '\u{A802}' | '\u{A806}' | '\u{A80B}' | '\u{A825}'..='\u{A826}' // phags-pa
        | '\u{A8C4}'..='\u{A8C5}' | '\u{A8E0}'..='\u{A8F1}' // devanagari extended
        | '\u{A926}'..='\u{A92D}' | '\u{A947}'..='\u{A951}' // javanese/rejang
        | '\u{A980}'..='\u{A982}' | '\u{A9B3}' | '\u{A9B6}'..='\u{A9B9}'
        | '\u{A9BC}'..='\u{A9BD}' // javanese
        | '\u{AAB0}' | '\u{AAB2}'..='\u{AAB4}' | '\u{AAB7}'..='\u{AAB8}'
        | '\u{AABE}'..='\u{AABF}' | '\u{AAC1}' | '\u{AAEC}'..='\u{AAEF}' | '\u{AAF6}' // tai viet
        | '\u{ABE5}' | '\u{ABE8}' | '\u{ABED}' // mei (manipuri)
        | '\u{FB1E}' // hebrew point judeo-spanish
        | '\u{FE20}'..='\u{FE2F}' // combining half marks
    )
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

    #[test]
    fn test_find_safe_text_pos_within_respects_bounds() {
        let mut canvas = Canvas::new(30, 20);
        let theme = Theme::new(BoxStyle::Rounded);
        // Box at (5, 5, 10, 5) -> top border at y=5, bottom at y=9, left at x=5, right at x=14
        canvas.draw_box(5, 5, 10, 5, &theme, None);

        // Outside the box, open area
        let bounds = Rect::new(0, 0, 30, 5);
        let pos = canvas.find_safe_text_pos_within(bounds, 2, 2, "OK");
        assert_eq!(pos, Some((2, 2)));

        // Preferred on top border of box at (7, 5): ladder shifts up into bounds (row 4)
        let shifted = canvas.find_safe_text_pos_within(bounds, 7, 5, "HI");
        assert!(shifted.is_some());
        let (sx, sy) = shifted.unwrap();
        assert!(sx >= bounds.x && sx + 2 <= bounds.x + bounds.width);
        assert!(sy >= bounds.y && sy < bounds.y + bounds.height);
        assert_eq!(sy, 4);

        // Strict bounds covering only the box obstacle: no safe cell
        let box_bounds = Rect::new(5, 5, 10, 5);
        let none_pos = canvas.find_safe_text_pos_within(box_bounds, 7, 7, "NOFIT");
        assert_eq!(none_pos, None);
    }

    #[test]
    fn test_find_safe_text_pos_unbounded_unchanged() {
        let mut canvas = Canvas::new(20, 10);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_box(2, 2, 8, 4, &theme, None);
        // Unbounded search shifts away from border
        let (safe_x, safe_y) = canvas.find_safe_text_pos(2, 2, "TEST");
        assert!(safe_y != 2 || safe_x != 2);
        assert!(canvas.can_place_text(safe_x, safe_y, "TEST"));
    }

    #[test]
    fn test_draw_text_safe_never_overwrites_border() {
        let mut canvas = Canvas::new(30, 30);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_box(5, 5, 10, 5, &theme, None);
        // Fill entire surrounding area with obstacle so ladder cannot escape
        canvas.add_obstacle(Rect::new(0, 0, 30, 30));

        let before = canvas.render(&theme);
        // Attempt to draw label pinned in an impossible spot (on border)
        let (ret_x, ret_y) = canvas.draw_text_safe(5, 5, "NOFIT");
        assert_eq!((ret_x, ret_y), (5, 5));
        let after = canvas.render(&theme);
        // Border must remain intact and label dropped
        assert_eq!(before, after);
        assert!(!after.contains("NOFIT"));
    }

    #[test]
    fn test_draw_text_safe_resolved_unchanged() {
        let mut canvas = Canvas::new(20, 10);
        let theme = Theme::new(BoxStyle::Rounded);
        canvas.draw_box(2, 2, 8, 4, &theme, None);
        // Preferred is border at (2, 2), safe ladder finds (2, 1) or another safe cell
        let (sx, sy) = canvas.draw_text_safe(2, 2, "SAFE");
        assert_ne!((sx, sy), (2, 2));
        let out = canvas.render(&theme);
        assert!(out.contains("SAFE"));
    }

    #[test]
    fn test_draw_text_safe_fallback_not_painted() {
        let mut canvas = Canvas::new(30, 30);
        let theme = Theme::new(BoxStyle::Rounded);
        // Block whole canvas with obstacle, preferred at center so all offsets stay < 30
        canvas.add_obstacle(Rect::new(0, 0, 30, 30));
        let (sx, sy) = canvas.draw_text_safe(15, 15, "BLOCKED");
        assert_eq!((sx, sy), (15, 15));
        let out = canvas.render(&theme);
        assert!(!out.contains("BLOCKED"));
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

#[cfg(test)]
mod display_width_tests {
    use super::*;

    #[test]
    fn nfd_combining_mark_counts_on_base() {
        // é = U+0065 + U+0301 (NFD): 1 column, same as NFC é
        assert_eq!(display_width("e\u{0301}"), 1);
        assert_eq!(display_width("caf\u{0065}\u{0301}"), 4);
        // NFC 'é' — same 16 columns as the NFD spelling of the same text
        assert_eq!(display_width("Caf\u{00e9} (decomposed)"), 17);
    }

    #[test]
    fn zwsp_and_bidi_controls_count_zero() {
        assert_eq!(display_width("zero\u{200b}width"), 9);
        // U+202E RTL override + U+202D LRO + U+2066-2069 isolates + LRM/RLM
        assert_eq!(display_width("\u{202e}ab"), 2);
        assert_eq!(
            display_width("a\u{202d}b\u{2066}c\u{2067}d\u{2068}e\u{2069}f\u{200e}g\u{200f}h"),
            8
        );
    }

    #[test]
    fn tabs_advance_on_four_col_stops() {
        assert_eq!(display_width("\t"), 4);
        assert_eq!(display_width("a\tb"), 1 + 3 + 1);
        assert_eq!(display_width("ab\t"), 4);
        assert_eq!(display_width("abc\td"), 5);
    }

    #[test]
    fn emoji_presentation_sequences_collapse() {
        // SEQ-W-04: skin-tone modifier and ZWJ-joined family are 2 columns
        assert_eq!(display_width("\u{1F44D}\u{1F3FD}"), 2); // thumbs up + medium skin tone
        assert_eq!(
            display_width("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
            2
        ); // family
        assert_eq!(display_width("\u{1F44D}\u{1F3FD} ok"), 5);
        assert_eq!(display_width("\u{1F1FA}\u{1F1F8}"), 2); // regional-indicator flag pair
        assert_eq!(display_width("\u{1F1FA}\u{1F1F8}\u{1F1EF}\u{1F1F5}"), 4); // two flags
    }

    #[test]
    fn combining_mark_after_wide_char_attaches_without_width() {
        // Devanagari-style stack on a wide base stays at the base width
        assert_eq!(display_width("\u{0915}\u{093F}"), 2);
    }

    #[test]
    fn draw_text_preserves_combining_mark_in_output() {
        let mut canvas = Canvas::new(12, 3);
        canvas.draw_text(0, 1, "cafe\u{0301}"); // NFD: c a f e + combining acute
        let theme = Theme::new(crate::theme::BoxStyle::Sharp);
        let out = canvas.render(&theme);
        assert!(
            out.contains("e\u{0301}"),
            "NFD accent must survive draw_text: {out:?}"
        );
        // The marked cell ends the text: column 4 is untouched
        assert_eq!(canvas.get_cell(4, 1).map(|c| c.ch), Some(' '));
    }

    #[test]
    fn draw_text_strips_bidi_and_zwsp() {
        let mut canvas = Canvas::new(20, 3);
        canvas.draw_text(0, 1, "a\u{202e}b\u{200b}c");
        let theme = Theme::new(crate::theme::BoxStyle::Sharp);
        let out = canvas.render(&theme);
        assert!(!out.contains('\u{202e}'), "bidi override must not render");
        assert!(!out.contains('\u{200b}'), "ZWSP must not render");
        assert!(out.contains("abc"), "visible text kept: {out:?}");
    }

    #[test]
    fn draw_text_expands_tabs_to_spaces() {
        let mut canvas = Canvas::new(20, 3);
        canvas.draw_text(0, 1, "a\tb");
        let theme = Theme::new(crate::theme::BoxStyle::Sharp);
        let out = canvas.render(&theme);
        assert!(!out.contains('\t'), "no literal tab may reach the canvas");
        assert!(out.contains("a   b"), "tab expands to col-4 stop: {out:?}");
        // 'b' sits exactly at column 4
        assert_eq!(canvas.get_cell(4, 1).map(|c| c.ch), Some('b'));
    }

    #[test]
    fn draw_text_emoji_cluster_occupies_two_cells() {
        let mut canvas = Canvas::new(20, 3);
        canvas.draw_text(0, 1, "\u{1F44D}\u{1F3FD}!"); // 👍🏽!
        // Cluster = cols 0-1 (col 1 is the wide continuation), '!' at col 2
        assert_eq!(canvas.get_cell(0, 1).map(|c| c.ch), Some('\u{1F44D}'));
        assert!(canvas.get_cell(1, 1).is_some_and(|c| c.is_continuation));
        assert_eq!(canvas.get_cell(2, 1).map(|c| c.ch), Some('!'));
        assert_eq!(canvas.get_cell(3, 1).map(|c| c.ch), Some(' '));
    }

    #[test]
    fn draw_text_flag_pair_keeps_both_codepoints_and_width() {
        let mut canvas = Canvas::new(20, 3);
        canvas.draw_text(0, 1, "\u{1F1FA}\u{1F1F8}x"); // 🇺🇸x
        let theme = Theme::new(crate::theme::BoxStyle::Sharp);
        let out = canvas.render(&theme);
        // Both regional indicators reach the output — a lone RI renders as a
        // broken half-flag in terminals
        assert!(out.contains('\u{1F1FA}'), "first RI: {out:?}");
        assert!(out.contains('\u{1F1F8}'), "second RI: {out:?}");
        // …but the pair occupies exactly 2 columns: 'x' at column 2
        assert_eq!(canvas.get_cell(2, 1).map(|c| c.ch), Some('x'));
    }

    #[test]
    fn draw_text_matches_display_width_advance() {
        for text in [
            "café",
            "a\tb",
            "\u{1F44D}\u{1F3FD} ok",
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} x",
            "zero\u{200b}width",
            "\u{202e}reversed",
        ] {
            let mut canvas = Canvas::new(64, 3);
            canvas.draw_text(1, 1, text);
            let expected_end = 1 + display_width(text);
            assert_eq!(
                canvas.get_cell(expected_end, 1).map(|c| c.ch),
                Some(' '),
                "text {text:?} must advance exactly display_width cols"
            );
        }
    }
}

#[cfg(test)]
mod ascii_dashed_tests {
    use super::*;

    #[test]
    fn ascii_dashed_hline_distinct_from_solid() {
        let mut dashed = Canvas::new(12, 3);
        dashed.draw_dashed_hline(1, 10, 1, &Theme::ascii());
        let mut solid = Canvas::new(12, 3);
        solid.draw_hline(1, 10, 1);
        let dashed_row = dashed.render(&Theme::ascii());
        let solid_row = solid.render(&Theme::ascii());
        assert!(
            dashed_row.contains('.'),
            "ascii dashed run must be dotted: {dashed_row:?}"
        );
        assert!(
            !dashed_row.contains('-'),
            "ascii dashed must not reuse the solid glyph: {dashed_row:?}"
        );
        assert!(
            solid_row.contains('-'),
            "solid run stays '-': {solid_row:?}"
        );
        assert_ne!(dashed_row, solid_row);
    }

    #[test]
    fn ascii_dashed_vline_distinct_from_solid() {
        let mut dashed = Canvas::new(6, 8);
        dashed.draw_dashed_vline(2, 1, 6, &Theme::ascii());
        let out = dashed.render(&Theme::ascii());
        assert!(out.contains(':'), "ascii vertical dashed run: {out:?}");
        for line in out.lines() {
            assert!(
                !line.contains('|'),
                "ascii dashed must not reuse solid '|': {out:?}"
            );
        }
    }

    #[test]
    fn ascii_dashed_crossing_resolves_to_junction() {
        let mut canvas = Canvas::new(12, 12);
        let theme = Theme::ascii();
        canvas.draw_dashed_hline(1, 10, 5, &theme);
        canvas.draw_dashed_vline(5, 1, 10, &theme);
        let out = canvas.render(&theme);
        // The crossing cell becomes a solid cross so both strokes stay readable
        let row = out.lines().nth(5).expect("crossing row");
        assert!(
            row.contains('+'),
            "dashed × dashed crossing resolves to '+': {row:?}"
        );
    }

    #[test]
    fn ascii_dashed_box_differs_from_solid_box() {
        let mut dashed = Canvas::new(12, 5);
        dashed.draw_dashed_box(1, 1, 9, 3, &Theme::ascii(), None);
        let mut solid = Canvas::new(12, 5);
        solid.draw_box(1, 1, 9, 3, &Theme::ascii(), None);
        let dashed_out = dashed.render(&Theme::ascii());
        let solid_out = solid.render(&Theme::ascii());
        assert!(
            dashed_out.contains('.'),
            "FC-SUB-05: ascii dashed border must be dotted: {dashed_out:?}"
        );
        assert_ne!(
            dashed_out, solid_out,
            "dashed border must differ from solid"
        );
    }
}

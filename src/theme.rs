use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BoxStyle {
    #[default]
    Rounded, // ╭ ─ ╮ │ │ ╰ ─ ╯
    Sharp,  // ┌ ─ ┐ │ │ └ ─ ┘
    Double, // ╔ ═ ╗ ║ ║ ╚ ═ ╝
    Heavy,  // ┏ ━ ┓ ┃ ┃ ┗ ━ ┛
    Ascii,  // + - + | | + - +
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ArrowStyle {
    #[default]
    Filled, // ► ◄ ▲ ▼
    Open,  // > < ^ v
    Ascii, // > < ^ v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    Double,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub box_style: BoxStyle,
    pub arrow_style: ArrowStyle,
    pub line_style: LineStyle,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            box_style: BoxStyle::Rounded,
            arrow_style: ArrowStyle::Filled,
            line_style: LineStyle::Solid,
        }
    }
}

impl Theme {
    #[must_use]
    pub fn new(box_style: BoxStyle) -> Self {
        let arrow_style = match box_style {
            BoxStyle::Ascii => ArrowStyle::Ascii,
            _ => ArrowStyle::Filled,
        };
        Self {
            box_style,
            arrow_style,
            line_style: LineStyle::Solid,
        }
    }

    #[must_use]
    pub fn ascii() -> Self {
        Self {
            box_style: BoxStyle::Ascii,
            arrow_style: ArrowStyle::Ascii,
            line_style: LineStyle::Solid,
        }
    }

    #[must_use]
    pub fn top_left_corner(&self) -> char {
        match self.box_style {
            BoxStyle::Rounded => '╭',
            BoxStyle::Sharp => '┌',
            BoxStyle::Double => '╔',
            BoxStyle::Heavy => '┏',
            BoxStyle::Ascii => '+',
        }
    }

    #[must_use]
    pub fn top_right_corner(&self) -> char {
        match self.box_style {
            BoxStyle::Rounded => '╮',
            BoxStyle::Sharp => '┐',
            BoxStyle::Double => '╗',
            BoxStyle::Heavy => '┓',
            BoxStyle::Ascii => '+',
        }
    }

    #[must_use]
    pub fn bottom_left_corner(&self) -> char {
        match self.box_style {
            BoxStyle::Rounded => '╰',
            BoxStyle::Sharp => '└',
            BoxStyle::Double => '╚',
            BoxStyle::Heavy => '┗',
            BoxStyle::Ascii => '+',
        }
    }

    #[must_use]
    pub fn bottom_right_corner(&self) -> char {
        match self.box_style {
            BoxStyle::Rounded => '╯',
            BoxStyle::Sharp => '┘',
            BoxStyle::Double => '╝',
            BoxStyle::Heavy => '┛',
            BoxStyle::Ascii => '+',
        }
    }

    #[must_use]
    pub fn horizontal_line(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '═',
            BoxStyle::Heavy => '━',
            BoxStyle::Ascii => '-',
            _ => '─',
        }
    }

    #[must_use]
    pub fn vertical_line(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '║',
            BoxStyle::Heavy => '┃',
            BoxStyle::Ascii => '|',
            _ => '│',
        }
    }

    #[must_use]
    pub fn tee_down(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '╦',
            BoxStyle::Heavy => '┳',
            BoxStyle::Ascii => '+',
            _ => '┬',
        }
    }

    #[must_use]
    pub fn tee_up(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '╩',
            BoxStyle::Heavy => '┻',
            BoxStyle::Ascii => '+',
            _ => '┴',
        }
    }

    #[must_use]
    pub fn tee_right(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '╠',
            BoxStyle::Heavy => '┣',
            BoxStyle::Ascii => '+',
            _ => '├',
        }
    }

    #[must_use]
    pub fn tee_left(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '╣',
            BoxStyle::Heavy => '┫',
            BoxStyle::Ascii => '+',
            _ => '┤',
        }
    }

    #[must_use]
    pub fn cross(&self) -> char {
        match self.box_style {
            BoxStyle::Double => '╬',
            BoxStyle::Heavy => '╋',
            BoxStyle::Ascii => '+',
            _ => '┼',
        }
    }

    #[must_use]
    pub fn arrow_right(&self) -> char {
        match self.arrow_style {
            ArrowStyle::Filled => '►',
            ArrowStyle::Open | ArrowStyle::Ascii => '>',
        }
    }

    #[must_use]
    pub fn arrow_left(&self) -> char {
        match self.arrow_style {
            ArrowStyle::Filled => '◄',
            ArrowStyle::Open | ArrowStyle::Ascii => '<',
        }
    }

    #[must_use]
    pub fn arrow_down(&self) -> char {
        match self.arrow_style {
            ArrowStyle::Filled => '▼',
            ArrowStyle::Open | ArrowStyle::Ascii => 'v',
        }
    }

    #[must_use]
    pub fn arrow_up(&self) -> char {
        match self.arrow_style {
            ArrowStyle::Filled => '▲',
            ArrowStyle::Open | ArrowStyle::Ascii => '^',
        }
    }

    // ---- Thick-edge glyphs (`==>` / `<==>` flowchart edges) ----

    #[must_use]
    pub fn thick_horizontal_line(&self) -> char {
        match self.box_style {
            BoxStyle::Ascii => '=',
            BoxStyle::Double => '═',
            _ => '━',
        }
    }

    #[must_use]
    pub fn thick_vertical_line(&self) -> char {
        match self.box_style {
            BoxStyle::Ascii => '|',
            BoxStyle::Double => '║',
            _ => '┃',
        }
    }

    #[must_use]
    pub fn thick_top_left_corner(&self) -> char {
        self.thick_corner('╔', '┏')
    }

    #[must_use]
    pub fn thick_top_right_corner(&self) -> char {
        self.thick_corner('╗', '┓')
    }

    #[must_use]
    pub fn thick_bottom_left_corner(&self) -> char {
        self.thick_corner('╚', '┗')
    }

    #[must_use]
    pub fn thick_bottom_right_corner(&self) -> char {
        self.thick_corner('╝', '┛')
    }

    #[must_use]
    pub fn thick_tee_down(&self) -> char {
        self.thick_junction('╦', '┳')
    }

    #[must_use]
    pub fn thick_tee_up(&self) -> char {
        self.thick_junction('╩', '┻')
    }

    #[must_use]
    pub fn thick_tee_right(&self) -> char {
        self.thick_junction('╠', '┣')
    }

    #[must_use]
    pub fn thick_tee_left(&self) -> char {
        self.thick_junction('╣', '┫')
    }

    #[must_use]
    pub fn thick_cross(&self) -> char {
        self.thick_junction('╬', '╋')
    }

    /// Double style is already the heaviest weight — keep its own glyphs there.
    /// Ascii stays 7-bit. Everything else goes heavy.
    fn thick_corner(&self, double: char, heavy: char) -> char {
        match self.box_style {
            BoxStyle::Ascii => '+',
            BoxStyle::Double => double,
            _ => heavy,
        }
    }

    fn thick_junction(&self, double: char, heavy: char) -> char {
        match self.box_style {
            BoxStyle::Ascii => '+',
            BoxStyle::Double => double,
            _ => heavy,
        }
    }
}

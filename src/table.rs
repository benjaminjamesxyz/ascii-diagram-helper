use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

use crate::color::Color;
use crate::schema::{TableSpec, TextAlign};
use crate::theme::Theme;

/// Sanitizes a table cell by stripping ANSI escape sequences, expanding tabs to
/// 4 spaces, and removing non-printable control characters.
///
/// Clean strings incur zero allocations.
#[must_use]
pub fn sanitize_cell(s: &str) -> Cow<'_, str> {
    if !s.chars().any(char::is_control) {
        return Cow::Borrowed(s);
    }

    let stripped = crate::color::Color::strip_ansi(s);
    let mut out = String::with_capacity(stripped.len());
    for ch in stripped.chars() {
        if ch == '\t' {
            out.push_str("    ");
        } else if !ch.is_control() {
            out.push(ch);
        }
    }
    Cow::Owned(out)
}

/// Splits a raw table row line into trimmed cell strings (ST-01, ST-03).
///
/// - A `\|` escape is a literal pipe inside the cell, not a column break
///   (markdown pipe-table escaping); any other backslash stays literal.
/// - Pipes flush with the line edges are delimiters and do not create cells,
///   so `|| A | B ||` yields `["A", "B"]` with no phantom columns. The strip
///   stops at the first non-empty raw cell on each side, so an explicit
///   blank edge cell (`| | A |`) and interior empties (`| a | | b |`) are
///   preserved.
#[must_use]
pub(crate) fn split_table_row(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                current.push('|');
            }
            '|' => cells.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    cells.push(current);

    while cells.first().is_some_and(String::is_empty) {
        cells.remove(0);
    }
    while cells.last().is_some_and(String::is_empty) {
        cells.pop();
    }

    cells.iter().map(|c| c.trim().to_string()).collect()
}

/// Paints `s` when `colored` and a color is set; otherwise returns it plain.
fn paint(colored: bool, color: Option<Color>, s: &str) -> String {
    match (colored, color) {
        (true, Some(c)) => c.paint(s),
        _ => s.to_string(),
    }
}

pub struct TableRenderer<'a> {
    spec: &'a TableSpec,
    theme: Theme,
}

impl<'a> TableRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a TableSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[must_use]
    pub fn render(&self, colored: bool) -> String {
        let num_cols = self.spec.headers.len();
        if num_cols == 0 {
            return String::new();
        }

        let headers: Vec<_> = self.spec.headers.iter().map(|h| sanitize_cell(h)).collect();
        let rows: Vec<Vec<_>> = self
            .spec
            .rows
            .iter()
            .map(|row| row.iter().map(|c| sanitize_cell(c)).collect())
            .collect();

        let grid = self.spec.color;
        // Border glyphs only — cell text stays terminal-default
        let bar = || paint(colored, grid, &self.theme.vertical_line().to_string());

        // Calculate column widths
        let mut col_widths = vec![0; num_cols];

        for (i, h) in headers.iter().enumerate() {
            col_widths[i] = UnicodeWidthStr::width(h.as_ref());
        }

        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    let w = UnicodeWidthStr::width(cell.as_ref());
                    if w > col_widths[i] {
                        col_widths[i] = w;
                    }
                }
            }
        }

        // Add 2 padding spaces to each column width
        for w in &mut col_widths {
            *w += 2;
        }

        let mut lines = Vec::new();

        // Top line
        let mut top = String::new();
        top.push_str(&paint(
            colored,
            grid,
            &self.theme.top_left_corner().to_string(),
        ));
        for (i, &w) in col_widths.iter().enumerate() {
            for _ in 0..w {
                top.push_str(&paint(
                    colored,
                    grid,
                    &self.theme.horizontal_line().to_string(),
                ));
            }
            if i + 1 < num_cols {
                top.push_str(&paint(colored, grid, &self.theme.tee_down().to_string()));
            }
        }
        top.push_str(&paint(
            colored,
            grid,
            &self.theme.top_right_corner().to_string(),
        ));
        lines.push(top);

        // Header row
        let mut header_line = String::new();
        header_line.push_str(&bar());
        for (i, h) in headers.iter().enumerate() {
            let align = self
                .spec
                .alignments
                .get(i)
                .copied()
                .unwrap_or(TextAlign::Center);
            header_line.push_str(&Self::format_cell(h.as_ref(), col_widths[i], align));
            header_line.push_str(&bar());
        }
        lines.push(header_line);

        // Header separator
        let mut sep = String::new();
        sep.push_str(&paint(colored, grid, &self.theme.tee_right().to_string()));
        for (i, &w) in col_widths.iter().enumerate() {
            for _ in 0..w {
                sep.push_str(&paint(
                    colored,
                    grid,
                    &self.theme.horizontal_line().to_string(),
                ));
            }
            if i + 1 < num_cols {
                sep.push_str(&paint(colored, grid, &self.theme.cross().to_string()));
            }
        }
        sep.push_str(&paint(colored, grid, &self.theme.tee_left().to_string()));
        lines.push(sep);

        // Data rows
        for row in &rows {
            let mut row_line = String::new();
            row_line.push_str(&bar());
            for (i, &w) in col_widths.iter().enumerate() {
                let cell = row.get(i).map_or("", |c| c.as_ref());
                let align = self
                    .spec
                    .alignments
                    .get(i)
                    .copied()
                    .unwrap_or(TextAlign::Left);
                row_line.push_str(&Self::format_cell(cell, w, align));
                row_line.push_str(&bar());
            }
            lines.push(row_line);
        }

        // Bottom line
        let mut bottom = String::new();
        bottom.push_str(&paint(
            colored,
            grid,
            &self.theme.bottom_left_corner().to_string(),
        ));
        for (i, &w) in col_widths.iter().enumerate() {
            for _ in 0..w {
                bottom.push_str(&paint(
                    colored,
                    grid,
                    &self.theme.horizontal_line().to_string(),
                ));
            }
            if i + 1 < num_cols {
                bottom.push_str(&paint(colored, grid, &self.theme.tee_up().to_string()));
            }
        }
        bottom.push_str(&paint(
            colored,
            grid,
            &self.theme.bottom_right_corner().to_string(),
        ));
        lines.push(bottom);

        lines.join("\n")
    }

    fn format_cell(text: &str, width: usize, align: TextAlign) -> String {
        let text_w = UnicodeWidthStr::width(text);
        if text_w >= width {
            return [" ", text, " "].concat();
        }

        // Pad to exactly `width` display columns with a one-space gutter on the
        // side opposite the text (Left: leading, Right: trailing).
        let total_pad = width - text_w;
        let lead = match align {
            TextAlign::Left => 1,
            TextAlign::Right => total_pad - 1,
            TextAlign::Center => total_pad / 2,
        };
        let trail = total_pad - lead;

        let mut out = String::with_capacity(width);
        for _ in 0..lead {
            out.push(' ');
        }
        out.push_str(text);
        for _ in 0..trail {
            out.push(' ');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::BoxStyle;

    #[test]
    fn test_table_render() {
        let spec = TableSpec {
            style: BoxStyle::Rounded,
            color: None,
            headers: vec![
                "Service".to_string(),
                "Port".to_string(),
                "Status".to_string(),
            ],
            rows: vec![
                vec![
                    "API Gateway".to_string(),
                    "8080".to_string(),
                    "OK".to_string(),
                ],
                vec![
                    "Auth Service".to_string(),
                    "8081".to_string(),
                    "OK".to_string(),
                ],
            ],
            alignments: vec![TextAlign::Left, TextAlign::Right, TextAlign::Center],
        };

        let renderer = TableRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("API Gateway"));
        assert!(out.contains("8080"));
        assert!(out.contains("OK"));
        assert!(!out.contains('\u{1b}'), "plain table has no ANSI");
    }

    #[test]
    fn test_table_grid_color() {
        let spec = TableSpec {
            style: BoxStyle::Rounded,
            color: Some(crate::color::Color::Blue),
            headers: vec!["A".to_string(), "B".to_string()],
            rows: vec![vec!["1".to_string(), "2".to_string()]],
            alignments: vec![],
        };
        let renderer = TableRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let plain = renderer.render(false);
        let colored = renderer.render(true);
        assert_eq!(
            crate::color::Color::strip_ansi(&colored),
            plain,
            "color never shifts layout"
        );
        assert!(colored.contains("\u{1b}[34m"), "grid painted blue");
        assert!(!colored.contains("[34m1"), "cell text not painted");
    }

    #[test]
    fn test_table_sanitize_tab() {
        let spec = TableSpec {
            style: BoxStyle::Rounded,
            color: None,
            headers: vec!["Header".to_string()],
            rows: vec![vec!["A\tB".to_string()], vec!["A    B".to_string()]],
            alignments: vec![TextAlign::Left],
        };
        let renderer = TableRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("A    B"));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[3].len(), lines[4].len());
        assert_eq!(sanitize_cell("A\tB"), "A    B");
    }

    #[test]
    fn test_table_sanitize_ansi_escape() {
        let spec = TableSpec {
            style: BoxStyle::Rounded,
            color: None,
            headers: vec!["Status".to_string()],
            rows: vec![
                vec!["\x1b[31mAlert\x1b[0m".to_string()],
                vec!["Alert".to_string()],
            ],
            alignments: vec![TextAlign::Left],
        };
        let renderer = TableRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("Alert"));
        assert!(!out.contains("\x1b[31m"));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[3], lines[4],
            "ANSI row renders identically to plain row"
        );
        assert_eq!(sanitize_cell("\x1b[31mAlert\x1b[0m"), "Alert");
    }

    #[test]
    fn test_table_sanitize_control_chars() {
        let spec = TableSpec {
            style: BoxStyle::Rounded,
            color: None,
            headers: vec!["Sound".to_string()],
            rows: vec![vec!["\x07beep\0\r".to_string()], vec!["beep".to_string()]],
            alignments: vec![TextAlign::Left],
        };
        let renderer = TableRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[3], lines[4]);
        assert_eq!(sanitize_cell("\x07beep\0\r"), "beep");
    }

    #[test]
    fn test_table_clean_cells_unmodified() {
        let input = "Clean UTF-8 text: 🚀 (test)";
        let sanitized = sanitize_cell(input);
        assert!(matches!(sanitized, Cow::Borrowed(_)));
        assert_eq!(sanitized, input);
    }

    #[test]
    fn test_split_table_row_escape_and_edges() {
        use crate::table::split_table_row;

        // Plain rows keep their delimiter handling.
        assert_eq!(split_table_row("| a | b |"), vec!["a", "b"]);
        assert_eq!(split_table_row("a | b"), vec!["a", "b"]);

        // ST-01: `\|` is a literal pipe, not a column break.
        assert_eq!(split_table_row("| x \\| y | 2 |"), vec!["x | y", "2"]);
        assert_eq!(split_table_row("\\|"), vec!["|"]);

        // ST-03: doubled edge pipes are delimiters, not phantom columns.
        assert_eq!(split_table_row("|| A | B ||"), vec!["A", "B"]);

        // Interior empty cells are preserved.
        assert_eq!(split_table_row("| a | | b |"), vec!["a", "", "b"]);
        // An explicit blank edge cell (space between pipes) is preserved.
        assert_eq!(split_table_row("| | a |"), vec!["", "a"]);

        // Empty edge strips leave nothing.
        assert!(split_table_row("||").is_empty());
    }

    #[test]
    fn test_parse_table_dsl_escaped_pipe_cell() {
        use crate::parser::parse_table_dsl;
        let dsl = "table\n| A | B |\n| x \\| y | 2 |";
        let spec = parse_table_dsl(dsl, BoxStyle::Rounded).unwrap();
        if let crate::schema::DiagramSpec::Table(t) = spec {
            assert_eq!(t.headers, vec!["A", "B"]);
            assert_eq!(t.rows, vec![vec!["x | y".to_string(), "2".to_string()]]);
            let out =
                crate::table::TableRenderer::new(&t, Theme::new(BoxStyle::Rounded)).render(false);
            assert!(out.contains("x | y"), "literal pipe survives: {out:?}");
        } else {
            panic!("Expected table diagram");
        }
    }

    #[test]
    fn test_parse_table_dsl_doubled_edge_pipes() {
        use crate::parser::parse_table_dsl;
        let dsl = "table\n|| A | B ||\n| 1 | 2 |";
        let spec = parse_table_dsl(dsl, BoxStyle::Rounded).unwrap();
        if let crate::schema::DiagramSpec::Table(t) = spec {
            assert_eq!(t.headers, vec!["A", "B"]);
            assert_eq!(t.rows, vec![vec!["1".to_string(), "2".to_string()]]);
        } else {
            panic!("Expected table diagram");
        }
    }

    #[test]
    fn test_parse_table_dsl_extra_cell_errors() {
        use crate::parser::parse_table_dsl;
        let err =
            parse_table_dsl("table\n| A | B |\n| 1 | 2 | 3 |", BoxStyle::Rounded).unwrap_err();
        assert!(
            err.contains("3 cells") && err.contains("2 columns") && err.contains("line 3"),
            "error must name row/column counts: {err}"
        );
    }

    #[test]
    fn test_parse_table_dsl_multiline_cell_rejected() {
        use crate::parser::parse_table_dsl;
        let dsl = "table\n| A | B |\n| line1\nline2 | 2 |";
        let err = parse_table_dsl(dsl, BoxStyle::Rounded).unwrap_err();
        assert!(
            err.contains("must start with '|'") && err.contains("line 4"),
            "error must point at the split row: {err}"
        );
    }

    #[test]
    fn test_parse_table_dsl_separator_count_mismatch_errors() {
        use crate::parser::parse_table_dsl;
        let dsl = "table\n| A | B | C |\n| --- | --- |\n| 1 | 2 | 3 |";
        let err = parse_table_dsl(dsl, BoxStyle::Rounded).unwrap_err();
        assert!(
            err.contains("separator defines 2 columns") && err.contains("header has 3"),
            "error must name expected vs actual: {err}"
        );
    }
}

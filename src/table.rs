use crate::schema::{TableSpec, TextAlign};
use crate::theme::Theme;
use unicode_width::UnicodeWidthStr;

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
    pub fn render(&self) -> String {
        let num_cols = self.spec.headers.len();
        if num_cols == 0 {
            return String::new();
        }

        // Calculate column widths
        let mut col_widths = vec![0; num_cols];

        for (i, h) in self.spec.headers.iter().enumerate() {
            col_widths[i] = UnicodeWidthStr::width(h.as_str());
        }

        for row in &self.spec.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    let w = UnicodeWidthStr::width(cell.as_str());
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
        top.push(self.theme.top_left_corner());
        for (i, &w) in col_widths.iter().enumerate() {
            for _ in 0..w {
                top.push(self.theme.horizontal_line());
            }
            if i + 1 < num_cols {
                top.push(self.theme.tee_down());
            }
        }
        top.push(self.theme.top_right_corner());
        lines.push(top);

        // Header row
        let mut header_line = String::new();
        header_line.push(self.theme.vertical_line());
        for (i, h) in self.spec.headers.iter().enumerate() {
            let align = self
                .spec
                .alignments
                .get(i)
                .copied()
                .unwrap_or(TextAlign::Center);
            header_line.push_str(&Self::format_cell(h, col_widths[i], align));
            header_line.push(self.theme.vertical_line());
        }
        lines.push(header_line);

        // Header separator
        let mut sep = String::new();
        sep.push(self.theme.tee_right());
        for (i, &w) in col_widths.iter().enumerate() {
            for _ in 0..w {
                sep.push(self.theme.horizontal_line());
            }
            if i + 1 < num_cols {
                sep.push(self.theme.cross());
            }
        }
        sep.push(self.theme.tee_left());
        lines.push(sep);

        // Data rows
        for row in &self.spec.rows {
            let mut row_line = String::new();
            row_line.push(self.theme.vertical_line());
            for (i, &w) in col_widths.iter().enumerate() {
                let cell = row.get(i).map_or("", std::string::String::as_str);
                let align = self
                    .spec
                    .alignments
                    .get(i)
                    .copied()
                    .unwrap_or(TextAlign::Left);
                row_line.push_str(&Self::format_cell(cell, w, align));
                row_line.push(self.theme.vertical_line());
            }
            lines.push(row_line);
        }

        // Bottom line
        let mut bottom = String::new();
        bottom.push(self.theme.bottom_left_corner());
        for (i, &w) in col_widths.iter().enumerate() {
            for _ in 0..w {
                bottom.push(self.theme.horizontal_line());
            }
            if i + 1 < num_cols {
                bottom.push(self.theme.tee_up());
            }
        }
        bottom.push(self.theme.bottom_right_corner());
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
        let out = renderer.render();
        assert!(out.contains("API Gateway"));
        assert!(out.contains("8080"));
        assert!(out.contains("OK"));
    }
}

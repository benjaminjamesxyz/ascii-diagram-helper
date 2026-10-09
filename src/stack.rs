use std::borrow::Cow;

use crate::color::Color;
use crate::schema::{StackLayerSpec, StackSpec};
use crate::theme::Theme;
use unicode_width::UnicodeWidthStr;

/// Paints `s` when `colored` and a color is set; otherwise returns it plain.
fn paint(colored: bool, color: Option<Color>, s: &str) -> String {
    match (colored, color) {
        (true, Some(c)) => c.paint(s),
        _ => s.to_string(),
    }
}

/// True when `s` looks like a memory address or hex ID rather than prose:
/// a `0x`/`0X`-prefixed hex number or a bare all-hex token (`DEADBEEF`).
///
/// The stack DSL splits a layer line on the first `:` and shows the prefix
/// right-aligned in an address column; this heuristic gates that split so
/// prose like `Note: x` keeps the whole line as its label instead of
/// becoming a pseudo-address. Pure decimal is deliberately NOT accepted
/// (labels like `2: one` stay labels); the cost is that hex words such as
/// `Feed: x` are treated as addresses — accepted tradeoff to keep the
/// heuristic simple (ST-05).
pub(crate) fn is_address_shaped(s: &str) -> bool {
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Expands tabs to spaces on 4-column stops counted from the string start
/// (same convention as the table/sequence sanitizers). Raw tabs inside box
/// text make terminals reflow the line so the borders drift; expanding
/// before width computation keeps every row exactly `box_w` wide.
/// Tab-free strings incur zero allocations.
pub(crate) fn expand_tabs(s: &str) -> Cow<'_, str> {
    if !s.contains('\t') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == '\t' {
            let stop = (col / 4 + 1) * 4;
            while col < stop {
                out.push(' ');
                col += 1;
            }
        } else {
            out.push(ch);
            col += 1;
        }
    }
    Cow::Owned(out)
}

pub struct StackRenderer<'a> {
    spec: &'a StackSpec,
    theme: Theme,
}

impl<'a> StackRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a StackSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[allow(clippy::too_many_lines, reason = "single linear pass stacking layers")]
    pub fn render(&self, colored: bool) -> String {
        if self.spec.layers.is_empty() {
            return String::new();
        }

        // Expand tabs up front so width math, rendering, and terminals agree.
        let layers: Vec<StackLayerSpec> = self
            .spec
            .layers
            .iter()
            .map(|l| StackLayerSpec {
                label: expand_tabs(&l.label).into_owned(),
                address_or_id: l
                    .address_or_id
                    .as_deref()
                    .map(|a| expand_tabs(a).into_owned()),
                description: l
                    .description
                    .as_deref()
                    .map(|d| expand_tabs(d).into_owned()),
                color: l.color,
            })
            .collect();
        let bottom_address = self
            .spec
            .bottom_address
            .as_deref()
            .map(|a| expand_tabs(a).into_owned());
        let title = self
            .spec
            .title
            .as_deref()
            .map(|t| expand_tabs(t).into_owned());

        let num_layers = layers.len();

        // Find max address/prefix width
        let bot_addr_w = bottom_address.as_deref().map_or(0, UnicodeWidthStr::width);
        let max_layer_addr_w = layers
            .iter()
            .map(|l| l.address_or_id.as_deref().map_or(0, UnicodeWidthStr::width))
            .max()
            .unwrap_or(0);
        let addr_w = max_layer_addr_w.max(bot_addr_w);

        // Find max content width
        let content_w = layers
            .iter()
            .map(|l| {
                let lw = UnicodeWidthStr::width(l.label.as_str());
                let dw = l.description.as_deref().map_or(0, UnicodeWidthStr::width);
                lw.max(dw)
            })
            .max()
            .unwrap_or(20)
            .max(24);

        let box_w = content_w + 4; // 2 padding + 2 borders

        let mut lines = Vec::new();

        // Title
        if let Some(title) = &title {
            let tw = UnicodeWidthStr::width(title.as_str());
            let indent = addr_w + 1;
            let total_span = indent + box_w;
            let tx = if total_span > tw {
                (total_span - tw) / 2
            } else {
                indent
            };
            lines.push(format!("{}{}", " ".repeat(tx), title));
            lines.push(String::new());
        }

        let prefix_pad = |text: Option<&str>| -> String {
            match text {
                Some(s) => {
                    let w = UnicodeWidthStr::width(s);
                    let pad = addr_w.saturating_sub(w);
                    let mut out = String::with_capacity(addr_w);
                    for _ in 0..pad {
                        out.push(' ');
                    }
                    out.push_str(s);
                    out
                }
                None => " ".repeat(addr_w),
            }
        };

        // Top line (first layer's color owns it)
        let first_color = layers.first().and_then(|l| l.color);
        let first_addr = layers.first().and_then(|l| l.address_or_id.as_deref());
        let top_border = format!(
            "{} {}{}{}",
            prefix_pad(first_addr),
            paint(
                colored,
                first_color,
                &self.theme.top_left_corner().to_string()
            ),
            paint(
                colored,
                first_color,
                &self.theme.horizontal_line().to_string().repeat(box_w - 2)
            ),
            paint(
                colored,
                first_color,
                &self.theme.top_right_corner().to_string()
            )
        );
        lines.push(top_border);

        for (i, layer) in layers.iter().enumerate() {
            // Layer content
            let lbl_w = UnicodeWidthStr::width(layer.label.as_str());
            let pad = (box_w - 2).saturating_sub(lbl_w);
            let left_pad = pad / 2;
            let right_pad = pad - left_pad;

            let row = format!(
                "{} {}{}{}{}{}",
                " ".repeat(addr_w),
                paint(
                    colored,
                    layer.color,
                    &self.theme.vertical_line().to_string()
                ),
                " ".repeat(left_pad),
                layer.label,
                " ".repeat(right_pad),
                paint(
                    colored,
                    layer.color,
                    &self.theme.vertical_line().to_string()
                )
            );
            lines.push(row);

            if let Some(ref desc) = layer.description {
                let desc_w = UnicodeWidthStr::width(desc.as_str());
                let d_pad = (box_w - 2).saturating_sub(desc_w);
                let d_left = d_pad / 2;
                let d_right = d_pad - d_left;

                let desc_row = format!(
                    "{} {}{}{}{}{}",
                    " ".repeat(addr_w),
                    paint(
                        colored,
                        layer.color,
                        &self.theme.vertical_line().to_string()
                    ),
                    " ".repeat(d_left),
                    desc,
                    " ".repeat(d_right),
                    paint(
                        colored,
                        layer.color,
                        &self.theme.vertical_line().to_string()
                    )
                );
                lines.push(desc_row);
            }

            // Separator or bottom (upper layer owns its bottom edge)
            if i + 1 < num_layers {
                let next_addr = layers[i + 1].address_or_id.as_deref();
                let sep = format!(
                    "{} {}{}{}",
                    prefix_pad(next_addr),
                    paint(colored, layer.color, &self.theme.tee_right().to_string()),
                    paint(
                        colored,
                        layer.color,
                        &self.theme.horizontal_line().to_string().repeat(box_w - 2)
                    ),
                    paint(colored, layer.color, &self.theme.tee_left().to_string())
                );
                lines.push(sep);
            }
        }

        // Bottom line (last layer's color owns it)
        let last_color = layers.last().and_then(|l| l.color);
        let bottom_border = format!(
            "{} {}{}{}",
            prefix_pad(bottom_address.as_deref()),
            paint(
                colored,
                last_color,
                &self.theme.bottom_left_corner().to_string()
            ),
            paint(
                colored,
                last_color,
                &self.theme.horizontal_line().to_string().repeat(box_w - 2)
            ),
            paint(
                colored,
                last_color,
                &self.theme.bottom_right_corner().to_string()
            )
        );
        lines.push(bottom_border);

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::StackLayerSpec;
    use crate::theme::BoxStyle;

    #[test]
    fn test_stack_render() {
        let spec = StackSpec {
            style: BoxStyle::Rounded,
            title: Some("Process Memory Layout".to_string()),
            layers: vec![
                StackLayerSpec {
                    label: "Stack".to_string(),
                    address_or_id: Some("0xFFFF".to_string()),
                    description: Some("grows down".to_string()),
                    color: None,
                },
                StackLayerSpec {
                    label: "Heap".to_string(),
                    address_or_id: Some("0x1000".to_string()),
                    description: Some("grows up".to_string()),
                    color: None,
                },
                StackLayerSpec {
                    label: "Text / Code".to_string(),
                    address_or_id: Some("0x0000".to_string()),
                    description: None,
                    color: None,
                },
            ],
            bottom_address: None,
            grows_down: true,
        };

        let renderer = StackRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("Process Memory Layout"));
        assert!(out.contains("0xFFFF"));
        assert!(out.contains("Stack"));
        assert!(out.contains("Heap"));
        assert!(out.contains("0x0000"));
        assert!(!out.contains('\u{1b}'), "plain stack has no ANSI");
    }

    #[test]
    fn test_stack_layer_color() {
        let spec = StackSpec {
            style: BoxStyle::Sharp,
            title: None,
            layers: vec![
                StackLayerSpec {
                    label: "Kernel".to_string(),
                    address_or_id: Some("0xFFFF".to_string()),
                    description: None,
                    color: Some(crate::color::Color::Red),
                },
                StackLayerSpec {
                    label: "User".to_string(),
                    address_or_id: None,
                    description: None,
                    color: Some(crate::color::Color::Green),
                },
            ],
            bottom_address: Some("0x0000".to_string()),
            grows_down: true,
        };
        let renderer = StackRenderer::new(&spec, Theme::new(BoxStyle::Sharp));
        let plain = renderer.render(false);
        let colored = renderer.render(true);
        assert_eq!(
            crate::color::Color::strip_ansi(&colored),
            plain,
            "color never shifts layout"
        );
        assert!(colored.contains("\u{1b}[31m"), "kernel layer red");
        assert!(colored.contains("\u{1b}[32m"), "user layer green");
        assert!(!colored.contains("[31mKernel"), "label text not painted");
    }

    #[test]
    fn test_stack_dsl_inverted_parens() {
        use crate::parser::parse_stack_dsl;
        use crate::schema::DiagramSpec;
        let dsl = "stack\n0xFFFF: x) (y\nfoo) bar (baz\n0xC000: User Stack (grows down)\n0x0000:";
        let spec = parse_stack_dsl(dsl, BoxStyle::Rounded).unwrap();
        if let DiagramSpec::Stack(s) = spec {
            assert_eq!(s.layers[0].label, "x) (y");
            assert_eq!(s.layers[0].description, None);
            assert_eq!(s.layers[1].label, "foo) bar (baz");
            assert_eq!(s.layers[1].description, None);
            assert_eq!(s.layers[2].label, "User Stack");
            assert_eq!(s.layers[2].description.as_deref(), Some("grows down"));
            assert_eq!(s.bottom_address.as_deref(), Some("0x0000"));

            let renderer = StackRenderer::new(&s, Theme::new(BoxStyle::Rounded));
            let out = renderer.render(false);
            assert!(out.contains("x) (y"));
            assert!(out.contains("foo) bar (baz"));
            assert!(out.contains("User Stack"));
        } else {
            panic!("Expected stack diagram");
        }
    }

    #[test]
    fn test_stack_dsl_empty_and_bare_address_errors() {
        use crate::parser::parse_stack_dsl;
        assert_eq!(
            parse_stack_dsl("stack", BoxStyle::Rounded).unwrap_err(),
            "No valid stack layers found"
        );
        assert_eq!(
            parse_stack_dsl("stack\n0x0000:", BoxStyle::Rounded).unwrap_err(),
            "No valid stack layers found"
        );
        assert_eq!(
            parse_stack_dsl("stack\n0xFFFF:\n0x8000: App", BoxStyle::Rounded).unwrap_err(),
            "Stack layer label cannot be empty"
        );
        assert_eq!(
            parse_stack_dsl(
                "stack\n0xFFFF: Top\n0x8000:\n0x0000: Bottom",
                BoxStyle::Rounded
            )
            .unwrap_err(),
            "Stack layer label cannot be empty"
        );
    }

    #[test]
    fn test_stack_address_shaped_tokens() {
        assert!(is_address_shaped("0xFFFF"));
        assert!(is_address_shaped("0x7fff5fbff8c0"));
        assert!(is_address_shaped("0X10"));
        assert!(is_address_shaped("DEADBEEF"));
        assert!(is_address_shaped("ff00"));
        assert!(!is_address_shaped("Note"));
        assert!(!is_address_shaped("base_pointer"));
        assert!(!is_address_shaped(""));
        assert!(!is_address_shaped("0x"));
        assert!(!is_address_shaped("Stack (grows)"));
    }

    #[test]
    fn test_stack_dsl_prose_colon_stays_label() {
        use crate::parser::parse_stack_dsl;
        use crate::schema::DiagramSpec;

        // ST-05: prose containing a colon is a label, not `address: label`.
        let spec = parse_stack_dsl("stack\nNote: x", BoxStyle::Rounded).unwrap();
        if let DiagramSpec::Stack(s) = spec {
            assert_eq!(s.layers.len(), 1);
            assert_eq!(s.layers[0].label, "Note: x");
            assert_eq!(s.layers[0].address_or_id, None);
            let out = StackRenderer::new(&s, Theme::new(BoxStyle::Rounded)).render(false);
            assert!(out.contains("Note: x"));
            // No address column anywhere: the top border starts at column 0
            // (single leading gutter space only).
            let first = out.lines().next().expect("non-empty render");
            assert!(first.starts_with(" ╭"), "no address gutter: {first:?}");
        } else {
            panic!("Expected stack diagram");
        }

        // Address-shaped prefixes keep the address column.
        let spec = parse_stack_dsl("stack\n0xFF: Top\nDEADBEEF: magic", BoxStyle::Rounded).unwrap();
        if let DiagramSpec::Stack(s) = spec {
            assert_eq!(s.layers[0].address_or_id.as_deref(), Some("0xFF"));
            assert_eq!(s.layers[1].address_or_id.as_deref(), Some("DEADBEEF"));
            assert_eq!(s.layers[1].label, "magic");
        } else {
            panic!("Expected stack diagram");
        }

        // A non-address-shaped trailing `word:` line becomes a layer, not a
        // bottom address.
        let spec = parse_stack_dsl("stack\n0xFF: Top\nNote:", BoxStyle::Rounded).unwrap();
        if let DiagramSpec::Stack(s) = spec {
            assert_eq!(s.bottom_address, None);
            assert_eq!(s.layers.len(), 2);
            assert_eq!(s.layers[1].label, "Note:");
        } else {
            panic!("Expected stack diagram");
        }

        // Shaped trailing `0x0:` still becomes the bottom address.
        let spec = parse_stack_dsl("stack\n0xFF: Top\n0x0000:", BoxStyle::Rounded).unwrap();
        if let DiagramSpec::Stack(s) = spec {
            assert_eq!(s.bottom_address.as_deref(), Some("0x0000"));
            assert_eq!(s.layers.len(), 1);
        } else {
            panic!("Expected stack diagram");
        }
    }

    #[test]
    fn test_expand_tabs_stops() {
        assert!(matches!(expand_tabs("clean"), Cow::Borrowed("clean")));
        assert_eq!(expand_tabs("a\tb"), "a   b");
        assert_eq!(expand_tabs("\t\tx"), "        x");
        assert_eq!(expand_tabs(""), "");
    }

    #[test]
    fn test_stack_tab_in_label_expands() {
        let spec = StackSpec {
            style: BoxStyle::Rounded,
            title: Some("t\ttle".to_string()),
            layers: vec![StackLayerSpec {
                label: "push a\tb".to_string(),
                address_or_id: None,
                description: None,
                color: None,
            }],
            bottom_address: None,
            grows_down: true,
        };
        let out = StackRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(!out.contains('\t'), "raw tab must not survive: {out:?}");
        assert!(
            out.contains("push a  b"),
            "tab expands to 4-col stop: {out:?}"
        );
        // Box lines (top border, rows, bottom border) share one DISPLAY
        // width, so borders cannot drift; the centered title and its blank
        // spacer are free-standing. (Byte lengths differ regardless: borders
        // are multi-byte glyphs.)
        let widths: Vec<usize> = out.lines().map(UnicodeWidthStr::width).collect();
        assert!(
            widths.iter().skip(2).all(|&w| w == widths[2]),
            "widths: {widths:?}"
        );
    }
}

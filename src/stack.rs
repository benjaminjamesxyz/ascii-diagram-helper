use crate::schema::StackSpec;
use crate::theme::Theme;
use unicode_width::UnicodeWidthStr;

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
    pub fn render(&self) -> String {
        if self.spec.layers.is_empty() {
            return String::new();
        }

        let num_layers = self.spec.layers.len();

        // Find max address/prefix width
        let bot_addr_w = self
            .spec
            .bottom_address
            .as_deref()
            .map_or(0, UnicodeWidthStr::width);
        let max_layer_addr_w = self
            .spec
            .layers
            .iter()
            .map(|l| l.address_or_id.as_deref().map_or(0, UnicodeWidthStr::width))
            .max()
            .unwrap_or(0);
        let addr_w = max_layer_addr_w.max(bot_addr_w);

        // Find max content width
        let content_w = self
            .spec
            .layers
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
        if let Some(ref title) = self.spec.title {
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

        // Top line
        let first_addr = self
            .spec
            .layers
            .first()
            .and_then(|l| l.address_or_id.as_deref());
        let top_border = format!(
            "{} {}{}{}",
            prefix_pad(first_addr),
            self.theme.top_left_corner(),
            self.theme.horizontal_line().to_string().repeat(box_w - 2),
            self.theme.top_right_corner()
        );
        lines.push(top_border);

        for (i, layer) in self.spec.layers.iter().enumerate() {
            // Layer content
            let lbl_w = UnicodeWidthStr::width(layer.label.as_str());
            let pad = (box_w - 2).saturating_sub(lbl_w);
            let left_pad = pad / 2;
            let right_pad = pad - left_pad;

            let row = format!(
                "{} {}{}{}{}{}",
                " ".repeat(addr_w),
                self.theme.vertical_line(),
                " ".repeat(left_pad),
                layer.label,
                " ".repeat(right_pad),
                self.theme.vertical_line()
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
                    self.theme.vertical_line(),
                    " ".repeat(d_left),
                    desc,
                    " ".repeat(d_right),
                    self.theme.vertical_line()
                );
                lines.push(desc_row);
            }

            // Separator or bottom
            if i + 1 < num_layers {
                let next_addr = self.spec.layers[i + 1].address_or_id.as_deref();
                let sep = format!(
                    "{} {}{}{}",
                    prefix_pad(next_addr),
                    self.theme.tee_right(),
                    self.theme.horizontal_line().to_string().repeat(box_w - 2),
                    self.theme.tee_left()
                );
                lines.push(sep);
            }
        }

        // Bottom line
        let bottom_border = format!(
            "{} {}{}{}",
            prefix_pad(self.spec.bottom_address.as_deref()),
            self.theme.bottom_left_corner(),
            self.theme.horizontal_line().to_string().repeat(box_w - 2),
            self.theme.bottom_right_corner()
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
                },
                StackLayerSpec {
                    label: "Heap".to_string(),
                    address_or_id: Some("0x1000".to_string()),
                    description: Some("grows up".to_string()),
                },
                StackLayerSpec {
                    label: "Text / Code".to_string(),
                    address_or_id: Some("0x0000".to_string()),
                    description: None,
                },
            ],
            bottom_address: None,
            grows_down: true,
        };

        let renderer = StackRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render();
        assert!(out.contains("Process Memory Layout"));
        assert!(out.contains("0xFFFF"));
        assert!(out.contains("Stack"));
        assert!(out.contains("Heap"));
        assert!(out.contains("0x0000"));
    }
}

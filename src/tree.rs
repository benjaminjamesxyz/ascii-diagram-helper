use crate::color::Color;
use crate::schema::{TreeNodeSpec, TreeSpec};
use crate::theme::{BoxStyle, Theme};

/// Paints `s` when `colored` and a color is set; otherwise returns it plain.
fn paint(colored: bool, color: Option<Color>, s: &str) -> String {
    match (colored, color) {
        (true, Some(c)) => c.paint(s),
        _ => s.to_string(),
    }
}

pub struct TreeRenderer<'a> {
    spec: &'a TreeSpec,
    theme: Theme,
}

impl<'a> TreeRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a TreeSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[must_use]
    pub fn render(&self, colored: bool) -> String {
        let mut lines = Vec::new();

        let root_label = match &self.spec.root.annotation {
            Some(ann) => format!("{} ({})", self.spec.root.name, ann),
            None => self.spec.root.name.clone(),
        };
        lines.push(root_label);

        let num_children = self.spec.root.children.len();
        for (i, child) in self.spec.root.children.iter().enumerate() {
            let is_last = i + 1 == num_children;
            self.render_node(
                child,
                "",
                self.spec.root.color,
                is_last,
                colored,
                &mut lines,
            );
        }

        lines.join("\n")
    }

    fn render_node(
        &self,
        node: &TreeNodeSpec,
        prefix: &str,
        inherited: Option<Color>,
        is_last: bool,
        colored: bool,
        lines: &mut Vec<String>,
    ) {
        let is_ascii = self.theme.box_style == BoxStyle::Ascii;
        // A node's own color overrides the ancestor's for its subtree glyphs
        let effective = node.color.or(inherited);

        let branch = if is_ascii {
            if is_last { "\\-- " } else { "|-- " }
        } else if is_last {
            "└── "
        } else {
            "├── "
        };

        let label = match &node.annotation {
            Some(ann) => format!(
                "{}{}{} ({})",
                prefix,
                paint(colored, effective, branch),
                node.name,
                ann
            ),
            None => format!(
                "{}{}{}",
                prefix,
                paint(colored, effective, branch),
                node.name
            ),
        };
        lines.push(label);

        let child_prefix = if is_ascii {
            if is_last { "    " } else { "|   " }
        } else if is_last {
            "    "
        } else {
            "│   "
        };
        let new_prefix = format!("{prefix}{}", paint(colored, effective, child_prefix));

        let num_children = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i + 1 == num_children;
            self.render_node(child, &new_prefix, effective, child_is_last, colored, lines);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_rendering() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "src/".to_string(),
                annotation: None,
                color: None,
                children: vec![
                    TreeNodeSpec {
                        name: "main.rs".to_string(),
                        annotation: Some("entry point".to_string()),
                        color: None,
                        children: vec![],
                    },
                    TreeNodeSpec {
                        name: "canvas.rs".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    },
                ],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("src/"));
        assert!(out.contains("├── main.rs (entry point)"));
        assert!(out.contains("└── canvas.rs"));
        assert!(!out.contains('\u{1b}'), "plain tree has no ANSI");
    }

    #[test]
    fn test_tree_color_inheritance() {
        let spec = TreeSpec {
            style: BoxStyle::Sharp,
            root: TreeNodeSpec {
                name: "root".to_string(),
                annotation: None,
                color: None,
                children: vec![TreeNodeSpec {
                    name: "dir".to_string(),
                    annotation: None,
                    color: Some(Color::Red),
                    children: vec![TreeNodeSpec {
                        name: "leaf".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    }],
                }],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Sharp));
        let plain = renderer.render(false);
        let colored = renderer.render(true);
        assert_eq!(
            crate::color::Color::strip_ansi(&colored),
            plain,
            "color never shifts layout"
        );
        // leaf inherits red from "dir": its branch glyph is painted
        // (prefix and branch are separate paint runs — reset between them)
        assert!(
            colored.contains("\u{1b}[31m└── \u{1b}[0mleaf"),
            "inherited color: {colored:?}"
        );
    }
}

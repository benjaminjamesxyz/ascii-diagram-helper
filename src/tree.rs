use crate::schema::{TreeNodeSpec, TreeSpec};
use crate::theme::{BoxStyle, Theme};

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
    pub fn render(&self) -> String {
        let mut lines = Vec::new();

        let root_label = match &self.spec.root.annotation {
            Some(ann) => format!("{} ({})", self.spec.root.name, ann),
            None => self.spec.root.name.clone(),
        };
        lines.push(root_label);

        let num_children = self.spec.root.children.len();
        for (i, child) in self.spec.root.children.iter().enumerate() {
            let is_last = i + 1 == num_children;
            self.render_node(child, "", is_last, &mut lines);
        }

        lines.join("\n")
    }

    fn render_node(
        &self,
        node: &TreeNodeSpec,
        prefix: &str,
        is_last: bool,
        lines: &mut Vec<String>,
    ) {
        let is_ascii = self.theme.box_style == BoxStyle::Ascii;

        let branch = if is_ascii {
            if is_last { "\\-- " } else { "|-- " }
        } else if is_last {
            "└── "
        } else {
            "├── "
        };

        let label = match &node.annotation {
            Some(ann) => format!("{}{}{} ({})", prefix, branch, node.name, ann),
            None => format!("{}{}{}", prefix, branch, node.name),
        };
        lines.push(label);

        let child_prefix = if is_ascii {
            if is_last { "    " } else { "|   " }
        } else if is_last {
            "    "
        } else {
            "│   "
        };
        let new_prefix = format!("{prefix}{child_prefix}");

        let num_children = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i + 1 == num_children;
            self.render_node(child, &new_prefix, child_is_last, lines);
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
                children: vec![
                    TreeNodeSpec {
                        name: "main.rs".to_string(),
                        annotation: Some("entry point".to_string()),
                        children: vec![],
                    },
                    TreeNodeSpec {
                        name: "canvas.rs".to_string(),
                        annotation: None,
                        children: vec![],
                    },
                ],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render();
        assert!(out.contains("src/"));
        assert!(out.contains("├── main.rs (entry point)"));
        assert!(out.contains("└── canvas.rs"));
    }
}
